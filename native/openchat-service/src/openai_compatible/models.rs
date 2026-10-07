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
    "jev-1.13-free",
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

pub(super) fn supports_image_input(
    storage: &AppStorage,
    model_id: &str,
) -> Result<bool, ServiceError> {
    let Some((models, _)) = load_model_catalog(storage)? else {
        return Ok(false);
    };
    Ok(models.iter().any(|model| {
        model.get("id").and_then(Value::as_str) == Some(model_id)
            && model.get("supportsImages").and_then(Value::as_bool) == Some(true)
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
                    if is_deprecated(details) {
                        return None;
                    }
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
                    let input_token_limit = details
                        .and_then(|details| details.get("limit"))
                        .and_then(|limits| limits.get("input"))
                        .and_then(Value::as_i64)
                        .filter(|input_limit| *input_limit > 0)
                        .map(|input_limit| {
                            context_window.map_or(input_limit, |context| input_limit.min(context))
                        });
                    let max_output_tokens = details
                        .and_then(|details| details.get("limit"))
                        .and_then(|limits| limits.get("output"))
                        .and_then(Value::as_i64)
                        .filter(|limit| *limit > 0);
                    let reasoning_supported = supports_reasoning(details);
                    let reasoning_levels = reasoning_levels(details);
                    let catalog_status = metadata_status(details);
                    let uses_responses_api = uses_responses_api_metadata(details);
                    let supports_images = supports_image_input_metadata(details);
                    let supports_tools = details
                        .and_then(|details| details.get("tool_call"))
                        .and_then(Value::as_bool);
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
            if model.get("catalogStatus").and_then(Value::as_str) == Some("deprecated") {
                return None;
            }
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
            has_current_reasoning_metadata
                && has_current_catalog_metadata
                && has_current_limit_metadata
                && has_current_image_metadata
                && model.get("supportsTools").is_some()
                && model
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(model_group)
                    == model.get("groupId").and_then(Value::as_str)
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

pub(super) fn supports_tool_calls(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Option<bool>, ServiceError> {
    let Some((models, _)) = load_model_catalog(storage)? else {
        return Ok(None);
    };
    Ok(models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
        .and_then(|model| model.get("supportsTools"))
        .and_then(Value::as_bool))
}

pub(super) fn is_responses_api_model(
    storage: &AppStorage,
    model_id: &str,
) -> Result<bool, ServiceError> {
    if let Some((models, _)) = load_model_catalog(storage)?
        && let Some(model) = models
            .iter()
            .find(|m| m.get("id").and_then(Value::as_str) == Some(model_id))
        && let Some(uses) = model.get("usesResponsesApi").and_then(Value::as_bool)
    {
        return Ok(uses);
    }
    Ok(model_id.starts_with("muse-")
        || model_id.starts_with("gpt-")
        || model_id.starts_with("grok-"))
}

pub(super) fn context_limits(
    storage: &AppStorage,
    model_id: &str,
) -> Result<(Option<i64>, Option<i64>, Option<i64>), ServiceError> {
    let models_json = storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT models_json FROM opencode_model_catalog WHERE catalog_id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| storage_error())?;
    let Some(models_json) = models_json else {
        return Ok((None, None, None));
    };
    let models =
        serde_json::from_str::<Vec<Value>>(&models_json).map_err(|_| invalid_response_error())?;
    let Some(model) = models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
    else {
        return Ok((None, None, None));
    };
    let context_window = model
        .get("contextWindow")
        .and_then(Value::as_i64)
        .filter(|window| *window > 0);
    let input_token_limit = model
        .get("inputTokenLimit")
        .and_then(Value::as_i64)
        .filter(|limit| *limit > 0)
        .map(|limit| context_window.map_or(limit, |context| limit.min(context)));
    let max_output_tokens = model
        .get("maxOutputTokens")
        .and_then(Value::as_i64)
        .filter(|limit| *limit > 0);
    Ok((context_window, input_token_limit, max_output_tokens))
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

pub(super) fn is_supported_free_chat_model(id: &str) -> bool {
    SUPPORTED_FREE_CHAT_MODELS.contains(&id)
}

pub(super) fn is_supported_paid_chat_model(id: &str) -> bool {
    SUPPORTED_PAID_CHAT_MODELS.contains(&id)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{has_model_groups, stale_catalog_fallback_allowed, supported_models};

    #[test]
    fn open_code_catalog_preserves_context_and_prompt_limits_separately() {
        let provider_models = json!({"data": [{"id": "big-pickle"}]});
        let metadata = json!({
            "opencode": {
                "models": {
                    "big-pickle": {
                        "name": "Big Pickle",
                        "limit": {"context": 262_144, "input": 131_072, "output": 16_384},
                        "tool_call": true
                    }
                }
            }
        });

        let models =
            supported_models(&provider_models, Some(&metadata)).expect("parse OpenCode models");

        assert_eq!(models[0]["contextWindow"], 262_144);
        assert_eq!(models[0]["inputTokenLimit"], 131_072);
        assert_eq!(models[0]["supportsTools"], true);
    }

    #[test]
    fn open_code_catalog_preserves_false_and_unknown_tool_support() {
        let provider_models = json!({"data": [{"id": "big-pickle"}]});
        let metadata = json!({
            "opencode": {
                "models": {
                    "big-pickle": {"tool_call": false}
                }
            }
        });

        let models =
            supported_models(&provider_models, Some(&metadata)).expect("parse OpenCode models");
        assert_eq!(models[0]["supportsTools"], false);

        let models = supported_models(&provider_models, None).expect("parse without metadata");
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
