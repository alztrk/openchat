//! Receives, bounds, and interprets a provider's streamed assistant response.
//!
//! This layer validates server-sent events and returns normalized text, usage,
//! tool-call, and completion state to conversation orchestration.

use std::{
    collections::BTreeMap,
    pin::Pin,
    time::{Duration, Instant as StdInstant, SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::{StatusCode, header::ACCEPT};
use serde_json::Value;
use tokio::time::Instant;

use crate::{
    domain::http_headers::sensitive_api_key_header,
    protocol::ServiceError,
    provider_schema::{
        ChatStreamDelta, ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolCall,
    },
    usage_statistics::{ProviderRateLimitObservation, UsageData},
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

const PROVIDER_ERROR_BODY_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct StreamedTurn {
    pub(super) round_content: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) provider_request_usage: Option<ProviderRequestUsage>,
    #[cfg(test)]
    pub(super) first_output_latency: Option<Duration>,
}

struct RoundReasoningState {
    summary_id: Option<String>,
    started_at: Option<Instant>,
}

impl RoundReasoningState {
    fn new() -> Self {
        Self {
            summary_id: None,
            started_at: None,
        }
    }
}

async fn append_visible_text(
    request: &mut ResponseStreamRequest<'_>,
    round_content: &mut String,
    text: &str,
) -> Result<(), ServiceError> {
    if text.is_empty() {
        return Ok(());
    }
    let needs_round_separator = round_content.is_empty()
        && !request.content.is_empty()
        && !request.content.ends_with('\n')
        && !text.starts_with('\n');
    if needs_round_separator {
        let separated = format!("\n\n{text}");
        request.content.push_str(&separated);
        round_content.push_str(text);
        return request
            .events
            .send(
                &ChatStreamEvent::TextAppended(ChatStreamDelta::new(
                    request.conversation_id,
                    request.message_id,
                    &separated,
                    request.created_at,
                ))
                .into_rpc(request.request_id.clone()),
            )
            .await
            .map_err(|_| protocol_error());
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
    round_reasoning: &mut RoundReasoningState,
    output: super::think_tag_filter::ThinkTagOutput,
) -> Result<(), ServiceError> {
    append_visible_text(request, round_content, &output.visible).await?;
    if !output.reasoning.is_empty() {
        append_legacy_reasoning(request, round_reasoning, &output.reasoning).await?;
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
    round_reasoning: &RoundReasoningState,
) -> Result<(), ServiceError> {
    let fallback_elapsed = i64::try_from(request.started.elapsed().as_micros()).unwrap_or(i64::MAX);
    let round_elapsed = round_reasoning
        .started_at
        .map(|started| i64::try_from(started.elapsed().as_micros()).unwrap_or(i64::MAX));
    let summaries = {
        let summaries = &mut *request.reasoning_summaries;
        let mut changed = false;
        for summary in summaries.iter_mut().filter(|summary| !summary.is_complete) {
            summary.is_complete = true;
            summary.elapsed_microseconds = if round_reasoning
                .summary_id
                .as_deref()
                .is_some_and(|id| id == summary.id)
            {
                round_elapsed.unwrap_or(fallback_elapsed)
            } else {
                fallback_elapsed
            };
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
    round_reasoning: &mut RoundReasoningState,
    reasoning: &str,
) -> Result<(), ServiceError> {
    if reasoning.is_empty() {
        return Ok(());
    }
    request.reasoning_content.push_str(reasoning);
    let started_at = *round_reasoning.started_at.get_or_insert_with(Instant::now);
    let elapsed_microseconds = i64::try_from(started_at.elapsed().as_micros()).unwrap_or(i64::MAX);
    let summaries = {
        let summaries = &mut *request.reasoning_summaries;
        let summary_id = round_reasoning.summary_id.get_or_insert_with(|| {
            if summaries.is_empty() {
                format!("reasoning_{}", request.message_id)
            } else {
                format!(
                    "reasoning_{}_{}",
                    request.message_id,
                    summaries.len().saturating_add(1)
                )
            }
        });
        let position = match summaries
            .iter()
            .position(|summary| summary.id == *summary_id)
        {
            Some(position) => position,
            None => {
                summaries.push(ReasoningSummary {
                    id: summary_id.clone(),
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
        summary.content.push_str(reasoning);
        summary.elapsed_microseconds = elapsed_microseconds;
        summaries.clone()
    };
    send_reasoning_summaries(request, summaries).await
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
    provider_processing_ms: Option<i64>,
    provider_rate_limits: Option<ProviderRateLimitObservation>,
    body: ProviderBody,
}

impl ProviderResponse {
    fn from_reqwest(response: reqwest::Response, provider_id: Option<&str>) -> Self {
        let status = response.status();
        let retry_after = retry_after_seconds(response.headers());
        let provider_processing_ms =
            openai_processing_ms(provider_id, response.url(), response.headers());
        let provider_rate_limits = provider_rate_limit_observation(
            provider_id,
            response.url(),
            response.headers(),
            current_unix_millis(),
        );
        let body = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|error| error.without_url().to_string()));
        Self {
            status,
            retry_after,
            provider_processing_ms,
            provider_rate_limits,
            body: Box::pin(body),
        }
    }

    fn from_free(response: opencode_free_transport::FreeResponse) -> Self {
        Self {
            status: response.status,
            retry_after: response.retry_after,
            provider_processing_ms: None,
            provider_rate_limits: None,
            body: response.body,
        }
    }
}

fn openai_processing_ms(
    provider_id: Option<&str>,
    url: &reqwest::Url,
    headers: &reqwest::header::HeaderMap,
) -> Option<i64> {
    if !matches!(provider_id, Some("openai" | "chatgpt_api"))
        || url.scheme() != "https"
        || url.host_str() != Some("api.openai.com")
        || url.port_or_known_default() != Some(443)
    {
        return None;
    }

    headers
        .get("openai-processing-ms")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|milliseconds| *milliseconds >= 0)
}

fn current_unix_millis() -> Option<i64> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()
}

fn provider_rate_limit_observation(
    provider_id: Option<&str>,
    url: &reqwest::Url,
    headers: &reqwest::header::HeaderMap,
    observed_at_unix_ms: Option<i64>,
) -> Option<ProviderRateLimitObservation> {
    let expected_host = match provider_id {
        Some("openai") => "api.openai.com",
        Some("groq") => "api.groq.com",
        _ => return None,
    };
    if url.scheme() != "https"
        || url.host_str() != Some(expected_host)
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }

    let observed_at_unix_ms = observed_at_unix_ms?;
    let request_limit = rate_limit_header(headers, "x-ratelimit-limit-requests");
    let request_remaining = rate_limit_header(headers, "x-ratelimit-remaining-requests");
    let request_reset_at_unix_ms =
        rate_limit_reset_at(headers, "x-ratelimit-reset-requests", observed_at_unix_ms);
    let token_limit = rate_limit_header(headers, "x-ratelimit-limit-tokens");
    let token_remaining = rate_limit_header(headers, "x-ratelimit-remaining-tokens");
    let token_reset_at_unix_ms =
        rate_limit_reset_at(headers, "x-ratelimit-reset-tokens", observed_at_unix_ms);
    if [
        request_limit,
        request_remaining,
        request_reset_at_unix_ms,
        token_limit,
        token_remaining,
        token_reset_at_unix_ms,
    ]
    .iter()
    .all(Option::is_none)
    {
        return None;
    }

    Some(ProviderRateLimitObservation {
        observed_at_unix_ms,
        request_limit,
        request_remaining,
        request_reset_at_unix_ms,
        token_limit,
        token_remaining,
        token_reset_at_unix_ms,
    })
}

fn rate_limit_header(headers: &reqwest::header::HeaderMap, name: &str) -> Option<i64> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
}

fn rate_limit_reset_at(
    headers: &reqwest::header::HeaderMap,
    name: &str,
    observed_at_unix_ms: i64,
) -> Option<i64> {
    let value = headers.get(name)?.to_str().ok()?;
    observed_at_unix_ms.checked_add(rate_limit_duration_ms(value)?)
}

fn rate_limit_duration_ms(value: &str) -> Option<i64> {
    let bytes = value.trim().as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return None;
    }

    let mut position = 0;
    let mut total_millis = 0.0_f64;
    while position < bytes.len() {
        let number_start = position;
        let mut saw_digit = false;
        while position < bytes.len() && bytes[position].is_ascii_digit() {
            saw_digit = true;
            position += 1;
        }
        if position < bytes.len() && bytes[position] == b'.' {
            position += 1;
            let fraction_start = position;
            while position < bytes.len() && bytes[position].is_ascii_digit() {
                saw_digit = true;
                position += 1;
            }
            if position == fraction_start {
                return None;
            }
        }
        if !saw_digit {
            return None;
        }

        let number = std::str::from_utf8(bytes.get(number_start..position)?).ok()?;
        let amount = number.parse::<f64>().ok()?;
        let multiplier = if bytes.get(position..position + 2) == Some(b"ms") {
            position += 2;
            1.0
        } else {
            match bytes.get(position).copied()? {
                b'h' => {
                    position += 1;
                    3_600_000.0
                }
                b'm' => {
                    position += 1;
                    60_000.0
                }
                b's' => {
                    position += 1;
                    1_000.0
                }
                _ => return None,
            }
        };
        total_millis += amount * multiplier;
        if !total_millis.is_finite() || total_millis > 31_536_000_000.0 {
            return None;
        }
    }

    Some(total_millis.round() as i64)
}

fn retry_after_seconds(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (1..=86_400).contains(seconds))
}

async fn read_error_body(
    body: &mut ProviderBody,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<Option<Value>, ServiceError> {
    read_error_body_until(
        body,
        cancellation,
        Instant::now() + PROVIDER_ERROR_BODY_TIMEOUT,
    )
    .await
}

async fn read_error_body_until(
    body: &mut ProviderBody,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
    deadline: Instant,
) -> Result<Option<Value>, ServiceError> {
    let mut collected = Vec::new();
    while collected.len() < MAX_PROVIDER_ERROR_BYTES {
        let next = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            result = tokio::time::timeout_at(deadline, body.next()) => match result {
                Ok(next) => next,
                Err(_) => return Ok(None),
            },
        };
        let Some(Ok(chunk)) = next else {
            break;
        };
        let remaining = MAX_PROVIDER_ERROR_BYTES - collected.len();
        collected.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    Ok(serde_json::from_slice::<Value>(&collected).ok())
}

async fn receive_with_client(
    mut request: ResponseStreamRequest<'_>,
    client: &reqwest::Client,
) -> Result<StreamedTurn, ServiceError> {
    let provider_request_started = StdInstant::now();
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
        ProviderResponse::from_reqwest(response, request.route.provider_id.as_deref())
    };
    let ProviderResponse {
        status,
        retry_after: retry_after_value,
        provider_processing_ms,
        provider_rate_limits,
        body: mut stream,
    } = response;
    if let Some(rate_limit_observation) = request.rate_limit_observation.as_mut() {
        **rate_limit_observation = provider_rate_limits.clone();
    }
    let retry_after_seconds = retry_after_value;
    if !status.is_success() {
        if request.route.is_opencode
            && request.route.is_free
            && matches!(
                status,
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            )
            && let Some(error_body) = read_error_body(&mut stream, request.cancellation).await?
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
    let mut round_reasoning = RoundReasoningState::new();
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
    let mut first_output_latency = None;

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
                if first_output_latency.is_none() && anthropic_event_has_output(&value) {
                    let elapsed = provider_request_started.elapsed();
                    first_output_latency = Some(elapsed);
                    if let Some(recorder) = request.first_output_recorder {
                        recorder.record(elapsed);
                    }
                }
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
                                    append_think_output(
                                        &mut request,
                                        &mut round_content,
                                        &mut round_reasoning,
                                        output,
                                    )
                                    .await?;
                                }
                            }
                            Some("thinking_delta") => {
                                if let Some(reasoning) = delta
                                    .and_then(|delta| delta.get("thinking"))
                                    .and_then(Value::as_str)
                                {
                                    append_legacy_reasoning(
                                        &mut request,
                                        &mut round_reasoning,
                                        reasoning,
                                    )
                                    .await?;
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
                        if let Some(reason) =
                            UsageData::from_messages_event(&value, "anthropic").completion_reason
                        {
                            provider_request_usage
                                .get_or_insert_with(ProviderRequestUsage::default)
                                .completion_reason = Some(reason);
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
                if first_output_latency.is_none() && responses_event_has_output(&value) {
                    let elapsed = provider_request_started.elapsed();
                    first_output_latency = Some(elapsed);
                    if let Some(recorder) = request.first_output_recorder {
                        recorder.record(elapsed);
                    }
                }
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
                        append_think_output(
                            &mut request,
                            &mut round_content,
                            &mut round_reasoning,
                            output,
                        )
                        .await?;
                    }
                    ResponsesStreamEffect::ReasoningDelta(reasoning) => {
                        if request.route.provider_id.as_deref() != Some("chatgpt_api") {
                            append_legacy_reasoning(&mut request, &mut round_reasoning, &reasoning)
                                .await?;
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
                            append_legacy_reasoning(&mut request, &mut round_reasoning, &delta)
                                .await?;
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
                        finish_reasoning_summaries(&mut request, &round_reasoning).await?;
                    }
                    ResponsesStreamEffect::Completed => {
                        saw_done = true;
                        finish_reasoning_summaries(&mut request, &round_reasoning).await?;
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
            let has_tool_delta = value
                .pointer("/choices/0/delta/tool_calls")
                .and_then(Value::as_array)
                .is_some_and(|deltas| !deltas.is_empty());
            if first_output_latency.is_none()
                && (!visible_delta.is_empty()
                    || !output.reasoning.is_empty()
                    || reasoning.is_some_and(|text| !text.is_empty())
                    || has_tool_delta)
            {
                let elapsed = provider_request_started.elapsed();
                first_output_latency = Some(elapsed);
                if let Some(recorder) = request.first_output_recorder {
                    recorder.record(elapsed);
                }
            }
            if !output.reasoning.is_empty() {
                append_legacy_reasoning(&mut request, &mut round_reasoning, &output.reasoning)
                    .await?;
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
            if let Some(reasoning) = reasoning {
                append_legacy_reasoning(&mut request, &mut round_reasoning, reasoning).await?;
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
    append_think_output(
        &mut request,
        &mut round_content,
        &mut round_reasoning,
        trailing_output,
    )
    .await?;

    let _ = finish_reasoning_summaries(&mut request, &round_reasoning).await;
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

    if let Some(milliseconds) = provider_processing_ms {
        provider_request_usage
            .get_or_insert_with(ProviderRequestUsage::default)
            .provider_processing_ms = Some(milliseconds);
    }

    if let Some(rate_limits) = provider_rate_limits {
        provider_request_usage
            .get_or_insert_with(ProviderRequestUsage::default)
            .provider_rate_limits = Some(rate_limits);
    }

    Ok(StreamedTurn {
        round_content,
        tool_calls: parse_streamed_tool_calls(tool_calls, request.route.is_opencode)?,
        provider_request_usage,
        #[cfg(test)]
        first_output_latency,
    })
}

fn responses_event_has_output(value: &Value) -> bool {
    matches!(
        value.get("type").and_then(Value::as_str),
        Some(
            "response.output_text.delta"
                | "response.function_call_arguments.delta"
                | "response.reasoning_text.delta"
                | "response.reasoning_summary_text.delta"
        )
    ) && value
        .get("delta")
        .and_then(Value::as_str)
        .is_some_and(|delta| !delta.is_empty())
}

fn anthropic_event_has_output(value: &Value) -> bool {
    if value.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return false;
    }
    match value.pointer("/delta/type").and_then(Value::as_str) {
        Some("text_delta") => value
            .pointer("/delta/text")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.is_empty()),
        Some("thinking_delta") => value
            .pointer("/delta/thinking")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.is_empty()),
        Some("input_json_delta") => value
            .pointer("/delta/partial_json")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.is_empty()),
        _ => false,
    }
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

#[path = "response_stream/tests.rs"]
#[cfg(test)]
mod tests;
