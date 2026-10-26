//! RPC handlers for renderer-owned settings, personal-memory facts, and ephemeral cache data.
//!
//! The handlers deliberately acquire a queued database connection only for the synchronous
//! SQLite operation. They do not await while holding the connection, so a slow provider request
//! cannot occupy the database queue.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

use crate::{
    protocol::{EventSink, Response, ServiceError},
    storage::{AppStorage, sqlite_error_code},
};

use super::invalid_request_params;

const MAX_SETTING_KEY_BYTES: usize = 128;
const MAX_SETTING_VALUE_BYTES: usize = 2 * 1024 * 1024;
const MAX_FACT_ID_BYTES: usize = 128;
const MAX_FACT_TEXT_BYTES: usize = 64 * 1024;
const MAX_FACT_CATEGORY_BYTES: usize = 128;
const MAX_SOURCE_CONVERSATION_ID_BYTES: usize = 128;
const MAX_BATCH_ENTRIES: usize = 1_000;
const MAX_PERSONAL_MEMORY_FACTS: usize = 100;
const MAX_CACHE_TTL_SECONDS: i64 = 31_536_000;

const SETTINGS_ERROR_CODE: &str = "storage_settings_unavailable";
const PERSONAL_MEMORY_ERROR_CODE: &str = "personal_memory_storage_unavailable";
const CACHE_ERROR_CODE: &str = "ephemeral_cache_unavailable";

/// Dispatches settings, personal-memory, and ephemeral-cache requests.
///
/// This function is async only to match the RPC dispatcher contract. Every branch performs its
/// complete synchronous database operation before returning and never awaits with a queued
/// connection alive.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    match method {
        "storage.settings.get" => get_setting(params, storage),
        "storage.settings.get_all" => {
            get_all_settings(storage).map(|settings| json!({"settings": settings}))
        }
        "storage.settings.set" => set_setting(params, storage, events).await,
        "storage.settings.set_batch" => set_settings_batch(params, storage, events).await,
        "storage.settings.delete" => delete_setting(params, storage, events).await,
        "storage.settings.validate_workspace" => validate_workspace_setting(params),
        "storage.personal_memory.list" => list_personal_memory_facts(storage),
        "storage.personal_memory.save" => save_personal_memory_fact(params, storage),
        "storage.personal_memory.delete" => delete_personal_memory_fact(params, storage),
        "storage.personal_memory.clear" => clear_personal_memory_facts(storage, events).await,
        "storage.cache.get" => get_cache_entry(params, storage),
        "storage.cache.set" => set_cache_entry(params, storage),
        "storage.settings.migrate_localstorage" => {
            migrate_localstorage(params, storage, events).await
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn get_setting(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let key = required_setting_key(params, "key")?;
    let connection = storage
        .connect()
        .map_err(|error| settings_sqlite_error(error))?;
    let value = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(settings_sqlite_error)?;
    Ok(json!({"valueJson": value}))
}

/// Validates a renderer-selected fallback working directory.
///
/// The returned value is the prompt-safe form, so the renderer stores the same
/// path the service would report to a provider.
fn validate_workspace_setting(params: &Value) -> Result<Value, ServiceError> {
    let value = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    let workspace =
        crate::workspace_settings::validated_workspace_path(std::path::Path::new(value.trim()))
            .ok_or_else(invalid_request_params)?;
    let path = crate::workspace_settings::prompt_workspace_path(&workspace)
        .ok_or_else(invalid_request_params)?;
    Ok(json!({"path": path}))
}

pub(super) fn get_all_settings(storage: &AppStorage) -> Result<Map<String, Value>, ServiceError> {
    let connection = storage
        .connect()
        .map_err(|error| settings_sqlite_error(error))?;
    let mut statement = connection
        .prepare("SELECT key, value_json FROM app_settings")
        .map_err(settings_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(settings_sqlite_error)?;
    let mut settings = Map::new();
    for row in rows {
        let (key, value) = row.map_err(settings_sqlite_error)?;
        settings.insert(key, Value::String(value));
    }
    Ok(settings)
}

async fn set_setting(
    params: &Value,
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let key = required_setting_key(params, "key")?;
    let value_json = required_setting_value(params, "valueJson")?;
    let now = current_unix_millis().map_err(|detail| settings_clock_error(detail))?;
    {
        let connection = storage
            .connect()
            .map_err(|error| settings_sqlite_error(error))?;
        connection
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                     updated_at = excluded.updated_at",
                params![key, value_json, now],
            )
            .map_err(settings_sqlite_error)?;
    }
    broadcast_setting_change(events, &key, &value_json).await?;
    Ok(json!({"saved": true}))
}

async fn set_settings_batch(
    params: &Value,
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let entries = parse_setting_entries(params, "entries")?;
    let now = current_unix_millis().map_err(|detail| settings_clock_error(detail))?;
    {
        let mut connection = storage
            .connect()
            .map_err(|error| settings_sqlite_error(error))?;
        let transaction = connection.transaction().map_err(settings_sqlite_error)?;
        for (key, value_json) in &entries {
            transaction
                .execute(
                    "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                         updated_at = excluded.updated_at",
                    params![key, value_json, now],
                )
                .map_err(settings_sqlite_error)?;
        }
        transaction.commit().map_err(settings_sqlite_error)?;
    }
    for (key, value_json) in &entries {
        broadcast_setting_change(events, key, value_json).await?;
    }
    Ok(json!({"saved": entries.len()}))
}

async fn delete_setting(
    params: &Value,
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let key = required_setting_key(params, "key")?;
    let deleted = {
        let connection = storage
            .connect()
            .map_err(|error| settings_sqlite_error(error))?;
        connection
            .execute("DELETE FROM app_settings WHERE key = ?1", [key])
            .map_err(settings_sqlite_error)?
    };
    broadcast_setting_change(events, &key, "null").await?;
    Ok(json!({"deleted": deleted > 0}))
}

fn list_personal_memory_facts(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage
        .connect()
        .map_err(|error| personal_memory_sqlite_error(error))?;
    let mut statement = connection
        .prepare(
            "SELECT id, fact_text, category, source_conversation_id, created_at, updated_at
             FROM personal_memory_facts ORDER BY updated_at DESC",
        )
        .map_err(personal_memory_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "factText": row.get::<_, String>(1)?,
                "category": row.get::<_, Option<String>>(2)?,
                "sourceConversationId": row.get::<_, Option<String>>(3)?,
                "createdAt": row.get::<_, i64>(4)?,
                "updatedAt": row.get::<_, i64>(5)?,
            }))
        })
        .map_err(personal_memory_sqlite_error)?;
    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(personal_memory_sqlite_error)?);
        if facts.len() > MAX_PERSONAL_MEMORY_FACTS {
            return Err(personal_memory_unavailable_with_detail(
                "stored_personal_memory_count_exceeded",
            ));
        }
    }
    Ok(json!({"facts": facts}))
}

fn save_personal_memory_fact(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let fact_value = params.get("fact").unwrap_or(params);
    let fact = parse_personal_memory_fact(fact_value)?;
    let now = current_unix_millis().map_err(|detail| personal_memory_clock_error(detail))?;
    let created_at = if fact.created_at > 0 {
        fact.created_at
    } else {
        now
    };
    let connection = storage
        .connect()
        .map_err(|error| personal_memory_sqlite_error(error))?;
    connection
        .execute(
            "INSERT INTO personal_memory_facts
             (id, fact_text, category, source_conversation_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                fact_text = excluded.fact_text,
                category = excluded.category,
                source_conversation_id = excluded.source_conversation_id,
                updated_at = excluded.updated_at",
            params![
                fact.id,
                fact.fact_text,
                fact.category,
                fact.source_conversation_id,
                created_at,
                now,
            ],
        )
        .map_err(personal_memory_sqlite_error)?;
    Ok(json!({"saved": true}))
}

fn delete_personal_memory_fact(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let id = required_bounded_string(params, "id", MAX_FACT_ID_BYTES, true)?;
    let connection = storage
        .connect()
        .map_err(|error| personal_memory_sqlite_error(error))?;
    let deleted = connection
        .execute("DELETE FROM personal_memory_facts WHERE id = ?1", [id])
        .map_err(personal_memory_sqlite_error)?;
    Ok(json!({"deleted": deleted > 0}))
}

async fn clear_personal_memory_facts(
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let now = current_unix_millis().map_err(|detail| personal_memory_clock_error(detail))?;
    {
        let mut connection = storage
            .connect()
            .map_err(|error| personal_memory_sqlite_error(error))?;
        let transaction = connection
            .transaction()
            .map_err(personal_memory_sqlite_error)?;
        transaction
            .execute("DELETE FROM personal_memory_facts", [])
            .map_err(personal_memory_sqlite_error)?;
        transaction
            .execute(
                "DELETE FROM app_settings WHERE key = 'privacy.personal_memory_facts'",
                [],
            )
            .map_err(personal_memory_sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('privacy.personal_memory_enabled', 'false', ?1)
                 ON CONFLICT(key) DO UPDATE SET value_json = 'false', updated_at = excluded.updated_at",
                [now],
            )
            .map_err(personal_memory_sqlite_error)?;
        transaction.commit().map_err(personal_memory_sqlite_error)?;
    }
    broadcast_setting_change(events, "privacy.personal_memory_enabled", "false").await?;
    broadcast_setting_change(events, "privacy.personal_memory_facts", "null").await?;
    Ok(json!({"cleared": true}))
}

fn get_cache_entry(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let key = required_setting_key(params, "key")?;
    let now = current_unix_millis().map_err(|detail| cache_clock_error(detail))?;
    let connection = storage
        .connect()
        .map_err(|error| cache_sqlite_error(error))?;
    let value = connection
        .query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params![key, now],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(cache_sqlite_error)?;
    Ok(json!({"valueJson": value}))
}

fn set_cache_entry(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let key = required_setting_key(params, "key")?;
    let value_json = required_setting_value(params, "valueJson")?;
    let ttl_seconds = params
        .get("ttlSeconds")
        .and_then(Value::as_i64)
        .filter(|value| (1..=MAX_CACHE_TTL_SECONDS).contains(value))
        .ok_or_else(invalid_request_params)?;
    let now = current_unix_millis().map_err(|detail| cache_clock_error(detail))?;
    let expires_at = now.saturating_add(ttl_seconds.saturating_mul(1_000));
    let mut connection = storage
        .connect()
        .map_err(|error| cache_sqlite_error(error))?;
    let transaction = connection.transaction().map_err(cache_sqlite_error)?;
    transaction
        .execute("DELETE FROM ephemeral_cache WHERE expires_at <= ?1", [now])
        .map_err(cache_sqlite_error)?;
    transaction
        .execute(
            "INSERT INTO ephemeral_cache (key, value_json, expires_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                 expires_at = excluded.expires_at",
            params![key, value_json, expires_at],
        )
        .map_err(cache_sqlite_error)?;
    transaction.commit().map_err(cache_sqlite_error)?;
    Ok(json!({"saved": true}))
}

async fn migrate_localstorage(
    params: &Value,
    storage: &AppStorage,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let settings = parse_setting_entries(params, "settings")?;
    let facts_value = params
        .get("facts")
        .and_then(Value::as_array)
        .ok_or_else(invalid_request_params)?;
    if facts_value.len() > MAX_PERSONAL_MEMORY_FACTS {
        return Err(invalid_request_params());
    }
    let facts = facts_value
        .iter()
        .map(parse_personal_memory_fact)
        .collect::<Result<Vec<_>, _>>()?;
    let now = current_unix_millis().map_err(|detail| settings_clock_error(detail))?;
    {
        let mut connection = storage
            .connect()
            .map_err(|error| settings_sqlite_error(error))?;
        let transaction = connection.transaction().map_err(settings_sqlite_error)?;
        for (key, value_json) in &settings {
            transaction
                .execute(
                    "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                         updated_at = excluded.updated_at",
                    params![key, value_json, now],
                )
                .map_err(settings_sqlite_error)?;
        }
        for fact in &facts {
            let created_at = if fact.created_at > 0 {
                fact.created_at
            } else {
                now
            };
            transaction
                .execute(
                    "INSERT INTO personal_memory_facts
                     (id, fact_text, category, source_conversation_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(id) DO UPDATE SET
                        fact_text = excluded.fact_text,
                        category = excluded.category,
                        source_conversation_id = excluded.source_conversation_id,
                        updated_at = excluded.updated_at",
                    params![
                        fact.id,
                        fact.fact_text,
                        fact.category,
                        fact.source_conversation_id,
                        created_at,
                        now,
                    ],
                )
                .map_err(settings_sqlite_error)?;
        }
        transaction.commit().map_err(settings_sqlite_error)?;
    }
    for (key, value_json) in &settings {
        broadcast_setting_change(events, key, value_json).await?;
    }
    Ok(json!({"migrated": true, "settings": settings.len(), "facts": facts.len()}))
}

async fn broadcast_setting_change(
    events: &EventSink,
    key: &str,
    value_json: &str,
) -> Result<(), ServiceError> {
    events
        .send(&Response::event(
            json!(0),
            "app.setting.changed",
            json!({"key": key, "value_json": value_json}),
        ))
        .await
        .map_err(|_| {
            ServiceError::new(
                "storage_event_unavailable",
                "A saved setting could not be synchronized with other application windows.",
                true,
            )
        })
}

fn parse_setting_entries(
    params: &Value,
    name: &str,
) -> Result<Vec<(String, String)>, ServiceError> {
    let entries = params
        .get(name)
        .and_then(Value::as_object)
        .ok_or_else(invalid_request_params)?;
    if entries.len() > MAX_BATCH_ENTRIES {
        return Err(invalid_request_params());
    }
    entries
        .iter()
        .map(|(key, value)| {
            validate_setting_key(key)?;
            let value = value.as_str().ok_or_else(invalid_request_params)?;
            validate_setting_value(value)?;
            Ok((key.clone(), value.to_owned()))
        })
        .collect()
}

fn required_setting_key<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    let value = params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    validate_setting_key(value)?;
    Ok(value)
}

fn required_setting_value<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    let value = params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    validate_setting_value(value)?;
    Ok(value)
}

fn validate_setting_key(value: &str) -> Result<(), ServiceError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_SETTING_KEY_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(invalid_request_params());
    }
    Ok(())
}

fn validate_setting_value(value: &str) -> Result<(), ServiceError> {
    if value.len() > MAX_SETTING_VALUE_BYTES {
        return Err(invalid_request_params());
    }
    Ok(())
}

fn required_bounded_string<'a>(
    params: &'a Value,
    name: &str,
    maximum_bytes: usize,
    reject_controls: bool,
) -> Result<&'a str, ServiceError> {
    let value = params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > maximum_bytes
        || (reject_controls && value.chars().any(char::is_control))
    {
        return Err(invalid_request_params());
    }
    Ok(value)
}

#[derive(Debug)]
struct PersonalMemoryFactInput {
    id: String,
    fact_text: String,
    category: Option<String>,
    source_conversation_id: Option<String>,
    created_at: i64,
}

fn parse_personal_memory_fact(value: &Value) -> Result<PersonalMemoryFactInput, ServiceError> {
    let id = required_bounded_string(value, "id", MAX_FACT_ID_BYTES, true)?.to_owned();
    let fact_text =
        required_bounded_string(value, "factText", MAX_FACT_TEXT_BYTES, false)?.to_owned();
    let category = optional_bounded_string(value, "category", MAX_FACT_CATEGORY_BYTES, true)?;
    let source_conversation_id = optional_bounded_string(
        value,
        "sourceConversationId",
        MAX_SOURCE_CONVERSATION_ID_BYTES,
        true,
    )?;
    let created_at = optional_nonnegative_i64(value, "createdAt")?.unwrap_or(0);
    Ok(PersonalMemoryFactInput {
        id,
        fact_text,
        category,
        source_conversation_id,
        created_at,
    })
}

fn optional_bounded_string(
    params: &Value,
    name: &str,
    maximum_bytes: usize,
    reject_controls: bool,
) -> Result<Option<String>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            if value.trim().is_empty()
                || value != value.trim()
                || value.len() > maximum_bytes
                || (reject_controls && value.chars().any(char::is_control))
            {
                return Err(invalid_request_params());
            }
            Ok(Some(value.clone()))
        }
        _ => Err(invalid_request_params()),
    }
}

fn optional_nonnegative_i64(params: &Value, name: &str) -> Result<Option<i64>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_i64()
            .filter(|value| *value >= 0)
            .map(Some)
            .ok_or_else(invalid_request_params),
        _ => Err(invalid_request_params()),
    }
}

fn current_unix_millis() -> Result<i64, &'static str> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock_before_unix_epoch")?;
    i64::try_from(duration.as_millis()).map_err(|_| "clock_value_overflow")
}

fn settings_sqlite_error(error: rusqlite::Error) -> ServiceError {
    settings_unavailable_with_detail(sqlite_error_code(&error))
}

fn personal_memory_sqlite_error(error: rusqlite::Error) -> ServiceError {
    personal_memory_unavailable_with_detail(sqlite_error_code(&error))
}

fn cache_sqlite_error(error: rusqlite::Error) -> ServiceError {
    cache_unavailable_with_detail(sqlite_error_code(&error))
}

fn settings_clock_error(detail: &'static str) -> ServiceError {
    settings_unavailable_with_detail(detail)
}

fn personal_memory_clock_error(detail: &'static str) -> ServiceError {
    personal_memory_unavailable_with_detail(detail)
}

fn cache_clock_error(detail: &'static str) -> ServiceError {
    cache_unavailable_with_detail(detail)
}

fn settings_unavailable_with_detail(detail: &'static str) -> ServiceError {
    ServiceError::new(
        SETTINGS_ERROR_CODE,
        "Application settings could not be read or saved.",
        true,
    )
    .with_details(detail)
}

fn personal_memory_unavailable_with_detail(detail: &'static str) -> ServiceError {
    ServiceError::new(
        PERSONAL_MEMORY_ERROR_CODE,
        "Personal memory could not be read or saved locally.",
        true,
    )
    .with_details(detail)
}

fn cache_unavailable_with_detail(detail: &'static str) -> ServiceError {
    ServiceError::new(
        CACHE_ERROR_CODE,
        "The ephemeral local cache could not be read or saved.",
        true,
    )
    .with_details(detail)
}
