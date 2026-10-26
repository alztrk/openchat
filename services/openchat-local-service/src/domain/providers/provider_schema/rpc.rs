//! Encodes provider-neutral chat stream events into the service RPC contract.
//!
//! Snapshot events preserve full message state; append-only text events carry
//! only the new fragment to avoid repeatedly serializing accumulated content.

use serde::Serialize;
use serde_json::{Value, json};

use crate::protocol::Response;

use super::{ChatStreamDelta, ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolActivity};

impl ChatStreamEvent {
    /// Converts an internal stream event into its stable RPC event envelope.
    pub fn into_rpc(self, request_id: Value) -> Response {
        match self {
            Self::Started(snapshot) => Response::event(
                request_id,
                "chat.started",
                snapshot_data(snapshot, None, None),
            ),
            Self::TextUpdated(snapshot) => Response::event(
                request_id,
                "chat.delta",
                snapshot_data(snapshot, None, None),
            ),
            Self::TextAppended(delta) => {
                Response::event(request_id, "chat.text.delta", delta_data(delta))
            }
            Self::ToolActivityUpdated { snapshot, activity } => Response::event(
                request_id,
                "chat.tool.updated",
                snapshot_data(snapshot, None, Some(activity)),
            ),
            Self::ReasoningSummariesUpdated {
                snapshot,
                summaries,
            } => Response::event(
                request_id,
                "chat.reasoning.delta",
                snapshot_data(snapshot, Some(summaries), None),
            ),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamData {
    conversation_id: String,
    message_id: String,
    content: String,
    created_at_unix_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_groups: Option<Vec<ReasoningSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_activity: Option<ToolActivity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamDeltaData {
    conversation_id: String,
    message_id: String,
    delta: String,
    created_at_unix_ms: i64,
}

fn snapshot_data(
    snapshot: ChatStreamSnapshot,
    reasoning_groups: Option<Vec<ReasoningSummary>>,
    tool_activity: Option<ToolActivity>,
) -> Value {
    json!(ChatStreamData {
        conversation_id: snapshot.conversation_id,
        message_id: snapshot.message_id,
        content: snapshot.content,
        created_at_unix_ms: snapshot.created_at_unix_ms,
        reasoning_groups,
        tool_activity,
    })
}

fn delta_data(delta: ChatStreamDelta) -> Value {
    json!(ChatStreamDeltaData {
        conversation_id: delta.conversation_id,
        message_id: delta.message_id,
        delta: delta.delta,
        created_at_unix_ms: delta.created_at_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use serde_json::json;

    use super::*;

    #[test]
    fn appended_text_rpc_contains_only_the_new_fragment() {
        let response = ChatStreamEvent::TextAppended(ChatStreamDelta::new(
            "conversation-id",
            "message-id",
            "new text",
            42,
        ))
        .into_rpc(json!(1));

        assert_eq!(response.event.as_deref(), Some("chat.text.delta"));
        let data = response
            .data
            .as_ref()
            .expect("text delta event should contain data");
        assert_eq!(data["conversationId"], "conversation-id");
        assert_eq!(data["messageId"], "message-id");
        assert_eq!(data["delta"], "new text");
        assert!(data.get("content").is_none());
    }

    #[test]
    #[ignore = "manual synthetic RPC payload benchmark"]
    fn benchmark_chat_text_snapshot_and_delta_payloads() {
        const CHUNK: &str = "abcdefgh";
        const ITERATIONS: usize = 10_000;
        const REPEATS: usize = 5;

        let mut snapshot_durations = Vec::with_capacity(REPEATS);
        let mut delta_durations = Vec::with_capacity(REPEATS);
        let mut snapshot_bytes = 0usize;
        let mut delta_bytes = 0usize;

        for _ in 0..REPEATS {
            let mut content = String::new();
            let started = Instant::now();
            let mut serialized_bytes = 0usize;
            for _ in 0..ITERATIONS {
                content.push_str(CHUNK);
                let response = ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                    "conversation-id",
                    "message-id",
                    &content,
                    42,
                ))
                .into_rpc(json!(1));
                serialized_bytes += serde_json::to_vec(&response)
                    .expect("serialize snapshot event")
                    .len();
            }
            snapshot_durations.push(started.elapsed());
            snapshot_bytes = serialized_bytes;

            let started = Instant::now();
            let mut serialized_bytes = 0usize;
            for _ in 0..ITERATIONS {
                let response = ChatStreamEvent::TextAppended(ChatStreamDelta::new(
                    "conversation-id",
                    "message-id",
                    CHUNK,
                    42,
                ))
                .into_rpc(json!(1));
                serialized_bytes += serde_json::to_vec(&response)
                    .expect("serialize delta event")
                    .len();
            }
            delta_durations.push(started.elapsed());
            delta_bytes = serialized_bytes;
        }

        snapshot_durations.sort();
        delta_durations.sort();
        let snapshot_median = snapshot_durations[REPEATS / 2];
        let delta_median = delta_durations[REPEATS / 2];
        assert!(delta_bytes < snapshot_bytes);
        println!(
            "10,000 RPC events: snapshot bytes={snapshot_bytes}, delta bytes={delta_bytes}, \
             snapshot median={snapshot_median:?}, delta median={delta_median:?}, byte reduction={:.1}%",
            (1.0 - delta_bytes as f64 / snapshot_bytes as f64) * 100.0,
        );
    }
}
