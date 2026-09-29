use serde::Serialize;
use serde_json::{Value, json};

use crate::protocol::Response;

use super::{ChatStreamEvent, ChatStreamSnapshot, ReasoningSummary, ToolActivity};

impl ChatStreamEvent {
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
