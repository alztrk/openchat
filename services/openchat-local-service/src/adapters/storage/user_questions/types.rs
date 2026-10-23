//! Persists bounded user-question groups and validates their revisioned answers.
//!
//! Stored group, item, choice, depth, and answer limits are rechecked when data is read so
//! corrupted rows cannot expand unboundedly into RPC responses.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_CHECKPOINT_BYTES: usize = 256 * 1024;
pub(crate) const MAX_QUESTION_GROUP_BYTES: usize = 128 * 1024;
pub(crate) const MAX_QUESTION_ITEM_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ANSWER_BYTES: usize = 128 * 1024;
pub(crate) const MAX_QUESTION_ITEMS: usize = 32;
pub(crate) const MAX_OPTION_COUNT: usize = 128;
pub(crate) const MAX_IDENTIFIER_BYTES: usize = 128;
pub(crate) const MAX_TEXT_BYTES: usize = 16 * 1024;
pub(crate) const MAX_DEPTH: usize = 64;

#[derive(Debug)]
pub(crate) enum UserQuestionError {
    Database(rusqlite::Error),
    InvalidInput(String),
    CorruptStoredData(String),
    NotFound(&'static str),
    Conflict(String),
    StaleRevision { expected: i64, actual: i64 },
}

impl fmt::Display for UserQuestionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::InvalidInput(message) => write!(formatter, "invalid input: {message}"),
            Self::CorruptStoredData(message) => write!(formatter, "corrupt stored data: {message}"),
            Self::NotFound(entity) => write!(formatter, "{entity} was not found"),
            Self::Conflict(message) => write!(formatter, "conflict: {message}"),
            Self::StaleRevision { expected, actual } => {
                write!(
                    formatter,
                    "stale revision: expected {expected}, current {actual}"
                )
            }
        }
    }
}

impl Error for UserQuestionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for UserQuestionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentRunStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
    Interrupted,
    Failed,
}

impl AgentRunStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }

    pub(super) fn parse(value: String) -> Result<Self, UserQuestionError> {
        match value.as_str() {
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            "failed" => Ok(Self::Failed),
            _ => Err(UserQuestionError::CorruptStoredData(
                "unknown agent run status".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QuestionGroupStatus {
    Pending,
    Answered,
    Cancelled,
    Expired,
}

impl QuestionGroupStatus {
    pub(super) fn parse(value: String) -> Result<Self, UserQuestionError> {
        match value.as_str() {
            "pending" => Ok(Self::Pending),
            "answered" => Ok(Self::Answered),
            "cancelled" => Ok(Self::Cancelled),
            "expired" => Ok(Self::Expired),
            _ => Err(UserQuestionError::CorruptStoredData(
                "unknown question group status".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QuestionKind {
    Choice,
    MultiChoice,
    Text,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QuestionOption {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionItem {
    pub(crate) id: String,
    pub(crate) kind: QuestionKind,
    pub(crate) title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) options: Vec<QuestionOption>,
    #[serde(default)]
    pub(crate) required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_length: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(rename_all_fields = "camelCase", deny_unknown_fields)]
pub(crate) enum QuestionAnswerValue {
    Choice { option_id: String },
    MultiChoice { option_ids: Vec<String> },
    Text { text: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionAnswer {
    pub(crate) question_id: String,
    pub(crate) value: QuestionAnswerValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewAgentRun {
    pub(crate) run_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewQuestionGroup {
    pub(crate) group_id: String,
    pub(crate) run_id: String,
    pub(crate) sequence: i64,
    pub(crate) questions: Vec<QuestionItem>,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentRun {
    pub(crate) run_id: String,
    pub(crate) conversation_id: String,
    pub(crate) status: AgentRunStatus,
    pub(crate) checkpoint: Option<Value>,
    pub(crate) checkpoint_revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentRunSummary {
    pub(crate) run_id: String,
    pub(crate) conversation_id: String,
    pub(crate) conversation_title: Option<String>,
    pub(crate) status: AgentRunStatus,
    pub(crate) updated_at_unix_ms: i64,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) run_kind: Option<String>,
    pub(crate) parent_run_id: Option<String>,
    pub(crate) objective: Option<String>,
    pub(crate) progress_phase: Option<String>,
    pub(crate) progress_tool_name: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingQuestionGroup {
    pub(crate) group_id: String,
    pub(crate) run_id: String,
    pub(crate) conversation_id: String,
    pub(crate) checkpoint: Option<Value>,
    pub(crate) sequence: i64,
    pub(crate) status: QuestionGroupStatus,
    pub(crate) revision: i64,
    pub(crate) questions: Vec<QuestionItem>,
    pub(crate) answers: Option<Vec<QuestionAnswer>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SubmittedAnswers {
    pub(crate) group: PendingQuestionGroup,
    pub(crate) idempotent: bool,
}
