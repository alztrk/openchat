use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};

use crate::{
    chatgpt_store::AssistantMessageWrite, protocol::ServiceError, provider_schema::ToolDefinition,
    storage::AppStorage, tools,
};

mod models;
mod provider_models;
mod request;
mod route;
mod stream;
pub use models::models;
pub use provider_models::models as models_for_provider;

pub(crate) fn context_usage_tool_definitions(
    storage: &AppStorage,
    provider_id: &str,
    model_id: Option<&str>,
) -> Result<Vec<(String, Value)>, ServiceError> {
    let uses_responses_api = if provider_id == "opencode" {
        match model_id {
            Some(model_id) => models::is_responses_api_model(storage, model_id)?,
            None => false,
        }
    } else {
        false
    };
    Ok(tools::context_usage_definitions(
        provider_id,
        uses_responses_api,
    ))
}
const CHAT_URL: &str = "https://opencode.ai/zen/v1/chat/completions";
const RESPONSES_URL: &str = "https://opencode.ai/zen/v1/responses";
const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";
const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_PROVIDER_ERROR_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy)]
pub(super) struct ApiCompatibleProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
}

pub(super) fn api_compatible_provider(provider_id: &str) -> Option<ApiCompatibleProvider> {
    match provider_id {
        "gemini" => Some(ApiCompatibleProvider {
            id: "gemini",
            name: "Gemini",
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        }),
        "groq" => Some(ApiCompatibleProvider {
            id: "groq",
            name: "Groq",
            base_url: "https://api.groq.com/openai/v1",
        }),
        "cerebras" => Some(ApiCompatibleProvider {
            id: "cerebras",
            name: "Cerebras",
            base_url: "https://api.cerebras.ai/v1",
        }),
        "openrouter" => Some(ApiCompatibleProvider {
            id: "openrouter",
            name: "OpenRouter",
            base_url: "https://openrouter.ai/api/v1",
        }),
        _ => None,
    }
}

mod context_compaction;
mod conversation;
pub use conversation::send_message;

fn chat_completion_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters.clone(),
        }
    })
}

pub(crate) fn opencode_session_id_for_conversation(
    conversation_id: &str,
    created_at: i64,
) -> String {
    use std::fmt::Write;

    let timestamp = if created_at > 0 {
        created_at as u64
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    };

    let current = (timestamp as u128 * 0x1000) | 1;
    let value = !current;
    let mut time_hex = String::with_capacity(12);
    for index in 0..6 {
        let shift = 40 - 8 * index;
        let byte = ((value >> shift) & 0xff) as u8;
        let _ = write!(&mut time_hex, "{:02x}", byte);
    }

    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(conversation_id.as_bytes());
    hasher.update(b":openchat_opencode_session");
    let hash = hasher.finalize();

    let mut random_str = String::with_capacity(14);
    for byte in &hash[..14] {
        random_str.push(CHARS[(byte % 62) as usize] as char);
    }

    format!("ses_{}{}", time_hex, random_str)
}

pub(crate) fn opencode_session_id() -> String {
    opencode_session_id_for_conversation("", 0)
}

fn client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| network_error())
}

pub(super) async fn opencode_free_tier_restricted(response: reqwest::Response) -> bool {
    let mut body = Vec::new();
    let mut chunks = response.bytes_stream();
    while body.len() < MAX_PROVIDER_ERROR_BYTES {
        let Some(Ok(chunk)) = chunks.next().await else {
            break;
        };
        let remaining = MAX_PROVIDER_ERROR_BYTES - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return false;
    };
    contains_free_tier_error(&value)
}

fn contains_free_tier_error(value: &Value) -> bool {
    match value {
        Value::String(message) => {
            message.contains("OpenCode's free tier can only be used from within OpenCode")
        }
        Value::Array(values) => values.iter().any(contains_free_tier_error),
        Value::Object(fields) => {
            fields.get("type").and_then(Value::as_str) == Some("FreeTierError")
                || fields.values().any(contains_free_tier_error)
        }
        _ => false,
    }
}

fn save_message(
    storage: &AppStorage,
    message: AssistantMessageWrite<'_>,
) -> Result<(), ServiceError> {
    crate::chatgpt_store::save_assistant_message(storage, message).map_err(|_| storage_error())
}

fn terminal_result(
    conversation_id: &str,
    message_id: &str,
    status: &str,
    elapsed: Duration,
    output_tokens: Option<i64>,
) -> Value {
    let tokens_per_second = output_tokens.and_then(|tokens| {
        let seconds = elapsed.as_secs_f64();
        (seconds > 0.0).then_some(tokens as f64 / seconds)
    });
    json!({
        "conversationId": conversation_id,
        "messageId": message_id,
        "status": status,
        "outputTokens": output_tokens,
        "tokensPerSecond": tokens_per_second,
        "elapsedMicroseconds": elapsed.as_micros(),
    })
}
fn http_error(status: StatusCode, provider_id: Option<&str>) -> ServiceError {
    let provider = provider_name(provider_id);
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ServiceError::new(
            "authentication_required",
            format!("{provider} rejected the request. Check the selected connection."),
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            format!("{provider} reported a usage limit. Try again later."),
            true,
        ),
        _ => ServiceError::new(
            "provider_request_failed",
            format!("{provider} could not complete the request."),
            status.is_server_error(),
        ),
    }
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "The model provider could not be reached. Check the network and try again.",
        true,
    )
}
fn invalid_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "The model provider returned a response OpenChat could not read.",
        true,
    )
}
fn storage_error() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "Chat history could not be updated.",
        false,
    )
}
fn route_error() -> ServiceError {
    ServiceError::new(
        "conversation_not_routed",
        "Choose a connected provider model before sending a message.",
        false,
    )
}
fn model_error() -> ServiceError {
    ServiceError::new(
        "model_unavailable",
        "The selected model is no longer available. Choose another model.",
        false,
    )
}
fn authentication_required_error() -> ServiceError {
    ServiceError::new(
        "authentication_required",
        "Add an OpenCode Console API key to use this paid model.",
        false,
    )
}
fn provider_authentication_required_error(provider_id: &str) -> ServiceError {
    let provider = provider_name(Some(provider_id));
    ServiceError::new(
        "authentication_required",
        format!("Add a {provider} API key in Settings to use this model."),
        false,
    )
}
fn provider_name(provider_id: Option<&str>) -> &'static str {
    match provider_id {
        Some("chatgpt_api") => "OpenAI",
        Some("gemini") => "Gemini",
        Some("groq") => "Groq",
        Some("cerebras") => "Cerebras",
        Some("openrouter") => "OpenRouter",
        _ => "OpenCode",
    }
}
fn openai_authentication_required_error() -> ServiceError {
    ServiceError::new(
        "authentication_required",
        "The selected OpenAI API key connection is unavailable. Check it in Settings.",
        false,
    )
}
fn invalid_retry_target_error() -> ServiceError {
    ServiceError::new(
        "invalid_retry_target",
        "The selected response can no longer be retried. Refresh the conversation and try again.",
        false,
    )
}
fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The provider request was cancelled.",
        true,
    )
}
fn protocol_error() -> ServiceError {
    ServiceError::new(
        "protocol_unavailable",
        "The local service could not deliver the response.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::stream::{
        SseLine, StreamedToolCall, append_tool_call_deltas, finish_reason, parse_sse_line,
        parse_streamed_tool_calls, pop_sse_line, stream_is_complete,
    };

    #[test]
    fn sse_lines_handle_fragmented_utf8_and_crlf() {
        let mut pending = b"data: {\"word\":\"".to_vec();
        assert!(pop_sse_line(&mut pending, false).is_none());

        pending.extend_from_slice("şeker\"}\r\n".as_bytes());
        let line = pop_sse_line(&mut pending, false).expect("complete SSE line");
        let SseLine::Data(value) = parse_sse_line(&line).expect("valid SSE JSON") else {
            panic!("expected a data event");
        };

        assert_eq!(value["word"], "şeker");
        assert!(pending.is_empty());
    }

    #[test]
    fn recognizes_opencode_free_tier_restriction_as_a_provider_policy_error() {
        let error = json!({
            "type": "error",
            "error": {
                "type": "FreeTierError",
                "message": "Error from provider (Console): OpenCode's free tier can only be used from within OpenCode"
            }
        });

        assert!(super::contains_free_tier_error(&error));
        assert!(!super::contains_free_tier_error(&json!({
            "error": {"type": "Unauthorized", "message": "Invalid API key"}
        })));
    }

    #[test]
    fn final_unterminated_tool_call_and_finish_reason_are_processed_at_eof() {
        let first = format!(
            "data: {}\n",
            json!({
                "choices": [{
                    "delta": {"tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "list_files", "arguments": "{\"path\":"}
                    }]},
                    "finish_reason": null
                }]
            })
        );
        let last = format!(
            "data: {}",
            json!({
                "choices": [{
                    "delta": {"tool_calls": [{
                        "index": 0,
                        "function": {"arguments": "\".\"}"}
                    }]},
                    "finish_reason": "tool_calls"
                }]
            })
        );

        let mut pending = Vec::new();
        let mut calls = BTreeMap::<usize, StreamedToolCall>::new();
        let mut saw_finish_reason = false;
        for (chunk, at_eof) in [(&first, false), (&last, true)] {
            pending.extend_from_slice(chunk.as_bytes());
            while let Some(line) = pop_sse_line(&mut pending, at_eof) {
                let SseLine::Data(value) = parse_sse_line(&line).expect("valid provider event")
                else {
                    continue;
                };
                saw_finish_reason |= finish_reason(&value).is_some();
                if let Some(deltas) = value
                    .pointer("/choices/0/delta/tool_calls")
                    .and_then(Value::as_array)
                {
                    append_tool_call_deltas(&mut calls, deltas).expect("append tool delta");
                }
            }
        }

        assert!(stream_is_complete(false, saw_finish_reason));
        let calls = parse_streamed_tool_calls(calls, false).expect("complete tool call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "list_files");
        assert_eq!(calls[0].arguments, json!({"path": "."}));
    }

    #[test]
    fn opencode_wire_tool_calls_are_translated_to_internal_names() {
        let mut calls = BTreeMap::new();
        calls.insert(
            0,
            StreamedToolCall {
                id: "call_1".to_owned(),
                name: "glob".to_owned(),
                arguments: "{\"path\":\".\"}".to_owned(),
            },
        );
        calls.insert(
            1,
            StreamedToolCall {
                id: "call_2".to_owned(),
                name: "read".to_owned(),
                arguments: "{\"filePath\":\"README.md\"}".to_owned(),
            },
        );
        calls.insert(
            2,
            StreamedToolCall {
                id: "call_3".to_owned(),
                name: "bash".to_owned(),
                arguments: "{\"command\":\"ls\"}".to_owned(),
            },
        );
        let parsed = parse_streamed_tool_calls(calls, true).expect("parsed opencode tools");
        assert_eq!(parsed[0].name, "list_files");
        assert_eq!(parsed[1].name, "read_file");
        assert_eq!(parsed[2].name, "execute_command");
    }

    #[test]
    fn opencode_deterministic_session_id() {
        let sid1 = super::opencode_session_id_for_conversation("conv_123", 1_700_000_000_000);
        let sid2 = super::opencode_session_id_for_conversation("conv_123", 1_700_000_000_000);
        let sid3 = super::opencode_session_id_for_conversation("conv_456", 1_700_000_000_000);

        assert_eq!(
            sid1, sid2,
            "Same conversation and timestamp must produce identical session IDs"
        );
        assert_ne!(
            sid1, sid3,
            "Different conversations must produce different session IDs"
        );
        assert!(
            sid1.starts_with("ses_"),
            "OpenCode session ID must start with ses_"
        );
        assert_eq!(
            sid1.len(),
            30,
            "OpenCode session ID must be 30 characters long"
        );
    }

    #[test]
    fn incomplete_or_malformed_streams_do_not_count_as_completed() {
        assert!(!stream_is_complete(false, false));
        assert!(stream_is_complete(true, false));
        assert!(stream_is_complete(false, true));

        let mut calls = BTreeMap::new();
        calls.insert(
            0,
            StreamedToolCall {
                id: "call_1".to_owned(),
                name: "list_files".to_owned(),
                arguments: "{\"path\":".to_owned(),
            },
        );
        assert_eq!(
            parse_streamed_tool_calls(calls, false)
                .expect_err("truncated tool arguments must be rejected")
                .code,
            "invalid_provider_response"
        );

        assert!(parse_sse_line(b"data: {\"choices\":[").is_err());
        assert!(parse_sse_line(b"data: \xff").is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn live_opencode_free_tier_smoke_test() {
        use reqwest::header::ACCEPT;

        let client = super::client().expect("client");
        let session_id = super::opencode_session_id();
        let body = json!({
            "model": "big-pickle",
            "messages": [
                {"role": "system", "content": "You are a concise assistant."},
                {"role": "user", "content": "Reply with 'pong' only."}
            ],
            "stream": true,
            "tools": crate::tools::opencode_wire_tools(&crate::tools::definitions()),
            "tool_choice": "none"
        });
        let response = client
            .post(super::CHAT_URL)
            .header("User-Agent", "opencode/1.18.30")
            .header("x-opencode-client", "cli")
            .header("x-opencode-session", session_id)
            .header("x-opencode-project", "global")
            .bearer_auth("public")
            .header(ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await
            .expect("send live request");

        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        assert!(
            status.is_success(),
            "Expected HTTP 200 OK from OpenCode, got: {} - body: {}",
            status,
            body_text
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_opencode_models_smoke_test() {
        let client = super::client().expect("client");
        let session_id = super::opencode_session_id();
        let response = client
            .get(super::models::MODELS_URL)
            .header("User-Agent", "opencode/1.18.30")
            .header("x-opencode-client", "cli")
            .header("x-opencode-session", session_id)
            .header("x-opencode-project", "global")
            .bearer_auth("public")
            .send()
            .await
            .expect("fetch models");

        let status = response.status();
        let text = response.text().await.expect("text");
        let json: serde_json::Value = serde_json::from_str(&text).expect("parse json");
        assert!(status.is_success());
        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .expect("data array");
        assert!(
            data.iter()
                .any(|m| m.get("id").and_then(|id| id.as_str()) == Some("big-pickle"))
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_opencode_models_full_test() {
        let storage = crate::storage::AppStorage::open().unwrap();
        let (_tx, mut rx) = tokio::sync::watch::channel(false);
        let res = super::models::models(&storage, None, true, &mut rx).await;
        println!("[FULL MODELS RESULT]: {:?}", res);
        assert!(res.is_ok());
    }

    #[test]
    fn responses_input_items_maps_system_user_assistant_and_tool_messages() {
        let messages = vec![
            json!({
                "role": "system",
                "content": "You are a helpful assistant."
            }),
            json!({
                "role": "user",
                "content": "Hello!"
            }),
            json!({
                "role": "assistant",
                "content": "Running tool.",
                "tool_calls": [
                    {
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"filePath\":\"README.md\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_123",
                "content": "file contents"
            }),
        ];

        let items = super::request::responses_input_items(&messages);
        assert_eq!(items.len(), 5);
        assert_eq!(items[0]["role"], "developer");
        assert_eq!(
            items[0]["content"][0]["text"],
            "You are a helpful assistant."
        );
        assert_eq!(items[1]["role"], "user");
        assert_eq!(items[1]["content"][0]["text"], "Hello!");
        assert_eq!(items[2]["role"], "assistant");
        assert_eq!(items[2]["content"][0]["text"], "Running tool.");
        assert_eq!(items[3]["type"], "function_call");
        assert_eq!(items[3]["call_id"], "call_123");
        assert_eq!(items[3]["name"], "read");
        assert_eq!(items[4]["type"], "function_call_output");
        assert_eq!(items[4]["call_id"], "call_123");
        assert_eq!(items[4]["output"], "file contents");
    }

    #[test]
    fn handle_responses_api_event_extracts_deltas_reasoning_and_tool_calls() {
        let mut calls = std::collections::BTreeMap::new();

        let event = json!({
            "type": "response.output_text.delta",
            "delta": "Hello"
        });
        match super::stream::handle_responses_api_event(&event, &mut calls).unwrap() {
            super::stream::ResponsesStreamEffect::TextDelta(t) => assert_eq!(t, "Hello"),
            _ => panic!("expected text delta"),
        }

        let event = json!({
            "type": "response.reasoning_text.delta",
            "delta": "Thinking..."
        });
        match super::stream::handle_responses_api_event(&event, &mut calls).unwrap() {
            super::stream::ResponsesStreamEffect::ReasoningDelta(r) => assert_eq!(r, "Thinking..."),
            _ => panic!("expected reasoning delta"),
        }

        let event = json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {
                "type": "function_call",
                "call_id": "call_abc",
                "name": "read"
            }
        });
        assert!(matches!(
            super::stream::handle_responses_api_event(&event, &mut calls).unwrap(),
            super::stream::ResponsesStreamEffect::None
        ));

        let event = json!({
            "type": "response.function_call_arguments.delta",
            "output_index": 0,
            "delta": "{\"filePath\":\"a.txt\"}"
        });
        assert!(matches!(
            super::stream::handle_responses_api_event(&event, &mut calls).unwrap(),
            super::stream::ResponsesStreamEffect::None
        ));

        let parsed = super::stream::parse_streamed_tool_calls(calls, true).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "call_abc");
        assert_eq!(parsed[0].name, "read_file");
        assert_eq!(parsed[0].arguments["filePath"], "a.txt");
    }

    #[tokio::test]
    #[ignore]
    async fn live_opencode_responses_api_muse_spark_smoke_test() {
        use reqwest::header::ACCEPT;

        let client = super::client().expect("client");
        let session_id = super::opencode_session_id();
        let defs = crate::tools::definitions();
        let req = crate::provider_schema::ProviderChatRequest {
            model: "muse-spark-1.3-contributor-free".to_owned(),
            instructions: "You are a concise assistant.".to_owned(),
            messages: vec![crate::provider_schema::ProviderMessage {
                role: crate::provider_schema::MessageRole::User,
                content: "Reply with 'pong' only.".to_owned(),
                images: Vec::new(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }],
            last_message_id: None,
            tools: defs,
            reasoning_effort: None,
        };
        let messages = super::request::completion_messages(&req, "opencode");
        let body = super::request::responses_api_body(&req, &messages);

        let response = client
            .post(super::RESPONSES_URL)
            .header("User-Agent", "opencode/1.18.30")
            .header("x-opencode-client", "cli")
            .header("x-opencode-session", session_id)
            .header("x-opencode-project", "global")
            .bearer_auth("public")
            .header(ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await
            .expect("send live responses request");

        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        assert!(
            status.is_success(),
            "Expected HTTP 200 OK from OpenCode Responses API, got: {} - body: {}",
            status,
            body_text
        );
        assert!(body_text.contains("response.created"));
    }
}
