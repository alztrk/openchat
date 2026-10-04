use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::provider_schema::ToolActivity;

mod compaction;
mod connections;
mod conversations;
mod memory;
mod models;
mod usage;

pub use compaction::{
    ConversationContextState, PromptUsage, load_conversation_context_state,
    reset_conversation_context, save_compaction_state, save_prompt_usage,
};
pub use connections::{
    connection_id_for_external_user, connection_workspace, create_connection, credential_reference,
    delete_connection, list_connections, reconnect_connection, replace_credential_reference,
    set_connection_auth_status, set_selected_connection, set_selected_workspace,
    set_title_preference, title_preference, update_profile, update_workspace_plan,
    workspace_external_id,
};
pub use conversations::{
    apply_generated_title, conversation_messages, conversation_messages_from_boundary,
    conversation_route, create_title_job, finish_title_job, is_retryable_latest_assistant_message,
    save_assistant_message, save_assistant_tool_checkpoint,
};
pub use memory::{
    ArchiveIndexSettings, ArchiveIndexTool, ArchivedMemoryExcerpt, archive_index_settings,
    prepare_semantic_search, retrieve_archived_memories, search_conversation_archive,
    semantic_search_is_ready, set_archive_tool_included, set_conversation_archive_included,
};
pub use models::{list_fresh_models, list_models, save_models, selected_model};
pub use usage::{latest_usage_snapshot, save_usage_snapshot};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptConnection {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub auth_status: String,
    pub is_selected: bool,
    pub workspaces: Vec<ChatGptWorkspace>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptWorkspace {
    pub id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptModel {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub context_window: Option<i64>,
    pub default_reasoning_level: Option<String>,
    pub reasoning_levels: Vec<String>,
    pub supports_reasoning_summary_parameter: bool,
    pub supports_images: bool,
    pub is_available: bool,
}

pub(crate) fn model_supports_images(model_id: &str) -> bool {
    let normalized = model_id.to_ascii_lowercase();
    normalized.starts_with("gpt-4o")
        || normalized.starts_with("gpt-4.1")
        || normalized.starts_with("gpt-4.5")
        || normalized.starts_with("gpt-4-turbo")
        || normalized.starts_with("gpt-4-vision")
        || normalized.starts_with("gpt-5")
        || normalized.starts_with("o1")
        || normalized.starts_with("o3")
        || normalized.starts_with("o4")
}

#[derive(Clone, Debug)]
pub struct NewChatGptConnection {
    pub id: String,
    pub external_user_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub credential_reference: String,
    pub workspaces: Vec<NewChatGptWorkspace>,
}

#[derive(Clone, Debug)]
pub struct NewChatGptWorkspace {
    pub id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct ConversationRoute {
    pub connection_id: String,
    pub workspace_id: String,
    pub model_id: String,
    pub title_is_automatic: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitlePreference {
    pub connection_id: Option<String>,
    pub workspace_id: Option<String>,
}

pub struct NewTitleJob<'a> {
    pub id: &'a str,
    pub conversation_id: &'a str,
    pub connection_id: &'a str,
    pub workspace_id: &'a str,
    pub model_id: Option<&'a str>,
    pub status: &'a str,
    pub reason_code: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub output_tokens: Option<i64>,
    pub tool_activities: Vec<ToolActivity>,
    pub attachments: Vec<StoredAttachment>,
}

#[derive(Clone, Debug)]
pub struct StoredAttachment {
    pub mime_type: String,
    pub kind: String,
    pub content: Option<Vec<u8>>,
}

impl ToolActivity {
    pub fn has_completed_result(&self) -> bool {
        self.output.is_some()
            && matches!(
                self.status,
                crate::provider_schema::ToolActivityStatus::Completed
                    | crate::provider_schema::ToolActivityStatus::Failed
                    | crate::provider_schema::ToolActivityStatus::Denied
                    | crate::provider_schema::ToolActivityStatus::Cancelled
            )
    }
}

pub struct AssistantMessageWrite<'a> {
    pub conversation_id: &'a str,
    pub message_id: &'a str,
    pub content: &'a str,
    pub status: &'a str,
    pub created_at_unix_ms: i64,
    pub output_tokens: Option<i64>,
    pub elapsed: Option<Duration>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub fetched_at_unix_ms: i64,
    pub freshness: String,
    pub ordinary_usage_allowed: Option<bool>,
    pub reset_credit_count: Option<i64>,
    pub reset_credit_details_state: String,
    pub buckets: Vec<UsageBucket>,
    pub reset_credits: Vec<ResetCredit>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    pub limit_id: String,
    pub used_percent: Option<f64>,
    pub window_seconds: Option<i64>,
    pub reset_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub reset_type: Option<String>,
    pub status: Option<String>,
    pub granted_at_unix_ms: Option<i64>,
    pub expires_at_unix_ms: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
}

pub struct NewUsageSnapshot {
    pub id: String,
    pub connection_id: String,
    pub workspace_id: String,
    pub fetched_at_unix_ms: i64,
    pub freshness: String,
    pub ordinary_usage_allowed: Option<bool>,
    pub reset_credit_count: Option<i64>,
    pub reset_credit_details_state: String,
    pub buckets: Vec<UsageBucket>,
    pub reset_credits: Vec<ResetCredit>,
}
pub(super) fn unix_time_millis() -> rusqlite::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}
