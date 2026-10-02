use std::{path::Path, sync::Arc};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch as tokio_watch;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt::ChatGptService,
    chatgpt_store, instructions, openai_api, openai_compatible,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, Request, Response, ServiceError},
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
};
pub(crate) async fn dispatch(
    storage: &AppStorage,
    service: &Arc<ChatGptService>,
    request: Request,
    mut cancellation: tokio_watch::Receiver<bool>,
    events: EventSink,
    permission_broker: ToolPermissionBroker,
) -> Result<Value, ServiceError> {
    match request.method.as_str() {
        "system.health" => Ok(json!({
            "status": "ready",
            "database_path": storage.database_path().to_string_lossy(),
            "storage_root": storage.root().to_string_lossy(),
            "schema_version": storage.schema_version(),
        })),
        "system.initialize" => {
            let schema_version = storage.initialize_backend_schema().map_err(|_| {
                ServiceError::new(
                    "storage_initialization_failed",
                    "Local conversation storage could not be initialized.",
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
            }))
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
            ) {
                return Err(invalid_context_usage_params());
            }
            let model_id = optional_string(&request.params, "modelId")?;
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
            let api_key = optional_api_key(&request.params, "apiKey")?;
            let api_key_connection_id = optional_string(&request.params, "apiKeyConnectionId")?;
            let reasoning_effort = optional_string(&request.params, "reasoningEffort")?;
            let custom_instructions = optional_string(&request.params, "customInstructions")?;
            let permission_mode = ToolPermissionMode::from_rpc(optional_string(
                &request.params,
                "toolPermissionMode",
            )?)?;
            instructions::validate_custom_instructions(custom_instructions)
                .map_err(|message| ServiceError::new("invalid_request_params", message, false))?;
            let excluded_assistant_message_id =
                optional_string(&request.params, "excludedAssistantMessageId")?;
            let route = storage
                .connect()
                .map_err(|_| {
                    ServiceError::new(
                        "storage_unavailable",
                        "Chat history could not be read.",
                        false,
                    )
                })?
                .query_row(
                    "SELECT conversations.provider_id, projects.folder_path,
                            conversations.api_key_connection_id
                     FROM conversations
                     LEFT JOIN projects ON projects.id = conversations.project_id
                     WHERE conversations.id = ?1",
                    [conversation_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
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
            let (provider_id, project_root, stored_api_key_connection_id) =
                route.unwrap_or((None, None, None));
            let context = ChatSendContext {
                request_id: request.id,
                conversation_id,
                excluded_assistant_message_id,
                custom_instructions,
                project_root: project_root.as_deref().map(Path::new),
                data_root: storage.root(),
                permission_mode,
                permission_broker: &permission_broker,
                cancellation: &mut cancellation,
                events,
            };
            match provider_id.as_deref() {
                Some(
                    "opencode" | "chatgpt_api" | "gemini" | "groq" | "cerebras" | "openrouter",
                ) => {
                    openai_compatible::send_message(
                        storage,
                        context,
                        api_key,
                        api_key_connection_id,
                        stored_api_key_connection_id,
                        reasoning_effort,
                    )
                    .await
                }
                _ => service.send_message(context, reasoning_effort).await,
            }
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

fn invalid_context_usage_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The context usage request is invalid.",
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
    use serde_json::json;

    use super::{required_memory_conversation_id, required_memory_search_query};

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
}
