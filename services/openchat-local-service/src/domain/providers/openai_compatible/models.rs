//! Loads OpenCode inference model metadata and provider capability information.
//!
//! Catalog freshness, model support, context limits, and modality flags are
//! validated before being returned to model selection.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    protocol::ServiceError,
    response_budget::{MAX_MODEL_CATALOG_RESPONSE_BYTES, read_bounded_response_body},
    storage::AppStorage,
};

use super::{
    cancelled_error, client, invalid_response_error, network_error_with_details, storage_error,
};
#[cfg(test)]
pub(crate) const MODELS_URL: &str = "https://opencode.ai/inference/v1/models";
const MODELS_DEV_URL: &str = "https://models.opencode.ai/api.json";
const MODEL_CATALOG_CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MODEL_METADATA_TIMEOUT: Duration = Duration::from_secs(5);

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
    Ok(cached.and_then(|(models_json, fetched_at)| {
        serde_json::from_str::<Vec<Value>>(&models_json)
            .ok()
            .map(|models| (models, fetched_at))
    }))
}

pub(super) struct RouteModelMetadata {
    pub(super) is_free: bool,
    pub(super) is_paid: bool,
    pub(super) uses_responses_api: bool,
    pub(super) inference_base_url: Option<String>,
    pub(super) supports_tool_calls: Option<bool>,
    pub(super) context_limits: super::ModelContextLimits,
    pub(super) supports_images: bool,
}

pub(super) fn route_model_metadata(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Option<RouteModelMetadata>, ServiceError> {
    let Some((models, _)) = load_model_catalog(storage)? else {
        return Ok(None);
    };
    let Some(model) = models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
    else {
        return Ok(None);
    };

    Ok(Some(RouteModelMetadata {
        is_free: model.get("groupId").and_then(Value::as_str) == Some("free"),
        is_paid: model.get("groupId").and_then(Value::as_str) == Some("paid"),
        uses_responses_api: model
            .get("usesResponsesApi")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        inference_base_url: model
            .get("inferenceBaseUrl")
            .and_then(Value::as_str)
            .and_then(validated_inference_base_url),
        supports_tool_calls: model.get("supportsTools").and_then(Value::as_bool),
        context_limits: model_context_limits(Some(model)),
        supports_images: model.get("supportsImages").and_then(Value::as_bool) == Some(true),
    }))
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

fn is_free_by_metadata(details: &Value) -> bool {
    let Some(cost) = details.get("cost") else {
        return false;
    };
    let Some(input_cost) = cost.get("input").and_then(Value::as_f64) else {
        return false;
    };
    let Some(output_cost) = cost.get("output").and_then(Value::as_f64) else {
        return false;
    };
    input_cost == 0.0
        && output_cost == 0.0
        && cost
            .get("tiers")
            .and_then(Value::as_array)
            .is_none_or(|tiers| {
                tiers.iter().all(|tier| {
                    tier.get("input")
                        .and_then(Value::as_f64)
                        .is_some_and(|tier_input_cost| tier_input_cost == 0.0)
                        && tier
                            .get("output")
                            .and_then(Value::as_f64)
                            .is_some_and(|tier_output_cost| tier_output_cost == 0.0)
                })
            })
}

fn is_paid_by_metadata(details: &Value) -> bool {
    let Some(cost) = details.get("cost") else {
        return false;
    };
    let Some(input_cost) = cost.get("input").and_then(Value::as_f64) else {
        return false;
    };
    let Some(output_cost) = cost.get("output").and_then(Value::as_f64) else {
        return false;
    };
    let tiers = cost.get("tiers").and_then(Value::as_array);
    if input_cost < 0.0
        || output_cost < 0.0
        || tiers.is_some_and(|tiers| {
            tiers.iter().any(|tier| {
                tier.get("input")
                    .and_then(Value::as_f64)
                    .is_none_or(|value| value < 0.0)
                    || tier
                        .get("output")
                        .and_then(Value::as_f64)
                        .is_none_or(|value| value < 0.0)
            })
        })
    {
        return false;
    }
    input_cost > 0.0
        || output_cost > 0.0
        || tiers.is_some_and(|tiers| {
            tiers.iter().any(|tier| {
                tier.get("input")
                    .and_then(Value::as_f64)
                    .is_some_and(|value| value > 0.0)
                    || tier
                        .get("output")
                        .and_then(Value::as_f64)
                        .is_some_and(|value| value > 0.0)
            })
        })
}

fn inference_base_url(details: &Value) -> Option<String> {
    let raw = details.pointer("/provider/api")?.as_str()?;
    validated_inference_base_url(raw)
}

fn validated_inference_base_url(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("opencode.ai")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path().trim_end_matches('/') != "/inference/openai/v1"
    {
        return None;
    }
    Some(url.as_str().trim_end_matches('/').to_owned())
}

fn model_group(details: &Value) -> Option<&'static str> {
    if is_free_by_metadata(details) {
        Some("free")
    } else if is_paid_by_metadata(details) {
        Some("paid")
    } else {
        None
    }
}

fn metadata_models(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.get("opencode")?.get("models")?.as_object()
}

fn supports_reasoning(details: Option<&Value>) -> bool {
    details
        .and_then(|model| model.get("reasoning"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn is_deprecated(details: Option<&Value>) -> bool {
    metadata_status(details) == Some("deprecated")
}

fn metadata_status(details: Option<&Value>) -> Option<&str> {
    details
        .and_then(|model| model.get("status"))
        .and_then(Value::as_str)
}

fn uses_responses_api_metadata(details: Option<&Value>) -> bool {
    details
        .and_then(|model| model.pointer("/provider/npm"))
        .and_then(Value::as_str)
        == Some("@ai-sdk/openai")
}

fn supports_image_input_metadata(details: Option<&Value>) -> bool {
    ["/modalities/input", "/capabilities/input"]
        .iter()
        .filter_map(|pointer| details.and_then(|model| model.pointer(pointer)))
        .any(|input| {
            input.as_array().is_some_and(|modalities| {
                modalities.iter().any(|mode| mode.as_str() == Some("image"))
            })
        })
}

fn reasoning_levels(details: Option<&Value>) -> Vec<String> {
    if !supports_reasoning(details) {
        return Vec::new();
    }

    let Some(options) = details
        .and_then(|model| model.get("reasoning_options"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    if let Some(effort) = options
        .iter()
        .find(|option| option.get("type").and_then(Value::as_str) == Some("effort"))
    {
        return effort
            .get("values")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }

    if options
        .iter()
        .any(|option| option.get("type").and_then(Value::as_str) == Some("toggle"))
    {
        return ["low", "medium", "high"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    }

    Vec::new()
}

async fn fetch_model_metadata(client: &Client) -> Result<Value, ServiceError> {
    let response = client
        .get(MODELS_DEV_URL)
        .timeout(MODEL_METADATA_TIMEOUT)
        .send()
        .await
        .map_err(network_error_with_details)?;
    if !response.status().is_success() {
        return Err(invalid_response_error());
    }
    let body = read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| invalid_response_error())?;
    if metadata_models(&value).is_none() {
        return Err(invalid_response_error());
    }
    Ok(value)
}

fn supported_models(metadata: &Value) -> Result<Vec<Value>, ServiceError> {
    let metadata_models = metadata_models(metadata).ok_or_else(invalid_response_error)?;

    Ok(metadata_models
        .iter()
        .filter_map(|(id, details)| {
            let group_id = model_group(details)?;
            if is_deprecated(Some(details)) {
                return None;
            }
            let inference_base_url = inference_base_url(details)?;
            let display_name = details
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(id);
            let description = details
                .get("description")
                .and_then(Value::as_str)
                .filter(|description| !description.trim().is_empty());
            let context_window = details
                .get("limit")
                .and_then(|limits| limits.get("context"))
                .and_then(Value::as_i64)
                .filter(|context_window| *context_window > 0);
            let input_token_limit = details
                .get("limit")
                .and_then(|limits| limits.get("input"))
                .and_then(Value::as_i64)
                .filter(|input_limit| *input_limit > 0)
                .map(|input_limit| {
                    context_window.map_or(input_limit, |context| input_limit.min(context))
                });
            let max_output_tokens = details
                .get("limit")
                .and_then(|limits| limits.get("output"))
                .and_then(Value::as_i64)
                .filter(|limit| *limit > 0);
            let reasoning_supported =
                details.get("reasoning").and_then(Value::as_bool) == Some(true);
            let reasoning_levels = reasoning_levels(Some(details));
            let catalog_status = metadata_status(Some(details));
            let uses_responses_api = uses_responses_api_metadata(Some(details));
            let supports_images = supports_image_input_metadata(Some(details));
            let supports_tools = details.get("tool_call").and_then(Value::as_bool);
            Some(json!({
                "id": id,
                "displayName": display_name,
                "description": description,
                "contextWindow": context_window,
                "inputTokenLimit": input_token_limit,
                "maxOutputTokens": max_output_tokens,
                "catalogStatus": catalog_status,
                "groupId": group_id,
                "defaultReasoningLevel": null,
                "reasoningLevels": reasoning_levels,
                "supportsReasoning": reasoning_supported,
                "supportsImages": supports_images,
                "supportsTools": supports_tools,
                "usesResponsesApi": uses_responses_api,
                "inferenceBaseUrl": inference_base_url,
                "isAvailable": true,
            }))
        })
        .collect())
}

fn canonicalize_cached_models(models: &[Value]) -> Vec<Value> {
    models
        .iter()
        .filter_map(|model| {
            let _id = model.get("id")?.as_str()?;
            if model.get("catalogStatus").and_then(Value::as_str) == Some("deprecated") {
                return None;
            }
            let group_id = match model.get("groupId").and_then(Value::as_str) {
                Some("free") => "free",
                Some("paid") => "paid",
                _ => return None,
            };
            let Some(base_url) = model
                .get("inferenceBaseUrl")
                .and_then(Value::as_str)
                .and_then(|value| inference_base_url(&json!({"provider": {"api": value}})))
            else {
                return None;
            };
            let mut model = model.clone();
            let fields = model.as_object_mut()?;
            fields.insert("groupId".to_owned(), json!(group_id));
            fields.insert("inferenceBaseUrl".to_owned(), json!(base_url));
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
            let has_current_reasoning_metadata = model
                .get("supportsReasoning")
                .and_then(Value::as_bool)
                .is_some();
            let has_current_catalog_metadata = model.get("catalogStatus").is_some();
            let has_current_limit_metadata = model.get("inputTokenLimit").is_some();
            let has_current_image_metadata = model
                .get("supportsImages")
                .and_then(Value::as_bool)
                .is_some();
            let valid_group = matches!(
                model.get("groupId").and_then(Value::as_str),
                Some("free" | "paid")
            );
            has_current_reasoning_metadata
                && has_current_catalog_metadata
                && has_current_limit_metadata
                && has_current_image_metadata
                && model.get("supportsTools").is_some()
                && model
                    .get("inferenceBaseUrl")
                    .and_then(Value::as_str)
                    .and_then(|value| validated_inference_base_url(value))
                    .is_some()
                && valid_group
        })
}

pub(super) fn supports_reasoning_level(
    storage: &AppStorage,
    model_id: &str,
    level: &str,
) -> Result<bool, ServiceError> {
    let Some((models, _)) = load_model_catalog(storage)? else {
        return Ok(false);
    };
    Ok(models.iter().any(|model| {
        model.get("id").and_then(Value::as_str) == Some(model_id)
            && model
                .get("reasoningLevels")
                .and_then(Value::as_array)
                .is_some_and(|levels| levels.iter().any(|value| value.as_str() == Some(level)))
    }))
}

fn model_context_limits(model: Option<&Value>) -> super::ModelContextLimits {
    let context_window = model
        .and_then(|model| model.get("contextWindow"))
        .and_then(Value::as_i64)
        .filter(|window| *window > 0);
    let input_token_limit = model
        .and_then(|model| model.get("inputTokenLimit"))
        .and_then(Value::as_i64)
        .filter(|limit| *limit > 0)
        .map(|limit| context_window.map_or(limit, |context| limit.min(context)));
    let max_output_tokens = model
        .and_then(|model| model.get("maxOutputTokens"))
        .and_then(Value::as_i64)
        .filter(|limit| *limit > 0);
    super::ModelContextLimits {
        context_window,
        input_token_limit,
        max_output_tokens,
    }
}

fn visible_models(models: &[Value], api_key: Option<&str>) -> Vec<Value> {
    models
        .iter()
        .filter(|model| {
            model
                .get("inferenceBaseUrl")
                .and_then(Value::as_str)
                .and_then(|value| validated_inference_base_url(value))
                .is_some()
                && (model.get("groupId").and_then(Value::as_str) != Some("paid")
                    || api_key.is_some())
        })
        .cloned()
        .collect()
}

fn models_response(models: &[Value], freshness: &str) -> Value {
    json!({"models": models, "freshness": freshness})
}

fn stale_catalog_fallback_allowed(error: &ServiceError) -> bool {
    error.code == "network_unavailable" || error.retryable
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
    let fetch = async {
        let metadata = fetch_model_metadata(&http_client).await?;
        let models = supported_models(&metadata)?;
        save_model_catalog(storage, &models)?;
        Ok::<_, ServiceError>((models, "current"))
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
        Err(error) if stale_catalog_fallback_allowed(&error) => match cached {
            Some((models, _)) => Ok(models_response(
                &visible_models(&canonicalize_cached_models(&models), api_key),
                "stale",
            )),
            None => Err(error),
        },
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        has_model_groups, load_model_catalog, stale_catalog_fallback_allowed, supported_models,
    };

    #[test]
    fn malformed_cached_catalog_is_treated_as_a_cache_miss() {
        let root =
            std::env::temp_dir().join(format!("openchat-opencode-cache-{}", uuid::Uuid::new_v4()));
        let storage = crate::storage::AppStorage::open_at(root.clone()).expect("open storage");
        let connection = storage.connect().expect("connect storage");
        connection
            .execute_batch(
                "CREATE TABLE opencode_model_catalog (
                    catalog_id INTEGER PRIMARY KEY,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    models_json TEXT NOT NULL
                );
                 INSERT INTO opencode_model_catalog
                    (catalog_id, fetched_at_unix_ms, models_json)
                 VALUES (1, 0, '{broken');",
            )
            .expect("insert malformed cached catalog");
        drop(connection);

        assert!(
            load_model_catalog(&storage)
                .expect("read cached catalog")
                .is_none()
        );

        drop(storage);
        std::fs::remove_dir_all(root).expect("remove isolated cache storage");
    }

    #[test]
    fn open_code_catalog_preserves_context_and_prompt_limits_separately() {
        let metadata = json!({
            "opencode": {
                "models": {
                    "big-pickle": {
                        "cost": {"input": 0, "output": 0},
                        "provider": {"api": "https://opencode.ai/inference/openai/v1"},
                        "name": "Big Pickle",
                        "limit": {"context": 262_144, "input": 131_072, "output": 16_384},
                        "tool_call": true
                    }
                }
            }
        });

        let models = supported_models(&metadata).expect("parse OpenCode models");

        assert_eq!(models[0]["contextWindow"], 262_144);
        assert_eq!(models[0]["inputTokenLimit"], 131_072);
        assert_eq!(models[0]["supportsTools"], true);
    }

    #[test]
    fn open_code_catalog_discovers_free_models_from_price_metadata() {
        let metadata = json!({"opencode": {"models": {
            "exo-free": {"cost": {"input": 0, "output": 0}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}},
            "ling-3.0-flash-fin-free": {"cost": {"input": 0, "output": 0}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}},
            "step-5-preview-free": {"cost": {"input": 0, "output": 0}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}},
            "nemotron-3-ultra-free": {"cost": {"input": 0, "output": 0}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}},
            "glm-5.3": {"cost": {"input": 0.5, "output": 1}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}},
            "qwen3.8-max": {"cost": {"input": 0.8, "output": 1}, "provider": {"api": "https://opencode.ai/inference/openai/v1"}}
        }}});

        let models = supported_models(&metadata).expect("catalog");
        assert_eq!(models.len(), 6);
        assert!(
            models
                .iter()
                .all(|model| model["groupId"] == "free" || model["groupId"] == "paid")
        );
        assert_eq!(
            models
                .iter()
                .filter(|model| model["groupId"] == "free")
                .count(),
            4
        );
        assert_eq!(
            models
                .iter()
                .filter(|model| model["groupId"] == "paid")
                .count(),
            2
        );
    }

    #[test]
    fn open_code_catalog_excludes_models_that_require_other_api_protocols() {
        let metadata = json!({"opencode": {"models": {
            "non-openai-model": {
                "cost": {"input": 1, "output": 1},
                "provider": {"api": "https://opencode.ai/inference/anthropic/v1"}
            }
        }}});

        let models = supported_models(&metadata).expect("catalog");
        assert!(models.is_empty());
    }

    #[test]
    fn open_code_catalog_preserves_false_and_unknown_tool_support() {
        let metadata = json!({
            "opencode": {
                "models": {
                    "big-pickle": {
                        "cost": {"input": 0, "output": 0},
                        "provider": {"api": "https://opencode.ai/inference/openai/v1"},
                        "tool_call": false
                    }
                }
            }
        });

        let models = supported_models(&metadata).expect("parse OpenCode models");
        assert_eq!(models[0]["supportsTools"], false);

        let paid_metadata = json!({"opencode": {"models": {
            "paid-model": {
                "cost": {"input": 0.5, "output": 1},
                "provider": {"api": "https://opencode.ai/inference/openai/v1"}
            }
        }}});
        let models = supported_models(&paid_metadata).expect("parse paid model metadata");
        assert!(models[0]["supportsTools"].is_null());
    }

    #[test]
    fn cached_catalog_requires_the_current_prompt_limit_field() {
        let mut model = json!({
            "id": "big-pickle",
            "groupId": "free",
            "supportsReasoning": false,
            "catalogStatus": null,
            "supportsImages": false,
            "supportsTools": null,
            "inferenceBaseUrl": "https://opencode.ai/inference/openai/v1",
        });
        assert!(!has_model_groups(std::slice::from_ref(&model)));
        model["inputTokenLimit"] = json!(null);
        assert!(has_model_groups(&[model]));
    }

    #[test]
    fn stale_catalog_fallback_does_not_hide_authentication_or_request_errors() {
        let network = super::super::network_error();
        let rate_limited =
            super::super::http_error(reqwest::StatusCode::TOO_MANY_REQUESTS, Some("opencode"));
        let temporary =
            super::super::http_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, Some("opencode"));
        let unauthorized =
            super::super::http_error(reqwest::StatusCode::UNAUTHORIZED, Some("opencode"));
        let forbidden = super::super::http_error(reqwest::StatusCode::FORBIDDEN, Some("opencode"));
        let invalid_request =
            super::super::http_error(reqwest::StatusCode::BAD_REQUEST, Some("opencode"));

        assert!(stale_catalog_fallback_allowed(&network));
        assert!(stale_catalog_fallback_allowed(&rate_limited));
        assert!(stale_catalog_fallback_allowed(&temporary));
        assert!(!stale_catalog_fallback_allowed(&unauthorized));
        assert!(!stale_catalog_fallback_allowed(&forbidden));
        assert!(!stale_catalog_fallback_allowed(&invalid_request));
    }
}
