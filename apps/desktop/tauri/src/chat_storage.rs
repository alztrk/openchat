//! Native conversation export and bounded chat diagnostics.

use serde::Serialize;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "chat_storage/markdown_export.rs"]
mod markdown_export;
#[path = "chat_storage/migrations.rs"]
mod migrations;

pub use markdown_export::*;
pub use migrations::*;

pub const MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES: usize = 255;
pub const MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES: usize = 128;
pub const MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES: usize = 20 * 1024 * 1024;

const MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES: u64 = 512 * 1024;
const MAX_CHAT_PERSISTENCE_DIAGNOSTIC_VALUE: u64 = 64 * 1024 * 1024;
static CHAT_STORAGE_DIAGNOSTIC_LOCK: Mutex<()> = Mutex::new(());

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatMessagePersistenceMetrics {
    content_bytes: u64,
    citation_source_bytes: u64,
    personal_memory_fact_id_bytes: u64,
    reasoning_summary_bytes: u64,
    tool_activity_bytes: u64,
    tool_activity_count: Option<u64>,
    attachment_count: u64,
    attachment_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Safe error contract for bounded conversation export commands.
pub struct ChatStorageError {
    code: &'static str,
    message: &'static str,
}

impl ChatStorageError {
    pub(crate) fn unavailable() -> Self {
        Self {
            code: "storage_unavailable",
            message: "Local conversation storage is unavailable.",
        }
    }

    pub(crate) fn invalid_input() -> Self {
        Self {
            code: "invalid_request_params",
            message: "The conversation data is invalid.",
        }
    }
}

/// Returns the chat schema version supported by this application build.
#[tauri::command]
pub fn get_chat_schema_version() -> i64 {
    CHAT_SCHEMA_VERSION
}

/// Records a failed renderer chat request using only a fixed stage and error code.
#[tauri::command]
pub fn record_chat_request_failure(
    stage: String,
    code: String,
    detail_code: Option<String>,
) -> Result<(), ChatStorageError> {
    let stage = match stage.as_str() {
        "conversationCreation" => "conversation_creation",
        "userMessagePersistence" => "user_message_persistence",
        "requestExecution" => "request_execution",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    if code.is_empty()
        || !valid_diagnostic_code(&code)
        || detail_code
            .as_deref()
            .is_some_and(|detail| !valid_diagnostic_code(detail))
    {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic(
        "app",
        "chat_request_failed",
        "stage",
        stage,
        &code,
        detail_code.as_deref(),
    )
}

/// Records bounded chat-message persistence failures without message content or IDs.
#[tauri::command]
pub fn record_chat_message_persistence_failure(
    stage: String,
    code: String,
    detail_code: Option<String>,
    metrics: ChatMessagePersistenceMetrics,
) -> Result<(), ChatStorageError> {
    let stage = match stage.as_str() {
        "userMessage" => "user_message",
        "assistantCheckpoint" => "assistant_checkpoint",
        "assistantFinal" => "assistant_final",
        "assistantRecovery" => "assistant_recovery",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    if !valid_diagnostic_code(&code)
        || detail_code
            .as_deref()
            .is_some_and(|detail| !valid_diagnostic_code(detail))
    {
        return Err(ChatStorageError::invalid_input());
    }
    if [
        metrics.content_bytes,
        metrics.citation_source_bytes,
        metrics.personal_memory_fact_id_bytes,
        metrics.reasoning_summary_bytes,
        metrics.tool_activity_bytes,
        metrics.attachment_bytes,
    ]
    .into_iter()
    .any(|value| value > MAX_CHAT_PERSISTENCE_DIAGNOSTIC_VALUE)
        || metrics
            .tool_activity_count
            .is_some_and(|count| count > 100_000)
        || metrics.attachment_count > 10
    {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic_with_message_metrics(
        "chat_persistence",
        "message_save_failed",
        "stage",
        stage,
        &code,
        detail_code.as_deref(),
        &metrics,
    )
}

/// Records file-change loading failures without conversation IDs, paths, or file contents.
#[tauri::command]
pub fn record_file_changes_load_failure(
    stage: String,
    code: String,
    detail_code: Option<String>,
) -> Result<(), ChatStorageError> {
    let stage = match stage.as_str() {
        "inline" => "inline",
        "panel" => "panel",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    if !valid_diagnostic_code(&code)
        || detail_code
            .as_deref()
            .is_some_and(|detail| !valid_diagnostic_code(detail))
    {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic(
        "file_changes",
        "load_failed",
        "stage",
        stage,
        &code,
        detail_code.as_deref(),
    )
}

/// Records a ChatGPT connection-removal stage using fixed labels and a stable error code.
#[tauri::command]
pub fn record_connection_removal_diagnostic(
    stage: String,
    code: Option<String>,
) -> Result<(), ChatStorageError> {
    let stage = match stage.as_str() {
        "request_started" => "request_started",
        "operation_accepted" => "operation_accepted",
        "operation_completed" => "operation_completed",
        "operation_failed" => "operation_failed",
        "reconcile_present" => "reconcile_present",
        "reconcile_absent" => "reconcile_absent",
        "reconcile_check_failed" => "reconcile_check_failed",
        "ui_failed" => "ui_failed",
        "ui_unknown" => "ui_unknown",
        "ui_succeeded" => "ui_succeeded",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    let code = code.unwrap_or_default();
    if !valid_diagnostic_code(&code) {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic("app", "connection_removal", "stage", stage, &code, None)
}

/// Records context-usage failures using a fixed operation label and stable error code.
#[tauri::command]
pub fn record_context_usage_failure(
    operation: String,
    code: String,
) -> Result<(), ChatStorageError> {
    let operation = match operation.as_str() {
        "contextConfiguration" => "context_configuration",
        "fullEstimate" => "full_estimate",
        "memoryInspection" => "memory_inspection",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    if code.is_empty() || !valid_diagnostic_code(&code) {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic(
        "context_usage",
        "request_failed",
        "operation",
        operation,
        &code,
        None,
    )
}

fn valid_diagnostic_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn append_diagnostic(
    component: &str,
    event: &str,
    field_name: &str,
    operation: &str,
    code: &str,
    detail_code: Option<&str>,
) -> Result<(), ChatStorageError> {
    #[cfg(target_os = "linux")]
    let logs_directory = linux_state_home()?.join("openchat").join("logs");
    #[cfg(not(target_os = "linux"))]
    let logs_directory =
        PathBuf::from(env::var_os("LOCALAPPDATA").ok_or_else(ChatStorageError::unavailable)?)
            .join("OpenChat")
            .join("logs");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?
        .as_millis();
    append_diagnostic_at(
        &logs_directory,
        timestamp,
        component,
        event,
        field_name,
        operation,
        code,
        detail_code,
    )
}

fn append_diagnostic_with_message_metrics(
    component: &str,
    event: &str,
    field_name: &str,
    operation: &str,
    code: &str,
    detail_code: Option<&str>,
    metrics: &ChatMessagePersistenceMetrics,
) -> Result<(), ChatStorageError> {
    #[cfg(target_os = "linux")]
    let logs_directory = linux_state_home()?.join("openchat").join("logs");
    #[cfg(not(target_os = "linux"))]
    let logs_directory =
        PathBuf::from(env::var_os("LOCALAPPDATA").ok_or_else(ChatStorageError::unavailable)?)
            .join("OpenChat")
            .join("logs");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?
        .as_millis();
    append_diagnostic_at_with_message_metrics(
        &logs_directory,
        timestamp,
        component,
        event,
        field_name,
        operation,
        code,
        detail_code,
        Some(metrics),
    )
}

#[cfg(target_os = "linux")]
fn linux_state_home() -> Result<PathBuf, ChatStorageError> {
    match env::var_os("XDG_STATE_HOME") {
        Some(path) if Path::new(&path).is_absolute() => Ok(PathBuf::from(path)),
        Some(_) => Err(ChatStorageError::unavailable()),
        None => Ok(
            PathBuf::from(env::var_os("HOME").ok_or_else(ChatStorageError::unavailable)?)
                .join(".local/state"),
        ),
    }
}

fn append_diagnostic_at(
    logs_directory: &Path,
    timestamp: u128,
    component: &str,
    event: &str,
    field_name: &str,
    operation: &str,
    code: &str,
    detail_code: Option<&str>,
) -> Result<(), ChatStorageError> {
    append_diagnostic_at_with_message_metrics(
        logs_directory,
        timestamp,
        component,
        event,
        field_name,
        operation,
        code,
        detail_code,
        None,
    )
}

fn append_diagnostic_at_with_message_metrics(
    logs_directory: &Path,
    timestamp: u128,
    component: &str,
    event: &str,
    field_name: &str,
    operation: &str,
    code: &str,
    detail_code: Option<&str>,
    metrics: Option<&ChatMessagePersistenceMetrics>,
) -> Result<(), ChatStorageError> {
    let _guard = CHAT_STORAGE_DIAGNOSTIC_LOCK
        .lock()
        .map_err(|_| ChatStorageError::unavailable())?;
    fs::create_dir_all(logs_directory).map_err(|_| ChatStorageError::unavailable())?;
    let log_path = logs_directory.join("openchat-app.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|_| ChatStorageError::unavailable())?;
    let code_field = if code.is_empty() {
        String::new()
    } else {
        format!(" code={code}")
    };
    let detail_code_field =
        detail_code.map_or_else(String::new, |value| format!(" detail_code={value}"));
    let metrics_fields = metrics.map_or_else(String::new, |metrics| {
        format!(
            " content_bytes={} citation_source_bytes={} personal_memory_fact_id_bytes={} reasoning_summary_bytes={} tool_activity_bytes={} tool_activity_count={} attachment_count={} attachment_bytes={}",
            metrics.content_bytes,
            metrics.citation_source_bytes,
            metrics.personal_memory_fact_id_bytes,
            metrics.reasoning_summary_bytes,
            metrics.tool_activity_bytes,
            metrics.tool_activity_count.map_or_else(|| "unknown".to_owned(), |count| count.to_string()),
            metrics.attachment_count,
            metrics.attachment_bytes,
        )
    });
    let process_id = std::process::id();
    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let line = format!(
        "timestamp_unix_ms={timestamp} pid={process_id} version={} build_profile={build_profile} component={component} event={event} {field_name}={operation}{code_field}{detail_code_field}{metrics_fields}\n",
        env!("CARGO_PKG_VERSION")
    );
    if file
        .metadata()
        .map_err(|_| ChatStorageError::unavailable())?
        .len()
        .saturating_add(line.len() as u64)
        > MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES
    {
        drop(file);
        return fs::write(log_path, line.as_bytes()).map_err(|_| ChatStorageError::unavailable());
    }
    file.write_all(line.as_bytes())
        .map_err(|_| ChatStorageError::unavailable())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{append_diagnostic_at, valid_diagnostic_code, MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-chat-storage-diagnostic-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create diagnostic test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn valid_diagnostic_code_accepts_alphanumeric_and_underscores() {
        assert!(valid_diagnostic_code("valid_code_123"));
        assert!(valid_diagnostic_code("simple"));
        assert!(!valid_diagnostic_code(""));
        assert!(!valid_diagnostic_code("INVALID_UPPERCASE"));
        assert!(!valid_diagnostic_code("invalid-kebab"));
        assert!(!valid_diagnostic_code(&"a".repeat(97)));
    }

    #[test]
    fn diagnostic_logging_records_safe_entry_and_bounds_size() {
        let directory = TestDirectory::new();
        let log_directory = directory.0.join("logs");
        fs::create_dir_all(&log_directory).expect("create log directory");
        let log_path = log_directory.join("openchat-app.log");
        fs::write(
            &log_path,
            vec![b'x'; MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES as usize],
        )
        .expect("fill diagnostic log");

        append_diagnostic_at(
            &log_directory,
            123,
            "app",
            "chat_request_failed",
            "stage",
            "request_execution",
            "provider_error",
            None,
        )
        .expect("append diagnostic");

        let contents = fs::read_to_string(&log_path).expect("read diagnostic log");
        let build_profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        assert_eq!(
            contents,
            format!(
                "timestamp_unix_ms=123 pid={} version={} build_profile={build_profile} component=app event=chat_request_failed stage=request_execution code=provider_error\n",
                std::process::id(),
                env!("CARGO_PKG_VERSION")
            )
        );
        assert!(
            log_path.metadata().expect("read log size").len() <= MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES
        );
    }
}
