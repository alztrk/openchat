use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch as tokio_watch;
use zeroize::Zeroizing;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt::ChatGptService,
    chatgpt_store, conversation_archive, hugging_face, instructions, local_engines, openai_api,
    openai_compatible,
    permissions::ToolPermissionBroker,
    profile_archive,
    protocol::{EventSink, Request, Response, ServiceError},
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
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
            let instructions = instructions::shared_instructions(
                custom_instructions,
                permission_mode,
                has_project,
            );
            let tool_definitions = openai_compatible::context_usage_tool_definitions(
                storage,
                provider_id,
                model_id,
                reported_supports_tool_calls,
            )?
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
            let requested_custom_instructions =
                optional_string(&request.params, "customInstructions")?;
            let permission_mode = ToolPermissionMode::from_rpc(optional_string(
                &request.params,
                "toolPermissionMode",
            )?)?;
            let excluded_assistant_message_id =
                optional_string(&request.params, "excludedAssistantMessageId")?;
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "storage_unavailable",
                    "Chat history could not be read.",
                    false,
                )
            })?;
            let route = connection
                .query_row(
                    "SELECT conversations.provider_id, projects.folder_path,
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
                stored_api_key_connection_id,
                model_id,
                connection_id,
                workspace_id,
            ) = route.unwrap_or((None, None, None, None, None, None));
            let resuming_question_run = resume_run_id.is_some();
            let (run_id, custom_instructions, reasoning_effort) = if let Some(run_id) =
                resume_run_id
            {
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
                if checkpoint.get("providerId").and_then(Value::as_str) != provider_id.as_deref()
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
                instructions::validate_custom_instructions(custom_instructions.as_deref())
                    .map_err(|message| {
                        ServiceError::new("question_run_checkpoint_invalid", message, false)
                    })?;
                crate::storage::user_questions::claim_run_resume(
                    &connection,
                    conversation_id,
                    run_id,
                )
                .map_err(map_question_storage_error)?;
                (run_id.to_owned(), custom_instructions, reasoning_effort)
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
                instructions::validate_custom_instructions(requested_custom_instructions).map_err(
                    |message| ServiceError::new("invalid_request_params", message, false),
                )?;
                let checkpoint = json!({
                    "version": 1,
                    "providerId": provider_id,
                    "modelId": model_id,
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "providerConnectionId": stored_api_key_connection_id,
                    "customInstructions": requested_custom_instructions,
                    "reasoningEffort": requested_reasoning_effort,
                });
                crate::storage::user_questions::save_checkpoint(
                    &connection,
                    conversation_id,
                    &run_id,
                    0,
                    &checkpoint,
                )
                .map_err(map_question_storage_error)?;
                (
                    run_id,
                    requested_custom_instructions.map(str::to_owned),
                    requested_reasoning_effort.map(str::to_owned),
                )
            };
            let context = ChatSendContext {
                request_id: request.id,
                run_id: run_id.clone(),
                conversation_id,
                excluded_assistant_message_id,
                custom_instructions: custom_instructions.as_deref(),
                project_root: project_root.as_deref().map(Path::new),
                data_root: storage.root(),
                storage,
                permission_mode,
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
                    )
                    .await
                }
                _ => {
                    service
                        .send_message(context, reasoning_effort.as_deref())
                        .await
                }
            };
            let outcome = match &result {
                Ok(response)
                    if response
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| status == "stopped") =>
                {
                    RunOutcome::Cancelled
                }
                Ok(_) => RunOutcome::Completed,
                Err(error) if error.code == "operation_cancelled" => {
                    if shutdown_requested.load(Ordering::SeqCst) {
                        RunOutcome::Interrupted
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
            user_question_broker
                .finish_run(storage, conversation_id, &run_id, outcome)
                .await?;
            result
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
        dispatch, required_memory_conversation_id, required_memory_search_query,
        required_secret_string, required_string_array,
    };

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
