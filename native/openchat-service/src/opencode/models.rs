use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    cancelled_error, client, http_error, invalid_response_error, network_error, storage_error,
};
const MODELS_URL: &str = "https://opencode.ai/inference/v1/models";
const MODEL_CATALOG_CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const SUPPORTED_FREE_CHAT_MODELS: &[&str] = &[
    "big-pickle",
    "deepseek-v4-flash-free",
    "ling-3.0-flash-fin-free",
    "longcat-2.5-preview-free",
    "mimo-v2.5-free",
    "mimo-v2.6-flash-free",
    "nemotron-3-ultra-free",
    "nemotron-3.5-lightning-free",
    "space-bunny-free",
];
const SUPPORTED_PAID_CHAT_MODELS: &[&str] = &[
    "deepseek-v4.1-flash",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "deepseek-v4-flash-vision-exp",
    "minimax-m3",
    "minimax-m2.7",
    "minimax-m2.5",
    "glm-5.3-flash",
    "glm-5.3",
    "glm-5.2",
    "glm-5.1",
    "glm-5",
    "kimi-k2.7-code",
    "kimi-k3",
    "kimi-k2.6",
    "kimi-k2.5",
    "qwen3.8-max",
];

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| storage_error())
}

fn load_model_catalog(storage: &AppStorage) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    let database = storage.connect().map_err(|_| storage_error())?;
    let cached = database
        .query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM opencode_model_catalog WHERE catalog_id = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|_| storage_error())?;
    cached
        .map(|(models_json, fetched_at)| {
            serde_json::from_str::<Vec<Value>>(&models_json)
                .map(|models| (models, fetched_at))
                .map_err(|_| invalid_response_error())
        })
        .transpose()
}

fn save_model_catalog(storage: &AppStorage, models: &[Value]) -> Result<(), ServiceError> {
    let models_json = serde_json::to_string(models).map_err(|_| invalid_response_error())?;
    storage
        .connect()
        .map_err(|_| storage_error())?
        .execute(
            "INSERT INTO opencode_model_catalog (catalog_id, fetched_at_unix_ms, models_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(catalog_id) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                models_json = excluded.models_json",
            rusqlite::params![current_time_millis()?, models_json],
        )
        .map_err(|_| storage_error())?;
    Ok(())
}

fn supported_models(value: &Value) -> Result<Vec<Value>, ServiceError> {
    value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response_error)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let id = model.get("id")?.as_str()?;
                    let is_free = is_supported_free_chat_model(id);
                    if !is_free && !is_supported_paid_chat_model(id) {
                        return None;
                    }
                    Some(json!({
                        "id": id,
                        "displayName": id,
                        "description": if is_free { "free" } else { "paid" },
                        "contextWindow": null,
                        "defaultReasoningLevel": null,
                        "reasoningLevels": [],
                        "isAvailable": true,
                    }))
                })
                .collect()
        })
}

fn visible_models(models: &[Value], api_key: Option<&str>) -> Vec<Value> {
    models
        .iter()
        .filter(|model| {
            model.get("description").and_then(Value::as_str) != Some("paid") || api_key.is_some()
        })
        .cloned()
        .collect()
}

fn models_response(models: &[Value], freshness: &str) -> Value {
    json!({"models": models, "freshness": freshness})
}

pub async fn models(
    storage: &AppStorage,
    api_key: Option<&str>,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let cached = load_model_catalog(storage)?;
    if !force_refresh
        && let Some((models, fetched_at)) = &cached
        && current_time_millis()?.saturating_sub(*fetched_at) < MODEL_CATALOG_CACHE_AGE_MS
    {
        return Ok(models_response(&visible_models(models, api_key), "current"));
    }

    let mut request = client()?.get(MODELS_URL);
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    let fetch = async {
        let response = request.send().await.map_err(|_| network_error())?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| invalid_response_error())?;
        let models = supported_models(&value)?;
        save_model_catalog(storage, &models)?;
        Ok::<_, ServiceError>(models)
    };
    let fetch_result = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Err(cancelled_error())
        }
        result = fetch => result,
    };

    match fetch_result {
        Ok(models) => Ok(models_response(
            &visible_models(&models, api_key),
            "current",
        )),
        Err(error) if error.code == "request_cancelled" => Err(error),
        Err(error) => match cached {
            Some((models, _)) => Ok(models_response(&visible_models(&models, api_key), "stale")),
            None => Err(error),
        },
    }
}

pub(super) fn is_supported_free_chat_model(id: &str) -> bool {
    SUPPORTED_FREE_CHAT_MODELS.contains(&id)
}

pub(super) fn is_supported_paid_chat_model(id: &str) -> bool {
    SUPPORTED_PAID_CHAT_MODELS.contains(&id)
}
