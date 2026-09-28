use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::response_parser::{
    empty_response_shape, parse_responses_tool_calls, response_output_text,
};
use super::{
    CHATGPT_CODEX_BASE, ChatGptService, MAX_STREAM_EVENT_BYTES, database_error, http_error,
    invalid_response_error, network_error, now_unix_millis, protocol_error,
};
use crate::{
    chat_operation::ChatSendContext,
    chatgpt_store::{self, ChatGptModel},
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary},
    tools::ToolExecutor,
};

struct ReasoningSummaryGroup {
    item_id: String,
    summary_index: u64,
    content: String,
    started_at: Instant,
    elapsed: Option<Duration>,
    is_complete: bool,
}

fn reasoning_summary_group_mut<'a>(
    groups: &'a mut Vec<ReasoningSummaryGroup>,
    item_id: &str,
    summary_index: u64,
) -> &'a mut ReasoningSummaryGroup {
    let index = match groups
        .iter()
        .position(|group| group.item_id == item_id && group.summary_index == summary_index)
    {
        Some(index) => index,
        None => {
            groups.push(ReasoningSummaryGroup {
                item_id: item_id.to_owned(),
                summary_index,
                content: String::new(),
                started_at: Instant::now(),
                elapsed: None,
                is_complete: false,
            });
            groups.len() - 1
        }
    };
    &mut groups[index]
}

fn reasoning_summary_groups_value(groups: &[ReasoningSummaryGroup]) -> Vec<ReasoningSummary> {
    let now = Instant::now();
    groups
        .iter()
        .map(|group| {
            let elapsed = group
                .elapsed
                .unwrap_or_else(|| now.saturating_duration_since(group.started_at));
            ReasoningSummary {
                id: format!("{}:{}", group.item_id, group.summary_index),
                content: group.content.clone(),
                elapsed_microseconds: i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX),
                is_complete: group.is_complete,
            }
        })
        .collect()
}

async fn send_reasoning_snapshot(
    events: &EventSink,
    request_id: &Value,
    conversation_id: &str,
    message_id: &str,
    content: &str,
    created_at_unix_ms: i64,
    groups: &[ReasoningSummaryGroup],
) -> Result<(), ServiceError> {
    events
        .send(
            &ChatStreamEvent::ReasoningSummariesUpdated {
                snapshot: ChatStreamSnapshot::new(
                    conversation_id,
                    message_id,
                    content,
                    created_at_unix_ms,
                ),
                summaries: reasoning_summary_groups_value(groups),
            }
            .into_rpc(request_id.clone()),
        )
        .await
        .map_err(|_| protocol_error())
}

fn finish_reasoning_summary_group(
    groups: &mut Vec<ReasoningSummaryGroup>,
    item_id: &str,
    summary_index: u64,
    content: Option<&str>,
) {
    let group = reasoning_summary_group_mut(groups, item_id, summary_index);
    if let Some(content) = content {
        group.content = content.to_owned();
    }
    group.elapsed = Some(Instant::now().saturating_duration_since(group.started_at));
    group.is_complete = true;
}

fn finish_reasoning_summary_groups(groups: &mut [ReasoningSummaryGroup]) {
    let now = Instant::now();
    for group in groups.iter_mut().filter(|group| !group.is_complete) {
        group.elapsed = Some(now.saturating_duration_since(group.started_at));
        group.is_complete = true;
    }
}

impl ChatGptService {
    pub(super) async fn stream_response(
        self: &Arc<Self>,
        context: ChatSendContext<'_>,
        route: &chatgpt_store::ConversationRoute,
        external_workspace_id: &str,
        model: &ChatGptModel,
        mut payload: Value,
    ) -> Result<Value, ServiceError> {
        let ChatSendContext {
            request_id,
            conversation_id,
            excluded_assistant_message_id,
            project_root,
            data_root,
            permission_mode,
            permission_broker,
            cancellation,
            events,
            ..
        } = context;
        let message_id = Uuid::new_v4().simple().to_string();
        let created_at = now_unix_millis()?;
        let started = Instant::now();
        let mut content = String::new();
        let mut reasoning_summaries = Vec::new();
        self.record_chatgpt_event("request_started", "responses", None, None, None, None);
        chatgpt_store::save_assistant_message(
            &self.storage,
            conversation_id,
            &message_id,
            &content,
            "streaming",
            created_at,
            None,
            None,
            None,
        )
        .map_err(database_error)?;

        events
            .send(
                &ChatStreamEvent::Started(ChatStreamSnapshot {
                    conversation_id: conversation_id.to_owned(),
                    message_id: message_id.clone(),
                    content: content.clone(),
                    created_at_unix_ms: created_at,
                })
                .into_rpc(request_id.clone()),
            )
            .await
            .map_err(|_| protocol_error())?;

        if *cancellation.borrow() {
            self.record_chatgpt_event(
                "request_cancelled",
                "responses",
                None,
                Some("request_cancelled"),
                Some(started.elapsed().as_millis()),
                None,
            );
            self.persist_terminal_message(
                conversation_id,
                &message_id,
                &content,
                "stopped",
                created_at,
                None,
                started.elapsed(),
            )?;
            return Ok(json!({
                "conversationId": conversation_id,
                "messageId": message_id,
                "status": "stopped",
                "outputTokens": Value::Null,
                "tokensPerSecond": Value::Null,
                "elapsedMicroseconds": started.elapsed().as_micros(),
            }));
        }

        let mut output_tokens = None;
        let mut tool_executor = ToolExecutor::new(project_root, data_root, permission_mode);
        'model_turn: loop {
            let mut round_output_tokens = None;
            let mut response_output_items = Vec::new();
            let response = match self
                .authorized_stream_request(
                    Method::POST,
                    format!("{CHATGPT_CODEX_BASE}/responses"),
                    &route.connection_id,
                    external_workspace_id,
                    Some(payload.clone()),
                    conversation_id,
                    cancellation,
                )
                .await
            {
                Ok(Some(response)) => {
                    let status = response.status();
                    self.record_chatgpt_event(
                        "http_response",
                        "responses",
                        Some(status.as_u16()),
                        None,
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    if status.is_success() {
                        response
                    } else {
                        let error = http_error(status);
                        self.record_chatgpt_event(
                            "request_failed",
                            "responses",
                            Some(status.as_u16()),
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        self.persist_terminal_message(
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            None,
                            started.elapsed(),
                        )?;
                        return Err(error);
                    }
                }
                Ok(None) => {
                    let elapsed = started.elapsed();
                    self.record_chatgpt_event(
                        "request_cancelled",
                        "responses",
                        None,
                        Some("request_cancelled"),
                        Some(elapsed.as_millis()),
                        None,
                    );
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "stopped",
                        created_at,
                        None,
                        elapsed,
                    )?;
                    return Ok(json!({
                        "conversationId": conversation_id,
                        "messageId": message_id,
                        "status": "stopped",
                        "outputTokens": Value::Null,
                        "tokensPerSecond": Value::Null,
                        "elapsedMicroseconds": elapsed.as_micros(),
                    }));
                }
                Err(error) => {
                    self.record_chatgpt_event(
                        "request_failed",
                        "responses",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        None,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
            };

            let mut body_stream = response.bytes_stream();
            let mut pending_bytes = Vec::new();
            let mut event_data = Vec::<String>::new();
            let mut completed = false;
            let mut failure = None;

            loop {
                let next = tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(started.elapsed().as_millis()),
                                None,
                            );
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "stopped",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
                            return Ok(json!({
                                "conversationId": conversation_id,
                                "messageId": message_id,
                                "status": "stopped",
                                "outputTokens": output_tokens,
                                "tokensPerSecond": output_tokens.and_then(|tokens| {
                                    let seconds = started.elapsed().as_secs_f64();
                                    (seconds > 0.0).then_some(tokens as f64 / seconds)
                                }),
                                "elapsedMicroseconds": started.elapsed().as_micros(),
                            }));
                        }
                        continue;
                    }
                    chunk = body_stream.next() => chunk,
                };

                let Some(chunk) = next else { break };
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "responses",
                            None,
                            Some("network_unavailable"),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        self.persist_terminal_message(
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            output_tokens,
                            started.elapsed(),
                        )?;
                        return Err(network_error());
                    }
                };
                pending_bytes.extend_from_slice(&chunk);
                if pending_bytes.len() > MAX_STREAM_EVENT_BYTES {
                    self.record_chatgpt_event(
                        "request_failed",
                        "responses",
                        None,
                        Some("response_event_too_large"),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(ServiceError::new(
                        "response_event_too_large",
                        "ChatGPT returned an oversized stream event. The partial answer was saved.",
                        false,
                    ));
                }
                while let Some(line_end) = pending_bytes.iter().position(|byte| *byte == b'\n') {
                    let mut line = pending_bytes.drain(..=line_end).collect::<Vec<_>>();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    if line.is_empty() {
                        if event_data.is_empty() {
                            continue;
                        }
                        let event = event_data.join("\n");
                        event_data.clear();
                        match self
                            .process_response_event(
                                &event,
                                &mut content,
                                &mut round_output_tokens,
                                &mut reasoning_summaries,
                                &mut response_output_items,
                                conversation_id,
                                &message_id,
                                created_at,
                                started,
                                &request_id,
                                &events,
                            )
                            .await
                        {
                            Ok(true) => completed = true,
                            Ok(false) => {}
                            Err(error) => {
                                failure = Some(error);
                                break;
                            }
                        }
                        if completed || failure.is_some() {
                            break;
                        }
                    } else if let Some(data) = line.strip_prefix(b"data:") {
                        let data = data.strip_prefix(b" ").unwrap_or(data);
                        match String::from_utf8(data.to_vec()) {
                            Ok(data) => event_data.push(data),
                            Err(_) => {
                                let error = ServiceError::new(
                                    "invalid_provider_response",
                                    "ChatGPT returned a malformed stream event.",
                                    false,
                                );
                                self.record_chatgpt_event(
                                    "request_failed",
                                    "responses",
                                    None,
                                    Some(error.code),
                                    Some(started.elapsed().as_millis()),
                                    None,
                                );
                                self.persist_terminal_message(
                                    conversation_id,
                                    &message_id,
                                    &content,
                                    "failed",
                                    created_at,
                                    output_tokens,
                                    started.elapsed(),
                                )?;
                                return Err(error);
                            }
                        }
                    }
                }
                if completed || failure.is_some() {
                    break;
                }
            }

            if let Some(error) = failure {
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
                return Err(error);
            }
            if !completed {
                let error = ServiceError::new(
                    "response_incomplete",
                    "The ChatGPT response ended before completion. The partial answer was saved.",
                    true,
                );
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
                return Err(error);
            }

            if let Some(round_tokens) = round_output_tokens {
                output_tokens = Some(output_tokens.unwrap_or(0i64).saturating_add(round_tokens));
            }
            let final_text = response_output_text(&response_output_items);
            if !final_text.is_empty() && final_text != content.as_str() {
                content = final_text;
                chatgpt_store::save_assistant_message(
                    &self.storage,
                    conversation_id,
                    &message_id,
                    &content,
                    "streaming",
                    created_at,
                    None,
                    None,
                    None,
                )
                .map_err(database_error)?;
                events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            conversation_id,
                            &message_id,
                            &content,
                            created_at,
                        ))
                        .into_rpc(request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
            }
            let tool_calls = match parse_responses_tool_calls(&response_output_items) {
                Ok(tool_calls) => tool_calls,
                Err(error) => {
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
            };
            if !tool_calls.is_empty() {
                if let Err(error) = tool_executor.begin_round(&tool_calls) {
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
                let mut results = Vec::with_capacity(tool_calls.len());
                for call in &tool_calls {
                    let snapshot =
                        ChatStreamSnapshot::new(conversation_id, &message_id, &content, created_at);
                    let result = match tool_executor
                        .execute_call(
                            call,
                            permission_broker,
                            &request_id,
                            &snapshot,
                            &events,
                            cancellation,
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) if error.code == "operation_cancelled" => {
                            let elapsed = started.elapsed();
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(elapsed.as_millis()),
                                None,
                            );
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "stopped",
                                created_at,
                                output_tokens,
                                elapsed,
                            )?;
                            return Ok(json!({
                                "conversationId": conversation_id,
                                "messageId": message_id,
                                "status": "stopped",
                                "outputTokens": output_tokens,
                                "tokensPerSecond": output_tokens.and_then(|tokens| {
                                    let seconds = elapsed.as_secs_f64();
                                    (seconds > 0.0).then_some(tokens as f64 / seconds)
                                }),
                                "elapsedMicroseconds": elapsed.as_micros(),
                                "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
                            }));
                        }
                        Err(error) => {
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
                            return Err(error);
                        }
                    };
                    results.push(result);
                }
                let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(invalid_response_error());
                };
                input.extend(response_output_items);
                for result in results {
                    let output = match serde_json::to_string(&result.output) {
                        Ok(output) => output,
                        Err(_) => {
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
                            return Err(invalid_response_error());
                        }
                    };
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": result.call_id,
                        "output": output
                    }));
                }
                continue 'model_turn;
            }
            if content.trim().is_empty() {
                let error = ServiceError::new(
                    "empty_provider_response",
                    "ChatGPT completed without returning visible text or requesting a tool.",
                    true,
                );
                self.record_chatgpt_event(
                    "response_without_content",
                    "responses",
                    None,
                    Some(empty_response_shape(&response_output_items)),
                    None,
                    Some(response_output_items.len()),
                );
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
            break 'model_turn;
        }

        let elapsed = started.elapsed();
        self.persist_terminal_message(
            conversation_id,
            &message_id,
            &content,
            "completed",
            created_at,
            output_tokens,
            elapsed,
        )?;
        self.record_chatgpt_event(
            "request_completed",
            "responses",
            None,
            None,
            Some(elapsed.as_millis()),
            None,
        );
        let title_route = route.clone();
        let title_conversation_id = conversation_id.to_owned();
        let title_excluded_assistant_message_id = excluded_assistant_message_id.map(str::to_owned);
        let title_events = events.clone();
        let service = Arc::clone(self);
        let conversation_model_id = model.id.clone();
        let title_is_automatic = route.title_is_automatic;
        if title_is_automatic {
            tokio::spawn(async move {
                service
                    .generate_title(
                        &title_conversation_id,
                        &title_route,
                        &conversation_model_id,
                        title_excluded_assistant_message_id.as_deref(),
                        title_events,
                    )
                    .await;
            });
        }
        Ok(json!({
            "conversationId": conversation_id,
            "messageId": message_id,
            "status": "completed",
            "outputTokens": output_tokens,
            "tokensPerSecond": output_tokens.and_then(|tokens| {
                let seconds = elapsed.as_secs_f64();
                (seconds > 0.0).then_some(tokens as f64 / seconds)
            }),
            "elapsedMicroseconds": elapsed.as_micros(),
            "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
        }))
    }

    async fn process_response_event(
        &self,
        event_data: &str,
        content: &mut String,
        output_tokens: &mut Option<i64>,
        reasoning_summaries: &mut Vec<ReasoningSummaryGroup>,
        response_output_items: &mut Vec<Value>,
        conversation_id: &str,
        message_id: &str,
        created_at: i64,
        started: Instant,
        request_id: &Value,
        events: &EventSink,
    ) -> Result<bool, ServiceError> {
        if event_data == "[DONE]" {
            finish_reasoning_summary_groups(reasoning_summaries);
            return Ok(true);
        }
        let event: Value = serde_json::from_str(event_data).map_err(|_| {
            ServiceError::new(
                "invalid_provider_response",
                "ChatGPT returned a malformed stream event. The partial answer was saved.",
                false,
            )
        })?;
        let kind = event.get("type").and_then(Value::as_str).ok_or_else(|| {
            ServiceError::new(
                "invalid_provider_response",
                "ChatGPT returned an unsupported stream event. The partial answer was saved.",
                false,
            )
        })?;
        match kind {
            "response.output_item.done" => {
                let item = event
                    .get("item")
                    .filter(|item| item.is_object())
                    .ok_or_else(invalid_response_error)?;
                response_output_items.push(item.clone());
            }
            "response.output_text.delta" => {
                let delta = event.get("delta").and_then(Value::as_str).ok_or_else(|| {
                    ServiceError::new(
                        "invalid_provider_response",
                        "ChatGPT returned an invalid text update. The partial answer was saved.",
                        false,
                    )
                })?;
                content.push_str(delta);
                chatgpt_store::save_assistant_message(
                    &self.storage,
                    conversation_id,
                    message_id,
                    content,
                    "streaming",
                    created_at,
                    None,
                    None,
                    None,
                )
                .map_err(database_error)?;
                events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            conversation_id,
                            message_id,
                            content,
                            created_at,
                        ))
                        .into_rpc(request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
            }
            "response.reasoning_summary_part.added" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    let group =
                        reasoning_summary_group_mut(reasoning_summaries, item_id, summary_index);
                    if let Some(part_content) = event
                        .get("part")
                        .and_then(|part| part.get("text"))
                        .and_then(Value::as_str)
                    {
                        group.content = part_content.to_owned();
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let (Some(item_id), Some(summary_index), Some(delta)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    let group =
                        reasoning_summary_group_mut(reasoning_summaries, item_id, summary_index);
                    group.content.push_str(delta);
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.reasoning_summary_text.done" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    finish_reasoning_summary_group(
                        reasoning_summaries,
                        item_id,
                        summary_index,
                        event.get("text").and_then(Value::as_str),
                    );
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.reasoning_summary_part.done" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    finish_reasoning_summary_group(
                        reasoning_summaries,
                        item_id,
                        summary_index,
                        event
                            .get("part")
                            .and_then(|part| part.get("text"))
                            .and_then(Value::as_str),
                    );
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.completed" => {
                finish_reasoning_summary_groups(reasoning_summaries);
                if let Some(response) = event.get("response") {
                    *output_tokens = response
                        .get("usage")
                        .and_then(|usage| usage.get("output_tokens"))
                        .and_then(Value::as_i64);
                    if let Some(items) = response
                        .get("output")
                        .and_then(Value::as_array)
                        .filter(|items| !items.is_empty())
                    {
                        response_output_items.clear();
                        response_output_items.extend(items.iter().cloned());
                    }
                }
                return Ok(true);
            }
            "response.failed" | "response.incomplete" | "error" => {
                return Err(ServiceError::new(
                    "provider_request_failed",
                    "ChatGPT could not complete the response. The partial answer was saved.",
                    false,
                ));
            }
            _ => {}
        }
        let _ = started;
        Ok(false)
    }
}
