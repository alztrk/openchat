mod chat_operation;
mod chatgpt;
mod chatgpt_store;
mod instructions;
mod oauth;
mod opencode;
mod permissions;
mod protocol;
mod provider_schema;
mod storage;
mod tools;

#[cfg(windows)]
mod credentials;

use std::{collections::HashMap, path::Path, sync::Arc};

use chat_operation::ChatSendContext;
use chatgpt::ChatGptService;
use permissions::ToolPermissionBroker;
use protocol::{EventSink, Request, Response, ServiceError};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use storage::AppStorage;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{Mutex, watch as tokio_watch},
    time::{Duration, sleep, timeout},
};
use tools::ToolPermissionMode;

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[tokio::main]
async fn main() {
    let storage = match AppStorage::open() {
        Ok(storage) => Arc::new(storage),
        Err(_) => {
            eprintln!("local_storage_initialization_failed");
            std::process::exit(1);
        }
    };
    let service = match ChatGptService::new(Arc::clone(&storage)) {
        Ok(service) => Arc::new(service),
        Err(error) => {
            eprintln!("local_service_initialization_failed: {}", error.code);
            std::process::exit(1);
        }
    };
    if run(storage, service).await.is_err() {
        eprintln!("local_service_protocol_failed");
        std::process::exit(1);
    }
}

async fn run(storage: Arc<AppStorage>, service: Arc<ChatGptService>) -> std::io::Result<()> {
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let output = EventSink::new();
    let active_operations = Arc::new(Mutex::new(
        HashMap::<String, tokio_watch::Sender<bool>>::new(),
    ));
    let permission_broker = ToolPermissionBroker::default();

    while let Some(line) = input.next_line().await? {
        if line.len() > MAX_REQUEST_BYTES {
            output
                .send(&Response::failure(
                    Value::Null,
                    ServiceError::new(
                        "request_too_large",
                        "The local service request exceeded its size limit.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }
        let request = match serde_json::from_str::<Request>(&line) {
            Ok(request) => request,
            Err(_) => {
                output
                    .send(&Response::failure(
                        Value::Null,
                        ServiceError::new(
                            "invalid_request",
                            "The local service request was not valid JSON RPC.",
                            false,
                        ),
                    ))
                    .await?;
                continue;
            }
        };
        if !request.id.is_string() && !request.id.is_u64() {
            output
                .send(&Response::failure(
                    Value::Null,
                    ServiceError::new(
                        "invalid_request_id",
                        "The local service request identifier must be a string or unsigned integer.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }
        if !request.params.is_object() {
            output
                .send(&Response::failure(
                    request.id,
                    ServiceError::new(
                        "invalid_request_params",
                        "The local service request parameters must be an object.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }

        if request.method == "system.shutdown" {
            output
                .send(&Response::success(request.id, json!({"stopping": true})))
                .await?;
            cancel_all(&active_operations).await;
            if timeout(Duration::from_millis(1500), async {
                loop {
                    if active_operations.lock().await.is_empty() {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .is_err()
            {
                eprintln!("local_service_shutdown_timed_out");
            }
            return Ok(());
        }
        if request.method == "system.cancel" {
            let target = request.params.get("targetId").and_then(request_key_value);
            let cancelled = if let Some(target) = target {
                let operations = active_operations.lock().await;
                operations
                    .get(&target)
                    .is_some_and(|cancellation| cancellation.send(true).is_ok())
            } else {
                false
            };
            output
                .send(&Response::success(
                    request.id,
                    json!({"cancelled": cancelled}),
                ))
                .await?;
            continue;
        }

        let operation_key = request_key(&request.id);
        let (cancellation_sender, cancellation_receiver) = tokio_watch::channel(false);
        {
            let mut operations = active_operations.lock().await;
            if operations.contains_key(&operation_key) {
                output
                    .send(&Response::failure(
                        request.id,
                        ServiceError::new(
                            "duplicate_request_id",
                            "The local service request identifier is already in use.",
                            false,
                        ),
                    ))
                    .await?;
                continue;
            }
            operations.insert(operation_key.clone(), cancellation_sender);
        }

        let operation_storage = Arc::clone(&storage);
        let operation_service = Arc::clone(&service);
        let operation_permissions = permission_broker.clone();
        let operation_output = output.clone();
        let operation_map = Arc::clone(&active_operations);
        tokio::spawn(async move {
            let id = request.id.clone();
            let response = match dispatch(
                &operation_storage,
                &operation_service,
                request,
                cancellation_receiver,
                operation_output.clone(),
                operation_permissions,
            )
            .await
            {
                Ok(result) => Response::success(id, result),
                Err(error) => Response::failure(id, error),
            };
            let _ = operation_output.send(&response).await;
            operation_map.lock().await.remove(&operation_key);
        });
    }

    cancel_all(&active_operations).await;
    Ok(())
}

async fn dispatch(
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
        "chatgpt.usage.get" => {
            let connection_id = required_string(&request.params, "connectionId")?;
            let workspace_id = required_string(&request.params, "workspaceId")?;
            service
                .usage(connection_id, workspace_id, &mut cancellation)
                .await
        }
        "chat.send" => {
            let conversation_id = required_string(&request.params, "conversationId")?;
            let open_code_api_key = optional_open_code_api_key(&request.params)?;
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
                    "SELECT conversations.provider_id, projects.folder_path
                     FROM conversations
                     LEFT JOIN projects ON projects.id = conversations.project_id
                     WHERE conversations.id = ?1",
                    [conversation_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
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
            let (provider_id, project_root) = route.unwrap_or((None, None));
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
            if provider_id.as_deref() == Some("opencode") {
                opencode::send_message(storage, context, open_code_api_key).await
            } else {
                service.send_message(context, reasoning_effort).await
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
            let api_key = optional_open_code_api_key(&request.params)?;
            let force_refresh = request
                .params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            opencode::models(storage, api_key, force_refresh, &mut cancellation).await
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

fn optional_open_code_api_key(params: &Value) -> Result<Option<&str>, ServiceError> {
    let api_key = optional_string(params, "apiKey")?;
    if api_key.is_some_and(|key| key.len() > 4096 || key.chars().any(char::is_control)) {
        return Err(ServiceError::new(
            "invalid_request_params",
            "The OpenCode API key parameter is invalid.",
            false,
        ));
    }
    Ok(api_key)
}

fn request_key(id: &Value) -> String {
    match id {
        Value::String(value) => format!("s:{value}"),
        Value::Number(value) => format!("n:{value}"),
        _ => String::new(),
    }
}

fn request_key_value(id: &Value) -> Option<String> {
    match id {
        Value::String(value) => Some(format!("s:{value}")),
        Value::Number(value) => Some(format!("n:{value}")),
        _ => None,
    }
}

async fn cancel_all(operations: &Mutex<HashMap<String, tokio_watch::Sender<bool>>>) {
    let operations = operations.lock().await;
    for cancellation in operations.values() {
        let _ = cancellation.send(true);
    }
}
