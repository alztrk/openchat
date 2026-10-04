use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    chatgpt::ChatGptService,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, Request, Response, ServiceError},
    rpc,
    storage::AppStorage,
    user_question_broker::UserQuestionBroker,
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{Mutex, watch as tokio_watch},
    time::{Duration, sleep, timeout},
};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum RunError {
    InvalidArguments,
    StorageInitialization,
    ServiceInitialization(&'static str),
    Protocol,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments => {
                formatter.write_str("usage: openchat_service [--data-root <absolute-path>]")
            }
            Self::StorageInitialization => {
                formatter.write_str("local_storage_initialization_failed")
            }
            Self::ServiceInitialization(code) => {
                write!(formatter, "local_service_initialization_failed: {code}")
            }
            Self::Protocol => formatter.write_str("local_service_protocol_failed"),
        }
    }
}

pub async fn run() -> Result<(), RunError> {
    let data_root = parse_data_root(std::env::args_os().skip(1))?;
    let storage = Arc::new(
        match data_root {
            Some(root) => AppStorage::open_at(root),
            None => AppStorage::open(),
        }
        .map_err(|_| RunError::StorageInitialization)?,
    );
    let service = Arc::new(
        ChatGptService::new(Arc::clone(&storage))
            .map_err(|error| RunError::ServiceInitialization(error.code))?,
    );
    run_protocol(storage, service)
        .await
        .map_err(|_| RunError::Protocol)
}

fn parse_data_root(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Option<PathBuf>, RunError> {
    let mut arguments = arguments.into_iter();
    let Some(argument) = arguments.next() else {
        return Ok(None);
    };
    if argument != OsStr::new("--data-root") {
        return Err(RunError::InvalidArguments);
    }
    let Some(root) = arguments.next().map(PathBuf::from) else {
        return Err(RunError::InvalidArguments);
    };
    if !root.is_absolute() || arguments.next().is_some() {
        return Err(RunError::InvalidArguments);
    }
    Ok(Some(root))
}
async fn run_protocol(
    storage: Arc<AppStorage>,
    service: Arc<ChatGptService>,
) -> std::io::Result<()> {
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let output = EventSink::new();
    let active_operations = Arc::new(Mutex::new(
        HashMap::<String, tokio_watch::Sender<bool>>::new(),
    ));
    let permission_broker = ToolPermissionBroker::default();
    let user_question_broker = UserQuestionBroker::default();
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let startup_recovered = Arc::new(AtomicBool::new(false));

    loop {
        let line = match input.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                finish_service_shutdown(&active_operations).await;
                return Err(error);
            }
        };
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
            shutdown_requested.store(true, Ordering::SeqCst);
            let response_result = output
                .send(&Response::success(request.id, json!({"stopping": true})))
                .await;
            finish_service_shutdown(&active_operations).await;
            response_result?;
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
        let operation_questions = user_question_broker.clone();
        let operation_shutdown = Arc::clone(&shutdown_requested);
        let operation_recovery = Arc::clone(&startup_recovered);
        let operation_output = output.clone();
        let operation_map = Arc::clone(&active_operations);
        tokio::spawn(async move {
            let id = request.id.clone();
            let response = match rpc::dispatch(
                &operation_storage,
                &operation_service,
                request,
                cancellation_receiver,
                operation_output.clone(),
                operation_permissions,
                operation_questions,
                operation_shutdown,
                operation_recovery,
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

    finish_service_shutdown(&active_operations).await;
    Ok(())
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

async fn finish_service_shutdown(operations: &Mutex<HashMap<String, tokio_watch::Sender<bool>>>) {
    cancel_all(operations).await;
    if let Err(error) = crate::local_engines::stop_runtime().await {
        eprintln!("local_engine_shutdown_failed:{}", error.code);
    }
    if timeout(Duration::from_millis(1500), async {
        loop {
            if operations.lock().await.is_empty() {
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
}

#[cfg(test)]
mod tests {
    use std::{env, ffi::OsString};

    use super::parse_data_root;

    #[test]
    fn data_root_option_requires_one_absolute_path() {
        let root = env::temp_dir().join("openchat-isolated-profile");
        let arguments = [OsString::from("--data-root"), root.clone().into_os_string()];

        assert_eq!(
            parse_data_root(arguments).expect("valid data-root option"),
            Some(root)
        );
        assert!(parse_data_root([]).expect("default data root").is_none());
    }

    #[test]
    fn invalid_data_root_arguments_fail_before_storage_opens() {
        assert!(parse_data_root([OsString::from("--data-root")]).is_err());
        assert!(
            parse_data_root([
                OsString::from("--data-root"),
                OsString::from("relative-profile"),
            ])
            .is_err()
        );
        assert!(
            parse_data_root([
                OsString::from("--data-root"),
                env::temp_dir().into_os_string(),
                OsString::from("unexpected"),
            ])
            .is_err()
        );
        assert!(parse_data_root([OsString::from("--unexpected")]).is_err());
    }
}
