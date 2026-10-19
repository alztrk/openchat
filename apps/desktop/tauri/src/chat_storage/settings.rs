//! Application settings, personal memory facts, ephemeral cache, and localStorage migration.

use rusqlite::{params, OptionalExtension};

use super::{
    open_database, unix_time_milliseconds, ChatStorageError, LocalStorageMigrationPayload,
    PersonalMemoryFact, MAXIMUM_FACT_CATEGORY_BYTES, MAXIMUM_FACT_ID_BYTES,
    MAXIMUM_FACT_TEXT_BYTES, MAXIMUM_SETTING_KEY_BYTES, MAXIMUM_SETTING_VALUE_BYTES,
};
use crate::request_bounds::BoundedText;

#[tauri::command]
/// Retrieves an application setting value from the unified database.
///
/// # Errors
/// Returns a storage error if the setting cannot be read.
pub fn get_app_setting(
    key: BoundedText<MAXIMUM_SETTING_KEY_BYTES>,
) -> Result<Option<String>, ChatStorageError> {
    let key = key.into_inner();
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Retrieves all application settings from the unified database.
///
/// # Errors
/// Returns a storage error if settings cannot be read.
pub fn get_all_app_settings() -> Result<std::collections::HashMap<String, String>, ChatStorageError>
{
    let connection = open_database()?;
    let mut statement = connection
        .prepare("SELECT key, value_json FROM app_settings")
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            Ok((key, value))
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let (key, value) = row.map_err(|_| ChatStorageError::unavailable())?;
        map.insert(key, value);
    }
    Ok(map)
}

#[tauri::command]
/// Sets an application setting value in the unified database.
///
/// # Errors
/// Returns a storage error if the setting cannot be persisted.
pub fn set_app_setting(
    app: tauri::AppHandle,
    key: BoundedText<MAXIMUM_SETTING_KEY_BYTES>,
    value_json: BoundedText<MAXIMUM_SETTING_VALUE_BYTES>,
) -> Result<(), ChatStorageError> {
    let key = key.into_inner();
    let value_json = value_json.into_inner();
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
    connection
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let _ = crate::window_broadcast::broadcast_app_setting_change(&app, &key, &value_json);
    Ok(())
}

#[tauri::command]
/// Sets a batch of application settings in a single transaction.
///
/// # Errors
/// Returns a storage error if the batch update fails.
pub fn set_app_settings_batch(
    app: tauri::AppHandle,
    entries: std::collections::HashMap<String, String>,
) -> Result<(), ChatStorageError> {
    if entries.len() > 1000 {
        return Err(ChatStorageError::invalid_input());
    }
    let now = unix_time_milliseconds()?;
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    for (key, value) in &entries {
        if key.len() > MAXIMUM_SETTING_KEY_BYTES || value.len() > MAXIMUM_SETTING_VALUE_BYTES {
            return Err(ChatStorageError::invalid_input());
        }
        transaction
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
                params![key, value, now],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;

    for (key, value) in entries {
        let _ = crate::window_broadcast::broadcast_app_setting_change(&app, &key, &value);
    }
    Ok(())
}

#[tauri::command]
/// Deletes an application setting by key.
///
/// # Errors
/// Returns a storage error if the deletion fails.
pub fn delete_app_setting(
    app: tauri::AppHandle,
    key: BoundedText<MAXIMUM_SETTING_KEY_BYTES>,
) -> Result<(), ChatStorageError> {
    let key = key.into_inner();
    let connection = open_database()?;
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", [key.as_str()])
        .map_err(|_| ChatStorageError::unavailable())?;
    let _ = crate::window_broadcast::broadcast_app_setting_change(&app, &key, "null");
    Ok(())
}

pub(crate) fn get_app_setting_raw(key: &str) -> Result<Option<String>, ChatStorageError> {
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())
}

pub(crate) fn set_app_setting_raw(key: &str, value_json: &str) -> Result<(), ChatStorageError> {
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
    connection
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

pub(crate) fn delete_app_setting_raw(key: &str) -> Result<(), ChatStorageError> {
    let connection = open_database()?;
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", [key])
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Lists all personal memory facts from the database.
///
/// # Errors
/// Returns a storage error if facts cannot be loaded.
pub fn list_personal_memory_facts() -> Result<Vec<PersonalMemoryFact>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, fact_text, category, source_conversation_id, created_at, updated_at
             FROM personal_memory_facts ORDER BY updated_at DESC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(PersonalMemoryFact {
                id: row.get(0)?,
                fact_text: row.get(1)?,
                category: row.get(2)?,
                source_conversation_id: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(|_| ChatStorageError::unavailable())?);
    }
    Ok(facts)
}

#[tauri::command]
/// Saves or updates a personal memory fact.
///
/// # Errors
/// Returns a storage error if the fact cannot be saved.
pub fn save_personal_memory_fact(fact: PersonalMemoryFact) -> Result<(), ChatStorageError> {
    if fact.id.trim().is_empty()
        || fact.id.len() > MAXIMUM_FACT_ID_BYTES
        || fact.fact_text.len() > MAXIMUM_FACT_TEXT_BYTES
        || fact
            .category
            .as_ref()
            .is_some_and(|c| c.len() > MAXIMUM_FACT_CATEGORY_BYTES)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
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
                if fact.created_at > 0 {
                    fact.created_at
                } else {
                    now
                },
                now
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Deletes a personal memory fact by ID.
///
/// # Errors
/// Returns a storage error if the deletion fails.
pub fn delete_personal_memory_fact(
    id: BoundedText<MAXIMUM_FACT_ID_BYTES>,
) -> Result<(), ChatStorageError> {
    let id = id.into_inner();
    let connection = open_database()?;
    connection
        .execute("DELETE FROM personal_memory_facts WHERE id = ?1", [id])
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Retrieves an unexpired ephemeral cache entry by key.
///
/// # Errors
/// Returns a storage error if the cache cannot be read.
pub fn get_cache_entry(
    key: BoundedText<MAXIMUM_SETTING_KEY_BYTES>,
) -> Result<Option<String>, ChatStorageError> {
    let key = key.into_inner();
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params![key, now],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Sets an ephemeral cache entry with a time-to-live in seconds.
///
/// # Errors
/// Returns a storage error if the entry cannot be saved.
pub fn set_cache_entry(
    key: BoundedText<MAXIMUM_SETTING_KEY_BYTES>,
    value_json: BoundedText<MAXIMUM_SETTING_VALUE_BYTES>,
    ttl_seconds: i64,
) -> Result<(), ChatStorageError> {
    let key = key.into_inner();
    let value_json = value_json.into_inner();
    if ttl_seconds <= 0 || ttl_seconds > 31_536_000 {
        return Err(ChatStorageError::invalid_input());
    }
    let now = unix_time_milliseconds()?;
    let expires_at = now.saturating_add(ttl_seconds.saturating_mul(1000));
    let connection = open_database()?;
    let _ = connection.execute(
        "DELETE FROM ephemeral_cache WHERE expires_at <= ?1",
        params![now],
    );
    connection
        .execute(
            "INSERT INTO ephemeral_cache (key, value_json, expires_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, expires_at = excluded.expires_at",
            params![key, value_json, expires_at],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Migrates frontend localStorage settings and personal memory facts into SQLite.
///
/// # Errors
/// Returns a storage error if migration fails.
pub fn migrate_localstorage_to_db(
    payload: LocalStorageMigrationPayload,
) -> Result<bool, ChatStorageError> {
    let now = unix_time_milliseconds()?;
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;

    for (key, value) in payload.settings {
        if key.len() > MAXIMUM_SETTING_KEY_BYTES || value.len() > MAXIMUM_SETTING_VALUE_BYTES {
            continue;
        }
        transaction
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
                params![key, value, now],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }

    for fact in payload.facts {
        if fact.id.trim().is_empty()
            || fact.id.len() > MAXIMUM_FACT_ID_BYTES
            || fact.fact_text.len() > MAXIMUM_FACT_TEXT_BYTES
        {
            continue;
        }
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
                    if fact.created_at > 0 {
                        fact.created_at
                    } else {
                        now
                    },
                    now
                ],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }

    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(true)
}
