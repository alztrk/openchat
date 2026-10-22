//! Prepares durable chat runs, resolves saved routing credentials, and finalizes outcomes.

use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{
    chatgpt_store, goals, instructions,
    openai_compatible::CustomProviderConfig,
    project_skills::ProjectSkill,
    protocol::ServiceError,
    provider_credentials::{self, ProviderCredentials},
    storage::{AppStorage, sqlite_error_code, user_questions::AgentRunStatus},
    tools::{self, ToolPermissionMode},
    user_question_broker::{RunOutcome, UserQuestionBroker},
};

/// Saved conversation routing information used to execute a durable chat send.
pub(super) struct SavedChatRoute {
    pub(super) provider_id: Option<String>,
    pub(super) project_root: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) stored_api_key_connection_id: Option<String>,
    pub(super) model_id: Option<String>,
    pub(super) connection_id: Option<String>,
    pub(super) workspace_id: Option<String>,
}

impl SavedChatRoute {
    /// Resolves only credentials referenced by the conversation's saved route.
    ///
    /// The renderer never supplies a credential for durable chat execution.
    ///
    /// # Errors
    /// Returns the credential store's stable error if the saved route cannot be resolved.
    pub(super) fn resolve_credentials(
        &self,
    ) -> Result<(Option<Zeroizing<String>>, Option<CustomProviderConfig>), ServiceError> {
        let Some(provider_id) = self
            .provider_id
            .as_deref()
            .filter(|id| provider_credentials::uses_saved_api_key(id))
        else {
            return Ok((None, None));
        };

        let credentials = ProviderCredentials::load()?;
        let connection_id = (provider_id != "opencode")
            .then_some(self.stored_api_key_connection_id.as_deref())
            .flatten();
        let api_key = credentials.api_key(provider_id, connection_id)?;
        let custom_provider = if provider_id.starts_with("custom_") {
            Some(credentials.custom_provider_config(provider_id)?)
        } else {
            None
        };

        Ok((api_key, custom_provider))
    }
}

/// Opens the chat database once and loads the conversation's saved route.
///
/// Returning the connection lets run and checkpoint operations reuse the same
/// request-scoped database handle.
///
/// # Errors
/// Returns `storage_unavailable` when the database cannot be opened or queried.
pub(super) fn load_saved_route(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<(Connection, SavedChatRoute), ServiceError> {
    let connection = storage.connect().map_err(|error| {
        log_chat_storage_failure(storage, "chat.send.route_open", &error);
        storage_unavailable()
    })?;
    let route = connection
        .query_row(
            "SELECT conversations.provider_id, projects.folder_path, projects.id,
                    conversations.api_key_connection_id, conversations.model_id,
                    conversations.connection_id, conversations.workspace_id
             FROM conversations
             LEFT JOIN projects ON projects.id = conversations.project_id
             WHERE conversations.id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| {
            log_chat_storage_failure(storage, "chat.send.route_read", &error);
            storage_unavailable()
        })?
        .unwrap_or((None, None, None, None, None, None, None));

    let (
        provider_id,
        project_root,
        project_id,
        stored_api_key_connection_id,
        model_id,
        connection_id,
        workspace_id,
    ) = route;

    Ok((
        connection,
        SavedChatRoute {
            provider_id,
            project_root,
            project_id,
            stored_api_key_connection_id,
            model_id,
            connection_id,
            workspace_id,
        },
    ))
}

/// Inputs needed to validate and persist the execution settings for one run.
pub(super) struct RunPreparation<'a> {
    /// Local store used to load project context and persist run state.
    pub(super) storage: &'a AppStorage,
    /// Request-scoped SQLite connection shared with route lookup and finalization.
    pub(super) connection: &'a Connection,
    /// Conversation whose provider route and transcript are being used.
    pub(super) conversation_id: &'a str,
    /// Saved provider and project route loaded for this conversation.
    pub(super) route: &'a SavedChatRoute,
    /// Existing run to resume, if the request continues a user question.
    pub(super) resume_run_id: Option<&'a str>,
    /// Requested reasoning effort for a new run.
    pub(super) requested_reasoning_effort: Option<&'a str>,
    /// Requested ChatGPT Fast mode setting.
    pub(super) requested_fast_mode: bool,
    /// Goal objective for a new durable goal run.
    pub(super) requested_goal_objective: Option<&'a str>,
    /// User-provided instructions for a new run.
    pub(super) requested_custom_instructions: Option<&'a str>,
    /// Skill IDs selected from the saved project.
    pub(super) requested_project_skill_ids: Vec<String>,
    /// Tool permission policy for this run.
    pub(super) permission_mode: ToolPermissionMode,
    /// Optional tool allowlist selected by the active profile.
    pub(super) profile_tool_allowlist: Option<HashSet<String>>,
}

/// Validated context and durable run state prepared for provider execution.
pub(super) struct PreparedChatRun {
    /// Durable ID used by progress events and question checkpoints.
    pub(super) run_id: String,
    /// Validated shared instructions saved for this run.
    pub(super) custom_instructions: Option<String>,
    /// Project instructions loaded or restored from a resume checkpoint.
    pub(super) project_instructions: Option<String>,
    /// Validated project skills loaded or restored from a resume checkpoint.
    pub(super) project_skills: Vec<ProjectSkill>,
    /// Bounded index excerpt selected for this request or restored from checkpoint.
    pub(super) project_index_context: Option<String>,
    /// Effective reasoning effort, restored from a resume checkpoint when needed.
    pub(super) reasoning_effort: Option<String>,
    /// Effective Fast mode setting, restored from a resume checkpoint when needed.
    pub(super) fast_mode: bool,
    /// Active durable goal state, if this run owns a goal.
    pub(super) goal_execution: Option<goals::GoalExecution>,
    /// Effective allowlist, restored from a resume checkpoint when needed.
    pub(super) profile_tool_allowlist: Option<HashSet<String>>,
}

/// Effective execution settings assembled from a new request or saved checkpoint.
struct PreparedRunState {
    /// Stable run identifier persisted with this execution.
    run_id: String,
    /// Validated user instructions saved for the run.
    custom_instructions: Option<String>,
    /// Project instructions, skills, and bounded index excerpt used by provider context.
    project_context: ProjectContext,
    /// Reasoning effort requested by the user or restored from the checkpoint.
    reasoning_effort: Option<String>,
    /// Whether this run uses the provider's supported Fast mode.
    fast_mode: bool,
    /// Durable goal state owned by this run, when one exists.
    goal_execution: Option<goals::GoalExecution>,
    /// Effective tool allowlist, restored from a checkpoint when resuming.
    profile_tool_allowlist: Option<HashSet<String>>,
}

/// Project-scoped context captured for one provider request and its resumable checkpoint.
struct ProjectContext {
    /// Validated project instruction text.
    instructions: Option<String>,
    /// Explicitly selected project skills.
    skills: Vec<ProjectSkill>,
    /// Bounded current index excerpt relevant to the latest user message.
    index_excerpt: Option<String>,
}

/// Validates goal and provider settings, restores or creates a run, and loads its project context.
///
/// Validation order and write boundaries are kept stable: goal and Fast mode checks run first,
/// project context is loaded before a new run is persisted, and the initial checkpoint is saved
/// atomically with that run.
///
/// # Errors
/// Returns the existing goal, checkpoint, project, or storage error when preparation cannot finish.
pub(super) fn prepare_run(input: RunPreparation<'_>) -> Result<PreparedChatRun, ServiceError> {
    let RunPreparation {
        storage,
        connection,
        conversation_id,
        route,
        resume_run_id,
        requested_reasoning_effort,
        requested_fast_mode,
        requested_goal_objective,
        requested_custom_instructions,
        requested_project_skill_ids,
        permission_mode,
        profile_tool_allowlist,
    } = input;

    validate_goal_start(
        storage,
        connection,
        conversation_id,
        requested_goal_objective,
    )?;
    validate_fast_mode(route.provider_id.as_deref(), requested_fast_mode)?;

    let mut prepared = if let Some(run_id) = resume_run_id {
        restore_run(storage, connection, conversation_id, route, run_id)?
    } else {
        let project_context = load_project_context(
            storage,
            conversation_id,
            route,
            permission_mode,
            &requested_project_skill_ids,
        )?;
        create_run(CreateRun {
            connection,
            conversation_id,
            route,
            goal_objective: requested_goal_objective,
            custom_instructions: requested_custom_instructions,
            reasoning_effort: requested_reasoning_effort,
            fast_mode: requested_fast_mode,
            project_context,
            profile_tool_allowlist,
        })?
    };

    if permission_mode == ToolPermissionMode::Plan {
        prepared.project_context = ProjectContext {
            instructions: None,
            skills: Vec::new(),
            index_excerpt: None,
        };
    }

    Ok(PreparedChatRun {
        run_id: prepared.run_id,
        custom_instructions: prepared.custom_instructions,
        project_instructions: prepared.project_context.instructions,
        project_skills: prepared.project_context.skills,
        project_index_context: prepared.project_context.index_excerpt,
        reasoning_effort: prepared.reasoning_effort,
        fast_mode: prepared.fast_mode,
        goal_execution: prepared.goal_execution,
        profile_tool_allowlist: prepared.profile_tool_allowlist,
    })
}

/// Ensures a new goal is tied to the latest user message and no other goal is active.
fn validate_goal_start(
    storage: &AppStorage,
    connection: &Connection,
    conversation_id: &str,
    objective: Option<&str>,
) -> Result<(), ServiceError> {
    let Some(objective) = objective else {
        return Ok(());
    };
    let messages =
        chatgpt_store::conversation_messages(storage, conversation_id).map_err(|error| {
            log_chat_storage_failure(storage, "chat.send.goal_history_read", &error);
            ServiceError::new(
                "storage_unavailable",
                "Chat history could not be read.",
                false,
            )
        })?;
    if messages
        .last()
        .is_none_or(|message| message.role != "user" || message.content.trim() != objective)
    {
        return Err(super::chat_request::invalid_chat_goal_request(
            "goal objective",
        ));
    }
    if crate::storage::user_questions::latest_active_goal_run(connection, conversation_id)
        .map_err(super::map_question_storage_error)?
        .is_some()
    {
        return Err(ServiceError::new(
            "goal_already_active",
            "Resume or stop the active goal before starting another goal in this chat.",
            false,
        ));
    }
    Ok(())
}

/// Rejects Fast mode when the conversation's saved provider does not support it.
fn validate_fast_mode(
    provider_id: Option<&str>,
    requested_fast_mode: bool,
) -> Result<(), ServiceError> {
    if requested_fast_mode && !matches!(provider_id, Some("chatgpt" | "chatgpt_api")) {
        return Err(ServiceError::new(
            "invalid_request_params",
            "Fast mode is only available for supported ChatGPT models.",
            false,
        ));
    }
    Ok(())
}

/// Validates a saved continuation checkpoint and claims its run for a single resume.
///
/// All checkpoint validation completes before the resume claim is persisted.
fn restore_run(
    storage: &AppStorage,
    connection: &Connection,
    conversation_id: &str,
    route: &SavedChatRoute,
    run_id: &str,
) -> Result<PreparedRunState, ServiceError> {
    let run = crate::storage::user_questions::load_run_for_conversation(
        connection,
        conversation_id,
        run_id,
    )
    .map_err(super::map_question_storage_error)?;
    let checkpoint = run.checkpoint.as_ref().ok_or_else(|| {
        ServiceError::new(
            "question_run_checkpoint_missing",
            "The pending AI response has no saved continuation data.",
            false,
        )
    })?;
    let profile_tool_allowlist =
        tools::parse_profile_tool_allowlist(checkpoint.get("profileToolAllowlist"))?;
    if checkpoint.get("providerId").and_then(Value::as_str) != route.provider_id.as_deref()
        || checkpoint.get("modelId").and_then(Value::as_str) != route.model_id.as_deref()
        || checkpoint.get("connectionId").and_then(Value::as_str) != route.connection_id.as_deref()
        || checkpoint.get("workspaceId").and_then(Value::as_str) != route.workspace_id.as_deref()
        || checkpoint
            .get("providerConnectionId")
            .or_else(|| checkpoint.get("apiKeyConnectionId"))
            .and_then(Value::as_str)
            != route.stored_api_key_connection_id.as_deref()
    {
        return Err(ServiceError::new(
            "question_route_changed",
            "The conversation model changed while the AI was waiting. Restore the original model to continue.",
            false,
        ));
    }

    let custom_instructions = checkpoint
        .get("customInstructions")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let project_context = ProjectContext {
        instructions: restore_project_instructions(checkpoint)?,
        skills: crate::project_skills::from_checkpoint(checkpoint.get("projectSkills"))?,
        index_excerpt: restore_project_index_excerpt(checkpoint)?,
    };
    let reasoning_effort = checkpoint
        .get("reasoningEffort")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let fast_mode = restore_fast_mode(checkpoint, route.provider_id.as_deref())?;
    instructions::validate_custom_instructions(custom_instructions.as_deref())
        .map_err(|message| ServiceError::new("question_run_checkpoint_invalid", message, false))?;
    let mut goal_execution = if checkpoint.get("goal").is_some() {
        Some(goals::execution_from_run(run.clone())?)
    } else {
        None
    };

    crate::storage::user_questions::claim_run_resume(connection, conversation_id, run_id)
        .map_err(super::map_question_storage_error)?;
    if let Some(goal) = goal_execution.as_mut() {
        goals::set_running(storage, goal)?;
    }

    Ok(PreparedRunState {
        run_id: run_id.to_owned(),
        custom_instructions,
        project_context,
        reasoning_effort,
        fast_mode,
        goal_execution,
        profile_tool_allowlist,
    })
}

/// Restores project instructions only when the checkpoint contains a valid string value.
fn restore_project_instructions(checkpoint: &Value) -> Result<Option<String>, ServiceError> {
    match checkpoint.get("projectInstructions") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            crate::project_instructions::validate(value).map_err(|_| {
                ServiceError::new(
                    "question_run_checkpoint_invalid",
                    "The saved project instructions are invalid.",
                    false,
                )
            })?;
            Ok(Some(value.clone()))
        }
        Some(_) => Err(ServiceError::new(
            "question_run_checkpoint_invalid",
            "The saved project instructions are invalid.",
            false,
        )),
    }
}

/// Restores the bounded project index excerpt saved for a pending provider turn.
fn restore_project_index_excerpt(checkpoint: &Value) -> Result<Option<String>, ServiceError> {
    match checkpoint.get("projectIndexContext") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.len() <= 12 * 1024 => Ok(Some(value.clone())),
        _ => Err(ServiceError::new(
            "question_run_checkpoint_invalid",
            "The saved project index context is invalid.",
            false,
        )),
    }
}

/// Validates the checkpoint's Fast mode flag against its saved provider route.
fn restore_fast_mode(checkpoint: &Value, provider_id: Option<&str>) -> Result<bool, ServiceError> {
    let enabled = match checkpoint.get("fastMode") {
        None => false,
        Some(Value::Bool(enabled)) => *enabled,
        _ => {
            return Err(ServiceError::new(
                "question_run_checkpoint_invalid",
                "The saved Fast mode setting is invalid.",
                false,
            ));
        }
    };
    if enabled && !matches!(provider_id, Some("chatgpt" | "chatgpt_api")) {
        return Err(ServiceError::new(
            "question_run_checkpoint_invalid",
            "The saved Fast mode setting does not match the selected provider.",
            false,
        ));
    }
    Ok(enabled)
}

/// Loads bounded project context before any new run state is written.
fn load_project_context(
    storage: &AppStorage,
    conversation_id: &str,
    route: &SavedChatRoute,
    permission_mode: ToolPermissionMode,
    requested_project_skill_ids: &[String],
) -> Result<ProjectContext, ServiceError> {
    let instructions = if permission_mode == ToolPermissionMode::Plan {
        None
    } else {
        route
            .project_root
            .as_deref()
            .map(|root| crate::project_instructions::load(Path::new(root)))
            .transpose()?
            .flatten()
    };
    let skills = if permission_mode == ToolPermissionMode::Plan {
        Vec::new()
    } else {
        match route.project_root.as_deref() {
            Some(root) => {
                crate::project_skills::load_selected(Path::new(root), requested_project_skill_ids)?
            }
            None if requested_project_skill_ids.is_empty() => Vec::new(),
            None => return Err(super::invalid_request_params()),
        }
    };
    let index_excerpt = if permission_mode == ToolPermissionMode::Plan {
        None
    } else if let (Some(project_id), Some(root)) =
        (route.project_id.as_deref(), route.project_root.as_deref())
    {
        let index_path = storage.root().join("cache").join("project-index.sqlite3");
        let state = crate::project_index::snapshot(&index_path, project_id)?;
        if state.enabled {
            let state = crate::project_index::sync(&index_path, project_id, Path::new(root))?;
            if state.status == "stale" {
                return Err(ServiceError::new(
                    "project_index_stale",
                    "The project index could not be fully synchronized. Retry sync in project options or disable the index.",
                    true,
                ));
            }
            let messages = chatgpt_store::conversation_messages(storage, conversation_id).map_err(
                |error| {
                    log_chat_storage_failure(storage, "chat.send.project_history_read", &error);
                    ServiceError::new(
                        "storage_unavailable",
                        "Chat history could not be read.",
                        false,
                    )
                },
            )?;
            messages
                .iter()
                .rev()
                .find(|message| message.role == "user")
                .map(|message| {
                    crate::project_index::search_current(&index_path, project_id, &message.content)
                })
                .transpose()?
                .flatten()
        } else {
            None
        }
    } else {
        None
    };
    Ok(ProjectContext {
        instructions,
        skills,
        index_excerpt,
    })
}

/// Inputs used to persist a new durable run after its request context is validated.
struct CreateRun<'a> {
    connection: &'a Connection,
    conversation_id: &'a str,
    route: &'a SavedChatRoute,
    goal_objective: Option<&'a str>,
    custom_instructions: Option<&'a str>,
    reasoning_effort: Option<&'a str>,
    fast_mode: bool,
    project_context: ProjectContext,
    profile_tool_allowlist: Option<HashSet<String>>,
}

/// Atomically stores a new run with the checkpoint required for later continuation.
fn create_run(input: CreateRun<'_>) -> Result<PreparedRunState, ServiceError> {
    let CreateRun {
        connection,
        conversation_id,
        route,
        goal_objective,
        custom_instructions,
        reasoning_effort,
        fast_mode,
        project_context,
        profile_tool_allowlist,
    } = input;
    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let mut checkpoint = json!({
        "version": 1,
        "providerId": route.provider_id,
        "modelId": route.model_id,
        "connectionId": route.connection_id,
        "workspaceId": route.workspace_id,
        "providerConnectionId": route.stored_api_key_connection_id,
        "customInstructions": custom_instructions,
        "projectInstructions": project_context.instructions,
        "projectSkills": project_context.skills,
        "projectIndexContext": project_context.index_excerpt,
        "reasoningEffort": reasoning_effort,
        "fastMode": fast_mode,
        "profileToolAllowlist": profile_tool_allowlist.as_ref().map(|names| {
            let mut names = names.iter().cloned().collect::<Vec<_>>();
            names.sort();
            names
        }),
    });
    if let Some(objective) = goal_objective {
        goals::initial_checkpoint(
            &mut checkpoint,
            objective,
            super::chat_request::current_time_unix_ms()?,
        );
    }
    crate::storage::user_questions::create_run_with_checkpoint(
        connection,
        &crate::storage::user_questions::NewAgentRun {
            run_id: run_id.clone(),
            conversation_id: conversation_id.to_owned(),
        },
        &checkpoint,
    )
    .map_err(super::map_question_storage_error)?;
    let goal_execution = if goal_objective.is_some() {
        let run = crate::storage::user_questions::load_run_for_conversation(
            connection,
            conversation_id,
            &run_id,
        )
        .map_err(super::map_question_storage_error)?;
        Some(goals::execution_from_run(run)?)
    } else {
        None
    };

    Ok(PreparedRunState {
        run_id,
        custom_instructions: custom_instructions.map(str::to_owned),
        project_context,
        reasoning_effort: reasoning_effort.map(str::to_owned),
        fast_mode,
        goal_execution,
        profile_tool_allowlist,
    })
}

/// Persists the terminal or recoverable state after provider execution.
///
/// Goal state is updated before the broker marks the associated run finished so
/// the stored goal and run lifecycle remain aligned.
///
/// # Errors
/// Returns a stable storage or goal-state error if finalization cannot complete.
pub(super) async fn finish_run(
    context: ChatRunFinalization<'_>,
    result: Result<Value, ServiceError>,
) -> Result<Value, ServiceError> {
    let ChatRunFinalization {
        storage,
        connection,
        conversation_id,
        run_id,
        resuming_question_run,
        shutdown_requested,
        user_question_broker,
    } = context;
    let current_run = crate::storage::user_questions::load_run_for_conversation(
        &connection,
        conversation_id,
        run_id,
    )
    .map_err(super::map_question_storage_error)?;
    let is_goal_run = current_run
        .checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.get("goal"))
        .is_some();
    let outcome = classify_run_outcome(
        &result,
        current_run.status,
        is_goal_run,
        resuming_question_run,
        shutdown_requested.load(Ordering::SeqCst),
    );

    if is_goal_run && current_run.status == AgentRunStatus::Running {
        let mut goal = goals::execution_from_run(current_run.clone())?;
        let (state, reason) = match outcome {
            RunOutcome::Completed => ("completed", None),
            RunOutcome::Paused => {
                let reason = result
                    .as_ref()
                    .err()
                    .map(|error| {
                        if super::chat_request::is_quota_or_rate_limit_error(error) {
                            "quota"
                        } else {
                            "request_failed"
                        }
                    })
                    .or_else(|| {
                        current_run
                            .checkpoint
                            .as_ref()
                            .and_then(|checkpoint| checkpoint.pointer("/goal/pauseReason"))
                            .and_then(Value::as_str)
                    });
                ("paused", reason)
            }
            RunOutcome::Cancelled => ("cancelled", Some("stopped_by_user")),
            RunOutcome::Interrupted => ("interrupted", Some("interrupted")),
            RunOutcome::Failed => ("failed", Some("request_failed")),
        };
        goals::save_final_state(storage, &mut goal, state, reason)?;
    }

    user_question_broker
        .finish_run(storage, conversation_id, run_id, outcome)
        .await?;
    result
}

/// Owns the scoped database connection and state needed to finish a chat run.
pub(super) struct ChatRunFinalization<'a> {
    /// Storage used when persisting a goal transition and run outcome.
    pub(super) storage: &'a AppStorage,
    /// Connection used to read the run state after provider execution.
    pub(super) connection: Connection,
    /// Conversation that owns the run.
    pub(super) conversation_id: &'a str,
    /// Run whose outcome is being finalized.
    pub(super) run_id: &'a str,
    /// Whether the provider call resumed a saved user-question run.
    pub(super) resuming_question_run: bool,
    /// Signal set when service shutdown, rather than user cancellation, interrupted the call.
    pub(super) shutdown_requested: &'a AtomicBool,
    /// Broker responsible for updating the durable run status.
    pub(super) user_question_broker: &'a UserQuestionBroker,
}

/// Maps provider, question, and shutdown states to the durable run outcome.
fn classify_run_outcome(
    result: &Result<Value, ServiceError>,
    status: AgentRunStatus,
    is_goal_run: bool,
    resuming_question_run: bool,
    shutdown_requested: bool,
) -> RunOutcome {
    match result {
        Ok(_) if is_goal_run && status == AgentRunStatus::Paused => RunOutcome::Paused,
        Ok(_) if is_goal_run && status == AgentRunStatus::Cancelled => RunOutcome::Cancelled,
        Err(_) if is_goal_run && status == AgentRunStatus::Paused => RunOutcome::Paused,
        Err(_) if is_goal_run && status == AgentRunStatus::Cancelled => RunOutcome::Cancelled,
        Ok(response)
            if response
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "paused") =>
        {
            RunOutcome::Paused
        }
        Ok(response)
            if response
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "stopped") =>
        {
            if is_goal_run && status == AgentRunStatus::Paused {
                RunOutcome::Paused
            } else {
                RunOutcome::Cancelled
            }
        }
        Ok(_) => RunOutcome::Completed,
        Err(error)
            if is_goal_run
                && (super::chat_request::is_quota_or_rate_limit_error(error)
                    || error.retryable) =>
        {
            RunOutcome::Paused
        }
        Err(error) if error.code == "operation_cancelled" => {
            if shutdown_requested {
                RunOutcome::Interrupted
            } else if is_goal_run && status == AgentRunStatus::Paused {
                RunOutcome::Paused
            } else {
                RunOutcome::Cancelled
            }
        }
        Err(error)
            if error.code == "question_delivery_failed"
                || error.code == "question_wait_interrupted"
                || error.code == "question_storage_unavailable" =>
        {
            RunOutcome::Interrupted
        }
        Err(_) if resuming_question_run => RunOutcome::Interrupted,
        Err(_) => RunOutcome::Failed,
    }
}

/// Creates the stable error used when saved chat routing cannot be read.
fn storage_unavailable() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "Chat history could not be read.",
        false,
    )
}

fn log_chat_storage_failure(
    storage: &AppStorage,
    operation: &'static str,
    error: &rusqlite::Error,
) {
    if storage
        .log_rpc_failure(operation, sqlite_error_code(error))
        .is_err()
    {
        eprintln!("chat_storage_diagnostic_write_failed");
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::{
        protocol::ServiceError, storage::user_questions::AgentRunStatus,
        user_question_broker::RunOutcome,
    };

    use super::classify_run_outcome;

    fn service_error(code: &'static str, retryable: bool) -> ServiceError {
        ServiceError::new(code, "test failure", retryable)
    }

    #[test]
    fn successful_responses_follow_provider_and_persisted_run_state() {
        assert_eq!(
            classify_run_outcome(&Ok(json!({})), AgentRunStatus::Running, false, false, false,),
            RunOutcome::Completed,
        );
        assert_eq!(
            classify_run_outcome(
                &Ok(json!({"status": "paused"})),
                AgentRunStatus::Running,
                false,
                false,
                false,
            ),
            RunOutcome::Paused,
        );
        assert_eq!(
            classify_run_outcome(
                &Ok(json!({"status": "stopped"})),
                AgentRunStatus::Running,
                false,
                false,
                false,
            ),
            RunOutcome::Cancelled,
        );
        assert_eq!(
            classify_run_outcome(&Ok(Value::Null), AgentRunStatus::Paused, true, false, false,),
            RunOutcome::Paused,
        );
        assert_eq!(
            classify_run_outcome(
                &Ok(Value::Null),
                AgentRunStatus::Cancelled,
                true,
                false,
                false,
            ),
            RunOutcome::Cancelled,
        );
    }

    #[test]
    fn failures_preserve_pause_cancel_and_interruption_semantics() {
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("quota_exceeded", false)),
                AgentRunStatus::Running,
                true,
                false,
                false,
            ),
            RunOutcome::Paused,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("provider_unavailable", true)),
                AgentRunStatus::Running,
                true,
                false,
                false,
            ),
            RunOutcome::Paused,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("operation_cancelled", false)),
                AgentRunStatus::Running,
                false,
                false,
                true,
            ),
            RunOutcome::Interrupted,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("operation_cancelled", false)),
                AgentRunStatus::Running,
                false,
                false,
                false,
            ),
            RunOutcome::Cancelled,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("question_delivery_failed", false)),
                AgentRunStatus::Running,
                false,
                false,
                false,
            ),
            RunOutcome::Interrupted,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("provider_unavailable", false)),
                AgentRunStatus::Running,
                false,
                true,
                false,
            ),
            RunOutcome::Interrupted,
        );
        assert_eq!(
            classify_run_outcome(
                &Err(service_error("provider_unavailable", false)),
                AgentRunStatus::Running,
                false,
                false,
                false,
            ),
            RunOutcome::Failed,
        );
    }
}
