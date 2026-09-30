use std::{path::Path, sync::Arc};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch as tokio_watch;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt::ChatGptService,
    instructions, openai_api, openai_compatible,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, Request, ServiceError},
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
