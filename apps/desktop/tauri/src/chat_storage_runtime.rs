//! Native conversation export and bounded chat diagnostics.

use serde::Serialize;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "chat_storage/markdown_export.rs"]
mod markdown_export;
#[path = "chat_storage/migrations.rs"]
mod migrations;

pub use markdown_export::*;
pub use migrations::*;

const MAX_CHAT_STORAGE_DIAGNOSTIC_BYTES: u64 = 512 * 1024;
const MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES: usize = 255;
const MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES: usize = 128;
const MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES: usize = 20 * 1024 * 1024;
static CHAT_STORAGE_DIAGNOSTIC_LOCK: Mutex<()> = Mutex::new(());

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
pub fn record_chat_request_failure(stage: String, code: String) -> Result<(), ChatStorageError> {
    let stage = match stage.as_str() {
        "conversationCreation" => "conversation_creation",
        "userMessagePersistence" => "user_message_persistence",
        "requestExecution" => "request_execution",
        _ => return Err(ChatStorageError::invalid_input()),
    };
    if code.is_empty() || !valid_diagnostic_code(&code) {
        return Err(ChatStorageError::invalid_input());
    }
    append_diagnostic("app", "chat_request_failed", "stage", stage, &code)
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
    append_diagnostic("app", "connection_removal", "stage", stage, &code)
}

/// Records context-usage failures using a fixed operation label and stable error code.
#[tauri::command]
pub fn record_context_usage_failure(
    operation: String,
    code: String,
) -> Result<(), ChatStorageError> {
    let operation = match operation.as_str() {
        "contextConfiguration" => "context_configuration",
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
    )
}

fn valid_diagnostic_code(value: &str) -> bool {
    value.len() <= 96
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
) -> Result<(), ChatStorageError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(ChatStorageError::unavailable)?;
    let logs_directory = PathBuf::from(local_app_data).join("OpenChat").join("logs");
    let _guard = CHAT_STORAGE_DIAGNOSTIC_LOCK
        .lock()
        .map_err(|_| ChatStorageError::unavailable())?;
    fs::create_dir_all(&logs_directory).map_err(|_| ChatStorageError::unavailable())?;
    let log_path = logs_directory.join("openchat-app.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|_| ChatStorageError::unavailable())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?
        .as_millis();
    let code_field = if code.is_empty() {
        String::new()
    } else {
        format!(" code={code}")
    };
    let line = format!(
        "timestamp_unix_ms={timestamp} component={component} event={event} {field_name}={operation}{code_field}\n"
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
