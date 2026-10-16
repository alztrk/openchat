use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

const CHAT_SCHEMA_VERSION: i64 = 18;
const MINIMUM_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
const MAX_MESSAGE_CHARS: usize = 1_000_000;
const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAXIMUM_ATTACHMENT_FILE_COUNT: usize = 10;
const MAXIMUM_ATTACHMENT_IMAGE_COUNT: usize = 3;
const MAXIMUM_ATTACHMENT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_TEXT_BYTES: usize = 1024 * 1024;
const MAXIMUM_ATTACHMENT_TOTAL_BYTES: usize = 14 * 1024 * 1024;
const MAXIMUM_CONVERSATION_IDS_PER_MUTATION: usize = 500;
const MAXIMUM_CONVERSATION_TAGS: usize = 12;
const MAXIMUM_CONVERSATION_TAG_CHARS: usize = 32;
const MAXIMUM_CONVERSATION_TITLE_CHARS: usize = 512;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStorageError {
    code: &'static str,
    message: &'static str,
}

impl ChatStorageError {
    fn unavailable() -> Self {
        Self {
            code: "storage_unavailable",
            message: "Local conversation storage is unavailable.",
        }
    }

    fn invalid_input() -> Self {
        Self {
            code: "invalid_request_params",
            message: "The conversation data is invalid.",
        }
    }

    fn unsupported_sqlite() -> Self {
        Self {
            code: "sqlite_version_unsupported",
            message: "This SQLite version is too old to safely open local conversation storage.",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConversation {
    id: String,
    title: String,
    title_source: String,
    provider_id: Option<String>,
    model_id: Option<String>,
    connection_id: Option<String>,
    workspace_id: Option<String>,
    api_key_connection_id: Option<String>,
    project_id: Option<String>,
    tags: Vec<String>,
    is_pinned: bool,
    is_archived: bool,
    is_bookmarked: bool,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProject {
    id: String,
    name: String,
    folder_path: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    provider_id: Option<String>,
    model_id: Option<String>,
    citation_sources: String,
    personal_memory_fact_ids: String,
    reasoning_summaries: String,
    tool_activities: String,
    status: String,
    failure_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredModelFavorite {
    provider_id: String,
    model_id: String,
    display_name: String,
    source_connection_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedOutputMessageKey {
    conversation_id: String,
    message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationInput {
    id: String,
    title: String,
    provider_id: String,
    model_id: String,
    connection_id: Option<String>,
    workspace_id: Option<String>,
    api_key_connection_id: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConversationRouteInput {
    conversation_id: String,
    provider_id: String,
    model_id: String,
    connection_id: Option<String>,
    workspace_id: Option<String>,
    api_key_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    id: String,
    name: String,
    folder_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMessageInput {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    provider_id: Option<String>,
    model_id: Option<String>,
    citation_sources: Option<String>,
    personal_memory_fact_ids: Option<String>,
    reasoning_summaries: Option<String>,
    tool_activities: Option<String>,
    status: String,
    failure_code: Option<String>,
    #[serde(default)]
    attachments: Vec<IncomingMessageAttachment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModelFavoriteInput {
    provider_id: String,
    model_id: String,
    display_name: String,
    is_favorite: bool,
    source_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IncomingMessageAttachment {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
    data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredAttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct StoredMessageContentEnvelope {
    #[serde(rename = "content")]
    _content: String,
    attachments: Vec<StoredAttachmentMetadata>,
}

struct ValidatedAttachment {
    metadata: StoredAttachmentMetadata,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDatabase {
    database_path: String,
    schema_version: i64,
}

#[tauri::command]
pub fn prepare_chat_database() -> Result<PreparedDatabase, ChatStorageError> {
    let database_path = database_path()?;
    let parent = database_path
        .parent()
        .ok_or_else(ChatStorageError::unavailable)?;
    fs::create_dir_all(parent).map_err(|_| ChatStorageError::unavailable())?;
    let mut connection = open_supported_connection(&database_path)?;

    let integrity: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if integrity != "ok" {
        return Err(ChatStorageError::unavailable());
    }

    connection
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|_| ChatStorageError::unavailable())?;
    migrate_chat_schema(&mut connection).map_err(|_| ChatStorageError::unavailable())?;

    let integrity: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if integrity != "ok" {
        return Err(ChatStorageError::unavailable());
    }
    Ok(PreparedDatabase {
        database_path: database_path.to_string_lossy().into_owned(),
        schema_version: CHAT_SCHEMA_VERSION,
    })
}

#[tauri::command]
pub fn has_completed_assistant_response() -> Result<bool, ChatStorageError> {
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM messages
                WHERE role = 'assistant' AND status = 'completed'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn list_conversations() -> Result<Vec<StoredConversation>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id, tags,
                    is_pinned, is_archived, is_bookmarked, created_at, updated_at
             FROM conversations
             ORDER BY is_bookmarked DESC, is_pinned DESC, updated_at DESC, id ASC
            ",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], stored_conversation_from_row)
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn list_projects() -> Result<Vec<StoredProject>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, folder_path, created_at, updated_at
             FROM projects ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(StoredProject {
                id: row.get(0)?,
                name: row.get(1)?,
                folder_path: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn create_project(input: CreateProjectInput) -> Result<(), ChatStorageError> {
    let id = input.id.trim();
    let name = input.name.trim();
    let folder_path = input.folder_path.trim();
    if !is_safe_identifier(id)
        || name.is_empty()
        || name.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
        || folder_path.is_empty()
        || folder_path.len() > 4096
        || !std::path::Path::new(folder_path).is_absolute()
    {
        return Err(ChatStorageError::invalid_input());
    }
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
    connection
        .execute(
            "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, name, folder_path, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
pub fn clear_conversation_history() -> Result<(), ChatStorageError> {
    let connection = open_database()?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute_batch(
            "DELETE FROM attachment_text_index;
             DELETE FROM saved_output_versions;
             DELETE FROM saved_outputs;
             DELETE FROM conversations;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;

    let attachment_directory = database_path()?
        .parent()
        .and_then(std::path::Path::parent)
        .map(|root| root.join("attachments"))
        .ok_or_else(ChatStorageError::unavailable)?;
    match fs::symlink_metadata(&attachment_directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(ChatStorageError::unavailable())
        }
        Ok(_) => {
            fs::remove_dir_all(attachment_directory).map_err(|_| ChatStorageError::unavailable())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ChatStorageError::unavailable()),
    }
}

#[tauri::command]
pub fn list_model_favorites() -> Result<Vec<StoredModelFavorite>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT provider_id, model_id, display_name, source_connection_id
             FROM model_favorites
             ORDER BY provider_id ASC, display_name ASC, model_id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let favorites = statement
        .query_map([], |row| {
            Ok(StoredModelFavorite {
                provider_id: row.get(0)?,
                model_id: row.get(1)?,
                display_name: row.get(2)?,
                source_connection_id: row.get(3)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    favorites
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn set_model_favorite(input: SetModelFavoriteInput) -> Result<(), ChatStorageError> {
    validate_model_favorite_input(&input)?;
    let connection = open_database()?;
    if input.is_favorite {
        let favorited_at = unix_time_milliseconds()?;
        connection
            .execute(
                "INSERT INTO model_favorites (
                    provider_id, model_id, display_name, favorited_at, source_connection_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(provider_id, model_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    favorited_at = excluded.favorited_at,
                    source_connection_id = excluded.source_connection_id",
                params![
                    input.provider_id,
                    input.model_id,
                    input.display_name.trim(),
                    favorited_at,
                    input.source_connection_id,
                ],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    } else {
        connection
            .execute(
                "DELETE FROM model_favorites WHERE provider_id = ?1 AND model_id = ?2",
                params![input.provider_id, input.model_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_conversation(
    conversation_id: String,
) -> Result<Option<StoredConversation>, ChatStorageError> {
    if conversation_id.trim().is_empty() || conversation_id.len() > 128 {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id, tags,
                    is_pinned, is_archived, is_bookmarked, created_at, updated_at
             FROM conversations WHERE id = ?1",
            [conversation_id],
            stored_conversation_from_row,
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())
}

fn stored_conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredConversation> {
    let tags_json = row.get::<_, String>(9)?;
    let tags = serde_json::from_str::<Vec<String>>(&tags_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(StoredConversation {
        id: row.get(0)?,
        title: row.get(1)?,
        title_source: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        connection_id: row.get(5)?,
        workspace_id: row.get(6)?,
        api_key_connection_id: row.get(7)?,
        project_id: row.get(8)?,
        tags,
        is_pinned: row.get(10)?,
        is_archived: row.get(11)?,
        is_bookmarked: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[tauri::command]
pub fn rename_conversation(conversation_id: String, title: String) -> Result<(), ChatStorageError> {
    let title = title.trim();
    if !is_safe_identifier(&conversation_id)
        || title.is_empty()
        || title.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
        || title.chars().any(char::is_control)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations SET title = ?1, title_source = 'manual', updated_at = ?2
             WHERE id = ?3",
            params![title, unix_time_milliseconds()?, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
pub fn set_conversation_route(input: SetConversationRouteInput) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&input.conversation_id)
        || !is_safe_identifier(&input.provider_id)
        || !is_valid_route_model_id(&input.model_id)
        || !is_optional_safe_identifier(input.connection_id.as_deref())
        || !is_optional_safe_identifier(input.workspace_id.as_deref())
        || !is_optional_safe_identifier(input.api_key_connection_id.as_deref())
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations
             SET provider_id = ?1, model_id = ?2, connection_id = ?3,
                 workspace_id = ?4, api_key_connection_id = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                input.provider_id,
                input.model_id,
                input.connection_id,
                input.workspace_id,
                input.api_key_connection_id,
                unix_time_milliseconds()?,
                input.conversation_id,
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
pub fn set_conversation_pinned(
    conversation_id: String,
    is_pinned: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(&conversation_id, "is_pinned", is_pinned)
}

#[tauri::command]
pub fn set_conversation_bookmarked(
    conversation_id: String,
    is_bookmarked: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(&conversation_id, "is_bookmarked", is_bookmarked)
}

#[tauri::command]
pub fn list_saved_output_message_keys() -> Result<Vec<SavedOutputMessageKey>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT conversation_id, message_id
             FROM saved_outputs
             ORDER BY conversation_id ASC, message_id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SavedOutputMessageKey {
                conversation_id: row.get(0)?,
                message_id: row.get(1)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn set_saved_output(
    conversation_id: String,
    message_id: String,
    is_saved: bool,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&conversation_id) || !is_safe_identifier(&message_id) {
        return Err(ChatStorageError::invalid_input());
    }

    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;

    if is_saved {
        let now = unix_time_milliseconds()?;
        let inserted_rows = transaction
            .execute(
                "INSERT OR IGNORE INTO saved_outputs (
                    conversation_id, message_id, conversation_title, content,
                    provider_id, model_id, saved_at, selected_version
                 )
                 SELECT c.id, m.id, c.title, m.content, m.provider_id, m.model_id, ?1, 1
                 FROM messages AS m
                 INNER JOIN conversations AS c ON c.id = m.conversation_id
                 WHERE c.id = ?2 AND m.id = ?3
                    AND m.role = 'assistant' AND m.status = 'completed'
                    AND length(trim(m.content)) > 0",
                params![now, conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if inserted_rows == 0 {
            let already_saved = transaction
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM saved_outputs
                        WHERE conversation_id = ?1 AND message_id = ?2
                    )",
                    params![conversation_id, message_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|_| ChatStorageError::unavailable())?;
            if !already_saved {
                return Err(ChatStorageError::invalid_input());
            }
        } else {
            transaction
                .execute(
                    "INSERT INTO saved_output_versions (
                        conversation_id, message_id, version_number, content, created_at
                     )
                     SELECT conversation_id, message_id, 1, content, saved_at
                     FROM saved_outputs
                     WHERE conversation_id = ?1 AND message_id = ?2",
                    params![conversation_id, message_id],
                )
                .map_err(|_| ChatStorageError::unavailable())?;
        }
    } else {
        transaction
            .execute(
                "DELETE FROM saved_output_versions
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        transaction
            .execute(
                "DELETE FROM saved_outputs
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }

    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

fn set_conversation_flag(
    conversation_id: &str,
    column: &str,
    value: bool,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !matches!(column, "is_pinned" | "is_bookmarked") {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            &format!("UPDATE conversations SET {column} = ?1 WHERE id = ?2"),
            params![value, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
pub fn set_conversation_tags(
    conversation_id: String,
    tags: Vec<String>,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&conversation_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > MAXIMUM_CONVERSATION_TAG_CHARS || tag.chars().any(char::is_control)
        {
            return Err(ChatStorageError::invalid_input());
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_owned());
        }
    }
    if normalized.len() > MAXIMUM_CONVERSATION_TAGS {
        return Err(ChatStorageError::invalid_input());
    }
    let tags_json =
        serde_json::to_string(&normalized).map_err(|_| ChatStorageError::invalid_input())?;
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations SET tags = ?1 WHERE id = ?2",
            params![tags_json, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
pub fn move_conversations(
    conversation_ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids)?;
    if project_id
        .as_deref()
        .is_some_and(|id| !is_safe_identifier(id))
    {
        return Err(ChatStorageError::invalid_input());
    }
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    if let Some(project_id) = project_id.as_deref() {
        let project_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if !project_exists {
            return Err(ChatStorageError::invalid_input());
        }
    }
    for conversation_id in ids {
        let updated_rows = transaction
            .execute(
                "UPDATE conversations SET project_id = ?1, is_pinned = 0 WHERE id = ?2",
                params![project_id, conversation_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if updated_rows != 1 {
            return Err(ChatStorageError::invalid_input());
        }
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn set_conversations_archived(
    conversation_ids: Vec<String>,
    is_archived: bool,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids)?;
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    for conversation_id in ids {
        let updated_rows = transaction
            .execute(
                "UPDATE conversations
                 SET is_archived = ?1,
                     is_pinned = CASE WHEN ?1 = 1 THEN 0 ELSE is_pinned END
                 WHERE id = ?2",
                params![is_archived, conversation_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if updated_rows != 1 {
            return Err(ChatStorageError::invalid_input());
        }
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

fn normalize_conversation_ids(ids: Vec<String>) -> Result<Vec<String>, ChatStorageError> {
    if ids.is_empty() || ids.len() > MAXIMUM_CONVERSATION_IDS_PER_MUTATION {
        return Err(ChatStorageError::invalid_input());
    }
    let mut unique = std::collections::HashSet::with_capacity(ids.len());
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        if !is_safe_identifier(&id) {
            return Err(ChatStorageError::invalid_input());
        }
        if unique.insert(id.clone()) {
            normalized.push(id);
        }
    }
    Ok(normalized)
}

#[tauri::command]
pub fn delete_conversation(conversation_id: String) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&conversation_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM attachment_text_index WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM saved_output_versions WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM saved_outputs WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let deleted_rows = transaction
        .execute(
            "DELETE FROM conversations WHERE id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if deleted_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;
    delete_conversation_attachments(&conversation_id)
}

fn delete_conversation_attachments(conversation_id: &str) -> Result<(), ChatStorageError> {
    let attachments_root = storage_root()?.join("attachments");
    let root_metadata = match fs::symlink_metadata(&attachments_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_root =
        fs::canonicalize(&attachments_root).map_err(|_| ChatStorageError::unavailable())?;
    let conversation_directory = attachments_root.join(conversation_id);
    let metadata = match fs::symlink_metadata(&conversation_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&conversation_directory).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_directory.starts_with(canonical_root) {
        return Err(ChatStorageError::unavailable());
    }
    cleanup_attachment_directory(&canonical_directory)
}

#[tauri::command]
pub fn save_conversation_markdown(path: String, content: String) -> Result<(), ChatStorageError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.is_absolute()
        || file_path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .map_or(true, |extension| !extension.eq_ignore_ascii_case("md"))
        || content.is_empty()
        || content.len() > 20 * 1024 * 1024
    {
        return Err(ChatStorageError::invalid_input());
    }
    fs::write(file_path, content).map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn get_messages(conversation_id: String) -> Result<Vec<StoredMessage>, ChatStorageError> {
    if conversation_id.trim().is_empty() || conversation_id.len() > 128 {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, provider_id, model_id,
                    citation_sources, personal_memory_fact_ids, reasoning_summaries,
                    tool_activities, status, failure_code
             FROM messages WHERE conversation_id = ?1
             ORDER BY created_at ASC, id ASC LIMIT 10000",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                output_tokens: row.get(5)?,
                tokens_per_second: row.get(6)?,
                elapsed_microseconds: row.get(7)?,
                provider_id: row.get(8)?,
                model_id: row.get(9)?,
                citation_sources: row.get(10)?,
                personal_memory_fact_ids: row.get(11)?,
                reasoning_summaries: row.get(12)?,
                tool_activities: row.get(13)?,
                status: row.get(14)?,
                failure_code: row.get(15)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
pub fn read_message_attachment(
    conversation_id: String,
    message_id: String,
    attachment_id: String,
) -> Result<String, ChatStorageError> {
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
    let attachments_root = storage_root()?.join("attachments");
    let canonical_root =
        fs::canonicalize(&attachments_root).map_err(|_| ChatStorageError::unavailable())?;
    let message_directory = attachments_root.join(&conversation_id).join(&message_id);
    let canonical_directory =
        fs::canonicalize(&message_directory).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(ChatStorageError::unavailable());
    }
    let attachment_path = canonical_directory.join(format!("{}.data", metadata.id));
    let canonical_attachment =
        fs::canonicalize(&attachment_path).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_attachment.starts_with(&canonical_directory) {
        return Err(ChatStorageError::unavailable());
    }
    let bytes = fs::read(canonical_attachment).map_err(|_| ChatStorageError::unavailable())?;
    validate_stored_attachment(metadata, &bytes)?;
    Ok(STANDARD.encode(bytes))
}

#[tauri::command]
pub fn create_conversation(input: CreateConversationInput) -> Result<(), ChatStorageError> {
    validate_conversation_input(&input)?;
    let connection = open_database()?;
    let now = unix_time_milliseconds()?;
    connection
        .execute(
            "INSERT INTO conversations (
                id, title, title_source, provider_id, model_id, connection_id,
                workspace_id, api_key_connection_id, project_id, created_at, updated_at
             ) VALUES (?1, ?2, 'automatic', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                input.id.trim(),
                input.title.trim(),
                input.provider_id,
                input.model_id,
                input.connection_id,
                input.workspace_id,
                input.api_key_connection_id,
                input.project_id,
                now,
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
pub fn save_message(input: SaveMessageInput) -> Result<(), ChatStorageError> {
    let validated_attachments = validate_incoming_attachments(&input.attachments)?;
    validate_message_input(&input, !validated_attachments.is_empty())?;
    let attachment_metadata = validated_attachments
        .iter()
        .map(|attachment| attachment.metadata.clone())
        .collect::<Vec<_>>();
    let content = if attachment_metadata.is_empty() {
        input.content.clone()
    } else {
        if input.role != "user" {
            return Err(ChatStorageError::invalid_input());
        }
        let envelope = json!({
            "content": input.content,
            "attachments": attachment_metadata,
        });
        format!(
            "{MESSAGE_CONTENT_PREFIX}{}",
            serde_json::to_string(&envelope).map_err(|_| ChatStorageError::invalid_input())?
        )
    };
    if content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let created_at = input.created_at.unwrap_or(unix_time_milliseconds()?);
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    let staged_attachments = if validated_attachments.is_empty() {
        None
    } else {
        Some(stage_message_attachments(
            &input.conversation_id,
            &input.id,
            &validated_attachments,
        )?)
    };
    if transaction
        .execute(
            "INSERT INTO messages (
                id, conversation_id, role, content, created_at, output_tokens,
                tokens_per_second, elapsed_microseconds, provider_id, model_id,
                citation_sources, personal_memory_fact_ids, reasoning_summaries,
                tool_activities, status, failure_code
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16
             ) ON CONFLICT(conversation_id, id) DO UPDATE SET
                role = excluded.role,
                content = excluded.content,
                created_at = excluded.created_at,
                output_tokens = excluded.output_tokens,
                tokens_per_second = excluded.tokens_per_second,
                elapsed_microseconds = excluded.elapsed_microseconds,
                provider_id = excluded.provider_id,
                model_id = excluded.model_id,
                citation_sources = excluded.citation_sources,
                personal_memory_fact_ids = excluded.personal_memory_fact_ids,
                reasoning_summaries = excluded.reasoning_summaries,
                tool_activities = excluded.tool_activities,
                status = excluded.status,
                failure_code = excluded.failure_code",
            params![
                input.id.trim(),
                input.conversation_id.trim(),
                input.role,
                content,
                created_at,
                input.output_tokens,
                input.tokens_per_second,
                input.elapsed_microseconds,
                input.provider_id,
                input.model_id,
                input.citation_sources.unwrap_or_else(|| "[]".to_owned()),
                input
                    .personal_memory_fact_ids
                    .unwrap_or_else(|| "[]".to_owned()),
                input.reasoning_summaries.unwrap_or_else(|| "[]".to_owned()),
                input.tool_activities.unwrap_or_else(|| "[]".to_owned()),
                input.status,
                input.failure_code,
            ],
        )
        .is_err()
    {
        drop(transaction);
        if let Some((staging, _)) = staged_attachments {
            cleanup_attachment_directory(&staging)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    if transaction
        .execute(
            "UPDATE conversations SET updated_at = MAX(updated_at, ?1) WHERE id = ?2",
            params![created_at, input.conversation_id.trim()],
        )
        .is_err()
    {
        drop(transaction);
        if let Some((staging, _)) = staged_attachments {
            cleanup_attachment_directory(&staging)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    let mut published_attachments = None;
    if let Some((staging, destination)) = staged_attachments {
        if fs::rename(&staging, &destination).is_err() {
            drop(transaction);
            cleanup_attachment_directory(&staging)?;
            return Err(ChatStorageError::unavailable());
        }
        published_attachments = Some(destination);
    }
    if transaction.commit().is_err() {
        if let Some(destination) = published_attachments {
            cleanup_attachment_directory(&destination)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    Ok(())
}

fn validate_conversation_input(input: &CreateConversationInput) -> Result<(), ChatStorageError> {
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input.title.trim().is_empty()
        || input.title.chars().count() > 500
        || input.provider_id.trim().is_empty()
        || input.model_id.trim().is_empty()
        || input.model_id.len() > 256
        || input
            .project_id
            .as_deref()
            .is_some_and(|project_id| !is_safe_identifier(project_id))
    {
        return Err(ChatStorageError::invalid_input());
    }

    let provider_id = input.provider_id.as_str();
    let has_oauth_route = input.connection_id.is_some() || input.workspace_id.is_some();
    let has_api_route = input.api_key_connection_id.is_some();
    let api_provider = provider_id == "chatgpt_api"
        || [
            "gemini",
            "groq",
            "cerebras",
            "openrouter",
            "mistral",
            "deepseek",
            "xai",
            "perplexity",
            "together",
            "sambanova",
            "anthropic",
            "fireworks",
            "deepinfra",
            "dashscope",
            "cohere",
        ]
        .contains(&provider_id);
    let local_provider = provider_id == "llama_cpp";
    let custom_provider = is_custom_provider_id(provider_id);
    let known_provider = provider_id == "chatgpt"
        || provider_id == "opencode"
        || local_provider
        || api_provider
        || custom_provider;

    if !known_provider
        || (provider_id == "chatgpt"
            && (has_api_route || input.connection_id.is_none() || input.workspace_id.is_none()))
        || ((provider_id == "opencode" || local_provider) && (has_oauth_route || has_api_route))
        || (api_provider && (has_oauth_route || !has_api_route))
        || (custom_provider
            && (has_oauth_route
                || !has_api_route
                || input.api_key_connection_id.as_deref() != Some(provider_id)))
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_model_favorite_input(input: &SetModelFavoriteInput) -> Result<(), ChatStorageError> {
    const API_KEY_PROVIDERS: [&str; 16] = [
        "chatgpt_api",
        "gemini",
        "groq",
        "cerebras",
        "openrouter",
        "mistral",
        "deepseek",
        "xai",
        "perplexity",
        "together",
        "sambanova",
        "anthropic",
        "fireworks",
        "deepinfra",
        "dashscope",
        "cohere",
    ];
    let api_key_provider = API_KEY_PROVIDERS.contains(&input.provider_id.as_str())
        || is_custom_provider_id(&input.provider_id);
    let known_provider = api_key_provider
        || matches!(
            input.provider_id.as_str(),
            "chatgpt" | "opencode" | "llama_cpp"
        );
    let valid_source_connection = match (api_key_provider, input.source_connection_id.as_deref()) {
        (true, Some(source_connection_id)) => is_safe_identifier(source_connection_id),
        (false, None) => true,
        _ => false,
    };

    if !known_provider
        || input.provider_id.trim() != input.provider_id
        || input.model_id.trim().is_empty()
        || input.model_id.trim() != input.model_id
        || input.model_id.len() > 256
        || input.model_id.chars().any(char::is_control)
        || input.display_name.trim().is_empty()
        || input.display_name.chars().count() > 256
        || input.display_name.chars().any(char::is_control)
        || !valid_source_connection
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_message_input(
    input: &SaveMessageInput,
    has_attachments: bool,
) -> Result<(), ChatStorageError> {
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input.conversation_id.trim().is_empty()
        || input.conversation_id.len() > 128
        || !matches!(input.role.as_str(), "user" | "assistant")
        || !matches!(
            input.status.as_str(),
            "streaming" | "completed" | "stopped" | "failed"
        )
        || input.content.chars().count() > MAX_MESSAGE_CHARS
        || (input.role == "user" && !has_attachments && input.content.trim().is_empty())
        || input.output_tokens.is_some_and(|value| value < 0)
        || input.elapsed_microseconds.is_some_and(|value| value < 0)
        || input
            .tokens_per_second
            .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(ChatStorageError::invalid_input());
    }
    for json in [
        input.citation_sources.as_deref(),
        input.personal_memory_fact_ids.as_deref(),
        input.reasoning_summaries.as_deref(),
        input.tool_activities.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let value = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|_| ChatStorageError::invalid_input())?;
        if !value.is_array() {
            return Err(ChatStorageError::invalid_input());
        }
    }
    Ok(())
}

fn validate_incoming_attachments(
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

fn validate_attachment_metadata(
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

fn validate_stored_attachment(
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

fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "html" => ("text", "text/html", MAXIMUM_ATTACHMENT_TEXT_BYTES),
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

fn has_valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

fn parse_stored_message_content(
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

fn stage_message_attachments(
    conversation_id: &str,
    message_id: &str,
    attachments: &[ValidatedAttachment],
) -> Result<(PathBuf, PathBuf), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let attachments_root = storage_root()?.join("attachments");
    fs::create_dir_all(&attachments_root).map_err(|_| ChatStorageError::unavailable())?;
    let canonical_root =
        fs::canonicalize(&attachments_root).map_err(|_| ChatStorageError::unavailable())?;
    let conversation_directory = attachments_root.join(conversation_id);
    fs::create_dir_all(&conversation_directory).map_err(|_| ChatStorageError::unavailable())?;
    let canonical_conversation =
        fs::canonicalize(&conversation_directory).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_conversation.starts_with(&canonical_root) {
        return Err(ChatStorageError::unavailable());
    }
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

fn cleanup_attachment_directory(path: &std::path::Path) -> Result<(), ChatStorageError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ChatStorageError::unavailable()),
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn is_optional_safe_identifier(value: Option<&str>) -> bool {
    value.is_none_or(is_safe_identifier)
}

fn is_valid_route_model_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 512 && !value.chars().any(char::is_control)
}

fn storage_root() -> Result<PathBuf, ChatStorageError> {
    database_path()?
        .parent()
        .and_then(std::path::Path::parent)
        .map(PathBuf::from)
        .ok_or_else(ChatStorageError::unavailable)
}

fn is_custom_provider_id(provider_id: &str) -> bool {
    let Some(suffix) = provider_id.strip_prefix("custom_") else {
        return false;
    };
    (12..=64).contains(&suffix.len())
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn database_path() -> Result<PathBuf, ChatStorageError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(ChatStorageError::unavailable)?;
    Ok(PathBuf::from(local_app_data)
        .join("OpenChat")
        .join("db")
        .join("openchat.sqlite3"))
}

fn open_database() -> Result<Connection, ChatStorageError> {
    open_supported_connection(&database_path()?)
}

fn open_supported_connection(path: &Path) -> Result<Connection, ChatStorageError> {
    if rusqlite::version_number() < MINIMUM_SQLITE_VERSION_NUMBER {
        return Err(ChatStorageError::unsupported_sqlite());
    }
    let connection = Connection::open(path).map_err(|_| ChatStorageError::unavailable())?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| ChatStorageError::unavailable())?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(connection)
}

fn unix_time_milliseconds() -> Result<i64, ChatStorageError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ChatStorageError::unavailable())
}

fn migrate_chat_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let mut version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CHAT_SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                output_tokens INTEGER,
                tokens_per_second REAL,
                elapsed_microseconds INTEGER,
                status TEXT NOT NULL,
                PRIMARY KEY (conversation_id, id)
             );",
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
        version = 1;
    }

    let transaction = connection.transaction()?;
    if version < 2 {
        add_column(
            &transaction,
            "conversations",
            "title_source",
            "TEXT NOT NULL DEFAULT 'automatic'",
        )?;
        add_column(&transaction, "conversations", "connection_id", "TEXT")?;
        add_column(&transaction, "conversations", "workspace_id", "TEXT")?;
        transaction.pragma_update(None, "user_version", 2)?;
        version = 2;
    }
    if version < 3 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                folder_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );",
        )?;
        add_column(
            &transaction,
            "conversations",
            "project_id",
            "TEXT REFERENCES projects(id) ON DELETE SET NULL",
        )?;
        transaction.pragma_update(None, "user_version", 3)?;
        version = 3;
    }
    if version < 4 {
        add_column(
            &transaction,
            "messages",
            "reasoning_summaries",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 4)?;
        version = 4;
    }
    if version < 5 {
        add_column(&transaction, "conversations", "provider_id", "TEXT")?;
        transaction.pragma_update(None, "user_version", 5)?;
        version = 5;
    }
    if version < 6 {
        create_model_favorites(&transaction)?;
        transaction.pragma_update(None, "user_version", 6)?;
        version = 6;
    }
    if version < 7 {
        add_column(
            &transaction,
            "messages",
            "tool_activities",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 7)?;
        version = 7;
    }
    if version < 8 {
        add_column(
            &transaction,
            "conversations",
            "api_key_connection_id",
            "TEXT",
        )?;
        add_column(
            &transaction,
            "model_favorites",
            "source_connection_id",
            "TEXT",
        )?;
        transaction.pragma_update(None, "user_version", 8)?;
        version = 8;
    }
    if version < 9 {
        transaction.execute_batch(
            "UPDATE conversations SET title_source = 'manual' WHERE title_source = 'user';
             UPDATE messages SET status = 'completed' WHERE status = 'complete';",
        )?;
        transaction.pragma_update(None, "user_version", 9)?;
        version = 9;
    }
    if version < 10 {
        add_column(&transaction, "messages", "failure_code", "TEXT")?;
        transaction.pragma_update(None, "user_version", 10)?;
        version = 10;
    }
    if version < 11 {
        add_column(
            &transaction,
            "conversations",
            "is_archived",
            "INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))",
        )?;
        transaction.pragma_update(None, "user_version", 11)?;
        version = 11;
    }
    if version < 12 {
        add_column(&transaction, "messages", "provider_id", "TEXT")?;
        add_column(&transaction, "messages", "model_id", "TEXT")?;
        add_column(
            &transaction,
            "messages",
            "citation_sources",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 12)?;
        version = 12;
    }
    if version < 13 {
        add_column(
            &transaction,
            "conversations",
            "tags",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 13)?;
        version = 13;
    }
    if version < 14 {
        add_column(
            &transaction,
            "conversations",
            "is_bookmarked",
            "INTEGER NOT NULL DEFAULT 0 CHECK (is_bookmarked IN (0, 1))",
        )?;
        transaction.pragma_update(None, "user_version", 14)?;
        version = 14;
    }
    if version < 15 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS saved_outputs (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                conversation_title TEXT NOT NULL,
                content TEXT NOT NULL,
                provider_id TEXT,
                model_id TEXT,
                saved_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id)
             );",
        )?;
        add_column(
            &transaction,
            "conversations",
            "product_workspace_id",
            "TEXT REFERENCES workspaces(id) ON DELETE SET NULL",
        )?;
        transaction.pragma_update(None, "user_version", 15)?;
        version = 15;
    }
    if version < 16 {
        add_column(
            &transaction,
            "saved_outputs",
            "selected_version",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS saved_output_versions (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                version_number INTEGER NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id, version_number)
             );
             INSERT INTO saved_output_versions
                (conversation_id, message_id, version_number, content, created_at)
             SELECT conversation_id, message_id, 1, content, saved_at FROM saved_outputs;",
        )?;
        transaction.pragma_update(None, "user_version", 16)?;
        version = 16;
    }
    if version < 17 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS attachment_text_index (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                attachment_id TEXT NOT NULL,
                attachment_name TEXT NOT NULL,
                content TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id, attachment_id)
             );",
        )?;
        transaction.pragma_update(None, "user_version", 17)?;
        version = 17;
    }
    if version < 18 {
        add_column(
            &transaction,
            "messages",
            "personal_memory_fact_ids",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 18)?;
    }
    transaction.commit()
}

fn add_column(
    connection: &Connection,
    table: &str,
    name: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == name {
            return Ok(());
        }
    }
    connection.execute_batch(&format!(
        "ALTER TABLE \"{table}\" ADD COLUMN \"{name}\" {definition}"
    ))
}

fn create_model_favorites(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS model_favorites (
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            favorited_at INTEGER NOT NULL,
            PRIMARY KEY (provider_id, model_id)
         );",
    )
}
