//! Attachment validation, secure filesystem staging, and storage operations.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, OptionalExtension};

use crate::request_bounds::BoundedText;

use super::{
    is_safe_identifier, open_database, set_app_setting_raw, storage_root, unix_time_milliseconds,
    AttachmentTextIndexStatus, ChatStorageError, ConversationAttachmentPage,
    ConversationAttachmentSummary, IncomingMessageAttachment, StoredAttachmentMetadata,
    StoredMessageContentEnvelope, ValidatedAttachment, MAXIMUM_ATTACHMENT_FILE_COUNT,
    MAXIMUM_ATTACHMENT_IMAGE_BYTES, MAXIMUM_ATTACHMENT_IMAGE_COUNT, MAXIMUM_ATTACHMENT_TEXT_BYTES,
    MAXIMUM_ATTACHMENT_TOTAL_BYTES, MAXIMUM_SAFE_IDENTIFIER_BYTES, MESSAGE_CONTENT_PREFIX,
};

const ATTACHMENT_TEXT_INDEX_SETTING: &str = "attachment_text_index.enabled";
const MAXIMUM_ATTACHMENT_LIBRARY_PAGE_SIZE: u32 = 100;
const MAXIMUM_ATTACHMENT_LIBRARY_SCAN: u32 = 10_000;
const ATTACHMENT_LIBRARY_BATCH_SIZE: u32 = 50;
const MAXIMUM_ATTACHMENT_INDEX_FILES: usize = 5_000;
const MAXIMUM_ATTACHMENT_INDEX_BYTES: usize = 100 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES: usize = 50_000;
const INDEXABLE_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "txt", "log", "md", "csv", "json", "jsonl", "xml", "html", "htm", "pdf", "docx", "xlsx", "css",
    "js", "jsx", "ts", "tsx", "dart", "rs", "py", "go", "java", "kt", "swift", "c", "h", "cpp",
    "hpp", "cs", "sh", "ps1", "yaml", "yml", "toml", "ini", "sql",
];

#[tauri::command]
/// Returns one validated attachment's bytes for a message in the specified conversation.
///
/// The lookup is scoped to the conversation/message pair and applies attachment size limits.
///
/// # Errors
/// Returns a validation or storage error when the message or attachment is unavailable.
pub fn read_message_attachment(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    attachment_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<String, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    let attachment_id = attachment_id.into_inner();
    if !is_safe_identifier(&conversation_id)
        || !is_safe_identifier(&message_id)
        || !is_safe_identifier(&attachment_id)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let stored_content = connection
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())?
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = parse_stored_message_content(&stored_content)?;
    let metadata = envelope
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or_else(ChatStorageError::unavailable)?;
    let bytes = read_attachment_bytes(&conversation_id, &message_id, metadata)?
        .ok_or_else(ChatStorageError::unavailable)?;
    Ok(STANDARD.encode(bytes))
}

#[tauri::command]
/// Returns a bounded page of saved attachments and their source conversations.
///
/// Search covers file names, conversation titles, and indexed text when indexing is enabled.
///
/// # Errors
/// Returns an input or storage error when pagination or saved metadata is invalid.
pub fn list_conversation_attachments(
    offset: u32,
    limit: u32,
    query: Option<String>,
    availability: String,
) -> Result<ConversationAttachmentPage, ChatStorageError> {
    if limit == 0
        || limit > MAXIMUM_ATTACHMENT_LIBRARY_PAGE_SIZE
        || offset > 1_000_000
        || !matches!(availability.as_str(), "all" | "available" | "missing")
    {
        return Err(ChatStorageError::invalid_input());
    }
    let query = query.unwrap_or_default().trim().to_lowercase();
    if query.chars().count() > 512 || query.chars().any(char::is_control) {
        return Err(ChatStorageError::invalid_input());
    }

    let index_enabled = attachment_text_index_enabled()?;
    let connection = open_database()?;
    let mut results = Vec::with_capacity(limit as usize + 1);
    let mut cursor = offset;
    let mut scanned = 0_u32;
    let mut found_extra_at = None;
    let mut reached_end = false;

    while results.len() <= limit as usize && scanned < MAXIMUM_ATTACHMENT_LIBRARY_SCAN {
        let batch_limit =
            ATTACHMENT_LIBRARY_BATCH_SIZE.min(MAXIMUM_ATTACHMENT_LIBRARY_SCAN - scanned);
        let batch =
            query_attachment_library_batch(&connection, cursor, batch_limit, index_enabled)?;
        if batch.is_empty() {
            reached_end = true;
            break;
        }
        for (position, candidate) in batch.iter().enumerate() {
            let candidate_offset = cursor + position as u32;
            let Some(attachment) = candidate.summary.as_ref() else {
                continue;
            };
            let available = attachment_path(
                &attachment.conversation_id,
                &attachment.message_id,
                &attachment.attachment_id,
                attachment.size_bytes,
            )?
            .is_some();
            let availability_matches = match availability.as_str() {
                "all" => true,
                "available" => available,
                "missing" => !available,
                _ => false,
            };
            let query_matches = query.is_empty()
                || attachment.file_name.to_lowercase().contains(&query)
                || attachment
                    .conversation_title
                    .to_lowercase()
                    .contains(&query)
                || candidate.indexed_content.to_lowercase().contains(&query);
            if !availability_matches || !query_matches {
                continue;
            }
            if results.len() == limit as usize {
                found_extra_at = Some(candidate_offset);
                break;
            }
            let mut result = attachment.clone();
            result.is_available = available;
            results.push(result);
        }
        if found_extra_at.is_some() {
            break;
        }
        cursor = cursor.saturating_add(batch.len() as u32);
        scanned = scanned.saturating_add(batch.len() as u32);
        if batch.len() < batch_limit as usize {
            reached_end = true;
            break;
        }
    }

    let next_offset = found_extra_at.or({
        if reached_end {
            None
        } else if scanned >= MAXIMUM_ATTACHMENT_LIBRARY_SCAN {
            Some(cursor)
        } else {
            None
        }
    });
    Ok(ConversationAttachmentPage {
        items: results,
        next_offset,
    })
}

#[tauri::command]
/// Returns whether the optional text index is enabled and its bounded storage use.
///
/// # Errors
/// Returns a storage error when index state cannot be read.
pub fn get_attachment_text_index_status() -> Result<AttachmentTextIndexStatus, ChatStorageError> {
    attachment_text_index_status()
}

#[tauri::command]
/// Rebuilds the opt-in text index from validated local conversation attachments.
///
/// The index is capped by file count and total bytes, and only becomes enabled after a
/// complete database replacement succeeds.
///
/// # Errors
/// Returns a storage error if any selected attachment cannot be validated or indexed.
pub fn rebuild_attachment_text_index() -> Result<AttachmentTextIndexStatus, ChatStorageError> {
    let candidates = load_attachment_index_candidates()?;
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
        let Some(bytes) = read_attachment_bytes(&conversation_id, &message_id, &metadata)? else {
            continue;
        };
        if total_bytes.saturating_add(bytes.len()) > MAXIMUM_ATTACHMENT_INDEX_BYTES {
            truncated = true;
            break;
        }
        let content = String::from_utf8(bytes).map_err(|_| ChatStorageError::unavailable())?;
        total_bytes += content.len();
        indexed.push((conversation_id, message_id, metadata, content));
    }

    if indexed.len() == MAXIMUM_ATTACHMENT_INDEX_FILES
        || total_bytes == MAXIMUM_ATTACHMENT_INDEX_BYTES
    {
        truncated = true;
    }

    let now = unix_time_milliseconds()?;
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute("DELETE FROM attachment_text_index", [])
        .map_err(|_| ChatStorageError::unavailable())?;
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO attachment_text_index
                 (conversation_id, message_id, attachment_id, attachment_name, content, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        for (conversation_id, message_id, metadata, content) in &indexed {
            statement
                .execute(params![
                    conversation_id,
                    message_id,
                    metadata.id,
                    metadata.name,
                    content,
                    now
                ])
                .map_err(|_| ChatStorageError::unavailable())?;
        }
    }
    transaction
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES (?1, 'true', ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = 'true', updated_at = excluded.updated_at",
            params![ATTACHMENT_TEXT_INDEX_SETTING, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES ('attachment_text_index.truncated', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![truncated, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;
    attachment_text_index_status()
}

#[tauri::command]
/// Disables attachment indexing before removing its derived data.
///
/// The disabled preference is committed first so a cleanup failure cannot re-enable indexing.
///
/// # Errors
/// Returns a storage error when cleanup fails; the preference remains disabled.
pub fn clear_attachment_text_index() -> Result<AttachmentTextIndexStatus, ChatStorageError> {
    set_app_setting_raw(ATTACHMENT_TEXT_INDEX_SETTING, "false")?;
    set_app_setting_raw("attachment_text_index.truncated", "false")?;
    let connection = open_database()?;
    connection
        .execute("DELETE FROM attachment_text_index", [])
        .map_err(|_| ChatStorageError::unavailable())?;
    attachment_text_index_status()
}

#[derive(Clone)]
struct AttachmentLibraryCandidate {
    summary: Option<ConversationAttachmentSummary>,
    indexed_content: String,
}

fn query_attachment_library_batch(
    connection: &rusqlite::Connection,
    offset: u32,
    limit: u32,
    index_enabled: bool,
) -> Result<Vec<AttachmentLibraryCandidate>, ChatStorageError> {
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.title, c.project_id, m.id, COALESCE(m.created_at, c.updated_at),
                    json_extract(attachment.value, '$.id'),
                    json_extract(attachment.value, '$.name'),
                    json_extract(attachment.value, '$.mimeType'),
                    json_extract(attachment.value, '$.sizeBytes'),
                    json_extract(attachment.value, '$.kind'),
                    CASE WHEN ?2 = 1 THEN COALESCE((
                        SELECT indexed.content FROM attachment_text_index indexed
                        WHERE indexed.conversation_id = m.conversation_id
                          AND indexed.message_id = m.id
                          AND indexed.attachment_id = json_extract(attachment.value, '$.id')
                    ), '') ELSE '' END
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             CROSS JOIN json_each(
                CASE WHEN json_valid(substr(m.content, length(?1) + 1))
                     THEN json_extract(substr(m.content, length(?1) + 1), '$.attachments')
                     ELSE '[]' END
             ) AS attachment
             WHERE substr(m.content, 1, length(?1)) = ?1
             ORDER BY COALESCE(m.created_at, 0) DESC, m.conversation_id ASC, m.id ASC,
                      json_extract(attachment.value, '$.id') ASC
             LIMIT ?3 OFFSET ?4",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map(
            params![MESSAGE_CONTENT_PREFIX, index_enabled, limit, offset],
            |row| {
                let summary = (|| -> rusqlite::Result<Option<ConversationAttachmentSummary>> {
                    let Some(conversation_id) = row.get::<_, Option<String>>(0)? else {
                        return Ok(None);
                    };
                    let Some(conversation_title) = row.get::<_, Option<String>>(1)? else {
                        return Ok(None);
                    };
                    let Some(message_id) = row.get::<_, Option<String>>(3)? else {
                        return Ok(None);
                    };
                    let Some(attachment_id) = row.get::<_, Option<String>>(5)? else {
                        return Ok(None);
                    };
                    let Some(file_name) = row.get::<_, Option<String>>(6)? else {
                        return Ok(None);
                    };
                    let Some(mime_type) = row.get::<_, Option<String>>(7)? else {
                        return Ok(None);
                    };
                    let Some(size_bytes) = row.get::<_, Option<i64>>(8)? else {
                        return Ok(None);
                    };
                    let Some(kind) = row.get::<_, Option<String>>(9)? else {
                        return Ok(None);
                    };
                    let Ok(size_bytes) = usize::try_from(size_bytes) else {
                        return Ok(None);
                    };
                    let metadata = StoredAttachmentMetadata {
                        id: attachment_id.clone(),
                        name: file_name.clone(),
                        mime_type: mime_type.clone(),
                        size_bytes,
                        kind,
                    };
                    if validate_attachment_metadata(&metadata).is_err() {
                        return Ok(None);
                    }
                    Ok(Some(ConversationAttachmentSummary {
                        conversation_id,
                        conversation_title,
                        project_id: row.get(2)?,
                        message_id,
                        attachment_id,
                        file_name,
                        mime_type,
                        size_bytes,
                        created_at: row.get(4)?,
                        is_available: false,
                    }))
                })()?;
                Ok(AttachmentLibraryCandidate {
                    summary,
                    indexed_content: row.get(10)?,
                })
            },
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ChatStorageError::unavailable())
}

fn attachment_path(
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    expected_size: usize,
) -> Result<Option<PathBuf>, ChatStorageError> {
    if !is_safe_identifier(conversation_id)
        || !is_safe_identifier(message_id)
        || !is_safe_identifier(attachment_id)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let storage_root = storage_root()?;
    let Some(attachments_root) = checked_attachment_root(&storage_root, false)? else {
        return Ok(None);
    };
    let Some(conversation_directory) =
        checked_attachment_subdirectory(&attachments_root, conversation_id, false)?
    else {
        return Ok(None);
    };
    let Some(message_directory) =
        checked_attachment_subdirectory(&conversation_directory, message_id, false)?
    else {
        return Ok(None);
    };
    let path = message_directory.join(format!("{attachment_id}.data"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != expected_size as u64
    {
        return Ok(None);
    }
    let canonical_path = fs::canonicalize(&path).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_path != path || !canonical_path.starts_with(&message_directory) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_path))
}

fn read_attachment_bytes(
    conversation_id: &str,
    message_id: &str,
    metadata: &StoredAttachmentMetadata,
) -> Result<Option<Vec<u8>>, ChatStorageError> {
    let Some(path) = attachment_path(
        conversation_id,
        message_id,
        &metadata.id,
        metadata.size_bytes,
    )?
    else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|_| ChatStorageError::unavailable())?;
    validate_stored_attachment(metadata, &bytes)?;
    Ok(Some(bytes))
}

struct AttachmentIndexCandidates {
    items: Vec<(String, String, StoredAttachmentMetadata)>,
    truncated: bool,
}

fn load_attachment_index_candidates() -> Result<AttachmentIndexCandidates, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT conversation_id, id, content
             FROM messages
             WHERE substr(content, 1, length(?1)) = ?1
             ORDER BY COALESCE(created_at, 0) DESC, conversation_id ASC, id ASC
             LIMIT ?2",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map(
            params![
                MESSAGE_CONTENT_PREFIX,
                (MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES + 1) as i64
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let messages = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ChatStorageError::unavailable())?;
    let truncated = messages.len() > MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES;
    let mut items = Vec::new();
    for (conversation_id, message_id, content) in messages
        .into_iter()
        .take(MAXIMUM_ATTACHMENT_INDEX_SCAN_MESSAGES)
    {
        let envelope = parse_stored_message_content(&content)?;
        for metadata in envelope.attachments {
            if metadata.kind == "text" && is_indexable_text_attachment(&metadata.name) {
                items.push((conversation_id.clone(), message_id.clone(), metadata));
            }
        }
    }
    Ok(AttachmentIndexCandidates { items, truncated })
}

fn is_indexable_text_attachment(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .is_some_and(|extension| INDEXABLE_ATTACHMENT_EXTENSIONS.contains(&extension.as_str()))
}

fn attachment_text_index_enabled() -> Result<bool, ChatStorageError> {
    match super::get_app_setting_raw(ATTACHMENT_TEXT_INDEX_SETTING)?.as_deref() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(ChatStorageError::unavailable()),
    }
}

fn attachment_text_index_status() -> Result<AttachmentTextIndexStatus, ChatStorageError> {
    let connection = open_database()?;
    let (file_count, indexed_bytes): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(content AS BLOB))), 0)
             FROM attachment_text_index",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let file_count = u32::try_from(file_count).map_err(|_| ChatStorageError::unavailable())?;
    let indexed_bytes =
        u64::try_from(indexed_bytes).map_err(|_| ChatStorageError::unavailable())?;
    let truncated = match super::get_app_setting_raw("attachment_text_index.truncated")?.as_deref()
    {
        None | Some("false") => false,
        Some("true") => true,
        Some(_) => return Err(ChatStorageError::unavailable()),
    };
    Ok(AttachmentTextIndexStatus {
        enabled: attachment_text_index_enabled()?,
        indexed_file_count: file_count,
        indexed_bytes,
        truncated,
    })
}

pub(crate) fn validate_incoming_attachments(
    attachments: &[IncomingMessageAttachment],
) -> Result<Vec<ValidatedAttachment>, ChatStorageError> {
    if attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT {
        return Err(ChatStorageError::invalid_input());
    }
    let mut ids = std::collections::HashSet::with_capacity(attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    let mut validated = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let metadata = StoredAttachmentMetadata {
            id: attachment.id.clone(),
            name: attachment.name.clone(),
            mime_type: attachment.mime_type.clone(),
            size_bytes: attachment.size_bytes,
            kind: attachment.kind.clone(),
        };
        validate_attachment_metadata(&metadata)?;
        if !ids.insert(metadata.id.clone()) {
            return Err(ChatStorageError::invalid_input());
        }
        let (_, _, maximum_bytes) =
            attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
        let maximum_base64_length = maximum_bytes.div_ceil(3).saturating_mul(4);
        if attachment.data_base64.len() > maximum_base64_length {
            return Err(ChatStorageError::invalid_input());
        }
        let bytes = STANDARD
            .decode(&attachment.data_base64)
            .map_err(|_| ChatStorageError::invalid_input())?;
        validate_stored_attachment(&metadata, &bytes)?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(ChatStorageError::invalid_input)?;
        if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES {
            return Err(ChatStorageError::invalid_input());
        }
        if metadata.kind == "image" {
            image_count += 1;
            if image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT {
                return Err(ChatStorageError::invalid_input());
            }
        }
        validated.push(ValidatedAttachment { metadata, bytes });
    }
    Ok(validated)
}

pub(crate) fn validate_attachment_metadata(
    metadata: &StoredAttachmentMetadata,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > 255
        || metadata.name.chars().any(char::is_control)
        || metadata.size_bytes == 0
    {
        return Err(ChatStorageError::invalid_input());
    }
    let (kind, mime_type, maximum_bytes) =
        attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
    if metadata.kind != kind
        || metadata.mime_type != mime_type
        || metadata.size_bytes > maximum_bytes
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

pub(crate) fn validate_stored_attachment(
    metadata: &StoredAttachmentMetadata,
    bytes: &[u8],
) -> Result<(), ChatStorageError> {
    validate_attachment_metadata(metadata)?;
    if bytes.is_empty() || bytes.len() != metadata.size_bytes {
        return Err(ChatStorageError::invalid_input());
    }
    let valid_content = match metadata.kind.as_str() {
        "image" => has_valid_image_signature(&metadata.mime_type, bytes),
        "text" => std::str::from_utf8(bytes).is_ok(),
        _ => false,
    };
    if !valid_content {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

pub(crate) fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "gif" => ("image", "image/gif", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "html" | "htm" => ("text", "text/html", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "pdf" => ("text", "application/pdf", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "docx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            MAXIMUM_ATTACHMENT_TEXT_BYTES,
        ),
        "xlsx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            MAXIMUM_ATTACHMENT_TEXT_BYTES,
        ),
        "css" => ("text", "text/css", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "js" | "jsx" => ("text", "text/javascript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "ts" | "tsx" => ("text", "text/typescript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "dart" | "rs" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h" | "cpp" | "hpp"
        | "cs" | "sh" | "ps1" | "yaml" | "yml" | "toml" | "ini" | "sql" => {
            ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES)
        }
        _ => return None,
    };
    Some(spec)
}

pub(crate) fn has_valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "image/gif" => {
            bytes.len() >= 6
                && &bytes[..4] == b"GIF8"
                && (bytes[4] == b'7' || bytes[4] == b'9')
                && bytes[5] == b'a'
        }
        _ => false,
    }
}

pub(crate) fn parse_stored_message_content(
    stored_content: &str,
) -> Result<StoredMessageContentEnvelope, ChatStorageError> {
    let encoded = stored_content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = serde_json::from_str::<StoredMessageContentEnvelope>(encoded)
        .map_err(|_| ChatStorageError::unavailable())?;
    if envelope.attachments.is_empty() || envelope.attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    let mut ids = std::collections::HashSet::with_capacity(envelope.attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    for attachment in &envelope.attachments {
        validate_attachment_metadata(attachment).map_err(|_| ChatStorageError::unavailable())?;
        if !ids.insert(attachment.id.as_str()) {
            return Err(ChatStorageError::unavailable());
        }
        total_bytes = total_bytes.saturating_add(attachment.size_bytes);
        if attachment.kind == "image" {
            image_count += 1;
        }
    }
    if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES || image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    Ok(envelope)
}

pub(crate) fn stage_message_attachments(
    conversation_id: &str,
    message_id: &str,
    attachments: &[ValidatedAttachment],
) -> Result<(PathBuf, PathBuf), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let root = storage_root()?;
    let canonical_root =
        checked_attachment_root(&root, true)?.ok_or_else(ChatStorageError::unavailable)?;
    let canonical_conversation =
        checked_attachment_subdirectory(&canonical_root, conversation_id, true)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let destination = canonical_conversation.join(message_id);
    if destination.exists() {
        return Err(ChatStorageError::unavailable());
    }
    let staging =
        canonical_conversation.join(format!(".stage-{message_id}-{}", unix_time_milliseconds()?));
    fs::create_dir(&staging).map_err(|_| ChatStorageError::unavailable())?;
    for attachment in attachments {
        let path = staging.join(format!("{}.data", attachment.metadata.id));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(&attachment.bytes)?;
            file.sync_all()
        })();
        if write_result.is_err() {
            cleanup_attachment_directory(&staging)?;
            return Err(ChatStorageError::unavailable());
        }
    }
    Ok((staging, destination))
}

/// Resolves the attachment root only when it is a real directory under application storage.
///
/// When `create` is true, the directory is created if absent; an absent directory otherwise
/// remains an explicit `None` result so deletion can be idempotent.
pub(crate) fn checked_attachment_root(
    storage_root: &Path,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| ChatStorageError::unavailable())?;
    let expected_root = canonical_storage_root.join("attachments");
    if create {
        match fs::create_dir(&expected_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_root =
        fs::canonicalize(&expected_root).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_root != expected_root || !canonical_root.starts_with(&canonical_storage_root) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_root))
}

/// Resolves one identifier-named attachment directory without following path redirections.
///
/// When `create` is true, a missing directory is created beneath the already checked parent.
pub(crate) fn checked_attachment_subdirectory(
    parent: &Path,
    identifier: &str,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    if !is_safe_identifier(identifier) {
        return Err(ChatStorageError::invalid_input());
    }
    let expected_directory = parent.join(identifier);
    if create {
        match fs::create_dir(&expected_directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&expected_directory).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_directory != expected_directory || !canonical_directory.starts_with(parent) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_directory))
}

pub(crate) fn cleanup_attachment_directory(path: &std::path::Path) -> Result<(), ChatStorageError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ChatStorageError::unavailable()),
    }
}

pub(crate) fn delete_conversation_attachments(
    conversation_id: &str,
) -> Result<(), ChatStorageError> {
    let storage_root = storage_root()?;
    let Some(attachments_root) = checked_attachment_root(&storage_root, false)? else {
        return Ok(());
    };
    let Some(canonical_directory) =
        checked_attachment_subdirectory(&attachments_root, conversation_id, false)?
    else {
        return Ok(());
    };
    cleanup_attachment_directory(&canonical_directory)
}

#[cfg(test)]
mod attachment_path_tests {
    use std::{fs, path::PathBuf};

    use super::{checked_attachment_root, checked_attachment_subdirectory};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-attachment-paths-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated attachment path directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::windows::fs::symlink_dir(source, destination)
            .expect("create isolated directory symlink");
    }

    #[cfg(unix)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::unix::fs::symlink(source, destination).expect("create isolated directory symlink");
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_root_rejects_a_symlink_to_an_external_directory() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        let external_directory = directory.0.join("external");
        fs::create_dir(&storage_root).expect("create storage root");
        fs::create_dir(&external_directory).expect("create external directory");
        fs::write(external_directory.join("keep.txt"), b"preserve")
            .expect("write external sentinel");
        create_directory_symlink(&external_directory, &storage_root.join("attachments"));

        assert!(checked_attachment_root(&storage_root, true).is_err());
        assert_eq!(
            fs::read(external_directory.join("keep.txt")).expect("read external sentinel"),
            b"preserve"
        );
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_subdirectory_rejects_a_symlink_even_when_it_stays_inside_the_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");
        let attachments_root = checked_attachment_root(&storage_root, true)
            .expect("create attachment root")
            .expect("attachment root exists");
        let legitimate_directory = attachments_root.join("conversation-legitimate");
        fs::create_dir(&legitimate_directory).expect("create legitimate conversation directory");
        fs::write(legitimate_directory.join("keep.txt"), b"preserve")
            .expect("write legitimate sentinel");
        create_directory_symlink(
            &legitimate_directory,
            &attachments_root.join("conversation-symlink"),
        );

        assert!(
            checked_attachment_subdirectory(&attachments_root, "conversation-symlink", false)
                .is_err()
        );
        assert_eq!(
            fs::read(legitimate_directory.join("keep.txt")).expect("read legitimate sentinel"),
            b"preserve"
        );
    }

    #[test]
    fn attachment_root_is_created_only_inside_the_application_storage_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");

        let attachment_root = checked_attachment_root(&storage_root, true)
            .expect("create checked attachment root")
            .expect("created attachment root");

        assert_eq!(
            attachment_root,
            fs::canonicalize(storage_root.join("attachments"))
                .expect("canonicalize created attachment root")
        );
        assert!(attachment_root.is_dir());
    }
}
