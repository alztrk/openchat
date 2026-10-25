//! Fetches and caches model catalogs for configured compatible providers.
//!
//! Custom provider metadata and built-in provider catalogs share bounded
//! parsing while retaining route-specific model capabilities.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::domain::http_headers::sensitive_api_key_header;
use crate::response_budget::{MAX_MODEL_CATALOG_RESPONSE_BYTES, read_bounded_response_body};
use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
};

use super::{
    CustomProviderConfig, CustomProviderProtocol, api_compatible_provider, cancelled_error,
    http_error, invalid_response_error,
};

const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_MODELS: usize = 1000;
const CEREBRAS_PUBLIC_MODELS_URL: &str = "https://api.cerebras.ai/public/v1/models";
const GEMINI_MODELS_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000";

fn catalog_client(provider: &str) -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| network_error(provider))
}

pub async fn models_for_custom_provider(
    storage: &AppStorage,
    provider: &CustomProviderConfig,
    api_key: Option<&str>,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if api_key.is_some_and(|key| {
        key.trim().is_empty() || key.len() > 4096 || key.chars().any(char::is_control)
    }) {
        return Err(invalid_request_error());
    }
    let cache_key = custom_catalog_key(provider, api_key);
    let cached = load_catalog(storage, &provider.id, &cache_key)?;
    if !force_refresh
        && let Some((models, fetched_at)) = &cached
        && current_time_millis()?.saturating_sub(*fetched_at) < CACHE_AGE_MS
    {
        return Ok(response(models, "current"));
    }

    let fetch = async {
        let client = catalog_client("Custom provider")?;
        let mut request = client.get(format!("{}/models", provider.base_url));
        if provider.protocol == CustomProviderProtocol::Anthropic {
            request = request.query(&[("limit", MAX_MODELS)]);
        }
        request = match (provider.protocol, api_key) {
            (CustomProviderProtocol::OpenAi, Some(api_key)) => request.bearer_auth(api_key),
            (CustomProviderProtocol::OpenAi, None) => request,
            (CustomProviderProtocol::Anthropic, Some(api_key)) => request
                .header(
                    "x-api-key",
                    sensitive_api_key_header(api_key).map_err(|_| invalid_request_error())?,
                )
                .header("anthropic-version", "2023-06-01"),
            (CustomProviderProtocol::Anthropic, None) => {
                request.header("anthropic-version", "2023-06-01")
            }
        };
        let response = request
            .send()
            .await
            .map_err(|_| network_error("Custom provider"))?;
        if !response.status().is_success() {
            return Err(http_error(response.status(), Some(&provider.id)));
        }
        let body = read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| invalid_response_error())?;
        let items = value
            .get("data")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty() && items.len() <= MAX_MODELS)
            .ok_or_else(invalid_response_error)?;
        let models = items
            .iter()
            .filter_map(|item| {
                let id = item.get("id")?.as_str()?.trim();
                if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                    return None;
                }
                let display_name = item
                    .get("display_name")
                    .or_else(|| item.get("name"))
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or(id);
                let context_window = item
                    .get("context_window")
                    .or_else(|| item.get("input_token_limit"))
                    .and_then(Value::as_i64)
                    .filter(|limit| *limit > 0);
                Some(json!({
                    "id": id,
                    "displayName": display_name,
                    "description": Value::Null,
                    "contextWindow": context_window,
                    "inputTokenLimit": context_window,
                    "maxOutputTokens": item.get("max_output_tokens").and_then(Value::as_i64),
                    "supportsImages": false,
                    "supportsTools": Value::Null,
                    "supportsReasoning": false,
                    "reasoningLevels": [],
                    "isAvailable": true,
                    "groupId": "models"
                }))
            })
            .collect::<Vec<_>>();
        if models.is_empty() {
            return Err(invalid_response_error());
        }
        save_catalog(storage, &provider.id, &cache_key, &models)?;
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

pub(super) fn custom_context_limits(
    storage: &AppStorage,
    provider: &CustomProviderConfig,
    api_key: Option<&str>,
    model_id: &str,
) -> Result<super::ModelContextLimits, ServiceError> {
    let cache_key = custom_catalog_key(provider, api_key);
    let Some((models, _)) = load_catalog(storage, &provider.id, &cache_key)? else {
        return Ok(super::ModelContextLimits::default());
    };
    let Some(model) = models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
    else {
        return Ok(super::ModelContextLimits::default());
    };
    let context_window = model
        .get("contextWindow")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    let input_token_limit = model
        .get("inputTokenLimit")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    let max_output_tokens = model
        .get("maxOutputTokens")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    Ok(super::ModelContextLimits {
        context_window,
        input_token_limit,
        max_output_tokens,
    })
}

fn custom_catalog_key(provider: &CustomProviderConfig, api_key: Option<&str>) -> String {
    api_key_hash(&format!(
        "{}\0{:?}\0{}",
        provider.base_url,
        provider.protocol,
        api_key.unwrap_or_default()
    ))
}

pub async fn models(
    storage: &AppStorage,
    provider_id: &str,
    api_key: &str,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let provider = api_compatible_provider(provider_id).ok_or_else(invalid_request_error)?;
    if (api_key.trim().is_empty() && provider_id != "kilo")
        || api_key.len() > 4096
        || api_key.chars().any(char::is_control)
    {
        return Err(invalid_request_error());
    }

    let key_hash = api_key_hash(api_key);
    let cached = load_catalog(storage, provider.id, &key_hash)?;
    if !force_refresh
        && let Some((models, fetched_at)) = &cached
        && has_current_limit_metadata(models)
        && current_time_millis()?.saturating_sub(*fetched_at) < CACHE_AGE_MS
    {
        return Ok(response(models, "current"));
    }

    let fetch = async {
        let client = catalog_client(provider.name)?;
        let mut request = client.get(format!("{}/models", provider.base_url));
        if provider.is_anthropic() {
            request = request
                .query(&[("limit", MAX_MODELS)])
                .header(
                    "x-api-key",
                    sensitive_api_key_header(api_key).map_err(|_| invalid_request_error())?,
                )
                .header("anthropic-version", "2023-06-01");
        } else if !api_key.trim().is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = request
            .send()
            .await
            .map_err(|_| network_error(provider.name))?;
        if !response.status().is_success() {
            return Err(http_error(response.status(), Some(provider.id)));
        }
        let body = read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| invalid_response_error())?;
        let metadata = if provider.id == "cerebras" {
            let response = client
                .get(CEREBRAS_PUBLIC_MODELS_URL)
                .send()
                .await
                .map_err(|_| network_error(provider.name))?;
            if !response.status().is_success() {
                return Err(http_error(response.status(), Some(provider.id)));
            }
            let body =
                read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
            Some(serde_json::from_slice(&body).map_err(|_| invalid_response_error())?)
        } else if provider.id == "gemini" {
            let response = client
                .get(GEMINI_MODELS_URL)
                .header(
                    "x-goog-api-key",
                    sensitive_api_key_header(api_key).map_err(|_| invalid_request_error())?,
                )
                .send()
                .await
                .map_err(|_| network_error(provider.name))?;
            if !response.status().is_success() {
                return Err(http_error(response.status(), Some(provider.id)));
            }
            let body =
                read_bounded_response_body(response, MAX_MODEL_CATALOG_RESPONSE_BYTES).await?;
            Some(serde_json::from_slice(&body).map_err(|_| invalid_response_error())?)
        } else {
            None
        };
        let mut models = parse_models(provider.id, &value, metadata.as_ref())?;
        if provider_id == "kilo" && api_key.trim().is_empty() {
            models.retain(|m| m.get("groupId").and_then(Value::as_str) == Some("free"));
        }
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
        Ok(mut models) => {
            if provider.id != "kilo" {
                crate::domain::usage_statistics::enrich_model_catalog_capabilities(
                    storage,
                    provider.id,
                    &mut models,
                    cancellation,
                )
                .await?;
            }
            save_catalog(storage, provider.id, &key_hash, &models)?;
            Ok(response(&models, "current"))
        }
        Err(error) if error.code == "network_unavailable" || error.retryable => match cached {
            Some((models, _)) => Ok(response(&models, "stale")),
            None => Err(error),
        },
        Err(error) => Err(error),
    }
}

fn default_provider_context_window(provider_id: &str) -> Option<i64> {
    match provider_id {
        "deepseek" => Some(64_000),
        "xai" => Some(131_072),
        "perplexity" => Some(128_000),
        "sambanova" => Some(131_072),
        "anthropic" => Some(200_000),
        "fireworks" => Some(131_072),
        "deepinfra" => Some(131_072),
        "dashscope" => Some(131_072),
        "cohere" => Some(128_000),
        "kilo" => Some(256_000),
        _ => None,
    }
}

fn default_provider_max_output_tokens(provider_id: &str) -> Option<i64> {
    match provider_id {
        "deepseek" | "xai" | "kilo" => Some(8_192),
        "perplexity" | "cohere" => Some(4_096),
        "anthropic" => Some(8_192),
        _ => None,
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
                        .and_then(|catalog| {
                            if provider_id == "gemini" {
                                catalog.get("models")
                            } else {
                                catalog.get("data")
                            }
                        })
                        .and_then(Value::as_array)
                        .and_then(|models| {
                            models.iter().find(|model| {
                                model_matches_id(provider_id, model, id)
                            })
                        });
                    if id.is_empty()
                        || (provider_id == "mistral"
                            && (item.get("archived").and_then(Value::as_bool) == Some(true)
                                || item
                                    .pointer("/capabilities/completion_chat")
                                    .and_then(Value::as_bool)
                                    != Some(true)))
                        || (provider_id == "openrouter" && !is_free_tool_model(item))
                        || (provider_id == "cerebras"
                            && !supports_cerebras_tools(details))
                    {
                        return None;
                    }
                    let input_token_limit = (provider_id == "gemini")
                        .then(|| {
                            details
                                .and_then(|model| model.get("inputTokenLimit"))
                                .and_then(Value::as_i64)
                        })
                        .flatten()
                        .filter(|limit| *limit > 0);
                    let context_window = ["context_length", "context_window"]
                        .iter()
                        .find_map(|key| item.get(key).and_then(Value::as_i64))
                        .or_else(|| {
                            (provider_id == "mistral")
                                .then(|| item.get("max_context_length"))
                                .flatten()
                                .and_then(Value::as_i64)
                        })
                        .or_else(|| {
                            details
                                .and_then(|model| model.pointer("/limits/max_context_length"))
                                .and_then(Value::as_i64)
                        })
                        .or(input_token_limit)
                        .or_else(|| default_provider_context_window(provider_id))
                        .filter(|value| *value > 0);
                    let input_token_limit = input_token_limit.or(context_window);
                    let max_output_tokens = [
                        item.pointer("/top_provider/max_completion_tokens"),
                        item.get("max_completion_tokens"),
                        details.and_then(|model| model.get("outputTokenLimit")),
                        details.and_then(|model| model.get("max_completion_tokens")),
                    ]
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_i64)
                    .find(|limit| *limit > 0)
                    .or_else(|| default_provider_max_output_tokens(provider_id));
                    let supports_images = if provider_id == "kilo" {
                        Some(supports_images(item, details).unwrap_or(false))
                    } else {
                        supports_images(item, details)
                    };
                    let supports_tools = provider_tool_support(provider_id, item).or_else(|| {
                        details.and_then(|model| provider_tool_support(provider_id, model))
                    });
                    let reasoning_levels: Vec<String> = Vec::new();
                    let is_kilo_free = provider_id == "kilo"
                        && (item.get("isFree").and_then(Value::as_bool) == Some(true)
                            || id.ends_with(":free")
                            || id.contains("/free")
                            || id == "kilo-auto/free");
                    let is_kilo_reasoning = provider_id == "kilo"
                        && (id.contains("reasoning")
                            || id.contains("r1")
                            || item
                                .get("supported_parameters")
                                .and_then(Value::as_array)
                                .is_some_and(|params| {
                                    params.iter().any(|p| {
                                        p.as_str() == Some("reasoning")
                                            || p.as_str() == Some("include_reasoning")
                                    })
                                }));
                    let supports_reasoning = if provider_id == "kilo" {
                        Some(is_kilo_reasoning)
                    } else {
                        item.get("reasoning")
                            .and_then(Value::as_bool)
                            .or_else(|| {
                                details
                                    .and_then(|model| model.get("reasoning"))
                                    .and_then(Value::as_bool)
                            })
                    };
                    let mut model = json!({
                        "id": id,
                        "displayName": item.get("name").or_else(|| details.and_then(|model| model.get("name").or_else(|| model.get("displayName")))).and_then(Value::as_str).filter(|name| !name.trim().is_empty()).unwrap_or(id),
                        "description": item.get("description").or_else(|| details.and_then(|model| model.get("description"))).and_then(Value::as_str).filter(|description| !description.trim().is_empty()),
                        "contextWindow": context_window,
                        "supportsImages": supports_images,
                        "supportsTools": supports_tools,
                        "inputTokenLimit": input_token_limit,
                        "maxOutputTokens": max_output_tokens,
                        "groupId": if provider_id == "openrouter" || is_kilo_free { "free" } else { "models" },
                        "defaultReasoningLevel": null,
                        "reasoningLevels": reasoning_levels,
                        "isAvailable": true,
                        "capabilityMetadataVersion": 2,
                    });
                    if let Some(supports_reasoning) = supports_reasoning {
                        model["supportsReasoning"] = json!(supports_reasoning);
                    }
                    Some(model)
                })
                .take(MAX_MODELS)
                .collect()
        })
        .ok_or_else(invalid_response_error)
}

fn model_matches_id(provider_id: &str, model: &Value, model_id: &str) -> bool {
    model.get("id").and_then(Value::as_str) == Some(model_id)
        || (provider_id == "gemini"
            && (model.get("baseModelId").and_then(Value::as_str) == Some(model_id)
                || model
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|name| name.strip_prefix("models/"))
                    == Some(model_id)))
}

fn has_current_limit_metadata(models: &[Value]) -> bool {
    models.iter().all(|model| {
        model
            .get("inputTokenLimit")
            .and_then(Value::as_u64)
            .is_some_and(|limit| limit > 0)
            && model
                .get("supportsImages")
                .is_some_and(|value| value.as_bool().is_some() || value.is_null())
            && model
                .get("supportsTools")
                .is_some_and(|value| value.as_bool().is_some() || value.is_null())
            && model
                .get("capabilityMetadataVersion")
                .and_then(Value::as_u64)
                == Some(2)
    })
}

fn provider_tool_support(provider_id: &str, model: &Value) -> Option<bool> {
    match provider_id {
        "gemini" => model.pointer("/function_calling").and_then(Value::as_bool),
        "groq" => model
            .pointer("/capabilities/tools")
            .and_then(Value::as_bool),
        "cerebras" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool)
            .zip(
                model
                    .pointer("/capabilities/tools")
                    .and_then(Value::as_bool),
            )
            .map(|(function_calling, tools)| function_calling && tools),
        "openrouter" => model.get("tool_call").and_then(Value::as_bool).or_else(|| {
            model
                .get("supported_parameters")
                .and_then(Value::as_array)
                .map(|parameters| {
                    parameters
                        .iter()
                        .any(|parameter| parameter.as_str() == Some("tools"))
                })
        }),
        "mistral" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "deepseek" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "xai" => None,
        "together" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "sambanova" | "anthropic" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "perplexity" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "fireworks" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "deepinfra" | "dashscope" | "cohere" => model
            .pointer("/capabilities/function_calling")
            .and_then(Value::as_bool),
        "kilo" => model
            .get("supported_parameters")
            .and_then(Value::as_array)
            .map(|params| params.iter().any(|p| p.as_str() == Some("tools")))
            .or(Some(true)),
        _ => None,
    }
}

pub(super) fn supports_tool_calls(
    storage: &AppStorage,
    provider_id: &str,
    api_key: &str,
    model_id: &str,
) -> Result<Option<bool>, ServiceError> {
    let key_hash = api_key_hash(api_key);
    let Some((models, _)) = load_catalog(storage, provider_id, &key_hash)? else {
        return Ok(None);
    };
    Ok(models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
        .and_then(|model| model.get("supportsTools"))
        .and_then(Value::as_bool))
}

fn supports_images(model: &Value, details: Option<&Value>) -> Option<bool> {
    let advertised_vision = model
        .pointer("/capabilities/vision")
        .or_else(|| details.and_then(|details| details.pointer("/capabilities/vision")))
        .and_then(Value::as_bool)
        .or_else(|| model.get("supportsImages").and_then(Value::as_bool))
        .or_else(|| {
            details
                .and_then(|details| details.get("supportsImages"))
                .and_then(Value::as_bool)
        });
    if let Some(supported) = advertised_vision {
        return Some(supported);
    }
    let modalities = [
        model.pointer("/architecture/input_modalities"),
        details.and_then(|details| details.pointer("/inputModalities")),
        details.and_then(|details| details.pointer("/modalities/input")),
    ]
    .into_iter()
    .flatten()
    .find_map(|modalities| {
        modalities
            .as_array()
            .map(|values| values.iter().any(|value| value.as_str() == Some("image")))
    });
    modalities
}

pub(super) fn mistral_reasoning_levels(_model_id: &str) -> Vec<String> {
    Vec::new()
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
    let connection = storage.connect().map_err(|_| storage_error())?;
    load_catalog_from_connection(&connection, provider_id, key_hash)
}

trait CatalogReadConnection {
    fn load_catalog_row(
        &self,
        provider_id: &str,
        key_hash: &str,
    ) -> rusqlite::Result<Option<(String, i64)>>;
}

impl CatalogReadConnection for rusqlite::Connection {
    fn load_catalog_row(
        &self,
        provider_id: &str,
        key_hash: &str,
    ) -> rusqlite::Result<Option<(String, i64)>> {
        self.query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM compatible_provider_model_catalog
             WHERE provider_id = ?1 AND api_key_hash = ?2",
            rusqlite::params![provider_id, key_hash],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
    }
}

impl CatalogReadConnection for StorageConnection<'_> {
    fn load_catalog_row(
        &self,
        provider_id: &str,
        key_hash: &str,
    ) -> rusqlite::Result<Option<(String, i64)>> {
        self.query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM compatible_provider_model_catalog
             WHERE provider_id = ?1 AND api_key_hash = ?2",
            rusqlite::params![provider_id, key_hash],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
    }
}

fn load_catalog_from_connection(
    connection: &impl CatalogReadConnection,
    provider_id: &str,
    key_hash: &str,
) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    connection
        .load_catalog_row(provider_id, key_hash)
        .map_err(|_| storage_error())?
        .map(|(models_json, fetched_at)| {
            serde_json::from_str::<Vec<Value>>(&models_json)
                .map(|models| (models, fetched_at))
                .map_err(|_| invalid_response_error())
        })
        .transpose()
}

pub(super) fn context_limits(
    storage: &AppStorage,
    provider_id: &str,
    api_key: &str,
    model_id: &str,
) -> Result<super::ModelContextLimits, ServiceError> {
    let key_hash = api_key_hash(api_key);
    let Some((models, _)) = load_catalog(storage, provider_id, &key_hash)? else {
        return Ok(super::ModelContextLimits::default());
    };
    let model = models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id));
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
    Ok(super::ModelContextLimits {
        context_window,
        input_token_limit,
        max_output_tokens,
    })
}

pub(super) fn supports_image_input(
    storage: &AppStorage,
    provider_id: &str,
    api_key: &str,
    model_id: &str,
) -> Result<bool, ServiceError> {
    let key_hash = api_key_hash(api_key);
    let Some((models, _)) = load_catalog(storage, provider_id, &key_hash)? else {
        return Ok(false);
    };
    Ok(models.iter().any(|model| {
        model.get("id").and_then(Value::as_str) == Some(model_id)
            && model.get("supportsImages").and_then(Value::as_bool) == Some(true)
    }))
}

#[cfg(test)]
fn model_context_window(models: &[Value], model_id: &str) -> Option<i64> {
    models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
        .and_then(|model| model.get("contextWindow"))
        .and_then(Value::as_i64)
        .filter(|window| *window > 0)
}

fn api_key_hash(api_key: &str) -> String {
    format!("{:x}", Sha256::digest(api_key.as_bytes()))
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rusqlite::Connection;
    use serde_json::json;

    use crate::storage::AppStorage;

    use super::{
        api_key_hash, catalog_client, has_current_limit_metadata, load_catalog_from_connection,
        model_context_window, parse_models, provider_tool_support, supports_tool_calls,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-provider-models-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    #[test]
    fn context_window_uses_only_the_selected_model_and_positive_metadata() {
        let models = vec![
            json!({"id": "known", "contextWindow": 131072}),
            json!({"id": "unknown", "contextWindow": null}),
            json!({"id": "invalid", "contextWindow": -1}),
        ];

        assert_eq!(model_context_window(&models, "known"), Some(131072));
        assert_eq!(model_context_window(&models, "unknown"), None);
        assert_eq!(model_context_window(&models, "invalid"), None);
        assert_eq!(model_context_window(&models, "missing"), None);
    }

    #[tokio::test]
    async fn provider_catalog_client_does_not_follow_redirects_with_credentials() {
        use tokio::{io::AsyncWriteExt, net::TcpListener};

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect fixture");
        let address = listener
            .local_addr()
            .expect("read redirect fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            read_request_headers(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("write redirect response");
        });

        let response = catalog_client("test provider")
            .expect("build catalog client")
            .get(format!("http://{address}/models"))
            .header("x-api-key", "test-credential")
            .send()
            .await
            .expect("redirect response should be returned as-is");

        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert_eq!(response.url().host_str(), Some("127.0.0.1"));
        server.await.expect("redirect fixture should complete");
    }

    /// Reads the loopback request before writing a deterministic fixture response.
    async fn read_request_headers(stream: &mut tokio::net::TcpStream) {
        use tokio::io::AsyncReadExt;

        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request headers");
            assert_ne!(read, 0, "client closed before sending request headers");
            request.push(byte[0]);
        }
    }

    #[test]
    fn model_catalog_keys_are_derived_without_retaining_the_api_key() {
        assert_ne!(api_key_hash("key-one"), api_key_hash("key-two"));
        assert_eq!(api_key_hash("key-one"), api_key_hash("key-one"));
    }

    #[test]
    fn cached_catalog_requires_current_prompt_limit_metadata() {
        assert!(!has_current_limit_metadata(&[json!({"id": "model"})]));
        assert!(!has_current_limit_metadata(&[json!({
            "id": "model",
            "inputTokenLimit": null,
            "supportsImages": false,
            "supportsTools": null
        })]));
        assert!(has_current_limit_metadata(&[json!({
            "id": "model",
            "inputTokenLimit": 8192,
            "supportsImages": false,
            "supportsTools": null,
            "capabilityMetadataVersion": 2
        })]));
    }

    #[test]
    fn provider_tool_capabilities_use_reported_metadata() {
        assert_eq!(
            provider_tool_support("gemini", &json!({"function_calling": true})),
            Some(true)
        );
        assert_eq!(provider_tool_support("gemini", &json!({})), None);
        assert_eq!(
            provider_tool_support("groq", &json!({"capabilities": {"tools": true}})),
            Some(true)
        );
        assert_eq!(provider_tool_support("groq", &json!({})), None);
        assert_eq!(
            provider_tool_support(
                "mistral",
                &json!({"capabilities": {"function_calling": false}})
            ),
            Some(false)
        );
        assert_eq!(provider_tool_support("mistral", &json!({})), None);
    }

    #[test]
    fn kilo_catalog_parses_free_and_paid_models_correctly() {
        let catalog = parse_models(
            "kilo",
            &json!({
                "data": [
                    {
                        "id": "kilo-auto/free",
                        "name": "Auto Free",
                        "description": "Free model router",
                        "context_length": 256000,
                        "isFree": true,
                        "supported_parameters": ["tools", "reasoning"],
                        "architecture": {
                            "input_modalities": ["text"]
                        },
                        "top_provider": {
                            "max_completion_tokens": 32768
                        }
                    },
                    {
                        "id": "anthropic/claude-opus-5.5",
                        "name": "Claude Opus 5.5",
                        "description": "Frontier intelligence",
                        "context_length": 200000,
                        "isFree": false,
                        "supported_parameters": ["tools", "reasoning"],
                        "architecture": {
                            "input_modalities": ["text", "image"]
                        },
                        "top_provider": {
                            "max_completion_tokens": 8192
                        }
                    }
                ]
            }),
            None,
        )
        .expect("parse Kilo models");

        assert_eq!(catalog.len(), 2);

        let free_model = &catalog[0];
        assert_eq!(free_model["id"], "kilo-auto/free");
        assert_eq!(free_model["groupId"], "free");
        assert_eq!(free_model["contextWindow"], 256000);
        assert_eq!(free_model["supportsTools"], true);
        assert_eq!(free_model["supportsImages"], false);
        assert_eq!(free_model["supportsReasoning"], true);

        let paid_model = &catalog[1];
        assert_eq!(paid_model["id"], "anthropic/claude-opus-5.5");
        assert_eq!(paid_model["groupId"], "models");
        assert_eq!(paid_model["contextWindow"], 200000);
        assert_eq!(paid_model["supportsTools"], true);
        assert_eq!(paid_model["supportsImages"], true);
    }

    #[test]
    fn provider_catalogs_keep_verified_context_window_fields() {
        let gemini = parse_models(
            "gemini",
            &json!({"data": [{"id": "gemini-3-flash"}]}),
            Some(&json!({
                "models": [{
                    "name": "models/gemini-3-flash",
                    "baseModelId": "gemini-3-flash",
                    "displayName": "Gemini Flash",
                    "inputTokenLimit": 1_000_000
                }]
            })),
        )
        .expect("parse Gemini models");
        assert_eq!(
            model_context_window(&gemini, "gemini-3-flash"),
            Some(1_000_000)
        );
        assert_eq!(gemini[0]["inputTokenLimit"], 1_000_000);

        let groq = parse_models(
            "groq",
            &json!({"data": [{"id": "llama", "context_window": 131072}]}),
            None,
        )
        .expect("parse Groq models");
        assert_eq!(model_context_window(&groq, "llama"), Some(131072));

        let cerebras = parse_models(
            "cerebras",
            &json!({"data": [{"id": "llama"}]}),
            Some(&json!({
                "data": [{
                    "id": "llama",
                    "limits": {"max_context_length": 131072},
                    "capabilities": {"function_calling": true, "tools": true}
                }]
            })),
        )
        .expect("parse Cerebras models");
        assert_eq!(model_context_window(&cerebras, "llama"), Some(131072));

        let openrouter = parse_models(
            "openrouter",
            &json!({"data": [{
                "id": "free-model",
                "context_length": 65536,
                "pricing": {"prompt": "0", "completion": "0"},
                "supported_parameters": ["tools"],
                "architecture": {"modality": "text->text"}
            }]}),
            None,
        )
        .expect("parse OpenRouter models");
        assert_eq!(model_context_window(&openrouter, "free-model"), Some(65536));

        let mistral = parse_models(
            "mistral",
            &json!({
                "data": [
                    {
                        "id": "mistral-small-latest",
                        "max_context_length": 131072,
                        "capabilities": {
                            "completion_chat": true,
                            "function_calling": true,
                            "vision": false
                        },
                        "archived": false
                    },
                    {
                        "id": "mistral-embed",
                        "max_context_length": 8192,
                        "capabilities": {
                            "completion_chat": false,
                            "function_calling": false,
                            "vision": false
                        },
                        "archived": false
                    }
                ]
            }),
            None,
        )
        .expect("parse Mistral models");
        assert_eq!(mistral.len(), 1);
        assert_eq!(
            model_context_window(&mistral, "mistral-small-latest"),
            Some(131072)
        );
        assert!(mistral[0]["supportsReasoning"].is_null());
        assert_eq!(mistral[0]["reasoningLevels"], json!([]));
        assert_eq!(mistral[0]["supportsTools"], true);

        let groq = parse_models(
            "groq",
            &json!({
                "data": [
                    {"id": "openai/gpt-oss-120b", "context_window": 131072},
                    {"id": "whisper-large-v3", "context_window": 448}
                ]
            }),
            None,
        )
        .expect("parse Groq models");
        assert!(groq[0]["supportsTools"].is_null());
        assert!(groq[1]["supportsTools"].is_null());

        let openrouter = parse_models(
            "openrouter",
            &json!({"data": [{
                "id": "free-model",
                "pricing": {"prompt": "0", "completion": "0"},
                "supported_parameters": ["tools"],
                "architecture": {"modality": "text->text"}
            }]}),
            None,
        )
        .expect("parse OpenRouter tool models");
        assert_eq!(openrouter[0]["supportsTools"], true);

        let cerebras = parse_models(
            "cerebras",
            &json!({"data": [{"id": "tool-model"}]}),
            Some(&json!({"data": [{
                "id": "tool-model",
                "capabilities": {"function_calling": true, "tools": true}
            }]})),
        )
        .expect("parse Cerebras tool models");
        assert_eq!(cerebras[0]["supportsTools"], true);

        let deepseek = parse_models(
            "deepseek",
            &json!({
                "data": [
                    {"id": "deepseek-chat"},
                    {"id": "deepseek-reasoner"}
                ]
            }),
            None,
        )
        .expect("parse DeepSeek models");
        assert_eq!(deepseek.len(), 2);
        assert_eq!(deepseek[0]["contextWindow"], 64_000);
        assert!(deepseek[0]["supportsTools"].is_null());
        assert!(deepseek[1]["supportsTools"].is_null());
        assert!(deepseek[1]["supportsReasoning"].is_null());

        let xai = parse_models(
            "xai",
            &json!({
                "data": [
                    {"id": "grok-2"},
                    {"id": "grok-2-vision"}
                ]
            }),
            None,
        )
        .expect("parse xAI models");
        assert_eq!(xai[0]["contextWindow"], 131_072);
        assert!(xai[0]["supportsTools"].is_null());
        assert!(xai[1]["supportsImages"].is_null());

        let perplexity = parse_models(
            "perplexity",
            &json!({
                "data": [
                    {"id": "sonar"},
                    {"id": "sonar-pro"},
                    {"id": "sonar-reasoning"}
                ]
            }),
            None,
        )
        .expect("parse Perplexity models");
        assert_eq!(perplexity[0]["contextWindow"], 128_000);
        assert_eq!(perplexity[1]["contextWindow"], 128_000);
        assert!(perplexity[2]["supportsReasoning"].is_null());
        assert!(perplexity[0]["supportsTools"].is_null());
    }

    #[test]
    fn context_window_catalog_lookup_is_scoped_to_provider_and_api_key_hash() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE compatible_provider_model_catalog (
                    provider_id TEXT NOT NULL,
                    api_key_hash TEXT NOT NULL,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    models_json TEXT NOT NULL,
                    PRIMARY KEY (provider_id, api_key_hash)
                );",
            )
            .expect("create provider catalog table");
        for (provider, key, window) in [
            ("gemini", api_key_hash("account-one"), 131072),
            ("gemini", api_key_hash("account-two"), 1_000_000),
            ("groq", api_key_hash("account-one"), 65536),
        ] {
            let models = json!([{"id": "model", "contextWindow": window}]).to_string();
            connection
                .execute(
                    "INSERT INTO compatible_provider_model_catalog
                        (provider_id, api_key_hash, fetched_at_unix_ms, models_json)
                     VALUES (?1, ?2, 1, ?3)",
                    rusqlite::params![provider, key, models],
                )
                .expect("insert model catalog");
        }

        let (first_account, _) =
            load_catalog_from_connection(&connection, "gemini", &api_key_hash("account-one"))
                .expect("read account one catalog")
                .expect("account one catalog exists");
        let (second_account, _) =
            load_catalog_from_connection(&connection, "gemini", &api_key_hash("account-two"))
                .expect("read account two catalog")
                .expect("account two catalog exists");
        let (different_provider, _) =
            load_catalog_from_connection(&connection, "groq", &api_key_hash("account-one"))
                .expect("read Groq catalog")
                .expect("Groq catalog exists");

        assert_eq!(model_context_window(&first_account, "model"), Some(131072));
        assert_eq!(
            model_context_window(&second_account, "model"),
            Some(1_000_000)
        );
        assert_eq!(
            model_context_window(&different_provider, "model"),
            Some(65536)
        );
    }

    #[test]
    fn tool_capability_catalog_lookup_is_scoped_to_provider_and_api_key_hash() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");
        storage
            .connect()
            .expect("connect storage")
            .execute_batch(
                "CREATE TABLE compatible_provider_model_catalog (
                    provider_id TEXT NOT NULL,
                    api_key_hash TEXT NOT NULL,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    models_json TEXT NOT NULL,
                    PRIMARY KEY (provider_id, api_key_hash)
                );",
            )
            .expect("create provider catalog table");
        for (provider, key, supports_tools) in [
            ("mistral", "account-one", true),
            ("mistral", "account-two", false),
            ("groq", "account-one", true),
        ] {
            let models = json!([{"id": "model", "supportsTools": supports_tools}]).to_string();
            storage
                .connect()
                .expect("connect storage")
                .execute(
                    "INSERT INTO compatible_provider_model_catalog
                        (provider_id, api_key_hash, fetched_at_unix_ms, models_json)
                     VALUES (?1, ?2, 1, ?3)",
                    rusqlite::params![provider, api_key_hash(key), models],
                )
                .expect("insert model catalog");
        }

        assert_eq!(
            supports_tool_calls(&storage, "mistral", "account-one", "model")
                .expect("read first account capability"),
            Some(true)
        );
        assert_eq!(
            supports_tool_calls(&storage, "mistral", "account-two", "model")
                .expect("read second account capability"),
            Some(false)
        );
        assert_eq!(
            supports_tool_calls(&storage, "groq", "account-one", "model")
                .expect("read other provider capability"),
            Some(true)
        );
    }
}
