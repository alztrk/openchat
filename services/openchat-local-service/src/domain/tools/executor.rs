//! Coordinates model-requested tools, permission rules, filesystem scopes, and per-turn limits.
//!
//! Execution delegates input validation and each operation to focused modules; permission mode
//! and project policy are checked before any operation is run.

use crate::{
    chat_operation::SubagentModelOption,
    chatgpt::{
        ChatGptService, ImageGenerationAuth, ImageGenerationRequest, ImageOutputFormat,
        ImageQuality,
    },
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{
        ChatStreamEvent, ChatStreamSnapshot, ToolActivity, ToolCall, ToolDefinition, ToolResult,
    },
    storage::AppStorage,
    user_question_broker::UserQuestionBroker,
};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex as StdMutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

mod execution;
mod paths;
mod policy;
mod validation;

#[cfg(test)]
pub(crate) use execution::execute_model_tool;
pub(crate) use execution::execute_model_tool_with_workspace_roots;
use paths::qualify_output_paths;
pub use policy::*;
pub(crate) use validation::{PreparedToolCall, ToolOperation};

fn save_project_memory(storage: &AppStorage, conversation_id: &str, note: &str) -> Value {
    let mut connection = match storage.connect() {
        Ok(connection) => connection,
        Err(_) => {
            return tool_error(
                "project_memory_unavailable",
                "Project memory could not be saved locally.",
            );
        }
    };
    let project_id = match connection
        .query_row(
            "SELECT project_id FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
    {
        Ok(Some(Some(project_id))) => project_id,
        Ok(_) => {
            return tool_error(
                "project_required",
                "Project notes can only be saved from a conversation attached to a saved project.",
            );
        }
        Err(_) => {
            return tool_error(
                "project_memory_unavailable",
                "Project memory could not be saved locally.",
            );
        }
    };
    match crate::project_memory::add_assistant(&mut connection, &project_id, conversation_id, note)
    {
        Ok(snapshot) => json!({
            "saved": true,
            "noteCount": snapshot.items.len(),
        }),
        Err(error) => tool_error(error.code, &error.message),
    }
}

fn save_personal_memory(storage: &AppStorage, conversation_id: &str, fact: &str) -> Value {
    let connection = match storage.connect() {
        Ok(connection) => connection,
        Err(_) => {
            return tool_error(
                "personal_memory_unavailable",
                "Personal memory could not be saved locally.",
            );
        }
    };
    let enabled_json = match connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = 'privacy.personal_memory_enabled'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(_) => {
            return tool_error(
                "personal_memory_unavailable",
                "Personal memory settings could not be read.",
            );
        }
    };
    let Some(enabled_json) = enabled_json else {
        return tool_error(
            "personal_memory_disabled",
            "Personal memory is turned off in Settings.",
        );
    };
    if !matches!(serde_json::from_str::<bool>(&enabled_json), Ok(true)) {
        return tool_error(
            "personal_memory_disabled",
            "Personal memory is turned off in Settings.",
        );
    }
    let fact_count =
        match connection.query_row("SELECT COUNT(*) FROM personal_memory_facts", [], |row| {
            row.get::<_, i64>(0)
        }) {
            Ok(count) => count,
            Err(_) => {
                return tool_error(
                    "personal_memory_unavailable",
                    "Personal memory could not be read locally.",
                );
            }
        };
    if fact_count >= 100 {
        return tool_error(
            "personal_memory_limit_reached",
            "Personal memory is full. Remove an existing fact in Settings before saving another.",
        );
    }
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => match i64::try_from(duration.as_millis()) {
            Ok(value) => value,
            Err(_) => {
                return tool_error(
                    "personal_memory_unavailable",
                    "Personal memory could not be saved locally.",
                );
            }
        },
        Err(_) => {
            return tool_error(
                "personal_memory_unavailable",
                "Personal memory could not be saved locally.",
            );
        }
    };
    let fact_id = Uuid::new_v4().simple().to_string();
    match connection.execute(
        "INSERT INTO personal_memory_facts
         (id, fact_text, category, source_conversation_id, created_at, updated_at)
         VALUES (?1, ?2, NULL, ?3, ?4, ?4)",
        rusqlite::params![fact_id, fact, conversation_id, now],
    ) {
        Ok(_) => json!({"saved": true, "factId": fact_id, "fact": fact}),
        Err(_) => tool_error(
            "personal_memory_unavailable",
            "Personal memory could not be saved locally.",
        ),
    }
}

pub(crate) enum ImageGenerationContext<'a> {
    ChatGptOAuth {
        service: &'a ChatGptService,
        connection_id: &'a str,
        workspace_id: &'a str,
        external_workspace_id: &'a str,
        model_id: &'a str,
        reasoning_effort: Option<String>,
        fast_mode: bool,
        turn_id: Option<&'a str>,
    },
    ApiKey {
        service: &'a ChatGptService,
        api_key: &'a str,
        model_id: &'a str,
        reasoning_effort: Option<String>,
    },
    CompatibleApiKey {
        api_key: &'a str,
        provider_id: &'a str,
        endpoint: &'a str,
        model_id: &'a str,
        reasoning_effort: Option<String>,
    },
    LocalEndpoint {
        api_key: Option<&'a str>,
        provider_id: &'a str,
        endpoint: &'a str,
        model_id: &'a str,
        reasoning_effort: Option<String>,
    },
}

/// Carries the request-scoped services and metadata needed to execute one tool call.
pub(crate) struct ToolExecutionContext<'a, 'image> {
    /// Permission broker that must approve operations requiring user consent.
    pub(crate) permissions: &'a ToolPermissionBroker,
    /// Request identifier used to correlate emitted tool events.
    pub(crate) request_id: &'a Value,
    /// Current assistant message snapshot attached to tool events.
    pub(crate) snapshot: &'a ChatStreamSnapshot,
    /// Event sink used to publish tool state changes.
    pub(crate) events: &'a EventSink,
    /// Cancellation receiver shared with the running model request.
    pub(crate) cancellation: &'a mut watch::Receiver<bool>,
    /// Persistent storage used for tool checkpoints and generated files.
    pub(crate) storage: &'a AppStorage,
    /// Durable run identifier used for user questions and delegated work.
    pub(crate) run_id: &'a str,
    /// Provider identifier for provider-specific tool execution behavior.
    pub(crate) provider_id: &'a str,
    /// Broker used to pause execution for user questions.
    pub(crate) user_questions: &'a UserQuestionBroker,
    /// Provider-specific credentials and settings for image generation.
    pub(crate) image_generation: Option<&'a ImageGenerationContext<'image>>,
}

struct ToolEventContext<'a> {
    request_id: &'a Value,
    snapshot: &'a ChatStreamSnapshot,
    events: &'a EventSink,
    cancellation: &'a mut watch::Receiver<bool>,
    storage: &'a AppStorage,
}

impl ToolPermissionMode {
    fn requires_approval(self, operation: &ToolOperation) -> bool {
        match self {
            Self::Plan => true,
            Self::RequireApproval => true,
            Self::ApproveSafeOperations => !operation.is_safe_for_auto_approval(),
            Self::FullAccess => false,
        }
    }
}

/// Owns the request-scoped state used to validate and execute model tool calls.
pub struct ToolExecutor {
    project_root: Option<PathBuf>,
    project_memory_context: Option<String>,
    project_roots: Vec<PathBuf>,
    security_scan_scope_only: bool,
    security_scan_public_web_research: bool,
    data_root: PathBuf,
    permission_mode: ToolPermissionMode,
    turn_state: StdMutex<ToolTurnState>,
    tool_activities: Mutex<Vec<ToolActivity>>,
    allowed_tool_names: HashSet<String>,
    permission_rules: ToolPermissionRules,
    #[cfg(windows)]
    computer_use: super::computer_use::ComputerUseSession,
    #[cfg(windows)]
    computer_use_approved: StdMutex<bool>,
    #[cfg(windows)]
    computer_use_images: Mutex<HashMap<String, String>>,
    chat_service: Option<ChatGptService>,
    subagent_models: Vec<SubagentModelOption>,
    subagent_tool_definitions: Vec<ToolDefinition>,
    #[cfg(windows)]
    project_tools: Vec<super::project_tools::ProjectTool>,
    #[cfg(windows)]
    mcp_registry: Option<std::sync::Arc<super::mcp::McpRegistry>>,
}

const MAX_DENIED_UNADVERTISED_TOOL_CALLS_PER_TURN: usize = 8;

#[derive(Default)]
struct ToolTurnState {
    round_sequence: usize,
    current_round_id: Option<String>,
    project_rule_prechecks: HashSet<String>,
    unadvertised_tool_denials: usize,
}

enum PendingFileChangeCapture {
    File {
        directory: std::sync::Arc<crate::filesystem::OpenedDirectory>,
        path: String,
        before: Result<
            (
                crate::file_changes::DirectoryIdentity,
                Option<crate::file_changes::FileSnapshot>,
            ),
            crate::file_changes::TrackingError,
        >,
        target: crate::filesystem::OpenedFile,
    },
    Workspace {
        before: Vec<(
            PathBuf,
            Result<crate::file_changes::WorkspaceSnapshot, crate::file_changes::TrackingError>,
        )>,
    },
    None,
}

pub(crate) fn tool_response_limit_error() -> ServiceError {
    ServiceError::new(
        "tool_response_limit",
        "A single model response requested more than the supported number of tool calls.",
        false,
    )
}

impl ToolExecutor {
    #[cfg(test)]
    pub fn new(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
    ) -> Self {
        Self::with_allowed_tool_names(
            project_root,
            data_root,
            permission_mode,
            crate::tools::definitions()
                .into_iter()
                .map(|tool| tool.name.to_owned()),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_allowed_tool_names(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
        allowed_tool_names: impl IntoIterator<Item = String>,
    ) -> Self {
        Self::with_permission_rules(
            project_root,
            data_root,
            permission_mode,
            allowed_tool_names,
            ToolPermissionRules::new(),
        )
    }

    pub(crate) fn with_permission_rules(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
        allowed_tool_names: impl IntoIterator<Item = String>,
        permission_rules: ToolPermissionRules,
    ) -> Self {
        Self {
            project_root: project_root.map(Path::to_path_buf),
            project_memory_context: None,
            project_roots: project_root.map(Path::to_path_buf).into_iter().collect(),
            security_scan_scope_only: false,
            security_scan_public_web_research: false,
            data_root: data_root.to_path_buf(),
            permission_mode,
            turn_state: StdMutex::new(ToolTurnState::default()),
            tool_activities: Mutex::new(Vec::new()),
            allowed_tool_names: allowed_tool_names.into_iter().collect(),
            permission_rules,
            #[cfg(windows)]
            computer_use: super::computer_use::ComputerUseSession::default(),
            #[cfg(windows)]
            computer_use_approved: StdMutex::new(false),
            #[cfg(windows)]
            computer_use_images: Mutex::new(HashMap::new()),
            chat_service: None,
            subagent_models: Vec::new(),
            subagent_tool_definitions: Vec::new(),
            #[cfg(windows)]
            project_tools: Vec::new(),
            #[cfg(windows)]
            mcp_registry: None,
        }
    }

    pub(crate) fn with_workspace_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.project_roots = roots;
        self
    }

    pub(crate) fn with_security_scan_scope(
        mut self,
        enabled: bool,
        public_web_research: bool,
    ) -> Self {
        self.security_scan_scope_only = enabled;
        self.security_scan_public_web_research = enabled && public_web_research;
        if enabled {
            // Security scans expose only read-only tools, so they can run independently of the
            // user's normal plan/approval mode while the path resolver enforces selected roots.
            self.permission_mode = ToolPermissionMode::FullAccess;
            self.allowed_tool_names =
                crate::security_scan::allowed_tool_names(self.security_scan_public_web_research);
        }
        self
    }

    /// Adds the parent run's bounded project-memory snapshot to delegated child runs.
    pub(crate) fn with_project_memory_context(mut self, context: Option<String>) -> Self {
        self.project_memory_context = context;
        self
    }

    pub(crate) fn with_subagent_models(mut self, models: Vec<SubagentModelOption>) -> Self {
        self.subagent_models = models;
        self
    }

    pub(crate) fn with_chat_service(mut self, service: ChatGptService) -> Self {
        self.chat_service = Some(service);
        self
    }

    pub(crate) fn with_subagent_tool_definitions(
        mut self,
        definitions: Vec<ToolDefinition>,
    ) -> Self {
        self.subagent_tool_definitions = definitions;
        self
    }

    pub(crate) fn child_executor(
        &self,
        supports_tools: Option<bool>,
        supports_image_generation: bool,
    ) -> Self {
        let mut definitions = self
            .subagent_tool_definitions
            .iter()
            .filter(|definition| {
                !matches!(
                    definition.name.as_str(),
                    "delegate_task"
                        | "computer_use"
                        | "list_subagent_models"
                        | "get_subagent_result"
                        | "cancel_subagent"
                        | "save_memory"
                        | "save_project_memory"
                        | "ask_user"
                        | crate::goals::START_GOAL_TOOL_NAME
                        | crate::goals::CONTROL_TOOL_NAME
                        | crate::goals::STOP_GOAL_TOOL_NAME
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if supports_tools == Some(false) {
            definitions.clear();
        } else if !supports_image_generation {
            definitions.retain(|definition| definition.name != "generate_image");
        }
        let mut executor = Self::with_permission_rules(
            self.project_root.as_deref(),
            &self.data_root,
            self.permission_mode,
            definitions.iter().map(|definition| definition.name.clone()),
            self.permission_rules.clone(),
        )
        .with_workspace_roots(self.project_roots.clone())
        .with_security_scan_scope(
            self.security_scan_scope_only,
            self.security_scan_public_web_research,
        )
        .with_project_memory_context(self.project_memory_context.clone())
        .with_subagent_tool_definitions(definitions);
        if let Some(service) = self.chat_service.clone() {
            executor = executor.with_chat_service(service);
        }
        #[cfg(windows)]
        {
            executor = executor
                .with_project_tools(self.project_tools.clone())
                .with_mcp_registry(self.mcp_registry.clone());
        }
        executor
    }

    pub(crate) fn subagent_tool_definitions(&self) -> &[ToolDefinition] {
        &self.subagent_tool_definitions
    }

    pub(crate) async fn take_computer_use_image(&self, call_id: &str) -> Option<String> {
        #[cfg(windows)]
        {
            self.computer_use_images.lock().await.remove(call_id)
        }
        #[cfg(not(windows))]
        {
            let _ = call_id;
            None
        }
    }

    #[cfg(windows)]
    async fn execute_computer_use_call(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        context: ToolEventContext<'_>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolEventContext {
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
        } = context;
        let mut activity_call = call.clone();
        if let Some(arguments) = activity_call.arguments.as_object_mut()
            && arguments.contains_key("text")
        {
            arguments.insert("text".to_owned(), json!("[redacted]"));
        }
        let was_approved = *self
            .computer_use_approved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !was_approved {
            if self.permission_rules.get("computer_use") == Some(&ToolPermissionRule::Deny) {
                let output = tool_error(
                    "permission_denied",
                    "Computer Use is denied by the active tool permission rules.",
                );
                self.emit_activity(
                    storage,
                    ToolActivity::denied(&activity_call, "desktop".to_owned(), output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
            let requires_approval = self.permission_rules.get("computer_use")
                == Some(&ToolPermissionRule::Ask)
                || (self.permission_rules.get("computer_use") != Some(&ToolPermissionRule::Allow)
                    && self.permission_mode != ToolPermissionMode::FullAccess);
            if requires_approval {
                self.emit_activity(
                    storage,
                    ToolActivity::awaiting_approval(&activity_call, "desktop".to_owned()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                let approved = permissions
                    .request_approval(
                        request_id,
                        "computer_use",
                        "desktop",
                        &call.arguments,
                        events,
                        cancellation,
                    )
                    .await;
                let approved = match approved {
                    Ok(approved) => approved,
                    Err(error) if error.code == "operation_cancelled" => {
                        let output = tool_error("operation_cancelled", "The request was stopped.");
                        self.emit_activity(
                            storage,
                            ToolActivity::cancelled(&activity_call, "desktop".to_owned(), output),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                        return Err(error);
                    }
                    Err(error) => return Err(error),
                };
                if !approved {
                    let output = tool_error(
                        "permission_denied",
                        "Computer Use was not approved. Do not retry unless the user asks.",
                    );
                    self.emit_activity(
                        storage,
                        ToolActivity::denied(&activity_call, "desktop".to_owned(), output.clone()),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Ok(ToolResult {
                        call_id: call.id.clone(),
                        output,
                    });
                }
            }
            *self
                .computer_use_approved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        }
        if *cancellation.borrow() {
            return Err(crate::permissions::operation_cancelled_error());
        }
        self.emit_activity(
            storage,
            ToolActivity::running(&activity_call, Some("desktop".to_owned())),
            request_id,
            snapshot,
            events,
        )
        .await?;
        let result = self.computer_use.execute(&call.arguments);
        if let Some(image) = result.image {
            self.computer_use_images
                .lock()
                .await
                .insert(call.id.clone(), image);
        }
        let activity = ToolActivity::finished(
            &activity_call,
            Some("desktop".to_owned()),
            result.output.clone(),
        );
        self.emit_activity(storage, activity, request_id, snapshot, events)
            .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output: result.output,
        })
    }

    #[cfg(windows)]
    pub(crate) fn with_project_tools(
        mut self,
        project_tools: Vec<super::project_tools::ProjectTool>,
    ) -> Self {
        self.project_tools = project_tools;
        self
    }

    #[cfg(windows)]
    pub(crate) fn with_mcp_registry(
        mut self,
        registry: Option<std::sync::Arc<super::mcp::McpRegistry>>,
    ) -> Self {
        self.mcp_registry = registry;
        self
    }

    fn validate_tool_name(&self, tool_name: &str) -> Result<String, ServiceError> {
        if self.allowed_tool_names.contains(tool_name) {
            return Ok(tool_name.to_owned());
        }
        if let Some(canonical) = crate::tools::canonical_tool_alias(tool_name)
            && self.allowed_tool_names.contains(canonical)
        {
            return Ok(canonical.to_owned());
        }
        Err(ServiceError::new(
            "invalid_provider_response",
            "The provider requested a tool that was not included in the request.",
            false,
        ))
    }

    async fn deny_unadvertised_tool(
        &self,
        call: &ToolCall,
        rejection_error: ServiceError,
        provider_id: &str,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        storage: &AppStorage,
    ) -> Result<ToolResult, ServiceError> {
        if !is_valid_wire_tool_name(&call.name) {
            return Err(rejection_error);
        }
        let exceeded_limit = {
            let mut turn_state = self
                .turn_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            turn_state.unadvertised_tool_denials =
                turn_state.unadvertised_tool_denials.saturating_add(1);
            turn_state.unadvertised_tool_denials > MAX_DENIED_UNADVERTISED_TOOL_CALLS_PER_TURN
        };
        if exceeded_limit {
            return Err(rejection_error);
        }

        let is_opencode = provider_id == "opencode";
        let display_name = if is_opencode {
            crate::tools::opencode_wire_name(&call.name)
        } else {
            call.name.clone()
        };
        let mut allowed = self
            .allowed_tool_names
            .iter()
            .filter(|name| !matches!(name.as_str(), "start_goal" | "stop_goal"))
            .map(|name| {
                if is_opencode {
                    crate::tools::opencode_wire_name(name)
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>();
        allowed.sort();
        allowed.dedup();
        let message = if allowed.is_empty() {
            format!(
                "Tool '{display_name}' is not available in this session. Do not call tools; respond directly."
            )
        } else {
            format!(
                "Tool '{display_name}' is not available in this session. Use only the available tools: {}.",
                allowed.join(", ")
            )
        };
        let output = tool_error("tool_not_available", &message);
        self.emit_activity(
            storage,
            ToolActivity::denied(call, String::new(), output.clone()),
            request_id,
            snapshot,
            events,
        )
        .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    async fn deny_plan_mode_tool(
        &self,
        call: &ToolCall,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        storage: &AppStorage,
    ) -> Result<ToolResult, ServiceError> {
        let output = tool_error(
            "plan_mode_tools_disabled",
            "Plan mode does not allow tool calls. Review the plan, then turn off Plan mode before taking action.",
        );
        self.emit_activity(
            storage,
            ToolActivity::denied(call, String::new(), output.clone()),
            request_id,
            snapshot,
            events,
        )
        .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    fn permission_rule(
        &self,
        call: &ToolCall,
        operation: &ToolOperation,
        target_path: Option<&Path>,
    ) -> ToolPermissionRule {
        if self.permission_mode == ToolPermissionMode::Plan {
            return ToolPermissionRule::Deny;
        }
        // Trust Handoff defense: Writing or editing executable hooks or IDE tasks
        // always requires explicit user confirmation regardless of auto-approval mode.
        if let Some(path) = target_path
            && matches!(
                operation,
                ToolOperation::Write { .. } | ToolOperation::Edit { .. }
            )
        {
            let path_str = path.to_string_lossy().to_lowercase();
            if path_str.contains(".git\\hooks")
                || path_str.contains(".git/hooks")
                || path_str.contains(".vscode")
                || path_str.contains(".idea")
            {
                return ToolPermissionRule::Ask;
            }
        }

        let task_rule = (call.name == "run_project_task")
            .then(|| {
                call.arguments
                    .get("task")
                    .and_then(Value::as_str)
                    .and_then(|task_id| {
                        project_task_permission_rule(&self.permission_rules, task_id)
                    })
            })
            .flatten();
        task_rule
            .or_else(|| self.permission_rules.get(&call.name))
            .copied()
            .unwrap_or_else(|| {
                if self.permission_mode.requires_approval(operation) {
                    ToolPermissionRule::Ask
                } else {
                    ToolPermissionRule::Allow
                }
            })
    }

    pub fn begin_round(&self, calls: &[ToolCall]) {
        if calls.is_empty() {
            return;
        }
        let mut turn_state = self
            .turn_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        turn_state.round_sequence = turn_state.round_sequence.saturating_add(1);
        turn_state.current_round_id = Some(format!("round-{}", turn_state.round_sequence));
    }

    pub(crate) fn reset_turn_state(&self) {
        let mut turn_state = self
            .turn_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *turn_state = ToolTurnState::default();
    }

    fn project_rules_required_for_write(
        &self,
        prepared: &PreparedToolCall,
    ) -> Result<Option<Vec<crate::domain::project_rules::ScopedProjectRule>>, ServiceError> {
        if !matches!(
            &prepared.operation,
            ToolOperation::Write { .. } | ToolOperation::Edit { .. }
        ) {
            return Ok(None);
        }
        if self.project_roots.is_empty() {
            return Ok(None);
        }
        let rules = crate::domain::project_rules::matching_rules_for_target(
            &self.project_roots,
            &prepared.target_path,
        )?;
        if rules.is_empty() {
            return Ok(None);
        }
        let serialized_rules = serde_json::to_vec(&rules).map_err(|_| {
            ServiceError::new(
                "project_rules_unavailable",
                "The matching project rules could not be prepared safely.",
                false,
            )
        })?;
        let mut fingerprint = Sha256::new();
        fingerprint.update(prepared.target_path.to_string_lossy().as_bytes());
        fingerprint.update([0]);
        fingerprint.update(serialized_rules);
        let fingerprint = format!("{:x}", fingerprint.finalize());
        let mut turn_state = self
            .turn_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if turn_state.project_rule_prechecks.contains(&fingerprint) {
            return Ok(None);
        }
        if turn_state.project_rule_prechecks.len() >= MAX_PROJECT_RULE_PRECHECKS {
            turn_state.project_rule_prechecks.clear();
        }
        turn_state.project_rule_prechecks.insert(fingerprint);
        Ok(Some(rules))
    }

    #[cfg(test)]
    pub async fn execute_call(
        &self,
        call: &ToolCall,
        context: ToolExecutionContext<'_, '_>,
    ) -> Result<ToolResult, ServiceError> {
        self.execute_call_with_image_context(call, context).await
    }

    pub async fn execute_call_with_image_context(
        &self,
        call: &ToolCall,
        context: ToolExecutionContext<'_, '_>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolExecutionContext {
            permissions,
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
            run_id,
            provider_id,
            user_questions,
            image_generation,
        } = context;
        let resolved_call: std::borrow::Cow<'_, ToolCall> =
            match self.validate_tool_name(&call.name) {
                Ok(resolved_name) if resolved_name == call.name => std::borrow::Cow::Borrowed(call),
                Ok(resolved_name) => std::borrow::Cow::Owned(ToolCall {
                    id: call.id.clone(),
                    name: resolved_name,
                    arguments: call.arguments.clone(),
                }),
                Err(error) => {
                    return self
                        .deny_unadvertised_tool(
                            call,
                            error,
                            provider_id,
                            request_id,
                            snapshot,
                            events,
                            storage,
                        )
                        .await;
                }
            };
        let call = resolved_call.as_ref();
        if self.permission_mode == ToolPermissionMode::Plan {
            return self
                .deny_plan_mode_tool(call, request_id, snapshot, events, storage)
                .await;
        }
        #[cfg(windows)]
        if call.name == "computer_use" {
            return self
                .execute_computer_use_call(
                    call,
                    permissions,
                    ToolEventContext {
                        request_id,
                        snapshot,
                        events,
                        cancellation: &mut *cancellation,
                        storage,
                    },
                )
                .await;
        }
        #[cfg(windows)]
        if call.name.starts_with("mcp__") {
            return self
                .execute_mcp_call(
                    call,
                    permissions,
                    ToolEventContext {
                        request_id,
                        snapshot,
                        events,
                        cancellation: &mut *cancellation,
                        storage,
                    },
                )
                .await;
        }
        if call.name == "generate_image" {
            return self
                .execute_image_call(
                    call,
                    ToolEventContext {
                        request_id,
                        snapshot,
                        events,
                        cancellation: &mut *cancellation,
                        storage,
                    },
                    image_generation,
                )
                .await;
        }
        if call.name == "ask_user" {
            let questions = match call
                .arguments
                .get("questions")
                .ok_or_else(question_arguments_error)
                .and_then(crate::user_question_broker::parse_questions)
            {
                Ok(questions) => questions,
                Err(error) => {
                    let output = tool_error(error.code, &error.message);
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(call, None, output.clone()),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Ok(ToolResult {
                        call_id: call.id.clone(),
                        output,
                    });
                }
            };
            let waiting_activity =
                self.prepare_activity(ToolActivity::waiting_for_user(call), snapshot);
            let tool_activities = self.record_activity(&waiting_activity).await;
            let answers = match user_questions
                .ask(
                    storage,
                    run_id,
                    provider_id,
                    request_id,
                    &snapshot.conversation_id,
                    &snapshot.message_id,
                    &call.id,
                    &call.name,
                    &call.arguments,
                    questions,
                    snapshot,
                    &waiting_activity,
                    &tool_activities,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(answers) => answers,
                Err(error) => {
                    if error.code == "operation_cancelled" {
                        self.emit_activity(
                            storage,
                            ToolActivity::cancelled(
                                call,
                                String::new(),
                                tool_error(error.code, &error.message),
                            ),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                    } else {
                        self.emit_activity(
                            storage,
                            ToolActivity::finished(
                                call,
                                None,
                                tool_error(error.code, &error.message),
                            ),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                    }
                    return Err(error);
                }
            };
            let output = serde_json::to_value(answers).map_err(|_| {
                ServiceError::new(
                    "question_answer_invalid",
                    "The submitted answer could not be read. Ask the user again.",
                    false,
                )
            })?;
            let output = json!({"answers": output});
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        }

        let mut prepared = match self.prepare_call(call) {
            Ok(prepared) => prepared,
            Err(output) => {
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };

        if matches!(
            &prepared.operation,
            ToolOperation::SaveMemory { .. } | ToolOperation::SaveProjectMemory { .. }
        ) && snapshot.conversation_id.starts_with("tmp_")
        {
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output: tool_error(
                    "temporary_chat_memory_unavailable",
                    "Memory cannot be saved in temporary chats.",
                ),
            });
        }

        let mut activity_call = call.clone();
        if call.name == "run_project_task"
            && let ToolOperation::Bash {
                command: Some(command),
                timeout_seconds: Some(timeout_seconds),
                ..
            } = &prepared.operation
        {
            activity_call.arguments["command"] = json!(command);
            activity_call.arguments["timeout_seconds"] = json!(timeout_seconds);
        }
        let call = &activity_call;

        let rule = self.permission_rule(call, &prepared.operation, Some(&prepared.target_path));
        let mut approved_file_root = None;
        let mut approved_file_root_identity = None;
        if rule == ToolPermissionRule::Deny {
            let output = tool_error(
                "permission_denied",
                "This tool is denied by the project's permission rules.",
            );
            self.emit_activity(
                storage,
                ToolActivity::denied(
                    call,
                    prepared.target_path.to_string_lossy().into_owned(),
                    output.clone(),
                ),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        }

        match self.project_rules_required_for_write(&prepared) {
            Ok(Some(rules)) => {
                let mut output = tool_error(
                    "project_rules_required",
                    "Review the matching project rules and retry the file operation.",
                );
                output["projectRules"] = json!(rules);
                output["retryRequired"] = json!(true);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(
                        call,
                        Some(prepared.target_path.to_string_lossy().into_owned()),
                        output.clone(),
                    ),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
            Ok(None) => {}
            Err(error) => {
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(
                        call,
                        Some(prepared.target_path.to_string_lossy().into_owned()),
                        output.clone(),
                    ),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        }

        if rule == ToolPermissionRule::Ask {
            let approved_target = prepared.target_path.clone();
            let approved_root = prepared.root.clone();
            let approved_relative_path = prepared.relative_path.clone();
            if matches!(
                &prepared.operation,
                ToolOperation::Write { .. } | ToolOperation::Edit { .. }
            ) {
                let opened_root = crate::file_changes::open_workspace_directory(&approved_root);
                match opened_root {
                    Ok((directory, canonical_root)) if canonical_root == approved_root => {
                        match crate::file_changes::workspace_directory_identity(&directory) {
                            Ok(identity) => approved_file_root_identity = Some(identity),
                            Err(_) => {
                                let output = tool_error(
                                    "workspace_unavailable",
                                    "The active project folder identity could not be read safely before approval.",
                                );
                                self.emit_activity(
                                    storage,
                                    ToolActivity::finished(
                                        call,
                                        Some(approved_target.to_string_lossy().into_owned()),
                                        output.clone(),
                                    ),
                                    request_id,
                                    snapshot,
                                    events,
                                )
                                .await?;
                                return Ok(ToolResult {
                                    call_id: call.id.clone(),
                                    output,
                                });
                            }
                        }
                    }
                    Ok(_) | Err(_) => {
                        let output = tool_error(
                            "workspace_unavailable",
                            "The active project folder could not be pinned safely before approval.",
                        );
                        self.emit_activity(
                            storage,
                            ToolActivity::finished(
                                call,
                                Some(approved_target.to_string_lossy().into_owned()),
                                output.clone(),
                            ),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                        return Ok(ToolResult {
                            call_id: call.id.clone(),
                            output,
                        });
                    }
                }
            }
            let target_path = prepared.target_path.to_string_lossy().into_owned();
            self.emit_activity(
                storage,
                ToolActivity::awaiting_approval(call, target_path.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            let approved = match permissions
                .request_approval(
                    request_id,
                    &call.name,
                    &target_path,
                    &call.arguments,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(approved) => approved,
                Err(error) if error.code == "operation_cancelled" => {
                    let output = tool_error("operation_cancelled", "The request was stopped.");
                    self.emit_activity(
                        storage,
                        ToolActivity::cancelled(call, target_path, output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(
                            call,
                            Some(target_path),
                            tool_error(error.code, &error.message),
                        ),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
            };
            if !approved {
                let output = tool_error(
                    "permission_denied",
                    "The user denied this tool call. Do not retry the same operation unless the user asks.",
                );
                self.emit_activity(
                    storage,
                    ToolActivity::denied(call, target_path, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }

            let is_non_fs = matches!(
                &prepared.operation,
                ToolOperation::Bash { .. }
                    | ToolOperation::SendTerminalInput { .. }
                    | ToolOperation::WebSearch { .. }
                    | ToolOperation::ReadUrlContent { .. }
                    | ToolOperation::SaveMemory { .. }
                    | ToolOperation::SaveProjectMemory { .. }
                    | ToolOperation::DelegateTask { .. }
                    | ToolOperation::ProjectTool { .. }
            );
            if !is_non_fs {
                match self.resolve_tool_path(
                    &prepared.requested_path,
                    prepared.operation.targets_directory(),
                    prepared.operation.can_create_file(),
                ) {
                    Ok((root, relative_path, target, scope))
                        if target == approved_target
                            && (approved_file_root_identity.is_none()
                                || (root == approved_root
                                    && relative_path == approved_relative_path)) =>
                    {
                        if let Some(expected_identity) = approved_file_root_identity {
                            match open_approved_file_root(&root, expected_identity) {
                                Ok(directory) => {
                                    approved_file_root = Some(std::sync::Arc::new(directory));
                                }
                                Err(code) => {
                                    let message = if code == "permission_target_changed" {
                                        "The active project folder changed after approval. Request permission again."
                                    } else {
                                        "The active project folder could not be checked safely after approval."
                                    };
                                    let output = tool_error(code, message);
                                    self.emit_activity(
                                        storage,
                                        ToolActivity::denied(call, target_path, output.clone()),
                                        request_id,
                                        snapshot,
                                        events,
                                    )
                                    .await?;
                                    return Ok(ToolResult {
                                        call_id: call.id.clone(),
                                        output,
                                    });
                                }
                            }
                        }
                        prepared.root = root;
                        prepared.relative_path = relative_path;
                        prepared.target_path = target;
                        prepared.scope = scope;
                    }
                    Ok(_) => {
                        let output = tool_error(
                            "permission_target_changed",
                            "The requested path changed after approval. Request permission again.",
                        );
                        self.emit_activity(
                            storage,
                            ToolActivity::denied(call, target_path, output.clone()),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                        return Ok(ToolResult {
                            call_id: call.id.clone(),
                            output,
                        });
                    }
                    Err(output) => {
                        self.emit_activity(
                            storage,
                            ToolActivity::finished(call, Some(target_path), output.clone()),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                        return Ok(ToolResult {
                            call_id: call.id.clone(),
                            output,
                        });
                    }
                }
            }
        }

        #[cfg(windows)]
        if let ToolOperation::ProjectTool { executable, .. } = &prepared.operation {
            let current = self
                .project_tools
                .iter()
                .find(|tool| tool.tool_name() == call.name)
                .and_then(|tool| tool.executable_path(&prepared.root).ok());
            if current.as_ref() != Some(executable)
                || current.as_ref() != Some(&prepared.target_path)
            {
                let output = tool_error(
                    "permission_target_changed",
                    "The configured tool changed before execution. Request permission again.",
                );
                self.emit_activity(
                    storage,
                    ToolActivity::finished(
                        call,
                        Some(prepared.target_path.to_string_lossy().into_owned()),
                        output.clone(),
                    ),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        }

        self.emit_activity(
            storage,
            ToolActivity::running(
                call,
                Some(prepared.target_path.to_string_lossy().into_owned()),
            ),
            request_id,
            snapshot,
            events,
        )
        .await?;
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(
                    call,
                    prepared.target_path.to_string_lossy().into_owned(),
                    output,
                ),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }
        let (mut output, file_changes, file_changes_error, recovery_checkpoint_id) = if matches!(
            &prepared.operation,
            ToolOperation::Write { .. } | ToolOperation::Edit { .. }
        ) {
            execute_direct_file_change_with_tracking(
                storage,
                &snapshot.conversation_id,
                &call.name,
                &prepared,
                approved_file_root,
            )
        } else {
            let file_change_capture = capture_file_change_before(&prepared, &self.project_roots);
            let output = match &prepared.operation {
                ToolOperation::SaveMemory { fact } => {
                    save_personal_memory(storage, &snapshot.conversation_id, fact)
                }
                ToolOperation::SaveProjectMemory { note } => {
                    save_project_memory(storage, &snapshot.conversation_id, note)
                }
                ToolOperation::ListSubagentModels => json!({
                    "models": self.subagent_models.iter().map(|model| json!({
                        "modelKey": model.key,
                        "providerId": model.provider_id,
                        "modelId": model.model_id,
                        "displayName": model.display_name,
                        "supportsTools": model.supports_tools,
                        "reasoningLevels": model.reasoning_levels,
                        "preferred": model.preferred,
                    })).collect::<Vec<_>>()
                }),
                ToolOperation::DelegateTask {
                    model_key,
                    reasoning_effort,
                    task,
                    context,
                } => {
                    match self
                        .subagent_models
                        .iter()
                        .find(|model| model.key == *model_key)
                        .cloned()
                    {
                        Some(model) => match self.chat_service.clone() {
                            Some(service) => {
                                let route = crate::chatgpt::subagents::resolve_child_run_route(
                                    &service,
                                    storage,
                                    &model,
                                    reasoning_effort.as_deref(),
                                    cancellation,
                                )
                                .await;
                                match route {
                                    Ok(route) => {
                                        let child_image_generation = image_generation
                                        .map(crate::chatgpt::subagents::OwnedImageGenerationContext::from_borrowed);
                                        let child_executor = self.child_executor(
                                            model.supports_tools,
                                            child_image_generation.is_some()
                                                || crate::tools::model_supports_image_generation(
                                                    &model.provider_id,
                                                    &model.model_id,
                                                ),
                                        );
                                        crate::chatgpt::subagents::start_child_analysis(
                                        crate::chatgpt::subagents::ChildRunTask {
                                            service: service.clone(),
                                            call: call.clone(),
                                            task: task.clone(),
                                            additional_context: context.clone(),
                                            project_memory_context: self.project_memory_context.clone(),
                                            parent_message_id: snapshot.message_id.clone(),
                                            project_root: self.project_root.clone(),
                                            permission_broker: permissions.clone(),
                                            request_id: request_id.clone(),
                                            snapshot: snapshot.clone(),
                                            events: events.clone(),
                                            storage: service.storage_handle(),
                                            parent_run_id: run_id.to_owned(),
                                            tool_executor: child_executor,
                                            image_generation: child_image_generation,
                                            user_questions: user_questions.clone(),
                                        },
                                        route,
                                    )
                                    .unwrap_or_else(|error| json!({"error":{"code":error.code,"message":error.message}}))
                                    }
                                    Err(error) => {
                                        json!({"error":{"code":error.code,"message":error.message}})
                                    }
                                }
                            }
                            None => {
                                json!({"error":{"code":"subagent_unavailable","message":"Delegated work is unavailable for this provider route."}})
                            }
                        },
                        None => {
                            json!({"error":{"code":"invalid_subagent_model","message":"The selected child model is no longer available. List the available models again."}})
                        }
                    }
                }
                ToolOperation::GetSubagentResult { run_id } => {
                    crate::chatgpt::subagents::child_run_result(
                        storage,
                        &snapshot.conversation_id,
                        run_id,
                    )
                    .unwrap_or_else(
                        |error| json!({"error":{"code":error.code,"message":error.message}}),
                    )
                }
                ToolOperation::CancelSubagent { run_id } => {
                    crate::chatgpt::subagents::cancel_child_analysis(
                        storage,
                        &snapshot.conversation_id,
                        run_id,
                    )
                    .unwrap_or_else(
                        |error| json!({"error":{"code":error.code,"message":error.message}}),
                    )
                }
                ToolOperation::WebSearch { .. } => {
                    execute_model_tool_with_workspace_roots(
                        &prepared,
                        &snapshot.conversation_id,
                        &self.project_roots,
                        self.permission_mode == ToolPermissionMode::FullAccess,
                    )
                    .await
                }
                #[cfg(windows)]
                ToolOperation::ProjectTool {
                    executable,
                    input,
                    timeout_seconds,
                    output_limit_bytes,
                } => match serde_json::to_string(input) {
                    Ok(serialized_input) => match super::terminal::TerminalSessionManager::global()
                        .execute_project_tool(super::terminal::ProjectToolRequest {
                            program: executable,
                            workdir: &prepared.root,
                            input: &serialized_input,
                            timeout_seconds: *timeout_seconds,
                            output_limit_bytes: *output_limit_bytes,
                            cancellation,
                            full_access: self.permission_mode == ToolPermissionMode::FullAccess,
                        })
                        .await
                    {
                        Ok(output) => output,
                        Err(error) => {
                            let mut output =
                                json!({"error":{"code":error.code,"message":error.message}});
                            if let Some(details) = error.details {
                                output["error"]["details"] = Value::String(details);
                            }
                            output
                        }
                    },
                    Err(_) => {
                        json!({"error":{"code":"project_tool_failed","message":"The configured tool input could not be encoded."}})
                    }
                },
                _ => {
                    execute_model_tool_with_workspace_roots(
                        &prepared,
                        &snapshot.conversation_id,
                        &self.project_roots,
                        self.permission_mode == ToolPermissionMode::FullAccess,
                    )
                    .await
                }
            };
            let (file_changes, file_changes_error, recovery_checkpoint_id) =
                finish_file_change_capture(
                    storage,
                    &snapshot.conversation_id,
                    &call.name,
                    &prepared,
                    file_change_capture,
                    false,
                );
            (
                output,
                file_changes,
                file_changes_error,
                recovery_checkpoint_id,
            )
        };
        if matches!(
            call.name.as_str(),
            "web_search" | "read_url_content" | "read_url"
        ) {
            qualify_citation_source_ids(&mut output, &call.id);
        }
        qualify_output_paths(&mut output, &prepared, &self.project_roots);
        redact_tool_output(&mut output);
        let target = prepared.target_path.to_string_lossy().into_owned();
        #[cfg(windows)]
        let is_cancelled = output.get("status").and_then(Value::as_str) == Some("cancelled");
        #[cfg(windows)]
        let mut activity = if is_cancelled {
            ToolActivity::cancelled(call, target, output.clone())
        } else {
            ToolActivity::finished(call, Some(target), output.clone())
        };
        #[cfg(not(windows))]
        let mut activity = ToolActivity::finished(call, Some(target), output.clone());
        activity.file_changes = file_changes;
        activity.file_changes_error = file_changes_error;
        let operation_failed = ToolActivity::output_indicates_failure(&output)
            || output.get("status").and_then(Value::as_str) == Some("cancelled");
        if operation_failed && let Some(checkpoint_id) = recovery_checkpoint_id {
            match crate::file_changes::checkpoints::mark_checkpoint_recovery_available(
                storage,
                &snapshot.conversation_id,
                &checkpoint_id,
            ) {
                Ok(()) => activity.recovery_checkpoint_id = Some(checkpoint_id),
                Err(_) => {
                    activity.file_changes_error = Some("recovery_unavailable".to_owned());
                }
            }
        }
        self.emit_activity(storage, activity, request_id, snapshot, events)
            .await?;
        #[cfg(windows)]
        if is_cancelled {
            return Err(crate::permissions::operation_cancelled_error());
        }
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    #[cfg(windows)]
    async fn execute_mcp_call(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        context: ToolEventContext<'_>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolEventContext {
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
        } = context;
        let Some(registry) = self.mcp_registry.as_ref() else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some(server_id) = registry.server_id_for_tool(&call.name) else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some(tool_name) = registry.tool_name_for_tool(&call.name) else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some((server_permission_fingerprint, definition_fingerprint)) =
            registry.permission_fingerprints_for_tool(&call.name)
        else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let target = format!("MCP server {server_id}, tool {tool_name}");
        let tool_rule = mcp_tool_permission_rule(
            &self.permission_rules,
            server_id,
            server_permission_fingerprint,
            tool_name,
            definition_fingerprint,
        );
        if tool_rule == ToolPermissionRule::Deny {
            let output = tool_error(
                "permission_denied",
                "This MCP tool is denied by the project's permission rules.",
            );
            self.emit_activity(
                storage,
                ToolActivity::denied(call, target, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        }
        if tool_rule != ToolPermissionRule::Allow {
            self.emit_activity(
                storage,
                ToolActivity::awaiting_approval(call, target.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            let approved = match permissions
                .request_approval(
                    request_id,
                    &call.name,
                    &target,
                    &call.arguments,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(approved) => approved,
                Err(error) if error.code == "operation_cancelled" => {
                    let output = tool_error("operation_cancelled", "The request was stopped.");
                    self.emit_activity(
                        storage,
                        ToolActivity::cancelled(call, target, output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    let output = tool_error(error.code, &error.message);
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(call, Some(target), output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
            };
            if !approved {
                let output = tool_error("permission_denied", "The user denied this MCP tool call.");
                self.emit_activity(
                    storage,
                    ToolActivity::denied(call, target, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        }

        self.emit_activity(
            storage,
            ToolActivity::running(call, Some(target.clone())),
            request_id,
            snapshot,
            events,
        )
        .await?;
        let arguments = match call.arguments.as_object() {
            Some(arguments) => arguments.clone(),
            None => {
                let output = tool_error(
                    "invalid_tool_arguments",
                    "The MCP tool arguments must be a JSON object.",
                );
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, Some(target), output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(call, target, output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }
        let call_future =
            registry.call_tool(&call.name, arguments, std::time::Duration::from_secs(120));
        tokio::pin!(call_future);
        let result = loop {
            tokio::select! {
                result = &mut call_future => break result,
                changed = cancellation.changed() => {
                    if changed.is_err() || *cancellation.borrow() {
                        let output = tool_error("operation_cancelled", "The request was stopped.");
                        self.emit_activity(
                            storage,
                            ToolActivity::cancelled(call, target, output),
                            request_id,
                            snapshot,
                            events,
                        ).await?;
                        return Err(crate::permissions::operation_cancelled_error());
                    }
                }
            }
        };
        let mut output = match result {
            Ok(result) if result.get("isError").and_then(Value::as_bool) == Some(true) => {
                let details = result
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| {
                        (item.get("type").and_then(Value::as_str) == Some("text"))
                            .then(|| item.get("text").and_then(Value::as_str))
                            .flatten()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut output = json!({
                    "error": {
                        "code": "mcp_tool_failed",
                        "message": "The MCP server reported a tool error."
                    },
                    "untrusted": true,
                    "result": result
                });
                if !details.is_empty() {
                    output["error"]["details"] =
                        Value::String(details.chars().take(1000).collect());
                }
                output
            }
            Ok(result) => json!({"untrusted": true, "result": result}),
            Err(error) => {
                let mut output =
                    tool_error("mcp_tool_failed", "The MCP tool call failed or timed out.");
                output["error"]["details"] = Value::String(error);
                output["untrusted"] = Value::Bool(true);
                output
            }
        };
        redact_tool_output(&mut output);
        self.emit_activity(
            storage,
            ToolActivity::finished(call, Some(target), output.clone()),
            request_id,
            snapshot,
            events,
        )
        .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    async fn emit_activity(
        &self,
        storage: &AppStorage,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let activity = self.prepare_activity(activity, snapshot);
        let activities = self.record_activity(&activity).await;
        crate::chatgpt_store::save_assistant_tool_checkpoint(storage, snapshot, &activities)
            .map_err(|_| tool_activity_storage_error())?;
        self.emit_prepared_activity(activity, request_id, snapshot, events)
            .await
    }

    async fn execute_image_call(
        &self,
        call: &ToolCall,
        context: ToolEventContext<'_>,
        image_generation: Option<&ImageGenerationContext<'_>>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolEventContext {
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
        } = context;
        let request = match parse_image_request(&call.arguments) {
            Ok(request) => request,
            Err(error) => {
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };
        let Some(image_generation) = image_generation else {
            let output = tool_error(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider connection.",
            );
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        };

        self.emit_activity(
            storage,
            ToolActivity::running(call, None),
            request_id,
            snapshot,
            events,
        )
        .await?;
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(call, String::new(), output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }

        let run_result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                connection_id,
                workspace_id,
                ..
            } => crate::image_library::begin_generation(
                storage,
                &request,
                crate::image_library::ImageGenerationOrigin {
                    source_kind: "chat",
                    provider_route: "oauth",
                    connection_id: Some(connection_id),
                    workspace_id: Some(workspace_id),
                    conversation_id: Some(&snapshot.conversation_id),
                    message_id: Some(&snapshot.message_id),
                },
            ),
            ImageGenerationContext::ApiKey { .. } => crate::image_library::begin_generation(
                storage,
                &request,
                crate::image_library::ImageGenerationOrigin {
                    source_kind: "chat",
                    provider_route: "api_key",
                    connection_id: None,
                    workspace_id: None,
                    conversation_id: Some(&snapshot.conversation_id),
                    message_id: Some(&snapshot.message_id),
                },
            ),
            ImageGenerationContext::CompatibleApiKey { .. }
            | ImageGenerationContext::LocalEndpoint { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider route.",
                false,
            )),
        };
        let generation_id = match run_result {
            Ok(id) => id,
            Err(error) => {
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };

        let result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                service,
                connection_id,
                workspace_id,
                turn_id,
                ..
            } => {
                service
                    .generate_image_with_account_quota(
                        &request,
                        ImageGenerationAuth::ChatGptOAuth {
                            connection_id,
                            workspace_id,
                        },
                        *turn_id,
                        cancellation,
                    )
                    .await
            }
            ImageGenerationContext::ApiKey {
                service, api_key, ..
            } => {
                service
                    .generate_image_with_account_quota(
                        &request,
                        ImageGenerationAuth::ApiKey(api_key),
                        None,
                        cancellation,
                    )
                    .await
            }
            ImageGenerationContext::CompatibleApiKey { .. }
            | ImageGenerationContext::LocalEndpoint { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider route.",
                false,
            )),
        };
        let generated = match result {
            Ok(result) => result,
            Err(error) if is_image_cancellation(&error) => {
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "cancelled",
                    Some(error.code),
                )?;
                self.emit_activity(
                    storage,
                    ToolActivity::cancelled(
                        call,
                        String::new(),
                        tool_error(error.code, &error.message),
                    ),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Err(error);
            }
            Err(error) => {
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "failed",
                    Some(error.code),
                )?;
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };

        let result = generated.result;

        if *cancellation.borrow() {
            crate::image_library::finish_generation(
                storage,
                &generation_id,
                "cancelled",
                Some("operation_cancelled"),
            )?;
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(call, String::new(), output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }

        let metadata = &result.metadata;
        let images = result
            .outputs
            .iter()
            .map(|image| (image.mime_type.as_str(), image.bytes.as_slice()))
            .collect::<Vec<_>>();
        let attachments = match crate::attachments::write_generated_images(
            storage.root(),
            &snapshot.conversation_id,
            &snapshot.message_id,
            &images,
        ) {
            Ok(attachments) => attachments,
            Err(error) => {
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "failed",
                    Some(error.code),
                )?;
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };
        if let Err(error) =
            crate::image_library::complete_chat_generation(storage, &generation_id, &attachments)
        {
            let cleanup_result = crate::attachments::delete_generated_images(
                storage.root(),
                &snapshot.conversation_id,
                &snapshot.message_id,
                &attachments,
            );
            let status_result = crate::image_library::finish_generation(
                storage,
                &generation_id,
                "failed",
                Some(error.code),
            );
            let error = match (cleanup_result, status_result) {
                (Err(cleanup_error), _) => cleanup_error,
                (_, Err(status_error)) => status_error,
                (Ok(()), Ok(())) => error,
            };
            let output = tool_error(error.code, &error.message);
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        }
        if crate::attachments::append_assistant_attachments(
            storage,
            &snapshot.conversation_id,
            &snapshot.message_id,
            &attachments,
        )
        .is_err()
        {
            let cleanup_result = crate::attachments::delete_generated_images(
                storage.root(),
                &snapshot.conversation_id,
                &snapshot.message_id,
                &attachments,
            );
            let revert_result = crate::image_library::revert_chat_generation(
                storage,
                &generation_id,
                "image_storage_persistence_failed",
            );
            let error = if cleanup_result.is_err() || revert_result.is_err() {
                image_storage_cleanup_error()
            } else {
                image_storage_persistence_error()
            };
            let output = tool_error(error.code, &error.message);
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        }

        let mut output = json!({
            "attachments": attachments,
            "generationId": generation_id,
            "created": metadata.created_unix_seconds,
            "generation": {
                "background": metadata.background,
                "quality": metadata.quality,
                "size": metadata.size.as_deref(),
            },
        });
        if let Some(request_id) = metadata.imagegen_request_id.as_deref() {
            output["imageRequestId"] = Value::String(request_id.to_owned());
        }
        self.emit_activity(
            storage,
            ToolActivity::finished(call, None, output.clone()),
            request_id,
            snapshot,
            events,
        )
        .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    fn prepare_activity(
        &self,
        mut activity: ToolActivity,
        snapshot: &ChatStreamSnapshot,
    ) -> ToolActivity {
        activity.round_id = self
            .turn_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_round_id
            .clone();
        activity.assistant_text_before_byte_offset = Some(snapshot.content.len());
        activity
    }

    #[cfg(test)]
    pub(crate) async fn recorded_activities(&self) -> Vec<ToolActivity> {
        self.tool_activities.lock().await.clone()
    }

    async fn record_activity(&self, activity: &ToolActivity) -> Vec<ToolActivity> {
        let mut activities = self.tool_activities.lock().await;
        if let Some(existing) = activities
            .iter_mut()
            .find(|existing| existing.call_id == activity.call_id)
        {
            *existing = activity.clone();
        } else {
            activities.push(activity.clone());
        }
        activities.clone()
    }

    async fn emit_prepared_activity(
        &self,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let event = ChatStreamEvent::ToolActivityUpdated {
            snapshot: snapshot.clone(),
            activity,
        }
        .into_rpc(request_id.clone());
        events.send(&event).await.map_err(|_| protocol_error())
    }
}

fn qualify_citation_source_ids(output: &mut Value, call_id: &str) {
    let suffix = call_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>();
    if suffix.is_empty() {
        return;
    }
    let qualify = |source_id: &mut Value| {
        let Some(id) = source_id.as_str() else {
            return;
        };
        *source_id = json!(format!("{id}-{suffix}"));
    };
    if let Some(results) = output.get_mut("results").and_then(Value::as_array_mut) {
        for result in results {
            if let Some(source_id) = result.get_mut("sourceId") {
                qualify(source_id);
            }
        }
    }
    if let Some(source_id) = output.get_mut("sourceId") {
        qualify(source_id);
    }
}

fn execute_direct_file_change_with_tracking(
    storage: &crate::storage::AppStorage,
    conversation_id: &str,
    tool_name: &str,
    prepared: &PreparedToolCall,
    approved_root_directory: Option<std::sync::Arc<crate::filesystem::OpenedDirectory>>,
) -> (
    Value,
    Vec<crate::file_changes::FileChangeSummary>,
    Option<String>,
    Option<String>,
) {
    let guard = match crate::file_changes::lock_file_changes() {
        Ok(guard) => guard,
        Err(_) => {
            return (
                tool_error(
                    "storage_unavailable",
                    "File changes could not be coordinated safely.",
                ),
                Vec::new(),
                Some("storage_unavailable".to_owned()),
                None,
            );
        }
    };
    let root_directory = match approved_root_directory {
        Some(directory) => {
            match crate::file_changes::workspace_directory_matches_path(&prepared.root, &directory)
            {
                Ok(true) => {}
                Ok(false) => {
                    return (
                        tool_error(
                            "permission_target_changed",
                            "The active project folder changed after approval. Request permission again.",
                        ),
                        Vec::new(),
                        Some("workspace_unavailable".to_owned()),
                        None,
                    );
                }
                Err(_) => {
                    return (
                        tool_error(
                            "workspace_unavailable",
                            "The active project folder could not be checked safely after approval.",
                        ),
                        Vec::new(),
                        Some("workspace_unavailable".to_owned()),
                        None,
                    );
                }
            }
            directory
        }
        None => match crate::file_changes::open_workspace_directory(&prepared.root) {
            Ok((directory, _)) => std::sync::Arc::new(directory),
            Err(error) => {
                let code = tracking_error_code(error);
                return (
                    tool_error(
                        code,
                        "The active project folder could not be opened safely.",
                    ),
                    Vec::new(),
                    Some(code.to_owned()),
                    None,
                );
            }
        },
    };
    let mut capture = match capture_direct_file_change_before(prepared, root_directory.clone()) {
        Ok(capture) => capture,
        Err(error) => {
            let mut output = tool_error(error.code, &error.message);
            if let Some(details) = error.details {
                output["error"]["details"] = json!(details);
            }
            drop(guard);
            return (output, Vec::new(), Some(error.code.to_owned()), None);
        }
    };
    let result = match (&mut capture, &prepared.operation) {
        (PendingFileChangeCapture::File { target, .. }, ToolOperation::Write { content }) => {
            crate::tools::write_opened_file(target, &prepared.relative_path, content)
        }
        (
            PendingFileChangeCapture::File { target, .. },
            ToolOperation::Edit {
                old_string,
                new_string,
            },
        ) => {
            crate::tools::edit_opened_file(target, &prepared.relative_path, old_string, new_string)
        }
        _ => Err(ServiceError::new(
            "tool_execution_unavailable",
            "This operation is not a direct file change.",
            false,
        )),
    };
    let output = result.unwrap_or_else(|error| {
        let mut output = tool_error(error.code, &error.message);
        if let Some(details) = error.details {
            output["error"]["details"] = json!(details);
        }
        output
    });
    let (file_changes, file_changes_error, recovery_checkpoint_id) =
        finish_file_change_capture(storage, conversation_id, tool_name, prepared, capture, true);
    drop(guard);
    (
        output,
        file_changes,
        file_changes_error,
        recovery_checkpoint_id,
    )
}

fn open_approved_file_root(
    root: &Path,
    expected_identity: crate::file_changes::DirectoryIdentity,
) -> Result<crate::filesystem::OpenedDirectory, &'static str> {
    let (directory, canonical_root) =
        crate::file_changes::open_workspace_directory(root).map_err(|_| "workspace_unavailable")?;
    if canonical_root != root {
        return Err("permission_target_changed");
    }
    match crate::file_changes::workspace_directory_identity(&directory) {
        Ok(identity) if identity == expected_identity => Ok(directory),
        Ok(_) => Err("permission_target_changed"),
        Err(_) => Err("workspace_unavailable"),
    }
}

fn capture_direct_file_change_before(
    prepared: &PreparedToolCall,
    directory: std::sync::Arc<crate::filesystem::OpenedDirectory>,
) -> Result<PendingFileChangeCapture, ServiceError> {
    let path = prepared.relative_path.clone();
    let root_identity =
        crate::file_changes::workspace_directory_identity(&directory).map_err(|error| {
            ServiceError::new(
                tracking_error_code(error),
                "The active project folder could not be tracked safely.",
                false,
            )
        })?;
    let (mut target, created) = match &prepared.operation {
        ToolOperation::Write { content } => {
            crate::tools::open_file_for_direct_write(&directory, &path, content.len())?
        }
        ToolOperation::Edit { old_string, .. } => (
            crate::tools::open_file_for_direct_edit(&directory, &path, old_string)?,
            false,
        ),
        _ => {
            return Err(ServiceError::new(
                "tool_execution_unavailable",
                "This operation is not a direct file change.",
                false,
            ));
        }
    };
    let before = if created {
        Ok((root_identity, None))
    } else {
        crate::file_changes::snapshot_opened_file(&mut target)
            .map(|snapshot| (root_identity, Some(snapshot)))
    };
    Ok(PendingFileChangeCapture::File {
        directory,
        path,
        before,
        target,
    })
}

fn capture_file_change_before(
    prepared: &PreparedToolCall,
    project_roots: &[PathBuf],
) -> PendingFileChangeCapture {
    match &prepared.operation {
        ToolOperation::Write { .. } | ToolOperation::Edit { .. } => PendingFileChangeCapture::None,
        ToolOperation::Bash { .. } | ToolOperation::SendTerminalInput { .. } => {
            PendingFileChangeCapture::Workspace {
                before: if project_roots.is_empty() {
                    vec![(
                        prepared.root.clone(),
                        crate::file_changes::capture_workspace(&prepared.root),
                    )]
                } else {
                    project_roots
                        .iter()
                        .map(|root| (root.clone(), crate::file_changes::capture_workspace(root)))
                        .collect()
                },
            }
        }
        #[cfg(windows)]
        ToolOperation::ProjectTool { .. } => PendingFileChangeCapture::Workspace {
            before: vec![(
                prepared.root.clone(),
                crate::file_changes::capture_workspace(&prepared.root),
            )],
        },
        _ => PendingFileChangeCapture::None,
    }
}

fn finish_file_change_capture(
    storage: &crate::storage::AppStorage,
    conversation_id: &str,
    tool_name: &str,
    prepared: &PreparedToolCall,
    capture: PendingFileChangeCapture,
    file_change_lock_held: bool,
) -> (
    Vec<crate::file_changes::FileChangeSummary>,
    Option<String>,
    Option<String>,
) {
    let mut retained_target = None;
    let grouped_deltas = match capture {
        PendingFileChangeCapture::None => return (Vec::new(), None, None),
        PendingFileChangeCapture::File {
            directory,
            path,
            before,
            mut target,
        } => {
            let Ok((before_root_identity, before)) = before else {
                return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
            };
            let target_matches_path = crate::filesystem::opened_file_matches_directory_entry(
                directory.as_ref(),
                Path::new(&path),
                &target,
            );
            match target_matches_path {
                Ok(true) => {}
                Ok(false) => {
                    return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
                }
                Err(_) => {
                    return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
                }
            }
            let after = match crate::file_changes::snapshot_opened_file(&mut target) {
                Ok(after) => Some(after),
                Err(error) => {
                    return (
                        Vec::new(),
                        Some(tracking_error_code(error).to_owned()),
                        None,
                    );
                }
            };
            match crate::filesystem::opened_file_matches_directory_entry(
                directory.as_ref(),
                Path::new(&path),
                &target,
            ) {
                Ok(true) => {}
                Ok(false) => {
                    return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
                }
                Err(_) => {
                    return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
                }
            }
            if before_root_identity
                != match crate::file_changes::workspace_directory_identity(&directory) {
                    Ok(identity) => identity,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                }
            {
                return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
            }
            let before_hash = before.as_ref().map(|file| file.hash.as_str());
            let after_hash = after.as_ref().map(|file| file.hash.as_str());
            retained_target = Some(target);
            if before_hash == after_hash {
                Vec::new()
            } else {
                vec![(
                    prepared.root.clone(),
                    vec![crate::file_changes::FileChangeDelta {
                        root_identity: before_root_identity,
                        path,
                        before,
                        after,
                    }],
                )]
            }
        }
        PendingFileChangeCapture::Workspace { before } => {
            let mut changes = Vec::new();
            for (root, before) in before {
                let Ok(before) = before else {
                    return (
                        Vec::new(),
                        Some("workspace_snapshot_unavailable".to_owned()),
                        None,
                    );
                };
                let after = match crate::file_changes::capture_workspace(&root) {
                    Ok(after) => after,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                };
                let deltas = match crate::file_changes::workspace_deltas(&before, &after) {
                    Ok(deltas) => deltas,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                };
                if !deltas.is_empty() {
                    changes.push((root, deltas));
                }
            }
            changes
        }
    };

    if grouped_deltas.is_empty() {
        return (Vec::new(), None, None);
    }
    let changes = match if file_change_lock_held {
        crate::file_changes::record_deltas_batch_locked(storage, conversation_id, &grouped_deltas)
    } else {
        crate::file_changes::record_deltas_batch(storage, conversation_id, &grouped_deltas)
    } {
        Ok(changes) => changes,
        Err(error) => {
            return (
                Vec::new(),
                Some(tracking_error_code(error).to_owned()),
                None,
            );
        }
    };
    let checkpoint = if file_change_lock_held {
        crate::file_changes::checkpoints::persist_checkpoint_locked(
            storage,
            conversation_id,
            tool_name,
            &grouped_deltas,
        )
    } else {
        crate::file_changes::checkpoints::persist_checkpoint(
            storage,
            conversation_id,
            tool_name,
            &grouped_deltas,
        )
    };
    let result = match checkpoint {
        Ok(checkpoint_id) => (changes, None, checkpoint_id),
        Err(error) => (
            Vec::new(),
            Some(tracking_error_code(error).to_owned()),
            None,
        ),
    };
    drop(retained_target);
    result
}

fn tracking_error_code(error: crate::file_changes::TrackingError) -> &'static str {
    match error {
        crate::file_changes::TrackingError::WorkspaceUnavailable => "workspace_unavailable",
        crate::file_changes::TrackingError::WorkspaceTooLarge => "workspace_too_large",
        crate::file_changes::TrackingError::SnapshotUnavailable => "snapshot_unavailable",
        crate::file_changes::TrackingError::StorageUnavailable => "storage_unavailable",
    }
}

#[cfg(test)]
#[path = "executor/policy_tests.rs"]
mod policy_tests;

fn question_arguments_error() -> ServiceError {
    ServiceError::new(
        "question_data_invalid",
        "The AI sent an invalid question. Ask it to try again.",
        false,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateImageArguments {
    prompt: String,
    model: String,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    moderation: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    count: Option<u8>,
}

fn parse_image_request(arguments: &Value) -> Result<ImageGenerationRequest, ServiceError> {
    let arguments = serde_json::from_value::<GenerateImageArguments>(arguments.clone())
        .map_err(|_| image_request_invalid())?;
    let quality = match arguments.quality.as_deref() {
        None => None,
        Some("low") => Some(ImageQuality::Low),
        Some("medium") => Some(ImageQuality::Medium),
        Some("high") => Some(ImageQuality::High),
        Some("xhigh") => Some(ImageQuality::XHigh),
        Some("max") => Some(ImageQuality::Max),
        Some("auto") => Some(ImageQuality::Auto),
        Some(_) => return Err(image_request_invalid()),
    };
    let background = match arguments.background.as_deref() {
        None => None,
        Some("transparent") => Some(crate::chatgpt::ImageBackground::Transparent),
        Some("opaque") => Some(crate::chatgpt::ImageBackground::Opaque),
        Some("auto") => Some(crate::chatgpt::ImageBackground::Auto),
        Some(_) => return Err(image_request_invalid()),
    };
    let moderation = match arguments.moderation.as_deref() {
        None => None,
        Some("auto") => Some(crate::chatgpt::ImageModeration::Auto),
        Some("low") => Some(crate::chatgpt::ImageModeration::Low),
        Some(_) => return Err(image_request_invalid()),
    };
    let output_format = match arguments.output_format.as_deref() {
        None => None,
        Some("png") => Some(ImageOutputFormat::Png),
        Some("jpeg") => Some(ImageOutputFormat::Jpeg),
        Some("webp") => Some(ImageOutputFormat::Webp),
        Some(_) => return Err(image_request_invalid()),
    };
    Ok(ImageGenerationRequest {
        prompt: arguments.prompt,
        model: arguments.model,
        size: arguments.size,
        quality,
        background,
        moderation,
        output_format,
        output_compression: arguments.output_compression,
        count: arguments.count.unwrap_or(1),
    })
}

fn image_request_invalid() -> ServiceError {
    ServiceError::new(
        "image_request_invalid",
        "The image generation request is invalid.",
        false,
    )
}

fn tool_activity_storage_error() -> ServiceError {
    ServiceError::new(
        "tool_activity_storage_failed",
        "The tool result could not be saved to this conversation.",
        true,
    )
}

fn image_storage_persistence_error() -> ServiceError {
    ServiceError::new(
        "image_storage_unavailable",
        "The generated image was received but could not be attached to the response.",
        true,
    )
}

fn image_storage_cleanup_error() -> ServiceError {
    ServiceError::new(
        "image_storage_unavailable",
        "The generated image could not be safely finalized locally.",
        true,
    )
}

fn is_image_cancellation(error: &ServiceError) -> bool {
    matches!(error.code, "operation_cancelled" | "request_cancelled")
}

fn is_valid_wire_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn tool_error(code: &str, message: &str) -> Value {
    json!({"error": {"code": code, "message": message}})
}

fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_protocol_failed",
        "The local service could not send the tool activity update.",
        true,
    )
}

/// Recursively scans JSON values in tool outputs and redacts known credential patterns and keys.
pub(crate) fn redact_tool_output(value: &mut Value) {
    match value {
        Value::String(text) => {
            *text = crate::storage::tool_index_redaction::redact_tool_index_text(text);
        }
        Value::Array(items) => {
            for item in items {
                redact_tool_output(item);
            }
        }
        Value::Object(map) => {
            for val in map.values_mut() {
                redact_tool_output(val);
            }
        }
        _ => {}
    }
}
