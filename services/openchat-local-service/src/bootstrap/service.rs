//! Owns the sidecar's newline-delimited request loop and bounded concurrent operations.
//!
//! Request frames, parsed JSON values, active-operation count, and aggregate in-flight budgets are
//! limited before dispatch. Cancellation and shutdown are propagated to active tasks before exit.

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
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
    io::{AsyncBufRead, AsyncBufReadExt, BufReader},
    sync::{Mutex, watch as tokio_watch},
    time::{Duration, sleep, timeout},
};
use zeroize::Zeroize;

const MAX_REQUEST_BYTES: usize = 24 * 1024 * 1024;
const MAX_REQUEST_ID_BYTES: usize = 256;
/// Keeps dispatch comparisons and request bookkeeping bounded to protocol method names.
const MAX_REQUEST_METHOD_BYTES: usize = 128;
const MAX_ACTIVE_OPERATIONS: usize = 64;
const MAX_ACTIVE_REQUEST_BYTES: usize = 64 * 1024 * 1024;
/// Bounds retained JSON trees across concurrent operations, in addition to raw request bytes.
const MAX_ACTIVE_REQUEST_PARAMETER_NODES: usize = 250_000;

/// Tracks RPC cancellation channels and work admitted under count, byte, and value limits.
#[derive(Default)]
struct ActiveOperations {
    requests: HashMap<String, ActiveOperation>,
    request_bytes: usize,
    parameter_nodes: usize,
}

/// Keeps cancellation and resource-budget ownership until one dispatched request finishes.
struct ActiveOperation {
    cancellation: tokio_watch::Sender<bool>,
    cancellation_gate: Arc<OperationGate>,
    request_bytes: usize,
    parameter_nodes: usize,
}

/// Serializes an operation's cancellation request against its final side effect.
#[derive(Default)]
pub(crate) struct OperationGate {
    state: AtomicU8,
}

impl OperationGate {
    const RUNNING: u8 = 0;
    const CANCELLATION_REQUESTED: u8 = 1;
    const COMMITTING: u8 = 2;
    const COMPLETED: u8 = 3;

    /// Accepts cancellation only while the operation has not claimed its commit step.
    pub(crate) fn request_cancellation(&self) -> bool {
        self.state
            .compare_exchange(
                Self::RUNNING,
                Self::CANCELLATION_REQUESTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Claims the final side effect unless cancellation or completion already won.
    pub(crate) fn try_begin_commit(&self) -> bool {
        self.state
            .compare_exchange(
                Self::RUNNING,
                Self::COMMITTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Prevents later cancellation after the dispatched operation has returned.
    fn complete(&self) {
        self.state.store(Self::COMPLETED, Ordering::Release);
    }
}

#[derive(Debug, Eq, PartialEq)]
enum OperationAdmissionError {
    DuplicateId,
    CapacityExceeded,
}

impl ActiveOperations {
    /// Registers a request only while both active-work budgets have capacity.
    fn register(
        &mut self,
        operation_key: String,
        cancellation: tokio_watch::Sender<bool>,
        cancellation_gate: Arc<OperationGate>,
        request_bytes: usize,
        parameter_nodes: usize,
    ) -> Result<(), OperationAdmissionError> {
        if self.requests.contains_key(&operation_key) {
            return Err(OperationAdmissionError::DuplicateId);
        }
        if self.requests.len() >= MAX_ACTIVE_OPERATIONS
            || self.request_bytes.saturating_add(request_bytes) > MAX_ACTIVE_REQUEST_BYTES
            || self.parameter_nodes.saturating_add(parameter_nodes)
                > MAX_ACTIVE_REQUEST_PARAMETER_NODES
        {
            return Err(OperationAdmissionError::CapacityExceeded);
        }

        self.request_bytes += request_bytes;
        self.parameter_nodes += parameter_nodes;
        self.requests.insert(
            operation_key,
            ActiveOperation {
                cancellation,
                cancellation_gate,
                request_bytes,
                parameter_nodes,
            },
        );
        Ok(())
    }

    /// Releases the count and payload budget associated with a completed operation.
    fn unregister(&mut self, operation_key: &str) {
        if let Some(operation) = self.requests.remove(operation_key) {
            self.request_bytes = self.request_bytes.saturating_sub(operation.request_bytes);
            self.parameter_nodes = self
                .parameter_nodes
                .saturating_sub(operation.parameter_nodes);
        }
    }
}

/// Keeps EOF, malformed-size, and valid request frames distinct on the RPC pipe.
enum BoundedRequestLine {
    EndOfInput,
    TooLarge,
    Line(String),
}

#[derive(Debug)]
/// A safe startup or protocol failure returned by the local service entry point.
pub enum RunError {
    /// The process arguments do not match the supported command-line form.
    InvalidArguments,
    /// The application storage could not be opened or initialized.
    StorageInitialization(String),
    /// A service could not be initialized; the values are its stable code and safe message.
    ServiceInitialization {
        /// Stable machine-readable code for the failed initialization step.
        code: &'static str,
        /// User-safe summary of the initialization failure.
        message: String,
        /// Optional diagnostic detail from the underlying subsystem.
        details: Option<String>,
    },
    /// The standard-input/output RPC transport failed.
    Protocol(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments => {
                formatter.write_str("usage: openchat-local-service [--data-root <absolute-path>]")
            }
            Self::StorageInitialization(details) => {
                write!(formatter, "local_storage_initialization_failed: {details}")
            }
            Self::ServiceInitialization {
                code,
                message,
                details,
            } => {
                write!(
                    formatter,
                    "local_service_initialization_failed: {code}: {message}"
                )?;
                if let Some(details) = details {
                    write!(formatter, " ({details})")?;
                }
                Ok(())
            }
            Self::Protocol(details) => {
                write!(formatter, "local_service_protocol_failed: {details}")
            }
        }
    }
}

/// Opens local storage and serves RPC requests over standard input and output.
///
/// The only accepted argument is `--data-root <absolute-path>`; when omitted,
/// the platform's normal OpenChat data root is used. Errors avoid embedding
/// request contents or credentials.
pub async fn run() -> Result<(), RunError> {
    let data_root = parse_data_root(std::env::args_os().skip(1))?;
    let storage = Arc::new(
        match data_root {
            Some(root) => AppStorage::open_at(root),
            None => AppStorage::open(),
        }
        .map_err(|error| RunError::StorageInitialization(error.to_string()))?,
    );
    let service = Arc::new(ChatGptService::new(Arc::clone(&storage)).map_err(|error| {
        RunError::ServiceInitialization {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    })?);
    run_protocol(storage, service)
        .await
        .map_err(|error| RunError::Protocol(error.to_string()))
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
    let mut input = BufReader::new(tokio::io::stdin());
    let output = EventSink::new();
    let active_operations = Arc::new(Mutex::new(ActiveOperations::default()));
    let permission_broker = ToolPermissionBroker::default();
    let user_question_broker = UserQuestionBroker::default();
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let startup_recovered = Arc::new(AtomicBool::new(false));

    loop {
        let mut line = match read_bounded_request_line(&mut input).await {
            Ok(BoundedRequestLine::Line(line)) => line,
            Ok(BoundedRequestLine::TooLarge) => {
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
            Ok(BoundedRequestLine::EndOfInput) => break,
            Err(error) => {
                finish_service_shutdown(&storage, &active_operations).await;
                return Err(error);
            }
        };
        let request_bytes = line.len();
        let parsed_request = crate::protocol::decode_request(&line);
        line.zeroize();
        let decoded_request = match parsed_request {
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
        let parameter_nodes = decoded_request.parameter_nodes;
        let request_too_complex = decoded_request.too_complex;
        let parameters_are_object = decoded_request.parameters_are_object;
        let mut request = decoded_request.request;
        if !is_valid_request_id(&request.id) {
            zeroize_passphrase(&mut request.params);
            output
                .send(&Response::failure(
                    Value::Null,
                    ServiceError::new(
                        "invalid_request_id",
                        "The local service request identifier must be a string of at most 256 bytes or an unsigned integer.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }
        if request.method.len() > MAX_REQUEST_METHOD_BYTES {
            zeroize_passphrase(&mut request.params);
            output
                .send(&Response::failure(
                    request.id.clone(),
                    ServiceError::new(
                        "invalid_request_method",
                        "The local service request method is too long.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }
        if request_too_complex {
            zeroize_passphrase(&mut request.params);
            output
                .send(&Response::failure(
                    request.id.clone(),
                    ServiceError::new(
                        "request_too_complex",
                        "The local service request contains too many JSON values.",
                        false,
                    ),
                ))
                .await?;
            continue;
        }
        if !parameters_are_object || !request.params.is_object() {
            zeroize_passphrase(&mut request.params);
            output
                .send(&Response::failure(
                    request.id.clone(),
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
            zeroize_passphrase(&mut request.params);
            shutdown_requested.store(true, Ordering::SeqCst);
            let response_result = output
                .send(&Response::success(
                    request.id.clone(),
                    json!({"stopping": true}),
                ))
                .await;
            finish_service_shutdown(&storage, &active_operations).await;
            response_result?;
            return Ok(());
        }
        if request.method == "system.cancel" {
            zeroize_passphrase(&mut request.params);
            let target = request.params.get("targetId").and_then(request_key_value);
            let cancelled = if let Some(target) = target {
                let operations = active_operations.lock().await;
                operations.requests.get(&target).is_some_and(|operation| {
                    operation.cancellation_gate.request_cancellation()
                        && operation.cancellation.send(true).is_ok()
                })
            } else {
                false
            };
            output
                .send(&Response::success(
                    request.id.clone(),
                    json!({"cancelled": cancelled}),
                ))
                .await?;
            continue;
        }

        if !matches!(
            request.method.as_str(),
            "conversation.archive.export"
                | "conversation.archive.inspect"
                | "conversation.archive.restore"
                | "profile.archive.export"
                | "profile.archive.prepare_restore"
        ) {
            zeroize_passphrase(&mut request.params);
        }

        let operation_key = request_key(&request.id);
        let (cancellation_sender, cancellation_receiver) = tokio_watch::channel(false);
        let cancellation_gate = Arc::new(OperationGate::default());
        let admission = {
            let mut operations = active_operations.lock().await;
            operations.register(
                operation_key.clone(),
                cancellation_sender,
                Arc::clone(&cancellation_gate),
                request_bytes,
                parameter_nodes,
            )
        };
        if let Err(error) = admission {
            zeroize_passphrase(&mut request.params);
            let service_error = match error {
                OperationAdmissionError::DuplicateId => ServiceError::new(
                    "duplicate_request_id",
                    "The local service request identifier is already in use.",
                    false,
                ),
                OperationAdmissionError::CapacityExceeded => ServiceError::new(
                    "local_service_busy",
                    "The local service is handling its request limit. Retry shortly.",
                    true,
                ),
            };
            output
                .send(&Response::failure(request.id.clone(), service_error))
                .await?;
            continue;
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
            let diagnostic_operation = rpc_failure_diagnostic_operation(&request);
            let response = match rpc::dispatch(rpc::RpcDispatchContext {
                storage: &operation_storage,
                service: &operation_service,
                request,
                cancellation: cancellation_receiver,
                cancellation_gate: Arc::clone(&cancellation_gate),
                events: operation_output.clone(),
                permission_broker: operation_permissions,
                user_question_broker: operation_questions,
                shutdown_requested: operation_shutdown,
                startup_recovered: operation_recovery,
            })
            .await
            {
                Ok(result) => Response::success(id, result),
                Err(error) => {
                    if let Some(operation) = diagnostic_operation {
                        if operation_storage
                            .log_rpc_failure(operation, error.code)
                            .is_err()
                        {
                            eprintln!("rpc_failure_diagnostic_write_failed");
                        }
                    }
                    Response::failure(id, error)
                }
            };
            cancellation_gate.complete();
            let _ = operation_output.send(&response).await;
            operation_map.lock().await.unregister(&operation_key);
        });
    }

    finish_service_shutdown(&storage, &active_operations).await;
    Ok(())
}

fn rpc_failure_diagnostic_operation(request: &Request) -> Option<&'static str> {
    match request.method.as_str() {
        "chat.send" => Some("chat.send"),
        "chat.temporary.send" => Some("chat.temporary.send"),
        "chat.memory.inspect" => Some("chat.memory.inspect"),
        "chatgpt.connections.delete" => Some("chatgpt.connections.delete"),
        "chat.context.usage.estimate" => Some("chat.context.usage.estimate"),
        "compatible.models.list"
            if request.params.get("providerId").and_then(Value::as_str) == Some("copilot") =>
        {
            Some("copilot.models.list")
        }
        _ => None,
    }
}

/// Reads one newline-delimited request while bounding retained input memory.
///
/// Oversized frames are drained through their delimiter so the following request remains aligned.
async fn read_bounded_request_line<R>(reader: &mut R) -> io::Result<BoundedRequestLine>
where
    R: AsyncBufRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut saw_input = false;
    let mut too_large = false;

    loop {
        let (consumed, reached_eof, reached_line_end) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                (0, true, false)
            } else {
                saw_input = true;
                let line_end = available
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .unwrap_or(available.len());
                let reached_line_end = line_end < available.len();
                let retained_limit = MAX_REQUEST_BYTES.saturating_add(1);
                if !too_large && bytes.len().saturating_add(line_end) <= retained_limit {
                    bytes.extend_from_slice(&available[..line_end]);
                } else {
                    too_large = true;
                    bytes.zeroize();
                }
                (
                    line_end + usize::from(reached_line_end),
                    false,
                    reached_line_end,
                )
            }
        };

        if reached_eof {
            break;
        }
        reader.consume(consumed);
        if reached_line_end {
            break;
        }
    }

    if !saw_input {
        return Ok(BoundedRequestLine::EndOfInput);
    }
    if too_large {
        return Ok(BoundedRequestLine::TooLarge);
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > MAX_REQUEST_BYTES {
        bytes.zeroize();
        return Ok(BoundedRequestLine::TooLarge);
    }

    match String::from_utf8(bytes) {
        Ok(line) => Ok(BoundedRequestLine::Line(line)),
        Err(error) => {
            let mut bytes = error.into_bytes();
            bytes.zeroize();
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the local service request line is not valid UTF-8",
            ))
        }
    }
}

fn zeroize_passphrase(params: &mut Value) {
    if let Some(Value::String(mut passphrase)) = params
        .as_object_mut()
        .and_then(|object| object.remove("passphrase"))
    {
        passphrase.zeroize();
    }
}

/// Accepts the supported RPC identifier types without allowing oversized string keys.
fn is_valid_request_id(id: &Value) -> bool {
    match id {
        Value::String(value) => value.len() <= MAX_REQUEST_ID_BYTES,
        Value::Number(value) => value.is_u64(),
        _ => false,
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
async fn cancel_all(operations: &Mutex<ActiveOperations>) {
    let operations = operations.lock().await;
    for operation in operations.requests.values() {
        if operation.cancellation_gate.request_cancellation() {
            let _ = operation.cancellation.send(true);
        }
    }
}

async fn finish_service_shutdown(storage: &AppStorage, operations: &Mutex<ActiveOperations>) {
    cancel_all(operations).await;
    if let Err(error) = crate::local_engines::stop_runtime(storage).await {
        eprintln!("local_engine_shutdown_failed:{}", error.code);
    }
    if timeout(Duration::from_millis(1500), async {
        loop {
            if operations.lock().await.requests.is_empty() {
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
    if crate::tools::terminal::TerminalSessionManager::stop_all_global()
        .await
        .is_err()
    {
        eprintln!("terminal_service_shutdown_failed");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        error::Error,
        ffi::OsString,
        fs, io,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, Instant},
    };

    use serde_json::json;
    use tokio::io::BufReader;
    use tokio::sync::watch as tokio_watch;

    use crate::{chatgpt::ChatGptService, storage::AppStorage};

    use super::{
        ActiveOperations, BoundedRequestLine, MAX_ACTIVE_OPERATIONS, MAX_ACTIVE_REQUEST_BYTES,
        MAX_ACTIVE_REQUEST_PARAMETER_NODES, MAX_REQUEST_BYTES, MAX_REQUEST_ID_BYTES,
        OperationAdmissionError, OperationGate, is_valid_request_id, parse_data_root,
        read_bounded_request_line, rpc_failure_diagnostic_operation, zeroize_passphrase,
    };
    use crate::protocol::Request;

    #[test]
    fn rpc_failure_diagnostics_use_fixed_labels_for_chat_and_connection_deletion() {
        let chat_request = Request {
            id: json!(1),
            method: "chat.send".to_owned(),
            params: json!({"message": "private user content"}),
        };
        assert_eq!(
            rpc_failure_diagnostic_operation(&chat_request),
            Some("chat.send")
        );

        let memory_inspect_request = Request {
            id: json!(2),
            method: "chat.memory.inspect".to_owned(),
            params: json!({"conversationId": "private conversation id"}),
        };
        assert_eq!(
            rpc_failure_diagnostic_operation(&memory_inspect_request),
            Some("chat.memory.inspect")
        );

        let connection_delete_request = Request {
            id: json!(3),
            method: "chatgpt.connections.delete".to_owned(),
            params: json!({"connectionId": "private connection id"}),
        };
        assert_eq!(
            rpc_failure_diagnostic_operation(&connection_delete_request),
            Some("chatgpt.connections.delete")
        );

        let copilot_catalog_request = Request {
            id: json!(4),
            method: "compatible.models.list".to_owned(),
            params: json!({"providerId": "copilot", "apiKey": "private credential"}),
        };
        assert_eq!(
            rpc_failure_diagnostic_operation(&copilot_catalog_request),
            Some("copilot.models.list")
        );

        let other_catalog_request = Request {
            id: json!(5),
            method: "compatible.models.list".to_owned(),
            params: json!({"providerId": "kilo"}),
        };
        assert_eq!(
            rpc_failure_diagnostic_operation(&other_catalog_request),
            None
        );
    }

    #[test]
    #[ignore = "manual debug service bootstrap component benchmark"]
    /// Measures database open and provider service construction separately in the debug profile.
    fn benchmarks_service_bootstrap_components() -> Result<(), Box<dyn Error>> {
        const SAMPLES: usize = 100;

        let directory = StartupBenchmarkDirectory::new()?;
        let mut cold_storage_samples = Vec::with_capacity(SAMPLES);
        for sample in 0..SAMPLES {
            let started = Instant::now();
            let storage = AppStorage::open_at(directory.path().join(format!("cold-{sample}")))
                .expect("open a new benchmark profile");
            cold_storage_samples.push(started.elapsed());
            drop(storage);
        }

        let warm_root = directory.path().join("warm");
        drop(AppStorage::open_at(warm_root.clone()).expect("create the warm benchmark profile"));
        let mut warm_storage_samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let storage = AppStorage::open_at(warm_root.clone())
                .expect("reopen the initialized benchmark profile");
            warm_storage_samples.push(started.elapsed());
            drop(storage);
        }

        let storage = Arc::new(
            AppStorage::open_at(warm_root).expect("open storage for service construction"),
        );
        let mut service_samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let service = ChatGptService::new(Arc::clone(&storage))
                .expect("construct the provider service without network requests");
            service_samples.push(started.elapsed());
            drop(service);
        }
        drop(storage);

        eprintln!(
            "service-bootstrap components debug n={SAMPLES}; cold-storage-open {}; warm-storage-open {}; provider-service-construction {}",
            format_latency_percentiles(&cold_storage_samples),
            format_latency_percentiles(&warm_storage_samples),
            format_latency_percentiles(&service_samples),
        );
        directory.cleanup()?;
        Ok(())
    }

    /// Owns an isolated temporary root used only by the ignored bootstrap benchmark.
    struct StartupBenchmarkDirectory {
        path: Option<PathBuf>,
    }

    impl StartupBenchmarkDirectory {
        /// Creates a unique temporary root so the benchmark cannot touch a user's profile.
        fn new() -> io::Result<Self> {
            let path = env::temp_dir().join(format!(
                "openchat-local-service-bootstrap-benchmark-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path: Some(path) })
        }

        /// Returns the active benchmark root.
        fn path(&self) -> &Path {
            self.path.as_deref().expect("benchmark directory is active")
        }

        /// Removes the benchmark root and reports cleanup failures to the test harness.
        fn cleanup(mut self) -> io::Result<()> {
            let path = self.path.take().expect("benchmark directory is active");
            fs::remove_dir_all(path)
        }
    }

    impl Drop for StartupBenchmarkDirectory {
        /// Makes a best-effort cleanup if the benchmark exits before explicit cleanup.
        fn drop(&mut self) {
            if let Some(path) = self.path.take()
                && let Err(error) = fs::remove_dir_all(path)
            {
                eprintln!(
                    "service_bootstrap_benchmark_temp_cleanup_failed: {}",
                    error.kind()
                );
            }
        }
    }

    /// Formats sample p50, p95, and p99 latencies in milliseconds.
    fn format_latency_percentiles(samples: &[Duration]) -> String {
        let mut sorted = samples.to_vec();
        assert!(!sorted.is_empty(), "benchmark samples must not be empty");
        sorted.sort_unstable();
        let percentile = |percent: usize| {
            let index = (sorted.len() - 1) * percent / 100;
            sorted[index].as_secs_f64() * 1_000.0
        };
        format!(
            "p50={:.3}ms p95={:.3}ms p99={:.3}ms",
            percentile(50),
            percentile(95),
            percentile(99),
        )
    }

    #[test]
    fn request_ids_accept_bounded_strings_and_unsigned_integers() {
        assert!(is_valid_request_id(&json!(
            "x".repeat(MAX_REQUEST_ID_BYTES)
        )));
        assert!(is_valid_request_id(&json!(
            "é".repeat(MAX_REQUEST_ID_BYTES / 2)
        )));
        assert!(!is_valid_request_id(&json!(
            "é".repeat(MAX_REQUEST_ID_BYTES / 2 + 1)
        )));
        assert!(!is_valid_request_id(&json!(
            "x".repeat(MAX_REQUEST_ID_BYTES + 1)
        )));
        assert!(is_valid_request_id(&json!(u64::MAX)));
        assert!(!is_valid_request_id(&json!(1.5)));
        assert!(!is_valid_request_id(&json!(-1)));
        assert!(!is_valid_request_id(&json!(null)));
    }

    #[test]
    fn cancellation_and_commit_gate_have_exactly_one_winner() {
        for _ in 0..64 {
            let gate = Arc::new(OperationGate::default());
            let barrier = Arc::new(std::sync::Barrier::new(3));

            let cancel_gate = Arc::clone(&gate);
            let cancel_barrier = Arc::clone(&barrier);
            let cancel = std::thread::spawn(move || {
                cancel_barrier.wait();
                cancel_gate.request_cancellation()
            });

            let commit_gate = Arc::clone(&gate);
            let commit_barrier = Arc::clone(&barrier);
            let commit = std::thread::spawn(move || {
                commit_barrier.wait();
                commit_gate.try_begin_commit()
            });

            barrier.wait();
            let cancellation_won = cancel.join().expect("cancellation task should finish");
            let commit_won = commit.join().expect("commit task should finish");

            assert_ne!(
                cancellation_won, commit_won,
                "cancellation and commit must not both win or lose"
            );
        }

        let completed = OperationGate::default();
        assert!(completed.try_begin_commit());
        completed.complete();
        assert!(!completed.request_cancellation());

        let cancelled = OperationGate::default();
        assert!(cancelled.request_cancellation());
        assert!(!cancelled.try_begin_commit());
    }

    #[test]
    fn active_operations_enforce_count_and_request_byte_limits() {
        let mut count_limited = ActiveOperations::default();
        for index in 0..MAX_ACTIVE_OPERATIONS {
            let (cancellation, _) = tokio_watch::channel(false);
            count_limited
                .register(
                    format!("request-{index}"),
                    cancellation,
                    Arc::new(OperationGate::default()),
                    1,
                    0,
                )
                .expect("operation should fit within the count limit");
        }
        let (cancellation, _) = tokio_watch::channel(false);
        assert_eq!(
            count_limited.register(
                "overflow".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                0,
            ),
            Err(OperationAdmissionError::CapacityExceeded)
        );
        let (cancellation, _) = tokio_watch::channel(false);
        assert_eq!(
            count_limited.register(
                "request-0".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                0,
            ),
            Err(OperationAdmissionError::DuplicateId)
        );
        count_limited.unregister("request-0");
        assert_eq!(count_limited.request_bytes, MAX_ACTIVE_OPERATIONS - 1);
        let (cancellation, _) = tokio_watch::channel(false);
        count_limited
            .register(
                "replacement".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                0,
            )
            .expect("completed operations release their count slot");

        let mut byte_limited = ActiveOperations::default();
        let (cancellation, _) = tokio_watch::channel(false);
        byte_limited
            .register(
                "large".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                MAX_ACTIVE_REQUEST_BYTES,
                0,
            )
            .expect("one request may use the full aggregate byte limit");
        let (cancellation, _) = tokio_watch::channel(false);
        assert_eq!(
            byte_limited.register(
                "next".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                0,
            ),
            Err(OperationAdmissionError::CapacityExceeded)
        );
        byte_limited.unregister("large");
        assert_eq!(byte_limited.request_bytes, 0);
        let (cancellation, _) = tokio_watch::channel(false);
        byte_limited
            .register(
                "small".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                0,
            )
            .expect("completed operations release their payload budget");
    }

    #[test]
    fn active_operations_enforce_and_release_parameter_node_budget() {
        let mut operations = ActiveOperations::default();
        let (cancellation, _) = tokio_watch::channel(false);
        operations
            .register(
                "large-tree".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                MAX_ACTIVE_REQUEST_PARAMETER_NODES,
            )
            .expect("one request may use the aggregate node limit");

        let (cancellation, _) = tokio_watch::channel(false);
        assert_eq!(
            operations.register(
                "next-node".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                1,
            ),
            Err(OperationAdmissionError::CapacityExceeded)
        );
        operations.unregister("large-tree");
        assert_eq!(operations.parameter_nodes, 0);

        let (cancellation, _) = tokio_watch::channel(false);
        operations
            .register(
                "replacement-tree".to_owned(),
                cancellation,
                Arc::new(OperationGate::default()),
                1,
                1,
            )
            .expect("completed requests release their parsed-value budget");
    }

    #[tokio::test]
    async fn request_reader_bounds_oversized_lines_and_keeps_following_requests() {
        let mut input = vec![b'x'; MAX_REQUEST_BYTES + 2];
        input.extend_from_slice(b"\n{}\r\nlast");
        let mut reader = BufReader::new(std::io::Cursor::new(input));

        assert!(matches!(
            read_bounded_request_line(&mut reader).await,
            Ok(BoundedRequestLine::TooLarge)
        ));
        assert!(matches!(
            read_bounded_request_line(&mut reader).await,
            Ok(BoundedRequestLine::Line(line)) if line == "{}"
        ));
        assert!(matches!(
            read_bounded_request_line(&mut reader).await,
            Ok(BoundedRequestLine::Line(line)) if line == "last"
        ));
        assert!(matches!(
            read_bounded_request_line(&mut reader).await,
            Ok(BoundedRequestLine::EndOfInput)
        ));
    }

    #[tokio::test]
    async fn request_reader_accepts_the_limit_with_a_crlf_ending() {
        let mut input = vec![b'x'; MAX_REQUEST_BYTES];
        input.extend_from_slice(b"\r\n");
        let mut reader = BufReader::new(std::io::Cursor::new(input));

        assert!(matches!(
            read_bounded_request_line(&mut reader).await,
            Ok(BoundedRequestLine::Line(line)) if line.len() == MAX_REQUEST_BYTES
        ));
    }

    #[test]
    fn request_cleanup_removes_the_archive_passphrase_field() {
        let mut params = json!({"passphrase": "temporary archive secret", "path": "file"});
        zeroize_passphrase(&mut params);
        assert!(params.get("passphrase").is_none());
        assert_eq!(params["path"], "file");
    }

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
