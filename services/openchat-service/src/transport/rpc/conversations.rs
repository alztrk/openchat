//! RPC routing for conversation memory, history, context estimates, and archives.

use std::path::Path;

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    chatgpt_store, conversation_archive, instructions, openai_compatible, profile_archive,
    protocol::{EventSink, Response, ServiceError},
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
};

use super::{
    invalid_request_params, is_custom_provider_id, optional_bool, optional_string,
    required_secret_string, required_string, tool_permissions,
};

/// Dispatches conversation memory, history, context-estimate, and archive requests.
pub(super) async fn dispatch(
    method: &str,
    params: &mut Value,
    request_id: &Value,
    storage: &AppStorage,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "chat.memory.inspect" => {
            let conversation_id = required_memory_conversation_id(params)?;
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
            let conversation_id = required_memory_conversation_id(params)?;
            let included = params
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
            let conversation_id = required_memory_conversation_id(params)?;
            let tool_name = required_string(params, "toolName")?;
            let included = params
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
            let provider_id = required_string(params, "providerId")?;
            if !matches!(
                provider_id,
                "chatgpt"
                    | "chatgpt_api"
                    | "opencode"
                    | "copilot"
                    | "gemini"
                    | "groq"
                    | "cerebras"
                    | "openrouter"
                    | "mistral"
                    | "deepseek"
                    | "xai"
                    | "perplexity"
                    | "together"
                    | "sambanova"
                    | "anthropic"
                    | "fireworks"
                    | "deepinfra"
                    | "dashscope"
                    | "cohere"
                    | "llama_cpp"
            ) && !is_custom_provider_id(provider_id)
            {
                return Err(invalid_context_usage_params());
            }
            let model_id = optional_string(params, "modelId")?;
            let reported_supports_tool_calls = match params.get("supportsTools") {
                None | Some(Value::Null) => None,
                Some(Value::Bool(supported)) => Some(*supported),
                _ => return Err(invalid_context_usage_params()),
            };
            let copilot_request_protocol = match params.get("requestProtocol") {
                None | Some(Value::Null) => None,
                Some(Value::String(protocol)) if provider_id == "copilot" => {
                    match protocol.as_str() {
                        "chatCompletions" | "messages" | "responses" => Some(protocol.as_str()),
                        _ => return Err(invalid_context_usage_params()),
                    }
                }
                _ => return Err(invalid_context_usage_params()),
            };
            if params.get("toolPermissionMode").is_some()
                || params.get("toolPermissionRules").is_some()
            {
                return Err(invalid_context_usage_params());
            }
            let plan_mode_requested = optional_bool(params, "planMode")?.unwrap_or(false);
            let profile_tool_allowlist =
                tools::parse_profile_tool_allowlist(params.get("profileToolAllowlist"))?;
            let custom_instructions = optional_string(params, "customInstructions")?;
            instructions::validate_custom_instructions(custom_instructions)
                .map_err(|_| invalid_context_usage_params())?;
            let conversation_id = optional_string(params, "conversationId")?;
            let project_id = optional_string(params, "projectId")?;
            let permission_mode = if plan_mode_requested {
                ToolPermissionMode::Plan
            } else {
                let connection = storage.connect().map_err(|_| {
                    ServiceError::new(
                        "storage_unavailable",
                        "Chat context usage could not be measured.",
                        true,
                    )
                })?;
                tool_permissions::load_mode(&connection)?
            };
            let project_root = match (conversation_id, project_id) {
                (Some(conversation_id), _) => {
                    let connection = storage.connect().map_err(|_| {
                        ServiceError::new(
                            "storage_unavailable",
                            "Chat context usage could not be measured.",
                            true,
                        )
                    })?;
                    connection
                        .query_row(
                            "SELECT projects.folder_path
                             FROM conversations
                             INNER JOIN projects ON projects.id = conversations.project_id
                             WHERE conversations.id = ?1",
                            [conversation_id],
                            |row| row.get::<_, Option<String>>(0),
                        )
                        .optional()
                        .map_err(|_| {
                            ServiceError::new(
                                "storage_unavailable",
                                "Chat context usage could not be measured.",
                                true,
                            )
                        })?
                        .flatten()
                        .filter(|path| !path.trim().is_empty())
                }
                (None, Some(project_id)) => {
                    let connection = storage.connect().map_err(|_| {
                        ServiceError::new(
                            "storage_unavailable",
                            "Chat context usage could not be measured.",
                            true,
                        )
                    })?;
                    connection
                        .query_row(
                            "SELECT folder_path FROM projects WHERE id = ?1",
                            [project_id],
                            |row| row.get::<_, Option<String>>(0),
                        )
                        .optional()
                        .map_err(|_| {
                            ServiceError::new(
                                "storage_unavailable",
                                "Chat context usage could not be measured.",
                                true,
                            )
                        })?
                        .flatten()
                        .filter(|path| !path.trim().is_empty())
                }
                (None, None) => None,
            };
            let project_context_unavailable = project_id.is_some() && project_root.is_none();
            let project_instructions = if permission_mode == ToolPermissionMode::Plan {
                None
            } else {
                project_root
                    .as_deref()
                    .map(|root| crate::project_instructions::load(Path::new(root)))
                    .transpose()?
                    .flatten()
            };
            let project_skill_ids = match params.get("projectSkillIds") {
                None => Vec::new(),
                Some(Value::Array(values)) => values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(invalid_context_usage_params)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => return Err(invalid_context_usage_params()),
            };
            crate::project_skills::validate_selected_ids(&project_skill_ids)?;
            let project_skills = if permission_mode == ToolPermissionMode::Plan {
                Vec::new()
            } else {
                match project_root.as_deref() {
                    Some(root) => {
                        crate::project_skills::load_selected(Path::new(root), &project_skill_ids)?
                    }
                    None if project_skill_ids.is_empty() => Vec::new(),
                    None => return Err(invalid_context_usage_params()),
                }
            };
            let has_project = project_root.is_some();
            let available_tools = if permission_mode == ToolPermissionMode::Plan {
                Vec::new()
            } else {
                openai_compatible::context_usage_tool_definitions(
                    storage,
                    provider_id,
                    model_id,
                    reported_supports_tool_calls,
                    copilot_request_protocol,
                )?
            };
            let instructions =
                instructions::shared_instructions(instructions::SharedInstructionInput {
                    custom: custom_instructions,
                    project: project_instructions.as_deref(),
                    project_skills: &project_skills,
                    project_index_context: None,
                    permission_mode,
                    has_project,
                    tools_available: !available_tools.is_empty(),
                    provider_id,
                });
            let tool_definitions = available_tools
                .into_iter()
                .filter(|(name, _)| {
                    profile_tool_allowlist
                        .as_ref()
                        .is_none_or(|allowed| allowed.contains(name))
                })
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
                "projectContextUnavailable": project_context_unavailable,
            }))
        }
        "chat.memory.search" => {
            let conversation_id = required_memory_conversation_id(params)?;
            let query = required_memory_search_query(params)?;
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
            let query = required_memory_search_query(params)?;
            let from_unix_ms = optional_history_timestamp(params, "fromUnixMs")?;
            let through_unix_ms = optional_history_timestamp(params, "throughUnixMs")?;
            if matches!((from_unix_ms, through_unix_ms), (Some(from), Some(through)) if from >= through)
            {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The conversation history date range is invalid.",
                    false,
                ));
            }
            let result_type = optional_history_filter_string(params, "resultType")?
                .unwrap_or_else(|| "all".to_owned());
            let sort_order = optional_history_filter_string(params, "sortOrder")?
                .unwrap_or_else(|| "relevance".to_owned());
            if !matches!(result_type.as_str(), "all" | "messages" | "conversations")
                || !matches!(sort_order.as_str(), "relevance" | "newest" | "oldest")
            {
                return Err(invalid_history_filter());
            }
            let filters = chatgpt_store::HistorySearchFilters {
                from_unix_ms,
                through_unix_ms,
                provider_id: optional_history_filter_string(params, "providerId")?,
                model_id: optional_history_filter_string(params, "modelId")?,
                project_id: optional_history_filter_string(params, "projectId")?,
                is_archived: optional_history_filter_bool(params, "isArchived")?,
                tag: optional_history_tag_filter(params)?,
                result_type,
                sort_order,
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
                    "isConversationTitleMatch": result.is_conversation_title_match,
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
            let passphrase = required_secret_string(params, "passphrase")?;
            let conversation_ids = required_string_array(params, "conversationIds")?;
            conversation_archive::export(storage, &conversation_ids, passphrase)
        }
        "conversation.archive.inspect" => {
            let passphrase = required_secret_string(params, "passphrase")?;
            let path = required_string(params, "path")?.to_owned();
            conversation_archive::inspect(storage, &path, passphrase)
        }
        "conversation.archive.restore" => {
            let passphrase = required_secret_string(params, "passphrase")?;
            let path = required_string(params, "path")?.to_owned();
            let conflict_policy = required_string(params, "conflictPolicy")?.to_owned();
            conversation_archive::restore(storage, &path, passphrase, &conflict_policy)
        }
        "profile.archive.export" => {
            let passphrase = required_secret_string(params, "passphrase")?;
            profile_archive::export(storage, passphrase)
        }
        "profile.archive.prepare_restore" => {
            let passphrase = required_secret_string(params, "passphrase")?;
            let path = required_string(params, "path")?.to_owned();
            let chat_schema_version = params
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
            let request_id = request_id.clone();
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
                chatgpt_store::prepare_semantic_search(storage, cancellation, progress_sender)
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
            let conversation_id = required_memory_conversation_id(params)?;
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
        "chat.memory.compact" => {
            let conversation_id = required_memory_conversation_id(params)?;
            let context_window = params
                .get("contextWindow")
                .and_then(Value::as_i64)
                .filter(|window| (2048..=2_000_000).contains(window))
                .ok_or_else(invalid_request_params)?;
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "storage_unavailable",
                    "Chat history could not be read.",
                    false,
                )
            })?;
            let route = connection
                .query_row(
                    "SELECT provider_id, model_id, connection_id, workspace_id
                     FROM conversations WHERE id = ?1",
                    [conversation_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
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
            let Some((Some(provider_id), Some(model_id), connection_id, workspace_id)) = route
            else {
                return Err(ServiceError::new(
                    "conversation_not_routed",
                    "Choose a model before compacting this conversation.",
                    false,
                ));
            };
            let messages =
                chatgpt_store::conversation_messages(storage, conversation_id).map_err(|_| {
                    ServiceError::new(
                        "storage_unavailable",
                        "Chat history could not be read.",
                        false,
                    )
                })?;
            let context_state =
                chatgpt_store::load_conversation_context_state(storage, conversation_id).map_err(
                    |_| {
                        ServiceError::new(
                            "conversation_memory_unavailable",
                            "Conversation memory could not be loaded.",
                            true,
                        )
                    },
                )?;
            let active_compaction_matches = context_state.as_ref().is_some_and(|state| {
                crate::context_compaction::compaction_payload_matches_route(
                    state,
                    &provider_id,
                    connection_id.as_deref(),
                    workspace_id.as_deref(),
                    &model_id,
                ) && crate::context_compaction::has_compaction_boundary(state, &messages)
            });
            let extends_local_summary = active_compaction_matches
                && context_state
                    .as_ref()
                    .is_some_and(|state| state.compaction_kind.as_deref() == Some("summary"));
            let start_index = extends_local_summary
                .then(|| {
                    context_state
                        .as_ref()
                        .and_then(|state| state.compacted_through_message_id.as_deref())
                })
                .flatten()
                .and_then(|boundary_id| {
                    messages
                        .iter()
                        .position(|message| message.id == boundary_id)
                })
                .map_or(0, |index| index + 1);
            let Some(compacted_through_index) = crate::context_compaction::compaction_prefix_end(
                &messages,
                start_index,
                context_window,
            ) else {
                return Ok(json!({"compacted": false}));
            };
            let existing_summary = context_state
                .as_ref()
                .filter(|_| active_compaction_matches)
                .filter(|state| state.compaction_kind.as_deref() == Some("summary"))
                .and_then(|state| state.compaction_payload.as_deref());
            let summary = crate::context_compaction::local_extractive_summary(
                existing_summary,
                &messages[start_index..=compacted_through_index],
                crate::context_compaction::local_summary_byte_limit(context_window),
                cancellation,
            )?;
            let boundary_id = messages[compacted_through_index].id.clone();
            let compacted_state = chatgpt_store::ConversationContextState {
                compaction_kind: Some("summary".to_owned()),
                compaction_payload: Some(summary),
                compaction_provider_id: Some(provider_id),
                compaction_connection_id: connection_id,
                compaction_workspace_id: workspace_id,
                compaction_model_id: Some(model_id),
                compacted_through_message_id: Some(boundary_id.clone()),
                last_prompt_tokens: None,
                last_prompt_message_id: None,
                last_prompt_provider_id: None,
                last_prompt_model_id: None,
                last_prompt_connection_id: None,
                last_prompt_workspace_id: None,
            };
            chatgpt_store::save_compaction_state(storage, conversation_id, &compacted_state)
                .map_err(|_| {
                    ServiceError::new(
                        "conversation_memory_unavailable",
                        "The compacted conversation context could not be saved.",
                        true,
                    )
                })?;
            Ok(json!({
                "compacted": true,
                "compactedThroughMessageId": boundary_id,
            }))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{protocol::EventSink, storage::AppStorage};

    use super::{
        dispatch, optional_history_tag_filter, required_memory_conversation_id,
        required_memory_search_query, required_secret_string, required_string_array,
    };

    #[test]
    fn memory_search_and_history_filters_reject_ambiguous_values() {
        assert!(required_memory_conversation_id(&json!({"conversationId": " "})).is_err());
        assert!(required_memory_conversation_id(&json!({"conversationId": " id "})).is_err());
        assert!(
            required_memory_conversation_id(&json!({
                "conversationId": "x".repeat(257)
            }))
            .is_err()
        );
        assert!(required_memory_search_query(&json!({"query": "a"})).is_err());
        assert!(required_memory_search_query(&json!({"query": "x".repeat(513)})).is_err());
        assert_eq!(
            required_memory_search_query(&json!({"query": "  archive clue  "}))
                .expect("valid archive query"),
            "archive clue"
        );
        assert_eq!(
            optional_history_tag_filter(&json!({"tag": "Research"})).expect("valid tag"),
            Some("Research".to_owned())
        );
        assert!(optional_history_tag_filter(&json!({"tag": "x".repeat(33)})).is_err());
        assert!(optional_history_tag_filter(&json!({"tag": 12})).is_err());
    }

    #[test]
    fn archive_inputs_are_bounded_and_removed_from_rpc_parameters() {
        let mut params = json!({
            "passphrase": "archive test phrase",
            "conversationIds": ["first", "second"]
        });
        let passphrase = required_secret_string(&mut params, "passphrase")
            .expect("archive passphrase should be extracted");
        assert_eq!(passphrase.as_str(), "archive test phrase");
        assert!(params.get("passphrase").is_none());
        assert_eq!(
            required_string_array(&params, "conversationIds")
                .expect("conversation IDs should parse"),
            ["first", "second"]
        );
        assert!(required_string_array(&json!({"conversationIds": []}), "conversationIds").is_err());
        assert!(
            required_string_array(&json!({"conversationIds": [" "]}), "conversationIds").is_err()
        );
    }

    #[tokio::test]
    async fn conversation_dispatch_preserves_errors_and_removes_archive_secrets() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let events = EventSink::new();
        let (_sender, mut cancellation) = watch::channel(false);
        let request_id = json!("conversation-test");

        let mut params = json!({"passphrase": "archive-test-secret"});
        let archive_error = dispatch(
            "conversation.archive.inspect",
            &mut params,
            &request_id,
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("missing archive paths are rejected");
        assert_eq!(archive_error.code, "invalid_request_params");
        assert!(params.get("passphrase").is_none());

        let mut params = json!({"providerId": "unsupported"});
        let context_error = dispatch(
            "chat.context.usage.estimate",
            &mut params,
            &request_id,
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("unsupported provider identifiers are rejected");
        assert_eq!(context_error.code, "invalid_request_params");

        let mut params = json!({});
        let unknown_error = dispatch(
            "chat.memory.unknown",
            &mut params,
            &request_id,
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("unknown conversation methods are rejected");
        assert_eq!(unknown_error.code, "method_not_found");
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-conversation-rpc-test-{}", Uuid::new_v4()));
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
