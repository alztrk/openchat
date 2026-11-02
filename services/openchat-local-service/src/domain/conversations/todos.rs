//! Durable todos scoped to one conversation.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection, StorageRow},
};

const MAX_TODO_TITLE_CHARS: usize = 300;
const MAX_TODO_DESCRIPTION_CHARS: usize = 2_000;
const MAX_TODO_BATCH: usize = 50;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationTodo {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) status: String,
    pub(crate) position: i64,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The todo request is invalid.",
        false,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "Conversation todos are unavailable.",
        true,
    )
}

fn conversation_not_found() -> ServiceError {
    ServiceError::new(
        "conversation_not_found",
        "The conversation was not found.",
        false,
    )
}

fn todo_not_found() -> ServiceError {
    ServiceError::new(
        "todo_not_found",
        "The todo was not found in this conversation.",
        false,
    )
}

fn now_ms() -> Result<i64, ServiceError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| storage_error())?
            .as_millis(),
    )
    .map_err(|_| storage_error())
}

fn validate_conversation(connection: &StorageConnection<'_>, id: &str) -> Result<(), ServiceError> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    let exists = connection
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            [id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| storage_error())?
        .is_some();
    if !exists {
        return Err(conversation_not_found());
    }
    Ok(())
}

fn validate_text(value: &str, maximum: usize) -> Result<String, ServiceError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    Ok(value.to_owned())
}

fn map_todo(row: &StorageRow) -> rusqlite::Result<ConversationTodo> {
    Ok(ConversationTodo {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        position: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub(crate) fn list(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Vec<ConversationTodo>, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    validate_conversation(&connection, conversation_id)?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, title, description, status, position, created_at, updated_at
             FROM conversation_todos WHERE conversation_id = ?1
             ORDER BY position, created_at, id LIMIT 500",
        )
        .map_err(|_| storage_error())?;
    statement
        .query_map([conversation_id], map_todo)
        .map_err(|_| storage_error())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| storage_error())
}

pub(crate) fn create_many(
    storage: &AppStorage,
    conversation_id: &str,
    items: &[Value],
) -> Result<Vec<ConversationTodo>, ServiceError> {
    if items.is_empty() || items.len() > MAX_TODO_BATCH {
        return Err(invalid_request());
    }
    let mut connection = storage.connect().map_err(|_| storage_error())?;
    validate_conversation(&connection, conversation_id)?;
    let transaction = connection.transaction().map_err(|_| storage_error())?;
    let base: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_todos WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|_| storage_error())?;
    let now = now_ms()?;
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(invalid_request());
        };
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "title" | "description"))
        {
            return Err(invalid_request());
        }
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        let title = validate_text(title, MAX_TODO_TITLE_CHARS)?;
        let description = match object.get("description") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(validate_text(value, MAX_TODO_DESCRIPTION_CHARS)?),
            _ => return Err(invalid_request()),
        };
        let position = base
            .checked_add(i64::try_from(index).map_err(|_| invalid_request())?)
            .ok_or_else(invalid_request)?;
        transaction
            .execute(
                "INSERT INTO conversation_todos (id, conversation_id, title, description, status, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?6)",
                params![Uuid::new_v4().simple().to_string(), conversation_id, title, description, position, now],
            )
            .map_err(|_| storage_error())?;
    }
    transaction.commit().map_err(|_| storage_error())?;
    drop(connection);
    list(storage, conversation_id)
}

pub(crate) fn update(
    storage: &AppStorage,
    conversation_id: &str,
    todo_id: &str,
    title: Option<&str>,
    description: Option<Option<&str>>,
    status: Option<&str>,
) -> Result<ConversationTodo, ServiceError> {
    if todo_id.is_empty() || todo_id.len() > 128 || todo_id.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    let title = title
        .map(|value| validate_text(value, MAX_TODO_TITLE_CHARS))
        .transpose()?;
    let description = description
        .map(|value| {
            value
                .map(|text| validate_text(text, MAX_TODO_DESCRIPTION_CHARS))
                .transpose()
        })
        .transpose()?;
    if title.is_none() && description.is_none() && status.is_none() {
        return Err(invalid_request());
    }
    if status.is_some_and(|value| !matches!(value, "open" | "completed" | "cancelled")) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(|_| storage_error())?;
    validate_conversation(&connection, conversation_id)?;
    let changed = connection
        .execute(
            "UPDATE conversation_todos SET
                title = COALESCE(?1, title),
                description = CASE WHEN ?2 = 1 THEN ?3 ELSE description END,
                status = COALESCE(?4, status), updated_at = ?5
             WHERE id = ?6 AND conversation_id = ?7",
            params![
                title,
                description.is_some(),
                description.flatten(),
                status,
                now_ms()?,
                todo_id,
                conversation_id
            ],
        )
        .map_err(|_| storage_error())?;
    if changed != 1 {
        return Err(todo_not_found());
    }
    drop(connection);
    list(storage, conversation_id)?
        .into_iter()
        .find(|todo| todo.id == todo_id)
        .ok_or_else(todo_not_found)
}

pub(crate) fn delete(
    storage: &AppStorage,
    conversation_id: &str,
    todo_id: &str,
) -> Result<(), ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    validate_conversation(&connection, conversation_id)?;
    let changed = connection
        .execute(
            "DELETE FROM conversation_todos WHERE id = ?1 AND conversation_id = ?2",
            params![todo_id, conversation_id],
        )
        .map_err(|_| storage_error())?;
    if changed != 1 {
        return Err(todo_not_found());
    }
    Ok(())
}

pub(crate) fn clear(storage: &AppStorage, conversation_id: &str) -> Result<usize, ServiceError> {
    let mut connection = storage.connect().map_err(|_| storage_error())?;
    validate_conversation(&connection, conversation_id)?;
    let transaction = connection.transaction().map_err(|_| storage_error())?;
    let deleted = transaction
        .execute(
            "DELETE FROM conversation_todos WHERE conversation_id = ?1",
            [conversation_id],
        )
        .map_err(|_| storage_error())?;
    transaction.commit().map_err(|_| storage_error())?;
    Ok(deleted)
}

pub(crate) fn execute_tool(
    storage: &AppStorage,
    conversation_id: &str,
    name: &str,
    arguments: &Value,
) -> Result<Value, ServiceError> {
    let object = arguments.as_object().ok_or_else(invalid_request)?;
    match name {
        "todo_list" if object.is_empty() => Ok(json!({"todos": list(storage, conversation_id)?})),
        "todo_create" if object.len() == 1 && object.contains_key("items") => {
            let items = object
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(invalid_request)?;
            create_many(storage, conversation_id, items)?;
            Ok(json!({"createdCount": items.len()}))
        }
        "todo_update"
            if object.keys().all(|key| {
                matches!(key.as_str(), "todoId" | "title" | "description" | "status")
            }) =>
        {
            let todo_id = object
                .get("todoId")
                .and_then(Value::as_str)
                .ok_or_else(invalid_request)?;
            let title = object
                .get("title")
                .map(|value| value.as_str().ok_or_else(invalid_request))
                .transpose()?;
            let description = object
                .get("description")
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::String(text) => Ok(Some(text.as_str())),
                    _ => Err(invalid_request()),
                })
                .transpose()?;
            let status = object
                .get("status")
                .map(|value| value.as_str().ok_or_else(invalid_request))
                .transpose()?;
            Ok(
                json!({"todo": update(storage, conversation_id, todo_id, title, description, status)?}),
            )
        }
        "todo_delete" if object.len() == 1 => {
            let todo_id = object
                .get("todoId")
                .and_then(Value::as_str)
                .ok_or_else(invalid_request)?;
            delete(storage, conversation_id, todo_id)?;
            Ok(json!({"deleted": true, "todoId": todo_id}))
        }
        "todo_clear" if object.is_empty() => {
            Ok(json!({"deletedCount": clear(storage, conversation_id)?}))
        }
        _ => Err(invalid_request()),
    }
}
