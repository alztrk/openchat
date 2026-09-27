mod chatgpt;
mod chatgpt_store;
mod oauth;
mod protocol;
mod storage;

#[cfg(windows)]
mod credentials;

use std::{collections::HashMap, sync::Arc};

use chatgpt::ChatGptService;
use protocol::{EventSink, Request, Response, ServiceError};
use serde_json::{Value, json};
use storage::AppStorage;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{Mutex, watch},
    time::{Duration, sleep, timeout},
};

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
    let active_operations = Arc::new(Mutex::new(HashMap::<String, watch::Sender<bool>>::new()));

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
        let (cancellation_sender, cancellation_receiver) = watch::channel(false);
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
    mut cancellation: watch::Receiver<bool>,
    events: EventSink,
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
            service
                .models(connection_id, workspace_id, &mut cancellation)
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
            let reasoning_effort = optional_string(&request.params, "reasoningEffort")?;
            service
                .send_message(
                    request.id,
                    conversation_id,
                    reasoning_effort,
                    &mut cancellation,
                    events,
                )
                .await
        }
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

async fn cancel_all(operations: &Mutex<HashMap<String, watch::Sender<bool>>>) {
    let operations = operations.lock().await;
    for cancellation in operations.values() {
        let _ = cancellation.send(true);
    }
}
