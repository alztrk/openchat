use std::time::{Duration, Instant};

use serde_json::Value;

use super::{ChatGptService, database_error, invalid_response_error, protocol_error};
use crate::{
    chatgpt_store::{self, AssistantMessageWrite},
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary},
};
pub(super) struct ReasoningSummaryGroup {
    item_id: String,
    summary_index: u64,
    content: String,
    started_at: Instant,
    elapsed: Option<Duration>,
    is_complete: bool,
}

pub(super) struct ResponseEventContext<'a> {
    pub(super) content: &'a mut String,
    pub(super) output_tokens: &'a mut Option<i64>,
    pub(super) reasoning_summaries: &'a mut Vec<ReasoningSummaryGroup>,
    pub(super) response_output_items: &'a mut Vec<Value>,
    pub(super) conversation_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) created_at: i64,
    pub(super) request_id: &'a Value,
    pub(super) events: &'a EventSink,
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

pub(super) fn reasoning_summary_groups_value(
    groups: &[ReasoningSummaryGroup],
) -> Vec<ReasoningSummary> {
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
    pub(super) async fn process_response_event(
        &self,
        event_data: &str,
        context: ResponseEventContext<'_>,
    ) -> Result<bool, ServiceError> {
        let ResponseEventContext {
            content,
            output_tokens,
            reasoning_summaries,
            response_output_items,
            conversation_id,
            message_id,
            created_at,
            request_id,
            events,
        } = context;
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
                    AssistantMessageWrite {
                        conversation_id,
                        message_id,
                        content,
                        status: "streaming",
                        created_at_unix_ms: created_at,
                        output_tokens: None,
                        elapsed: None,
                    },
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
        Ok(false)
    }
}
