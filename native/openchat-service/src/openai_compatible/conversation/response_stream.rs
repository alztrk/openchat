use std::collections::BTreeMap;

use futures_util::StreamExt;
use reqwest::{StatusCode, header::ACCEPT};
use serde_json::Value;

use crate::{
    protocol::ServiceError,
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolCall},
};

use super::super::{
    MAX_EVENT_BYTES, cancelled_error, chat_request_http_error, client, invalid_response_error,
    network_error, opencode_free_tier_restricted, protocol_error,
    stream::{
        ProviderRequestUsage, ResponsesStreamEffect, SseLine, StreamedToolCall,
        append_tool_call_deltas, finish_reason, handle_responses_api_event, parse_sse_line,
        parse_streamed_tool_calls, pop_sse_line, stream_is_complete, update_chat_completion_usage,
        update_provider_request_usage,
    },
};
use super::ResponseStreamRequest;

pub(super) struct StreamedTurn {
    pub(super) round_content: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) provider_request_usage: Option<ProviderRequestUsage>,
}

fn request_contains_tool_payload(body: &Value) -> bool {
    let has_tool_definitions = body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty());
    let has_chat_history = body
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|calls| !calls.is_empty())
                    || message.get("role").and_then(Value::as_str) == Some("tool")
            })
        });
    let has_responses_history = body
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("function_call" | "function_call_output")
                )
            })
        });

    has_tool_definitions || has_chat_history || has_responses_history
}

pub(super) async fn receive(
    request: ResponseStreamRequest<'_>,
) -> Result<StreamedTurn, ServiceError> {
    let client = client()?;
    receive_with_client(request, &client).await
}

async fn receive_with_client(
    request: ResponseStreamRequest<'_>,
    client: &reqwest::Client,
) -> Result<StreamedTurn, ServiceError> {
    let mut provider_request = client.post(request.route.chat_url.as_str());
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
    } else if let Some(api_key) = request.route.local_api_key.as_deref() {
        provider_request = provider_request.bearer_auth(api_key.as_str());
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
        if is_local_runtime(request.route.provider_id.as_deref()) {
            return Err(local_inference_error());
        }
        return Err(chat_request_http_error(
            status,
            request.route.provider_id.as_deref(),
            request_contains_tool_payload(request.body),
        ));
    }

    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut round_content = String::new();
    let mut tool_calls = BTreeMap::<usize, StreamedToolCall>::new();
    let supports_provider_citations = matches!(
        request.route.provider_id.as_deref(),
        Some("openai" | "openrouter" | "groq")
    );
    let mut provider_citation_sources = Vec::<Value>::new();
    let mut provider_citation_ids = BTreeMap::<(String, String), String>::new();
    let mistral_reference_sources = if request.route.provider_id.as_deref() == Some("mistral") {
        mistral_reference_sources(request.body)
    } else {
        Vec::new()
    };
    let mut saw_done = false;
    let mut saw_finish_reason = false;
    let mut stream_ended = false;
    let mut provider_request_usage: Option<ProviderRequestUsage> = None;

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
                update_provider_request_usage(
                    request.route.provider_id.as_deref().unwrap_or_default(),
                    &value,
                    &mut provider_request_usage,
                );
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

            if supports_provider_citations {
                collect_provider_citations(
                    &value,
                    &mut provider_citation_ids,
                    &mut provider_citation_sources,
                );
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
            let mut visible_delta = text.unwrap_or_default().to_owned();
            if let Some(delta) = mistral_delta.as_ref() {
                for reference_id in &delta.reference_ids {
                    let Some(source_id) = mistral_reference_sources.get(*reference_id) else {
                        continue;
                    };
                    visible_delta.push_str(" [");
                    visible_delta.push_str(source_id);
                    visible_delta.push(']');
                }
            }
            if !visible_delta.is_empty() {
                request.content.push_str(&visible_delta);
                round_content.push_str(&visible_delta);
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
            update_provider_request_usage(
                request.route.provider_id.as_deref().unwrap_or_default(),
                &value,
                &mut provider_request_usage,
            );
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

    if !provider_citation_sources.is_empty() {
        append_provider_citation_anchors(
            request.content,
            &mut round_content,
            &provider_citation_sources,
        );
        request
            .events
            .send(&crate::protocol::Response::event(
                request.request_id.clone(),
                "chat.citations.updated",
                serde_json::json!({
                    "conversationId": request.conversation_id,
                    "messageId": request.message_id,
                    "content": request.content,
                    "createdAtUnixMs": request.created_at,
                    "sources": provider_citation_sources,
                }),
            ))
            .await
            .map_err(|_| protocol_error())?;
    }

    Ok(StreamedTurn {
        round_content,
        tool_calls: parse_streamed_tool_calls(tool_calls, request.route.is_opencode)?,
        provider_request_usage,
    })
}

fn collect_provider_citations(
    value: &Value,
    ids: &mut BTreeMap<(String, String), String>,
    sources: &mut Vec<Value>,
) {
    let annotations = value
        .pointer("/choices/0/delta/annotations")
        .or_else(|| value.pointer("/choices/0/message/annotations"))
        .or_else(|| value.pointer("/choices/0/annotations"))
        .or_else(|| value.get("citations"));
    let Some(annotations) = annotations.and_then(Value::as_array) else {
        return;
    };
    for annotation in annotations {
        if sources.len() >= 32 {
            break;
        }
        if annotation
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "url_citation" && kind != "citation")
        {
            continue;
        }
        let citation = annotation.get("url_citation").unwrap_or(annotation);
        let Some(raw_url) = citation
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| url.len() <= 4096)
        else {
            continue;
        };
        let Ok(url) = url::Url::parse(raw_url) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            continue;
        }
        let Some(title) = citation
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty() && title.len() <= 512)
        else {
            continue;
        };
        let normalized_url = url.to_string();
        let key = (normalized_url.clone(), title.to_owned());
        if ids.contains_key(&key) {
            continue;
        }
        let id = format!("P{}", sources.len() + 1);
        ids.insert(key, id.clone());
        sources.push(serde_json::json!({
            "id": id,
            "title": title,
            "url": normalized_url,
            "sourceType": "provider_native"
        }));
    }
}

fn append_provider_citation_anchors(
    content: &mut String,
    round_content: &mut String,
    sources: &[Value],
) {
    for source in sources {
        let Some(id) = source.get("id").and_then(Value::as_str) else {
            continue;
        };
        let reference = format!(" [{id}]");
        content.push_str(&reference);
        round_content.push_str(&reference);
    }
}

fn connection_error(route: &super::super::route::ChatRoute) -> ServiceError {
    if is_local_runtime(route.provider_id.as_deref()) {
        ServiceError::new(
            "local_engine_runtime_unavailable",
            "The local inference server stopped responding. Restart the model and try again.",
            true,
        )
    } else {
        network_error()
    }
}

fn is_local_runtime(provider_id: Option<&str>) -> bool {
    matches!(provider_id, Some("llama_cpp" | "exllama" | "vllm"))
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
    reference_ids: Vec<usize>,
}

fn mistral_content_delta(value: &Value) -> MistralContentDelta {
    let mut delta = MistralContentDelta {
        text: String::new(),
        reasoning: String::new(),
        reference_ids: Vec::new(),
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
                    Some("reference") => {
                        if let Some(ids) = part.get("reference_ids").and_then(Value::as_array) {
                            delta.reference_ids.extend(ids.iter().filter_map(|id| {
                                id.as_u64()
                                    .and_then(|value| usize::try_from(value).ok())
                                    .or_else(|| {
                                        id.as_str().and_then(|value| value.parse::<usize>().ok())
                                    })
                            }));
                        }
                    }
                    _ => append_text_field(part, &mut delta.text),
                }
            }
        }
        _ => {}
    }
    delta
}

fn mistral_reference_sources(body: &Value) -> Vec<String> {
    let tool_sources = body
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
        .filter_map(|message| message.get("content").and_then(Value::as_str))
        .filter_map(|content| serde_json::from_str::<Value>(content).ok())
        .filter_map(|result| {
            let mut indexed_sources = result
                .as_object()
                .into_iter()
                .flat_map(|object| object.iter())
                .filter_map(|(key, value)| {
                    key.parse::<usize>()
                        .ok()
                        .map(|index| (index, value.get("sourceId").and_then(Value::as_str)))
                })
                .collect::<Vec<_>>();
            indexed_sources.sort_by_key(|(index, _)| *index);
            if !indexed_sources.is_empty() {
                return Some(
                    indexed_sources
                        .into_iter()
                        .map(|(_, source_id)| source_id.map(str::to_owned))
                        .collect::<Vec<_>>(),
                );
            }
            let mut sources = result
                .get("results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|source| {
                    source
                        .get("sourceId")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>();
            if let Some(source_id) = result.get("sourceId").and_then(Value::as_str) {
                sources.insert(0, Some(source_id.to_owned()));
            }
            (!sources.is_empty()).then_some(sources)
        })
        .take(2)
        .collect::<Vec<_>>();

    // Mistral reference IDs index one tool result's reference object. Avoid
    // attaching a citation when multiple independent tool results make that
    // index ambiguous, or when a result is missing a source ID.
    let [sources] = tool_sources.as_slice() else {
        return Vec::new();
    };
    if sources.len() > 64 || sources.iter().any(Option::is_none) {
        return Vec::new();
    }
    sources.iter().flatten().cloned().collect()
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
    use std::time::{Duration, Instant};

    use reqwest::Client;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::{oneshot, watch},
        task::JoinHandle,
    };

    use super::{
        ResponseStreamRequest, StreamedTurn, append_provider_citation_anchors,
        collect_provider_citations, connection_error, local_inference_error, mistral_content_delta,
        mistral_reference_sources, receive_with_client, request_contains_tool_payload,
    };
    use crate::{
        openai_compatible::route::ChatRoute,
        protocol::{EventSink, ServiceError},
    };

    struct FixtureOutcome {
        result: Result<StreamedTurn, ServiceError>,
        content: String,
        reasoning: String,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        request: Vec<u8>,
    }

    async fn run_fixture(
        status: u16,
        response_body: Vec<u8>,
        uses_responses_api: bool,
    ) -> FixtureOutcome {
        let (url, server) = spawn_fixture_server(status, response_body).await;
        let mut route = route("gemini");
        route.chat_url = url;
        route.is_free = false;
        route.uses_responses_api = uses_responses_api;
        let body = if uses_responses_api {
            json!({
                "model": "test-model",
                "input": [
                    {"type": "function_call", "call_id": "prior-call", "name": "read", "arguments": "{}"},
                    {"type": "function_call_output", "call_id": "prior-call", "output": "stored result"}
                ],
                "tools": [{"type": "function", "name": "search"}]
            })
        } else {
            json!({
                "model": "test-model",
                "messages": [
                    {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "prior-call",
                            "type": "function",
                            "function": {"name": "read", "arguments": "{}"}
                        }]
                    },
                    {"role": "tool", "tool_call_id": "prior-call", "content": "stored result"}
                ],
                "tools": [{"type": "function", "function": {"name": "search"}}]
            })
        };
        let client = fixture_client(Duration::from_secs(5));
        let (events, _event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let result = receive_for_test(
            &client,
            &route,
            &body,
            &mut cancellation,
            &events,
            &mut content,
            &mut reasoning,
            &mut output_tokens,
            &mut input_tokens,
        )
        .await;
        let request = server.await.expect("fixture server should finish");
        FixtureOutcome {
            result,
            content,
            reasoning,
            input_tokens,
            output_tokens,
            request,
        }
    }

    async fn receive_for_test(
        client: &Client,
        route: &ChatRoute,
        body: &serde_json::Value,
        cancellation: &mut watch::Receiver<bool>,
        events: &EventSink,
        content: &mut String,
        reasoning: &mut String,
        output_tokens: &mut Option<i64>,
        input_tokens: &mut Option<i64>,
    ) -> Result<StreamedTurn, ServiceError> {
        receive_with_client(
            ResponseStreamRequest {
                route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body,
                request_id: &json!("fixture-request"),
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content,
                reasoning_content: reasoning,
                output_tokens,
                input_tokens,
                started: Instant::now(),
                cancellation,
                events,
            },
            client,
        )
        .await
    }

    fn fixture_client(timeout: Duration) -> Client {
        Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .build()
            .expect("build local fixture client")
    }

    async fn spawn_fixture_server(
        status: u16,
        response_body: Vec<u8>,
    ) -> (String, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local HTTP fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let request = read_http_request(&mut stream).await;
            let headers = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            stream
                .write_all(headers.as_bytes())
                .await
                .expect("write fixture response headers");
            stream
                .write_all(&response_body)
                .await
                .expect("write fixture response body");
            request
        });
        (format!("http://{address}/v1/chat/completions"), server)
    }

    async fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        loop {
            let mut chunk = [0_u8; 4096];
            let count = stream.read(&mut chunk).await.expect("read fixture request");
            assert_ne!(count, 0, "fixture request ended before its body");
            request.extend_from_slice(&chunk[..count]);
            let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                return request;
            }
        }
    }

    fn sse_event(value: serde_json::Value, terminate_line: bool) -> Vec<u8> {
        let suffix = if terminate_line { "\n" } else { "" };
        format!("data: {value}{suffix}").into_bytes()
    }

    fn request_headers_and_body(request: &[u8]) -> (String, serde_json::Value) {
        let header_end = request
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .expect("fixture request should contain headers");
        let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
        let body = serde_json::from_slice(&request[header_end + 4..])
            .expect("fixture request body should be JSON");
        (headers, body)
    }

    fn expect_service_error<T>(result: Result<T, ServiceError>) -> ServiceError {
        match result {
            Err(error) => error,
            Ok(_) => panic!("expected provider operation to fail"),
        }
    }

    async fn spawn_pending_stream_server(
        initial_body: Vec<u8>,
        remaining_body: Vec<u8>,
    ) -> (
        String,
        oneshot::Receiver<()>,
        oneshot::Sender<()>,
        JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind pending stream fixture");
        let address = listener.local_addr().expect("read fixture address");
        let (started_sender, started_receiver) = oneshot::channel();
        let (continue_sender, continue_receiver) = oneshot::channel();
        let body_length = initial_body.len() + remaining_body.len();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let _request = read_http_request(&mut stream).await;
            let headers = format!(
                "HTTP/1.1 200 Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {body_length}\r\n\r\n"
            );
            stream
                .write_all(headers.as_bytes())
                .await
                .expect("write pending response headers");
            stream
                .write_all(&initial_body)
                .await
                .expect("write initial response event");
            let _ = started_sender.send(());
            let _ = continue_receiver.await;
            let _ = stream.write_all(&remaining_body).await;
        });
        (
            format!("http://{address}/v1/chat/completions"),
            started_receiver,
            continue_sender,
            server,
        )
    }

    #[test]
    fn recognizes_tools_in_definitions_and_both_history_wire_formats() {
        assert!(request_contains_tool_payload(&json!({
            "tools": [{"type": "function"}]
        })));
        assert!(request_contains_tool_payload(&json!({
            "messages": [{"role": "assistant", "tool_calls": [{"id": "call_1"}]}]
        })));
        assert!(request_contains_tool_payload(&json!({
            "messages": [{"role": "tool", "content": "done"}]
        })));
        assert!(request_contains_tool_payload(&json!({
            "input": [{"type": "function_call_output"}]
        })));
        assert!(!request_contains_tool_payload(&json!({
            "tools": [],
            "messages": [{"role": "user", "content": "hello"}],
            "input": [{"type": "message", "role": "user"}]
        })));
    }

    #[test]
    fn parses_provider_url_citations_and_appends_local_references() {
        let value = json!({
            "choices": [{
                "delta": {
                    "annotations": [
                        {
                            "type": "url_citation",
                            "url": "https://example.org/article",
                            "title": "Example article"
                        },
                        {
                            "type": "url_citation",
                            "url": "javascript:alert(1)",
                            "title": "Unsafe"
                        }
                    ]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        collect_provider_citations(&value, &mut ids, &mut sources);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["id"], "P1");
        assert_eq!(sources[0]["sourceType"], "provider_native");

        let mut content = "A sourced claim.".to_owned();
        let mut round_content = "A sourced claim.".to_owned();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources);
        assert_eq!(content, "A sourced claim. [P1]");
        assert_eq!(round_content, content);
    }

    #[test]
    fn parses_openai_nested_url_citations() {
        let value = json!({
            "choices": [{
                "message": {
                    "annotations": [{
                        "type": "url_citation",
                        "url_citation": {
                            "url": "https://example.org/article",
                            "title": "Example article",
                            "start_index": 4,
                            "end_index": 18
                        }
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        collect_provider_citations(&value, &mut ids, &mut sources);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["title"], "Example article");
        assert_eq!(sources[0]["url"], "https://example.org/article");
        assert_eq!(sources[0]["sourceType"], "provider_native");
    }

    fn route(provider_id: &str) -> ChatRoute {
        ChatRoute {
            model_id: "test-model".to_owned(),
            provider_model_id: "test-model".to_owned(),
            provider_id: Some(provider_id.to_owned()),
            chat_url: String::new(),
            is_free: true,
            is_opencode: false,
            uses_responses_api: false,
            context_window: None,
            input_token_limit: None,
            max_output_tokens: None,
            supports_images: false,
            supports_tool_calls: None,
            connection_id: None,
            local_api_key: None,
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
        assert_eq!(
            connection_error(&route("exllama")).code,
            "local_engine_runtime_unavailable"
        );
        assert_eq!(
            connection_error(&route("vllm")).code,
            "local_engine_runtime_unavailable"
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

    #[test]
    fn maps_mistral_reference_ids_to_one_tool_result_and_skips_ambiguous_results() {
        let body = json!({
            "messages": [{
                "role": "tool",
                "content": "{\"0\":{\"sourceId\":\"S1-call1234\"},\"1\":{\"sourceId\":\"S2-call1234\"}}"
            }]
        });
        assert_eq!(
            mistral_reference_sources(&body),
            ["S1-call1234", "S2-call1234"]
        );

        let ambiguous = json!({
            "messages": [
                {"role": "tool", "content": "{\"sourceId\":\"U1-call1234\"}"},
                {"role": "tool", "content": "{\"results\":[{\"sourceId\":\"S1-call5678\"}]}"}
            ]
        });
        assert!(mistral_reference_sources(&ambiguous).is_empty());
    }

    #[tokio::test]
    async fn chat_completions_fixture_flushes_final_event_and_orders_tool_calls() {
        let mut response_body = Vec::new();
        response_body.extend(sse_event(
            json!({"choices": [{"delta": {"content": "Searching."}}]}),
            true,
        ));
        response_body.extend(sse_event(
            json!({"choices": [{"delta": {"tool_calls": [{
                "index": 1,
                "id": "call_second",
                "function": {"name": "read", "arguments": "{\"path\":\"notes.md\"}"}
            }]}}]}),
            true,
        ));
        response_body.extend(sse_event(
            json!({"choices": [{"delta": {"tool_calls": [{
                "index": 0,
                "id": "call_first",
                "function": {"name": "search", "arguments": "{\"query\":\"rust\"}"}
            }]}}]}),
            true,
        ));
        response_body.extend(sse_event(
            json!({
                "choices": [{"finish_reason": "tool_calls", "delta": {}}],
                "usage": {"prompt_tokens": 11, "completion_tokens": 3}
            }),
            false,
        ));

        let outcome = run_fixture(200, response_body, false).await;
        let turn = outcome.result.expect("fixture stream should complete");
        assert_eq!(outcome.content, "Searching.");
        assert_eq!(turn.round_content, "Searching.");
        assert_eq!(outcome.input_tokens, Some(11));
        assert_eq!(outcome.output_tokens, Some(3));
        assert_eq!(turn.tool_calls.len(), 2);
        assert_eq!(turn.tool_calls[0].id, "call_first");
        assert_eq!(turn.tool_calls[0].name, "search");
        assert_eq!(turn.tool_calls[0].arguments["query"], "rust");
        assert_eq!(turn.tool_calls[1].id, "call_second");
        assert_eq!(turn.tool_calls[1].name, "read");
        assert_eq!(turn.tool_calls[1].arguments["path"], "notes.md");

        let (headers, body) = request_headers_and_body(&outcome.request);
        assert!(headers.starts_with("post /v1/chat/completions "));
        assert!(headers.contains("authorization: bearer fixture-api-key"));
        assert_eq!(body["messages"][1]["tool_call_id"], "prior-call");
        assert_eq!(body["messages"][1]["content"], "stored result");
        assert_eq!(body["tools"][0]["function"]["name"], "search");
        assert!(outcome.reasoning.is_empty());
    }

    #[tokio::test]
    async fn responses_fixture_reassembles_tool_arguments_and_unterminated_completion() {
        let mut response_body = Vec::new();
        response_body.extend(sse_event(
            json!({
                "type": "response.output_item.added",
                "output_index": 0,
                "item": {"type": "function_call", "call_id": "call_response", "name": "search"}
            }),
            true,
        ));
        response_body.extend(sse_event(
            json!({
                "type": "response.function_call_arguments.delta",
                "output_index": 0,
                "delta": "{\"query\":\"docs\"}"
            }),
            true,
        ));
        response_body.extend(sse_event(
            json!({"type": "response.output_text.delta", "delta": "Found it."}),
            true,
        ));
        response_body.extend(sse_event(
            json!({
                "type": "response.completed",
                "response": {
                    "output": [{
                        "type": "function_call",
                        "call_id": "call_response",
                        "name": "search",
                        "arguments": "{\"query\":\"docs\"}"
                    }],
                    "usage": {"input_tokens": 20, "output_tokens": 4}
                }
            }),
            false,
        ));

        let outcome = run_fixture(200, response_body, true).await;
        let turn = outcome.result.expect("Responses fixture should complete");
        assert_eq!(outcome.content, "Found it.");
        assert_eq!(outcome.input_tokens, Some(20));
        assert_eq!(outcome.output_tokens, Some(4));
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].id, "call_response");
        assert_eq!(turn.tool_calls[0].arguments["query"], "docs");

        let (headers, body) = request_headers_and_body(&outcome.request);
        assert!(headers.starts_with("post /v1/chat/completions "));
        assert!(body.get("messages").is_none());
        assert_eq!(body["input"][1]["type"], "function_call_output");
    }

    #[tokio::test]
    async fn malformed_and_incomplete_sse_streams_return_safe_provider_errors() {
        let malformed = run_fixture(200, b"data: {not-json}\n".to_vec(), false).await;
        let malformed_error = expect_service_error(malformed.result);
        assert_eq!(malformed_error.code, "invalid_provider_response");
        assert!(!malformed_error.message.contains("not-json"));

        let incomplete = run_fixture(
            200,
            sse_event(
                json!({"choices": [{"delta": {"content": "partial"}}]}),
                false,
            ),
            false,
        )
        .await;
        let incomplete_error = expect_service_error(incomplete.result);
        assert_eq!(incomplete_error.code, "invalid_provider_response");
        assert_eq!(incomplete.content, "partial");
    }

    #[tokio::test]
    async fn local_http_status_fixtures_map_auth_quota_and_server_failures() {
        for (status, expected_code) in [
            (401, "authentication_required"),
            (429, "rate_limited"),
            (503, "provider_request_failed"),
        ] {
            let outcome =
                run_fixture(status, b"provider-secret-response-body".to_vec(), false).await;
            let error = expect_service_error(outcome.result);
            assert_eq!(error.code, expected_code);
            assert!(!error.message.contains("provider-secret-response-body"));
        }
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_open_local_provider_stream() {
        let initial = sse_event(
            json!({"choices": [{"delta": {"content": "partial"}}]}),
            true,
        );
        let remaining = sse_event(
            json!({"choices": [{"finish_reason": "stop", "delta": {}}]}),
            true,
        );
        let (url, started, continue_server, server) =
            spawn_pending_stream_server(initial, remaining).await;
        let mut route = route("gemini");
        route.chat_url = url;
        route.is_free = false;
        let client = fixture_client(Duration::from_secs(5));
        let (events, _event_receiver) = EventSink::test_channel();
        let (cancel_sender, mut cancellation) = watch::channel(false);
        let body = json!({"model": "test-model", "messages": []});
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let mut receive = Box::pin(receive_for_test(
            &client,
            &route,
            &body,
            &mut cancellation,
            &events,
            &mut content,
            &mut reasoning,
            &mut output_tokens,
            &mut input_tokens,
        ));
        tokio::select! {
            started = started => {
                started.expect("fixture should keep the stream open");
                cancel_sender.send(true).expect("cancellation receiver is active");
            }
            result = &mut receive => {
                let _ = result;
                panic!("stream ended before cancellation");
            },
        }
        let error = expect_service_error(receive.await);
        assert_eq!(error.code, "request_cancelled");
        continue_server
            .send(())
            .expect("release fixture server after cancellation");
        server.await.expect("fixture server should close");
    }

    #[tokio::test]
    async fn stalled_local_response_uses_the_request_timeout_error_path() {
        let body = sse_event(
            json!({"choices": [{"finish_reason": "stop", "delta": {}}]}),
            true,
        );
        let (url, started, continue_server, server) =
            spawn_pending_stream_server(Vec::new(), body).await;
        let mut route = route("gemini");
        route.chat_url = url;
        route.is_free = false;
        let client = fixture_client(Duration::from_millis(100));
        let (events, _event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let body = json!({"model": "test-model", "messages": []});
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let mut receive = Box::pin(receive_for_test(
            &client,
            &route,
            &body,
            &mut cancellation,
            &events,
            &mut content,
            &mut reasoning,
            &mut output_tokens,
            &mut input_tokens,
        ));
        tokio::select! {
            started = tokio::time::timeout(Duration::from_secs(2), started) => {
                started.expect("fixture response should start")
                    .expect("fixture should signal its open response");
            }
            result = &mut receive => {
                let _ = result;
                panic!("request finished before the stalled response timeout");
            }
        }
        let error = expect_service_error(
            tokio::time::timeout(Duration::from_secs(2), receive)
                .await
                .expect("request timeout should be bounded"),
        );
        assert_eq!(error.code, "network_unavailable");
        continue_server
            .send(())
            .expect("release fixture server after timeout");
        server.await.expect("fixture server should close");
    }
}
