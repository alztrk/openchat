//! RPC handlers for the durable conversation database.
//!
//! The renderer used to call Tauri commands that opened the SQLite file directly.  These
//! handlers are the storage boundary used while that migration is completed.  Every branch
//! acquires a queued connection for a bounded synchronous operation and drops it before the
//! dispatcher can await another service operation.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection, sqlite_error_code},
};

use super::invalid_request_params;

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TITLE_CHARS: usize = 512;
const MAX_MODEL_ID_CHARS: usize = 512;
const MAX_MESSAGE_CONTENT_BYTES: usize = 4_000_000;
const MAX_MESSAGE_METADATA_BYTES: usize = 4_000_000;
const MAX_CONVERSATION_IDS: usize = 500;
const MAX_PROJECT_NAME_BYTES: usize = MAX_TITLE_CHARS * 4;
const MAX_WORKSPACE_ROOT_BYTES: usize = 32 * 1024;
const MAX_SEARCH_QUERY_BYTES: usize = 512;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_ATTACHMENT_COUNT: usize = 10;
const MAX_IMAGE_COUNT: usize = 3;
const MAX_ATTACHMENT_BYTES: usize = 14 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_ATTACHMENT_NAME_BYTES: usize = 255 * 4;
const MAX_ATTACHMENT_MIME_BYTES: usize = 64;
const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAX_ATTACHMENT_SCAN: usize = 10_000;
const MAX_ATTACHMENT_PAGE: usize = 100;
const ATTACHMENT_TEXT_INDEX_SETTING: &str = "attachment_text_index.enabled";
const MAX_ATTACHMENT_QUERY_CHARS: usize = 512;
const MAX_ATTACHMENT_OFFSET: u64 = 1_000_000;
const ATTACHMENT_LIBRARY_BATCH_SIZE: u64 = 50;

macro_rules! conversation_from_row {
    ($row:expr) => {{
        let row = $row;
        let agent_json = row.get::<_, Option<String>>(13)?;
        let agent = agent_json
            .map(|value| {
                serde_json::from_str::<Value>(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        13,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .transpose()?;
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "titleSource": row.get::<_, String>(2)?,
            "providerId": row.get::<_, Option<String>>(3)?,
            "modelId": row.get::<_, Option<String>>(4)?,
            "connectionId": row.get::<_, Option<String>>(5)?,
            "workspaceId": row.get::<_, Option<String>>(6)?,
            "apiKeyConnectionId": row.get::<_, Option<String>>(7)?,
            "projectId": row.get::<_, Option<String>>(8)?,
            "isPinned": row.get::<_, bool>(9)?,
            "isArchived": row.get::<_, bool>(10)?,
            "createdAt": row.get::<_, i64>(11)?,
            "updatedAt": row.get::<_, i64>(12)?,
            "agent": agent,
        }))
    }};
}

macro_rules! message_from_row {
    ($row:expr) => {{
        let row = $row;
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "conversationId": row.get::<_, String>(1)?,
            "role": row.get::<_, String>(2)?,
            "content": row.get::<_, String>(3)?,
            "createdAt": row.get::<_, Option<i64>>(4)?,
            "outputTokens": row.get::<_, Option<i64>>(5)?,
            "tokensPerSecond": row.get::<_, Option<f64>>(6)?,
            "elapsedMicroseconds": row.get::<_, Option<i64>>(7)?,
            "providerId": row.get::<_, Option<String>>(8)?,
            "modelId": row.get::<_, Option<String>>(9)?,
            "citationSources": row.get::<_, String>(10)?,
            "personalMemoryFactIds": row.get::<_, String>(11)?,
            "reasoningSummaries": row.get::<_, String>(12)?,
            "toolActivities": row.get::<_, String>(13)?,
            "status": row.get::<_, String>(14)?,
            "failureCode": row.get::<_, Option<String>>(15)?,
        }))
    }};
}

/// Dispatches durable conversation, project, message, favorite, and attachment storage calls.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "storage.chat.conversations.list" => list_conversations(storage),
        "storage.chat.conversations.get" => get_conversation(params, storage),
        "storage.chat.conversations.create" => create_conversation(params, storage),
        "storage.chat.conversations.rename" => rename_conversation(params, storage),
        "storage.chat.conversations.route.set" => set_conversation_route(params, storage),
        "storage.chat.conversations.pinned.set" => set_conversation_pinned(params, storage),
        "storage.chat.conversations.project.set" => move_conversations(params, storage),
        "storage.chat.conversations.archived.set" => set_conversations_archived(params, storage),
        "storage.chat.conversations.delete" => delete_conversation(params, storage),
        "storage.chat.conversations.clear" => clear_conversation_history(storage),
        "storage.chat.projects.list" => list_projects(storage),
        "storage.chat.projects.create" => create_project(params, storage),
        "storage.chat.projects.workspace.add" => add_project_workspace_root(params, storage),
        "storage.chat.projects.workspace.remove" => remove_project_workspace_root(params, storage),
        "storage.chat.messages.list" => list_messages(params, storage),
        "storage.chat.messages.save" => save_message(params, storage),
        "storage.chat.messages.search" => search_messages(params, storage),
        "storage.chat.responses.has_completed" => has_completed_assistant_response(storage),
        "storage.chat.favorites.list" => list_model_favorites(storage),
        "storage.chat.favorites.set" => set_model_favorite(params, storage),
        "storage.chat.attachments.read" => read_message_attachment(params, storage),
        "storage.chat.attachments.list" => list_conversation_attachments(params, storage),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn list_conversations(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id,
                    is_pinned, is_archived, created_at, updated_at, agent_json
             FROM conversations
             ORDER BY is_pinned DESC, updated_at DESC, id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| conversation_from_row!(row))
        .map_err(storage_error)?;
    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row.map_err(storage_error)?);
    }
    Ok(json!({"conversations": conversations}))
}

fn get_conversation(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let conversation = connection
        .query_row(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id,
                    is_pinned, is_archived, created_at, updated_at, agent_json
             FROM conversations WHERE id = ?1",
            [id],
            |row| conversation_from_row!(row),
        )
        .optional()
        .map_err(storage_error)?;
    Ok(json!({"conversation": conversation}))
}

fn create_conversation(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let title = required_title(input, "title")?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let connection_id = optional_safe_id(input, "connectionId")?;
    let workspace_id = optional_safe_id(input, "workspaceId")?;
    let api_key_connection_id = optional_safe_id(input, "apiKeyConnectionId")?;
    let project_id = optional_safe_id(input, "projectId")?;
    let agent_json = optional_json_object(input, "agent")?.map(|agent| agent.to_string());
    let now = unix_time_millis()?;
    let connection = storage.connect().map_err(storage_error)?;
    connection
        .execute(
            "INSERT INTO conversations (
                id, title, title_source, provider_id, model_id, connection_id,
                workspace_id, api_key_connection_id, project_id, agent_json, created_at, updated_at
             ) VALUES (?1, ?2, 'automatic', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                id,
                title,
                provider_id,
                model_id,
                connection_id,
                workspace_id,
                api_key_connection_id,
                project_id,
                agent_json,
                now,
            ],
        )
        .map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

fn rename_conversation(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let title = required_title(input, "title")?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations SET title = ?1, title_source = 'manual', updated_at = ?2
             WHERE id = ?3",
            params![title, unix_time_millis()?, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

fn set_conversation_route(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let connection_id = optional_safe_id(input, "connectionId")?;
    let workspace_id = optional_safe_id(input, "workspaceId")?;
    let api_key_connection_id = optional_safe_id(input, "apiKeyConnectionId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations
             SET provider_id = ?1, model_id = ?2, connection_id = ?3,
                 workspace_id = ?4, api_key_connection_id = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                provider_id,
                model_id,
                connection_id,
                workspace_id,
                api_key_connection_id,
                unix_time_millis()?,
                conversation_id,
            ],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

fn set_conversation_pinned(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let is_pinned = input
        .get("isPinned")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations SET is_pinned = ?1 WHERE id = ?2",
            params![is_pinned, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

fn move_conversations(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let ids = required_id_array(input, "conversationIds")?;
    let project_id = optional_safe_id(input, "projectId")?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    if let Some(project_id) = project_id.as_deref() {
        let exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage_error)?;
        if !exists {
            return Err(not_found("project"));
        }
    }
    for id in &ids {
        let changed = transaction
            .execute(
                "UPDATE conversations SET project_id = ?1, is_pinned = 0 WHERE id = ?2",
                params![project_id, id],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found("conversation"));
        }
    }
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

fn set_conversations_archived(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let ids = required_id_array(input, "conversationIds")?;
    let is_archived = input
        .get("isArchived")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    for id in &ids {
        let changed = transaction
            .execute(
                "UPDATE conversations
                 SET is_archived = ?1,
                     is_pinned = CASE WHEN ?1 = 1 THEN 0 ELSE is_pinned END
                 WHERE id = ?2",
                params![is_archived, id],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found("conversation"));
        }
    }
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

fn delete_conversation(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM attachment_text_index WHERE conversation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM saved_output_versions WHERE conversation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    transaction
        .execute("DELETE FROM saved_outputs WHERE conversation_id = ?1", [id])
        .map_err(storage_error)?;
    let deleted = transaction
        .execute("DELETE FROM conversations WHERE id = ?1", [id])
        .map_err(storage_error)?;
    if deleted != 1 {
        return Err(not_found("conversation"));
    }
    transaction.commit().map_err(storage_error)?;
    remove_attachment_directory(storage.root(), id)?;
    Ok(json!({"deleted": true}))
}

fn clear_conversation_history(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute_batch(
            "DELETE FROM attachment_text_index;
             DELETE FROM saved_output_versions;
             DELETE FROM saved_outputs;
             DELETE FROM conversations;",
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    remove_all_attachment_directories(storage.root())?;
    Ok(json!({"cleared": true}))
}

fn list_projects(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, folder_path, created_at, updated_at
             FROM projects ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let name = row.get::<_, String>(1)?;
            let folder_path = row.get::<_, String>(2)?;
            let created_at = row.get::<_, i64>(3)?;
            let updated_at = row.get::<_, i64>(4)?;
            Ok((id, name, folder_path, created_at, updated_at))
        })
        .map_err(storage_error)?;
    let mut projects = Vec::new();
    for row in rows {
        let (id, name, folder_path, created_at, updated_at) = row.map_err(storage_error)?;
        let mut roots = connection
            .prepare(
                "SELECT root_path FROM project_workspace_roots
                 WHERE project_id = ?1 ORDER BY position ASC, root_id ASC",
            )
            .map_err(storage_error)?;
        let roots = roots
            .query_map([&id], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        projects.push(json!({
            "id": id,
            "name": name,
            "folderPath": folder_path,
            "workspaceRoots": roots,
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }
    Ok(json!({"projects": projects}))
}

fn create_project(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let name = required_bounded_string(input, "name", MAX_PROJECT_NAME_BYTES, true)?;
    if name.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid_request_params());
    }
    let folder_path = validate_project_folder_path(required_bounded_string(
        input,
        "folderPath",
        MAX_WORKSPACE_ROOT_BYTES,
        true,
    )?)?;
    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, name, folder_path, now],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO project_workspace_roots
                (root_id, project_id, root_path, position, created_at)
             VALUES (lower(hex(randomblob(16))), ?1, ?2, 0, ?3)",
            params![id, folder_path, now],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

fn add_project_workspace_root(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let project_id = required_safe_id(input, "projectId")?;
    let root_path = validate_project_folder_path(required_bounded_string(
        input,
        "folderPath",
        MAX_WORKSPACE_ROOT_BYTES,
        true,
    )?)?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let roots = {
        let mut statement = transaction
            .prepare(
                "SELECT root_path, position FROM project_workspace_roots
                 WHERE project_id = ?1 ORDER BY position ASC",
            )
            .map_err(storage_error)?;
        statement
            .query_map([project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?
    };
    if roots.is_empty()
        || roots.len() >= 8
        || roots
            .iter()
            .any(|(path, _)| paths_overlap(path, &root_path))
    {
        return Err(invalid_request_params());
    }
    let position = roots
        .iter()
        .map(|(_, position)| *position)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(invalid_request_params)?;
    transaction
        .execute(
            "INSERT INTO project_workspace_roots
                (root_id, project_id, root_path, position, created_at)
             VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4)",
            params![project_id, root_path, position, unix_time_millis()?],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

fn validate_project_folder_path(value: &str) -> Result<String, ServiceError> {
    let path = Path::new(value);
    if !path.is_absolute() || value.chars().any(char::is_control) {
        return Err(invalid_request_params());
    }
    let canonical_path = fs::canonicalize(path).map_err(|_| invalid_request_params())?;
    if !canonical_path.is_dir() {
        return Err(invalid_request_params());
    }
    let canonical_path = canonical_path
        .to_str()
        .filter(|path| path.len() <= MAX_WORKSPACE_ROOT_BYTES)
        .ok_or_else(invalid_request_params)?;
    Ok(canonical_path.to_owned())
}

fn remove_project_workspace_root(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let project_id = required_safe_id(input, "projectId")?;
    let root_path = required_bounded_string(input, "rootPath", MAX_WORKSPACE_ROOT_BYTES, true)?;
    let connection = storage.connect().map_err(storage_error)?;
    let removed = connection
        .execute(
            "DELETE FROM project_workspace_roots
             WHERE project_id = ?1 AND root_path = ?2 AND position > 0",
            params![project_id, root_path],
        )
        .map_err(storage_error)?;
    if removed != 1 {
        return Err(not_found("workspace root"));
    }
    Ok(json!({"deleted": true}))
}

fn list_messages(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, provider_id, model_id,
                    citation_sources, personal_memory_fact_ids, reasoning_summaries,
                    tool_activities, status, failure_code
             FROM messages WHERE conversation_id = ?1
             ORDER BY created_at ASC, id ASC LIMIT 10000",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([conversation_id], |row| message_from_row!(row))
        .map_err(storage_error)?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(storage_error)?);
    }
    Ok(json!({"messages": messages}))
}

fn save_message(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let role = required_message_role(input, "role")?;
    let mut content = required_content(input, "content")?;
    let status = required_message_status(input, "status")?;
    let provider_id = optional_safe_id(input, "providerId")?;
    let model_id = optional_model_id(input, "modelId")?;
    let citation_sources = optional_json_array(input, "citationSources")?;
    let personal_memory_fact_ids = optional_json_array(input, "personalMemoryFactIds")?;
    let reasoning_summaries = optional_json_array(input, "reasoningSummaries")?;
    let tool_activities = optional_json_array(input, "toolActivities")?;
    let failure_code = optional_failure_code(input, "failureCode")?;
    let created_at = input
        .get("createdAt")
        .map(|value| value.as_i64().ok_or_else(invalid_request_params))
        .transpose()?
        .unwrap_or(unix_time_millis()?);
    let output_tokens = optional_nonnegative_i64(input, "outputTokens")?;
    let tokens_per_second = optional_nonnegative_f64(input, "tokensPerSecond")?;
    let elapsed_microseconds = optional_nonnegative_i64(input, "elapsedMicroseconds")?;
    let attachments = parse_attachment_inputs(input.get("attachments"))?;
    if role != "user" && !attachments.is_empty() {
        return Err(invalid_request_params());
    }
    let attachment_metadata = attachments
        .iter()
        .map(|attachment| {
            json!({
                "id": attachment.id,
                "name": attachment.name,
                "mimeType": attachment.mime_type,
                "sizeBytes": attachment.size_bytes,
                "kind": attachment.kind,
            })
        })
        .collect::<Vec<_>>();
    if !attachments.is_empty() && !content.starts_with(MESSAGE_CONTENT_PREFIX) {
        content = format!(
            "{MESSAGE_CONTENT_PREFIX}{}",
            json!({"content": content, "attachments": attachment_metadata})
        );
    }
    if content.len() > MAX_MESSAGE_CONTENT_BYTES {
        return Err(invalid_request_params());
    }
    let staged = if attachments.is_empty() {
        None
    } else {
        Some(stage_attachments(
            storage.root(),
            conversation_id,
            id,
            &attachments,
        )?)
    };
    let write_result = (|| -> Result<(), ServiceError> {
        let mut connection = storage.connect().map_err(storage_error)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, provider_id, model_id,
                    citation_sources, personal_memory_fact_ids, reasoning_summaries,
                    tool_activities, status, failure_code
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                            ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(conversation_id, id) DO UPDATE SET
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
                    id,
                    conversation_id,
                    role,
                    content,
                    created_at,
                    output_tokens,
                    tokens_per_second,
                    elapsed_microseconds,
                    provider_id,
                    model_id,
                    citation_sources,
                    personal_memory_fact_ids,
                    reasoning_summaries,
                    tool_activities,
                    status,
                    failure_code,
                ],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "UPDATE conversations SET updated_at = MAX(updated_at, ?1) WHERE id = ?2",
                params![created_at, conversation_id],
            )
            .map_err(storage_error)?;
        if let Some((staging, destination)) = staged.as_ref() {
            fs::rename(staging, destination).map_err(|_| storage_unavailable())?;
        }
        transaction.commit().map_err(storage_error)
    })();
    if write_result.is_err() {
        if let Some((staging, destination)) = staged {
            let _ = remove_directory_if_present(&staging);
            let _ = remove_directory_if_present(&destination);
        }
        return write_result.map(|_| json!({"saved": true}));
    }
    Ok(json!({"saved": true}))
}

fn search_messages(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let query =
        optional_bounded_string(input, "query", MAX_SEARCH_QUERY_BYTES, false)?.unwrap_or_default();
    let role = match input.get("roleFilter") {
        None | Some(Value::Null) => None,
        Some(Value::String(role)) if role == "user" || role == "assistant" => Some(role.as_str()),
        _ => return Err(invalid_request_params()),
    };
    let has_code_only = input
        .get("hasCodeOnly")
        .map(|value| value.as_bool().ok_or_else(invalid_request_params))
        .transpose()?
        .unwrap_or(false);
    let has_query = !query.trim().is_empty();
    if !has_query && role.is_none() && !has_code_only {
        return Ok(json!({"messages": []}));
    }
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| (1..=MAX_SEARCH_RESULTS).contains(limit))
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(40);
    let sql = "SELECT id, conversation_id, role, content, created_at
         FROM messages
         WHERE (?1 = '' OR instr(openchat_normalize_text(content), ?1) > 0)
           AND (?2 = '' OR role = ?2)
           AND (?3 = 0 OR content LIKE '%```%')
         ORDER BY created_at DESC, id DESC LIMIT ?4";
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection.prepare(&sql).map_err(storage_error)?;
    let normalized = normalize_search_text(&query);
    let role_value = role.unwrap_or("");
    let rows = statement
        .query_map(
            params![normalized, role_value, has_code_only, limit as i64],
            |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "conversationId": row.get::<_, String>(1)?,
                    "role": row.get::<_, String>(2)?,
                    "content": row.get::<_, String>(3)?,
                    "createdAt": row.get::<_, Option<i64>>(4)?,
                }))
            },
        )
        .map_err(storage_error)?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(storage_error)?);
    }
    Ok(json!({"messages": messages}))
}

fn has_completed_assistant_response(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let completed = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM messages WHERE role = 'assistant' AND status = 'completed'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    Ok(json!({"completed": completed}))
}

fn list_model_favorites(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT provider_id, model_id, display_name, source_connection_id
             FROM model_favorites
             ORDER BY provider_id ASC, display_name ASC, model_id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "providerId": row.get::<_, String>(0)?,
                "modelId": row.get::<_, String>(1)?,
                "displayName": row.get::<_, String>(2)?,
                "sourceConnectionId": row.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(storage_error)?;
    let mut favorites = Vec::new();
    for row in rows {
        favorites.push(row.map_err(storage_error)?);
    }
    Ok(json!({"favorites": favorites}))
}

fn set_model_favorite(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let display_name = required_bounded_string(input, "displayName", MAX_TITLE_CHARS * 4, true)?;
    let is_favorite = input
        .get("isFavorite")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let source_connection_id = optional_safe_id(input, "sourceConnectionId")?;
    let connection = storage.connect().map_err(storage_error)?;
    if is_favorite {
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
                    provider_id,
                    model_id,
                    display_name,
                    unix_time_millis()?,
                    source_connection_id
                ],
            )
            .map_err(storage_error)?;
    } else {
        connection
            .execute(
                "DELETE FROM model_favorites WHERE provider_id = ?1 AND model_id = ?2",
                params![provider_id, model_id],
            )
            .map_err(storage_error)?;
    }
    Ok(json!({"saved": true}))
}

fn read_message_attachment(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let message_id = required_safe_id(input, "messageId")?;
    let attachment_id = required_safe_id(input, "attachmentId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let content = connection
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?
        .ok_or_else(|| not_found("message"))?;
    let envelope = parse_attachment_envelope(&content)?;
    let metadata = envelope
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or_else(|| not_found("attachment"))?;
    let path = attachment_path(storage.root(), conversation_id, message_id, attachment_id)?;
    let bytes = fs::read(path).map_err(|_| storage_unavailable())?;
    if bytes.len() != metadata.size_bytes {
        return Err(storage_unavailable());
    }
    Ok(json!({"dataBase64": STANDARD.encode(bytes)}))
}

fn list_conversation_attachments(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let offset = input
        .get("offset")
        .map(|value| {
            value
                .as_u64()
                .filter(|offset| *offset <= MAX_ATTACHMENT_OFFSET)
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(0);
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| (1..=MAX_ATTACHMENT_PAGE).contains(limit))
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(50);
    let query = match input.get("query") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.chars().count() > MAX_ATTACHMENT_QUERY_CHARS
                || value.chars().any(char::is_control)
            {
                return Err(invalid_request_params());
            }
            value.to_lowercase()
        }
        _ => return Err(invalid_request_params()),
    };
    let availability = match input.get("availability") {
        None | Some(Value::Null) => "all",
        Some(Value::String(value)) => value.as_str(),
        _ => return Err(invalid_request_params()),
    };
    if !matches!(availability, "all" | "available" | "missing") {
        return Err(invalid_request_params());
    }

    let connection = storage.connect().map_err(storage_error)?;
    let index_enabled = attachment_text_index_enabled(&connection)?;
    let mut cursor = offset;
    let mut scanned = 0_u64;
    let mut items = Vec::new();
    let mut found_extra_at = None;
    let mut reached_end = false;

    while items.len() <= limit && scanned < MAX_ATTACHMENT_SCAN as u64 {
        let batch_limit =
            ATTACHMENT_LIBRARY_BATCH_SIZE.min((MAX_ATTACHMENT_SCAN as u64).saturating_sub(scanned));
        let batch =
            query_attachment_library_batch(&connection, cursor, batch_limit, index_enabled)?;
        if batch.is_empty() {
            reached_end = true;
            break;
        }

        for (position, candidate) in batch.iter().enumerate() {
            let candidate_offset = cursor.saturating_add(position as u64);
            let Some(summary) = candidate.summary.as_ref() else {
                continue;
            };
            let available = attachment_is_available(
                storage.root(),
                &summary.conversation_id,
                &summary.message_id,
                &summary.attachment_id,
                summary.size_bytes,
            )?;
            if (availability == "available" && !available)
                || (availability == "missing" && available)
            {
                continue;
            }
            if !query.is_empty()
                && !summary.file_name.to_lowercase().contains(&query)
                && !summary.conversation_title.to_lowercase().contains(&query)
                && !candidate.indexed_content.to_lowercase().contains(&query)
            {
                continue;
            }
            if items.len() == limit {
                found_extra_at = Some(candidate_offset);
                break;
            }
            items.push(json!({
                "conversationId": summary.conversation_id,
                "conversationTitle": summary.conversation_title,
                "projectId": summary.project_id,
                "messageId": summary.message_id,
                "attachmentId": summary.attachment_id,
                "fileName": summary.file_name,
                "mimeType": summary.mime_type,
                "sizeBytes": summary.size_bytes,
                "createdAt": summary.created_at,
                "isAvailable": available,
            }));
        }
        if found_extra_at.is_some() {
            break;
        }
        cursor = cursor.saturating_add(batch.len() as u64);
        scanned = scanned.saturating_add(batch.len() as u64);
        if batch.len() < batch_limit as usize {
            reached_end = true;
            break;
        }
    }

    let next_offset = found_extra_at.or_else(|| {
        if reached_end {
            None
        } else if scanned >= MAX_ATTACHMENT_SCAN as u64 {
            Some(cursor)
        } else {
            None
        }
    });
    Ok(json!({"items": items, "nextOffset": next_offset}))
}

struct AttachmentLibraryCandidate {
    summary: Option<AttachmentLibrarySummary>,
    indexed_content: String,
}

struct AttachmentLibrarySummary {
    conversation_id: String,
    conversation_title: String,
    project_id: Option<String>,
    message_id: String,
    attachment_id: String,
    file_name: String,
    mime_type: String,
    size_bytes: usize,
    created_at: i64,
}

fn query_attachment_library_batch(
    connection: &StorageConnection<'_>,
    offset: u64,
    limit: u64,
    index_enabled: bool,
) -> Result<Vec<AttachmentLibraryCandidate>, ServiceError> {
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
        .map_err(storage_error)?;
    let rows = statement
        .query_map(
            params![
                MESSAGE_CONTENT_PREFIX,
                index_enabled,
                limit as i64,
                offset as i64
            ],
            |row| {
                let summary = (|| -> rusqlite::Result<Option<AttachmentLibrarySummary>> {
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
                    let metadata = AttachmentMetadata {
                        id: attachment_id.clone(),
                        name: file_name.clone(),
                        mime_type: mime_type.clone(),
                        size_bytes,
                        kind,
                    };
                    if validate_attachment_metadata(&metadata).is_err() {
                        return Ok(None);
                    }
                    Ok(Some(AttachmentLibrarySummary {
                        conversation_id,
                        conversation_title,
                        project_id: row.get(2)?,
                        message_id,
                        attachment_id,
                        file_name,
                        mime_type,
                        size_bytes,
                        created_at: row.get(4)?,
                    }))
                })()?;
                Ok(AttachmentLibraryCandidate {
                    summary,
                    indexed_content: row.get(10)?,
                })
            },
        )
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

fn attachment_text_index_enabled(connection: &StorageConnection<'_>) -> Result<bool, ServiceError> {
    let value = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [ATTACHMENT_TEXT_INDEX_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    match value.as_deref() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(storage_unavailable()),
    }
}

#[derive(Clone)]
struct AttachmentInput {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct AttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
}

struct AttachmentEnvelope {
    _content: String,
    attachments: Vec<AttachmentMetadata>,
}

fn parse_attachment_inputs(value: Option<&Value>) -> Result<Vec<AttachmentInput>, ServiceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value.as_array().ok_or_else(invalid_request_params)?;
    if items.len() > MAX_ATTACHMENT_COUNT {
        return Err(invalid_request_params());
    }
    let mut ids = HashSet::new();
    let mut total = 0_usize;
    let mut images = 0_usize;
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(invalid_request_params)?;
        let id = required_safe_id(object, "id")?.to_owned();
        if !ids.insert(id.clone()) {
            return Err(invalid_request_params());
        }
        let name = required_bounded_string(object, "name", MAX_ATTACHMENT_NAME_BYTES, true)?;
        let mime_type =
            required_bounded_string(object, "mimeType", MAX_ATTACHMENT_MIME_BYTES, true)?;
        let kind = required_bounded_string(object, "kind", 16, true)?;
        if kind != "image" && kind != "text" {
            return Err(invalid_request_params());
        }
        let size_bytes = object
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size > 0)
            .ok_or_else(invalid_request_params)?;
        let encoded = object
            .get("dataBase64")
            .and_then(Value::as_str)
            .ok_or_else(invalid_request_params)?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| invalid_request_params())?;
        if bytes.len() != size_bytes
            || size_bytes
                > if kind == "image" {
                    MAX_IMAGE_BYTES
                } else {
                    MAX_TEXT_BYTES
                }
        {
            return Err(invalid_request_params());
        }
        total = total
            .checked_add(size_bytes)
            .ok_or_else(invalid_request_params)?;
        if total > MAX_ATTACHMENT_BYTES {
            return Err(invalid_request_params());
        }
        if kind == "image" {
            images += 1;
            if images > MAX_IMAGE_COUNT {
                return Err(invalid_request_params());
            }
            if !valid_image_signature(&mime_type, &bytes) {
                return Err(invalid_request_params());
            }
        } else if std::str::from_utf8(&bytes).is_err() {
            return Err(invalid_request_params());
        }
        result.push(AttachmentInput {
            id,
            name: name.to_owned(),
            mime_type: mime_type.to_owned(),
            size_bytes,
            kind: kind.to_owned(),
            bytes,
        });
    }
    Ok(result)
}

fn parse_attachment_envelope(content: &str) -> Result<AttachmentEnvelope, ServiceError> {
    let encoded = content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(|| storage_unavailable())?;
    let value: Value = serde_json::from_str(encoded).map_err(|_| storage_unavailable())?;
    let object = value.as_object().ok_or_else(|| storage_unavailable())?;
    let text = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| storage_unavailable())?
        .to_owned();
    let items = object
        .get("attachments")
        .and_then(Value::as_array)
        .ok_or_else(|| storage_unavailable())?;
    if items.is_empty() || items.len() > MAX_ATTACHMENT_COUNT {
        return Err(storage_unavailable());
    }
    let mut ids = HashSet::new();
    let mut total = 0_usize;
    let mut image_count = 0_usize;
    let mut attachments = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(|| storage_unavailable())?;
        let id = required_safe_id(object, "id")
            .map_err(|_| storage_unavailable())?
            .to_owned();
        if !ids.insert(id.clone()) {
            return Err(storage_unavailable());
        }
        let name = required_bounded_string(object, "name", MAX_ATTACHMENT_NAME_BYTES, true)
            .map_err(|_| storage_unavailable())?;
        let mime_type =
            required_bounded_string(object, "mimeType", MAX_ATTACHMENT_MIME_BYTES, true)
                .map_err(|_| storage_unavailable())?;
        let kind =
            required_bounded_string(object, "kind", 16, true).map_err(|_| storage_unavailable())?;
        let size_bytes = object
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size > 0)
            .ok_or_else(|| storage_unavailable())?;
        if kind != "image" && kind != "text" {
            return Err(storage_unavailable());
        }
        if kind == "image" {
            image_count += 1;
            if image_count > MAX_IMAGE_COUNT || size_bytes > MAX_IMAGE_BYTES {
                return Err(storage_unavailable());
            }
        } else if size_bytes > MAX_TEXT_BYTES {
            return Err(storage_unavailable());
        }
        total = total.saturating_add(size_bytes);
        if total > MAX_ATTACHMENT_BYTES {
            return Err(storage_unavailable());
        }
        attachments.push(AttachmentMetadata {
            id,
            name: name.to_owned(),
            mime_type: mime_type.to_owned(),
            size_bytes,
            kind: kind.to_owned(),
        });
    }
    Ok(AttachmentEnvelope {
        _content: text,
        attachments,
    })
}

fn validate_attachment_metadata(metadata: &AttachmentMetadata) -> Result<(), ServiceError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > 255
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

fn stage_attachments(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachments: &[AttachmentInput],
) -> Result<(PathBuf, PathBuf), ServiceError> {
    let root = checked_directory(storage_root, "attachments", true)?;
    let conversation = checked_child_directory(&root, conversation_id, true)?;
    let destination = conversation.join(message_id);
    if destination.exists() {
        return Err(storage_unavailable());
    }
    let staging = conversation.join(format!(".stage-{message_id}-{}", unix_time_millis()?));
    fs::create_dir(&staging).map_err(|_| storage_unavailable())?;
    for attachment in attachments {
        let path = staging.join(format!("{}.data", attachment.id));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(&attachment.bytes)?;
            file.sync_all()
        })();
        if write_result.is_err() {
            let _ = remove_directory_if_present(&staging);
            return Err(storage_unavailable());
        }
    }
    Ok((staging, destination))
}

fn attachment_path(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
) -> Result<PathBuf, ServiceError> {
    let root = checked_directory(storage_root, "attachments", false)?;
    let conversation = checked_child_directory(&root, conversation_id, false)?;
    let message = checked_child_directory(&conversation, message_id, false)?;
    let path = message.join(format!("{attachment_id}.data"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| storage_unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(storage_unavailable());
    }
    Ok(path)
}

fn attachment_is_available(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    expected_size: usize,
) -> Result<bool, ServiceError> {
    if !is_safe_identifier(conversation_id)
        || !is_safe_identifier(message_id)
        || !is_safe_identifier(attachment_id)
    {
        return Err(invalid_request_params());
    }
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| storage_unavailable())?;
    let Some(attachments_root) =
        optional_checked_directory(&canonical_storage_root, "attachments")?
    else {
        return Ok(false);
    };
    let Some(conversation_directory) =
        optional_checked_directory(&attachments_root, conversation_id)?
    else {
        return Ok(false);
    };
    let Some(message_directory) = optional_checked_directory(&conversation_directory, message_id)?
    else {
        return Ok(false);
    };
    let path = message_directory.join(format!("{attachment_id}.data"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(false);
    }
    if metadata.len() != expected_size as u64 {
        return Ok(false);
    }
    let canonical_path = fs::canonicalize(&path).map_err(|_| storage_unavailable())?;
    if canonical_path != path || !canonical_path.starts_with(&message_directory) {
        return Err(storage_unavailable());
    }
    Ok(true)
}

fn optional_checked_directory(parent: &Path, name: &str) -> Result<Option<PathBuf>, ServiceError> {
    if !is_safe_identifier(name) {
        return Err(invalid_request_params());
    }
    let expected_directory = parent.join(name);
    let metadata = match fs::symlink_metadata(&expected_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&expected_directory).map_err(|_| storage_unavailable())?;
    if canonical_directory != expected_directory || !canonical_directory.starts_with(parent) {
        return Err(storage_unavailable());
    }
    Ok(Some(canonical_directory))
}

fn checked_directory(parent: &Path, name: &str, create: bool) -> Result<PathBuf, ServiceError> {
    let parent = fs::canonicalize(parent).map_err(|_| storage_unavailable())?;
    let path = parent.join(name);
    if create {
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(storage_unavailable()),
        }
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| storage_unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical = fs::canonicalize(&path).map_err(|_| storage_unavailable())?;
    if canonical != path || !canonical.starts_with(&parent) {
        return Err(storage_unavailable());
    }
    Ok(canonical)
}

fn checked_child_directory(
    parent: &Path,
    name: &str,
    create: bool,
) -> Result<PathBuf, ServiceError> {
    if !is_safe_identifier(name) {
        return Err(invalid_request_params());
    }
    checked_directory(parent, name, create)
}

fn remove_attachment_directory(
    storage_root: &Path,
    conversation_id: &str,
) -> Result<(), ServiceError> {
    let attachments = match checked_directory(storage_root, "attachments", false) {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    if !is_safe_identifier(conversation_id) {
        return Err(invalid_request_params());
    }
    let path = attachments.join(conversation_id);
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(storage_unavailable()),
    }
}

fn remove_all_attachment_directories(storage_root: &Path) -> Result<(), ServiceError> {
    let attachments = match checked_directory(storage_root, "attachments", false) {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    for entry in fs::read_dir(&attachments).map_err(|_| storage_unavailable())? {
        let entry = entry.map_err(|_| storage_unavailable())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| storage_unavailable())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(storage_unavailable());
        }
        fs::remove_dir_all(entry.path()).map_err(|_| storage_unavailable())?;
    }
    Ok(())
}

fn remove_directory_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
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

fn input_object(params: &Value) -> Result<&Map<String, Value>, ServiceError> {
    params
        .get("input")
        .unwrap_or(params)
        .as_object()
        .ok_or_else(invalid_request_params)
}

fn required_bounded_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    max_bytes: usize,
    non_empty: bool,
) -> Result<&'a str, ServiceError> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    if value.len() > max_bytes
        || value.chars().any(char::is_control)
        || (non_empty && value.trim().is_empty())
    {
        return Err(invalid_request_params());
    }
    Ok(value.trim())
}

fn optional_bounded_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    max_bytes: usize,
    non_empty: bool,
) -> Result<Option<&'a str>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(_)) => {
            required_bounded_string(object, name, max_bytes, non_empty).map(Some)
        }
        _ => Err(invalid_request_params()),
    }
}

fn required_safe_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)?;
    if !is_safe_identifier(value) {
        return Err(invalid_request_params());
    }
    Ok(value)
}

fn optional_safe_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)? else {
        return Ok(None);
    };
    if !is_safe_identifier(value) {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

fn required_title<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, ServiceError> {
    let title = required_bounded_string(object, name, MAX_TITLE_CHARS * 4, true)?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid_request_params());
    }
    Ok(title)
}

fn required_model_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, MAX_MODEL_ID_CHARS * 4, true)?;
    if value.chars().count() > MAX_MODEL_ID_CHARS {
        return Err(invalid_request_params());
    }
    Ok(value)
}

fn optional_model_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_MODEL_ID_CHARS * 4, true)? else {
        return Ok(None);
    };
    if value.chars().count() > MAX_MODEL_ID_CHARS {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

fn required_id_array(object: &Map<String, Value>, name: &str) -> Result<Vec<String>, ServiceError> {
    let values = object
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(invalid_request_params)?;
    if values.is_empty() || values.len() > MAX_CONVERSATION_IDS {
        return Err(invalid_request_params());
    }
    let mut ids = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let id = value.as_str().ok_or_else(invalid_request_params)?;
        if !is_safe_identifier(id) || !seen.insert(id.to_owned()) {
            return Err(invalid_request_params());
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}

fn optional_json_object<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(_)) => Ok(object.get(name)),
        _ => Err(invalid_request_params()),
    }
}

fn optional_json_array(object: &Map<String, Value>, name: &str) -> Result<String, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok("[]".to_owned()),
        Some(Value::Array(value)) => {
            let encoded = serde_json::to_string(value).map_err(|_| invalid_request_params())?;
            if encoded.len() > MAX_MESSAGE_METADATA_BYTES {
                return Err(invalid_request_params());
            }
            Ok(encoded)
        }
        Some(Value::String(value)) => {
            if value.len() > MAX_MESSAGE_METADATA_BYTES
                || serde_json::from_str::<Value>(value)
                    .map(|v| !v.is_array())
                    .unwrap_or(true)
            {
                return Err(invalid_request_params());
            }
            Ok(value.clone())
        }
        _ => Err(invalid_request_params()),
    }
}

fn required_content<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<String, ServiceError> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    if value.len() > MAX_MESSAGE_CONTENT_BYTES {
        return Err(invalid_request_params());
    }
    Ok(value.to_owned())
}

fn required_message_role<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, 16, true)?;
    if !matches!(value, "user" | "assistant") {
        return Err(invalid_request_params());
    }
    Ok(value)
}

fn required_message_status<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, 16, true)?;
    if !matches!(value, "streaming" | "completed" | "failed" | "stopped") {
        return Err(invalid_request_params());
    }
    Ok(value)
}

fn optional_failure_code<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)? else {
        return Ok(None);
    };
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    }) {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

fn optional_nonnegative_i64(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<i64>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .filter(|value| *value >= 0)
            .map(Some)
            .ok_or_else(invalid_request_params),
        _ => Err(invalid_request_params()),
    }
}

fn optional_nonnegative_f64(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<f64>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(Some)
            .ok_or_else(invalid_request_params),
        _ => Err(invalid_request_params()),
    }
}

fn normalize_search_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            'İ' | 'I' | 'ı' => result.push('i'),
            'Ç' | 'ç' => result.push('c'),
            'Ş' | 'ş' => result.push('s'),
            'Ğ' | 'ğ' => result.push('g'),
            'Ü' | 'ü' => result.push('u'),
            'Ö' | 'ö' => result.push('o'),
            other => result.extend(other.to_lowercase()),
        }
    }
    result
}

fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    let right = right
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    left == right
        || right
            .strip_prefix(&left)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || left
            .strip_prefix(&right)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
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

fn not_found(resource: &str) -> ServiceError {
    ServiceError::new(
        "storage_chat_not_found",
        format!("The requested {resource} was not found."),
        false,
    )
}
