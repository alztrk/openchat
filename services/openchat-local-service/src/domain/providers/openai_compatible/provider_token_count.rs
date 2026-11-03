//! Provider token-count preflight for supported first-party APIs.
//!
//! Calls are limited to requests already estimated near the compaction boundary.
//! Unsupported payloads and provider errors leave the existing estimate path in
//! control, while cancellation still stops the pending chat request.

use std::{collections::HashMap, time::Duration};

use futures_util::StreamExt;
use reqwest::{Url, header::CONTENT_TYPE};
use serde_json::{Map, Value, json};
use tokio::sync::watch;

use crate::{
    context_compaction::{self, COMPACTION_TRIGGER_PERCENT, MIN_CONTEXT_WINDOW},
    domain::http_headers::sensitive_api_key_header,
    protocol::ServiceError,
};

use super::{
    cancelled_error, client, http_error, invalid_response_error, network_error_with_details, route,
};

const TOKEN_COUNT_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_TOKEN_COUNT_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOKEN_COUNT_RESPONSE_BYTES: usize = 16 * 1024;
const TOKEN_COUNT_PROBE_PERCENT: i64 = 90;

#[derive(Clone, Copy)]
enum TokenCountAuth {
    OpenAiBearer,
    AnthropicApiKey,
    GeminiApiKey,
}

struct TokenCountEndpoint {
    provider_id: &'static str,
    url: String,
    auth: TokenCountAuth,
    response_field: &'static str,
}

/// Counts a provider request only after the local estimate reaches the nearby
/// compaction boundary. Unsupported requests return `None` for normal fallback.
pub(super) async fn count_near_context_limit(
    route: &route::ChatRoute,
    api_key: Option<&str>,
    request_body: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Option<i64>, ServiceError> {
    let Some(endpoint) = token_count_endpoint(route) else {
        return Ok(None);
    };
    let Some(api_key) = api_key else {
        return Ok(None);
    };
    let Some(context_window) = route
        .request_context_limit()
        .filter(|window| *window >= MIN_CONTEXT_WINDOW)
    else {
        return Ok(None);
    };

    let trigger = route
        .native_compaction_threshold
        .unwrap_or_else(|| context_window.saturating_mul(COMPACTION_TRIGGER_PERCENT) / 100);
    let probe_threshold = trigger.saturating_mul(TOKEN_COUNT_PROBE_PERCENT) / 100;
    let estimated_tokens =
        context_compaction::request_context_token_estimate(&route.model_id, request_body);
    if estimated_tokens.saturating_add(1024) < probe_threshold {
        return Ok(None);
    }

    let Some(body) = token_count_body(endpoint.provider_id, request_body) else {
        return Ok(None);
    };
    let Ok(encoded_body) = serde_json::to_vec(&body) else {
        log_unavailable(endpoint.provider_id, "invalid_request");
        return Ok(None);
    };
    if encoded_body.len() > MAX_TOKEN_COUNT_REQUEST_BYTES {
        return Ok(None);
    }

    match send_token_count_request(&endpoint, api_key, encoded_body, cancellation).await {
        Ok(input_tokens) => Ok(Some(input_tokens)),
        Err(error) if is_cancelled(&error) => Err(error),
        Err(error) => {
            log_unavailable(endpoint.provider_id, &error.code);
            Ok(None)
        }
    }
}

pub(super) fn supports_route(route: &route::ChatRoute) -> bool {
    token_count_endpoint(route).is_some()
}

fn token_count_endpoint(route: &route::ChatRoute) -> Option<TokenCountEndpoint> {
    match route.provider_id.as_deref() {
        Some("chatgpt_api") if route.uses_responses_api => Some(TokenCountEndpoint {
            provider_id: "chatgpt_api",
            url: "https://api.openai.com/v1/responses/input_tokens".to_owned(),
            auth: TokenCountAuth::OpenAiBearer,
            response_field: "input_tokens",
        }),
        Some("anthropic") if route.is_anthropic() => {
            let mut url = Url::parse(&route.chat_url).ok()?;
            let base_path = url.path().strip_suffix("/messages")?;
            url.set_path(&format!("{base_path}/messages/count_tokens"));
            Some(TokenCountEndpoint {
                provider_id: "anthropic",
                url: url.to_string(),
                auth: TokenCountAuth::AnthropicApiKey,
                response_field: "input_tokens",
            })
        }
        Some("gemini") => Some(TokenCountEndpoint {
            provider_id: "gemini",
            url: gemini_count_url(&route.provider_model_id)?,
            auth: TokenCountAuth::GeminiApiKey,
            response_field: "totalTokens",
        }),
        _ => None,
    }
}

fn gemini_count_url(model_id: &str) -> Option<String> {
    let model_id = model_id.strip_prefix("models/").unwrap_or(model_id);
    if model_id.is_empty()
        || !model_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
    {
        return None;
    }

    let mut url = Url::parse("https://generativelanguage.googleapis.com/v1beta/").ok()?;
    url.path_segments_mut()
        .ok()?
        .push("models")
        .push(&format!("{model_id}:countTokens"));
    Some(url.to_string())
}

fn token_count_body(provider_id: &str, request_body: &Value) -> Option<Value> {
    match provider_id {
        "chatgpt_api" => openai_responses_count_body(request_body),
        "anthropic" => anthropic_count_body(request_body),
        "gemini" => gemini_count_body(request_body),
        _ => None,
    }
}

fn openai_responses_count_body(request_body: &Value) -> Option<Value> {
    let mut body = Map::new();
    for field in [
        "model",
        "input",
        "instructions",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "reasoning",
        "text",
    ] {
        if let Some(value) = request_body.get(field) {
            body.insert(field.to_owned(), value.clone());
        }
    }
    (body.contains_key("model") && body.contains_key("input")).then_some(Value::Object(body))
}

fn anthropic_count_body(request_body: &Value) -> Option<Value> {
    let mut body = Map::new();
    for field in [
        "model",
        "messages",
        "system",
        "tools",
        "thinking",
        "output_config",
    ] {
        if let Some(value) = request_body.get(field) {
            body.insert(field.to_owned(), value.clone());
        }
    }
    (body.contains_key("model") && body.contains_key("messages")).then_some(Value::Object(body))
}

fn gemini_count_body(request_body: &Value) -> Option<Value> {
    let messages = request_body.get("messages")?.as_array()?;
    let mut contents = Vec::<Value>::new();
    let mut system_parts = Vec::<Value>::new();
    let mut function_names = HashMap::<String, String>::new();

    for message in messages {
        match message.get("role")?.as_str()? {
            "system" => {
                system_parts.extend(gemini_parts(message.get("content"))?);
            }
            "user" | "assistant" => {
                let role = if message.get("role")?.as_str()? == "assistant" {
                    "model"
                } else {
                    "user"
                };
                let mut parts = gemini_parts(message.get("content"))?;
                if let Some(tool_calls) = message.get("tool_calls") {
                    for call in tool_calls.as_array()? {
                        let call_id = call.get("id")?.as_str()?;
                        let function = call.get("function")?;
                        let name = function.get("name")?.as_str()?;
                        let arguments = function.get("arguments")?.as_str()?;
                        let arguments: Value = serde_json::from_str(arguments).ok()?;
                        if !arguments.is_object() {
                            return None;
                        }
                        function_names.insert(call_id.to_owned(), name.to_owned());
                        parts.push(json!({
                            "functionCall": {
                                "name": name,
                                "args": arguments,
                            },
                        }));
                    }
                }
                append_gemini_content(&mut contents, role, parts);
            }
            "tool" => {
                let call_id = message.get("tool_call_id")?.as_str()?;
                let name = function_names.get(call_id)?;
                let content = message.get("content")?.as_str()?;
                let value = serde_json::from_str::<Value>(content)
                    .unwrap_or_else(|_| json!({"content": content}));
                let response = if value.is_object() {
                    value
                } else {
                    json!({"result": value})
                };
                append_gemini_content(
                    &mut contents,
                    "user",
                    vec![json!({
                        "functionResponse": {
                            "name": name,
                            "response": response,
                        },
                    })],
                );
            }
            _ => return None,
        }
    }

    if contents.is_empty() {
        return None;
    }
    let mut generate_request = json!({"contents": contents});
    if !system_parts.is_empty() {
        generate_request["systemInstruction"] = json!({"parts": system_parts});
    }
    if let Some(tools) = request_body.get("tools") {
        let mut declarations = Vec::new();
        for tool in tools.as_array()? {
            if tool.get("type")?.as_str()? != "function" {
                return None;
            }
            let function = tool.get("function")?;
            let mut declaration = Map::new();
            declaration.insert(
                "name".to_owned(),
                Value::String(function.get("name")?.as_str()?.to_owned()),
            );
            if let Some(description) = function.get("description").and_then(Value::as_str) {
                declaration.insert(
                    "description".to_owned(),
                    Value::String(description.to_owned()),
                );
            }
            if let Some(parameters) = function.get("parameters") {
                if !parameters.is_object() {
                    return None;
                }
                declaration.insert("parameters".to_owned(), parameters.clone());
            }
            declarations.push(Value::Object(declaration));
        }
        if !declarations.is_empty() {
            generate_request["tools"] = json!([{"functionDeclarations": declarations}]);
        }
    }

    Some(json!({"generateContentRequest": generate_request}))
}

fn gemini_parts(content: Option<&Value>) -> Option<Vec<Value>> {
    match content {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::String(text)) => Some(
            (!text.is_empty())
                .then(|| json!({"text": text}))
                .into_iter()
                .collect(),
        ),
        Some(Value::Array(parts)) => {
            let mut converted = Vec::new();
            for part in parts {
                match part.get("type")?.as_str()? {
                    "text" | "input_text" => {
                        if let Some(text) = part.get("text").and_then(Value::as_str)
                            && !text.is_empty()
                        {
                            converted.push(json!({"text": text}));
                        }
                    }
                    "image_url" => {
                        let image_url = part
                            .pointer("/image_url/url")
                            .or_else(|| part.get("image_url"))
                            .and_then(Value::as_str)?;
                        converted.push(gemini_inline_image(image_url)?);
                    }
                    _ => return None,
                }
            }
            Some(converted)
        }
        _ => None,
    }
}

fn gemini_inline_image(image_url: &str) -> Option<Value> {
    let data_url = image_url.strip_prefix("data:")?;
    let (mime_type, encoded) = data_url.split_once(";base64,")?;
    if !matches!(
        mime_type,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    ) || encoded.is_empty()
        || encoded
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'+' | b'/' | b'='))
    {
        return None;
    }
    Some(json!({
        "inlineData": {
            "mimeType": mime_type,
            "data": encoded,
        },
    }))
}

fn append_gemini_content(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    if let Some(last) = contents.last_mut()
        && last.get("role").and_then(Value::as_str) == Some(role)
        && let Some(existing_parts) = last.get_mut("parts").and_then(Value::as_array_mut)
    {
        existing_parts.extend(parts);
    } else {
        contents.push(json!({"role": role, "parts": parts}));
    }
}

async fn send_token_count_request(
    endpoint: &TokenCountEndpoint,
    api_key: &str,
    encoded_body: Vec<u8>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<i64, ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }

    let mut request = client()?
        .post(&endpoint.url)
        .header(CONTENT_TYPE, "application/json")
        .timeout(TOKEN_COUNT_TIMEOUT);
    match endpoint.auth {
        TokenCountAuth::OpenAiBearer => request = request.bearer_auth(api_key),
        TokenCountAuth::AnthropicApiKey => {
            let credential = sensitive_api_key_header(api_key).map_err(|_| {
                ServiceError::new(
                    "invalid_provider_credential",
                    "The configured provider API key is invalid.",
                    false,
                )
            })?;
            request = request
                .header("x-api-key", credential)
                .header("anthropic-version", "2023-06-01");
        }
        TokenCountAuth::GeminiApiKey => {
            let credential = sensitive_api_key_header(api_key).map_err(|_| {
                ServiceError::new(
                    "invalid_provider_credential",
                    "The configured provider API key is invalid.",
                    false,
                )
            })?;
            request = request.header("x-goog-api-key", credential);
        }
    }

    let response = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        response = request.body(encoded_body).send() => {
            response.map_err(network_error_with_details)?
        }
    };
    if !response.status().is_success() {
        return Err(http_error(response.status(), Some(endpoint.provider_id)));
    }

    let mut stream = response.bytes_stream();
    let mut encoded = Vec::new();
    loop {
        let next = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(network_error_with_details)?;
        if encoded.len().saturating_add(chunk.len()) > MAX_TOKEN_COUNT_RESPONSE_BYTES {
            return Err(invalid_response_error());
        }
        encoded.extend_from_slice(&chunk);
    }

    let response: Value = serde_json::from_slice(&encoded).map_err(|_| invalid_response_error())?;
    response
        .get(endpoint.response_field)
        .and_then(Value::as_i64)
        .filter(|tokens| *tokens >= 0)
        .ok_or_else(invalid_response_error)
}

fn is_cancelled(error: &ServiceError) -> bool {
    matches!(error.code, "operation_cancelled" | "request_cancelled")
}

fn log_unavailable(provider_id: &str, code: &str) {
    eprintln!("provider_token_count_unavailable:{provider_id}:{code}");
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        anthropic_count_body, gemini_count_body, gemini_count_url, openai_responses_count_body,
    };

    #[test]
    fn openai_count_body_keeps_prompt_fields_and_omits_stream_controls() {
        let body = openai_responses_count_body(&json!({
            "model": "gpt-6",
            "input": [{"role": "user", "content": "Continue"}],
            "tools": [{"type": "function", "name": "read_file"}],
            "reasoning": {"effort": "high"},
            "stream": true,
            "store": false,
            "context_management": [{"type": "compaction"}],
        }))
        .expect("valid Responses request is countable");

        assert_eq!(body["model"], "gpt-6");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["tools"][0]["name"], "read_file");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert!(body.get("stream").is_none());
        assert!(body.get("store").is_none());
        assert!(body.get("context_management").is_none());
    }

    #[test]
    fn anthropic_count_body_keeps_messages_tools_and_checkpoint_content() {
        let body = anthropic_count_body(&json!({
            "model": "claude-sonnet",
            "messages": [{
                "role": "assistant",
                "content": [{"type": "compaction", "content": "opaque"}],
            }],
            "system": "Instructions",
            "tools": [{"name": "read_file", "input_schema": {"type": "object"}}],
            "max_tokens": 4096,
            "stream": true,
            "context_management": {"edits": []},
        }))
        .expect("valid Messages request is countable");

        assert_eq!(body["messages"][0]["content"][0]["type"], "compaction");
        assert_eq!(body["system"], "Instructions");
        assert!(body["tools"].is_array());
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("stream").is_none());
        assert!(body.get("context_management").is_none());
    }

    #[test]
    fn gemini_count_body_maps_system_images_function_calls_and_results() {
        let body = gemini_count_body(&json!({
            "messages": [
                {"role": "system", "content": "System instruction"},
                {"role": "user", "content": [
                    {"type": "text", "text": "Describe this"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,aGVsbG8="}},
                ]},
                {"role": "assistant", "content": null, "tool_calls": [{
                    "id": "call-1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": "{\"query\":\"x\"}"},
                }]},
                {"role": "tool", "tool_call_id": "call-1", "content": "{\"ok\":true}"},
            ],
            "tools": [{"type": "function", "function": {
                "name": "lookup",
                "description": "Look up a record",
                "parameters": {"type": "object", "properties": {"query": {"type": "string"}}},
            }}],
            "stream": true,
        }))
        .expect("text, inline image and function history are countable");

        let generate = &body["generateContentRequest"];
        assert_eq!(
            generate["systemInstruction"]["parts"][0]["text"],
            "System instruction"
        );
        assert_eq!(generate["contents"][0]["role"], "user");
        assert_eq!(
            generate["contents"][0]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(generate["contents"][1]["role"], "model");
        assert_eq!(
            generate["contents"][1]["parts"][0]["functionCall"]["args"]["query"],
            "x"
        );
        assert_eq!(generate["contents"][2]["role"], "user");
        assert_eq!(
            generate["contents"][2]["parts"][0]["functionResponse"]["response"]["ok"],
            true
        );
        assert_eq!(
            generate["tools"][0]["functionDeclarations"][0]["name"],
            "lookup"
        );
    }

    #[test]
    fn gemini_count_skips_external_images_and_invalid_model_names() {
        assert!(
            gemini_count_body(&json!({
                "messages": [{"role": "user", "content": [{
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/image.png"},
                }]}],
            }))
            .is_none()
        );
        assert!(gemini_count_url("models/gemini-2.5-flash").is_some());
        assert!(gemini_count_url("models/org/model").is_none());
    }
}
