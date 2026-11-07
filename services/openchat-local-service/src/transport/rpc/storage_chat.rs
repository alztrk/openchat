//! RPC handlers for the durable conversation database.
//!
//! The renderer used to call Tauri commands that opened the SQLite file directly.  These
//! handlers are the storage boundary used while that migration is completed.  Every branch
//! acquires a queued connection for a bounded synchronous operation and drops it before the
//! dispatcher can await another service operation.

pub(super) use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) use base64::{Engine as _, engine::general_purpose::STANDARD};
pub(super) use rusqlite::{OptionalExtension, params};
pub(super) use serde_json::{Map, Value, json};
pub(super) use zeroize::{Zeroize, Zeroizing};

pub(super) use crate::{
    conversation_limits::{
        MAXIMUM_ATTACHMENT_FILE_COUNT as MAX_ATTACHMENT_COUNT,
        MAXIMUM_ATTACHMENT_IMAGE_BYTES as MAX_IMAGE_BYTES,
        MAXIMUM_ATTACHMENT_IMAGE_COUNT as MAX_IMAGE_COUNT,
        MAXIMUM_ATTACHMENT_KIND_BYTES as MAX_ATTACHMENT_KIND_BYTES,
        MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES as MAX_ATTACHMENT_MIME_BYTES,
        MAXIMUM_ATTACHMENT_NAME_BYTES as MAX_ATTACHMENT_NAME_BYTES, MAXIMUM_ATTACHMENT_NAME_CHARS,
        MAXIMUM_ATTACHMENT_TEXT_BYTES as MAX_TEXT_BYTES,
        MAXIMUM_ATTACHMENT_TOTAL_BYTES as MAX_ATTACHMENT_BYTES,
        MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES as MAX_MESSAGE_CONTENT_BYTES,
        MAXIMUM_CHAT_MESSAGE_METADATA_BYTES as MAX_MESSAGE_METADATA_BYTES, MESSAGE_CONTENT_PREFIX,
    },
    protocol::{
        MAX_REQUEST_PARAMETER_NODES, ServiceError, clear_sensitive_json_values,
        parse_bounded_json_value,
    },
    service::OperationGate,
    storage::{
        AppStorage, StorageConnection, StorageRow, is_retryable_storage_error_code,
        sqlite_error_code,
    },
};

pub(super) use super::invalid_request_params;

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TITLE_CHARS: usize = 512;
const MAX_MODEL_ID_CHARS: usize = 512;
const MAX_AGENT_METADATA_BYTES: usize = 128 * 1024;
const MAX_EMBEDDED_METADATA_JSON_DEPTH: usize = 4;
const MAX_EMBEDDED_METADATA_JSON_BYTES: usize = 4_000_000;
const MAX_STRUCTURED_METADATA_JSON_NESTING: usize = 128;
const SENSITIVE_METADATA_KEY_MARKERS: [&str; 5] =
    ["apiKey", "secret", "passphrase", "secrets", "credential"];
const MAX_CONVERSATION_IDS: usize = 500;
const MAX_PROJECT_NAME_BYTES: usize = MAX_TITLE_CHARS * 4;
const MAX_WORKSPACE_ROOT_BYTES: usize = 32 * 1024;
const MAX_SEARCH_QUERY_BYTES: usize = 512;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_ATTACHMENT_SCAN: usize = 10_000;
const MAX_ATTACHMENT_PAGE: usize = 100;
const MAX_MESSAGE_PAGE: usize = 100;
const ATTACHMENT_TEXT_INDEX_SETTING: &str = "attachment_text_index.enabled";
const MAX_ATTACHMENT_QUERY_CHARS: usize = 512;
const MAX_ATTACHMENT_OFFSET: u64 = 1_000_000;
const ATTACHMENT_LIBRARY_BATCH_SIZE: u64 = 50;
pub(super) const MAX_CONVERSATION_PAGE: usize = 64;
pub(super) const MAX_PROJECT_PAGE: usize = 32;
pub(super) const MAX_LIST_OFFSET: usize = 1_000_000;
pub(super) const MAX_MESSAGE_PAGE_RESPONSE_BYTES: usize = 12 * 1024 * 1024;
pub(super) const MAX_MESSAGE_SEARCH_RESPONSE_BYTES: usize = 12 * 1024 * 1024;

macro_rules! conversation_from_row {
    ($row:expr) => {{
        let row = $row;
        let agent_json = row.get::<_, Option<String>>(13)?;
        let agent = agent_json.map(parse_stored_agent_json).transpose()?;
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
            "reasoningEffort": row.get::<_, Option<String>>(14)?,
            "unreadMessageId": row.get::<_, Option<String>>(15)?,
            "agent": agent,
        }))
    }};
}

#[path = "storage_chat/attachments.rs"]
mod attachments;
#[path = "storage_chat/conversations.rs"]
mod conversations;
#[path = "storage_chat/errors.rs"]
mod errors;
#[path = "storage_chat/projects.rs"]
mod projects;
#[path = "storage_chat/todos.rs"]
mod todos;
#[path = "storage_chat/validation.rs"]
mod validation;

use attachments::*;
pub(super) use attachments::{
    cleanup_orphaned_attachment_directories,
    cleanup_orphaned_attachment_directories_with_cancellation,
    recover_abandoned_attachment_staging_directories, remove_attachment_directory,
};
use conversations::*;
use errors::*;
use projects::*;
use validation::*;

pub(super) fn list_offset(params: &Value) -> Result<usize, ServiceError> {
    if params.is_null() {
        return Ok(0);
    }
    let input = input_object(params)?;
    let offset = input
        .get("offset")
        .map(|value| {
            value
                .as_u64()
                .and_then(|offset| usize::try_from(offset).ok())
                .filter(|offset| *offset <= MAX_LIST_OFFSET)
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(0);
    Ok(offset)
}

/// Dispatches durable conversation, project, message, favorite, and attachment storage calls.
#[cfg(test)]
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    dispatch_inner(method, params, storage, None).await
}

/// Dispatches chat-storage requests with the operation cancellation gate used by the RPC host.
pub(super) async fn dispatch_with_gate(
    method: &str,
    params: &Value,
    storage: &AppStorage,
    cancellation_gate: &OperationGate,
) -> Result<Value, ServiceError> {
    dispatch_inner(method, params, storage, Some(cancellation_gate)).await
}

async fn dispatch_inner(
    method: &str,
    params: &Value,
    storage: &AppStorage,
    cancellation_gate: Option<&OperationGate>,
) -> Result<Value, ServiceError> {
    match method {
        "storage.chat.conversations.list" => list_conversations(params, storage),
        "storage.chat.conversations.get" => get_conversation(params, storage),
        "storage.chat.conversations.unread.set" => set_conversation_unread(params, storage),
        "storage.chat.conversations.unread.clear" => clear_conversation_unread(params, storage),
        "storage.chat.conversations.create" => create_conversation(params, storage),
        "storage.chat.conversations.rename" => rename_conversation(params, storage),
        "storage.chat.conversations.route.set" => set_conversation_route(params, storage),
        "storage.chat.conversations.reasoning.set" => set_conversation_reasoning(params, storage),
        "storage.chat.conversations.pinned.set" => set_conversation_pinned(params, storage),
        "storage.chat.conversations.project.set" => move_conversations(params, storage),
        "storage.chat.conversations.archived.set" => set_conversations_archived(params, storage),
        "storage.chat.conversations.delete" => delete_conversation(params, storage),
        "storage.chat.conversations.clear" => clear_conversation_history(storage),
        "storage.chat.projects.list" => list_projects(params, storage),
        "storage.chat.projects.create" => create_project(params, storage),
        "storage.chat.projects.workspace.add" => add_project_workspace_root(params, storage),
        "storage.chat.projects.workspace.remove" => remove_project_workspace_root(params, storage),
        "storage.chat.messages.list" => list_messages(params, storage),
        "storage.chat.messages.save" => save_message(params, storage, cancellation_gate),
        "storage.chat.messages.search" => search_messages(params, storage),
        "storage.chat.responses.has_completed" => has_completed_assistant_response(storage),
        "storage.chat.favorites.list" => list_model_favorites(storage),
        "storage.chat.favorites.set" => set_model_favorite(params, storage),
        "storage.chat.todos.list" => todos::list(params, storage),
        "storage.chat.todos.create" => todos::create(params, storage),
        "storage.chat.todos.update" => todos::update(params, storage),
        "storage.chat.todos.delete" => todos::delete(params, storage),
        "storage.chat.todos.clear" => todos::clear(params, storage),
        "storage.chat.attachments.read" => read_message_attachment(params, storage),
        "storage.chat.attachments.list" => list_conversation_attachments(params, storage),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

#[path = "storage_chat/tests.rs"]
#[cfg(test)]
mod tests;
