//! Applies streamed ChatGPT response events to assistant message checkpoints.
//!
//! Checkpointing coalesces text progress while preserving tool, reasoning, and
//! completion state needed by the conversation event stream.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::{ChatGptService, database_error, invalid_response_error, protocol_error};
use crate::{
    chatgpt_store::{self, AssistantMessageWrite},
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamDelta, ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary},
    usage_statistics::UsageData,
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
    pub(super) input_tokens: &'a mut Option<i64>,
    pub(super) usage: &'a mut UsageData,
    pub(super) reasoning_summaries: &'a mut Vec<ReasoningSummaryGroup>,
    pub(super) response_output_items: &'a mut Vec<Value>,
    pub(super) conversation_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) created_at: i64,
    pub(super) request_id: &'a Value,
    pub(super) events: &'a EventSink,
    pub(super) persist_message: bool,
    pub(super) checkpoint: Option<&'a mut AssistantMessageCheckpoint>,
}

const ASSISTANT_MESSAGE_CHECKPOINT_BYTES: usize = 256 * 1024;

/// Bounds how much streamed assistant text remains unwritten during normal operation.
#[derive(Default)]
pub(super) struct AssistantMessageCheckpoint {
    unpersisted_text_bytes: usize,
}

impl AssistantMessageCheckpoint {
    /// Records a newly received fragment and reports when a checkpoint is due.
    fn record_text_delta(&mut self, byte_count: usize) -> bool {
        self.unpersisted_text_bytes = self.unpersisted_text_bytes.saturating_add(byte_count);
        self.unpersisted_text_bytes >= ASSISTANT_MESSAGE_CHECKPOINT_BYTES
    }

    /// Resets the pending byte count after the full message has been saved.
    fn mark_persisted(&mut self) {
        self.unpersisted_text_bytes = 0;
    }
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
            input_tokens,
            usage,
            reasoning_summaries,
            response_output_items,
            conversation_id,
            message_id,
            created_at,
            request_id,
            events,
            persist_message,
            mut checkpoint,
        } = context;
        if event_data == "[DONE]" {
            finish_reasoning_summary_groups(reasoning_summaries);
            return Ok(true);
        }
        let partial_response = if persist_message {
            "The partial answer was saved."
        } else {
            "The partial answer remains in this temporary chat."
        };
        let malformed_event =
            format!("ChatGPT returned a malformed stream event. {partial_response}");
        let unsupported_event =
            format!("ChatGPT returned an unsupported stream event. {partial_response}");
        let event: Value = serde_json::from_str(event_data)
            .map_err(|_| ServiceError::new("invalid_provider_response", malformed_event, false))?;
        let kind = event.get("type").and_then(Value::as_str).ok_or_else(|| {
            ServiceError::new("invalid_provider_response", unsupported_event, false)
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
                        format!("ChatGPT returned an invalid text update. {partial_response}"),
                        false,
                    )
                })?;
                content.push_str(delta);
                let should_checkpoint = persist_message
                    && checkpoint
                        .as_mut()
                        .is_some_and(|checkpoint| checkpoint.record_text_delta(delta.len()));
                if should_checkpoint {
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
                    if let Some(checkpoint) = checkpoint.as_mut() {
                        checkpoint.mark_persisted();
                    }
                }
                events
                    .send(
                        &ChatStreamEvent::TextAppended(ChatStreamDelta::new(
                            conversation_id,
                            message_id,
                            delta,
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
                    *usage = UsageData::from_responses_event(&event, "chatgpt");
                    *output_tokens = usage.output_tokens;
                    *input_tokens = usage.input_tokens;
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
                if event.get("response").is_some() {
                    *usage = UsageData::from_responses_event(&event, "chatgpt");
                    *output_tokens = usage.output_tokens;
                    *input_tokens = usage.input_tokens;
                }
                return Err(ServiceError::new(
                    "provider_request_failed",
                    format!("ChatGPT could not complete the response. {partial_response}"),
                    false,
                ));
            }
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc};

    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        chatgpt::ChatGptService, protocol::EventSink, storage::AppStorage,
        usage_statistics::UsageData,
    };

    use super::{
        ASSISTANT_MESSAGE_CHECKPOINT_BYTES, AssistantMessageCheckpoint, ResponseEventContext,
    };

    #[tokio::test]
    async fn temporary_response_text_is_not_saved_to_durable_messages() {
        let directory = TestDirectory::new();
        let storage = Arc::new(
            AppStorage::open_at(directory.0.clone()).expect("open temporary test storage"),
        );
        storage
            .connect()
            .expect("open database connection")
            .execute_batch(
                "CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    UNIQUE (conversation_id, id)
                );",
            )
            .expect("create message table");
        let service =
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service");
        let mut content = String::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let mut usage = UsageData::default();
        let mut reasoning_summaries = Vec::new();
        let mut response_output_items = Vec::new();
        let (events, mut event_receiver) = EventSink::test_channel();

        service
            .process_response_event(
                &json!({
                    "type": "response.output_text.delta",
                    "delta": "private temporary text"
                })
                .to_string(),
                ResponseEventContext {
                    content: &mut content,
                    output_tokens: &mut output_tokens,
                    input_tokens: &mut input_tokens,
                    usage: &mut usage,
                    reasoning_summaries: &mut reasoning_summaries,
                    response_output_items: &mut response_output_items,
                    conversation_id: "tmp_0123456789abcdef0123456789abcdef",
                    message_id: "0123456789abcdef0123456789abcdef",
                    created_at: 1,
                    request_id: &json!(1),
                    events: &events,
                    persist_message: false,
                    checkpoint: None,
                },
            )
            .await
            .expect("temporary response event should be accepted");

        assert_eq!(content, "private temporary text");
        let event = event_receiver
            .recv()
            .await
            .expect("temporary response should emit a text delta");
        assert_eq!(event.event.as_deref(), Some("chat.text.delta"));
        let data = event.data.expect("text delta should contain event data");
        assert_eq!(data["delta"], "private temporary text");
        assert!(data.get("content").is_none());
        let durable_messages: i64 = storage
            .connect()
            .expect("open database connection")
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .expect("count durable messages");
        assert_eq!(durable_messages, 0);
    }

    #[test]
    fn assistant_message_checkpoint_waits_for_the_configured_text_budget() {
        let mut checkpoint = AssistantMessageCheckpoint::default();

        assert!(!checkpoint.record_text_delta(ASSISTANT_MESSAGE_CHECKPOINT_BYTES - 1));
        assert!(checkpoint.record_text_delta(1));
        checkpoint.mark_persisted();
        assert!(!checkpoint.record_text_delta(ASSISTANT_MESSAGE_CHECKPOINT_BYTES - 1));
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-temporary-response-test-{}",
                Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
