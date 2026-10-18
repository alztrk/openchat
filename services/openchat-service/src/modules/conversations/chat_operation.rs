//! Defines the context assembled for chat sends and title generation.
//!
//! These types pass resolved conversation, model, provider, permission, and
//! cancellation state between RPC orchestration and provider execution.

use std::path::Path;

use serde_json::Value;
use tokio::sync::watch;

use crate::{
    goals::GoalExecution,
    permissions::ToolPermissionBroker,
    protocol::EventSink,
    tools::{ToolPermissionMode, ToolPermissionRules},
};
use crate::{storage::AppStorage, user_question_broker::UserQuestionBroker};

#[derive(Clone)]
/// Selects how the service obtains a conversation title after a chat send.
pub enum TitleGenerationRequest {
    /// Do not request or generate a title.
    Disabled,
    /// Generate a title using the model that handled the conversation.
    Conversation,
    /// Generate a title with an explicitly selected provider and model.
    Model(TitleGenerationModel),
}

#[derive(Clone)]
/// Provider route and credentials used only for title generation.
pub struct TitleGenerationModel {
    /// Provider route identifier.
    pub provider_id: String,
    /// Model identifier on that provider route.
    pub model_id: String,
    /// OAuth connection used by providers that require account authorization.
    pub connection_id: Option<String>,
    /// OAuth workspace associated with the selected connection.
    pub workspace_id: Option<String>,
    /// Saved API-key connection identifier, when the route uses stored credentials.
    pub api_key_connection_id: Option<String>,
    /// Ephemeral API key; this value is zeroized when dropped.
    pub api_key: Option<zeroize::Zeroizing<String>>,
    /// Custom provider configuration for compatible API routes.
    pub custom_provider: Option<crate::openai_compatible::CustomProviderConfig>,
}

/// Fully resolved dependencies and permissions for one chat send operation.
///
/// The context borrows application services and request-owned cancellation state;
/// it is assembled after transport validation and must not be persisted.
pub struct ChatSendContext<'a> {
    /// Correlates emitted events with the initiating RPC request.
    pub request_id: Value,
    /// Identifier for the current execution, used by tools and persistence.
    pub run_id: String,
    /// Optional persisted goal associated with this execution.
    pub goal: Option<GoalExecution>,
    /// Conversation whose history and messages are being processed.
    pub conversation_id: &'a str,
    /// Assistant message excluded when regenerating or editing a branch.
    pub excluded_assistant_message_id: Option<&'a str>,
    /// User-level instructions resolved for this request.
    pub custom_instructions: Option<&'a str>,
    /// Instructions loaded from the active project.
    pub project_instructions: Option<&'a str>,
    /// Validated skill content selected for the active project.
    pub project_skills: Vec<crate::project_skills::ProjectSkill>,
    /// Bounded local index excerpts available to project-aware tools.
    pub project_index_context: Option<String>,
    /// Canonical project root used to scope project tools, if any.
    pub project_root: Option<&'a Path>,
    /// Application data root used by local conversation and tool storage.
    pub data_root: &'a Path,
    /// Local storage adapter used by this operation.
    pub storage: &'a AppStorage,
    /// Permission mode that governs tool execution for this send.
    pub permission_mode: ToolPermissionMode,
    /// Per-tool allow, ask, or deny rules resolved for this request.
    pub tool_permission_rules: ToolPermissionRules,
    /// Optional tool allowlist imposed by the active profile.
    pub profile_tool_allowlist: Option<std::collections::HashSet<String>>,
    #[cfg(windows)]
    /// Validated project MCP configurations for the Windows tool runtime.
    pub mcp_configs: Vec<crate::tools::mcp::StdioServerConfig>,
    #[cfg(windows)]
    /// Connected MCP registry, when approval and initialization already succeeded.
    pub mcp_registry: Option<std::sync::Arc<crate::tools::mcp::McpRegistry>>,
    /// Broker that requests permission before protected tools execute.
    pub permission_broker: &'a ToolPermissionBroker,
    /// Broker that pauses and resumes runs for interactive user answers.
    pub user_question_broker: &'a UserQuestionBroker,
    /// Cancellation signal shared with provider and tool operations.
    pub cancellation: &'a mut watch::Receiver<bool>,
    /// Event sink for progress and assistant output.
    pub events: EventSink,
    /// Optional context-window override applied to the active request.
    pub context_limit_override: Option<i64>,
    /// Title-generation route selected for this conversation.
    pub title_generation: TitleGenerationRequest,
}
