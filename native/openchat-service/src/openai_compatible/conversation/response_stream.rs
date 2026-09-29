use std::collections::BTreeMap;

use futures_util::StreamExt;
use reqwest::header::ACCEPT;
use serde_json::Value;

use crate::{
    protocol::ServiceError,
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolCall},
};

use super::super::{
    MAX_EVENT_BYTES, cancelled_error, client, http_error, invalid_response_error, network_error,
    protocol_error,
    stream::{
        SseLine, StreamedToolCall, append_tool_call_deltas, finish_reason, parse_sse_line,
        parse_streamed_tool_calls, pop_sse_line, stream_is_complete,
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
            .send() => response.map_err(|_| network_error())?,
    };
    if !response.status().is_success() {
        return Err(http_error(
            response.status(),
            request.route.provider_id.as_deref(),
        ));
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
            pending.extend_from_slice(&chunk.map_err(|_| network_error())?);
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
            saw_finish_reason |= finish_reason(&value).is_some();
            if let Some(text) = value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
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
            if let Some(reasoning) = value
                .pointer("/choices/0/delta/reasoning_content")
                .or_else(|| value.pointer("/choices/0/delta/reasoning"))
                .and_then(Value::as_str)
            {
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
            if let Some(tokens) = value
                .pointer("/usage/completion_tokens")
                .and_then(Value::as_i64)
            {
                *request.output_tokens =
                    Some(request.output_tokens.unwrap_or(0).saturating_add(tokens));
            }
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
