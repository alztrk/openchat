//! Tauri commands for the local conversation, project, message, and saved-output database.
//!
//! Request values are bounded and validated before SQL or filesystem operations. Attachments
//! are stored as validated database content, while the database connection enforces the
//! minimum SQLite version required by the current schema.

use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::{
    project_folder::consume_project_folder_selection,
    request_bounds::{BoundedText, BoundedVec},
};

mod attachments;
mod maintenance;
mod markdown_export;
mod migrations;
mod saved_outputs;
mod settings;
mod types;

pub use attachments::*;
pub use maintenance::*;
pub use markdown_export::*;
pub use migrations::*;
pub use saved_outputs::*;
pub use settings::*;
pub use types::*;


#[tauri::command]
/// Checks the local database, applies supported schema migrations, and returns its location.
///
/// # Errors
/// Returns a storage or SQLite compatibility error when the database cannot be prepared safely.
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
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let auto_vacuum: i64 = connection
        .query_row("PRAGMA auto_vacuum", [], |row| row.get(0))
        .unwrap_or(0);
    if auto_vacuum == 0 {
        let _ = connection.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;");
    }
    migrate_chat_schema(&mut connection).map_err(|_| ChatStorageError::unavailable())?;

    // Maintenance: prune expired ephemeral cache entries and check freelist
    if let Ok(now) = unix_time_milliseconds() {
        let _ = purge_expired_cache(&connection, now);
    }
    let _ = perform_incremental_vacuum_if_needed(&connection);

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
/// Reports whether the database contains a completed assistant response.
///
/// # Errors
/// Returns a storage error if the query cannot be completed.
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
/// Returns conversations ordered for the application's history list.
///
/// # Errors
/// Returns a storage error if the database query fails.
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
/// Returns projects stored in the local conversation database.
///
/// # Errors
/// Returns a storage error if the database query fails.
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
/// Creates a project using a validated, single-use native folder selection.
///
/// # Errors
/// Returns a validation or storage error if the selection is invalid, expired, or already used.
pub fn create_project(input: CreateProjectInput) -> Result<(), ChatStorageError> {
    let id = input.id.trim();
    let name = input.name.trim();
    if !is_safe_identifier(id)
        || name.is_empty()
        || name.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let folder_path = consume_project_folder_selection(input.folder_selection_id.trim())
        .map_err(|_| ChatStorageError::invalid_input())?;
    let folder_path = folder_path
        .to_str()
        .ok_or_else(ChatStorageError::invalid_input)?
        .to_owned();
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
/// Deletes conversation history while preserving unrelated application settings and catalogs.
///
/// # Errors
/// Returns a storage error if the database transaction cannot complete.
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

    let storage_root = storage_root()?;
    match checked_attachment_root(&storage_root, false)? {
        Some(attachment_directory) => {
            fs::remove_dir_all(attachment_directory).map_err(|_| ChatStorageError::unavailable())
        }
        None => Ok(()),
    }
}

#[tauri::command]
/// Returns model favorites in the order used by the model picker.
///
/// # Errors
/// Returns a storage error if the database query fails.
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
/// Adds or removes a model from the user's favorites.
///
/// # Errors
/// Returns a validation or storage error.
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
/// Loads one conversation and its persisted organization metadata.
///
/// # Errors
/// Returns a validation or storage error when the ID is invalid or the query fails.
pub fn get_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<Option<StoredConversation>, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
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

pub(crate) fn stored_conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredConversation> {
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
/// Replaces a conversation title after validating its length and content.
///
/// # Errors
/// Returns a validation or storage error.
pub fn rename_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    title: BoundedText<MAXIMUM_CONVERSATION_TITLE_BYTES>,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let title = title.into_inner();
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
/// Changes the provider/model route and optional connection metadata for a conversation.
///
/// # Errors
/// Returns a validation or storage error.
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
/// Sets whether a conversation is pinned in the local history list.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_pinned(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    is_pinned: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(&conversation_id.into_inner(), "is_pinned", is_pinned)
}

#[tauri::command]
/// Sets whether a conversation is bookmarked.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_bookmarked(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    is_bookmarked: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(
        &conversation_id.into_inner(),
        "is_bookmarked",
        is_bookmarked,
    )
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
/// Replaces a conversation's bounded set of searchable tags.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_tags(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    tags: BoundedVec<
        BoundedText<MAXIMUM_CONVERSATION_TAG_BYTES>,
        MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS,
    >,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    if !is_safe_identifier(&conversation_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in tags.into_strings() {
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
/// Moves a bounded set of conversations into or out of a project.
///
/// # Errors
/// Returns a validation or storage error if any ID or destination is invalid.
pub fn move_conversations(
    conversation_ids: BoundedVec<
        BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
    >,
    project_id: Option<BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>>,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids.into_strings())?;
    let project_id = project_id.map(BoundedText::into_inner);
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
/// Sets archived state for a bounded set of conversations.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversations_archived(
    conversation_ids: BoundedVec<
        BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
    >,
    is_archived: bool,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids.into_strings())?;
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
/// Permanently deletes one conversation and its dependent persisted content.
///
/// # Errors
/// Returns a validation or storage error.
pub fn delete_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
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


#[tauri::command]
/// Loads messages for one conversation in transcript order.
///
/// # Errors
/// Returns a validation or storage error.
pub fn get_messages(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<Vec<StoredMessage>, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
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
/// Creates a new conversation after validating its ID, route, and optional project.
///
/// # Errors
/// Returns a validation or storage error.
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
/// Persists one message and its validated attachments using a database transaction.
///
/// # Errors
/// Returns a validation or storage error; oversized or malformed attachment data is rejected.
pub fn save_message(input: SaveMessageInput) -> Result<(), ChatStorageError> {
    let validated_attachments = validate_incoming_attachments(&input.attachments.0)?;
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



pub(crate) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_SAFE_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn is_optional_safe_identifier(value: Option<&str>) -> bool {
    value.is_none_or(is_safe_identifier)
}

pub(crate) fn is_valid_route_model_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 512 && !value.chars().any(char::is_control)
}

pub(crate) fn storage_root() -> Result<PathBuf, ChatStorageError> {
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

pub(crate) fn open_database() -> Result<Connection, ChatStorageError> {
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
        .pragma_update(None, "trusted_schema", false)
        .map_err(|_| ChatStorageError::unavailable())?;
    let trusted_schema_enabled = connection
        .pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if trusted_schema_enabled {
        return Err(ChatStorageError::unavailable());
    }
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    connection
        .create_scalar_function(
            "openchat_redact_credentials",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
                | rusqlite::functions::FunctionFlags::SQLITE_INNOCUOUS,
            |ctx| {
                let value = ctx.get::<Option<String>>(0)?;
                Ok(value)
            },
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(connection)
}

#[cfg(test)]
mod database_connection_tests {
    use std::path::Path;

    use super::open_supported_connection;

    #[test]
    fn supported_connections_disable_trusted_schema() {
        let connection = open_supported_connection(Path::new(":memory:"))
            .expect("open configured in-memory database");
        let trusted_schema_enabled = connection
            .pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))
            .expect("read trusted schema setting");

        assert!(!trusted_schema_enabled);
    }

    #[test]
    fn supported_connections_register_redact_function_and_pragmas() {
        let connection = open_supported_connection(Path::new(":memory:"))
            .expect("open configured in-memory database");
        let result: String = connection
            .query_row(
                "SELECT openchat_redact_credentials('test-key-123')",
                [],
                |row| row.get(0),
            )
            .expect("execute openchat_redact_credentials scalar function");
        assert_eq!(result, "test-key-123");
    }
}

pub(crate) fn unix_time_milliseconds() -> Result<i64, ChatStorageError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ChatStorageError::unavailable())
}



#[tauri::command]
/// Runs SQLite's optimizer pragma to update query planner statistics before idle or shutdown.
///
/// # Errors
/// Returns a storage error if PRAGMA optimize fails.
pub fn optimize_database() -> Result<(), ChatStorageError> {
    let connection = open_database()?;
    connection
        .execute("PRAGMA optimize", [])
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[cfg(test)]
mod request_bound_tests {
    use serde_json::{json, Value};

    use super::{
        BoundedIncomingAttachments, CreateConversationInput, SaveMessageInput,
        MAXIMUM_ATTACHMENT_FILE_COUNT, MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION, MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS,
        MAXIMUM_MESSAGE_METADATA_BYTES, MAXIMUM_NEW_CONVERSATION_TITLE_BYTES,
        MAXIMUM_SAFE_IDENTIFIER_BYTES,
    };
    use crate::request_bounds::{BoundedText, BoundedVec};

    fn attachment(data_base64: &str) -> Value {
        json!({
            "id": "attachment-1",
            "name": "note.txt",
            "mimeType": "text/plain",
            "sizeBytes": 1,
            "kind": "text",
            "dataBase64": data_base64,
        })
    }

    #[test]
    fn conversation_mutation_deserialization_keeps_existing_item_limits() {
        let ids_at_limit = Value::Array(
            (0..MAXIMUM_CONVERSATION_IDS_PER_MUTATION)
                .map(|_| json!("conversation-1"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<
                BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
                MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
            >,
        >(ids_at_limit)
        .is_ok());

        let ids_over_limit = Value::Array(
            (0..=MAXIMUM_CONVERSATION_IDS_PER_MUTATION)
                .map(|_| json!("conversation-1"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<
                BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
                MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
            >,
        >(ids_over_limit)
        .is_err());

        let tags_at_supported_input_limit = Value::Array(
            (0..MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS)
                .map(|_| json!("tag"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<BoundedText<128>, MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS>,
        >(tags_at_supported_input_limit)
        .is_ok());
    }

    #[test]
    fn conversation_creation_deserializes_existing_renderer_fields_with_bounds() {
        let request = json!({
            "id": "conversation-1",
            "title": "A new conversation",
            "providerId": "opencode",
            "modelId": "gpt-6-luna",
            "connectionId": null,
            "workspaceId": null,
            "apiKeyConnectionId": null,
            "projectId": null
        });
        let parsed = serde_json::from_value::<CreateConversationInput>(request.clone())
            .expect("the existing renderer conversation shape should deserialize");
        assert_eq!(parsed.provider_id, "opencode");

        let mut oversized_id = request.clone();
        oversized_id["id"] = Value::String("x".repeat(MAXIMUM_SAFE_IDENTIFIER_BYTES + 1));
        assert!(serde_json::from_value::<CreateConversationInput>(oversized_id).is_err());

        let mut oversized_title = request;
        oversized_title["title"] =
            Value::String("x".repeat(MAXIMUM_NEW_CONVERSATION_TITLE_BYTES + 1));
        assert!(serde_json::from_value::<CreateConversationInput>(oversized_title).is_err());
    }

    #[test]
    fn attachment_deserialization_enforces_count_and_cumulative_size_limits() {
        let attachments_at_limit = Value::Array(
            (0..MAXIMUM_ATTACHMENT_FILE_COUNT)
                .map(|_| attachment(""))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedIncomingAttachments<
                MAXIMUM_ATTACHMENT_FILE_COUNT,
                MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
            >,
        >(attachments_at_limit)
        .is_ok());

        let attachments_over_count = Value::Array(
            (0..=MAXIMUM_ATTACHMENT_FILE_COUNT)
                .map(|_| attachment(""))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedIncomingAttachments<
                MAXIMUM_ATTACHMENT_FILE_COUNT,
                MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
            >,
        >(attachments_over_count)
        .is_err());

        let encoded_limit = Value::Array(vec![attachment("123456"), attachment("abcdef")]);
        assert!(serde_json::from_value::<BoundedIncomingAttachments<2, 12>>(encoded_limit).is_ok());
        let encoded_over_limit = Value::Array(vec![attachment("123456"), attachment("abcdefg")]);
        assert!(
            serde_json::from_value::<BoundedIncomingAttachments<2, 12>>(encoded_over_limit)
                .is_err()
        );
    }

    #[test]
    fn save_message_deserializes_the_existing_renderer_attachment_shape() {
        let request = json!({
            "id": "message-1",
            "conversationId": "conversation-1",
            "role": "user",
            "content": "Review this file.",
            "status": "completed",
            "attachments": [attachment("eA==")],
        });
        let message = serde_json::from_value::<SaveMessageInput>(request)
            .expect("the existing renderer message payload should deserialize");

        assert_eq!(message.content, "Review this file.");
        assert_eq!(message.attachments.0.len(), 1);
        assert_eq!(message.attachments.0[0].data_base64, "eA==");

        let oversized_metadata = json!({
            "id": "message-1",
            "conversationId": "conversation-1",
            "role": "assistant",
            "content": "Response",
            "status": "completed",
            "toolActivities": "x".repeat(MAXIMUM_MESSAGE_METADATA_BYTES + 1),
        });
        assert!(serde_json::from_value::<SaveMessageInput>(oversized_metadata).is_err());
    }
}


#[cfg(test)]
mod settings_and_unified_storage_tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        migrate_chat_schema(&mut connection).expect("migrate schema");
        connection
    }

    #[test]
    fn schema_migration_19_creates_all_unified_tables() {
        let conn = in_memory_db();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(tables.contains(&"app_settings".to_string()));
        assert!(tables.contains(&"personal_memory_facts".to_string()));
        assert!(tables.contains(&"secure_vault".to_string()));
        assert!(tables.contains(&"ephemeral_cache".to_string()));
    }

    #[test]
    fn schema_migration_20_creates_covering_and_sort_indexes() {
        let conn = in_memory_db();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type='index'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(indexes.contains(&"idx_conversations_sort_list".to_string()));
        assert!(indexes.contains(&"idx_messages_conversation_sort".to_string()));
    }

    #[test]
    fn app_settings_crud_works_inside_transaction() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            params!["openchat.theme", "\"dark\"", 1000],
        ).unwrap();

        let val: String = conn.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            ["openchat.theme"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(val, "\"dark\"");

        // Update
        conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params!["openchat.theme", "\"light\"", 2000],
        ).unwrap();
        let val2: String = conn.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            ["openchat.theme"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(val2, "\"light\"");
    }

    #[test]
    fn personal_memory_facts_round_trip() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO personal_memory_facts (id, fact_text, category, source_conversation_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["fact-1", "User prefers concise answers", "preferences", "conv-1", 1000, 1000],
        ).unwrap();

        let fact: String = conn.query_row(
            "SELECT fact_text FROM personal_memory_facts WHERE id = ?1",
            ["fact-1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(fact, "User prefers concise answers");
    }

    #[test]
    fn ephemeral_cache_ttl_filtering() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO ephemeral_cache (key, value_json, expires_at) VALUES (?1, ?2, ?3)",
            params!["cache-key", "{\"pricing\":1}", 5000],
        ).unwrap();

        // Valid when query time < expires_at
        let active: Option<String> = conn.query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params!["cache-key", 4000],
            |row| row.get(0),
        ).optional().unwrap();
        assert_eq!(active, Some("{\"pricing\":1}".to_string()));

        // Expired when query time >= expires_at
        let expired: Option<String> = conn.query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params!["cache-key", 6000],
            |row| row.get(0),
        ).optional().unwrap();
        assert_eq!(expired, None);
    }
}
