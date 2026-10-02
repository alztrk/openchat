use rusqlite::{OptionalExtension, params, types::Type};

use crate::storage::AppStorage;

use super::{ChatGptModel, unix_time_millis};

pub fn save_models(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    client_version: &str,
    models: &[ChatGptModel],
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let fetched_at = unix_time_millis()?;
    transaction.execute(
        "DELETE FROM chatgpt_models WHERE connection_id = ?1 AND workspace_id = ?2",
        params![connection_id, workspace_id],
    )?;
    for model in models {
        let reasoning_levels = serde_json::to_string(&model.reasoning_levels)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        transaction.execute(
            "INSERT INTO chatgpt_models (
                connection_id, workspace_id, model_id, display_name, description,
                context_window, reasoning_levels_json, is_available, fetched_at_unix_ms,
                default_reasoning_level,
                supports_reasoning_summary_parameter,
                client_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                connection_id,
                workspace_id,
                model.id,
                model.display_name,
                model.description,
                model.context_window,
                reasoning_levels,
                model.is_available,
                fetched_at,
                model.default_reasoning_level,
                model.supports_reasoning_summary_parameter,
                client_version,
            ],
        )?;
    }
    transaction.execute(
        "INSERT INTO chatgpt_model_catalog_state (
            connection_id, workspace_id, fetched_at_unix_ms, client_version
        ) VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(connection_id, workspace_id) DO UPDATE SET
            fetched_at_unix_ms = excluded.fetched_at_unix_ms,
            client_version = excluded.client_version",
        params![connection_id, workspace_id, fetched_at, client_version],
    )?;
    transaction.commit()
}

pub fn list_fresh_models(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    client_version: &str,
    max_age_millis: i64,
) -> rusqlite::Result<Option<Vec<ChatGptModel>>> {
    let database = storage.connect()?;
    let cache_state = database
        .query_row(
            "SELECT fetched_at_unix_ms, client_version
             FROM chatgpt_model_catalog_state
             WHERE connection_id = ?1 AND workspace_id = ?2",
            params![connection_id, workspace_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let now = unix_time_millis()?;
    let Some((fetched_at, cached_version)) = cache_state else {
        return Ok(None);
    };
    if cached_version != client_version || fetched_at < now.saturating_sub(max_age_millis) {
        return Ok(None);
    }
    drop(database);
    list_models(storage, connection_id, workspace_id).map(Some)
}

pub fn list_models(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<Vec<ChatGptModel>> {
    let database = storage.connect()?;
    let mut statement = database.prepare(
        "SELECT model_id, display_name, description, context_window,
                reasoning_levels_json, is_available, default_reasoning_level,
                supports_reasoning_summary_parameter
         FROM chatgpt_models
         WHERE connection_id = ?1 AND workspace_id = ?2
         ORDER BY is_available DESC, model_id COLLATE NOCASE",
    )?;
    let rows = statement.query_map(params![connection_id, workspace_id], |row| {
        let levels = row.get::<_, String>(4)?;
        let reasoning_levels = parse_reasoning_levels(&levels, 4)?;
        let id: String = row.get(0)?;
        Ok(ChatGptModel {
            id: id.clone(),
            display_name: row.get(1)?,
            description: row.get(2)?,
            context_window: row.get(3)?,
            reasoning_levels,
            is_available: row.get(5)?,
            default_reasoning_level: row.get(6)?,
            supports_reasoning_summary_parameter: row.get(7)?,
            supports_images: super::model_supports_images(&id),
        })
    })?;
    rows.collect()
}

pub fn selected_model(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    model_id: &str,
) -> rusqlite::Result<Option<ChatGptModel>> {
    let database = storage.connect()?;
    database
        .query_row(
            "SELECT model_id, display_name, description, context_window,
                reasoning_levels_json, is_available, default_reasoning_level,
                supports_reasoning_summary_parameter
         FROM chatgpt_models
         WHERE connection_id = ?1 AND workspace_id = ?2 AND model_id = ?3",
            params![connection_id, workspace_id, model_id],
            |row| {
                let levels = row.get::<_, String>(4)?;
                let id: String = row.get(0)?;
                Ok(ChatGptModel {
                    id: id.clone(),
                    display_name: row.get(1)?,
                    description: row.get(2)?,
                    context_window: row.get(3)?,
                    reasoning_levels: parse_reasoning_levels(&levels, 4)?,
                    is_available: row.get(5)?,
                    default_reasoning_level: row.get(6)?,
                    supports_reasoning_summary_parameter: row.get(7)?,
                    supports_images: super::model_supports_images(&id),
                })
            },
        )
        .optional()
}

fn parse_reasoning_levels(value: &str, column_index: usize) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str::<Vec<String>>(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Text, Box::new(error))
    })
}
