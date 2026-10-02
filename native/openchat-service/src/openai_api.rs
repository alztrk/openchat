use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::{protocol::ServiceError, storage::AppStorage};

const MODELS_URL: &str = "https://api.openai.com/v1/models";
const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_MODELS: usize = 1000;

pub async fn models(
    storage: &AppStorage,
    api_key: &str,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if api_key.len() > 4096 || !api_key.starts_with("sk-") || api_key.chars().any(char::is_control)
    {
        return Err(ServiceError::new(
            "invalid_request_params",
            "The OpenAI API key parameter is invalid.",
            false,
        ));
    }

    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let cached = load_catalog(storage, &key_hash)?;
    if !force_refresh
        && let Some((models, fetched_at)) = &cached
        && current_time_millis()?.saturating_sub(*fetched_at) < CACHE_AGE_MS
    {
        return Ok(response(models, "current"));
    }

    let fetch = async {
        let client = Client::builder()
            .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|_| network_error())?;
        let response = client
            .get(MODELS_URL)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|_| network_error())?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| invalid_response_error())?;
        let models = parse_models(&value)?;
        save_catalog(storage, &key_hash, &models)?;
        Ok::<_, ServiceError>(models)
    };

    let fetched = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(ServiceError::new("request_cancelled", "The model request was cancelled.", true));
        }
        result = fetch => result,
    };
    match fetched {
        Ok(models) => Ok(response(&models, "current")),
        Err(error) if error.code == "network_unavailable" || error.retryable => match cached {
            Some((models, _)) => Ok(response(&models, "stale")),
            None => Err(error),
        },
        Err(error) => Err(error),
    }
}

fn parse_models(value: &Value) -> Result<Vec<Value>, ServiceError> {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let id = model.get("id")?.as_str()?.trim();
                    (!id.is_empty()).then(|| {
                        // The OpenAI Models API does not return per-model context limits.
                        json!({
                            "id": id,
                            "displayName": id,
                            "contextWindow": null,
                            "supportsImages": crate::chatgpt_store::model_supports_images(id),
                            "defaultReasoningLevel": null,
                            "reasoningLevels": [],
                            "isAvailable": true,
                        })
                    })
                })
                .take(MAX_MODELS)
                .collect()
        })
        .ok_or_else(invalid_response_error)
}

pub(crate) fn supports_image_input(model_id: &str) -> bool {
    crate::chatgpt_store::model_supports_images(model_id)
}

fn load_catalog(
    storage: &AppStorage,
    key_hash: &str,
) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    let database = storage.connect().map_err(|_| storage_error())?;
    database
        .query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM openai_api_model_catalog WHERE api_key_hash = ?1",
            [key_hash],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|_| storage_error())?
        .map(|(models_json, fetched_at)| {
            serde_json::from_str::<Vec<Value>>(&models_json)
                .map(|models| (models, fetched_at))
                .map_err(|_| invalid_response_error())
        })
        .transpose()
}

fn save_catalog(
    storage: &AppStorage,
    key_hash: &str,
    models: &[Value],
) -> Result<(), ServiceError> {
    let models_json = serde_json::to_string(models).map_err(|_| invalid_response_error())?;
    storage
        .connect()
        .map_err(|_| storage_error())?
        .execute(
            "INSERT INTO openai_api_model_catalog (api_key_hash, fetched_at_unix_ms, models_json)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(api_key_hash) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                models_json = excluded.models_json",
            rusqlite::params![key_hash, current_time_millis()?, models_json],
        )
        .map_err(|_| storage_error())?;
    Ok(())
}

fn response(models: &[Value], freshness: &str) -> Value {
    json!({"models": models, "freshness": freshness})
}

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| storage_error())
}

fn http_error(status: StatusCode) -> ServiceError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ServiceError::new(
            "authentication_required",
            "OpenAI rejected the API key. Check the connection in Settings.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            "OpenAI reported a usage limit. Try again later.",
            true,
        ),
        _ => ServiceError::new(
            "provider_request_failed",
            "OpenAI could not return its model list.",
            status.is_server_error(),
        ),
    }
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "OpenAI could not be reached. Check the network and try again.",
        true,
    )
}

fn invalid_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "OpenAI returned a model list OpenChat could not read.",
        false,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "The local model catalog could not be read or saved.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_models;

    #[test]
    fn openai_model_list_does_not_invent_context_limits() {
        let models = parse_models(&json!({
            "data": [{"id": "gpt-example", "owned_by": "openai"}]
        }))
        .expect("valid OpenAI model list");

        assert_eq!(models.len(), 1);
        assert!(models[0]["contextWindow"].is_null());
    }
}
