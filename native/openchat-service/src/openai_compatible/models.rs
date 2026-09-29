use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    cancelled_error, client, http_error, invalid_response_error, network_error, storage_error,
};
pub(crate) const MODELS_URL: &str = "https://opencode.ai/zen/v1/models";
const MODELS_DEV_URL: &str = "https://models.opencode.ai/api.json";
const MODEL_CATALOG_CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MODEL_METADATA_TIMEOUT: Duration = Duration::from_secs(5);
const SUPPORTED_FREE_CHAT_MODELS: &[&str] = &[
    "big-pickle",
    "deepseek-v4-flash-free",
    "jev-1.13-free",
    "ling-3.0-flash-fin-free",
    "longcat-2.5-preview-free",
    "mimo-v2.5-free",
    "mimo-v2.6-flash-free",
    "muse-spark-1.2-contributor-free",
    "muse-spark-1.3-contributor-free",
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

fn model_group(id: &str) -> Option<&'static str> {
    if is_supported_free_chat_model(id) {
        Some("free")
    } else if is_supported_paid_chat_model(id) {
        Some("paid")
    } else {
        None
    }
}

fn metadata_models(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.get("opencode")?.get("models")?.as_object()
}

async fn fetch_model_metadata(client: &Client) -> Result<Value, ServiceError> {
    let response = client
        .get(MODELS_DEV_URL)
        .header("User-Agent", "opencode/1.18.30")
        .timeout(MODEL_METADATA_TIMEOUT)
        .send()
        .await
        .map_err(|_| network_error())?;
    if !response.status().is_success() {
        return Err(invalid_response_error());
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| invalid_response_error())?;
    if metadata_models(&value).is_none() {
        return Err(invalid_response_error());
    }
    Ok(value)
}

fn supported_models(value: &Value, metadata: Option<&Value>) -> Result<Vec<Value>, ServiceError> {
    let metadata_models = metadata.and_then(metadata_models);
    value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response_error)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let id = model.get("id")?.as_str()?;
                    let group_id = model_group(id)?;
                    let details = metadata_models.and_then(|models| models.get(id));
                    let display_name = details
                        .and_then(|details| details.get("name"))
                        .and_then(Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or(id);
                    let description = details
                        .and_then(|details| details.get("description"))
                        .and_then(Value::as_str)
                        .filter(|description| !description.trim().is_empty());
                    let context_window = details
                        .and_then(|details| details.get("limit"))
                        .and_then(|limits| limits.get("context"))
                        .and_then(Value::as_i64)
                        .filter(|context_window| *context_window > 0);
                    Some(json!({
                        "id": id,
                        "displayName": display_name,
                        "description": description,
                        "contextWindow": context_window,
                        "groupId": group_id,
                        "defaultReasoningLevel": null,
                        "reasoningLevels": [],
                        "isAvailable": true,
                    }))
                })
                .collect()
        })
}

fn canonicalize_cached_models(models: &[Value]) -> Vec<Value> {
    models
        .iter()
        .filter_map(|model| {
            let id = model.get("id")?.as_str()?;
            let group_id = model_group(id)?;
            let mut model = model.clone();
            let fields = model.as_object_mut()?;
            fields.insert("groupId".to_owned(), json!(group_id));
            if matches!(
                fields.get("description").and_then(Value::as_str),
                Some("free" | "paid")
            ) {
                fields.remove("description");
            }
            Some(model)
        })
        .collect()
}

fn has_model_groups(models: &[Value]) -> bool {
    !models.is_empty()
        && models.iter().all(|model| {
            model
                .get("id")
                .and_then(Value::as_str)
                .and_then(model_group)
                == model.get("groupId").and_then(Value::as_str)
        })
}

fn visible_models(models: &[Value], api_key: Option<&str>) -> Vec<Value> {
    models
        .iter()
        .filter(|model| {
            model.get("groupId").and_then(Value::as_str) != Some("paid") || api_key.is_some()
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
        && has_model_groups(models)
        && current_time_millis()?.saturating_sub(*fetched_at) < MODEL_CATALOG_CACHE_AGE_MS
    {
        return Ok(models_response(&visible_models(models, api_key), "current"));
    }

    let http_client = client()?;
    let session_id = super::opencode_session_id();
    let mut request = http_client
        .get(MODELS_URL)
        .header("User-Agent", "opencode/1.18.30")
        .header("x-opencode-client", "cli")
        .header("x-opencode-session", session_id)
        .header("x-opencode-project", "global");
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    } else {
        request = request.bearer_auth("public");
    }
    let fetch = async {
        let provider_fetch = async {
            let response = request.send().await.map_err(|_| network_error())?;
            if !response.status().is_success() {
                return Err(http_error(response.status(), Some("opencode")));
            }
            response
                .json::<Value>()
                .await
                .map_err(|_| invalid_response_error())
        };
        let optional_metadata =
            async { Ok::<_, ServiceError>(fetch_model_metadata(&http_client).await.ok()) };
        let (provider_catalog, metadata) = tokio::try_join!(provider_fetch, optional_metadata)?;
        let models = supported_models(&provider_catalog, metadata.as_ref())?;
        let freshness = if metadata.is_some() {
            save_model_catalog(storage, &models)?;
            "current"
        } else if let Some((cached_models, _)) = &cached {
            return Ok::<_, ServiceError>((canonicalize_cached_models(cached_models), "stale"));
        } else {
            "current"
        };
        Ok::<_, ServiceError>((models, freshness))
    };
    let fetch_result = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Err(cancelled_error())
        }
        result = fetch => result,
    };

    match fetch_result {
        Ok((models, freshness)) => Ok(models_response(
            &visible_models(&models, api_key),
            freshness,
        )),
        Err(error) if error.code == "request_cancelled" => Err(error),
        Err(error) => match cached {
            Some((models, _)) => Ok(models_response(
                &visible_models(&canonicalize_cached_models(&models), api_key),
                "stale",
            )),
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
