use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch as tokio_watch;
use zeroize::Zeroizing;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt::ChatGptService,
    chatgpt_store, conversation_archive, goals, hugging_face, instructions, local_engines,
    openai_api, openai_compatible,
    permissions::ToolPermissionBroker,
    profile_archive,
    protocol::{EventSink, Request, Response, ServiceError},
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
    usage_statistics,
    user_question_broker::{self, RunOutcome, UserQuestionBroker},
};
pub(crate) async fn dispatch(
    storage: &AppStorage,
    service: &Arc<ChatGptService>,
    mut request: Request,
    mut cancellation: tokio_watch::Receiver<bool>,
    events: EventSink,
    permission_broker: ToolPermissionBroker,
    user_question_broker: UserQuestionBroker,
    shutdown_requested: Arc<AtomicBool>,
    startup_recovered: Arc<AtomicBool>,
) -> Result<Value, ServiceError> {
    let method = request.method.clone();
    match method.as_str() {
        "system.health" => Ok(json!({
            "status": "ready",
            "database_path": storage.database_path().to_string_lossy(),
            "storage_root": storage.root().to_string_lossy(),
            "schema_version": storage.schema_version(),
        })),
        "system.database.prepare" => {
            let chat_schema_version = request
                .params
                .get("chatSchemaVersion")
                .and_then(Value::as_i64)
                .filter(|version| (1..=100).contains(version))
                .ok_or_else(|| {
                    ServiceError::new(
                        "invalid_request_params",
                        "The local database schema version is invalid.",
                        false,
                    )
                })?;
            let backup_created = storage
                .prepare_chat_schema_migration(chat_schema_version)
                .map_err(|_| {
                    ServiceError::new(
                        "database_prepare_failed",
                        "The local database could not be safely prepared for an update.",
                        false,
                    )
                })?;
            Ok(json!({"status": "ready", "backupCreated": backup_created}))
        }
        "system.initialize" => {
            let schema_version = storage.initialize_backend_schema().map_err(|_| {
                ServiceError::new(
                    "storage_initialization_failed",
                    "Local conversation storage could not be initialized.",
                    false,
                )
            })?;
            usage_statistics::mark_interrupted_requests(storage).map_err(|_| {
                ServiceError::new(
                    "usage_statistics_unavailable",
                    "Local usage statistics could not be recovered.",
                    true,
                )
            })?;
            if startup_recovered
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let recovery = (|| {
                    let connection = storage.connect().map_err(|_| ())?;
                    crate::storage::user_questions::interrupt_running_runs_after_restart(
                        &connection,
                        None,
                    )
                    .map(|_| ())
                    .map_err(|_| ())
                })();
                if recovery.is_err() {
                    startup_recovered.store(false, Ordering::SeqCst);
                    return Err(ServiceError::new(
                        "question_storage_unavailable",
                        "Pending AI questions could not be recovered.",
                        true,
                    ));
                }
            }
            storage.commit_pending_profile_restore().map_err(|_| {
                ServiceError::new(
                    "profile_archive_storage_failed",
                    "The restored profile could not be finalized safely.",
                    false,
                )
            })?;
            Ok(json!({"status": "ready", "schema_version": schema_version}))
        }
        "chat.file_changes.list" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let changes = crate::file_changes::list_changes(storage, conversation_id)
                .map_err(file_changes_unavailable)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.diff" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let change_id = required_string(&request.params, "changeId")?;
            crate::file_changes::read_diff(storage, conversation_id, change_id)
                .map_err(file_changes_unavailable)
        }
        "chat.file_changes.revert" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let change_id = required_string(&request.params, "changeId")?;
            let changes = crate::file_changes::revert_change(
                storage,
                conversation_id,
                change_id,
            )
            .map_err(|error| match error {
                crate::file_changes::RevertError::Conflict => ServiceError::new(
                    "file_change_conflict",
                    "This file changed after the assistant edited it. The current file was kept intact.",
                    false,
                ),
                crate::file_changes::RevertError::AlreadyReverted => ServiceError::new(
                    "file_change_already_reverted",
                    "This file change has already been reverted.",
                    false,
                ),
                crate::file_changes::RevertError::Unavailable => file_changes_unavailable(()),
            })?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.delete" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            crate::file_changes::delete_conversation_changes(storage, conversation_id)
                .map_err(file_changes_unavailable)?;
            Ok(json!({"deleted": true}))
        }
        "chat.file_changes.delete_all" => {
            crate::file_changes::delete_all_changes(storage).map_err(file_changes_unavailable)?;
            Ok(json!({"deleted": true}))
        }
        "project.worktrees.list" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            crate::git_worktrees::list(storage, project_id, project_root)
                .await
                .map_err(git_worktree_error)
        }
        "project.mcp.catalog.get" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            tools::mcp::server_catalog(&project_root)
        }
        "project.mcp.catalog.save" => {
            let project_id = required_string(&request.params, "projectId")?;
            let catalog = request
                .params
                .get("catalog")
                .ok_or_else(invalid_request_params)?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            tools::mcp::save_server_catalog(&project_root, catalog)
        }
        "project.mcp.server.check" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let server = request
                .params
                .get("server")
                .ok_or_else(invalid_request_params)?;
            let config = tools::mcp::parse_server_config(&project_root, project_id, server)?;
            if !config.enabled {
                return Ok(json!({"status": "disabled", "toolCount": 0, "toolNames": []}));
            }
            let server_id = config.id.clone();
            let registry = tools::mcp::McpRegistry::connect(vec![config])
                .await
                .map_err(|_| {
                    ServiceError::new(
                        "mcp_server_unavailable",
                        "The MCP server could not start or provide its tool list.",
                        true,
                    )
                })?;
            let tool_names = registry
                .as_ref()
                .map(|registry| registry.tool_names_for_server(&server_id))
                .unwrap_or_default();
            Ok(
                json!({"status": "connected", "toolCount": tool_names.len(), "toolNames": tool_names}),
            )
        }
        "project.mcp.credential.set" => {
            let project_id = required_string(&request.params, "projectId")?.to_owned();
            let server_id = required_string(&request.params, "serverId")?.to_owned();
            let environment_name = required_string(&request.params, "environmentName")?.to_owned();
            let secret = required_secret_string(&mut request.params, "secret")?;
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            crate::credentials::CredentialStore
                .store_mcp_secret(&reference, secret)
                .map_err(mcp_credential_error)?;
            Ok(json!({"stored": true}))
        }
        "project.mcp.credential.remove" => {
            let project_id = required_string(&request.params, "projectId")?.to_owned();
            let server_id = required_string(&request.params, "serverId")?.to_owned();
            let environment_name = required_string(&request.params, "environmentName")?.to_owned();
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            crate::credentials::CredentialStore
                .delete_mcp_secret(&reference)
                .map_err(mcp_credential_error)?;
            Ok(json!({"removed": true}))
        }
        "project.mcp.credential.status" => {
            let project_id = required_string(&request.params, "projectId")?.to_owned();
            let server_id = required_string(&request.params, "serverId")?.to_owned();
            let environment_name = required_string(&request.params, "environmentName")?.to_owned();
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            let stored = crate::credentials::CredentialStore
                .has_mcp_secret(&reference)
                .map_err(mcp_credential_error)?;
            Ok(json!({"stored": stored}))
        }
        "project.worktrees.create" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            crate::git_worktrees::create(storage, project_id, project_root)
                .await
                .map_err(git_worktree_error)
        }
        "project.worktrees.review" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            let worktree_id = required_string(&request.params, "worktreeId")?;
            crate::git_worktrees::review(storage, project_id, project_root, worktree_id)
                .await
                .map_err(git_worktree_error)
        }
        "project.worktrees.tasks" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            let worktree_id = required_string(&request.params, "worktreeId")?;
            let task_root = crate::git_worktrees::project_root_for_worktree(
                storage,
                project_id,
                project_root,
                worktree_id,
            )
            .await
            .map_err(git_worktree_error)?;
            let tasks = crate::tools::project_tasks::load_project_tasks(&task_root)?;
            Ok(json!({"tasks": tasks}))
        }
        "project.worktrees.run_task" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            let worktree_id = required_string(&request.params, "worktreeId")?;
            let task_id = required_string(&request.params, "taskId")?;
            let expected_command = required_string(&request.params, "expectedCommand")?;
            let permission_mode = crate::tools::ToolPermissionMode::from_rpc(optional_string(
                &request.params,
                "toolPermissionMode",
            )?)?;
            let permission_rules = crate::tools::parse_tool_permission_rules(
                request.params.get("toolPermissionRules"),
            )?;
            let confirmed = request
                .params
                .get("confirmed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            match permission_rules.get("run_project_task") {
                Some(crate::tools::ToolPermissionRule::Deny) => {
                    return Err(ServiceError::new(
                        "permission_denied",
                        "The project permission rules deny named task execution.",
                        false,
                    ));
                }
                Some(crate::tools::ToolPermissionRule::Ask) if !confirmed => {
                    return Err(tool_permission_confirmation_required());
                }
                Some(crate::tools::ToolPermissionRule::Ask)
                | Some(crate::tools::ToolPermissionRule::Allow) => {}
                None if permission_mode != crate::tools::ToolPermissionMode::FullAccess
                    && !confirmed =>
                {
                    return Err(tool_permission_confirmation_required());
                }
                None => {}
            }
            let expected_timeout = request
                .params
                .get("expectedTimeoutSeconds")
                .and_then(Value::as_u64)
                .filter(|seconds| (5..=600).contains(seconds))
                .ok_or_else(invalid_request_params)?;
            let task_root = crate::git_worktrees::project_root_for_worktree(
                storage,
                project_id,
                project_root,
                worktree_id,
            )
            .await
            .map_err(git_worktree_error)?;
            let task = crate::tools::project_tasks::load_project_task(&task_root, task_id)?;
            if task.command != expected_command || task.timeout_seconds != expected_timeout {
                return Err(ServiceError::new(
                    "project_task_changed",
                    "The named task changed after it was displayed. Reload the task list and review it again.",
                    false,
                ));
            }
            crate::tools::terminal::TerminalSessionManager::global()
                .execute_to_completion(
                    &task.command,
                    &task_root,
                    task.timeout_seconds,
                    &mut cancellation,
                )
                .await
                .map_err(|_| {
                    ServiceError::new(
                        "project_task_execution_failed",
                        "The named task could not run inside the process sandbox.",
                        true,
                    )
                })
        }
        "project.worktrees.remove" => {
            let project_id = required_string(&request.params, "projectId")?;
            let project_root = required_string(&request.params, "projectRoot")?;
            let worktree_id = required_string(&request.params, "worktreeId")?;
            crate::git_worktrees::remove(storage, project_id, project_root, worktree_id)
                .await
                .map_err(git_worktree_error)
        }
        "chatgpt.oauth.start" => service.oauth_sign_in(&mut cancellation).await,
        "chatgpt.connections.list" => service.list_connections(),
        "chatgpt.connections.delete" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            service.delete_connection(connection_id).await
        }
        "chatgpt.titles.get" => service.title_preference(),
        "chatgpt.titles.select" => {
            let connection_id = optional_string(&request.params, "connectionId")?;
            let workspace_id = optional_string(&request.params, "workspaceId")?;
            service.select_title_target(connection_id, workspace_id)
        }
        "chatgpt.connections.select" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            service.select_connection(connection_id)
        }
        "chatgpt.workspaces.select" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            let workspace_id = required_string(&request.params, "workspaceId")?;
            service.select_workspace(connection_id, workspace_id)
        }
        "chatgpt.models.list" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            let workspace_id = required_string(&request.params, "workspaceId")?;
            let force_refresh = request
                .params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            service
                .models(
                    connection_id,
                    workspace_id,
                    force_refresh,
                    &mut cancellation,
                )
                .await
        }
        "chatgpt.api.models.list" => {
            let api_key = required_string(&request.params, "apiKey")?;
            let force_refresh = request
                .params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            openai_api::models(storage, api_key, force_refresh, &mut cancellation).await
        }
        "chatgpt.usage.get" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            let workspace_id = required_string(&request.params, "workspaceId")?;
            service
                .usage(connection_id, workspace_id, &mut cancellation)
                .await
        }
        "statistics.get" => {
            let query = statistics_query(&request.params)?;
            usage_statistics::get_statistics(storage, &query, &mut cancellation).await
        }
        "chatgpt.reset_credits.consume" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            let workspace_id = required_string(&request.params, "workspaceId")?;
            let credit_id = required_string(&request.params, "creditId")?;
            service
                .consume_reset_credit(connection_id, workspace_id, credit_id, &mut cancellation)
                .await
        }
        "chat.memory.inspect" => {
            let conversation_id = required_memory_conversation_id(&request.params)?;
            let state = chatgpt_store::load_conversation_context_state(storage, conversation_id)
                .map_err(|_| {
                    ServiceError::new(
                        "conversation_memory_unavailable",
                        "Conversation memory could not be loaded.",
                        true,
                    )
                })?;
            let archive_index_settings =
                chatgpt_store::archive_index_settings(storage, conversation_id).map_err(|_| {
                    ServiceError::new(
                        "conversation_memory_unavailable",
                        "Conversation archive settings could not be loaded.",
                        true,
                    )
                })?;
            let compaction_kind = state
                .as_ref()
                .and_then(|context| context.compaction_kind.as_deref());
            let compaction_provider_id = state
                .as_ref()
                .and_then(|context| context.compaction_provider_id.as_deref());
            let compaction_model_id = state
                .as_ref()
                .and_then(|context| context.compaction_model_id.as_deref());
            let compaction_connection_id = state
                .as_ref()
                .and_then(|context| context.compaction_connection_id.as_deref());
            let compaction_workspace_id = state
                .as_ref()
                .and_then(|context| context.compaction_workspace_id.as_deref());
            let compacted_through_message_id = state
                .as_ref()
                .and_then(|context| context.compacted_through_message_id.as_deref());
            let summary = state.as_ref().and_then(|context| {
                if context.compaction_kind.as_deref() == Some("summary") {
                    context.compaction_payload.as_deref()
                } else {
                    None
                }
            });
            let last_prompt = state.as_ref().and_then(|context| {
                Some(json!({
                    "inputTokens": context.last_prompt_tokens?,
                    "messageId": context.last_prompt_message_id.as_deref()?,
                    "providerId": context.last_prompt_provider_id.as_deref()?,
                    "modelId": context.last_prompt_model_id.as_deref()?,
                    "connectionId": context.last_prompt_connection_id.as_deref(),
                    "workspaceId": context.last_prompt_workspace_id.as_deref(),
                }))
            });
            Ok(json!({
                "compactionKind": compaction_kind,
                "compactionProviderId": compaction_provider_id,
                "compactionModelId": compaction_model_id,
                "compactionConnectionId": compaction_connection_id,
                "compactionWorkspaceId": compaction_workspace_id,
                "compactedThroughMessageId": compacted_through_message_id,
                "summary": summary,
                "lastPrompt": last_prompt,
                "archiveIndexSettings": archive_index_settings_json(&archive_index_settings),
            }))
        }
        "chat.memory.archive.set_conversation" => {
            let conversation_id = required_memory_conversation_id(&request.params)?;
            let included = request
                .params
                .get("included")
                .and_then(Value::as_bool)
                .ok_or_else(invalid_memory_archive_settings_params)?;
            let settings = chatgpt_store::set_conversation_archive_included(
                storage,
                conversation_id,
                included,
            )
            .await
            .map_err(|_| {
                ServiceError::new(
                    "conversation_memory_unavailable",
                    "Conversation archive settings could not be saved.",
                    true,
                )
            })?;
            Ok(json!({"archiveIndexSettings": archive_index_settings_json(&settings)}))
        }
        "chat.memory.archive.set_tool" => {
            let conversation_id = required_memory_conversation_id(&request.params)?;
            let tool_name = required_string(&request.params, "toolName")?;
            let included = request
                .params
                .get("included")
                .and_then(Value::as_bool)
                .ok_or_else(invalid_memory_archive_settings_params)?;
            let settings = chatgpt_store::set_archive_tool_included(
                storage,
                conversation_id,
                tool_name,
                included,
            )
            .await
            .map_err(|_| {
                ServiceError::new(
                    "conversation_memory_unavailable",
                    "Conversation archive settings could not be saved.",
                    true,
                )
            })?;
            Ok(json!({"archiveIndexSettings": archive_index_settings_json(&settings)}))
        }
        "chat.context.usage.estimate" => {
            let provider_id = required_string(&request.params, "providerId")?;
            if !matches!(
                provider_id,
                "chatgpt"
                    | "chatgpt_api"
                    | "opencode"
                    | "gemini"
                    | "groq"
                    | "cerebras"
                    | "openrouter"
                    | "mistral"
                    | "llama_cpp"
                    | "vllm"
                    | "exllama"
            ) {
                return Err(invalid_context_usage_params());
            }
            let model_id = optional_string(&request.params, "modelId")?;
            let reported_supports_tool_calls = match request.params.get("supportsTools") {
                None | Some(Value::Null) => None,
                Some(Value::Bool(supported)) => Some(*supported),
                _ => return Err(invalid_context_usage_params()),
            };
            let permission_mode = ToolPermissionMode::from_rpc(Some(required_string(
                &request.params,
                "toolPermissionMode",
            )?))?;
            let custom_instructions = optional_string(&request.params, "customInstructions")?;
            instructions::validate_custom_instructions(custom_instructions)
                .map_err(|_| invalid_context_usage_params())?;
            let conversation_id = optional_string(&request.params, "conversationId")?;
            let has_project = match conversation_id {
                Some(conversation_id) => storage
                    .connect()
                    .and_then(|connection| {
                        connection.query_row(
                            "SELECT EXISTS(
                                SELECT 1 FROM conversations
                                INNER JOIN projects ON projects.id = conversations.project_id
                                WHERE conversations.id = ?1
                                  AND projects.folder_path IS NOT NULL
                                  AND projects.folder_path <> ''
                            )",
                            [conversation_id],
                            |row| row.get::<_, bool>(0),
                        )
                    })
                    .map_err(|_| {
                        ServiceError::new(
                            "storage_unavailable",
                            "Chat context usage could not be measured.",
                            true,
                        )
                    })?,
                None => false,
            };
            let available_tools = openai_compatible::context_usage_tool_definitions(
                storage,
                provider_id,
                model_id,
                reported_supports_tool_calls,
            )?;
            let instructions = instructions::shared_instructions(
                custom_instructions,
                permission_mode,
                has_project,
                !available_tools.is_empty(),
                provider_id,
            );
            let tool_definitions = available_tools
                .into_iter()
                .map(|(name, definition)| {
                    json!({
                        "name": name,
                        "tokens": crate::context_compaction::request_context_token_estimate(&definition),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "instructionsTokens": crate::context_compaction::text_token_estimate(&instructions),
                "toolDefinitions": tool_definitions,
            }))
        }
        "chat.memory.search" => {
            let conversation_id = required_memory_conversation_id(&request.params)?;
            let query = required_memory_search_query(&request.params)?;
            let results =
                chatgpt_store::search_conversation_archive(storage, conversation_id, query)
                    .await
                    .map_err(|_| {
                        ServiceError::new(
                            "conversation_memory_unavailable",
                            "Conversation memory could not be searched.",
                            true,
                        )
                    })?;
            Ok(json!({
                "results": results.into_iter().map(|excerpt| json!({
                    "messageId": excerpt.message_id,
                    "role": excerpt.role,
                    "content": excerpt.content,
                    "createdAtUnixMs": excerpt.created_at_unix_ms,
                })).collect::<Vec<_>>(),
            }))
        }
        "chat.history.search" => {
            let query = required_memory_search_query(&request.params)?;
            let from_unix_ms = optional_history_timestamp(&request.params, "fromUnixMs")?;
            let through_unix_ms = optional_history_timestamp(&request.params, "throughUnixMs")?;
            if matches!((from_unix_ms, through_unix_ms), (Some(from), Some(through)) if from >= through)
            {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The conversation history date range is invalid.",
                    false,
                ));
            }
            let filters = chatgpt_store::HistorySearchFilters {
                from_unix_ms,
                through_unix_ms,
                provider_id: optional_history_filter_string(&request.params, "providerId")?,
                model_id: optional_history_filter_string(&request.params, "modelId")?,
                project_id: optional_history_filter_string(&request.params, "projectId")?,
                is_archived: optional_history_filter_bool(&request.params, "isArchived")?,
                tag: optional_history_tag_filter(&request.params)?,
            };
            let results = chatgpt_store::search_chat_history(storage, query, filters)
                .await
                .map_err(|_| {
                    ServiceError::new(
                        "history_search_unavailable",
                        "Conversation history could not be searched.",
                        true,
                    )
                })?;
            Ok(json!({
                "results": results.into_iter().map(|result| json!({
                    "conversationId": result.conversation_id,
                    "conversationTitle": result.conversation_title,
                    "messageId": result.message_id,
                    "role": result.role,
                    "excerpt": result.excerpt,
                    "createdAtUnixMs": result.created_at_unix_ms,
                })).collect::<Vec<_>>(),
            }))
        }
        "chat.history.search_filters" => {
            let options: chatgpt_store::HistorySearchFilterOptions =
                chatgpt_store::history_search_filter_options(storage).map_err(|_| {
                    ServiceError::new(
                        "history_search_unavailable",
                        "Conversation history filters could not be loaded.",
                        true,
                    )
                })?;
            Ok(json!({
                "providerIds": options.provider_ids,
                "modelIds": options.model_ids,
                "tags": options.tags,
            }))
        }
        "conversation.archive.export" => {
            let passphrase = required_secret_string(&mut request.params, "passphrase")?;
            let conversation_ids = required_string_array(&request.params, "conversationIds")?;
            let path = required_string(&request.params, "path")?.to_owned();
            conversation_archive::export(storage, &conversation_ids, &path, passphrase)
        }
        "conversation.archive.inspect" => {
            let passphrase = required_secret_string(&mut request.params, "passphrase")?;
            let path = required_string(&request.params, "path")?.to_owned();
            conversation_archive::inspect(storage, &path, passphrase)
        }
        "conversation.archive.restore" => {
            let passphrase = required_secret_string(&mut request.params, "passphrase")?;
            let path = required_string(&request.params, "path")?.to_owned();
            let conflict_policy = required_string(&request.params, "conflictPolicy")?.to_owned();
            conversation_archive::restore(storage, &path, passphrase, &conflict_policy)
        }
        "profile.archive.export" => {
            let passphrase = required_secret_string(&mut request.params, "passphrase")?;
            let path = required_string(&request.params, "path")?.to_owned();
            profile_archive::export(storage, &path, passphrase)
        }
        "profile.archive.prepare_restore" => {
            let passphrase = required_secret_string(&mut request.params, "passphrase")?;
            let path = required_string(&request.params, "path")?.to_owned();
            let chat_schema_version = request
                .params
                .get("chatSchemaVersion")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    ServiceError::new(
                        "invalid_request_params",
                        "The current chat database schema version is invalid.",
                        false,
                    )
                })?;
            profile_archive::prepare_restore(storage, &path, passphrase, chat_schema_version)
        }
        "chat.memory.semantic.status" => Ok(json!({
            "ready": chatgpt_store::semantic_search_is_ready(storage).await,
        })),
        "chat.memory.semantic.prepare" => {
            let (progress_sender, mut progress_receiver) =
                tokio::sync::mpsc::unbounded_channel::<Value>();
            let progress_events = events.clone();
            let request_id = request.id.clone();
            let progress_forwarder = tokio::spawn(async move {
                while let Some(data) = progress_receiver.recv().await {
                    progress_events
                        .send(&Response::event(
                            request_id.clone(),
                            "chat.memory.semantic.progress",
                            data,
                        ))
                        .await
                        .map_err(|_| ())?;
                }
                Ok::<(), ()>(())
            });
            let preparation =
                chatgpt_store::prepare_semantic_search(storage, &mut cancellation, progress_sender)
                    .await;
            let progress_result = progress_forwarder.await;
            if !matches!(progress_result, Ok(Ok(()))) {
                return Err(ServiceError::new(
                    "semantic_memory_progress_failed",
                    "Semantic search progress could not be delivered.",
                    true,
                ));
            }
            preparation.map_err(|_| {
                if *cancellation.borrow() {
                    ServiceError::new(
                        "operation_cancelled",
                        "Semantic search preparation was cancelled.",
                        false,
                    )
                } else {
                    ServiceError::new(
                        "semantic_memory_prepare_failed",
                        "Local semantic search could not be prepared. Try again.",
                        true,
                    )
                }
            })?;
            Ok(json!({"ready": true}))
        }
        "local.engines.list" => local_engines::list(storage).await,
        "local.engines.llama_server_path.set" => local_engines::set_llama_server_executable_path(
            storage,
            optional_string(&request.params, "path")?,
        ),
        "local.engines.external_llama_server.detect" => {
            local_engines::detect_external_llama_servers().await
        }
        "local.engines.external_llama_server.connect" => {
            local_engines::connect_external_llama_server(
                required_non_zero_u32(&request.params, "processId")?,
                required_port(&request.params, "port")?,
            )
            .await
        }
        "local.engines.external_llama_server.disconnect" => {
            Ok(local_engines::disconnect_external_llama_server(
                required_non_zero_u32(&request.params, "processId")?,
                required_port(&request.params, "port")?,
            )
            .await)
        }
        "local.engines.chat_models.list" => {
            local_engines::chat_model_catalog(
                storage,
                required_string(&request.params, "engineId")?,
            )
            .await
        }
        "local.models.list" => local_engines::model_catalog(storage),
        "local.models.discover" => {
            local_engines::discover_models(
                storage,
                required_string(&request.params, "engineId")?,
                required_string(&request.params, "modelDirectory")?,
            )
            .await
        }
        "models.hub.search" => {
            let query = match request.params.get("query") {
                None | Some(Value::Null) => "",
                Some(Value::String(query)) => query,
                _ => {
                    return Err(ServiceError::new(
                        "invalid_request_params",
                        "The model search query must be text.",
                        false,
                    ));
                }
            };
            let format = required_string(&request.params, "format")?;
            let sort = required_string(&request.params, "sort")?;
            let cursor = match request.params.get("cursor") {
                None | Some(Value::Null) => None,
                Some(Value::String(cursor)) => Some(cursor.as_str()),
                _ => {
                    return Err(ServiceError::new(
                        "invalid_request_params",
                        "The model search cursor must be text.",
                        false,
                    ));
                }
            };
            hugging_face::search(query, format, sort, cursor).await
        }
        "models.hub.files" => {
            let repo_id = required_string(&request.params, "repoId")?;
            let format = required_string(&request.params, "format")?;
            hugging_face::files(repo_id, format).await
        }
        "models.hub.download" => {
            let repo_id = required_string(&request.params, "repoId")?;
            let revision = required_string(&request.params, "revision")?;
            let format = required_string(&request.params, "format")?;
            let group_id = required_string(&request.params, "groupId")?;
            let component_path = optional_string(&request.params, "componentPath")?;
            let model_directory = optional_string(&request.params, "modelDirectory")?;
            hugging_face::download(
                storage,
                repo_id,
                revision,
                format,
                group_id,
                component_path,
                model_directory,
                &request.id,
                &events,
                &mut cancellation,
            )
            .await
        }
        "local.models.register" => {
            local_engines::register_model(
                storage,
                required_string(&request.params, "engineId")?,
                required_string(&request.params, "modelPath")?,
                optional_string(&request.params, "modelDirectory")?,
                required_string(&request.params, "storageAction")?,
            )
            .await
        }
        "local.models.remove" => {
            let model_id = required_string(&request.params, "modelId")?;
            local_engines::remove_model(storage, model_id).await
        }
        "local.engines.start" => {
            let model_id = required_string(&request.params, "modelId")?;
            local_engines::start_model(storage, model_id, &mut cancellation).await
        }
        "local.engines.stop" => local_engines::stop_runtime(storage).await,
        "local.engines.stop_model" => {
            local_engines::stop_model(storage, required_string(&request.params, "modelId")?).await
        }
        "local.engines.install" => {
            let engine_id = required_string(&request.params, "engineId")?;
            let variant_id = required_string(&request.params, "variantId")?;
            let (manifest, variant) = local_engines::find_variant(engine_id, variant_id)?;
            if !local_engines::can_install(&manifest, &variant).await {
                return Err(ServiceError::new(
                    "local_engine_install_unavailable",
                    "The selected local engine package is not available for this device.",
                    false,
                ));
            }
            let installed = local_engines::installer::install_engine_variant(
                storage,
                &manifest,
                &variant,
                &request.id,
                &events,
                &mut cancellation,
            )
            .await?;
            Ok(json!({
                "installed": true,
                "engineId": installed.engine_id,
                "variantId": installed.variant_id,
                "releaseTag": installed.release_tag,
            }))
        }
        "chat.memory.reset_compaction" => {
            let conversation_id = required_memory_conversation_id(&request.params)?;
            let reset = chatgpt_store::reset_conversation_context(storage, conversation_id)
                .map_err(|_| {
                    ServiceError::new(
                        "conversation_memory_unavailable",
                        "The compacted conversation context could not be reset.",
                        true,
                    )
                })?;
            Ok(json!({"reset": reset}))
        }
        "chat.send" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let resume_run_id = optional_string(&request.params, "resumeRunId")?;
            let api_key = optional_api_key(&request.params, "apiKey")?;
            let api_key_connection_id = optional_string(&request.params, "apiKeyConnectionId")?;
            let requested_reasoning_effort = optional_string(&request.params, "reasoningEffort")?;
            let requested_fast_mode = optional_bool(&request.params, "fastMode")?.unwrap_or(false);
            let goal_requested = optional_bool(&request.params, "goal")?.unwrap_or(false);
            let requested_goal_objective = if goal_requested {
                Some(goals::validate_objective(
                    optional_string(&request.params, "goalObjective")?
                        .ok_or_else(|| invalid_chat_goal_request("goal objective"))?,
                )?)
            } else {
                if optional_string(&request.params, "goalObjective")?.is_some() {
                    return Err(invalid_chat_goal_request("goal objective"));
                }
                None
            };
            let requested_custom_instructions =
                optional_string(&request.params, "customInstructions")?;
            let permission_mode = ToolPermissionMode::from_rpc(optional_string(
                &request.params,
                "toolPermissionMode",
            )?)?;
            let tool_permission_rules =
                tools::parse_tool_permission_rules(request.params.get("toolPermissionRules"))?;
            let excluded_assistant_message_id =
                optional_string(&request.params, "excludedAssistantMessageId")?;
            if (resume_run_id.is_some() && goal_requested)
                || (goal_requested && excluded_assistant_message_id.is_some())
            {
                return Err(invalid_chat_goal_request("goal start parameters"));
            }
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "storage_unavailable",
                    "Chat history could not be read.",
                    false,
                )
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
                .map_err(|_| {
                    ServiceError::new(
                        "storage_unavailable",
                        "Chat history could not be read.",
                        false,
                    )
                })?;
            let (
                provider_id,
                project_root,
                project_id,
                stored_api_key_connection_id,
                model_id,
                connection_id,
                workspace_id,
            ) = route.unwrap_or((None, None, None, None, None, None, None));
            if let Some(objective) = requested_goal_objective {
                let messages = chatgpt_store::conversation_messages(storage, conversation_id)
                    .map_err(|_| {
                        ServiceError::new(
                            "storage_unavailable",
                            "Chat history could not be read.",
                            false,
                        )
                    })?;
                if messages.last().is_none_or(|message| {
                    message.role != "user" || message.content.trim() != objective
                }) {
                    return Err(invalid_chat_goal_request("goal objective"));
                }
                if crate::storage::user_questions::latest_active_goal_run(
                    &connection,
                    conversation_id,
                )
                .map_err(map_question_storage_error)?
                .is_some()
                {
                    return Err(ServiceError::new(
                        "goal_already_active",
                        "Resume or stop the active goal before starting another goal in this chat.",
                        false,
                    ));
                }
            }
            if requested_fast_mode
                && !matches!(provider_id.as_deref(), Some("chatgpt" | "chatgpt_api"))
            {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "Fast mode is only available for supported ChatGPT models.",
                    false,
                ));
            }
            let resuming_question_run = resume_run_id.is_some();
            let (run_id, custom_instructions, reasoning_effort, fast_mode, goal_execution) =
                if let Some(run_id) = resume_run_id {
                    let run = crate::storage::user_questions::load_run_for_conversation(
                        &connection,
                        conversation_id,
                        run_id,
                    )
                    .map_err(map_question_storage_error)?;
                    let checkpoint = run.checkpoint.as_ref().ok_or_else(|| {
                        ServiceError::new(
                            "question_run_checkpoint_missing",
                            "The pending AI response has no saved continuation data.",
                            false,
                        )
                    })?;
                    if checkpoint.get("providerId").and_then(Value::as_str)
                        != provider_id.as_deref()
                        || checkpoint.get("modelId").and_then(Value::as_str) != model_id.as_deref()
                        || checkpoint.get("connectionId").and_then(Value::as_str)
                            != connection_id.as_deref()
                        || checkpoint.get("workspaceId").and_then(Value::as_str)
                            != workspace_id.as_deref()
                        || checkpoint
                            .get("providerConnectionId")
                            .or_else(|| checkpoint.get("apiKeyConnectionId"))
                            .and_then(Value::as_str)
                            != stored_api_key_connection_id.as_deref()
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
                    let reasoning_effort = checkpoint
                        .get("reasoningEffort")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let fast_mode = match checkpoint.get("fastMode") {
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
                    if fast_mode
                        && !matches!(provider_id.as_deref(), Some("chatgpt" | "chatgpt_api"))
                    {
                        return Err(ServiceError::new(
                            "question_run_checkpoint_invalid",
                            "The saved Fast mode setting does not match the selected provider.",
                            false,
                        ));
                    }
                    instructions::validate_custom_instructions(custom_instructions.as_deref())
                        .map_err(|message| {
                            ServiceError::new("question_run_checkpoint_invalid", message, false)
                        })?;
                    let mut goal_execution = if checkpoint.get("goal").is_some() {
                        Some(goals::execution_from_run(run.clone())?)
                    } else {
                        None
                    };
                    crate::storage::user_questions::claim_run_resume(
                        &connection,
                        conversation_id,
                        run_id,
                    )
                    .map_err(map_question_storage_error)?;
                    if let Some(goal) = goal_execution.as_mut() {
                        goals::set_running(storage, goal)?;
                    }
                    (
                        run_id.to_owned(),
                        custom_instructions,
                        reasoning_effort,
                        fast_mode,
                        goal_execution,
                    )
                } else {
                    let run_id = uuid::Uuid::new_v4().simple().to_string();
                    crate::storage::user_questions::create_run(
                        &connection,
                        &crate::storage::user_questions::NewAgentRun {
                            run_id: run_id.clone(),
                            conversation_id: conversation_id.to_owned(),
                        },
                    )
                    .map_err(map_question_storage_error)?;
                    instructions::validate_custom_instructions(requested_custom_instructions)
                        .map_err(|message| {
                            ServiceError::new("invalid_request_params", message, false)
                        })?;
                    let mut checkpoint = json!({
                        "version": 1,
                        "providerId": provider_id,
                        "modelId": model_id,
                        "connectionId": connection_id,
                        "workspaceId": workspace_id,
                        "providerConnectionId": stored_api_key_connection_id,
                        "customInstructions": requested_custom_instructions,
                        "reasoningEffort": requested_reasoning_effort,
                        "fastMode": requested_fast_mode,
                    });
                    if let Some(objective) = requested_goal_objective {
                        goals::initial_checkpoint(
                            &mut checkpoint,
                            objective,
                            current_time_unix_ms()?,
                        );
                    }
                    crate::storage::user_questions::save_checkpoint(
                        &connection,
                        conversation_id,
                        &run_id,
                        0,
                        &checkpoint,
                    )
                    .map_err(map_question_storage_error)?;
                    let goal_execution = if requested_goal_objective.is_some() {
                        let run = crate::storage::user_questions::load_run_for_conversation(
                            &connection,
                            conversation_id,
                            &run_id,
                        )
                        .map_err(map_question_storage_error)?;
                        Some(goals::execution_from_run(run)?)
                    } else {
                        None
                    };
                    (
                        run_id,
                        requested_custom_instructions.map(str::to_owned),
                        requested_reasoning_effort.map(str::to_owned),
                        requested_fast_mode,
                        goal_execution,
                    )
                };
            if let Some(goal) = goal_execution.as_ref() {
                events
                    .send(&Response::event(
                        request.id.clone(),
                        "chat.goal.updated",
                        goals::goal_value(goal, "running", None),
                    ))
                    .await
                    .map_err(|_| {
                        ServiceError::new(
                            "protocol_unavailable",
                            "The active goal could not be shown in the application.",
                            true,
                        )
                    })?;
            }
            #[cfg(windows)]
            let mcp_configs = if matches!(
                provider_id.as_deref(),
                Some(
                    "chatgpt"
                        | "chatgpt_api"
                        | "opencode"
                        | "gemini"
                        | "groq"
                        | "cerebras"
                        | "openrouter"
                        | "mistral"
                        | "llama_cpp"
                        | "vllm"
                        | "exllama"
                )
            ) {
                match project_root.as_deref() {
                    Some(project_root) => {
                        match tools::mcp::load_server_configs(
                            Path::new(project_root),
                            project_id.as_deref().unwrap_or_default(),
                        ) {
                            Ok(configs) => configs,
                            Err(error) => {
                                crate::storage::user_questions::mark_run_failed(
                                    &connection,
                                    conversation_id,
                                    &run_id,
                                )
                                .map_err(map_question_storage_error)?;
                                return Err(error);
                            }
                        }
                    }
                    None => Vec::new(),
                }
            } else {
                Vec::new()
            };
            let context = ChatSendContext {
                request_id: request.id,
                run_id: run_id.clone(),
                goal: goal_execution,
                conversation_id,
                excluded_assistant_message_id,
                custom_instructions: custom_instructions.as_deref(),
                project_root: project_root.as_deref().map(Path::new),
                data_root: storage.root(),
                storage,
                permission_mode,
                tool_permission_rules,
                #[cfg(windows)]
                mcp_configs,
                #[cfg(windows)]
                mcp_registry: None,
                permission_broker: &permission_broker,
                user_question_broker: &user_question_broker,
                cancellation: &mut cancellation,
                events,
            };
            let result = match provider_id.as_deref() {
                Some(
                    "opencode" | "chatgpt_api" | "gemini" | "groq" | "cerebras" | "openrouter"
                    | "mistral" | "llama_cpp" | "vllm" | "exllama",
                ) => {
                    openai_compatible::send_message(
                        storage,
                        service,
                        context,
                        api_key,
                        api_key_connection_id,
                        stored_api_key_connection_id,
                        reasoning_effort.as_deref(),
                        fast_mode,
                    )
                    .await
                }
                _ => {
                    service
                        .send_message(context, reasoning_effort.as_deref(), fast_mode)
                        .await
                }
            };
            let current_run = crate::storage::user_questions::load_run_for_conversation(
                &connection,
                conversation_id,
                &run_id,
            )
            .map_err(map_question_storage_error)?;
            let is_goal_run = current_run
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.get("goal"))
                .is_some();
            let outcome = match &result {
                Ok(_)
                    if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Paused =>
                {
                    RunOutcome::Paused
                }
                Ok(_)
                    if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Cancelled =>
                {
                    RunOutcome::Cancelled
                }
                Err(_)
                    if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Paused =>
                {
                    RunOutcome::Paused
                }
                Err(_)
                    if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Cancelled =>
                {
                    RunOutcome::Cancelled
                }
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
                    if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Paused
                    {
                        RunOutcome::Paused
                    } else {
                        RunOutcome::Cancelled
                    }
                }
                Ok(_) => RunOutcome::Completed,
                Err(error)
                    if is_goal_run && (is_quota_or_rate_limit_error(error) || error.retryable) =>
                {
                    RunOutcome::Paused
                }
                Err(error) if error.code == "operation_cancelled" => {
                    if shutdown_requested.load(Ordering::SeqCst) {
                        RunOutcome::Interrupted
                    } else if is_goal_run
                        && current_run.status
                            == crate::storage::user_questions::AgentRunStatus::Paused
                    {
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
            };
            if is_goal_run
                && current_run.status == crate::storage::user_questions::AgentRunStatus::Running
            {
                let mut goal = goals::execution_from_run(current_run.clone())?;
                let (state, reason) = match outcome {
                    RunOutcome::Completed => ("completed", None),
                    RunOutcome::Paused => {
                        let reason = result
                            .as_ref()
                            .err()
                            .map(|error| {
                                if is_quota_or_rate_limit_error(error) {
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
                .finish_run(storage, conversation_id, &run_id, outcome)
                .await?;
            result
        }
        "chat.goal.active" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let goal = goals::latest_active_goal(storage, conversation_id)?;
            Ok(json!({"goal": goal}))
        }
        "chat.runs.list" => {
            let requested_limit = match request.params.get("limit") {
                Some(value) => value.as_u64().ok_or_else(invalid_run_list_request)?,
                None => 50,
            };
            let limit = usize::try_from(requested_limit)
                .ok()
                .filter(|limit| (1..=50).contains(limit))
                .ok_or_else(invalid_run_list_request)?;
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "agent_runs_unavailable",
                    "Run status could not be loaded.",
                    true,
                )
            })?;
            let runs = crate::storage::user_questions::list_run_summaries(&connection, limit)
                .map_err(|_| {
                    ServiceError::new(
                        "agent_runs_unavailable",
                        "Run status could not be loaded.",
                        true,
                    )
                })?;
            Ok(json!({"runs": runs}))
        }
        "chat.goal.pause" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let run_id = required_string(&request.params, "runId")?;
            goals::pause_run(storage, conversation_id, run_id, "user_paused")
        }
        "chat.goal.stop" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let run_id = required_string(&request.params, "runId")?;
            goals::stop_run(storage, conversation_id, run_id)
        }
        "chat.questions.list" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let groups = user_question_broker
                .list_pending(storage, conversation_id)
                .await?;
            Ok(json!({"questions": groups}))
        }
        "chat.questions.respond" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let group_id = required_string(&request.params, "groupId")?;
            let expected_revision = request
                .params
                .get("revision")
                .and_then(Value::as_i64)
                .filter(|revision| *revision >= 0)
                .ok_or_else(|| invalid_question_request("question revision"))?;
            let answers = user_question_broker::parse_answers(
                request
                    .params
                    .get("answers")
                    .ok_or_else(|| invalid_question_request("question answers"))?,
            )?;
            user_question_broker
                .submit(
                    storage,
                    conversation_id,
                    group_id,
                    expected_revision,
                    &answers,
                )
                .await
        }
        "chat.tool.permission.respond" => {
            let approval_request_id = required_string(&request.params, "approvalRequestId")?;
            let approved = request
                .params
                .get("approved")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    ServiceError::new(
                        "invalid_request_params",
                        "The tool permission decision is invalid.",
                        false,
                    )
                })?;
            permission_broker
                .respond(approval_request_id, approved)
                .await
        }
        "opencode.models.list" => {
            let api_key = optional_api_key(&request.params, "apiKey")?;
            let force_refresh = request
                .params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            openai_compatible::models(storage, api_key, force_refresh, &mut cancellation).await
        }
        "compatible.models.list" => {
            let provider_id = required_string(&request.params, "providerId")?;
            let api_key = required_string(&request.params, "apiKey")?;
            let force_refresh = request
                .params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            openai_compatible::models_for_provider(
                storage,
                provider_id,
                api_key,
                force_refresh,
                &mut cancellation,
            )
            .await
        }
        "list_files" => tools::list_files(
            required_string(&request.params, "root")?,
            optional_string(&request.params, "path")?.unwrap_or(""),
            request
                .params
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
            request
                .params
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100) as usize,
        ),
        "search_files" => tools::search_files(
            required_string(&request.params, "root")?,
            optional_string(&request.params, "path")?.unwrap_or(""),
            required_string(&request.params, "query")?,
            request
                .params
                .get("includeHidden")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            request
                .params
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
            request
                .params
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(50) as usize,
        ),
        "read_file" => tools::read_file(
            required_string(&request.params, "root")?,
            required_string(&request.params, "path")?,
            request
                .params
                .get("startLine")
                .and_then(Value::as_u64)
                .unwrap_or(1) as usize,
            request
                .params
                .get("lineCount")
                .and_then(Value::as_u64)
                .unwrap_or(200) as usize,
        ),
        "get_file_info" => tools::get_file_info(
            required_string(&request.params, "root")?,
            required_string(&request.params, "path")?,
        ),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn required_string<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ServiceError::new(
                "invalid_request_params",
                "A required local service parameter is missing or invalid.",
                false,
            )
        })
}

fn project_root_for_mcp(
    storage: &AppStorage,
    project_id: &str,
) -> Result<std::path::PathBuf, ServiceError> {
    let connection = storage.connect().map_err(|_| {
        ServiceError::new(
            "mcp_catalog_unavailable",
            "The project MCP configuration could not be accessed.",
            true,
        )
    })?;
    let folder_path = connection
        .query_row(
            "SELECT folder_path FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| {
            ServiceError::new(
                "mcp_catalog_unavailable",
                "The project MCP configuration could not be accessed.",
                true,
            )
        })?
        .ok_or_else(|| {
            ServiceError::new(
                "project_unavailable",
                "The selected project is no longer available.",
                false,
            )
        })?;
    let root = std::fs::canonicalize(folder_path).map_err(|_| {
        ServiceError::new(
            "project_unavailable",
            "The selected project folder is unavailable.",
            false,
        )
    })?;
    if !root.is_dir() {
        return Err(ServiceError::new(
            "project_unavailable",
            "The selected project folder is unavailable.",
            false,
        ));
    }
    Ok(root)
}

fn required_string_array(params: &Value, name: &str) -> Result<Vec<String>, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 5_000)
        .and_then(|values| {
            values
                .iter()
                .map(Value::as_str)
                .map(|value| value.filter(|value| !value.trim().is_empty()))
                .collect::<Option<Vec<_>>>()
        })
        .map(|values| values.into_iter().map(str::to_owned).collect())
        .ok_or_else(invalid_request_params)
}

fn required_secret_string(
    params: &mut Value,
    name: &str,
) -> Result<Zeroizing<String>, ServiceError> {
    let value = params
        .as_object_mut()
        .and_then(|object| object.remove(name));
    match value {
        Some(Value::String(secret)) => Ok(Zeroizing::new(secret)),
        _ => Err(invalid_request_params()),
    }
}

fn invalid_request_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "A required local service parameter is missing or invalid.",
        false,
    )
}

fn mcp_credential_error(error: crate::credentials::CredentialStoreError) -> ServiceError {
    use crate::credentials::CredentialStoreError;

    match error {
        CredentialStoreError::InvalidReference => invalid_request_params(),
        CredentialStoreError::EmptyToken => ServiceError::new(
            "mcp_credential_empty",
            "Enter a non-empty MCP credential value.",
            false,
        ),
        CredentialStoreError::TokenTooLarge => ServiceError::new(
            "mcp_credential_too_large",
            "An MCP credential exceeds the 2,500-byte limit.",
            false,
        ),
        _ => ServiceError::new(
            "mcp_credential_storage_unavailable",
            "The MCP credential could not be accessed in Windows Credential Manager.",
            true,
        ),
    }
}

fn tool_permission_confirmation_required() -> ServiceError {
    ServiceError::new(
        "tool_permission_confirmation_required",
        "Confirm the displayed named task before running it.",
        false,
    )
}

fn file_changes_unavailable<T>(_: T) -> ServiceError {
    ServiceError::new(
        "file_changes_unavailable",
        "Conversation file changes could not be loaded or updated.",
        true,
    )
}

fn git_worktree_error(error: crate::git_worktrees::GitWorktreeError) -> ServiceError {
    use crate::git_worktrees::GitWorktreeError;
    match error {
        GitWorktreeError::InvalidInput => ServiceError::new(
            "invalid_request_params",
            "The project worktree request is invalid.",
            false,
        ),
        GitWorktreeError::ProjectUnavailable => ServiceError::new(
            "project_unavailable",
            "The project folder or selected worktree is unavailable.",
            false,
        ),
        GitWorktreeError::NotRepository => ServiceError::new(
            "project_not_git_repository",
            "This project folder is not inside a Git repository.",
            false,
        ),
        GitWorktreeError::UnsafePath => ServiceError::new(
            "project_worktree_path_unsafe",
            "The project worktree path failed a safety check.",
            false,
        ),
        GitWorktreeError::GitFailed => ServiceError::new(
            "project_worktree_operation_failed",
            "Git could not complete the worktree operation. Check the repository state and try again.",
            false,
        ),
        GitWorktreeError::TimedOut => ServiceError::new(
            "project_worktree_timed_out",
            "Git did not finish the worktree operation before the time limit.",
            true,
        ),
        GitWorktreeError::Unavailable => ServiceError::new(
            "project_worktree_unavailable",
            "The Git worktree service is unavailable.",
            true,
        ),
    }
}

fn required_non_zero_u32(params: &Value, name: &str) -> Result<u32, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid_local_engine_request)
}

fn required_port(params: &Value, name: &str) -> Result<u16, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid_local_engine_request)
}

fn invalid_local_engine_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The local engine request parameters are invalid.",
        false,
    )
}

fn required_memory_conversation_id(params: &Value) -> Result<&str, ServiceError> {
    let conversation_id = required_string(params, "conversationId")?;
    if conversation_id != conversation_id.trim()
        || conversation_id.chars().count() > 256
        || conversation_id.chars().any(char::is_control)
    {
        return Err(ServiceError::new(
            "invalid_request_params",
            "The conversation memory target is invalid.",
            false,
        ));
    }
    Ok(conversation_id)
}

fn required_memory_search_query(params: &Value) -> Result<&str, ServiceError> {
    let query = required_string(params, "query")?.trim();
    if !(2..=512).contains(&query.chars().count()) {
        return Err(ServiceError::new(
            "invalid_request_params",
            "The conversation memory search query is invalid.",
            false,
        ));
    }
    Ok(query)
}

fn optional_history_timestamp(params: &Value, field: &str) -> Result<Option<i64>, ServiceError> {
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    let timestamp = value
        .as_i64()
        .filter(|timestamp| (0..=8_640_000_000_000_000).contains(timestamp));
    timestamp.map(Some).ok_or_else(|| {
        ServiceError::new(
            "invalid_request_params",
            "The conversation history date range is invalid.",
            false,
        )
    })
}

fn optional_history_filter_string(
    params: &Value,
    field: &str,
) -> Result<Option<String>, ServiceError> {
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(invalid_history_filter());
    };
    if value.is_empty()
        || value.trim() != value
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        return Err(invalid_history_filter());
    }
    Ok(Some(value.to_owned()))
}

fn optional_history_tag_filter(params: &Value) -> Result<Option<String>, ServiceError> {
    let tag = optional_history_filter_string(params, "tag")?;
    if tag.as_ref().is_some_and(|value| value.chars().count() > 32) {
        return Err(invalid_history_filter());
    }
    Ok(tag)
}

fn optional_history_filter_bool(params: &Value, field: &str) -> Result<Option<bool>, ServiceError> {
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    value.as_bool().map(Some).ok_or_else(invalid_history_filter)
}

fn invalid_history_filter() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The conversation history filters are invalid.",
        false,
    )
}

fn archive_index_settings_json(settings: &chatgpt_store::ArchiveIndexSettings) -> Value {
    json!({
        "included": settings.included,
        "tools": settings.tools.iter().map(|tool: &chatgpt_store::ArchiveIndexTool| json!({
            "name": tool.name,
            "included": tool.included,
        })).collect::<Vec<_>>(),
    })
}

fn invalid_memory_archive_settings_params() -> ServiceError {
    ServiceError::new(
        "conversation_memory_settings_invalid",
        "Conversation archive settings are invalid.",
        false,
    )
}

fn invalid_context_usage_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The context usage request is invalid.",
        false,
    )
}

fn invalid_run_list_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The run list limit must be between 1 and 50.",
        false,
    )
}

fn map_question_storage_error(
    error: crate::storage::user_questions::UserQuestionError,
) -> ServiceError {
    use crate::storage::user_questions::UserQuestionError;
    match error {
        UserQuestionError::InvalidInput(_) => invalid_question_request("question data"),
        UserQuestionError::NotFound(_) => ServiceError::new(
            "question_run_not_found",
            "The pending AI response could not be found.",
            false,
        ),
        UserQuestionError::Conflict(_) | UserQuestionError::StaleRevision { .. } => {
            ServiceError::new(
                "question_run_unavailable",
                "The pending AI response is no longer available to resume.",
                false,
            )
        }
        UserQuestionError::Database(_) | UserQuestionError::CorruptStoredData(_) => {
            ServiceError::new(
                "question_storage_unavailable",
                "The pending AI response could not be resumed.",
                true,
            )
        }
    }
}

fn invalid_question_request(field: &str) -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        format!("The {field} value is invalid."),
        false,
    )
}

fn optional_string<'a>(params: &'a Value, name: &str) -> Result<Option<&'a str>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
        _ => Err(ServiceError::new(
            "invalid_request_params",
            "An optional local service parameter has an invalid value.",
            false,
        )),
    }
}

fn optional_bool(params: &Value, name: &str) -> Result<Option<bool>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        _ => Err(ServiceError::new(
            "invalid_request_params",
            "An optional local service parameter has an invalid value.",
            false,
        )),
    }
}

fn current_time_unix_ms() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        ServiceError::new("clock_unavailable", "System time is unavailable.", false)
    })?;
    i64::try_from(elapsed.as_millis())
        .map_err(|_| ServiceError::new("clock_unavailable", "System time is out of range.", false))
}

fn invalid_chat_goal_request(field: &str) -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        format!("The {field} is invalid."),
        false,
    )
}

fn is_quota_or_rate_limit_error(error: &ServiceError) -> bool {
    matches!(
        error.code,
        "rate_limited" | "quota_exceeded" | "insufficient_quota" | "resource_exhausted"
    )
}

fn statistics_query(params: &Value) -> Result<usage_statistics::StatisticsQuery, ServiceError> {
    let from_unix_ms = params
        .get("fromUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(invalid_request_params)?;
    let to_unix_ms = params
        .get("toUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value > from_unix_ms)
        .ok_or_else(invalid_request_params)?;
    let provider_id = optional_statistics_filter(params, "providerId")?;
    let model_id = optional_statistics_filter(params, "modelId")?;
    let operation = optional_statistics_filter(params, "operation")?;
    let reasoning_effort = optional_statistics_filter(params, "reasoningEffort")?;
    if operation.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "chat" | "tool_follow_up" | "compaction" | "title_generation"
        )
    }) || reasoning_effort.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "none"
                | "minimal"
                | "low"
                | "medium"
                | "high"
                | "xhigh"
                | "max"
                | "ultra"
                | "unspecified"
        )
    }) {
        return Err(invalid_request_params());
    }
    let fast_mode = optional_bool(params, "fastMode")?;
    let offset = params.get("offset").and_then(Value::as_i64).unwrap_or(0);
    let limit = params.get("limit").and_then(Value::as_i64).unwrap_or(100);
    if !(0..=100_000_000).contains(&offset) || !(1..=200).contains(&limit) {
        return Err(invalid_request_params());
    }
    Ok(usage_statistics::StatisticsQuery {
        from_unix_ms,
        to_unix_ms,
        provider_id,
        model_id,
        operation,
        reasoning_effort,
        fast_mode,
        offset,
        limit,
    })
}

fn optional_statistics_filter(params: &Value, name: &str) -> Result<Option<String>, ServiceError> {
    optional_string(params, name)?
        .map(|value| {
            if value == value.trim()
                && value.chars().count() <= 256
                && !value.chars().any(char::is_control)
            {
                Ok(value.to_owned())
            } else {
                Err(invalid_request_params())
            }
        })
        .transpose()
}

fn optional_api_key<'a>(
    params: &'a Value,
    parameter: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let api_key = optional_string(params, parameter)?;
    if api_key.is_some_and(|key| key.len() > 4096 || key.chars().any(char::is_control)) {
        return Err(ServiceError::new(
            "invalid_request_params",
            "The API key parameter is invalid.",
            false,
        ));
    }
    Ok(api_key)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool},
    };

    use rusqlite::params;
    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{
        chatgpt::ChatGptService,
        permissions::ToolPermissionBroker,
        protocol::{EventSink, Request},
        storage::{AppStorage, user_questions},
        user_question_broker::UserQuestionBroker,
    };

    use super::{
        dispatch, optional_history_tag_filter, required_memory_conversation_id,
        required_memory_search_query, required_secret_string, required_string_array,
    };

    #[test]
    fn validates_history_tag_filters() {
        assert_eq!(
            optional_history_tag_filter(&json!({"tag": "Research"})).expect("valid tag filter"),
            Some("Research".to_owned())
        );
        assert!(optional_history_tag_filter(&json!({"tag": "x".repeat(33)})).is_err());
        assert!(optional_history_tag_filter(&json!({"tag": 12})).is_err());
    }

    #[test]
    fn archive_rpc_removes_and_bounds_sensitive_parameters() {
        let mut params = json!({
            "passphrase": "secret phrase never persisted",
            "conversationIds": ["first", "second"]
        });
        let passphrase = required_secret_string(&mut params, "passphrase")
            .expect("archive passphrase should be extracted");
        assert_eq!(passphrase.as_str(), "secret phrase never persisted");
        assert!(params.get("passphrase").is_none());
        assert_eq!(
            required_string_array(&params, "conversationIds")
                .expect("conversation ids should parse"),
            ["first", "second"]
        );
        assert!(required_string_array(&json!({"conversationIds": []}), "conversationIds").is_err());
        assert!(
            required_string_array(&json!({"conversationIds": [" "]}), "conversationIds").is_err()
        );
    }

    #[test]
    fn memory_rpc_rejects_unbounded_or_ambiguous_values() {
        assert!(required_memory_conversation_id(&json!({"conversationId": " "})).is_err());
        assert!(required_memory_conversation_id(&json!({"conversationId": " id "})).is_err());
        assert!(
            required_memory_conversation_id(&json!({
                "conversationId": "x".repeat(257)
            }))
            .is_err()
        );
        assert!(required_memory_search_query(&json!({"query": "a"})).is_err());
        assert!(
            required_memory_search_query(&json!({
                "query": "x".repeat(513)
            }))
            .is_err()
        );
        assert_eq!(
            required_memory_search_query(&json!({"query": "  archive clue  "}))
                .expect("valid archive query"),
            "archive clue"
        );
    }

    #[tokio::test]
    async fn chat_send_saves_initial_provider_checkpoint_before_provider_authentication() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                 ALTER TABLE conversations ADD COLUMN project_id TEXT;
                 ALTER TABLE conversations ADD COLUMN model_id TEXT;
                 ALTER TABLE conversations ADD COLUMN connection_id TEXT;
                 ALTER TABLE conversations ADD COLUMN workspace_id TEXT;
                 CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT);
                 CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    UNIQUE (conversation_id, id)
                 );",
            )
            .expect("create desktop conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize native conversation schema");
        let connection = storage.connect().expect("connect initialized test storage");
        connection
            .execute(
                "INSERT INTO conversations (
                    id, provider_id, project_id, model_id, connection_id, workspace_id,
                    api_key_connection_id
                 ) VALUES (?1, 'gemini', NULL, 'test-model', NULL, NULL, 'gemini')",
                params!["conversation"],
            )
            .expect("insert API-key provider conversation");
        drop(connection);

        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.send".to_owned(),
            params: json!({
                "conversationId": "conversation",
                "apiKeyConnectionId": "gemini",
            }),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);
        let result = dispatch(
            &storage,
            &service,
            request,
            cancellation,
            EventSink::new(),
            ToolPermissionBroker::default(),
            UserQuestionBroker::default(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(true)),
        )
        .await;

        let error = result.expect_err("provider authentication should be required");
        assert_eq!(error.code, "authentication_required");

        let connection = storage.connect().expect("reconnect test storage");
        let run_id: String = connection
            .query_row(
                "SELECT id FROM agent_runs WHERE conversation_id = ?1",
                ["conversation"],
                |row| row.get(0),
            )
            .expect("chat.send should create a run");
        let run = user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
            .expect("load chat.send run");
        let checkpoint = run
            .checkpoint
            .expect("initial route checkpoint should be saved");
        assert_eq!(checkpoint["providerConnectionId"], "gemini");
        assert!(checkpoint.get("apiKey").is_none());
        assert!(checkpoint.get("apiKeyConnectionId").is_none());

        drop(connection);
        drop(service);
        drop(storage);
    }

    #[tokio::test]
    async fn chat_run_list_rpc_returns_recoverable_runs() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL, title TEXT);
                 ALTER TABLE conversations ADD COLUMN project_id TEXT;
                 ALTER TABLE conversations ADD COLUMN model_id TEXT;
                 ALTER TABLE conversations ADD COLUMN connection_id TEXT;
                 ALTER TABLE conversations ADD COLUMN workspace_id TEXT;
                 CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT);
                 CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    UNIQUE (conversation_id, id)
                 );",
            )
            .expect("create desktop conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize native conversation schema");
        let connection = storage.connect().expect("connect initialized storage");
        connection
            .execute(
                "INSERT INTO conversations (id, title) VALUES ('conversation', 'Background task')",
                [],
            )
            .expect("insert conversation");
        user_questions::create_run(
            &connection,
            &user_questions::NewAgentRun {
                run_id: "active-run".to_owned(),
                conversation_id: "conversation".to_owned(),
            },
        )
        .expect("create active run");
        user_questions::save_checkpoint(
            &connection,
            "conversation",
            "active-run",
            0,
            &json!({"providerId": "gemini", "modelId": "gemini-pro"}),
        )
        .expect("save run route");
        drop(connection);

        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.runs.list".to_owned(),
            params: json!({}),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);
        let response = dispatch(
            &storage,
            &service,
            request,
            cancellation,
            EventSink::new(),
            ToolPermissionBroker::default(),
            UserQuestionBroker::default(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(true)),
        )
        .await
        .expect("list runs");

        assert_eq!(response["runs"][0]["runId"], "active-run");
        assert_eq!(response["runs"][0]["conversationId"], "conversation");
        assert_eq!(response["runs"][0]["conversationTitle"], "Background task");
        assert_eq!(response["runs"][0]["providerId"], "gemini");
        assert_eq!(response["runs"][0]["modelId"], "gemini-pro");

        drop(service);
        drop(storage);
    }

    #[test]
    fn mcp_catalog_rpc_resolves_the_saved_project_path() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir(&project_root).expect("create project folder");
        let storage =
            Arc::new(AppStorage::open_at(directory.0.join("data")).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT)",
                [],
            )
            .expect("create project table");
        connection
            .execute(
                "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                params!["project-1", project_root.to_string_lossy().into_owned()],
            )
            .expect("save project folder");
        drop(connection);
        assert_eq!(
            super::project_root_for_mcp(&storage, "project-1").expect("resolve saved project"),
            fs::canonicalize(project_root).expect("canonicalize project")
        );
        assert!(super::project_root_for_mcp(&storage, "missing").is_err());
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("openchat-rpc-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
