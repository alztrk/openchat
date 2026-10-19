//! Implements Copilot-compatible model, usage, and inference routes.
//!
//! Requests use the active OAuth connection and workspace; route selection and
//! response validation are kept separate from general compatible providers.

use std::{
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, RequestBuilder, header::ACCEPT, redirect::Policy};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use url::Url;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{cancelled_error, http_error, invalid_response_error, network_error, storage_error};

const PROVIDER_ID: &str = "copilot";
const USER_URL: &str = "https://api.github.com/copilot_internal/user";
const GITHUB_API_VERSION: &str = "2025-04-01";
const COPILOT_API_VERSION: &str = "2026-01-09";
const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_MODELS: usize = 1000;
const MAX_USER_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_MODEL_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelEndpoint {
    ChatCompletions,
    Responses,
    Messages,
}

#[derive(Clone, Copy)]
pub(super) enum RequestIntent {
    Conversation,
    Background,
}

impl RequestIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Conversation => "conversation-panel",
            Self::Background => "conversation-background",
        }
    }
}

impl ModelEndpoint {
    fn path(self) -> &'static str {
        match self {
            Self::ChatCompletions => "/chat/completions",
            Self::Responses => "/responses",
            Self::Messages => "/v1/messages",
        }
    }

    fn protocol(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chatCompletions",
            Self::Responses => "responses",
            Self::Messages => "messages",
        }
    }

    fn from_metadata(value: &Value) -> Option<Self> {
        let endpoints = value.get("supported_endpoints")?.as_array()?;
        let contains = |endpoint: &str| {
            endpoints
                .iter()
                .any(|value| value.as_str() == Some(endpoint))
        };
        if contains("/responses") {
            Some(Self::Responses)
        } else if contains("/v1/messages") {
            Some(Self::Messages)
        } else if contains("/chat/completions") {
            Some(Self::ChatCompletions)
        } else {
            None
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "/chat/completions" => Some(Self::ChatCompletions),
            "/responses" => Some(Self::Responses),
            "/v1/messages" => Some(Self::Messages),
            _ => None,
        }
    }
}

pub(super) struct ModelRoute {
    pub(super) chat_url: String,
    pub(super) uses_responses_api: bool,
    pub(super) reasoning_levels: Vec<String>,
    pub(super) context_window: Option<i64>,
    pub(super) input_token_limit: Option<i64>,
    pub(super) max_output_tokens: Option<i64>,
    pub(super) supports_images: bool,
    pub(super) supports_tool_calls: Option<bool>,
}

pub(super) async fn models(
    storage: &AppStorage,
    access_token: &str,
    connection_id: &str,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    validate_access_token(access_token)?;
    validate_connection_id(connection_id)?;
    let key_hash = account_hash(storage)?;
    let cached = load_catalog(storage, &key_hash)?;
    if !force_refresh
        && let Some((models, fetched_at)) = &cached
        && current_time_millis()?.saturating_sub(*fetched_at) < CACHE_AGE_MS
    {
        return Ok(response(models, "current"));
    }

    let fetched = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        result = fetch_models(access_token) => result,
    };
    match fetched {
        Ok(models) => {
            save_catalog(storage, &key_hash, &models)?;
            Ok(response(&models, "current"))
        }
        Err(error) if error.retryable => match cached {
            Some((models, _)) => Ok(response(&models, "stale")),
            None => Err(error),
        },
        Err(error) => Err(error),
    }
}

pub(super) async fn usage(
    access_token: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    validate_access_token(access_token)?;
    let user = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        result = fetch_user(access_token) => result?,
    };
    Ok(parse_usage_snapshot(&user))
}

pub(super) fn model_route(
    storage: &AppStorage,
    connection_id: &str,
    model_id: &str,
) -> Result<Option<ModelRoute>, ServiceError> {
    validate_connection_id(connection_id)?;
    let Some((models, _)) = load_catalog(storage, &account_hash(storage)?)? else {
        return Ok(None);
    };
    let Some(model) = models
        .iter()
        .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
    else {
        return Ok(None);
    };
    let Some(endpoint) = model
        .get("_copilotEndpoint")
        .and_then(Value::as_str)
        .and_then(ModelEndpoint::parse)
    else {
        return Ok(None);
    };
    let Some(api_base) = model
        .get("_copilotApiBase")
        .and_then(Value::as_str)
        .and_then(validated_api_base)
    else {
        return Err(invalid_response_error());
    };

    Ok(Some(ModelRoute {
        chat_url: format!("{api_base}{}", endpoint.path()),
        uses_responses_api: endpoint == ModelEndpoint::Responses,
        reasoning_levels: model
            .get("reasoningLevels")
            .and_then(Value::as_array)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        context_window: positive_integer(model.get("contextWindow")),
        input_token_limit: positive_integer(model.get("inputTokenLimit")),
        max_output_tokens: positive_integer(model.get("maxOutputTokens")),
        supports_images: model
            .get("supportsImages")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        supports_tool_calls: model.get("supportsTools").and_then(Value::as_bool),
    }))
}

fn authorize_user(request: RequestBuilder, access_token: &str) -> RequestBuilder {
    request
        .bearer_auth(access_token)
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
}

fn authorize_catalog(request: RequestBuilder, access_token: &str) -> RequestBuilder {
    request.bearer_auth(access_token)
}

pub(super) fn authorize_inference(
    request: RequestBuilder,
    access_token: &str,
    intent: RequestIntent,
) -> RequestBuilder {
    let intent = intent.as_str();
    request
        .bearer_auth(access_token)
        .header("X-GitHub-Api-Version", COPILOT_API_VERSION)
        .header("OpenAI-Intent", intent)
        .header("X-Interaction-Type", intent)
        .header("X-Request-Id", uuid::Uuid::new_v4().to_string())
}

pub(super) fn client() -> Result<Client, ServiceError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .redirect(Policy::none())
        .build()
        .map_err(|_| network_error())?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

fn catalog_client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(5)
        .redirect(Policy::none())
        .build()
        .map_err(|_| network_error())
}

async fn fetch_models(access_token: &str) -> Result<Vec<Value>, ServiceError> {
    let client = catalog_client()?;
    let user = fetch_user_with_client(&client, access_token).await?;
    let api_base = user
        .pointer("/endpoints/api")
        .and_then(Value::as_str)
        .and_then(validated_api_base)
        .ok_or_else(invalid_response_error)?;

    let response = authorize_catalog(
        client
            .get(format!("{api_base}/models"))
            .header(ACCEPT, "application/json"),
        access_token,
    )
    .send()
    .await
    .map_err(|_| network_error())?;
    if !response.status().is_success() {
        return Err(http_error(response.status(), Some(PROVIDER_ID)));
    }
    let value = read_json_response(response, MAX_MODEL_RESPONSE_BYTES).await?;
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_MODELS)
        .ok_or_else(invalid_response_error)?;
    let models = parse_models(items, &api_base);
    if models.is_empty() {
        return Err(invalid_response_error());
    }
    Ok(models)
}

async fn fetch_user(access_token: &str) -> Result<Value, ServiceError> {
    let client = catalog_client()?;
    fetch_user_with_client(&client, access_token).await
}

async fn fetch_user_with_client(
    client: &Client,
    access_token: &str,
) -> Result<Value, ServiceError> {
    let response = authorize_user(
        client.get(USER_URL).header(ACCEPT, "application/json"),
        access_token,
    )
    .send()
    .await
    .map_err(|_| network_error())?;
    if !response.status().is_success() {
        return Err(http_error(response.status(), Some(PROVIDER_ID)));
    }
    read_json_response(response, MAX_USER_RESPONSE_BYTES).await
}

fn parse_usage_snapshot(user: &Value) -> Value {
    let quotas = user
        .get("quota_snapshots")
        .and_then(Value::as_object)
        .map(|snapshots| {
            snapshots
                .iter()
                .enumerate()
                .filter_map(|(index, (key, snapshot))| {
                    let kind = match key.as_str() {
                        "chat" => "chat",
                        "completions" => "completions",
                        "premium_interactions" => "premiumInteractions",
                        _ => "other",
                    };
                    parse_quota_snapshot(
                        snapshot,
                        kind,
                        if kind == "other" {
                            format!("other-{index}")
                        } else {
                            kind.to_owned()
                        },
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "planType": safe_account_label(user.get("copilot_plan")),
        "accessType": safe_account_label(user.get("access_type_sku")),
        "quotas": quotas,
    })
}

fn parse_quota_snapshot(value: &Value, kind: &str, id: String) -> Option<Value> {
    if kind == "other" {
        return Some(json!({
            "id": id,
            "kind": kind,
            "entitlement": null,
            "remaining": null,
            "percentRemaining": null,
            "resetAtUnixMs": null,
            "unlimited": null,
            "hasQuota": null,
            "tokenBasedBilling": null,
        }));
    }
    let object = value.as_object()?;
    let entitlement = safe_quota_number(object.get("entitlement"), true);
    let remaining = object
        .get("quota_remaining")
        .or_else(|| object.get("remaining"));
    let unlimited = match object.get("unlimited").and_then(Value::as_bool) {
        Some(true) => Some(true),
        Some(false) if entitlement == Some(-1.0) => Some(true),
        Some(value) => Some(value),
        None if entitlement == Some(-1.0) => Some(true),
        None => None,
    };

    Some(json!({
        "id": id,
        "kind": kind,
        "entitlement": entitlement,
        "remaining": safe_quota_number(remaining, false),
        "percentRemaining": safe_percent(object.get("percent_remaining")),
        "resetAtUnixMs": safe_reset_time(object.get("quota_reset_at")),
        "unlimited": unlimited,
        "hasQuota": object.get("has_quota").and_then(Value::as_bool),
        "tokenBasedBilling": object.get("token_based_billing").and_then(Value::as_bool),
    }))
}

fn safe_account_label(value: Option<&Value>) -> Option<&str> {
    let label = value?.as_str()?.trim();
    (!label.is_empty() && label.len() <= 128 && !label.chars().any(char::is_control))
        .then_some(label)
}

fn safe_quota_number(value: Option<&Value>, allow_unlimited_sentinel: bool) -> Option<f64> {
    let number = value?.as_f64()?;
    let maximum_safe_integer = 9_007_199_254_740_991_f64;
    if !number.is_finite()
        || number.abs() > maximum_safe_integer
        || (number < 0.0 && !(allow_unlimited_sentinel && number == -1.0))
    {
        None
    } else {
        Some(number)
    }
}

fn safe_percent(value: Option<&Value>) -> Option<f64> {
    safe_quota_number(value, false).filter(|percent| *percent <= 100.0)
}

fn safe_reset_time(value: Option<&Value>) -> Option<i64> {
    let seconds = value?.as_i64().or_else(|| {
        value
            .and_then(Value::as_u64)
            .and_then(|seconds| i64::try_from(seconds).ok())
    })?;
    if seconds < 0 {
        return None;
    }
    seconds
        .checked_mul(1000)
        .filter(|milliseconds| *milliseconds <= 8_640_000_000_000_000)
}

async fn read_json_response(
    mut response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Value, ServiceError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(invalid_response_error());
    }
    let capacity = response
        .content_length()
        .map(|length| (length as usize).min(maximum_bytes))
        .unwrap_or_default();
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await.map_err(|_| network_error())? {
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(invalid_response_error());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| invalid_response_error())
}

fn parse_models(items: &[Value], api_base: &str) -> Vec<Value> {
    items
        .iter()
        .filter_map(|item| {
            if item.pointer("/capabilities/type").and_then(Value::as_str) != Some("chat")
                || item.get("model_picker_enabled").and_then(Value::as_bool) == Some(false)
            {
                return None;
            }
            let id = item.get("id")?.as_str()?.trim();
            if id.is_empty() || id.len() > 512 || id.chars().any(char::is_control) {
                return None;
            }
            let endpoint = ModelEndpoint::from_metadata(item);
            let supports = item.get("capabilities")?.get("supports");
            let limits = item.get("capabilities")?.get("limits");
            let supports_streaming =
                supports.and_then(|supports| supports.get("streaming")) == Some(&Value::Bool(true));
            let input_token_limit = positive_integer(
                limits.and_then(|limits| limits.get("max_prompt_tokens")),
            );
            let context_window = positive_integer(
                limits.and_then(|limits| limits.get("max_context_window_tokens")),
            )
            .or(input_token_limit);
            let reasoning_levels = if endpoint == Some(ModelEndpoint::Responses) {
                supports
                    .and_then(|supports| supports.get("reasoning_effort"))
                    .and_then(Value::as_array)
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(Value::as_str)
                            .filter(|level| {
                                matches!(
                                    *level,
                                    "low" | "medium" | "high" | "xhigh" | "minimal"
                                )
                            })
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut model = json!({
                "id": id,
                "displayName": item.get("name").and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty()).unwrap_or(id),
                "description": Value::Null,
                "contextWindow": context_window,
                "supportsImages": supports.and_then(|supports| supports.get("vision")).and_then(Value::as_bool).unwrap_or(false),
                "supportsTools": supports.and_then(|supports| supports.get("tool_calls")).and_then(Value::as_bool),
                "inputTokenLimit": input_token_limit,
                "maxOutputTokens": positive_integer(limits.and_then(|limits| limits.get("max_output_tokens"))),
                "groupId": item.get("model_picker_category").and_then(Value::as_str)
                    .filter(|category| !category.trim().is_empty() && category.len() <= 128 && !category.chars().any(char::is_control))
                    .unwrap_or("models"),
                "requestProtocol": endpoint.map(ModelEndpoint::protocol),
                "supportsReasoning": !reasoning_levels.is_empty(),
                "reasoningLevels": reasoning_levels,
                "isAvailable": endpoint.is_some() && supports_streaming,
                "_copilotApiBase": api_base,
                "_copilotEndpoint": endpoint.map(|endpoint| endpoint.path()),
            });
            if let Some(max_output) = model
                .get("maxOutputTokens")
                .and_then(Value::as_i64)
                .filter(|value| *value > 0)
                && let Some(context) = model
                    .get("contextWindow")
                    .and_then(Value::as_i64)
                    .filter(|value| *value > 0)
                && max_output > context
            {
                model["maxOutputTokens"] = json!(context);
            }
            Some(model)
        })
        .take(MAX_MODELS)
        .collect()
}

fn validate_access_token(access_token: &str) -> Result<(), ServiceError> {
    if !access_token.is_empty()
        && access_token.len() <= 4096
        && access_token
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte))
    {
        Ok(())
    } else {
        Err(ServiceError::new(
            "provider_credential_invalid",
            "The GitHub Copilot OAuth access token is invalid. Reconnect your account.",
            false,
        ))
    }
}

fn validate_connection_id(connection_id: &str) -> Result<(), ServiceError> {
    if connection_id == "copilot" {
        Ok(())
    } else {
        Err(ServiceError::new(
            "provider_credential_invalid",
            "The GitHub Copilot account route is invalid.",
            false,
        ))
    }
}

fn validated_api_base(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if url.scheme() != "https"
        || !matches!(url.port(), None | Some(443))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || !is_allowed_api_host(&host)
    {
        return None;
    }
    url.set_path("");
    Some(url.to_string().trim_end_matches('/').to_owned())
}

fn is_allowed_api_host(host: &str) -> bool {
    host == "api.githubcopilot.com"
        || [
            ".individual.githubcopilot.com",
            ".business.githubcopilot.com",
            ".enterprise.githubcopilot.com",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
        || is_ghe_copilot_proxy(host)
}

fn is_ghe_copilot_proxy(host: &str) -> bool {
    let labels = host.split('.').collect::<Vec<_>>();
    labels.len() == 4
        && labels[0] == "copilot-proxy"
        && !labels[1].is_empty()
        && labels[1]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && labels[2] == "ghe"
        && labels[3] == "com"
}

fn response(models: &[Value], freshness: &str) -> Value {
    let models = models
        .iter()
        .map(|model| {
            let mut model = model.clone();
            if let Some(object) = model.as_object_mut() {
                object.remove("_copilotApiBase");
                object.remove("_copilotEndpoint");
            }
            model
        })
        .collect::<Vec<_>>();
    json!({"models": models, "freshness": freshness})
}

fn positive_integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).filter(|value| *value > 0)
}

fn account_hash(storage: &AppStorage) -> Result<String, ServiceError> {
    let github_user_id = storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT github_user_id FROM copilot_oauth_connection WHERE id = 'copilot' AND auth_status = 'active'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| storage_error())?
        .ok_or_else(|| super::provider_authentication_required_error("copilot"))?;
    let digest = Sha256::digest(github_user_id.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn load_catalog(
    storage: &AppStorage,
    key_hash: &str,
) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM compatible_provider_model_catalog
             WHERE provider_id = ?1 AND api_key_hash = ?2",
            rusqlite::params![PROVIDER_ID, key_hash],
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
            "INSERT INTO compatible_provider_model_catalog
                (provider_id, api_key_hash, fetched_at_unix_ms, models_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider_id, api_key_hash) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                models_json = excluded.models_json",
            rusqlite::params![PROVIDER_ID, key_hash, current_time_millis()?, models_json],
        )
        .map_err(|_| storage_error())?;
    Ok(())
}

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| storage_error())
}

#[cfg(test)]
mod tests {
    use reqwest::Client;
    use serde_json::{Value, json};

    use super::{
        ModelEndpoint, RequestIntent, authorize_inference, is_allowed_api_host, parse_models,
        parse_usage_snapshot, validated_api_base,
    };

    #[test]
    fn inference_auth_uses_capi_headers_and_conversation_intent() {
        let request = authorize_inference(
            Client::new().post("https://api.githubcopilot.com/responses"),
            "gho_test-token",
            RequestIntent::Conversation,
        )
        .build()
        .expect("valid request");

        assert_eq!(request.headers()["authorization"], "Bearer gho_test-token");
        assert_eq!(request.headers()["x-github-api-version"], "2026-01-09");
        assert_eq!(request.headers()["openai-intent"], "conversation-panel");
        assert_eq!(
            request.headers()["x-interaction-type"],
            "conversation-panel"
        );
        assert!(request.headers().contains_key("x-request-id"));
    }

    #[test]
    fn endpoint_selection_prefers_responses_then_messages_then_chat_completions() {
        assert_eq!(
            ModelEndpoint::from_metadata(
                &json!({"supported_endpoints": ["/chat/completions", "/responses", "/v1/messages"]})
            ),
            Some(ModelEndpoint::Responses)
        );
        assert_eq!(
            ModelEndpoint::from_metadata(
                &json!({"supported_endpoints": ["/chat/completions", "/v1/messages"]})
            ),
            Some(ModelEndpoint::Messages)
        );
        assert_eq!(
            ModelEndpoint::from_metadata(&json!({"supported_endpoints": ["/chat/completions"]})),
            Some(ModelEndpoint::ChatCompletions)
        );
        assert_eq!(
            ModelEndpoint::from_metadata(&json!({"supported_endpoints": ["ws:/responses"]})),
            None
        );
    }

    #[test]
    fn account_api_host_must_be_a_known_https_host() {
        assert_eq!(
            validated_api_base("https://api.githubcopilot.com/"),
            Some("https://api.githubcopilot.com".to_owned())
        );
        assert!(is_allowed_api_host("api.individual.githubcopilot.com"));
        assert!(is_allowed_api_host("copilot-proxy.acme.ghe.com"));
        assert!(validated_api_base("http://api.githubcopilot.com/").is_none());
        assert!(validated_api_base("https://api.githubcopilot.com.attacker.test/").is_none());
        assert!(validated_api_base("https://attacker.test/").is_none());
    }

    #[test]
    fn catalog_keeps_only_chat_models_and_marks_unknown_transports_unavailable() {
        let models = parse_models(
            &[
                json!({
                    "id": "supported-model",
                    "name": "Supported",
                    "model_picker_enabled": true,
                    "supported_endpoints": ["/responses"],
                    "capabilities": {
                        "type": "chat",
                        "limits": {"max_prompt_tokens": 12000, "max_output_tokens": 2000},
                        "supports": {"streaming": true, "vision": true, "tool_calls": true, "reasoning_effort": ["low", "high"]}
                    }
                }),
                json!({
                    "id": "socket-only",
                    "name": "Socket only",
                    "model_picker_enabled": true,
                    "supported_endpoints": ["ws:/responses"],
                    "capabilities": {"type": "chat", "supports": {}}
                }),
                json!({
                    "id": "non-streaming",
                    "name": "Non-streaming",
                    "model_picker_enabled": true,
                    "supported_endpoints": ["/responses"],
                    "capabilities": {"type": "chat", "supports": {"streaming": false}}
                }),
                json!({
                    "id": "embedding",
                    "name": "Embedding",
                    "model_picker_enabled": true,
                    "supported_endpoints": ["/responses"],
                    "capabilities": {"type": "embeddings", "supports": {}}
                }),
            ],
            "https://api.githubcopilot.com",
        );

        assert_eq!(models.len(), 3);
        assert_eq!(models[0]["isAvailable"], true);
        assert_eq!(models[0]["_copilotEndpoint"], "/responses");
        assert_eq!(models[0]["supportsImages"], true);
        assert_eq!(models[0]["supportsTools"], true);
        assert_eq!(models[0]["reasoningLevels"], json!(["low", "high"]));
        assert_eq!(models[1]["isAvailable"], false);
        assert_eq!(models[2]["isAvailable"], false);
    }

    #[test]
    fn usage_snapshot_keeps_only_documented_account_and_quota_fields() {
        let snapshot = parse_usage_snapshot(&json!({
            "copilot_plan": "business",
            "access_type_sku": "business_seat",
            "quota_snapshots": {
                "chat": {
                    "entitlement": 100,
                    "quota_remaining": 60,
                    "percent_remaining": 60,
                    "quota_reset_at": 1_900_000_000_i64,
                    "has_quota": true,
                    "token_based_billing": false
                },
                "unknown_internal_bucket": {
                    "entitlement": 999,
                    "quota_remaining": 900,
                    "percent_remaining": 90
                }
            },
            "analytics_tracking_id": "must-not-leave-the-service"
        }));

        assert_eq!(snapshot["planType"], "business");
        assert_eq!(snapshot["accessType"], "business_seat");
        let quotas = snapshot["quotas"].as_array().expect("quota list");
        assert_eq!(quotas.len(), 2);
        let chat = quotas
            .iter()
            .find(|quota| quota["kind"] == "chat")
            .expect("chat quota");
        assert_eq!(chat["entitlement"], 100.0);
        assert_eq!(chat["remaining"], 60.0);
        assert_eq!(chat["percentRemaining"], 60.0);
        assert_eq!(chat["resetAtUnixMs"], 1_900_000_000_000_i64);
        assert_eq!(chat["hasQuota"], true);

        let unknown = quotas
            .iter()
            .find(|quota| quota["kind"] == "other")
            .expect("unknown quota marker");
        assert_eq!(unknown["entitlement"], Value::Null);
        assert_eq!(unknown["remaining"], Value::Null);
        assert_eq!(snapshot.get("analytics_tracking_id"), None);
    }

    #[test]
    fn usage_snapshot_preserves_zero_and_unlimited_without_inventing_missing_values() {
        let snapshot = parse_usage_snapshot(&json!({
            "copilot_plan": "",
            "access_type_sku": 42,
            "quota_snapshots": {
                "premium_interactions": {
                    "entitlement": -1,
                    "remaining": 0,
                    "percent_remaining": 0,
                    "quota_reset_at": 0,
                    "unlimited": true
                },
                "chat": {
                    "entitlement": -2,
                    "quota_remaining": -1,
                    "percent_remaining": 101,
                    "quota_reset_at": -5
                },
                "completions": {
                    "quota_remaining": 5,
                    "quota_reset_at": 9_000_000_000_000_i64
                }
            }
        }));

        assert_eq!(snapshot["planType"], Value::Null);
        assert_eq!(snapshot["accessType"], Value::Null);
        let quotas = snapshot["quotas"].as_array().expect("quota list");
        let premium = quotas
            .iter()
            .find(|quota| quota["kind"] == "premiumInteractions")
            .expect("premium quota");
        assert_eq!(premium["entitlement"], -1.0);
        assert_eq!(premium["remaining"], 0.0);
        assert_eq!(premium["percentRemaining"], 0.0);
        assert_eq!(premium["resetAtUnixMs"], 0);
        assert_eq!(premium["unlimited"], true);

        let chat = quotas
            .iter()
            .find(|quota| quota["kind"] == "chat")
            .expect("chat quota");
        assert_eq!(chat["entitlement"], Value::Null);
        assert_eq!(chat["remaining"], Value::Null);
        assert_eq!(chat["percentRemaining"], Value::Null);
        assert_eq!(chat["resetAtUnixMs"], Value::Null);
        let completions = quotas
            .iter()
            .find(|quota| quota["kind"] == "completions")
            .expect("completions quota");
        assert_eq!(completions["remaining"], 5.0);
        assert_eq!(completions["resetAtUnixMs"], Value::Null);
    }

    #[test]
    fn usage_snapshot_marks_absent_quota_data_unavailable() {
        let snapshot = parse_usage_snapshot(&json!({"copilot_plan": "individual"}));
        assert_eq!(snapshot["planType"], "individual");
        assert_eq!(snapshot["quotas"], json!([]));
    }
}
