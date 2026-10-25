//! RPC handlers for the optional local attachment text index.
//!
//! The renderer reaches this module through the local service so index maintenance uses the
//! same queued database boundary as the rest of durable conversation storage.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection, sqlite_error_code},
};

const ATTACHMENT_TEXT_INDEX_SETTING: &str = "attachment_text_index.enabled";
const ATTACHMENT_TEXT_INDEX_TRUNCATED_SETTING: &str = "attachment_text_index.truncated";
const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_ATTACHMENT_COUNT: usize = 10;
const MAX_IMAGE_COUNT: usize = 3;
const MAX_ATTACHMENT_BYTES: usize = 14 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_ATTACHMENT_NAME_CHARS: usize = 255;
const MAXIMUM_ATTACHMENT_INDEX_FILES: usize = 5_000;
const MAXIMUM_ATTACHMENT_INDEX_BYTES: usize = 100 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES: usize = 50_000;
const INDEXABLE_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "txt", "log", "md", "csv", "json", "jsonl", "xml", "html", "htm", "pdf", "docx", "xlsx", "css",
    "js", "jsx", "ts", "tsx", "dart", "rs", "py", "go", "java", "kt", "swift", "c", "h", "cpp",
    "hpp", "cs", "sh", "ps1", "yaml", "yml", "toml", "ini", "sql",
];

/// Dispatches optional attachment text-index operations.
pub(super) async fn dispatch(method: &str, storage: &AppStorage) -> Result<Value, ServiceError> {
    match method {
        "storage.chat.attachments.text_index.status" => get_status(storage),
        "storage.chat.attachments.text_index.rebuild" => rebuild(storage),
        "storage.chat.attachments.text_index.clear" => clear(storage),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn get_status(storage: &AppStorage) -> Result<Value, ServiceError> {
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
    Ok(json!({
        "enabled": enabled,
        "indexedFileCount": indexed_file_count,
        "indexedBytes": indexed_bytes,
        "truncated": truncated,
    }))
}

fn rebuild(storage: &AppStorage) -> Result<Value, ServiceError> {
    let candidates = load_candidates(storage)?;
    let mut indexed = Vec::new();
    let mut total_bytes = 0_usize;
    let mut truncated = candidates.truncated;

    for (conversation_id, message_id, metadata) in candidates.items {
        if !is_indexable_text_attachment(&metadata.name) {
            continue;
        }
        if indexed.len() >= MAXIMUM_ATTACHMENT_INDEX_FILES {
            truncated = true;
            break;
        }
        let Some(bytes) =
            read_attachment_bytes(storage.root(), &conversation_id, &message_id, &metadata)?
        else {
            continue;
        };
        if total_bytes.saturating_add(bytes.len()) > MAXIMUM_ATTACHMENT_INDEX_BYTES {
            truncated = true;
            break;
        }
        let content = String::from_utf8(bytes).map_err(|_| storage_unavailable())?;
        total_bytes += content.len();
        indexed.push((conversation_id, message_id, metadata, content));
    }

    if indexed.len() == MAXIMUM_ATTACHMENT_INDEX_FILES
        || total_bytes == MAXIMUM_ATTACHMENT_INDEX_BYTES
    {
        truncated = true;
    }

    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
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
        for (conversation_id, message_id, metadata, content) in &indexed {
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
        }
    }
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
    transaction.commit().map_err(storage_error)?;
    drop(connection);
    get_status(storage)
}

fn clear(storage: &AppStorage) -> Result<Value, ServiceError> {
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
    get_status(storage)
}

fn load_candidates(storage: &AppStorage) -> Result<AttachmentIndexCandidates, ServiceError> {
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
    let rows = statement
        .query_map(
            params![
                MESSAGE_CONTENT_PREFIX,
                (MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES + 1) as i64,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(storage_error)?;
    let messages = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)?;
    let truncated = messages.len() > MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES;
    let mut items = Vec::new();
    for (conversation_id, message_id, content) in messages
        .into_iter()
        .take(MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES)
    {
        let attachments = parse_attachment_envelope(&content)?;
        for metadata in attachments {
            if metadata.kind == "text" && is_indexable_text_attachment(&metadata.name) {
                items.push((conversation_id.clone(), message_id.clone(), metadata));
            }
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
        || metadata.name.chars().count() > MAX_ATTACHMENT_NAME_CHARS
        || metadata.name.chars().any(char::is_control)
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

#[derive(Clone)]
struct AttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
}
