//! RPC handlers for the optional local attachment text index.
//!
//! The renderer reaches this module through the local service so index maintenance uses the
//! same queued database boundary as the rest of durable conversation storage.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex as StdMutex, OnceLock, Weak},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    conversation_limits::{
        MAXIMUM_ATTACHMENT_FILE_COUNT as MAX_ATTACHMENT_COUNT,
        MAXIMUM_ATTACHMENT_IMAGE_BYTES as MAX_IMAGE_BYTES,
        MAXIMUM_ATTACHMENT_IMAGE_COUNT as MAX_IMAGE_COUNT, MAXIMUM_ATTACHMENT_KIND_BYTES,
        MAXIMUM_ATTACHMENT_NAME_CHARS, MAXIMUM_ATTACHMENT_TEXT_BYTES as MAX_TEXT_BYTES,
        MAXIMUM_ATTACHMENT_TOTAL_BYTES as MAX_ATTACHMENT_BYTES, MESSAGE_CONTENT_PREFIX,
    },
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection, sqlite_error_code},
};

const ATTACHMENT_TEXT_INDEX_SETTING: &str = "attachment_text_index.enabled";
const ATTACHMENT_TEXT_INDEX_TRUNCATED_SETTING: &str = "attachment_text_index.truncated";
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAXIMUM_ATTACHMENT_INDEX_FILES: usize = 5_000;
const MAXIMUM_ATTACHMENT_INDEX_BYTES: usize = 100 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES: usize = 50_000;
const INDEXABLE_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "txt", "log", "md", "csv", "json", "jsonl", "xml", "html", "htm", "pdf", "docx", "xlsx", "css",
    "js", "jsx", "ts", "tsx", "dart", "rs", "py", "go", "java", "kt", "swift", "c", "h", "cpp",
    "hpp", "cs", "sh", "ps1", "yaml", "yml", "toml", "ini", "sql",
];

/// Dispatches optional attachment text-index operations.
pub(super) fn dispatch(
    method: &str,
    storage: &AppStorage,
    cancellation: &watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "storage.chat.attachments.text_index.status" => get_status(storage),
        "storage.chat.attachments.text_index.rebuild" => rebuild(storage, cancellation),
        "storage.chat.attachments.text_index.clear" => clear(storage),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn get_status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let operation = operation_snapshot(storage)?;
    let persisted = if operation.in_progress {
        operation
            .persisted
            .clone()
            .ok_or_else(storage_unavailable)?
    } else {
        read_persisted_status(storage)?
    };
    Ok(json!({
        "enabled": persisted.enabled,
        "indexedFileCount": persisted.indexed_file_count,
        "indexedBytes": persisted.indexed_bytes,
        "truncated": persisted.truncated,
        "operationInProgress": operation.in_progress,
        "progress": operation.progress.map(progress_value),
    }))
}

fn read_persisted_status(
    storage: &AppStorage,
) -> Result<AttachmentIndexPersistedStatus, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let (indexed_file_count, indexed_bytes): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(content AS BLOB))), 0)
             FROM attachment_text_index",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(storage_error)?;
    let indexed_file_count =
        u32::try_from(indexed_file_count).map_err(|_| storage_unavailable())?;
    let indexed_bytes = u64::try_from(indexed_bytes).map_err(|_| storage_unavailable())?;
    let enabled = read_boolean_setting(&connection, ATTACHMENT_TEXT_INDEX_SETTING)?;
    let truncated = read_boolean_setting(&connection, ATTACHMENT_TEXT_INDEX_TRUNCATED_SETTING)?;
    Ok(AttachmentIndexPersistedStatus {
        enabled,
        indexed_file_count,
        indexed_bytes,
        truncated,
    })
}

fn rebuild(
    storage: &AppStorage,
    cancellation: &watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    check_cancelled(cancellation)?;
    let lease = begin_operation(storage)?;
    let candidates = load_candidates(storage, cancellation, &lease)?;
    let mut indexed = Vec::new();
    let mut indexed_bytes = 0_usize;
    let mut truncated = candidates.truncated;
    let total_candidates = candidates.items.len();
    for (index, (conversation_id, message_id, metadata)) in candidates.items.into_iter().enumerate()
    {
        check_cancelled(cancellation)?;
        if indexed.len() >= MAXIMUM_ATTACHMENT_INDEX_FILES {
            truncated = true;
            break;
        }
        let Some(bytes) =
            read_attachment_bytes(storage.root(), &conversation_id, &message_id, &metadata)?
        else {
            lease.set_progress(AttachmentIndexProgress::Read {
                completed_files: index + 1,
                total_files: total_candidates,
                indexed_files: indexed.len(),
                indexed_bytes,
            })?;
            continue;
        };
        if indexed_bytes.saturating_add(bytes.len()) > MAXIMUM_ATTACHMENT_INDEX_BYTES {
            truncated = true;
            break;
        }
        let content = String::from_utf8(bytes).map_err(|_| storage_unavailable())?;
        indexed_bytes += content.len();
        indexed.push((conversation_id, message_id, metadata, content));
        lease.set_progress(AttachmentIndexProgress::Read {
            completed_files: index + 1,
            total_files: total_candidates,
            indexed_files: indexed.len(),
            indexed_bytes,
        })?;
    }
    if indexed.len() == MAXIMUM_ATTACHMENT_INDEX_FILES
        || indexed_bytes == MAXIMUM_ATTACHMENT_INDEX_BYTES
    {
        truncated = true;
    }
    check_cancelled(cancellation)?;

    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    lease.set_progress(AttachmentIndexProgress::Write {
        completed_files: 0,
        total_files: indexed.len(),
    })?;
    check_cancelled(cancellation)?;
    transaction
        .execute("DELETE FROM attachment_text_index", [])
        .map_err(storage_error)?;
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO attachment_text_index
                 (conversation_id, message_id, attachment_id, attachment_name, content, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(storage_error)?;
        for (index, (conversation_id, message_id, metadata, content)) in indexed.iter().enumerate()
        {
            check_cancelled(cancellation)?;
            statement
                .execute(params![
                    conversation_id,
                    message_id,
                    metadata.id,
                    metadata.name,
                    content,
                    now,
                ])
                .map_err(storage_error)?;
            lease.set_progress(AttachmentIndexProgress::Write {
                completed_files: index + 1,
                total_files: indexed.len(),
            })?;
        }
    }
    check_cancelled(cancellation)?;
    upsert_boolean_setting(
        |sql, key, value, updated_at| {
            transaction.execute(
                sql,
                params![key, if value { "true" } else { "false" }, updated_at],
            )
        },
        ATTACHMENT_TEXT_INDEX_SETTING,
        true,
        now,
    )?;
    upsert_boolean_setting(
        |sql, key, value, updated_at| {
            transaction.execute(
                sql,
                params![key, if value { "true" } else { "false" }, updated_at],
            )
        },
        ATTACHMENT_TEXT_INDEX_TRUNCATED_SETTING,
        truncated,
        now,
    )?;
    check_cancelled(cancellation)?;
    transaction.commit().map_err(storage_error)?;
    lease.finish();
    drop(connection);
    get_status(storage)
}

fn clear(storage: &AppStorage) -> Result<Value, ServiceError> {
    let lease = begin_operation(storage)?;
    // Commit the disabled state before cleanup so a cleanup failure cannot leave an enabled,
    // partially rebuilt index visible to attachment searches.
    let now = unix_time_millis()?;
    {
        let mut connection = storage.connect().map_err(storage_error)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        upsert_boolean_setting(
            |sql, key, value, updated_at| {
                transaction.execute(
                    sql,
                    params![key, if value { "true" } else { "false" }, updated_at],
                )
            },
            ATTACHMENT_TEXT_INDEX_SETTING,
            false,
            now,
        )?;
        upsert_boolean_setting(
            |sql, key, value, updated_at| {
                transaction.execute(
                    sql,
                    params![key, if value { "true" } else { "false" }, updated_at],
                )
            },
            ATTACHMENT_TEXT_INDEX_TRUNCATED_SETTING,
            false,
            now,
        )?;
        transaction.commit().map_err(storage_error)?;
    }
    {
        let connection = storage.connect().map_err(storage_error)?;
        connection
            .execute("DELETE FROM attachment_text_index", [])
            .map_err(storage_error)?;
    }
    lease.finish();
    get_status(storage)
}

fn load_candidates(
    storage: &AppStorage,
    cancellation: &watch::Receiver<bool>,
    lease: &AttachmentIndexOperationLease,
) -> Result<AttachmentIndexCandidates, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT conversation_id, id, content
             FROM messages
             WHERE substr(content, 1, length(?1)) = ?1
             ORDER BY COALESCE(created_at, 0) DESC, conversation_id ASC, id ASC
             LIMIT ?2",
        )
        .map_err(storage_error)?;
    let mut rows = statement
        .query(params![
            MESSAGE_CONTENT_PREFIX,
            (MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES + 1) as i64,
        ])
        .map_err(storage_error)?;
    let mut items = Vec::new();
    let mut messages_scanned = 0_usize;
    let mut truncated = false;
    loop {
        check_cancelled(cancellation)?;
        let Some(row) = rows.next().map_err(storage_error)? else {
            break;
        };
        if messages_scanned == MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES {
            truncated = true;
            break;
        }
        let conversation_id = row.get::<_, String>(0).map_err(storage_error)?;
        let message_id = row.get::<_, String>(1).map_err(storage_error)?;
        let content = row.get::<_, String>(2).map_err(storage_error)?;
        messages_scanned += 1;
        let attachments = parse_attachment_envelope(&content)?;
        for metadata in attachments {
            if metadata.kind != "text" || !is_indexable_text_attachment(&metadata.name) {
                continue;
            }
            items.push((conversation_id.clone(), message_id.clone(), metadata));
        }
        lease.set_progress(AttachmentIndexProgress::Scan {
            completed_messages: messages_scanned,
            total_messages: MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES,
        })?;
        if truncated {
            break;
        }
    }
    Ok(AttachmentIndexCandidates { items, truncated })
}

fn parse_attachment_envelope(content: &str) -> Result<Vec<AttachmentMetadata>, ServiceError> {
    let encoded = content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(storage_unavailable)?;
    let object = serde_json::from_str::<Value>(encoded)
        .map_err(|_| storage_unavailable())?
        .as_object()
        .cloned()
        .ok_or_else(storage_unavailable)?;
    if object.get("content").and_then(Value::as_str).is_none() {
        return Err(storage_unavailable());
    }
    let items = object
        .get("attachments")
        .and_then(Value::as_array)
        .ok_or_else(storage_unavailable)?;
    if items.is_empty() || items.len() > MAX_ATTACHMENT_COUNT {
        return Err(storage_unavailable());
    }
    let mut ids = HashSet::with_capacity(items.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    let mut attachments = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(storage_unavailable)?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| is_safe_identifier(id))
            .ok_or_else(storage_unavailable)?
            .to_owned();
        if !ids.insert(id.clone()) {
            return Err(storage_unavailable());
        }
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(storage_unavailable)?
            .to_owned();
        let mime_type = object
            .get("mimeType")
            .and_then(Value::as_str)
            .ok_or_else(storage_unavailable)?
            .to_owned();
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(storage_unavailable)?
            .to_owned();
        let size_bytes = object
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size > 0)
            .ok_or_else(storage_unavailable)?;
        let metadata = AttachmentMetadata {
            id,
            name,
            mime_type,
            size_bytes,
            kind,
        };
        validate_attachment_metadata(&metadata)?;
        total_bytes = total_bytes
            .checked_add(metadata.size_bytes)
            .ok_or_else(storage_unavailable)?;
        if total_bytes > MAX_ATTACHMENT_BYTES {
            return Err(storage_unavailable());
        }
        if metadata.kind == "image" {
            image_count += 1;
            if image_count > MAX_IMAGE_COUNT {
                return Err(storage_unavailable());
            }
        }
        attachments.push(metadata);
    }
    Ok(attachments)
}

fn read_attachment_bytes(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    metadata: &AttachmentMetadata,
) -> Result<Option<Vec<u8>>, ServiceError> {
    let Some(path) = attachment_path(
        storage_root,
        conversation_id,
        message_id,
        &metadata.id,
        metadata.size_bytes,
    )?
    else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|_| storage_unavailable())?;
    validate_stored_attachment(metadata, &bytes)?;
    Ok(Some(bytes))
}

fn attachment_path(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    expected_size: usize,
) -> Result<Option<PathBuf>, ServiceError> {
    if !is_safe_identifier(conversation_id)
        || !is_safe_identifier(message_id)
        || !is_safe_identifier(attachment_id)
    {
        return Err(invalid_request_params());
    }
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| storage_unavailable())?;
    let Some(attachments_root) = checked_directory(&canonical_storage_root, "attachments")? else {
        return Ok(None);
    };
    let Some(conversation_directory) = checked_directory(&attachments_root, conversation_id)?
    else {
        return Ok(None);
    };
    let Some(message_directory) = checked_directory(&conversation_directory, message_id)? else {
        return Ok(None);
    };
    let path = message_directory.join(format!("{attachment_id}.data"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != expected_size as u64
    {
        return Ok(None);
    }
    let canonical_path = fs::canonicalize(&path).map_err(|_| storage_unavailable())?;
    if canonical_path != path || !canonical_path.starts_with(&message_directory) {
        return Err(storage_unavailable());
    }
    Ok(Some(canonical_path))
}

fn checked_directory(parent: &Path, name: &str) -> Result<Option<PathBuf>, ServiceError> {
    let expected = parent.join(name);
    let metadata = match fs::symlink_metadata(&expected) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical = fs::canonicalize(&expected).map_err(|_| storage_unavailable())?;
    if canonical != expected || !canonical.starts_with(parent) {
        return Err(storage_unavailable());
    }
    Ok(Some(canonical))
}

fn validate_stored_attachment(
    metadata: &AttachmentMetadata,
    bytes: &[u8],
) -> Result<(), ServiceError> {
    validate_attachment_metadata(metadata)?;
    if bytes.is_empty() || bytes.len() != metadata.size_bytes {
        return Err(storage_unavailable());
    }
    if metadata.kind == "text" && std::str::from_utf8(bytes).is_err() {
        return Err(storage_unavailable());
    }
    Ok(())
}

fn validate_attachment_metadata(metadata: &AttachmentMetadata) -> Result<(), ServiceError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > MAXIMUM_ATTACHMENT_NAME_CHARS
        || metadata.name.chars().any(char::is_control)
        || metadata.kind.len() > MAXIMUM_ATTACHMENT_KIND_BYTES
        || metadata.size_bytes == 0
    {
        return Err(storage_unavailable());
    }
    let Some((kind, mime_type, maximum_bytes)) = attachment_spec(&metadata.name) else {
        return Err(storage_unavailable());
    };
    if metadata.kind != kind
        || metadata.mime_type != mime_type
        || metadata.size_bytes > maximum_bytes
    {
        return Err(storage_unavailable());
    }
    Ok(())
}

fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAX_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAX_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAX_IMAGE_BYTES),
        "gif" => ("image", "image/gif", MAX_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAX_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAX_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAX_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAX_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAX_TEXT_BYTES),
        "html" | "htm" => ("text", "text/html", MAX_TEXT_BYTES),
        "pdf" => ("text", "application/pdf", MAX_TEXT_BYTES),
        "docx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            MAX_TEXT_BYTES,
        ),
        "xlsx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            MAX_TEXT_BYTES,
        ),
        "css" => ("text", "text/css", MAX_TEXT_BYTES),
        "js" | "jsx" => ("text", "text/javascript", MAX_TEXT_BYTES),
        "ts" | "tsx" => ("text", "text/typescript", MAX_TEXT_BYTES),
        "dart" | "rs" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h" | "cpp" | "hpp"
        | "cs" | "sh" | "ps1" | "yaml" | "yml" | "toml" | "ini" | "sql" => {
            ("text", "text/plain", MAX_TEXT_BYTES)
        }
        _ => return None,
    };
    Some(spec)
}

fn is_indexable_text_attachment(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .is_some_and(|extension| INDEXABLE_ATTACHMENT_EXTENSIONS.contains(&extension.as_str()))
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn read_boolean_setting(
    connection: &StorageConnection<'_>,
    key: &str,
) -> Result<bool, ServiceError> {
    let value = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    match value.as_deref() {
        None | Some("false") | Some("0") => Ok(false),
        Some("true") | Some("1") => Ok(true),
        Some(_) => Err(storage_unavailable()),
    }
}

fn upsert_boolean_setting<F>(
    execute: F,
    key: &str,
    value: bool,
    updated_at: i64,
) -> Result<(), ServiceError>
where
    F: FnOnce(&str, &str, bool, i64) -> rusqlite::Result<usize>,
{
    execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            key,
            value,
            updated_at,
        )
        .map_err(storage_error)?;
    Ok(())
}

fn unix_time_millis() -> Result<i64, ServiceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_unavailable())?
        .as_millis();
    i64::try_from(millis).map_err(|_| storage_unavailable())
}

fn storage_error(error: rusqlite::Error) -> ServiceError {
    ServiceError::new(
        "storage_chat_unavailable",
        "Local conversation storage is unavailable.",
        true,
    )
    .with_details(sqlite_error_code(&error))
}

fn storage_unavailable() -> ServiceError {
    ServiceError::new(
        "storage_chat_unavailable",
        "Local conversation storage is unavailable.",
        true,
    )
}

fn invalid_request_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The request parameters are invalid.",
        false,
    )
}

struct AttachmentIndexCandidates {
    items: Vec<(String, String, AttachmentMetadata)>,
    truncated: bool,
}

type SharedOperationState = Arc<StdMutex<AttachmentIndexOperationState>>;

#[derive(Clone, Default)]
struct AttachmentIndexOperationState {
    in_progress: bool,
    progress: Option<AttachmentIndexProgress>,
    persisted: Option<AttachmentIndexPersistedStatus>,
}

#[derive(Clone)]
struct AttachmentIndexPersistedStatus {
    enabled: bool,
    indexed_file_count: u32,
    indexed_bytes: u64,
    truncated: bool,
}

#[derive(Clone)]
enum AttachmentIndexProgress {
    Scan {
        completed_messages: usize,
        total_messages: usize,
    },
    Read {
        completed_files: usize,
        total_files: usize,
        indexed_files: usize,
        indexed_bytes: usize,
    },
    Write {
        completed_files: usize,
        total_files: usize,
    },
}

struct AttachmentIndexOperationLease {
    state: SharedOperationState,
    finished: bool,
}

impl AttachmentIndexOperationLease {
    fn set_progress(&self, progress: AttachmentIndexProgress) -> Result<(), ServiceError> {
        let mut state = self.state.lock().map_err(|_| storage_unavailable())?;
        state.progress = Some(progress);
        Ok(())
    }

    fn finish(mut self) {
        self.release();
        self.finished = true;
    }

    fn release(&self) {
        // Operation state is small and process-local, so recover the lock only to clear stale busy
        // state after a panic; normal reads and updates still report poisoning as an error.
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.in_progress = false;
        state.progress = None;
        state.persisted = None;
    }
}

impl Drop for AttachmentIndexOperationLease {
    fn drop(&mut self) {
        if !self.finished {
            self.release();
        }
    }
}

fn operation_state(storage: &AppStorage) -> Result<SharedOperationState, ServiceError> {
    static STATES: OnceLock<
        StdMutex<HashMap<PathBuf, Weak<StdMutex<AttachmentIndexOperationState>>>>,
    > = OnceLock::new();
    let states = STATES.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut states = states.lock().map_err(|_| storage_unavailable())?;
    states.retain(|_, state| state.strong_count() > 0);
    if let Some(state) = states.get(storage.root()).and_then(Weak::upgrade) {
        return Ok(state);
    }
    let state = Arc::new(StdMutex::new(AttachmentIndexOperationState::default()));
    states.insert(storage.root().to_path_buf(), Arc::downgrade(&state));
    Ok(state)
}

fn operation_snapshot(storage: &AppStorage) -> Result<AttachmentIndexOperationState, ServiceError> {
    let state = operation_state(storage)?;
    state
        .lock()
        .map(|state| state.clone())
        .map_err(|_| storage_unavailable())
}

fn begin_operation(storage: &AppStorage) -> Result<AttachmentIndexOperationLease, ServiceError> {
    let state = operation_state(storage)?;
    {
        let snapshot = state.lock().map_err(|_| storage_unavailable())?;
        if snapshot.in_progress {
            return Err(attachment_index_busy());
        }
    }
    let persisted = read_persisted_status(storage)?;
    {
        let mut snapshot = state.lock().map_err(|_| storage_unavailable())?;
        if snapshot.in_progress {
            return Err(attachment_index_busy());
        }
        snapshot.in_progress = true;
        snapshot.progress = None;
        snapshot.persisted = Some(persisted);
    }
    Ok(AttachmentIndexOperationLease {
        state,
        finished: false,
    })
}

fn progress_value(progress: AttachmentIndexProgress) -> Value {
    match progress {
        AttachmentIndexProgress::Scan {
            completed_messages,
            total_messages,
        } => json!({
            "phase": "scan",
            "completedMessages": completed_messages,
            "totalMessages": total_messages,
        }),
        AttachmentIndexProgress::Read {
            completed_files,
            total_files,
            indexed_files,
            indexed_bytes,
        } => json!({
            "phase": "read",
            "completedFiles": completed_files,
            "totalFiles": total_files,
            "indexedFiles": indexed_files,
            "indexedBytes": indexed_bytes,
        }),
        AttachmentIndexProgress::Write {
            completed_files,
            total_files,
        } => json!({
            "phase": "write",
            "completedFiles": completed_files,
            "totalFiles": total_files,
        }),
    }
}

fn attachment_index_busy() -> ServiceError {
    ServiceError::new(
        "attachment_index_busy",
        "An attachment text-index operation is already in progress.",
        true,
    )
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        Err(ServiceError::new(
            "operation_cancelled",
            "The attachment text-index update was cancelled.",
            false,
        ))
    } else {
        Ok(())
    }
}

#[derive(Clone)]
struct AttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{CHAT_SCHEMA_VERSION, storage::AppStorage};

    use super::{
        AttachmentIndexProgress, begin_operation, dispatch, get_status, operation_snapshot,
    };

    #[test]
    fn status_exposes_progress_and_rejects_a_second_index_operation() {
        let (_directory, storage) = create_storage();
        let initial = get_status(&storage).expect("read idle status");
        assert_eq!(initial["operationInProgress"], json!(false));
        assert_eq!(initial["progress"], json!(null));

        let lease = begin_operation(&storage).expect("begin index operation");
        lease
            .set_progress(AttachmentIndexProgress::Scan {
                completed_messages: 12,
                total_messages: 50_000,
            })
            .expect("update operation progress");

        let busy = begin_operation(&storage)
            .err()
            .expect("a second operation must not start");
        assert_eq!(busy.code, "attachment_index_busy");
        let active = get_status(&storage).expect("read active status");
        assert_eq!(active["operationInProgress"], json!(true));
        assert_eq!(active["progress"]["phase"], json!("scan"));
        assert_eq!(active["progress"]["completedMessages"], json!(12));

        lease.finish();
        let complete = get_status(&storage).expect("read completed status");
        assert_eq!(complete["operationInProgress"], json!(false));
        assert_eq!(complete["progress"], json!(null));
    }

    #[test]
    fn cancelled_rebuild_preserves_the_previous_index() {
        let (_directory, storage) = create_storage();
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "INSERT INTO attachment_text_index
                 (conversation_id, message_id, attachment_id, attachment_name, content, indexed_at)
                 VALUES ('conversation', 'message', 'attachment', 'notes.txt', 'previous', 1)",
                [],
            )
            .expect("insert existing index row");
        drop(connection);
        let (_sender, cancellation) = watch::channel(true);

        let error = dispatch(
            "storage.chat.attachments.text_index.rebuild",
            &storage,
            &cancellation,
        )
        .expect_err("a cancelled rebuild must stop");
        assert_eq!(error.code, "operation_cancelled");
        assert!(
            !operation_snapshot(&storage)
                .expect("read operation state")
                .in_progress
        );

        let connection = storage.connect().expect("reconnect test storage");
        let previous_content = connection
            .query_row("SELECT content FROM attachment_text_index", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("read previous index row");
        assert_eq!(previous_content, "previous");
    }

    #[test]
    fn empty_rebuild_commits_enabled_status_and_finishes_the_operation() {
        let (_directory, storage) = create_storage();
        let (_sender, cancellation) = watch::channel(false);

        let result = dispatch(
            "storage.chat.attachments.text_index.rebuild",
            &storage,
            &cancellation,
        )
        .expect("rebuild an empty index");

        assert_eq!(result["enabled"], json!(true));
        assert_eq!(result["indexedFileCount"], json!(0));
        assert_eq!(result["operationInProgress"], json!(false));
        assert_eq!(result["progress"], json!(null));
    }

    fn create_storage() -> (TestDirectory, AppStorage) {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        storage
            .prepare_chat_schema_migration(CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        (directory, storage)
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-attachment-index-test-{}", Uuid::new_v4()));
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
