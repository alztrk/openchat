use std::path::Path;

use serde_json::Value;
use tokio::sync::watch;

use crate::{permissions::ToolPermissionBroker, protocol::EventSink, tools::ToolPermissionMode};
use crate::{storage::AppStorage, user_question_broker::UserQuestionBroker};

pub struct ChatSendContext<'a> {
    pub request_id: Value,
    pub run_id: String,
    pub conversation_id: &'a str,
    pub excluded_assistant_message_id: Option<&'a str>,
    pub custom_instructions: Option<&'a str>,
    pub project_root: Option<&'a Path>,
    pub data_root: &'a Path,
    pub storage: &'a AppStorage,
    pub permission_mode: ToolPermissionMode,
    pub permission_broker: &'a ToolPermissionBroker,
    pub user_question_broker: &'a UserQuestionBroker,
    pub cancellation: &'a mut watch::Receiver<bool>,
    pub events: EventSink,
}
