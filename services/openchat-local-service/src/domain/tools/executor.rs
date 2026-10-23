//! Coordinates model-requested tools, permission rules, filesystem scopes, and per-turn limits.
//!
//! Execution delegates input validation and each operation to focused modules; permission mode
//! and project policy are checked before any operation is run.

use crate::{
    chat_operation::SubagentModelOption,
    chatgpt::{ChatGptService, ImageGenerationAuth, ImageGenerationRequest, ImageQuality},
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
mod validation;
#[cfg(test)]
pub(crate) use execution::execute_model_tool;
pub(crate) use execution::execute_model_tool_with_workspace_roots;
use paths::qualify_output_paths;
pub(crate) use validation::{PreparedToolCall, ToolOperation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Global policy applied before a model-requested tool is allowed to execute.
pub enum ToolPermissionMode {
    /// Denies tool calls so the model can produce a plan without taking action.
    Plan,
    /// Requires explicit approval for every operation.
    RequireApproval,
    /// Automatically approves only operations classified as safe by the executor.
    ApproveSafeOperations,
    /// Executes allowed operations without an additional per-call approval request.
    FullAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Per-tool project policy that asks, allows, or denies an operation.
pub enum ToolPermissionRule {
    /// Ask the user before running the operation.
    Ask,
    /// Allow the operation within the active permission mode and path scope.
    Allow,
    /// Deny the operation before it touches a resource.
    Deny,
}

pub(crate) type ToolPermissionRules = HashMap<String, ToolPermissionRule>;
const MAX_TOOL_PERMISSION_RULES: usize = 1080;

pub(crate) fn project_task_permission_rule<'a>(
    rules: &'a ToolPermissionRules,
    task_id: &str,
) -> Option<&'a ToolPermissionRule> {
    super::project_tasks::permission_rule_name(task_id)
        .and_then(|name| rules.get(&name))
        .or_else(|| rules.get("run_project_task"))
}

#[cfg(windows)]
fn mcp_tool_permission_rule(
    rules: &ToolPermissionRules,
    server_id: &str,
    tool_name: &str,
) -> ToolPermissionRule {
    let server_rule = super::mcp::server_permission_rule(rules, server_id);
    if server_rule == ToolPermissionRule::Deny {
        return ToolPermissionRule::Deny;
    }
    rules
        .get(&format!("mcp__{server_id}__{tool_name}"))
        .copied()
        .unwrap_or(server_rule)
}

pub(crate) fn parse_tool_permission_rules(
    value: Option<&Value>,
) -> Result<ToolPermissionRules, ServiceError> {
    let Some(value) = value else {
        return Ok(ToolPermissionRules::new());
    };
    let Some(rules) = value.as_object() else {
        return Err(invalid_tool_permission_rules());
    };
    if rules.len() > MAX_TOOL_PERMISSION_RULES {
        return Err(invalid_tool_permission_rules());
    }

    let mut parsed = ToolPermissionRules::with_capacity(rules.len());
    for (name, rule) in rules {
        let rule = match rule.as_str() {
            Some("ask") => ToolPermissionRule::Ask,
            Some("allow") => ToolPermissionRule::Allow,
            Some("deny") => ToolPermissionRule::Deny,
            _ => return Err(invalid_tool_permission_rules()),
        };
        if matches!(name.as_str(), "git_status" | "git_diff" | "git_history") {
            continue;
        }
        if !is_permission_rule_tool(name) {
            return Err(invalid_tool_permission_rules());
        }
        parsed.insert(name.clone(), rule);
    }
    Ok(parsed)
}

fn is_permission_rule_tool(name: &str) -> bool {
    if super::project_tasks::is_permission_rule_name(name) {
        return true;
    }
    if super::project_tools::is_permission_rule_name(name) {
        return true;
    }
    if let Some(mcp_tool_prefix) = name.strip_prefix("mcp__") {
        #[cfg(windows)]
        return super::mcp::valid_permission_rule_suffix(mcp_tool_prefix);
        #[cfg(not(windows))]
        return {
            let _ = mcp_tool_prefix;
            false
        };
    }
    #[cfg(windows)]
    if name == "computer_use" {
        return true;
    }
    matches!(
        name,
        "list_files"
            | "search_files"
            | "read_file"
            | "write_file"
            | "edit_file"
            | "get_file_info"
            | "execute_command"
            | "send_terminal_input"
            | "web_search"
            | "read_url_content"
            | "save_memory"
            | "run_project_task"
            | "delegate_task"
            | "list_subagent_models"
            | "get_subagent_result"
            | "cancel_subagent"
    )
}

fn invalid_tool_permission_rules() -> ServiceError {
    ServiceError::new(
        "invalid_tool_permission_rules",
        "One or more project tool permission rules are invalid.",
        false,
    )
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
    project_roots: Vec<PathBuf>,
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

#[derive(Default)]
struct ToolTurnState {
    round_sequence: usize,
    current_round_id: Option<String>,
}

enum PendingFileChangeCapture {
    File {
        path: String,
        before:
            Result<Option<crate::file_changes::FileSnapshot>, crate::file_changes::TrackingError>,
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
            project_roots: project_root.map(Path::to_path_buf).into_iter().collect(),
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
                return Ok(ToolResult { call_id: call.id.clone(), output });
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
                    request_id, "computer_use", "desktop",
                    &call.arguments, events, cancellation,
                ).await;
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
                        ).await?;
                        return Err(error);
                    }
                    Err(error) => return Err(error),
                };
                if !approved {
                    let output = tool_error("permission_denied", "Computer Use was not approved. Do not retry unless the user asks.");
                    self.emit_activity(
                        storage,
                        ToolActivity::denied(&activity_call, "desktop".to_owned(), output.clone()),
                        request_id,
                        snapshot,
                        events,
                    ).await?;
                    return Ok(ToolResult { call_id: call.id.clone(), output });
                }
            }
            *self.computer_use_approved.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
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
        ).await?;
        let result = self.computer_use.execute(&call.arguments);
        if let Some(image) = result.image {
            self.computer_use_images.lock().await.insert(call.id.clone(), image);
        }
        let activity = ToolActivity::finished(
            &activity_call,
            Some("desktop".to_owned()),
            result.output.clone(),
        );
        self.emit_activity(storage, activity, request_id, snapshot, events).await?;
        Ok(ToolResult { call_id: call.id.clone(), output: result.output })
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

    fn validate_tool_name(&self, tool_name: &str) -> Result<(), ServiceError> {
        if self.allowed_tool_names.contains(tool_name) {
            Ok(())
        } else {
            Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested a tool that was not included in the request.",
                false,
            ))
        }
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
        self.validate_tool_name(&call.name)?;
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

        if matches!(&prepared.operation, ToolOperation::SaveMemory { .. })
            && snapshot.conversation_id.starts_with("tmp_")
        {
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output: tool_error(
                    "temporary_chat_memory_unavailable",
                    "Personal memory is unavailable in temporary chats.",
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

        if rule == ToolPermissionRule::Ask {
            let approved_target = prepared.target_path.clone();
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
                prepared.operation,
                ToolOperation::Bash { .. }
                    | ToolOperation::SendTerminalInput { .. }
                    | ToolOperation::WebSearch { .. }
                    | ToolOperation::ReadUrlContent { .. }
                    | ToolOperation::SaveMemory { .. }
                    | ToolOperation::DelegateTask { .. }
                    | ToolOperation::ProjectTool { .. }
            );
            if !is_non_fs {
                match self.resolve_tool_path(
                    &prepared.requested_path,
                    prepared.operation.targets_directory(),
                    prepared.operation.can_create_file(),
                ) {
                    Ok((root, relative_path, target, scope)) if target == approved_target => {
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
        let file_change_capture = capture_file_change_before(&prepared, &self.project_roots);
        let mut output = match &prepared.operation {
            ToolOperation::SaveMemory { fact } => {
                save_personal_memory(storage, &snapshot.conversation_id, fact)
            }
            ToolOperation::ListSubagentModels => json!({
                "models": self.subagent_models.iter().map(|model| json!({
                    "modelKey": model.key,
                    "providerId": model.provider_id,
                    "modelId": model.model_id,
                    "displayName": model.display_name,
                    "supportsTools": model.supports_tools,
                    "reasoningLevels": model.reasoning_levels,
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
                                        child_image_generation.is_some(),
                                    );
                                    crate::chatgpt::subagents::start_child_analysis(
                                        crate::chatgpt::subagents::ChildRunTask {
                                            service: service.clone(),
                                            call: call.clone(),
                                            task: task.clone(),
                                            additional_context: context.clone(),
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
                    .execute_project_tool(
                        executable,
                        &prepared.root,
                        &serialized_input,
                        *timeout_seconds,
                        *output_limit_bytes,
                        cancellation,
                        self.permission_mode == ToolPermissionMode::FullAccess,
                    )
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
        if matches!(
            call.name.as_str(),
            "web_search" | "read_url_content" | "read_url"
        ) {
            qualify_citation_source_ids(&mut output, &call.id);
        }
        qualify_output_paths(&mut output, &prepared, &self.project_roots);
        redact_tool_output(&mut output);
        let (file_changes, file_changes_error) = finish_file_change_capture(
            storage,
            &snapshot.conversation_id,
            &call.name,
            &prepared,
            file_change_capture,
        );
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
        let target = format!("MCP server {server_id}, tool {tool_name}");
        let tool_rule = mcp_tool_permission_rule(&self.permission_rules, server_id, tool_name);
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

        let result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                service,
                connection_id,
                workspace_id,
                turn_id,
                ..
            } => {
                service
                    .generate_image(
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
                    .generate_image(
                        &request,
                        ImageGenerationAuth::ApiKey(api_key),
                        None,
                        cancellation,
                    )
                    .await
            }
            ImageGenerationContext::CompatibleApiKey { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider route.",
                false,
            )),
            ImageGenerationContext::LocalEndpoint { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this local route.",
                false,
            )),
        };
        let result = match result {
            Ok(result) => result,
            Err(error) if is_image_cancellation(&error) => {
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
            let error = if cleanup_result.is_err() {
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

fn capture_file_change_before(
    prepared: &PreparedToolCall,
    project_roots: &[PathBuf],
) -> PendingFileChangeCapture {
    match &prepared.operation {
        ToolOperation::Write { .. } | ToolOperation::Edit { .. } => {
            PendingFileChangeCapture::File {
                path: prepared.relative_path.clone(),
                before: crate::file_changes::capture_file(&prepared.root, &prepared.relative_path),
            }
        }
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
) -> (Vec<crate::file_changes::FileChangeSummary>, Option<String>) {
    let grouped_deltas = match capture {
        PendingFileChangeCapture::None => return (Vec::new(), None),
        PendingFileChangeCapture::File { path, before } => {
            let Ok(before) = before else {
                return (Vec::new(), Some("snapshot_unavailable".to_owned()));
            };
            let after = match crate::file_changes::capture_file(&prepared.root, &path) {
                Ok(after) => after,
                Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
            };
            let before_hash = before.as_ref().map(|file| file.hash.as_str());
            let after_hash = after.as_ref().map(|file| file.hash.as_str());
            if before_hash == after_hash {
                Vec::new()
            } else {
                vec![(
                    prepared.root.clone(),
                    vec![crate::file_changes::FileChangeDelta {
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
                    );
                };
                let after = match crate::file_changes::capture_workspace(&root) {
                    Ok(after) => after,
                    Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
                };
                let deltas = match crate::file_changes::workspace_deltas(&before, &after) {
                    Ok(deltas) => deltas,
                    Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
                };
                if !deltas.is_empty() {
                    changes.push((root, deltas));
                }
            }
            changes
        }
    };

    if grouped_deltas.is_empty() {
        return (Vec::new(), None);
    }
    let mut changes = Vec::new();
    for (root, deltas) in &grouped_deltas {
        match crate::file_changes::record_deltas(storage, conversation_id, root, deltas.clone()) {
            Ok(root_changes) => changes.extend(root_changes),
            Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
        }
    }
    match crate::file_changes::checkpoints::persist_checkpoint(
        storage,
        conversation_id,
        tool_name,
        &grouped_deltas,
    ) {
        Ok(()) => (changes, None),
        Err(error) => (Vec::new(), Some(tracking_error_code(error).to_owned())),
    }
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
mod policy_tests {
    use std::path::Path;

    use serde_json::json;

    use super::{
        ToolCall, ToolExecutor, ToolPermissionMode, ToolPermissionRule, ToolPermissionRules,
        parse_tool_permission_rules, qualify_citation_source_ids,
    };
    use crate::tools::executor::validation::ToolOperation;

    #[test]
    fn rejects_calls_that_were_not_advertised_to_the_model() {
        let executor = ToolExecutor::with_allowed_tool_names(
            None,
            Path::new("."),
            ToolPermissionMode::FullAccess,
            ["read_file".to_owned()],
        );

        assert!(executor.validate_tool_name("read_file").is_ok());
        assert_eq!(
            executor
                .validate_tool_name("ask_user")
                .expect_err("unadvertised question tool must be rejected")
                .code,
            "invalid_provider_response"
        );
    }

    #[test]
    fn citation_source_ids_are_unique_to_the_tool_call() {
        let mut output = json!({
            "sourceId": "U1",
            "results": [{"sourceId": "S1"}, {"sourceId": "S2"}]
        });

        qualify_citation_source_ids(&mut output, "call_1234abcd");

        assert_eq!(output["sourceId"], "U1-call1234");
        assert_eq!(output["results"][0]["sourceId"], "S1-call1234");
        assert_eq!(output["results"][1]["sourceId"], "S2-call1234");
    }

    #[test]
    fn project_tool_rules_override_the_global_mode_without_changing_its_default() {
        let operation = ToolOperation::Write {
            content: "content".to_owned(),
        };
        let mut rules = ToolPermissionRules::new();
        rules.insert("write_file".to_owned(), ToolPermissionRule::Allow);
        rules.insert("execute_command".to_owned(), ToolPermissionRule::Deny);
        let executor = ToolExecutor::with_permission_rules(
            None,
            Path::new("."),
            ToolPermissionMode::RequireApproval,
            ["write_file".to_owned(), "execute_command".to_owned()],
            rules,
        );

        assert_eq!(
            executor.permission_rule(
                &ToolCall {
                    id: "call_write".to_owned(),
                    name: "write_file".to_owned(),
                    arguments: json!({}),
                },
                &operation,
                None,
            ),
            ToolPermissionRule::Allow
        );
        assert_eq!(
            executor.permission_rule(
                &ToolCall {
                    id: "call_command".to_owned(),
                    name: "execute_command".to_owned(),
                    arguments: json!({}),
                },
                &operation,
                None,
            ),
            ToolPermissionRule::Deny
        );
        assert_eq!(
            executor.permission_rule(
                &ToolCall {
                    id: "call_read".to_owned(),
                    name: "read_file".to_owned(),
                    arguments: json!({}),
                },
                &operation,
                None,
            ),
            ToolPermissionRule::Ask
        );
    }

    #[test]
    fn plan_mode_denies_every_tool_even_when_project_rules_allow_them() {
        let mut rules = ToolPermissionRules::new();
        rules.insert("write_file".to_owned(), ToolPermissionRule::Allow);
        rules.insert("read_file".to_owned(), ToolPermissionRule::Deny);
        let executor = ToolExecutor::with_permission_rules(
            None,
            Path::new("."),
            ToolPermissionMode::Plan,
            ["write_file".to_owned(), "read_file".to_owned()],
            rules,
        );

        let write_call = ToolCall {
            id: "call_write".to_owned(),
            name: "write_file".to_owned(),
            arguments: json!({}),
        };
        assert_eq!(
            executor.permission_rule(
                &write_call,
                &ToolOperation::Write {
                    content: "change".to_owned(),
                },
                None,
            ),
            ToolPermissionRule::Deny
        );

        let read_call = ToolCall {
            id: "call_read".to_owned(),
            name: "read_file".to_owned(),
            arguments: json!({}),
        };
        assert_eq!(
            executor.permission_rule(
                &read_call,
                &ToolOperation::Read {
                    start_line: 1,
                    line_count: 10,
                },
                None,
            ),
            ToolPermissionRule::Deny
        );

        let default_read_executor = ToolExecutor::with_allowed_tool_names(
            None,
            Path::new("."),
            ToolPermissionMode::Plan,
            ["read_file".to_owned()],
        );
        assert_eq!(
            default_read_executor.permission_rule(
                &read_call,
                &ToolOperation::Read {
                    start_line: 1,
                    line_count: 10,
                },
                None,
            ),
            ToolPermissionRule::Deny
        );

        let web_call = ToolCall {
            id: "call_search".to_owned(),
            name: "web_search".to_owned(),
            arguments: json!({}),
        };
        let web_operation = ToolOperation::WebSearch {
            query: "Rust documentation".to_owned(),
            limit: Some(3),
        };
        assert!(!web_operation.is_safe_for_auto_approval());
        assert_eq!(
            default_read_executor.permission_rule(&web_call, &web_operation, None),
            ToolPermissionRule::Deny
        );
    }

    #[test]
    fn named_task_rules_override_the_project_wide_task_rule() {
        let operation = ToolOperation::Bash {
            command: Some("cargo test".to_owned()),
            terminal_id: None,
            input: None,
            action: None,
            timeout_seconds: Some(60),
            wait_ms: None,
        };
        let rules = parse_tool_permission_rules(Some(&json!({
            "run_project_task": "allow",
            "run_project_task__verify": "deny"
        })))
        .expect("valid named task rules");
        let executor = ToolExecutor::with_permission_rules(
            None,
            Path::new("."),
            ToolPermissionMode::FullAccess,
            ["run_project_task".to_owned()],
            rules,
        );
        let call = ToolCall {
            id: "call_verify".to_owned(),
            name: "run_project_task".to_owned(),
            arguments: json!({"task": "verify"}),
        };
        assert_eq!(
            executor.permission_rule(&call, &operation, None),
            ToolPermissionRule::Deny
        );
        let other_call = ToolCall {
            id: "call_lint".to_owned(),
            name: "run_project_task".to_owned(),
            arguments: json!({"task": "lint"}),
        };
        assert_eq!(
            executor.permission_rule(&other_call, &operation, None),
            ToolPermissionRule::Allow
        );
        assert_eq!(
            super::project_task_permission_rule(&executor.permission_rules, "verify"),
            Some(&ToolPermissionRule::Deny)
        );
    }

    #[test]
    fn permission_rule_parser_rejects_unknown_tools_and_values() {
        let valid = json!({
            "execute_command": "ask",
            "git_diff": "allow",
            "project_tool__lint": "deny"
        });
        let rules = parse_tool_permission_rules(Some(&valid)).expect("valid rules");
        assert_eq!(rules["execute_command"], ToolPermissionRule::Ask);
        assert_eq!(rules["project_tool__lint"], ToolPermissionRule::Deny);

        #[cfg(windows)]
        {
            let mcp = json!({
                "mcp__local_docs__*": "allow",
                "mcp__local_docs__read-file": "deny"
            });
            let rules = parse_tool_permission_rules(Some(&mcp)).expect("valid MCP rule");
            assert_eq!(rules["mcp__local_docs__*"], ToolPermissionRule::Allow);
            assert_eq!(
                rules["mcp__local_docs__read-file"],
                ToolPermissionRule::Deny
            );
            assert!(
                parse_tool_permission_rules(Some(&json!({
                    "mcp__Invalid__*": "allow"
                })))
                .is_err()
            );
            assert!(
                parse_tool_permission_rules(Some(&json!({
                    "mcp__local_docs__read/file": "allow"
                })))
                .is_err()
            );
        }

        let invalid_tool = json!({"custom_command": "allow"});
        assert_eq!(
            parse_tool_permission_rules(Some(&invalid_tool))
                .expect_err("unknown tools must be rejected")
                .code,
            "invalid_tool_permission_rules"
        );
        let invalid_value = json!({"execute_command": "inherit"});
        assert_eq!(
            parse_tool_permission_rules(Some(&invalid_value))
                .expect_err("unknown rule values must be rejected")
                .code,
            "invalid_tool_permission_rules"
        );
        let invalid_task_rule = json!({"run_project_task__Verify": "allow"});
        assert_eq!(
            parse_tool_permission_rules(Some(&invalid_task_rule))
                .expect_err("task permission ids must use catalog id syntax")
                .code,
            "invalid_tool_permission_rules"
        );
    }

    #[cfg(windows)]
    #[test]
    fn permission_rule_parser_accepts_the_full_mcp_tool_limit_and_rejects_more() {
        let mut rules = serde_json::Map::new();
        for server in 0..8 {
            rules.insert(format!("mcp__server_{server}__*"), json!("ask"));
            for tool in 0..128 {
                rules.insert(format!("mcp__server_{server}__tool_{tool}"), json!("deny"));
            }
        }
        for tool in [
            "list_files",
            "search_files",
            "read_file",
            "write_file",
            "edit_file",
            "get_file_info",
            "execute_command",
            "send_terminal_input",
            "git_status",
            "git_diff",
            "git_history",
            "web_search",
            "read_url_content",
            "delegate_task",
            "run_project_task",
        ] {
            rules.insert(tool.to_owned(), json!("ask"));
        }
        for task in 0..32 {
            rules.insert(format!("run_project_task__task_{task}"), json!("deny"));
        }
        rules.insert("mcp__legacy_server__*".to_owned(), json!("allow"));
        assert_eq!(rules.len(), super::MAX_TOOL_PERMISSION_RULES);
        assert!(
            parse_tool_permission_rules(Some(&serde_json::Value::Object(rules.clone()))).is_ok()
        );

        rules.insert("mcp__overflow_server__*".to_owned(), json!("allow"));
        assert!(parse_tool_permission_rules(Some(&serde_json::Value::Object(rules))).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn mcp_server_permission_defaults_to_ask_and_uses_local_project_override() {
        let mut rules = ToolPermissionRules::new();
        assert_eq!(
            crate::tools::mcp::server_permission_rule(&rules, "local_docs"),
            ToolPermissionRule::Ask
        );
        rules.insert("mcp__local_docs__*".to_owned(), ToolPermissionRule::Allow);
        rules.insert("mcp__remote_docs__*".to_owned(), ToolPermissionRule::Deny);
        assert_eq!(
            crate::tools::mcp::server_permission_rule(&rules, "local_docs"),
            ToolPermissionRule::Allow
        );
        assert_eq!(
            crate::tools::mcp::server_permission_rule(&rules, "remote_docs"),
            ToolPermissionRule::Deny
        );
        assert_eq!(
            super::mcp_tool_permission_rule(&rules, "local_docs", "read-file"),
            ToolPermissionRule::Allow
        );
        rules.insert(
            "mcp__local_docs__read-file".to_owned(),
            ToolPermissionRule::Deny,
        );
        assert_eq!(
            super::mcp_tool_permission_rule(&rules, "local_docs", "read-file"),
            ToolPermissionRule::Deny
        );
        assert_eq!(
            super::mcp_tool_permission_rule(&rules, "local_docs", "write-file"),
            ToolPermissionRule::Allow
        );
        rules.insert("mcp__local_docs__*".to_owned(), ToolPermissionRule::Deny);
        assert_eq!(
            super::mcp_tool_permission_rule(&rules, "local_docs", "write-file"),
            ToolPermissionRule::Deny
        );
    }

    #[test]
    fn trust_handoff_defense_requires_approval_even_in_full_access_mode() {
        use std::path::Path;
        let executor = ToolExecutor::new(None, Path::new("test"), ToolPermissionMode::FullAccess);
        let write_hook_call = ToolCall {
            id: "call_hook".to_owned(),
            name: "write_file".to_owned(),
            arguments: json!({"path": ".git/hooks/pre-commit", "content": "echo pwn"}),
        };
        let write_op = ToolOperation::Write {
            content: "echo pwn".to_owned(),
        };

        // In FullAccess, writing to .git/hooks must still result in Ask
        assert_eq!(
            executor.permission_rule(
                &write_hook_call,
                &write_op,
                Some(Path::new(".git/hooks/pre-commit"))
            ),
            ToolPermissionRule::Ask
        );

        // Normal files in FullAccess must be allowed
        let normal_call = ToolCall {
            id: "call_normal".to_owned(),
            name: "write_file".to_owned(),
            arguments: json!({"path": "src/main.rs", "content": "fn main() {}"}),
        };
        let normal_op = ToolOperation::Write {
            content: "fn main() {}".to_owned(),
        };
        assert_eq!(
            executor.permission_rule(&normal_call, &normal_op, Some(Path::new("src/main.rs"))),
            ToolPermissionRule::Allow
        );
    }

    #[test]
    fn redact_tool_output_masks_live_credentials_in_json() {
        let mut output = json!({
            "stdout": "Loaded configuration:\nOPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz1234567890\nDone.",
            "nested": {
                "token": "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
                "normal": "hello world"
            },
            "list": [
                "api_key: secret-value-1234",
                "normal item"
            ]
        });

        super::redact_tool_output(&mut output);

        let stdout = output["stdout"].as_str().unwrap();
        assert!(!stdout.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"));
        assert!(stdout.contains("[REDACTED]"));

        let nested_token = output["nested"]["token"].as_str().unwrap();
        assert!(!nested_token.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
        assert_eq!(nested_token, "[REDACTED]");

        let list_item = output["list"][0].as_str().unwrap();
        assert!(!list_item.contains("secret-value-1234"));
        assert!(list_item.contains("[REDACTED]"));

        assert_eq!(output["nested"]["normal"], "hello world");
        assert_eq!(output["list"][1], "normal item");
    }
}

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
    model: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
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
    Ok(ImageGenerationRequest {
        prompt: arguments.prompt,
        model: arguments.model.unwrap_or_else(|| "gpt-image-2".to_owned()),
        size: arguments.size,
        quality,
        background,
        output_format: None,
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
