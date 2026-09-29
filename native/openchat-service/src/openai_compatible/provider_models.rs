use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{api_compatible_provider, cancelled_error, http_error, invalid_response_error};

const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_MODELS: usize = 1000;
const CEREBRAS_PUBLIC_MODELS_URL: &str = "https://api.cerebras.ai/public/v1/models";

pub async fn models(
    storage: &AppStorage,
    provider_id: &str,
    api_key: &str,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let provider = api_compatible_provider(provider_id).ok_or_else(invalid_request_error)?;
    if api_key.trim().is_empty() || api_key.len() > 4096 || api_key.chars().any(char::is_control) {
        return Err(invalid_request_error());
    }

    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let cached = load_catalog(storage, provider.id, &key_hash)?;
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
            .map_err(|_| network_error(provider.name))?;
        let response = client
            .get(format!("{}/models", provider.base_url))
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|_| network_error(provider.name))?;
        if !response.status().is_success() {
            return Err(http_error(response.status(), Some(provider.id)));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| invalid_response_error())?;
        let metadata = if provider.id == "cerebras" {
            let response = client
                .get(CEREBRAS_PUBLIC_MODELS_URL)
                .send()
                .await
                .map_err(|_| network_error(provider.name))?;
            if !response.status().is_success() {
                return Err(http_error(response.status(), Some(provider.id)));
            }
            Some(
                response
                    .json::<Value>()
                    .await
                    .map_err(|_| invalid_response_error())?,
            )
        } else {
            None
        };
        let models = parse_models(provider.id, &value, metadata.as_ref())?;
        save_catalog(storage, provider.id, &key_hash, &models)?;
        Ok::<_, ServiceError>(models)
    };

    let fetched = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
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

fn parse_models(
    provider_id: &str,
    value: &Value,
    metadata: Option<&Value>,
) -> Result<Vec<Value>, ServiceError> {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("id")?.as_str()?.trim();
                    let details = metadata
                        .and_then(|catalog| catalog.get("data"))
                        .and_then(Value::as_array)
                        .and_then(|models| {
                            models.iter().find(|model| {
                                model.get("id").and_then(Value::as_str) == Some(id)
                            })
                        });
                    if id.is_empty()
                        || (provider_id == "openrouter" && !is_free_tool_model(item))
                        || (provider_id == "cerebras"
                            && !supports_cerebras_tools(details))
                    {
                        return None;
                    }
                    let context_window = ["context_length", "context_window"]
                        .iter()
                        .find_map(|key| item.get(key).and_then(Value::as_i64))
                        .or_else(|| {
                            details
                                .and_then(|model| model.pointer("/limits/max_context_length"))
                                .and_then(Value::as_i64)
                        })
                        .filter(|value| *value > 0);
                    Some(json!({
                        "id": id,
                        "displayName": item.get("name").or_else(|| details.and_then(|model| model.get("name"))).and_then(Value::as_str).filter(|name| !name.trim().is_empty()).unwrap_or(id),
                        "description": item.get("description").or_else(|| details.and_then(|model| model.get("description"))).and_then(Value::as_str).filter(|description| !description.trim().is_empty()),
                        "contextWindow": context_window,
                        "groupId": if provider_id == "openrouter" { "free" } else { "models" },
                        "defaultReasoningLevel": null,
                        "reasoningLevels": [],
                        "isAvailable": true,
                    }))
                })
                .take(MAX_MODELS)
                .collect()
        })
        .ok_or_else(invalid_response_error)
}

fn supports_cerebras_tools(model: Option<&Value>) -> bool {
    model
        .and_then(|model| model.get("capabilities"))
        .is_some_and(|capabilities| {
            capabilities
                .get("function_calling")
                .and_then(Value::as_bool)
                == Some(true)
                && capabilities.get("tools").and_then(Value::as_bool) == Some(true)
        })
}

fn is_free_tool_model(model: &Value) -> bool {
    let Some(pricing) = model.get("pricing") else {
        return false;
    };
    let free_price = ["prompt", "completion"].iter().all(|key| {
        pricing
            .get(key)
            .and_then(price_value)
            .is_some_and(|price| price == 0.0)
    });
    let supports_tools = model
        .get("supported_parameters")
        .and_then(Value::as_array)
        .is_some_and(|parameters| {
            parameters
                .iter()
                .any(|value| value.as_str() == Some("tools"))
        });
    let text_chat = model
        .pointer("/architecture/modality")
        .and_then(Value::as_str)
        .is_some_and(|modality| {
            let Some((input, output)) = modality.split_once("->") else {
                return false;
            };
            input.contains("text") && output.contains("text")
        });
    free_price && supports_tools && text_chat
}

fn price_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
}

fn load_catalog(
    storage: &AppStorage,
    provider_id: &str,
    key_hash: &str,
) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM compatible_provider_model_catalog
             WHERE provider_id = ?1 AND api_key_hash = ?2",
            rusqlite::params![provider_id, key_hash],
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
    provider_id: &str,
    key_hash: &str,
    models: &[Value],
) -> Result<(), ServiceError> {
    let models_json = serde_json::to_string(models).map_err(|_| invalid_response_error())?;
    storage
        .connect()
        .map_err(|_| storage_error())?
        .execute(
            "INSERT INTO compatible_provider_model_catalog
                (provider_id, api_key_hash, fetched_at_unix_ms, models_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider_id, api_key_hash) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                models_json = excluded.models_json",
            rusqlite::params![provider_id, key_hash, current_time_millis()?, models_json],
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

fn invalid_request_error() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The provider API key or provider identifier is invalid.",
        false,
    )
}

fn network_error(provider: &str) -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        format!("{provider} could not be reached. Check the network and try again."),
        true,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "The local model catalog could not be read or saved.",
        false,
    )
}
