//! Receives, bounds, and interprets a provider's streamed assistant response.
//!
//! This layer validates server-sent events and returns normalized text, usage,
//! tool-call, and completion state to conversation orchestration.

use std::collections::BTreeMap;
use std::pin::Pin;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::{StatusCode, header::ACCEPT};
use serde_json::Value;

use crate::{
    domain::http_headers::sensitive_api_key_header,
    protocol::ServiceError,
    provider_schema::{
        ChatStreamDelta, ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolCall,
    },
};

use super::super::{
    MAX_EVENT_BYTES, MAX_PROVIDER_ERROR_BYTES, apply_opencode_headers, cancelled_error,
    chat_request_http_error_with_retry_after, client, copilot, invalid_response_error,
    network_error, opencode_free_headers, opencode_free_transport, opencode_free_usage_limit_error,
    protocol_error,
    stream::{
        ProviderRequestUsage, ResponsesStreamEffect, SseLine, StreamedToolCall,
        append_tool_call_deltas, finish_reason, handle_responses_api_event, parse_sse_line,
        parse_streamed_tool_calls, pop_sse_line, stream_is_complete, update_chat_completion_usage,
        update_provider_request_usage,
    },
};
use super::{ResponseStreamRequest, think_tag_filter::ThinkTagFilter};

/// Response body stream where failures are reduced to a diagnostic string so
/// both the `reqwest` and OpenSSL transports can share one code path.
type ProviderBody = Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>;

pub(super) struct StreamedTurn {
    pub(super) round_content: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) provider_request_usage: Option<ProviderRequestUsage>,
}

async fn append_visible_text(
    request: &mut ResponseStreamRequest<'_>,
    round_content: &mut String,
    text: &str,
) -> Result<(), ServiceError> {
    if text.is_empty() {
        return Ok(());
    }
    request.content.push_str(text);
    round_content.push_str(text);
    request
        .events
        .send(
            &ChatStreamEvent::TextAppended(ChatStreamDelta::new(
                request.conversation_id,
                request.message_id,
                text,
                request.created_at,
            ))
            .into_rpc(request.request_id.clone()),
        )
        .await
        .map_err(|_| protocol_error())
}

async fn append_think_output(
    request: &mut ResponseStreamRequest<'_>,
    round_content: &mut String,
    output: super::think_tag_filter::ThinkTagOutput,
) -> Result<(), ServiceError> {
    append_visible_text(request, round_content, &output.visible).await?;
    if !output.reasoning.is_empty() {
        append_legacy_reasoning(request, &output.reasoning).await?;
    }
    Ok(())
}

async fn send_reasoning_summaries(
    request: &ResponseStreamRequest<'_>,
    summaries: Vec<ReasoningSummary>,
) -> Result<(), ServiceError> {
    request
        .events
        .send(
            &ChatStreamEvent::ReasoningSummariesUpdated {
                snapshot: ChatStreamSnapshot::new(
                    request.conversation_id,
                    request.message_id,
                    request.content,
                    request.created_at,
                ),
                summaries,
            }
            .into_rpc(request.request_id.clone()),
        )
        .await
        .map_err(|_| protocol_error())
}

async fn update_reasoning_summary(
    request: &mut ResponseStreamRequest<'_>,
    item_id: &str,
    summary_index: u64,
    text: Option<&str>,
    append_text: bool,
    is_complete: bool,
) -> Result<(), ServiceError> {
    let summary_id = format!(
        "reasoning_{}_{}:{}",
        request.message_id, item_id, summary_index
    );
    let elapsed_microseconds =
        i64::try_from(request.started.elapsed().as_micros()).unwrap_or(i64::MAX);
    let summaries = {
        let summaries = &mut *request.reasoning_summaries;
        let position = match summaries
            .iter()
            .position(|summary| summary.id == summary_id)
        {
            Some(position) => position,
            None => {
                summaries.push(ReasoningSummary {
                    id: summary_id,
                    content: String::new(),
                    elapsed_microseconds,
                    is_complete: false,
                });
                summaries.len().saturating_sub(1)
            }
        };
        let Some(summary) = summaries.get_mut(position) else {
            return Err(invalid_response_error());
        };
        if let Some(text) = text {
            if append_text {
                summary.content.push_str(text);
            } else {
                summary.content = text.to_owned();
            }
        }
        summary.elapsed_microseconds = elapsed_microseconds;
        summary.is_complete |= is_complete;
        summaries.clone()
    };
    send_reasoning_summaries(request, summaries).await
}

async fn finish_reasoning_summaries(
    request: &mut ResponseStreamRequest<'_>,
) -> Result<(), ServiceError> {
    if request.route.provider_id.as_deref() != Some("chatgpt_api") {
        return Ok(());
    }
    let elapsed_microseconds =
        i64::try_from(request.started.elapsed().as_micros()).unwrap_or(i64::MAX);
    let summaries = {
        let summaries = &mut *request.reasoning_summaries;
        let mut changed = false;
        for summary in summaries.iter_mut().filter(|summary| !summary.is_complete) {
            summary.is_complete = true;
            summary.elapsed_microseconds = elapsed_microseconds;
            changed = true;
        }
        changed.then(|| summaries.clone())
    };
    if let Some(summaries) = summaries {
        send_reasoning_summaries(request, summaries).await?;
    }
    Ok(())
}

async fn append_legacy_reasoning(
    request: &mut ResponseStreamRequest<'_>,
    reasoning: &str,
) -> Result<(), ServiceError> {
    request.reasoning_content.push_str(reasoning);
    let summary = ReasoningSummary {
        id: format!("reasoning_{}", request.message_id),
        content: request.reasoning_content.clone(),
        elapsed_microseconds: i64::try_from(request.started.elapsed().as_micros())
            .unwrap_or(i64::MAX),
        is_complete: false,
    };
    send_reasoning_summaries(request, vec![summary]).await
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
    let client = if request.route.is_copilot {
        copilot::client()?
    } else {
        client()?
    };
    receive_with_client(request, &client).await
}

struct ProviderResponse {
    status: StatusCode,
    retry_after: Option<u64>,
    body: ProviderBody,
}

impl ProviderResponse {
    fn from_reqwest(response: reqwest::Response) -> Self {
        let status = response.status();
        let retry_after = retry_after_seconds(response.headers());
        let body = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|error| error.without_url().to_string()));
        Self {
            status,
            retry_after,
            body: Box::pin(body),
        }
    }

    fn from_free(response: opencode_free_transport::FreeResponse) -> Self {
        Self {
            status: response.status,
            retry_after: response.retry_after,
            body: response.body,
        }
    }
}

fn retry_after_seconds(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (1..=86_400).contains(seconds))
}

async fn read_error_body(body: &mut ProviderBody) -> Option<Value> {
    let mut collected = Vec::new();
    while collected.len() < MAX_PROVIDER_ERROR_BYTES {
        let Some(Ok(chunk)) = body.next().await else {
            break;
        };
        let remaining = MAX_PROVIDER_ERROR_BYTES - collected.len();
        collected.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    serde_json::from_slice::<Value>(&collected).ok()
}

async fn receive_with_client(
    mut request: ResponseStreamRequest<'_>,
    client: &reqwest::Client,
) -> Result<StreamedTurn, ServiceError> {
    let response = if request.route.is_opencode && request.route.is_free {
        // The free tier requires a TLS profile only OpenSSL can produce.
        let headers =
            opencode_free_headers(request.session_id, request.route.project_id.as_deref());
        let body = serde_json::to_vec(request.body).map_err(|_| invalid_response_error())?;
        tokio::select! {
            changed = request.cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            response = opencode_free_transport::send(
                request.route.chat_url.as_str(),
                headers,
                body,
            ) => ProviderResponse::from_free(response?),
        }
    } else {
        let mut provider_request = client.post(request.route.chat_url.as_str());
        if request.route.is_copilot {
            let Some(api_key) = request.api_key else {
                return Err(super::super::provider_authentication_required_error(
                    "copilot",
                ));
            };
            provider_request = copilot::authorize_inference(
                provider_request,
                api_key,
                copilot::RequestIntent::Conversation,
            );
        } else if request.route.is_opencode {
            if let Some(session_id) = request.session_id {
                provider_request = apply_opencode_headers(
                    provider_request,
                    session_id,
                    request.route.project_id.as_deref(),
                );
            }
            if request.route.is_free {
                provider_request = provider_request.bearer_auth("public");
            } else if let Some(api_key) = request.api_key {
                provider_request = provider_request.bearer_auth(api_key);
            }
        } else if request.route.is_anthropic()
            && let Some(api_key) = request.api_key
        {
            provider_request = provider_request
                .header(
                    "x-api-key",
                    sensitive_api_key_header(api_key).map_err(|_| {
                        ServiceError::new(
                            "invalid_provider_credential",
                            "The configured provider API key is invalid.",
                            false,
                        )
                    })?,
                )
                .header("anthropic-version", "2023-06-01");
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
                .send() => response.map_err(|error| {
                    connection_error(request.route, error.without_url().to_string())
                })?,
        };
        ProviderResponse::from_reqwest(response)
    };
    let ProviderResponse {
        status,
        retry_after: retry_after_value,
        body: mut stream,
    } = response;
    let retry_after_seconds = retry_after_value;
    if !status.is_success() {
        if request.route.is_opencode
            && request.route.is_free
            && matches!(
                status,
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            )
            && let Some(error_body) = read_error_body(&mut stream).await
        {
            if status == StatusCode::FORBIDDEN
                && super::super::contains_free_tier_error(&error_body)
            {
                return Err(super::super::opencode_free_tier_restricted_error());
            }
            if status == StatusCode::TOO_MANY_REQUESTS
                && super::super::contains_free_usage_limit_error(&error_body)
            {
                return Err(opencode_free_usage_limit_error(retry_after_seconds));
            }
        }
        if is_local_runtime(request.route.provider_id.as_deref()) {
            return Err(
                local_inference_error().with_details(format!("HTTP status {}", status.as_u16()))
            );
        }
        return Err(chat_request_http_error_with_retry_after(
            status,
            request.route.provider_id.as_deref(),
            request_contains_tool_payload(request.body),
            retry_after_seconds,
        ));
    }
    let mut pending = Vec::new();
    let mut round_content = String::new();
    let mut tool_calls = BTreeMap::<usize, StreamedToolCall>::new();
    let mut think_filter = ThinkTagFilter::new();
    let supports_provider_citations = matches!(
        request.route.provider_id.as_deref(),
        Some(
            "openai" | "chatgpt_api" | "opencode" | "openrouter" | "groq" | "gemini" | "perplexity"
        )
    );
    let mut provider_citation_sources = Vec::<Value>::new();
    let mut provider_citation_ids = BTreeMap::<(String, String), String>::new();
    let mut provider_citation_positions = BTreeMap::<String, usize>::new();
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
            let chunk = chunk.map_err(|details| connection_error(request.route, details))?;
            request.response_budget.consume(chunk.len())?;
            pending.extend_from_slice(&chunk);
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

            if request.route.is_anthropic() {
                match value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                {
                    "message_start" => {
                        let usage = value.pointer("/message/usage");
                        if let Some(tokens) = value
                            .pointer("/message/usage/input_tokens")
                            .and_then(Value::as_i64)
                            .filter(|tokens| *tokens >= 0)
                        {
                            *request.input_tokens = Some(tokens);
                            provider_request_usage
                                .get_or_insert_with(ProviderRequestUsage::default)
                                .prompt_tokens = Some(tokens);
                        }
                        if let Some(usage) = usage {
                            let request_usage = provider_request_usage
                                .get_or_insert_with(ProviderRequestUsage::default);
                            if let Some(tokens) = usage
                                .get("cache_read_input_tokens")
                                .and_then(Value::as_i64)
                                .filter(|tokens| *tokens >= 0)
                            {
                                request_usage.cached_tokens = Some(tokens);
                            }
                            if let Some(tokens) = usage
                                .get("cache_creation_input_tokens")
                                .and_then(Value::as_i64)
                                .filter(|tokens| *tokens >= 0)
                            {
                                request_usage.cache_write_tokens = Some(tokens);
                            }
                        }
                    }
                    "content_block_start" => {
                        let block = value.get("content_block");
                        if block
                            .and_then(|block| block.get("type"))
                            .and_then(Value::as_str)
                            == Some("tool_use")
                        {
                            let index = value
                                .get("index")
                                .and_then(Value::as_u64)
                                .and_then(|index| usize::try_from(index).ok())
                                .ok_or_else(invalid_response_error)?;
                            if tool_calls.len() >= crate::tools::MAX_TOOL_CALLS_PER_RESPONSE
                                && !tool_calls.contains_key(&index)
                            {
                                return Err(crate::tools::tool_response_limit_error());
                            }
                            let call = tool_calls.entry(index).or_default();
                            call.id = block
                                .and_then(|block| block.get("id"))
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned();
                            call.name = block
                                .and_then(|block| block.get("name"))
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned();
                        }
                    }
                    "content_block_delta" => {
                        let delta = value.get("delta");
                        match delta
                            .and_then(|delta| delta.get("type"))
                            .and_then(Value::as_str)
                        {
                            Some("text_delta") => {
                                if let Some(text) = delta
                                    .and_then(|delta| delta.get("text"))
                                    .and_then(Value::as_str)
                                {
                                    let output = think_filter.push(text);
                                    append_think_output(&mut request, &mut round_content, output)
                                        .await?;
                                }
                            }
                            Some("thinking_delta") => {
                                if let Some(reasoning) = delta
                                    .and_then(|delta| delta.get("thinking"))
                                    .and_then(Value::as_str)
                                {
                                    append_legacy_reasoning(&mut request, reasoning).await?;
                                }
                            }
                            Some("input_json_delta") => {
                                let index = value
                                    .get("index")
                                    .and_then(Value::as_u64)
                                    .and_then(|index| usize::try_from(index).ok())
                                    .ok_or_else(invalid_response_error)?;
                                let call = tool_calls
                                    .get_mut(&index)
                                    .ok_or_else(invalid_response_error)?;
                                if let Some(partial_json) = delta
                                    .and_then(|delta| delta.get("partial_json"))
                                    .and_then(Value::as_str)
                                {
                                    if call.arguments.len().saturating_add(partial_json.len())
                                        > crate::tools::MAX_TOOL_ARGUMENT_BYTES
                                    {
                                        return Err(invalid_response_error());
                                    }
                                    call.arguments.push_str(partial_json);
                                }
                            }
                            _ => {}
                        }
                    }
                    "message_delta" => {
                        if let Some(tokens) = value
                            .pointer("/usage/output_tokens")
                            .and_then(Value::as_i64)
                            .filter(|tokens| *tokens >= 0)
                        {
                            *request.output_tokens = Some(tokens);
                            provider_request_usage
                                .get_or_insert_with(ProviderRequestUsage::default)
                                .completion_tokens = Some(tokens);
                        }
                        if let Some(tokens) = value
                            .pointer("/usage/output_tokens_details/thinking_tokens")
                            .and_then(Value::as_i64)
                            .filter(|tokens| *tokens >= 0)
                        {
                            provider_request_usage
                                .get_or_insert_with(ProviderRequestUsage::default)
                                .reasoning_tokens = Some(tokens);
                        }
                        saw_finish_reason |= value
                            .pointer("/delta/stop_reason")
                            .and_then(Value::as_str)
                            .is_some();
                    }
                    "message_stop" => saw_done = true,
                    "error" => {
                        return Err(ServiceError::new(
                            "model_response_failed",
                            "The provider returned a streaming error.",
                            false,
                        ));
                    }
                    _ => {}
                }
                continue;
            }

            if request.route.uses_responses_api {
                if supports_provider_citations {
                    collect_provider_citations(
                        &value,
                        &mut provider_citation_ids,
                        &mut provider_citation_sources,
                        &mut provider_citation_positions,
                    );
                }
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
                        let output = think_filter.push(&text);
                        append_think_output(&mut request, &mut round_content, output).await?;
                    }
                    ResponsesStreamEffect::ReasoningDelta(reasoning) => {
                        if request.route.provider_id.as_deref() != Some("chatgpt_api") {
                            append_legacy_reasoning(&mut request, &reasoning).await?;
                        }
                    }
                    ResponsesStreamEffect::ReasoningSummaryDelta {
                        item_id,
                        summary_index,
                        delta,
                    } => {
                        if request.route.provider_id.as_deref() == Some("chatgpt_api") {
                            update_reasoning_summary(
                                &mut request,
                                &item_id,
                                summary_index,
                                Some(&delta),
                                true,
                                false,
                            )
                            .await?;
                        } else {
                            append_legacy_reasoning(&mut request, &delta).await?;
                        }
                    }
                    ResponsesStreamEffect::ReasoningSummaryDone {
                        item_id,
                        summary_index,
                        text,
                    } => {
                        if request.route.provider_id.as_deref() == Some("chatgpt_api") {
                            update_reasoning_summary(
                                &mut request,
                                &item_id,
                                summary_index,
                                text.as_deref(),
                                false,
                                true,
                            )
                            .await?;
                        }
                    }
                    ResponsesStreamEffect::UsageTokens(tokens) => {
                        *request.output_tokens =
                            Some(request.output_tokens.unwrap_or(0).saturating_add(tokens));
                        saw_done = true;
                        finish_reasoning_summaries(&mut request).await?;
                    }
                    ResponsesStreamEffect::Completed => {
                        saw_done = true;
                        finish_reasoning_summaries(&mut request).await?;
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
                    &mut provider_citation_positions,
                );
            }

            saw_finish_reason |= finish_reason(&value).is_some();
            let is_mistral = request.route.provider_id.as_deref() == Some("mistral");
            let mistral_delta = is_mistral.then(|| mistral_content_delta(&value));
            let gemini_delta = gemini_content_delta(&value);
            let text = mistral_delta
                .as_ref()
                .and_then(|delta| (!delta.text.is_empty()).then_some(delta.text.as_str()))
                .or_else(|| {
                    gemini_delta
                        .as_ref()
                        .and_then(|(_, text)| (!text.is_empty()).then_some(text.as_str()))
                })
                .or_else(|| {
                    (!is_mistral)
                        .then(|| value.pointer("/choices/0/delta/content"))
                        .flatten()
                        .and_then(Value::as_str)
                });
            let output = think_filter.push(text.unwrap_or_default());
            let mut visible_delta = output.visible;
            if !output.reasoning.is_empty() {
                append_legacy_reasoning(&mut request, &output.reasoning).await?;
            }
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
            append_visible_text(&mut request, &mut round_content, &visible_delta).await?;
            let reasoning = mistral_delta
                .as_ref()
                .and_then(|delta| (!delta.reasoning.is_empty()).then_some(delta.reasoning.as_str()))
                .or_else(|| {
                    gemini_delta.as_ref().and_then(|(reasoning, _)| {
                        (!reasoning.is_empty()).then_some(reasoning.as_str())
                    })
                })
                .or_else(|| {
                    value
                        .pointer("/choices/0/delta/reasoning_content")
                        .or_else(|| value.pointer("/choices/0/delta/reasoning"))
                        .or_else(|| value.pointer("/choices/0/delta/thinking"))
                        .and_then(Value::as_str)
                });
            if let Some(reasoning) = reasoning {
                append_legacy_reasoning(&mut request, reasoning).await?;
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

    let trailing_output = think_filter.finish();
    append_think_output(&mut request, &mut round_content, trailing_output).await?;

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
            &provider_citation_positions,
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
    positions: &mut BTreeMap<String, usize>,
) {
    let mut annotation_groups = Vec::new();
    if let Some(annotations) = value
        .pointer("/choices/0/delta/annotations")
        .or_else(|| value.pointer("/choices/0/message/annotations"))
        .or_else(|| value.pointer("/choices/0/annotations"))
        .or_else(|| value.get("citations"))
        && let Some(annotations) = annotations.as_array()
    {
        annotation_groups.push(annotations);
    }
    if let Some(output) = value.pointer("/response/output").and_then(Value::as_array) {
        for item in output {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if let Some(annotations) = part.get("annotations").and_then(Value::as_array) {
                        annotation_groups.push(annotations);
                    }
                }
            }
        }
    }
    for annotation in annotation_groups.into_iter().flatten() {
        if annotation
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "url_citation" && kind != "citation")
        {
            continue;
        }
        let citation = annotation.get("url_citation").unwrap_or(annotation);
        let position = citation
            .get("end_index")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .map(|end_index| end_index.saturating_add(1));
        store_provider_citation(citation, ids, sources, positions, position);
    }
    collect_gemini_grounding_citations(value, ids, sources, positions);
}

fn collect_gemini_grounding_citations(
    value: &Value,
    ids: &mut BTreeMap<(String, String), String>,
    sources: &mut Vec<Value>,
    positions: &mut BTreeMap<String, usize>,
) {
    let Some(candidate) = value
        .pointer("/candidates/0")
        .or_else(|| value.get("candidate"))
    else {
        return;
    };
    let Some(chunks) = candidate
        .pointer("/groundingMetadata/groundingChunks")
        .and_then(Value::as_array)
    else {
        return;
    };
    let supports = candidate
        .pointer("/groundingMetadata/groundingSupports")
        .and_then(Value::as_array);
    let text_parts = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array);

    let mut positions_by_chunk = BTreeMap::<usize, usize>::new();
    for support in supports.into_iter().flatten().take(128) {
        let Some(segment) = support.get("segment") else {
            continue;
        };
        let Some(end_byte) = segment
            .get("endIndex")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
        else {
            continue;
        };
        let part_index = segment
            .get("partIndex")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or(0);
        let Some(parts) = text_parts else {
            continue;
        };
        let Some(current_part) = parts
            .get(part_index)
            .and_then(|part| part.get("text"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if end_byte > current_part.len() || !current_part.is_char_boundary(end_byte) {
            continue;
        }
        let prefix = parts
            .iter()
            .take(part_index)
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<String>();
        let position = format!("{prefix}{}", &current_part[..end_byte])
            .chars()
            .count();
        for chunk_index in support
            .get("groundingChunkIndices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(64)
            .filter_map(Value::as_u64)
            .filter_map(|index| usize::try_from(index).ok())
        {
            positions_by_chunk
                .entry(chunk_index)
                .and_modify(|previous| *previous = (*previous).max(position))
                .or_insert(position);
        }
    }

    for (chunk_index, chunk) in chunks.iter().take(64).enumerate() {
        if sources.len() >= 32 {
            break;
        }
        let Some(web) = chunk.get("web") else {
            continue;
        };
        let Some(raw_url) = web.get("uri").and_then(Value::as_str) else {
            continue;
        };
        let Some(title) = web.get("title").and_then(Value::as_str) else {
            continue;
        };
        let position = positions_by_chunk.get(&chunk_index).copied();
        let citation = serde_json::json!({"url": raw_url, "title": title});
        store_provider_citation(&citation, ids, sources, positions, position);
    }
}

fn store_provider_citation(
    citation: &Value,
    ids: &mut BTreeMap<(String, String), String>,
    sources: &mut Vec<Value>,
    positions: &mut BTreeMap<String, usize>,
    position: Option<usize>,
) {
    if sources.len() >= 32 {
        return;
    }
    let Some(raw_url) = citation
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| url.len() <= 4096)
    else {
        return;
    };
    let Ok(url) = url::Url::parse(raw_url) else {
        return;
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return;
    }
    let Some(title) = citation
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty() && title.len() <= 512)
    else {
        return;
    };
    let normalized_url = url.to_string();
    let key = (normalized_url.clone(), title.to_owned());
    if ids.contains_key(&key) {
        return;
    }
    let id = format!("P{}", sources.len() + 1);
    ids.insert(key, id.clone());
    if let Some(position) = position {
        positions.insert(id.clone(), position);
    }
    sources.push(serde_json::json!({
        "id": id,
        "title": title,
        "url": normalized_url,
        "sourceType": "provider_native"
    }));
}

fn append_provider_citation_anchors(
    content: &mut String,
    round_content: &mut String,
    sources: &[Value],
    positions: &BTreeMap<String, usize>,
) {
    let round_length = round_content.chars().count();
    let content_prefix_length = content.chars().count().saturating_sub(round_length);
    let mut insertions = sources
        .iter()
        .filter_map(|source| {
            let id = source.get("id").and_then(Value::as_str)?;
            Some((
                positions
                    .get(id)
                    .copied()
                    .unwrap_or(round_length)
                    .min(round_length),
                format!(" [{id}]"),
            ))
        })
        .collect::<Vec<_>>();
    insertions.sort_by_key(|(index, _)| *index);
    for (index, reference) in insertions.into_iter().rev() {
        let round_byte = round_content
            .char_indices()
            .nth(index)
            .map_or(round_content.len(), |(byte, _)| byte);
        round_content.insert_str(round_byte, &reference);
        let content_index = content_prefix_length.saturating_add(index);
        let content_byte = content
            .char_indices()
            .nth(content_index)
            .map_or(content.len(), |(byte, _)| byte);
        content.insert_str(content_byte, &reference);
    }
}

fn connection_error(route: &super::super::route::ChatRoute, details: String) -> ServiceError {
    if is_local_runtime(route.provider_id.as_deref()) {
        ServiceError::new(
            "local_engine_runtime_unavailable",
            "The local inference server stopped responding. Restart the model and try again.",
            true,
        )
        .with_details(details)
    } else {
        network_error().with_details(details)
    }
}

fn is_local_runtime(provider_id: Option<&str>) -> bool {
    matches!(provider_id, Some("llama_cpp"))
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

fn gemini_content_delta(value: &Value) -> Option<(String, String)> {
    let parts = value
        .pointer("/candidates/0/content/parts")
        .or_else(|| value.pointer("/candidate/content/parts"))?
        .as_array()?;
    let mut reasoning = String::new();
    let mut text = String::new();
    for part in parts {
        let Some(part_text) = part.get("text").and_then(Value::as_str) else {
            continue;
        };
        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            reasoning.push_str(part_text);
        } else {
            text.push_str(part_text);
        }
    }
    Some((reasoning, text))
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
        response_budget::ProviderResponseBudget,
    };

    struct FixtureOutcome {
        result: Result<StreamedTurn, ServiceError>,
        content: String,
        reasoning: String,
        reasoning_summaries: Vec<crate::provider_schema::ReasoningSummary>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        request: Vec<u8>,
        events: Vec<crate::protocol::Response>,
    }

    async fn run_fixture(
        status: u16,
        response_body: Vec<u8>,
        uses_responses_api: bool,
    ) -> FixtureOutcome {
        let mut response_budget = ProviderResponseBudget::default();
        run_fixture_with_budget(
            status,
            response_body,
            uses_responses_api,
            &mut response_budget,
        )
        .await
    }

    async fn run_fixture_with_budget(
        status: u16,
        response_body: Vec<u8>,
        uses_responses_api: bool,
        response_budget: &mut ProviderResponseBudget,
    ) -> FixtureOutcome {
        run_fixture_with_provider_budget(
            "gemini",
            status,
            response_body,
            uses_responses_api,
            response_budget,
        )
        .await
    }

    async fn run_fixture_for_provider(
        provider_id: &str,
        status: u16,
        response_body: Vec<u8>,
        uses_responses_api: bool,
    ) -> FixtureOutcome {
        let mut response_budget = ProviderResponseBudget::default();
        run_fixture_with_provider_budget(
            provider_id,
            status,
            response_body,
            uses_responses_api,
            &mut response_budget,
        )
        .await
    }

    async fn run_fixture_with_provider_budget(
        provider_id: &str,
        status: u16,
        response_body: Vec<u8>,
        uses_responses_api: bool,
        response_budget: &mut ProviderResponseBudget,
    ) -> FixtureOutcome {
        let (url, server) = spawn_fixture_server(status, response_body).await;
        let mut route = route(provider_id);
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
        let (events, mut event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let result = receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                cancellation: &mut cancellation,
                events: &events,
                response_budget,
            },
        )
        .await;
        let mut emitted_events = Vec::new();
        while let Ok(event) = event_receiver.try_recv() {
            emitted_events.push(event);
        }
        let request = server.await.expect("fixture server should finish");
        FixtureOutcome {
            result,
            content,
            reasoning,
            reasoning_summaries,
            input_tokens,
            output_tokens,
            request,
            events: emitted_events,
        }
    }

    #[tokio::test]
    async fn openai_api_surfaces_summary_groups_and_ignores_reasoning_text() {
        let mut response = Vec::new();
        response.extend(sse_event(
            json!({"type": "response.reasoning_text.delta", "delta": "private reasoning"}),
            true,
        ));
        response.extend(sse_event(
            json!({
                "type": "response.reasoning_summary_text.delta",
                "item_id": "rs_item",
                "summary_index": 0,
                "delta": "First summary"
            }),
            true,
        ));
        response.extend(sse_event(
            json!({
                "type": "response.reasoning_summary_text.delta",
                "item_id": "rs_item",
                "summary_index": 1,
                "delta": "Second summary"
            }),
            true,
        ));
        response.extend(sse_event(
            json!({
                "type": "response.reasoning_summary_text.done",
                "item_id": "rs_item",
                "summary_index": 0,
                "text": "First summary"
            }),
            true,
        ));
        response.extend(sse_event(
            json!({
                "type": "response.reasoning_summary_part.done",
                "item_id": "rs_item",
                "summary_index": 1,
                "part": {"type": "summary_text", "text": "Second summary"}
            }),
            true,
        ));
        response.extend(sse_event(
            json!({"type": "response.output_text.delta", "delta": "Final answer"}),
            true,
        ));
        response.extend(sse_event(
            json!({
                "type": "response.completed",
                "response": {"output": [], "usage": {"output_tokens": 1}}
            }),
            true,
        ));

        let outcome = run_fixture_for_provider("chatgpt_api", 200, response, true).await;

        assert!(
            outcome.result.is_ok(),
            "provider error: {:?}",
            outcome.result.as_ref().err().map(|error| error.code)
        );
        assert_eq!(outcome.content, "Final answer");
        assert!(outcome.reasoning.is_empty());
        assert_eq!(outcome.reasoning_summaries.len(), 2);
        assert_eq!(outcome.reasoning_summaries[0].content, "First summary");
        assert_eq!(outcome.reasoning_summaries[1].content, "Second summary");
        assert!(
            outcome
                .reasoning_summaries
                .iter()
                .all(|summary| summary.is_complete)
        );
    }

    async fn receive_for_test(
        client: &Client,
        request: ResponseStreamRequest<'_>,
    ) -> Result<StreamedTurn, ServiceError> {
        receive_with_client(request, client).await
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
        let mut positions = std::collections::BTreeMap::new();
        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["id"], "P1");
        assert_eq!(sources[0]["sourceType"], "provider_native");

        let mut content = "A sourced claim.".to_owned();
        let mut round_content = "A sourced claim.".to_owned();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
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
        let mut positions = std::collections::BTreeMap::new();
        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["title"], "Example article");
        assert_eq!(sources[0]["url"], "https://example.org/article");
        assert_eq!(sources[0]["sourceType"], "provider_native");
    }

    #[test]
    fn parses_gemini_grounding_chunks_and_places_sources_after_supported_text() {
        let value = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Claim. More text."}]},
                "groundingMetadata": {
                    "groundingChunks": [
                        {"web": {"uri": "https://example.org/grounded", "title": "Grounded source"}},
                        {"web": {"uri": "https://example.org/other", "title": "Other source"}}
                    ],
                    "groundingSupports": [{
                        "segment": {"partIndex": 0, "endIndex": 6, "text": "Claim."},
                        "groundingChunkIndices": [0]
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();

        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0]["title"], "Grounded source");
        assert_eq!(positions.get("P1"), Some(&6));
        assert!(!positions.contains_key("P2"));
        let mut content = "Claim. More text.".to_owned();
        let mut round_content = content.clone();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
        assert_eq!(content, "Claim. [P1] More text. [P2]");
    }

    #[test]
    fn converts_gemini_utf8_byte_offsets_to_character_positions() {
        let value = json!({
            "candidates": [{
                "content": {"parts": [{"text": "İzmir güzel."}]},
                "groundingMetadata": {
                    "groundingChunks": [{"web": {
                        "uri": "https://example.org/izmir",
                        "title": "İzmir source"
                    }}],
                    "groundingSupports": [{
                        "segment": {"partIndex": 0, "endIndex": 6, "text": "İzmir"},
                        "groundingChunkIndices": [0]
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();

        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        let mut content = "İzmir güzel.".to_owned();
        let mut round_content = content.clone();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
        assert_eq!(content, "İzmir [P1] güzel.");
    }

    #[test]
    fn rejects_unsafe_gemini_grounding_chunk_urls() {
        let value = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Claim."}]},
                "groundingMetadata": {
                    "groundingChunks": [
                        {"web": {"uri": "file:///private/data", "title": "Local file"}},
                        {"web": {"uri": "https://user:pass@example.org/private", "title": "Credential URL"}}
                    ],
                    "groundingSupports": []
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();

        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        assert!(sources.is_empty());
        assert!(positions.is_empty());
    }

    #[test]
    fn ignores_gemini_grounding_chunks_beyond_the_metadata_limit() {
        let mut chunks = vec![json!({"retrievedContext": {"title": "Ignored context"}}); 64];
        chunks.push(json!({"web": {
            "uri": "https://example.org/over-limit",
            "title": "Over-limit source"
        }}));
        let value = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Claim."}]},
                "groundingMetadata": {
                    "groundingChunks": chunks,
                    "groundingSupports": [{
                        "segment": {"partIndex": 0, "endIndex": 6},
                        "groundingChunkIndices": [64]
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();

        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        assert!(sources.is_empty());
        assert!(positions.is_empty());
    }

    #[test]
    fn places_provider_citations_at_valid_unicode_text_offsets() {
        let value = json!({
            "choices": [{
                "message": {
                    "annotations": [{
                        "type": "url_citation",
                        "url_citation": {
                            "url": "https://example.org/article",
                            "title": "Example article",
                            "start_index": 0,
                            "end_index": 2
                        }
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();
        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        let mut content = "Earlier. Été is warm.".to_owned();
        let mut round_content = "Été is warm.".to_owned();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);

        assert_eq!(content, "Earlier. Été [P1] is warm.");
        assert_eq!(round_content, "Été [P1] is warm.");
    }

    #[test]
    fn appends_provider_citations_when_offsets_are_missing_or_out_of_range() {
        let value = json!({
            "choices": [{
                "message": {
                    "annotations": [{
                        "type": "url_citation",
                        "url_citation": {
                            "url": "https://example.org/article",
                            "title": "Example article",
                            "end_index": 9000
                        }
                    }]
                }
            }]
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();
        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        let mut content = "Answer.".to_owned();
        let mut round_content = "Answer.".to_owned();
        append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);

        assert_eq!(content, "Answer. [P1]");
        assert_eq!(round_content, "Answer. [P1]");
    }

    #[test]
    fn parses_responses_api_completed_output_citations() {
        let value = json!({
            "type": "response.completed",
            "response": {
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "A sourced answer.",
                        "annotations": [{
                            "type": "url_citation",
                            "url": "https://example.org/source",
                            "title": "Provider source"
                        }]
                    }]
                }]
            }
        });
        let mut ids = std::collections::BTreeMap::new();
        let mut sources = Vec::new();
        let mut positions = std::collections::BTreeMap::new();

        collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["url"], "https://example.org/source");
        assert_eq!(sources[0]["title"], "Provider source");
    }

    fn route(provider_id: &str) -> ChatRoute {
        ChatRoute {
            custom_enable_thinking: false,
            model_id: "test-model".to_owned(),
            provider_model_id: "test-model".to_owned(),
            provider_id: Some(provider_id.to_owned()),
            chat_url: String::new(),
            is_free: true,
            is_opencode: false,
            is_copilot: false,
            uses_responses_api: false,
            reasoning_levels: Vec::new(),
            context_window: None,
            input_token_limit: None,
            max_output_tokens: None,
            context_limit_override: None,
            supports_images: false,
            supports_tool_calls: None,
            connection_id: None,
            local_api_key: None,
            project_id: None,
        }
    }

    #[test]
    fn identifies_local_runtime_disconnects_separately_from_provider_network_errors() {
        assert_eq!(
            connection_error(&route("llama_cpp"), "connection reset".to_owned()).code,
            "local_engine_runtime_unavailable"
        );
        assert_eq!(
            connection_error(&route("opencode"), "connection reset".to_owned()).code,
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
        let text_event = outcome
            .events
            .iter()
            .find(|event| event.event.as_deref() == Some("chat.text.delta"))
            .expect("provider text should be streamed as an append-only event");
        let text_data = text_event
            .data
            .as_ref()
            .expect("text delta should contain event data");
        assert_eq!(text_data["delta"], "Searching.");
        assert!(text_data.get("content").is_none());
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
    async fn gemini_grounding_metadata_streams_clickable_citation_sources() {
        let mut response_body = Vec::new();
        response_body.extend(sse_event(
            json!({
                "candidates": [{
                    "content": {"parts": [{"text": "Claim. More text."}]},
                    "groundingMetadata": {
                        "groundingChunks": [{"web": {
                            "uri": "https://example.org/grounded",
                            "title": "Grounded source"
                        }}],
                        "groundingSupports": [{
                            "segment": {"partIndex": 0, "endIndex": 6, "text": "Claim."},
                            "groundingChunkIndices": [0]
                        }]
                    }
                }]
            }),
            true,
        ));
        response_body.extend(sse_event(
            json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}),
            true,
        ));
        response_body.extend(b"data: [DONE]\n\n".iter().copied());

        let outcome = run_fixture(200, response_body, false).await;

        assert!(
            outcome.result.is_ok(),
            "provider error: {:?}",
            outcome.result.as_ref().err().map(|error| error.code)
        );
        assert_eq!(outcome.content, "Claim. [P1] More text.");
        let citation_event = outcome
            .events
            .iter()
            .find(|event| event.event.as_deref() == Some("chat.citations.updated"))
            .expect("stream should emit the citation update");
        assert_eq!(
            citation_event.data.as_ref().unwrap()["sources"][0]["id"],
            "P1"
        );
        assert_eq!(
            citation_event.data.as_ref().unwrap()["sources"][0]["url"],
            "https://example.org/grounded"
        );
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
    async fn provider_response_budget_is_shared_across_follow_up_turns() {
        let mut first_response = Vec::new();
        first_response.extend(sse_event(
            json!({"choices": [{"delta": {"content": "first turn"}}]}),
            true,
        ));
        first_response.extend(sse_event(
            json!({"choices": [{"finish_reason": "tool_calls", "delta": {}}]}),
            true,
        ));
        let mut response_budget = ProviderResponseBudget::with_limit(first_response.len());

        let first = run_fixture_with_budget(200, first_response, false, &mut response_budget).await;
        assert!(first.result.is_ok());
        assert_eq!(first.content, "first turn");

        let second = run_fixture_with_budget(
            200,
            sse_event(
                json!({"choices": [{"delta": {"content": "follow-up turn"}}]}),
                true,
            ),
            false,
            &mut response_budget,
        )
        .await;

        let error = expect_service_error(second.result);
        assert_eq!(error.code, "response_too_large");
    }

    #[tokio::test]
    async fn oversized_provider_stream_keeps_content_received_within_the_budget() {
        let initial = sse_event(
            json!({"choices": [{"delta": {"content": "</think>partial"}}]}),
            true,
        );
        let remaining = sse_event(
            json!({"choices": [{"delta": {"content": "over limit"}}]}),
            true,
        );
        let (url, started, continue_server, server) =
            spawn_pending_stream_server(initial.clone(), remaining).await;
        let mut route = route("gemini");
        route.chat_url = url;
        route.is_free = false;
        let client = fixture_client(Duration::from_secs(5));
        let (events, mut event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let body = json!({"model": "test-model", "messages": []});
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let mut response_budget = ProviderResponseBudget::with_limit(initial.len());
        let mut receive = Box::pin(receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
        ));

        tokio::select! {
            started = started => {
                started.expect("fixture server should remain active");
            }
            result = &mut receive => {
                let _ = result;
                panic!("stream ended before the initial event was released");
            }
        }
        tokio::select! {
            event = tokio::time::timeout(Duration::from_secs(2), event_receiver.recv()) => {
                event
                    .expect("the initial event should be processed before the next chunk")
                    .expect("the initial content delta should emit an event");
            }
            result = &mut receive => {
                let _ = result;
                panic!("stream ended before the initial content was confirmed");
            }
        }
        continue_server
            .send(())
            .expect("release fixture server after the initial content is processed");

        let error = expect_service_error(receive.await);
        assert_eq!(error.code, "response_too_large");
        assert_eq!(content, "partial");
        server.await.expect("fixture server should finish");
    }

    #[tokio::test]
    async fn opencode_free_requests_send_affinity_headers_and_public_auth() {
        let response = sse_event(
            json!({"choices": [{"finish_reason": "stop", "delta": {"content": "ok"}}]}),
            true,
        );
        let (url, server) = spawn_fixture_server(200, response).await;
        let mut route = route("opencode");
        route.chat_url = url;
        route.is_opencode = true;
        route.is_free = true;
        route.project_id = Some("project-test".to_owned());
        let client = fixture_client(Duration::from_secs(5));
        let (events, _event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let body = json!({"model": "big-pickle", "messages": [{"role": "user", "content": "hi"}]});
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let mut response_budget = ProviderResponseBudget::default();

        let result = receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: None,
                session_id: Some("ses_testaffinity"),
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
        )
        .await;

        assert!(result.is_ok());
        let request = server.await.expect("fixture server should finish");
        let request = String::from_utf8(request)
            .expect("request bytes are UTF-8")
            .to_ascii_lowercase();
        assert!(request.contains("authorization: bearer public"));
        assert!(request.contains("x-opencode-session-id: ses_testaffinity"));
        assert!(request.contains("x-session-affinity: ses_testaffinity"));
        assert!(request.contains("x-session-id: ses_testaffinity"));
        assert!(request.contains("x-opencode-session: ses_testaffinity"));
        assert!(request.contains("x-opencode-client: cli"));
        assert!(request.contains("user-agent: opencode/latest/2.1.4/cli"));
        assert!(request.contains("x-opencode-project: project-test"));
    }

    #[tokio::test]
    async fn opencode_free_usage_limit_returns_a_specific_error() {
        let response = br#"{"type":"error","error":{"type":"FreeUsageLimitError","message":"Free usage limit reached. Free models work in OpenCode."}}"#.to_vec();
        let (url, server) = spawn_fixture_server(429, response).await;
        let mut route = route("opencode");
        route.chat_url = url;
        route.is_opencode = true;
        route.is_free = true;
        let client = fixture_client(Duration::from_secs(5));
        let (events, _event_receiver) = EventSink::test_channel();
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let body = json!({"model": "big-pickle", "messages": []});
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let mut response_budget = ProviderResponseBudget::default();

        let error = expect_service_error(
            receive_for_test(
                &client,
                ResponseStreamRequest {
                    route: &route,
                    api_key: None,
                    session_id: Some("ses_testaffinity"),
                    body: &body,
                    request_id: &request_id,
                    conversation_id: "fixture-conversation",
                    message_id: "fixture-message",
                    created_at: 1,
                    content: &mut content,
                    reasoning_content: &mut reasoning,
                    reasoning_summaries: &mut reasoning_summaries,
                    output_tokens: &mut output_tokens,
                    input_tokens: &mut input_tokens,
                    started: Instant::now(),
                    cancellation: &mut cancellation,
                    events: &events,
                    response_budget: &mut response_budget,
                },
            )
            .await,
        );
        assert_eq!(error.code, "opencode_free_usage_limit");
        assert!(!error.retryable);
        server.await.expect("fixture server should finish");
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
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let mut response_budget = ProviderResponseBudget::default();
        let mut receive = Box::pin(receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
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
        let mut reasoning_summaries = Vec::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let request_id = json!("fixture-request");
        let mut response_budget = ProviderResponseBudget::default();
        let mut receive = Box::pin(receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
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
