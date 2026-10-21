//! Shared request, tool, and stream types used across provider adapters.
//!
//! Provider-specific payloads are translated into these internal contracts
//! before transport events or persisted conversation data are produced.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::protocol::ServiceError;

mod rpc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Role assigned to a message in provider conversation history.
pub enum MessageRole {
    /// Message written by the user.
    User,
    /// Message written by the assistant.
    Assistant,
    /// Output returned from a tool call.
    Tool,
}

impl MessageRole {
    /// Parses a persisted role or returns an invalid-history service error.
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

    /// Returns the stable lowercase role name used by provider payloads.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug)]
/// Provider-neutral conversation message with optional image and tool data.
pub struct ProviderMessage {
    /// Message role.
    pub role: MessageRole,
    /// Text content sent to the provider.
    pub content: String,
    /// Encoded image data associated with a user message.
    pub images: Vec<String>,
    /// Tool calls emitted by an assistant message.
    pub tool_calls: Vec<ToolCall>,
    /// Tool-call identifier for a tool response message.
    pub tool_call_id: Option<String>,
}

impl ProviderMessage {
    /// Creates a plain text history message with no image or tool metadata.
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
/// Function schema made available to a provider for one chat request.
pub struct ToolDefinition {
    /// Stable function name.
    pub name: String,
    /// User-facing description included in the provider schema.
    pub description: String,
    /// JSON Schema describing accepted arguments.
    pub parameters: Value,
}

#[derive(Clone, Debug)]
/// Provider-neutral request assembled for a chat completion.
pub struct ProviderChatRequest {
    /// Model identifier selected for the request.
    pub model: String,
    /// System or developer instructions sent before conversation messages.
    pub instructions: String,
    /// Ordered conversation history.
    pub messages: Vec<ProviderMessage>,
    /// Most recent persisted message identifier, when available.
    pub last_message_id: Option<String>,
    /// Tools permitted for this request.
    pub tools: Vec<ToolDefinition>,
    /// Requested reasoning effort, when supported.
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Debug)]
/// One assistant-issued provider tool invocation.
pub struct ToolCall {
    /// Provider-generated call identifier.
    pub id: String,
    /// Internal tool name.
    pub name: String,
    /// Parsed tool argument object.
    pub arguments: Value,
}

#[derive(Clone, Debug)]
/// Result paired with a previously issued tool call.
pub struct ToolResult {
    /// Identifier of the invocation being answered.
    pub call_id: String,
    /// Structured tool result returned to the provider.
    pub output: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Lifecycle state of one assistant tool activity.
pub enum ToolActivityStatus {
    /// Waiting for user approval before execution.
    AwaitingApproval,
    /// Waiting for the user to answer a tool question.
    WaitingForUser,
    /// Tool execution is in progress.
    Running,
    /// Tool execution completed successfully.
    Completed,
    /// Tool execution failed.
    Failed,
    /// The user or permission rules denied execution.
    Denied,
    /// Execution was cancelled.
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Persistable and streamable state for one provider tool call.
pub struct ToolActivity {
    /// Provider-generated call identifier.
    pub call_id: String,
    /// Internal tool name.
    pub name: String,
    /// Parsed arguments submitted for execution.
    pub arguments: Value,
    /// Provider tool-round identifier, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_id: Option<String>,
    /// Assistant content byte offset before the tool round, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_text_before_byte_offset: Option<usize>,
    /// Optional path or target shown for this tool activity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    /// Structured output, present after completion, denial, failure, or cancellation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    /// File changes recorded for this activity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_changes: Vec<crate::file_changes::FileChangeSummary>,
    /// Safe display message when file-change details could not be collected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_changes_error: Option<String>,
    /// Current tool activity lifecycle state.
    pub status: ToolActivityStatus,
}

impl ToolActivity {
    /// Creates a tool activity that awaits explicit user approval.
    pub fn awaiting_approval(call: &ToolCall, target_path: String) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: None,
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::AwaitingApproval,
        }
    }

    /// Creates a tool activity as it begins execution.
    pub fn running(call: &ToolCall, target_path: Option<String>) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path,
            output: None,
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::Running,
        }
    }

    /// Creates a tool activity that is paused for an interactive answer.
    pub fn waiting_for_user(call: &ToolCall) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: None,
            output: None,
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::WaitingForUser,
        }
    }

    /// Creates a terminal activity, deriving success or failure from its output.
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
            file_changes: Vec::new(),
            file_changes_error: None,
            status,
        }
    }

    /// Creates a terminal activity for a denied tool invocation.
    pub fn denied(call: &ToolCall, target_path: String, output: Value) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: Some(output),
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::Denied,
        }
    }

    /// Creates a terminal activity for a cancelled tool invocation.
    pub fn cancelled(call: &ToolCall, target_path: String, output: Value) -> Self {
        Self {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            round_id: None,
            assistant_text_before_byte_offset: None,
            target_path: Some(target_path),
            output: Some(output),
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::Cancelled,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// One reasoning summary emitted during a provider response.
pub struct ReasoningSummary {
    /// Stable provider or service identifier for this summary.
    pub id: String,
    /// Summary text accumulated so far.
    pub content: String,
    /// Elapsed processing time reported in microseconds.
    pub elapsed_microseconds: i64,
    /// Whether the provider has completed this summary.
    pub is_complete: bool,
}

#[derive(Clone, Debug)]
/// Full assistant message state sent at stream start and checkpoints.
pub struct ChatStreamSnapshot {
    /// Conversation that owns this message.
    pub conversation_id: String,
    /// Stable identifier of the assistant message.
    pub message_id: String,
    /// Complete assistant content at this checkpoint.
    pub content: String,
    /// Message creation time in Unix milliseconds.
    pub created_at_unix_ms: i64,
}

impl ChatStreamSnapshot {
    /// Creates a full snapshot, copying the message identifiers and content.
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

/// Carries a text fragment appended to an assistant message.
#[derive(Clone, Debug)]
pub struct ChatStreamDelta {
    /// Conversation that owns the message.
    pub conversation_id: String,
    /// Stable identifier of the assistant message being updated.
    pub message_id: String,
    /// New text to append to the message content.
    pub delta: String,
    /// Message creation time in Unix milliseconds.
    pub created_at_unix_ms: i64,
}

impl ChatStreamDelta {
    /// Creates a text update without copying the accumulated assistant response.
    pub fn new(
        conversation_id: &str,
        message_id: &str,
        delta: &str,
        created_at_unix_ms: i64,
    ) -> Self {
        Self {
            conversation_id: conversation_id.to_owned(),
            message_id: message_id.to_owned(),
            delta: delta.to_owned(),
            created_at_unix_ms,
        }
    }
}

/// Carries full state snapshots and append-only text updates for a chat stream.
///
/// Tool activity stays inline because the event is immediately converted into RPC data. Boxing it
/// would add a heap allocation to each tool status update without changing the serialized payload.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
/// Event variants emitted while a provider response is processed.
pub enum ChatStreamEvent {
    /// Initial full message state.
    Started(ChatStreamSnapshot),
    /// Full message checkpoint after a provider update.
    TextUpdated(ChatStreamSnapshot),
    /// Append-only text fragment for the message.
    TextAppended(ChatStreamDelta),
    /// Tool activity state paired with its full message snapshot.
    ToolActivityUpdated {
        snapshot: ChatStreamSnapshot,
        activity: ToolActivity,
    },
    /// Reasoning summaries paired with their full message snapshot.
    ReasoningSummariesUpdated {
        snapshot: ChatStreamSnapshot,
        summaries: Vec<ReasoningSummary>,
    },
}
