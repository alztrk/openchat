//! Defines and re-exports persistent ChatGPT and conversation storage operations.
//!
//! Connection metadata, conversation messages, model catalogs, usage, context
//! checkpoints, and archived memory share the service's local SQLite database.

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
    save_prompt_usage_on_connection,
};
pub use connections::{
    complete_credential_deletion, connection_id_for_external_user, connection_workspace,
    create_connection, credential_reference, delete_connection, list_connections,
    pending_credential_deletions, reconnect_connection, replace_credential_reference,
    set_connection_auth_status, set_selected_connection, set_selected_workspace,
    set_title_preference, title_preference, update_profile, update_workspace_plan,
    workspace_external_id,
};
#[allow(unused_imports)]
pub use conversations::{
    apply_generated_title, conversation_messages, conversation_messages_from_boundary,
    conversation_route, conversation_title_is_automatic, create_title_job, finish_title_job,
    is_custom_title, is_default_title, is_retryable_latest_assistant_message,
    save_assistant_message, save_assistant_message_on_connection, save_assistant_tool_checkpoint,
    should_generate_automatic_title, should_generate_automatic_title_on_connection,
};
pub use memory::{
    ArchiveIndexSettings, ArchiveIndexTool, ArchivedMemoryExcerpt, HistorySearchFilterOptions,
    HistorySearchFilters, ProjectHistoryExcerpt, history_search_filter_options,
    prepare_semantic_search, retrieve_archived_memories, search_chat_history,
    search_conversation_archive, search_project_history, semantic_search_is_ready,
    set_archive_tool_included, set_conversation_archive_included, turkish_search,
};
pub use models::{list_fresh_models, list_models, save_models, selected_model};
pub use usage::{latest_usage_snapshot, save_usage_snapshot};

/// Retries a storage write only for bounded, recognized transient database failures.
pub(crate) async fn retry_storage_operation<T, F>(
    storage: &crate::storage::AppStorage,
    operation: &'static str,
    mut write: F,
) -> rusqlite::Result<T>
where
    F: FnMut() -> rusqlite::Result<T> + Send,
{
    let mut attempt = 0;
    loop {
        match write() {
            Ok(value) => return Ok(value),
            Err(error) => {
                attempt += 1;
                let code = crate::storage::sqlite_error_code(&error);
                let retry_delay = storage_operation_retry_delay(code, attempt);
                let event = if retry_delay.is_some() {
                    "storage_retry_scheduled"
                } else {
                    "storage_failed"
                };
                let delay_ms = retry_delay.map(|delay| delay.as_millis());
                if storage
                    .log_chatgpt_storage_diagnostic(event, operation, code, attempt, delay_ms)
                    .is_err()
                {
                    eprintln!("chatgpt_storage_diagnostic_write_failed");
                }
                if let Some(delay) = retry_delay {
                    tokio::time::sleep(delay).await;
                } else {
                    return Err(error);
                }
            }
        }
    }
}

/// Saves an assistant message with bounded retries for transient storage failures.
pub(crate) async fn save_assistant_message_with_retry(
    storage: &crate::storage::AppStorage,
    message: AssistantMessageWrite<'_>,
    operation: &'static str,
) -> rusqlite::Result<()> {
    retry_storage_operation(storage, operation, || {
        save_assistant_message(storage, message)
    })
    .await
}

/// Saves an assistant tool checkpoint with bounded retries for transient storage failures.
pub(crate) async fn save_assistant_tool_checkpoint_with_retry(
    storage: &crate::storage::AppStorage,
    snapshot: &crate::provider_schema::ChatStreamSnapshot,
    activities: &[ToolActivity],
    operation: &'static str,
) -> rusqlite::Result<()> {
    retry_storage_operation(storage, operation, || {
        save_assistant_tool_checkpoint(storage, snapshot, activities)
    })
    .await
}

fn storage_operation_retry_delay(code: &str, attempt: usize) -> Option<Duration> {
    const SQLITE_BUSY_RETRY_DELAYS: [Duration; 3] = [
        Duration::from_millis(150),
        Duration::from_millis(400),
        Duration::from_millis(900),
    ];

    match code {
        "sqlite_busy" | "sqlite_busy_snapshot" | "sqlite_locked" | "sqlite_5" | "sqlite_517" => {
            SQLITE_BUSY_RETRY_DELAYS.get(attempt - 1).copied()
        }
        "sqlite_busy_timeout"
        | "sqlite_io_error"
        | "database_queue_full"
        | "database_queue_wait_timeout"
        | "storage_broker_timeout"
            if attempt == 1 =>
        {
            Some(Duration::from_millis(500))
        }
        _ => None,
    }
}

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
    pub supports_fast_mode: bool,
    pub is_available: bool,
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
    #[allow(dead_code)]
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
    pub id: String,
    pub name: String,
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

#[derive(Clone, Copy)]
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
