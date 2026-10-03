use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::protocol::ServiceError;

mod rpc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub fn from_history(value: &str) -> Result<Self, ServiceError> {
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            _ => Err(ServiceError::new(
                "conversation_history_invalid",
                "The conversation contains an unsupported message role.",
                false,
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: String,
    pub images: Vec<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
}

impl ProviderMessage {
    pub fn from_history(role: &str, content: &str) -> Result<Self, ServiceError> {
        Ok(Self {
            role: MessageRole::from_history(role)?,
            content: content.to_owned(),
            images: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Clone, Debug)]
pub struct ProviderChatRequest {
    pub model: String,
    pub instructions: String,
    pub messages: Vec<ProviderMessage>,
    pub last_message_id: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug)]
pub struct ToolResult {
    pub call_id: String,
    pub output: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolActivityStatus {
    AwaitingApproval,
    WaitingForUser,
    Running,
    Completed,
    Failed,
    Denied,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActivity {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_text_before_byte_offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    pub status: ToolActivityStatus,
}

impl ToolActivity {
    pub fn awaiting_approval(call: &ToolCall, target_path: String) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: None,
            status: ToolActivityStatus::AwaitingApproval,
        }
    }

    pub fn running(call: &ToolCall, target_path: Option<String>) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path,
            output: None,
            status: ToolActivityStatus::Running,
        }
    }

    pub fn waiting_for_user(call: &ToolCall) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: None,
            output: None,
            status: ToolActivityStatus::WaitingForUser,
        }
    }

    pub fn finished(call: &ToolCall, target_path: Option<String>, output: Value) -> Self {
        let status = if output.get("error").is_some() {
            ToolActivityStatus::Failed
        } else {
            ToolActivityStatus::Completed
        };
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path,
            output: Some(output),
            status,
        }
    }

    pub fn denied(call: &ToolCall, target_path: String, output: Value) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: Some(output),
            status: ToolActivityStatus::Denied,
        }
    }

    pub fn cancelled(call: &ToolCall, target_path: String, output: Value) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: Some(output),
            status: ToolActivityStatus::Cancelled,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummary {
    pub id: String,
    pub content: String,
    pub elapsed_microseconds: i64,
    pub is_complete: bool,
}

#[derive(Clone, Debug)]
pub struct ChatStreamSnapshot {
    pub conversation_id: String,
    pub message_id: String,
    pub content: String,
    pub created_at_unix_ms: i64,
}

impl ChatStreamSnapshot {
    pub fn new(
        conversation_id: &str,
        message_id: &str,
        content: &str,
        created_at_unix_ms: i64,
    ) -> Self {
        Self {
            conversation_id: conversation_id.to_owned(),
            message_id: message_id.to_owned(),
            content: content.to_owned(),
            created_at_unix_ms,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChatStreamEvent {
    Started(ChatStreamSnapshot),
    TextUpdated(ChatStreamSnapshot),
    ToolActivityUpdated {
        snapshot: ChatStreamSnapshot,
        activity: ToolActivity,
    },
    ReasoningSummariesUpdated {
        snapshot: ChatStreamSnapshot,
        summaries: Vec<ReasoningSummary>,
    },
}
