use std::collections::BTreeMap;

use futures_util::StreamExt;
use reqwest::{StatusCode, header::ACCEPT};
use serde_json::Value;

use crate::{
    protocol::ServiceError,
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolCall},
};

use super::super::{
    MAX_EVENT_BYTES, cancelled_error, client, http_error, invalid_response_error, network_error,
    opencode_free_tier_restricted, protocol_error,
    stream::{
        ResponsesStreamEffect, SseLine, StreamedToolCall, append_tool_call_deltas, finish_reason,
        handle_responses_api_event, parse_sse_line, parse_streamed_tool_calls, pop_sse_line,
        stream_is_complete, update_chat_completion_usage,
    },
};
use super::ResponseStreamRequest;

pub(super) struct StreamedTurn {
    pub(super) round_content: String,
    pub(super) tool_calls: Vec<ToolCall>,
}

pub(super) async fn receive(
    request: ResponseStreamRequest<'_>,
) -> Result<StreamedTurn, ServiceError> {
    let mut provider_request = client()?.post(request.route.chat_url.as_str());
    if request.route.is_opencode {
        if let Some(session_id) = request.session_id {
            provider_request = provider_request
                .header("User-Agent", "opencode/1.18.30")
                .header("x-opencode-client", "cli")
                .header("x-opencode-session", session_id)
                .header("x-opencode-project", "global");
        }
        if request.route.is_free {
            provider_request = provider_request.bearer_auth("public");
        } else if let Some(api_key) = request.api_key {
            provider_request = provider_request.bearer_auth(api_key);
        }
    } else if !request.route.is_free
        && let Some(api_key) = request.api_key
    {
        provider_request = provider_request.bearer_auth(api_key);
    }
    let response = tokio::select! {
        changed = request.cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        response = provider_request
            .header(ACCEPT, "text/event-stream")
            .json(request.body)
            .send() => response.map_err(|_| connection_error(request.route))?,
    };
    if !response.status().is_success() {
        let status = response.status();
        if request.route.is_opencode
            && request.route.is_free
            && status == StatusCode::FORBIDDEN
            && opencode_free_tier_restricted(response).await
        {
            return Err(ServiceError::new(
                "opencode_free_tier_restricted",
                "OpenCode free models can only be used within OpenCode.",
                false,
            ));
        }
        if request.route.provider_id.as_deref() == Some("llama_cpp") {
            return Err(local_inference_error());
        }
        return Err(http_error(status, request.route.provider_id.as_deref()));
    }

    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut round_content = String::new();
    let mut tool_calls = BTreeMap::<usize, StreamedToolCall>::new();
    let mut saw_done = false;
    let mut saw_finish_reason = false;
    let mut stream_ended = false;

    while !stream_ended {
        let chunk = tokio::select! {
            changed = request.cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            chunk = stream.next() => chunk,
        };
        if let Some(chunk) = chunk {
            pending.extend_from_slice(&chunk.map_err(|_| connection_error(request.route))?);
            if pending.len() > MAX_EVENT_BYTES {
                return Err(invalid_response_error());
            }
        } else {
            stream_ended = true;
        }

        while let Some(line) = pop_sse_line(&mut pending, stream_ended) {
            let event = parse_sse_line(&line).map_err(|_| invalid_response_error())?;
            let value = match event {
                SseLine::Ignore => continue,
                SseLine::Done => {
                    saw_done = true;
                    continue;
                }
                SseLine::Data(value) => value,
            };

            if request.route.uses_responses_api {
                if let Some(tokens) = value
                    .pointer("/response/usage/input_tokens")
                    .and_then(Value::as_i64)
                    .filter(|tokens| *tokens >= 0)
                {
                    *request.input_tokens = Some(tokens);
                }
                match handle_responses_api_event(&value, &mut tool_calls)? {
                    ResponsesStreamEffect::TextDelta(text) => {
                        request.content.push_str(&text);
                        round_content.push_str(&text);
                        request
                            .events
                            .send(
                                &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                                    request.conversation_id,
                                    request.message_id,
                                    request.content,
                                    request.created_at,
                                ))
                                .into_rpc(request.request_id.clone()),
                            )
                            .await
                            .map_err(|_| protocol_error())?;
                    }
                    ResponsesStreamEffect::ReasoningDelta(reasoning) => {
                        request.reasoning_content.push_str(&reasoning);
                        let summary = ReasoningSummary {
                            id: format!("reasoning_{}", request.message_id),
                            content: request.reasoning_content.clone(),
                            elapsed_microseconds: request.started.elapsed().as_micros() as i64,
                            is_complete: false,
                        };
                        let _ = request
                            .events
                            .send(
                                &ChatStreamEvent::ReasoningSummariesUpdated {
                                    snapshot: ChatStreamSnapshot::new(
                                        request.conversation_id,
                                        request.message_id,
                                        request.content,
                                        request.created_at,
                                    ),
                                    summaries: vec![summary],
                                }
                                .into_rpc(request.request_id.clone()),
                            )
                            .await;
                    }
                    ResponsesStreamEffect::UsageTokens(tokens) => {
                        *request.output_tokens =
                            Some(request.output_tokens.unwrap_or(0).saturating_add(tokens));
                        saw_done = true;
                    }
                    ResponsesStreamEffect::Completed => {
                        saw_done = true;
                    }
                    ResponsesStreamEffect::Failed(msg) => {
                        return Err(ServiceError::new("model_response_failed", msg, false));
                    }
                    ResponsesStreamEffect::None => {}
                }
                continue;
            }

            saw_finish_reason |= finish_reason(&value).is_some();
            let is_mistral = request.route.provider_id.as_deref() == Some("mistral");
            let mistral_delta = is_mistral.then(|| mistral_content_delta(&value));
            let text = mistral_delta
                .as_ref()
                .and_then(|delta| (!delta.text.is_empty()).then_some(delta.text.as_str()))
                .or_else(|| {
                    (!is_mistral)
                        .then(|| value.pointer("/choices/0/delta/content"))
                        .flatten()
                        .and_then(Value::as_str)
                });
            if let Some(text) = text {
                request.content.push_str(text);
                round_content.push_str(text);
                request
                    .events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            request.conversation_id,
                            request.message_id,
                            request.content,
                            request.created_at,
                        ))
                        .into_rpc(request.request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
            }
            let reasoning = mistral_delta
                .as_ref()
                .and_then(|delta| (!delta.reasoning.is_empty()).then_some(delta.reasoning.as_str()))
                .or_else(|| {
                    value
                        .pointer("/choices/0/delta/reasoning_content")
                        .or_else(|| value.pointer("/choices/0/delta/reasoning"))
                        .and_then(Value::as_str)
                });
            if let Some(reasoning) = reasoning {
                request.reasoning_content.push_str(reasoning);
                let summary = ReasoningSummary {
                    id: format!("reasoning_{}", request.message_id),
                    content: request.reasoning_content.clone(),
                    elapsed_microseconds: request.started.elapsed().as_micros() as i64,
                    is_complete: false,
                };
                let _ = request
                    .events
                    .send(
                        &ChatStreamEvent::ReasoningSummariesUpdated {
                            snapshot: ChatStreamSnapshot::new(
                                request.conversation_id,
                                request.message_id,
                                request.content,
                                request.created_at,
                            ),
                            summaries: vec![summary],
                        }
                        .into_rpc(request.request_id.clone()),
                    )
                    .await;
            }
            update_chat_completion_usage(&value, request.input_tokens, request.output_tokens);
            if let Some(deltas) = value
                .pointer("/choices/0/delta/tool_calls")
                .and_then(Value::as_array)
            {
                append_tool_call_deltas(&mut tool_calls, deltas)?;
            }
        }
    }

    if !request.reasoning_content.is_empty() {
        let summary = ReasoningSummary {
            id: format!("reasoning_{}", request.message_id),
            content: request.reasoning_content.clone(),
            elapsed_microseconds: request.started.elapsed().as_micros() as i64,
            is_complete: true,
        };
        let _ = request
            .events
            .send(
                &ChatStreamEvent::ReasoningSummariesUpdated {
                    snapshot: ChatStreamSnapshot::new(
                        request.conversation_id,
                        request.message_id,
                        request.content,
                        request.created_at,
                    ),
                    summaries: vec![summary],
                }
                .into_rpc(request.request_id.clone()),
            )
            .await;
    }
    if !stream_is_complete(saw_done, saw_finish_reason) {
        return Err(invalid_response_error());
    }

    Ok(StreamedTurn {
        round_content,
        tool_calls: parse_streamed_tool_calls(tool_calls, request.route.is_opencode)?,
    })
}

fn connection_error(route: &super::super::route::ChatRoute) -> ServiceError {
    if route.provider_id.as_deref() == Some("llama_cpp") {
        ServiceError::new(
            "local_engine_runtime_unavailable",
            "The local inference server stopped responding. Restart the model and try again.",
            true,
        )
    } else {
        network_error()
    }
}

fn local_inference_error() -> ServiceError {
    ServiceError::new(
        "local_model_inference_failed",
        "The local model could not handle this request. Check its chat template and available memory.",
        false,
    )
}

struct MistralContentDelta {
    text: String,
    reasoning: String,
}

fn mistral_content_delta(value: &Value) -> MistralContentDelta {
    let mut delta = MistralContentDelta {
        text: String::new(),
        reasoning: String::new(),
    };
    let Some(content) = value.pointer("/choices/0/delta/content") else {
        return delta;
    };
    match content {
        Value::String(text) => delta.text.push_str(text),
        Value::Array(parts) => {
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("thinking") => {
                        append_mistral_thinking(part.get("thinking"), &mut delta.reasoning)
                    }
                    Some("text") => append_text_field(part, &mut delta.text),
                    _ => append_text_field(part, &mut delta.text),
                }
            }
        }
        _ => {}
    }
    delta
}

fn append_mistral_thinking(value: Option<&Value>, output: &mut String) {
    match value {
        Some(Value::String(text)) => output.push_str(text),
        Some(Value::Array(parts)) => {
            for part in parts {
                append_text_field(part, output);
            }
        }
        _ => {}
    }
}

fn append_text_field(value: &Value, output: &mut String) {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        output.push_str(text);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{connection_error, local_inference_error, mistral_content_delta};
    use crate::openai_compatible::route::ChatRoute;

    fn route(provider_id: &str) -> ChatRoute {
        ChatRoute {
            model_id: "test-model".to_owned(),
            provider_id: Some(provider_id.to_owned()),
            chat_url: String::new(),
            is_free: true,
            is_opencode: false,
            uses_responses_api: false,
            context_window: None,
            input_token_limit: None,
            supports_images: false,
            supports_tool_calls: None,
            connection_id: None,
        }
    }

    #[test]
    fn identifies_local_runtime_disconnects_separately_from_provider_network_errors() {
        assert_eq!(
            connection_error(&route("llama_cpp")).code,
            "local_engine_runtime_unavailable"
        );
        assert_eq!(
            connection_error(&route("opencode")).code,
            "network_unavailable"
        );
    }

    #[test]
    fn reports_local_server_request_rejections_as_model_inference_errors() {
        assert_eq!(local_inference_error().code, "local_model_inference_failed");
    }

    #[test]
    fn parses_mistral_thinking_and_text_content_parts() {
        let delta = mistral_content_delta(&json!({
            "choices": [{
                "delta": {
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": [{"type": "text", "text": "First step. "}]
                        },
                        {"type": "text", "text": "Answer."}
                    ]
                }
            }]
        }));

        assert_eq!(delta.reasoning, "First step. ");
        assert_eq!(delta.text, "Answer.");
    }

    #[test]
    fn parses_plain_mistral_text_deltas() {
        let delta = mistral_content_delta(&json!({
            "choices": [{"delta": {"content": "Answer."}}]
        }));

        assert!(delta.reasoning.is_empty());
        assert_eq!(delta.text, "Answer.");
    }
}
