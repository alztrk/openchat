//! Lists OpenAI API models and maintains their local freshness-aware catalog.
//!
//! Provider payloads are parsed before caching, while unavailable network or
//! storage states remain explicit in the returned catalog response.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::{
    protocol::ServiceError,
    response_budget::{MAX_MODEL_CATALOG_RESPONSE_BYTES, read_bounded_response_body},
    storage::AppStorage,
};

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
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| network_error().with_details(error.without_url().to_string()))?;
        let response = client
            .get(MODELS_URL)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|error| network_error().with_details(error.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }
        let body = read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| invalid_response_error())?;
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

const REASONING_SUMMARY_MODELS: &[&str] = &[
    "gpt-6-astra",
    "gpt-6.1-sol",
    "gpt-6-sol",
    "gpt-6-luna",
    "gpt-5.6",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.6-cyber",
    "gpt-5.5",
    "gpt-5.5-pro",
    "gpt-5.4",
    "gpt-5.4-pro",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "gpt-5.2",
    "gpt-5.2-pro",
    "gpt-5.2-codex",
    "gpt-5.1",
    "gpt-5.1-codex",
    "gpt-5.1-codex-max",
    "gpt-5.1-codex-mini",
    "gpt-5",
    "gpt-5-pro",
    "gpt-5-mini",
    "gpt-5-nano",
    "gpt-5-codex",
    "gpt-5.3-codex",
    "o1-pro",
    "o4-mini",
    "o3",
    "o3-pro",
    "o3-mini",
    "o3-deep-research",
    "o4-mini-deep-research",
    "o1",
    "o1-mini",
    "codex-mini-latest",
    "computer-use-preview",
];

/// The OpenAI Models API does not include reasoning-summary capabilities.
/// Keep this list aligned with OpenAI's documented reasoning model families.
pub(crate) fn supports_reasoning_summary(model_id: &str) -> bool {
    REASONING_SUMMARY_MODELS.iter().any(|model| {
        model_id == *model
            || model_id
                .strip_prefix(model)
                .and_then(|suffix| suffix.strip_prefix('-'))
                .is_some_and(|snapshot| {
                    !snapshot.is_empty()
                        && snapshot
                            .chars()
                            .all(|character| character.is_ascii_digit() || character == '-')
                })
    })
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
        )
        .with_details(format!("HTTP status {}", status.as_u16())),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            "OpenAI reported a usage limit. Try again later.",
            true,
        )
        .with_details(format!("HTTP status {}", status.as_u16())),
        _ => ServiceError::new(
            "provider_request_failed",
            "OpenAI could not return its model list.",
            status.is_server_error(),
        )
        .with_details(format!("HTTP status {}", status.as_u16())),
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

    use super::{parse_models, supports_reasoning_summary};

    #[test]
    fn openai_model_list_does_not_invent_context_limits() {
        let models = parse_models(&json!({
            "data": [{"id": "gpt-example", "owned_by": "openai"}]
        }))
        .expect("valid OpenAI model list");

        assert_eq!(models.len(), 1);
        assert!(models[0]["contextWindow"].is_null());
    }

    #[test]
    fn reasoning_summaries_are_enabled_only_for_documented_openai_models() {
        for model_id in [
            "gpt-6-astra",
            "gpt-5.4-mini-2026-03-17",
            "gpt-5.5-pro-2026-04-23",
            "o4-mini",
            "o3-pro-2025-06-10",
            "computer-use-preview",
        ] {
            assert!(supports_reasoning_summary(model_id), "{model_id}");
        }

        for model_id in ["gpt-4.1", "gpt-6-astra-custom", "ft:gpt-6-astra:team"] {
            assert!(!supports_reasoning_summary(model_id), "{model_id}");
        }
    }
}
