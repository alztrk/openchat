use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_CHECKPOINT_BYTES: usize = 256 * 1024;
pub(crate) const MAX_QUESTION_GROUP_BYTES: usize = 128 * 1024;
pub(crate) const MAX_QUESTION_ITEM_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ANSWER_BYTES: usize = 128 * 1024;
const MAX_QUESTION_ITEMS: usize = 32;
const MAX_OPTION_COUNT: usize = 128;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 16 * 1024;
const MAX_DEPTH: usize = 64;

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
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }

    fn parse(value: String) -> Result<Self, UserQuestionError> {
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
    fn parse(value: String) -> Result<Self, UserQuestionError> {
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
    pub(crate) conversation_id: String,
    pub(crate) status: AgentRunStatus,
    pub(crate) checkpoint: Option<Value>,
    pub(crate) checkpoint_revision: i64,
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

pub(crate) fn create_run(
    connection: &Connection,
    new_run: &NewAgentRun,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(&new_run.run_id, "run id")?;
    validate_identifier(&new_run.conversation_id, "conversation id")?;
    let now = unix_time_millis()?;
    let result = connection.execute(
        "INSERT INTO agent_runs (
            id, conversation_id, status, created_at_unix_ms, updated_at_unix_ms
        ) VALUES (?1, ?2, 'running', ?3, ?3)",
        params![new_run.run_id, new_run.conversation_id, now],
    );
    match result {
        Ok(_) => load_run(connection, &new_run.run_id),
        Err(error) if is_constraint_error(&error) => Err(UserQuestionError::Conflict(
            "the run id is already in use or the conversation does not exist".to_owned(),
        )),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn load_run_for_conversation(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let run = load_run(connection, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    Ok(run)
}

pub(crate) fn next_question_sequence(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<i64, UserQuestionError> {
    let run = load_run_for_conversation(connection, conversation_id, run_id)?;
    if run.status != AgentRunStatus::Running {
        return Err(invalid_run_transition(run.status, "ask a question"));
    }
    let maximum = connection.query_row(
        "SELECT MAX(sequence) FROM pending_question_groups WHERE run_id = ?1",
        [run_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    maximum
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| UserQuestionError::InvalidInput("question sequence overflow".to_owned()))
}

pub(crate) fn save_checkpoint(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
    expected_revision: i64,
    checkpoint: &Value,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    validate_non_negative(expected_revision, "checkpoint revision")?;
    let checkpoint_json = validate_checkpoint(checkpoint)?;
    let now = unix_time_millis()?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    if run.checkpoint_revision != expected_revision {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }
    if !matches!(
        run.status,
        AgentRunStatus::Running | AgentRunStatus::Paused | AgentRunStatus::Interrupted
    ) {
        return Err(invalid_run_transition(run.status, "save checkpoint"));
    }
    let changed = transaction.execute(
        "UPDATE agent_runs
         SET checkpoint_json = ?1,
             checkpoint_revision = checkpoint_revision + 1,
             checkpoint_updated_at_unix_ms = ?2,
             updated_at_unix_ms = ?2
         WHERE id = ?3
           AND checkpoint_revision = ?4
           AND status IN ('running', 'paused', 'interrupted')",
        params![checkpoint_json, now, run_id, expected_revision],
    )?;
    if changed == 0 {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }
    transaction.commit()?;
    load_run(connection, run_id)
}

pub(crate) fn pause_with_questions(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
    expected_revision: i64,
    checkpoint: &Value,
    group: &NewQuestionGroup,
) -> Result<PendingQuestionGroup, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    validate_non_negative(expected_revision, "checkpoint revision")?;
    validate_question_group(group)?;
    if group.run_id != run_id {
        return Err(UserQuestionError::InvalidInput(
            "question group belongs to a different run".to_owned(),
        ));
    }

    let checkpoint_json = validate_checkpoint(checkpoint)?;
    let now = unix_time_millis()?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    if run.status != AgentRunStatus::Running {
        return Err(invalid_run_transition(run.status, "pause"));
    }
    if run.checkpoint_revision != expected_revision {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }

    let maximum_sequence = transaction.query_row(
        "SELECT MAX(sequence) FROM pending_question_groups WHERE run_id = ?1",
        [run_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    if maximum_sequence.is_some_and(|value| group.sequence <= value) {
        return Err(UserQuestionError::Conflict(
            "question group sequence must increase for the run".to_owned(),
        ));
    }

    transaction.execute(
        "UPDATE agent_runs
         SET status = 'paused',
             checkpoint_json = ?1,
             checkpoint_revision = checkpoint_revision + 1,
             checkpoint_updated_at_unix_ms = ?2,
             updated_at_unix_ms = ?2
         WHERE id = ?3 AND checkpoint_revision = ?4 AND status = 'running'",
        params![checkpoint_json, now, run_id, expected_revision],
    )?;

    insert_question_group(&transaction, group, now)?;
    transaction.commit()?;
    load_question_group(connection, &group.group_id)
}

#[cfg(test)]
pub(crate) fn list_pending_groups(
    connection: &Connection,
    conversation_id: Option<&str>,
) -> Result<Vec<PendingQuestionGroup>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Deferred)?;
    let mut group_ids = Vec::new();
    if let Some(conversation_id) = conversation_id {
        let mut statement = transaction.prepare(
            "SELECT groups.id
             FROM pending_question_groups AS groups
             INNER JOIN agent_runs AS runs ON runs.id = groups.run_id
             WHERE groups.status = 'pending' AND runs.conversation_id = ?1
             ORDER BY groups.created_at_unix_ms, groups.sequence, groups.id",
        )?;
        let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            group_ids.push(row?);
        }
    } else {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM pending_question_groups
             WHERE status = 'pending'
             ORDER BY created_at_unix_ms, sequence, id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            group_ids.push(row?);
        }
    }

    let groups = group_ids
        .into_iter()
        .map(|group_id| load_question_group(&transaction, &group_id))
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit()?;
    Ok(groups)
}

pub(crate) fn list_recoverable_question_groups(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Vec<PendingQuestionGroup>, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Deferred)?;
    let mut group_ids = Vec::new();
    let mut statement = transaction.prepare(
        "SELECT groups.id
         FROM pending_question_groups AS groups
         INNER JOIN agent_runs AS runs ON runs.id = groups.run_id
         WHERE runs.conversation_id = ?1
           AND (
             groups.status = 'pending'
             OR (
               groups.status = 'answered'
               AND runs.status IN ('paused', 'interrupted')
               AND NOT EXISTS (
                 SELECT 1
                 FROM pending_question_groups AS newer
                 WHERE newer.run_id = groups.run_id
                   AND newer.sequence > groups.sequence
                   AND newer.status IN ('pending', 'answered')
               )
             )
           )
         ORDER BY groups.created_at_unix_ms, groups.sequence, groups.id",
    )?;
    let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
    for row in rows {
        group_ids.push(row?);
    }
    drop(statement);
    let groups = group_ids
        .into_iter()
        .map(|group_id| load_question_group(&transaction, &group_id))
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit()?;
    Ok(groups)
}

pub(crate) fn submit_answers(
    connection: &Connection,
    conversation_id: &str,
    group_id: &str,
    expected_revision: i64,
    answers: &[QuestionAnswer],
) -> Result<SubmittedAnswers, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(group_id, "question group id")?;
    validate_non_negative(expected_revision, "question group revision")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let group = load_question_group(&transaction, group_id)?;
    ensure_group_conversation(&group, conversation_id)?;
    let canonical_answers = validate_answers(&group.questions, answers)?;

    match group.status {
        QuestionGroupStatus::Answered => {
            let stored_answers = group.answers.as_ref().ok_or_else(|| {
                UserQuestionError::CorruptStoredData(
                    "answered question group has no answers".to_owned(),
                )
            })?;
            let stored_canonical_answers = validate_answers(&group.questions, stored_answers)?;
            if stored_canonical_answers == canonical_answers {
                transaction.commit()?;
                return Ok(SubmittedAnswers {
                    group,
                    idempotent: true,
                });
            }
            Err(UserQuestionError::Conflict(
                "a different answer was already submitted for this question group".to_owned(),
            ))
        }
        QuestionGroupStatus::Cancelled | QuestionGroupStatus::Expired => {
            Err(UserQuestionError::Conflict(
                "the question group is no longer accepting answers".to_owned(),
            ))
        }
        QuestionGroupStatus::Pending => {
            if group.revision != expected_revision {
                return Err(UserQuestionError::StaleRevision {
                    expected: expected_revision,
                    actual: group.revision,
                });
            }
            let now = unix_time_millis()?;
            let changed = transaction.execute(
                "UPDATE pending_question_groups
                 SET status = 'answered',
                     revision = revision + 1,
                     answers_json = ?1,
                     answered_at_unix_ms = ?2,
                     updated_at_unix_ms = ?2
                 WHERE id = ?3 AND status = 'pending' AND revision = ?4",
                params![canonical_answers, now, group_id, expected_revision],
            )?;
            if changed != 1 {
                return Err(UserQuestionError::StaleRevision {
                    expected: expected_revision,
                    actual: group.revision,
                });
            }
            transaction.commit()?;
            Ok(SubmittedAnswers {
                group: load_question_group(connection, group_id)?,
                idempotent: false,
            })
        }
    }
}

pub(crate) fn mark_run_resumed(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Running => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Paused | AgentRunStatus::Interrupted => {
            let has_pending = transaction.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM pending_question_groups
                    WHERE run_id = ?1 AND status = 'pending'
                )",
                [run_id],
                |row| row.get::<_, bool>(0),
            )?;
            if has_pending {
                return Err(UserQuestionError::Conflict(
                    "the run still has unanswered questions".to_owned(),
                ));
            }
            let now = unix_time_millis()?;
            transaction.execute(
                "UPDATE agent_runs SET status = 'running', updated_at_unix_ms = ?1
                 WHERE id = ?2",
                params![now, run_id],
            )?;
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "resume")),
    }
}

pub(crate) fn claim_run_resume(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    if !matches!(
        run.status,
        AgentRunStatus::Paused | AgentRunStatus::Interrupted
    ) {
        return Err(invalid_run_transition(run.status, "claim resume"));
    }
    let has_pending = transaction.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM pending_question_groups
            WHERE run_id = ?1 AND status = 'pending'
        )",
        [run_id],
        |row| row.get::<_, bool>(0),
    )?;
    if has_pending {
        return Err(UserQuestionError::Conflict(
            "the run still has unanswered questions".to_owned(),
        ));
    }
    let now = unix_time_millis()?;
    let changed = transaction.execute(
        "UPDATE agent_runs SET status = 'running', updated_at_unix_ms = ?1
         WHERE id = ?2 AND conversation_id = ?3 AND status IN ('paused', 'interrupted')",
        params![now, run_id, conversation_id],
    )?;
    if changed != 1 {
        return Err(UserQuestionError::Conflict(
            "the run was already resumed".to_owned(),
        ));
    }
    transaction.commit()?;
    load_run(connection, run_id)
}

pub(crate) fn mark_run_completed(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Completed => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Running => {
            let has_pending = transaction.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM pending_question_groups
                    WHERE run_id = ?1 AND status = 'pending'
                )",
                [run_id],
                |row| row.get::<_, bool>(0),
            )?;
            if has_pending {
                return Err(UserQuestionError::Conflict(
                    "the run still has unanswered questions".to_owned(),
                ));
            }
            let now = unix_time_millis()?;
            transaction.execute(
                "UPDATE agent_runs
                 SET status = 'completed', finished_at_unix_ms = ?1, updated_at_unix_ms = ?1
                 WHERE id = ?2",
                params![now, run_id],
            )?;
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "complete")),
    }
}

pub(crate) fn mark_run_cancelled(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Cancelled => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Running | AgentRunStatus::Paused | AgentRunStatus::Interrupted => {
            let has_pending = transaction.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM pending_question_groups
                    WHERE run_id = ?1 AND status = 'pending'
                )",
                [run_id],
                |row| row.get::<_, bool>(0),
            )?;
            let now = unix_time_millis()?;
            if has_pending {
                transaction.execute(
                    "UPDATE pending_question_groups
                     SET status = 'cancelled', revision = revision + 1, updated_at_unix_ms = ?1
                     WHERE run_id = ?2 AND status = 'pending'",
                    params![now, run_id],
                )?;
                transaction.execute(
                    "UPDATE agent_runs
                     SET status = 'cancelled', finished_at_unix_ms = ?1, updated_at_unix_ms = ?1
                     WHERE id = ?2",
                    params![now, run_id],
                )?;
            } else {
                let latest_group_status = transaction
                    .query_row(
                        "SELECT status
                         FROM pending_question_groups
                         WHERE run_id = ?1
                         ORDER BY sequence DESC, id DESC
                         LIMIT 1",
                        [run_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if latest_group_status.as_deref() == Some("answered") {
                    transaction.execute(
                        "UPDATE agent_runs
                         SET status = 'interrupted', updated_at_unix_ms = ?1
                         WHERE id = ?2",
                        params![now, run_id],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE agent_runs
                         SET status = 'cancelled', finished_at_unix_ms = ?1, updated_at_unix_ms = ?1
                         WHERE id = ?2",
                        params![now, run_id],
                    )?;
                }
            }
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "cancel")),
    }
}

pub(crate) fn mark_run_interrupted(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Interrupted => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Running | AgentRunStatus::Paused => {
            let now = unix_time_millis()?;
            transaction.execute(
                "UPDATE agent_runs SET status = 'interrupted', updated_at_unix_ms = ?1
                 WHERE id = ?2",
                params![now, run_id],
            )?;
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "interrupt")),
    }
}

pub(crate) fn mark_run_failed(
    connection: &Connection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Failed => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Running | AgentRunStatus::Paused | AgentRunStatus::Interrupted => {
            let now = unix_time_millis()?;
            transaction.execute(
                "UPDATE pending_question_groups
                 SET status = 'cancelled', revision = revision + 1, updated_at_unix_ms = ?1
                 WHERE run_id = ?2 AND status = 'pending'",
                params![now, run_id],
            )?;
            transaction.execute(
                "UPDATE agent_runs
                 SET status = 'failed', finished_at_unix_ms = ?1, updated_at_unix_ms = ?1
                 WHERE id = ?2",
                params![now, run_id],
            )?;
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "fail")),
    }
}

/// Marks all running runs as interrupted during service startup recovery.
pub(crate) fn interrupt_running_runs_after_restart(
    connection: &Connection,
    conversation_id: Option<&str>,
) -> Result<Vec<AgentRun>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let mut run_ids = Vec::new();
    if let Some(conversation_id) = conversation_id {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM agent_runs
             WHERE status = 'running' AND conversation_id = ?1
             ORDER BY updated_at_unix_ms, id",
        )?;
        let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            run_ids.push(row?);
        }
    } else {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM agent_runs
             WHERE status = 'running'
             ORDER BY updated_at_unix_ms, id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            run_ids.push(row?);
        }
    }
    if !run_ids.is_empty() {
        let now = unix_time_millis()?;
        for run_id in &run_ids {
            transaction.execute(
                "UPDATE agent_runs
                 SET status = 'interrupted', updated_at_unix_ms = ?1
                 WHERE id = ?2 AND status = 'running'",
                params![now, run_id],
            )?;
        }
    }
    transaction.commit()?;
    run_ids
        .into_iter()
        .map(|run_id| load_run(connection, &run_id))
        .collect()
}

#[cfg(test)]
pub(crate) fn list_recoverable_runs(
    connection: &Connection,
    conversation_id: Option<&str>,
) -> Result<Vec<AgentRun>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Deferred)?;
    let mut run_ids = Vec::new();
    if let Some(conversation_id) = conversation_id {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM agent_runs
             WHERE status IN ('paused', 'interrupted') AND conversation_id = ?1
             ORDER BY updated_at_unix_ms, id",
        )?;
        let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            run_ids.push(row?);
        }
    } else {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM agent_runs
             WHERE status IN ('paused', 'interrupted')
             ORDER BY updated_at_unix_ms, id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            run_ids.push(row?);
        }
    }
    let runs = run_ids
        .into_iter()
        .map(|run_id| load_run(&transaction, &run_id))
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit()?;
    Ok(runs)
}

fn insert_question_group(
    transaction: &Transaction<'_>,
    group: &NewQuestionGroup,
    now: i64,
) -> Result<(), UserQuestionError> {
    let inserted = transaction.execute(
        "INSERT INTO pending_question_groups (
            id, run_id, sequence, status, created_at_unix_ms, updated_at_unix_ms
        ) VALUES (?1, ?2, ?3, 'pending', ?4, ?4)",
        params![group.group_id, group.run_id, group.sequence, now],
    );
    match inserted {
        Ok(_) => {}
        Err(error) if is_constraint_error(&error) => {
            return Err(UserQuestionError::Conflict(
                "question group id or sequence is already in use".to_owned(),
            ));
        }
        Err(error) => return Err(error.into()),
    }
    for (position, item) in group.questions.iter().enumerate() {
        let question_json = serde_json::to_string(item).map_err(|error| {
            UserQuestionError::InvalidInput(format!("question item could not be encoded: {error}"))
        })?;
        let inserted = transaction.execute(
            "INSERT INTO pending_question_items (group_id, item_id, position, question_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![group.group_id, item.id, position as i64, question_json],
        );
        match inserted {
            Ok(_) => {}
            Err(error) if is_constraint_error(&error) => {
                return Err(UserQuestionError::Conflict(
                    "question item id is already in use".to_owned(),
                ));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn load_run(connection: &Connection, run_id: &str) -> Result<AgentRun, UserQuestionError> {
    let row = connection
        .query_row(
            "SELECT conversation_id, status, checkpoint_json, checkpoint_revision
             FROM agent_runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((conversation_id, status, checkpoint_json, checkpoint_revision)) = row else {
        return Err(UserQuestionError::NotFound("agent run"));
    };
    let checkpoint = checkpoint_json
        .map(|json| parse_stored_json(&json, "agent run checkpoint"))
        .transpose()?;
    if let Some(checkpoint) = &checkpoint {
        validate_checkpoint(checkpoint).map_err(|_| {
            UserQuestionError::CorruptStoredData(
                "agent run checkpoint failed semantic validation".to_owned(),
            )
        })?;
    }
    Ok(AgentRun {
        conversation_id,
        status: AgentRunStatus::parse(status)?,
        checkpoint,
        checkpoint_revision,
    })
}

fn load_question_group(
    connection: &Connection,
    group_id: &str,
) -> Result<PendingQuestionGroup, UserQuestionError> {
    let row = connection
        .query_row(
            "SELECT groups.id, groups.run_id, runs.conversation_id,
                    runs.checkpoint_json,
                    groups.sequence, groups.status, groups.revision,
                    groups.answers_json
             FROM pending_question_groups AS groups
             INNER JOIN agent_runs AS runs ON runs.id = groups.run_id
             WHERE groups.id = ?1",
            [group_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((
        group_id,
        run_id,
        conversation_id,
        checkpoint_json,
        sequence,
        status,
        revision,
        answers_json,
    )) = row
    else {
        return Err(UserQuestionError::NotFound("question group"));
    };

    let checkpoint = checkpoint_json
        .map(|json| parse_stored_json(&json, "agent run checkpoint"))
        .transpose()?;
    if let Some(checkpoint) = &checkpoint {
        validate_checkpoint(checkpoint).map_err(|_| {
            UserQuestionError::CorruptStoredData(
                "agent run checkpoint failed semantic validation".to_owned(),
            )
        })?;
    }

    let mut statement = connection.prepare(
        "SELECT item_id, question_json
         FROM pending_question_items
         WHERE group_id = ?1
         ORDER BY position",
    )?;
    let item_rows = statement.query_map([&group_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut questions = Vec::new();
    for item_row in item_rows {
        let (item_id, json) = item_row?;
        let question = parse_stored_json::<QuestionItem>(&json, "question item")?;
        validate_question_item(&question).map_err(|_| {
            UserQuestionError::CorruptStoredData(
                "stored question item failed semantic validation".to_owned(),
            )
        })?;
        if question.id != item_id {
            return Err(UserQuestionError::CorruptStoredData(
                "question item id does not match its stored definition".to_owned(),
            ));
        }
        questions.push(question);
    }
    if questions.is_empty() {
        return Err(UserQuestionError::CorruptStoredData(
            "question group has no items".to_owned(),
        ));
    }
    let status = QuestionGroupStatus::parse(status)?;
    let answers = answers_json
        .map(|json| parse_stored_json::<Vec<QuestionAnswer>>(&json, "question answers"))
        .transpose()?;
    let stored_group = NewQuestionGroup {
        group_id: group_id.clone(),
        run_id: run_id.clone(),
        sequence,
        questions: questions.clone(),
    };
    validate_question_group(&stored_group).map_err(|_| {
        UserQuestionError::CorruptStoredData(
            "stored question group failed semantic validation".to_owned(),
        )
    })?;
    if let Some(answers) = &answers {
        validate_answers(&questions, answers).map_err(|_| {
            UserQuestionError::CorruptStoredData(
                "stored question answers failed semantic validation".to_owned(),
            )
        })?;
    }
    match (status, answers.is_some()) {
        (QuestionGroupStatus::Answered, false)
        | (QuestionGroupStatus::Pending, true)
        | (QuestionGroupStatus::Cancelled, true)
        | (QuestionGroupStatus::Expired, true) => {
            return Err(UserQuestionError::CorruptStoredData(
                "question group status and answers do not match".to_owned(),
            ));
        }
        _ => {}
    }
    Ok(PendingQuestionGroup {
        group_id,
        run_id,
        conversation_id,
        checkpoint,
        sequence,
        status,
        revision,
        questions,
        answers,
    })
}

fn validate_question_group(group: &NewQuestionGroup) -> Result<(), UserQuestionError> {
    validate_identifier(&group.group_id, "question group id")?;
    validate_identifier(&group.run_id, "run id")?;
    if group.sequence <= 0 {
        return Err(UserQuestionError::InvalidInput(
            "question group sequence must be positive".to_owned(),
        ));
    }
    if group.questions.is_empty() || group.questions.len() > MAX_QUESTION_ITEMS {
        return Err(UserQuestionError::InvalidInput(format!(
            "question groups must contain between 1 and {MAX_QUESTION_ITEMS} items"
        )));
    }
    let mut ids = HashSet::with_capacity(group.questions.len());
    let mut total_bytes: usize = 2;
    for item in &group.questions {
        validate_question_item(item)?;
        if !ids.insert(item.id.as_str()) {
            return Err(UserQuestionError::InvalidInput(
                "question item ids must be unique within a group".to_owned(),
            ));
        }
        let item_bytes = serde_json::to_vec(item).map_err(|error| {
            UserQuestionError::InvalidInput(format!("question item could not be encoded: {error}"))
        })?;
        if item_bytes.len() > MAX_QUESTION_ITEM_BYTES {
            return Err(UserQuestionError::InvalidInput(
                "question item exceeds its size limit".to_owned(),
            ));
        }
        total_bytes = total_bytes.saturating_add(item_bytes.len() + 1);
    }
    if total_bytes > MAX_QUESTION_GROUP_BYTES {
        return Err(UserQuestionError::InvalidInput(
            "question group exceeds its size limit".to_owned(),
        ));
    }
    Ok(())
}

fn validate_question_item(item: &QuestionItem) -> Result<(), UserQuestionError> {
    validate_identifier(&item.id, "question id")?;
    validate_text(&item.title, "question title", false)?;
    if let Some(description) = &item.description {
        validate_text(description, "question description", true)?;
    }
    if let Some(placeholder) = &item.placeholder {
        validate_text(placeholder, "question placeholder", true)?;
    }
    if let Some(max_length) = item.max_length {
        if max_length == 0 || max_length > MAX_TEXT_BYTES {
            return Err(UserQuestionError::InvalidInput(
                "question text length constraint is outside the allowed range".to_owned(),
            ));
        }
    }
    if item.options.len() > MAX_OPTION_COUNT {
        return Err(UserQuestionError::InvalidInput(
            "question has too many options".to_owned(),
        ));
    }
    let mut option_ids = HashSet::with_capacity(item.options.len());
    for option in &item.options {
        validate_identifier(&option.id, "question option id")?;
        validate_text(&option.label, "question option label", false)?;
        if !option_ids.insert(option.id.as_str()) {
            return Err(UserQuestionError::InvalidInput(
                "question option ids must be unique".to_owned(),
            ));
        }
    }
    match item.kind {
        QuestionKind::Choice if item.options.len() < 2 => Err(UserQuestionError::InvalidInput(
            "choice questions must provide at least two options".to_owned(),
        )),
        QuestionKind::Text if !item.options.is_empty() => Err(UserQuestionError::InvalidInput(
            "text questions cannot provide choice options".to_owned(),
        )),
        _ => Ok(()),
    }
}

fn validate_answers(
    questions: &[QuestionItem],
    answers: &[QuestionAnswer],
) -> Result<String, UserQuestionError> {
    if answers.len() > questions.len() {
        return Err(UserQuestionError::InvalidInput(
            "too many answers were supplied".to_owned(),
        ));
    }
    let mut question_ids = HashSet::with_capacity(answers.len());
    for answer in answers {
        if !question_ids.insert(answer.question_id.as_str()) {
            return Err(UserQuestionError::InvalidInput(
                "each question may be answered only once".to_owned(),
            ));
        }
        let question = questions
            .iter()
            .find(|question| question.id == answer.question_id)
            .ok_or_else(|| {
                UserQuestionError::InvalidInput("answer references an unknown question".to_owned())
            })?;
        match (&question.kind, &answer.value) {
            (QuestionKind::Choice, QuestionAnswerValue::Choice { option_id }) => {
                if !question
                    .options
                    .iter()
                    .any(|option| option.id == *option_id)
                {
                    return Err(UserQuestionError::InvalidInput(
                        "answer references an unknown choice".to_owned(),
                    ));
                }
            }
            (QuestionKind::Text, QuestionAnswerValue::Text { text }) => {
                validate_text(text, "answer", true)?;
                if let Some(max_length) = question.max_length {
                    if text.chars().count() > max_length {
                        return Err(UserQuestionError::InvalidInput(
                            "answer exceeds the question text limit".to_owned(),
                        ));
                    }
                }
                if question.required && text.trim().is_empty() {
                    return Err(UserQuestionError::InvalidInput(
                        "a required answer cannot be empty".to_owned(),
                    ));
                }
            }
            _ => {
                return Err(UserQuestionError::InvalidInput(
                    "answer type does not match the question".to_owned(),
                ));
            }
        }
    }
    for question in questions {
        if question.required && !question_ids.contains(question.id.as_str()) {
            return Err(UserQuestionError::InvalidInput(
                "a required question is unanswered".to_owned(),
            ));
        }
    }
    let question_order = questions
        .iter()
        .enumerate()
        .map(|(position, question)| (question.id.as_str(), position))
        .collect::<HashMap<_, _>>();
    let mut canonical_answers = answers.to_vec();
    canonical_answers.sort_by_key(|answer| {
        question_order
            .get(answer.question_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    let json = serde_json::to_vec(&canonical_answers).map_err(|error| {
        UserQuestionError::InvalidInput(format!("answers could not be encoded: {error}"))
    })?;
    if json.len() > MAX_ANSWER_BYTES {
        return Err(UserQuestionError::InvalidInput(
            "answers exceed their size limit".to_owned(),
        ));
    }
    String::from_utf8(json).map_err(|error| {
        UserQuestionError::InvalidInput(format!("answers were not valid UTF-8: {error}"))
    })
}

fn validate_checkpoint(checkpoint: &Value) -> Result<String, UserQuestionError> {
    if !checkpoint.is_object() {
        return Err(UserQuestionError::InvalidInput(
            "checkpoint must be a JSON object".to_owned(),
        ));
    }
    validate_json_tree(checkpoint, 0)?;
    let json = serde_json::to_vec(checkpoint).map_err(|error| {
        UserQuestionError::InvalidInput(format!("checkpoint could not be encoded: {error}"))
    })?;
    if json.len() > MAX_CHECKPOINT_BYTES {
        return Err(UserQuestionError::InvalidInput(
            "checkpoint exceeds its size limit".to_owned(),
        ));
    }
    String::from_utf8(json).map_err(|error| {
        UserQuestionError::InvalidInput(format!("checkpoint was not valid UTF-8: {error}"))
    })
}

fn validate_json_tree(value: &Value, depth: usize) -> Result<(), UserQuestionError> {
    if depth > MAX_DEPTH {
        return Err(UserQuestionError::InvalidInput(
            "JSON nesting exceeds its limit".to_owned(),
        ));
    }
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let normalized = key
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if is_sensitive_key(&normalized) {
                    return Err(UserQuestionError::InvalidInput(
                        "checkpoint contains a credential-like field".to_owned(),
                    ));
                }
                validate_json_tree(child, depth + 1)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                validate_json_tree(child, depth + 1)?;
            }
        }
        Value::String(text) if text.len() > MAX_CHECKPOINT_BYTES => {
            return Err(UserQuestionError::InvalidInput(
                "checkpoint contains an oversized string".to_owned(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    key == "token"
        || key.contains("apikey")
        || key.contains("accesstoken")
        || key.contains("refreshtoken")
        || key.contains("idtoken")
        || key.contains("authtoken")
        || key.contains("sessiontoken")
        || key.contains("authorization")
        || key.contains("bearer")
        || key.contains("oauth")
        || key.contains("credential")
        || key.contains("clientsecret")
        || key.contains("privatekey")
        || key.contains("password")
        || key.contains("secret")
}

fn validate_identifier(value: &str, name: &str) -> Result<(), UserQuestionError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(UserQuestionError::InvalidInput(format!(
            "{name} must contain only ASCII letters, numbers, '.', ':', '_' or '-' and be at most {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, name: &str, allow_empty: bool) -> Result<(), UserQuestionError> {
    if (!allow_empty && value.trim().is_empty())
        || value.len() > MAX_TEXT_BYTES
        || value.contains('\0')
    {
        return Err(UserQuestionError::InvalidInput(format!(
            "{name} is empty, contains a NUL byte, or exceeds its size limit"
        )));
    }
    Ok(())
}

fn validate_non_negative(value: i64, name: &str) -> Result<(), UserQuestionError> {
    if value < 0 {
        return Err(UserQuestionError::InvalidInput(format!(
            "{name} must not be negative"
        )));
    }
    Ok(())
}

fn ensure_run_conversation(run: &AgentRun, conversation_id: &str) -> Result<(), UserQuestionError> {
    if run.conversation_id == conversation_id {
        Ok(())
    } else {
        Err(UserQuestionError::NotFound("agent run"))
    }
}

fn ensure_group_conversation(
    group: &PendingQuestionGroup,
    conversation_id: &str,
) -> Result<(), UserQuestionError> {
    if group.conversation_id == conversation_id {
        Ok(())
    } else {
        Err(UserQuestionError::NotFound("question group"))
    }
}

fn invalid_run_transition(status: AgentRunStatus, action: &str) -> UserQuestionError {
    UserQuestionError::Conflict(format!(
        "cannot {action} an agent run in the {} state",
        status.as_str()
    ))
}

fn parse_stored_json<T: for<'de> Deserialize<'de>>(
    json: &str,
    field: &'static str,
) -> Result<T, UserQuestionError> {
    serde_json::from_str(json).map_err(|_| UserQuestionError::CorruptStoredData(field.to_owned()))
}

fn is_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        )
    )
}

fn unix_time_millis() -> Result<i64, UserQuestionError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            UserQuestionError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
        })?;
    i64::try_from(elapsed.as_millis()).map_err(|error| {
        UserQuestionError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use rusqlite::Connection;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        NewAgentRun, NewQuestionGroup, QuestionAnswer, QuestionAnswerValue, QuestionGroupStatus,
        QuestionItem, QuestionKind, QuestionOption, UserQuestionError, create_run,
        interrupt_running_runs_after_restart, list_pending_groups,
        list_recoverable_question_groups, list_recoverable_runs, load_run, mark_run_cancelled,
        mark_run_failed, mark_run_interrupted, mark_run_resumed, next_question_sequence,
        pause_with_questions, save_checkpoint, submit_answers,
    };

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        prepare_connection(&connection);
        connection
    }

    fn prepare_connection(connection: &Connection) {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                 CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                 );
                 CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                 );
                 INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                    VALUES (15, 0);",
            )
            .expect("create question storage fixture");
        super::super::schema::initialize_schema(connection, super::super::schema::SCHEMA_VERSION)
            .expect("apply question storage schema");
        connection
            .execute("INSERT INTO conversations (id) VALUES ('conversation')", [])
            .expect("insert question storage conversation");
    }

    fn run(connection: &Connection, id: &str) {
        create_run(
            connection,
            &NewAgentRun {
                run_id: id.to_owned(),
                conversation_id: "conversation".to_owned(),
            },
        )
        .expect("create agent run");
    }

    fn questions(group_id: &str, run_id: &str, sequence: i64) -> NewQuestionGroup {
        NewQuestionGroup {
            group_id: group_id.to_owned(),
            run_id: run_id.to_owned(),
            sequence,
            questions: vec![
                QuestionItem {
                    id: "mode".to_owned(),
                    kind: QuestionKind::Choice,
                    title: "Choose a mode".to_owned(),
                    description: None,
                    options: vec![
                        QuestionOption {
                            id: "fast".to_owned(),
                            label: "Fast".to_owned(),
                        },
                        QuestionOption {
                            id: "safe".to_owned(),
                            label: "Safe".to_owned(),
                        },
                    ],
                    required: true,
                    placeholder: None,
                    max_length: None,
                },
                QuestionItem {
                    id: "note".to_owned(),
                    kind: QuestionKind::Text,
                    title: "Optional note".to_owned(),
                    description: None,
                    options: Vec::new(),
                    required: false,
                    placeholder: Some("Write a note".to_owned()),
                    max_length: Some(200),
                },
            ],
        }
    }

    fn answers(note: &str) -> Vec<QuestionAnswer> {
        vec![
            QuestionAnswer {
                question_id: "mode".to_owned(),
                value: QuestionAnswerValue::Choice {
                    option_id: "safe".to_owned(),
                },
            },
            QuestionAnswer {
                question_id: "note".to_owned(),
                value: QuestionAnswerValue::Text {
                    text: note.to_owned(),
                },
            },
        ]
    }

    #[test]
    fn pause_list_and_submit_support_multiple_question_items_and_idempotent_duplicates() {
        let connection = connection();
        run(&connection, "run-1");
        assert_eq!(
            next_question_sequence(&connection, "conversation", "run-1")
                .expect("first question sequence"),
            1
        );
        let paused = pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1, "messages": ["before pause"]}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause run");
        assert_eq!(paused.status, QuestionGroupStatus::Pending);
        assert_eq!(paused.questions.len(), 2);
        assert_eq!(
            paused.checkpoint,
            Some(json!({"turn": 1, "messages": ["before pause"]}))
        );
        assert_eq!(
            load_run(&connection, "run-1").unwrap().checkpoint_revision,
            1
        );
        assert_eq!(
            list_pending_groups(&connection, Some("conversation"))
                .expect("list pending groups")
                .len(),
            1
        );

        let first = submit_answers(&connection, "conversation", "group-1", 0, &answers("done"))
            .expect("submit answers");
        assert!(!first.idempotent);
        assert_eq!(first.group.status, QuestionGroupStatus::Answered);
        assert_eq!(
            list_recoverable_question_groups(&connection, "conversation")
                .expect("list answered but unresumed question groups")
                .len(),
            1
        );
        let mut reordered_answers = answers("done");
        reordered_answers.reverse();
        let duplicate = submit_answers(
            &connection,
            "conversation",
            "group-1",
            0,
            &reordered_answers,
        )
        .expect("accept identical duplicate");
        assert!(duplicate.idempotent);
        assert!(matches!(
            submit_answers(
                &connection,
                "conversation",
                "group-1",
                0,
                &answers("different"),
            ),
            Err(UserQuestionError::Conflict(_))
        ));

        mark_run_resumed(&connection, "conversation", "run-1").expect("resume after answers");
        assert_eq!(
            next_question_sequence(&connection, "conversation", "run-1")
                .expect("next question sequence"),
            2
        );
        let mut second_group = questions("group-2", "run-1", 2);
        second_group.questions[0].id = "mode-2".to_owned();
        second_group.questions[1].id = "note-2".to_owned();
        let second = pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            1,
            &json!({"turn": 2}),
            &second_group,
        )
        .expect("pause a consecutive question group");
        assert_eq!(second.sequence, 2);
        assert_eq!(
            list_pending_groups(&connection, Some("conversation"))
                .expect("list consecutive pending group")
                .iter()
                .map(|group| group.group_id.as_str())
                .collect::<Vec<_>>(),
            vec!["group-2"]
        );
    }

    #[test]
    fn recoverable_question_groups_keep_only_latest_consecutive_resume_point() {
        let connection = connection();
        run(&connection, "run-1");

        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause first question group");
        submit_answers(&connection, "conversation", "group-1", 0, &answers("first"))
            .expect("answer first question group");
        mark_run_resumed(&connection, "conversation", "run-1").expect("resume after first group");

        let mut second_group = questions("group-2", "run-1", 2);
        second_group.questions[0].id = "mode-2".to_owned();
        second_group.questions[1].id = "note-2".to_owned();
        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            1,
            &json!({"turn": 2}),
            &second_group,
        )
        .expect("pause second question group");

        let pending_recoverable = list_recoverable_question_groups(&connection, "conversation")
            .expect("list current pending question group");
        assert_eq!(
            pending_recoverable
                .iter()
                .map(|group| group.group_id.as_str())
                .collect::<Vec<_>>(),
            vec!["group-2"]
        );
        assert_eq!(pending_recoverable[0].status, QuestionGroupStatus::Pending);

        let second_answers = vec![
            QuestionAnswer {
                question_id: "mode-2".to_owned(),
                value: QuestionAnswerValue::Choice {
                    option_id: "safe".to_owned(),
                },
            },
            QuestionAnswer {
                question_id: "note-2".to_owned(),
                value: QuestionAnswerValue::Text {
                    text: "second".to_owned(),
                },
            },
        ];
        submit_answers(&connection, "conversation", "group-2", 0, &second_answers)
            .expect("answer second question group");

        let answered_recoverable = list_recoverable_question_groups(&connection, "conversation")
            .expect("list latest answered question group");
        assert_eq!(
            answered_recoverable
                .iter()
                .map(|group| group.group_id.as_str())
                .collect::<Vec<_>>(),
            vec!["group-2"]
        );
        assert_eq!(
            answered_recoverable[0].status,
            QuestionGroupStatus::Answered
        );
    }

    #[test]
    fn checkpoint_rejects_credentials_and_oversized_values() {
        let connection = connection();
        run(&connection, "run-1");
        assert!(matches!(
            save_checkpoint(
                &connection,
                "conversation",
                "run-1",
                0,
                &json!({"access_token": "secret"}),
            ),
            Err(UserQuestionError::InvalidInput(_))
        ));
        assert!(matches!(
            save_checkpoint(
                &connection,
                "conversation",
                "run-1",
                0,
                &json!({"api_key": "must not be stored"}),
            ),
            Err(UserQuestionError::InvalidInput(_))
        ));
        save_checkpoint(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"providerConnectionId": "gemini"}),
        )
        .expect("provider connection metadata is not a credential");
        let oversized = "x".repeat(super::MAX_CHECKPOINT_BYTES);
        assert!(matches!(
            save_checkpoint(
                &connection,
                "conversation",
                "run-1",
                0,
                &json!({"state": oversized}),
            ),
            Err(UserQuestionError::InvalidInput(_))
        ));
    }

    #[test]
    fn mutations_are_scoped_to_the_run_conversation() {
        let connection = connection();
        connection
            .execute(
                "INSERT INTO conversations (id) VALUES ('other-conversation')",
                [],
            )
            .expect("insert second conversation");
        run(&connection, "run-1");

        assert!(matches!(
            save_checkpoint(
                &connection,
                "other-conversation",
                "run-1",
                0,
                &json!({"turn": 1}),
            ),
            Err(UserQuestionError::NotFound("agent run"))
        ));
        assert!(matches!(
            pause_with_questions(
                &connection,
                "other-conversation",
                "run-1",
                0,
                &json!({"turn": 1}),
                &questions("group-1", "run-1", 1),
            ),
            Err(UserQuestionError::NotFound("agent run"))
        ));

        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause scoped run");
        assert!(matches!(
            submit_answers(
                &connection,
                "other-conversation",
                "group-1",
                0,
                &answers("wrong conversation"),
            ),
            Err(UserQuestionError::NotFound("question group"))
        ));
        assert!(matches!(
            mark_run_cancelled(&connection, "other-conversation", "run-1"),
            Err(UserQuestionError::NotFound("agent run"))
        ));
        assert_eq!(
            list_pending_groups(&connection, Some("conversation"))
                .expect("list scoped group")
                .len(),
            1
        );
    }

    #[test]
    fn reads_reject_semantically_invalid_stored_json() {
        let invalid_checkpoint_connection = connection();
        run(&invalid_checkpoint_connection, "run-1");
        let invalid_checkpoint = json!({"access_token": "must not be stored"}).to_string();
        invalid_checkpoint_connection
            .execute(
                "UPDATE agent_runs
                 SET status = 'paused', checkpoint_json = ?1,
                     checkpoint_updated_at_unix_ms = 0
                 WHERE id = 'run-1'",
                [&invalid_checkpoint],
            )
            .expect("write syntactically valid invalid checkpoint fixture");
        assert!(matches!(
            list_recoverable_runs(&invalid_checkpoint_connection, Some("conversation")),
            Err(UserQuestionError::CorruptStoredData(_))
        ));

        let invalid_question_connection = connection();
        run(&invalid_question_connection, "run-1");
        pause_with_questions(
            &invalid_question_connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause run for invalid question fixture");
        let invalid_question = json!({
            "id": "mode",
            "kind": "choice",
            "title": "Broken choice",
            "options": [],
            "required": true,
        })
        .to_string();
        invalid_question_connection
            .execute(
                "UPDATE pending_question_items
                 SET question_json = ?1 WHERE item_id = 'mode'",
                [&invalid_question],
            )
            .expect("write syntactically valid invalid question fixture");
        assert!(matches!(
            list_pending_groups(&invalid_question_connection, Some("conversation")),
            Err(UserQuestionError::CorruptStoredData(_))
        ));
    }

    #[test]
    fn database_byte_guards_reject_oversized_unicode_payloads() {
        let connection = connection();
        run(&connection, "run-1");
        let oversized_checkpoint = format!(
            "{{\"state\":\"{}\"}}",
            "é".repeat((super::MAX_CHECKPOINT_BYTES / 2) + 1)
        );
        assert!(
            connection
                .execute(
                    "UPDATE agent_runs
                     SET checkpoint_json = ?1, checkpoint_updated_at_unix_ms = 0
                     WHERE id = 'run-1'",
                    [&oversized_checkpoint],
                )
                .is_err()
        );
    }

    #[test]
    fn restart_recovery_interrupts_running_runs_and_failed_runs_are_terminal() {
        let connection = connection();
        run(&connection, "run-1");
        let recovered = interrupt_running_runs_after_restart(&connection, Some("conversation"))
            .expect("interrupt running run after restart");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, super::AgentRunStatus::Interrupted);
        assert!(
            interrupt_running_runs_after_restart(&connection, Some("conversation"))
                .expect("repeat restart recovery")
                .is_empty()
        );
        let failed = mark_run_failed(&connection, "conversation", "run-1")
            .expect("mark interrupted run failed");
        assert_eq!(failed.status, super::AgentRunStatus::Failed);
        assert!(matches!(
            mark_run_resumed(&connection, "conversation", "run-1"),
            Err(UserQuestionError::Conflict(_))
        ));
        assert!(
            list_recoverable_runs(&connection, Some("conversation"))
                .expect("list recoverable runs")
                .is_empty()
        );
    }

    #[test]
    fn invalid_answers_and_stale_revisions_are_rejected() {
        let connection = connection();
        run(&connection, "run-1");
        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause run");
        let missing_required = vec![QuestionAnswer {
            question_id: "note".to_owned(),
            value: QuestionAnswerValue::Text {
                text: "optional".to_owned(),
            },
        }];
        assert!(matches!(
            submit_answers(&connection, "conversation", "group-1", 0, &missing_required),
            Err(UserQuestionError::InvalidInput(_))
        ));
        assert!(matches!(
            submit_answers(&connection, "conversation", "group-1", 1, &answers("done")),
            Err(UserQuestionError::StaleRevision { .. })
        ));
    }

    #[test]
    fn interrupted_run_and_pending_group_survive_a_restart() {
        let path = temporary_database_path();
        {
            let connection = Connection::open(&path).expect("open durable database");
            prepare_connection(&connection);
            run(&connection, "run-1");
            pause_with_questions(
                &connection,
                "conversation",
                "run-1",
                0,
                &json!({"turn": 1}),
                &questions("group-1", "run-1", 1),
            )
            .expect("pause durable run");
            mark_run_interrupted(&connection, "conversation", "run-1")
                .expect("interrupt durable run");
        }
        {
            let connection = Connection::open(&path).expect("reopen durable database");
            connection
                .execute_batch("PRAGMA foreign_keys = ON;")
                .expect("enable durable foreign keys");
            let pending = list_pending_groups(&connection, None).expect("recover pending group");
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].run_id, "run-1");
            assert_eq!(pending[0].checkpoint, Some(json!({"turn": 1})));
            assert_eq!(
                load_run(&connection, "run-1").unwrap().checkpoint_revision,
                1
            );
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn cancelling_a_run_cancels_pending_groups_and_blocks_resume() {
        let connection = connection();
        run(&connection, "run-1");
        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause run");
        assert!(
            connection
                .execute(
                    "UPDATE agent_runs
                     SET status = 'cancelled', finished_at_unix_ms = 1
                     WHERE id = 'run-1'",
                    [],
                )
                .is_err()
        );
        mark_run_cancelled(&connection, "conversation", "run-1").expect("cancel run");
        assert!(
            list_pending_groups(&connection, None)
                .expect("list pending groups")
                .is_empty()
        );
        assert!(matches!(
            mark_run_resumed(&connection, "conversation", "run-1"),
            Err(UserQuestionError::Conflict(_))
        ));
    }

    #[test]
    fn cancellation_race_preserves_answer_first_and_cancels_cancel_first() {
        let answer_first = connection();
        run(&answer_first, "run-1");
        pause_with_questions(
            &answer_first,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause answer-first run");
        submit_answers(
            &answer_first,
            "conversation",
            "group-1",
            0,
            &answers("answer-first"),
        )
        .expect("answer before cancellation");

        let interrupted = mark_run_cancelled(&answer_first, "conversation", "run-1")
            .expect("preserve answered group for resume");
        assert_eq!(interrupted.status, super::AgentRunStatus::Interrupted);
        let recoverable = list_recoverable_question_groups(&answer_first, "conversation")
            .expect("list answered group after cancellation race");
        assert_eq!(recoverable.len(), 1);
        assert_eq!(recoverable[0].group_id, "group-1");
        assert_eq!(recoverable[0].status, QuestionGroupStatus::Answered);
        mark_run_resumed(&answer_first, "conversation", "run-1")
            .expect("resume interrupted answer-first run");

        let cancel_first = connection();
        run(&cancel_first, "run-1");
        pause_with_questions(
            &cancel_first,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause cancel-first run");
        let cancelled = mark_run_cancelled(&cancel_first, "conversation", "run-1")
            .expect("cancel pending group");
        assert_eq!(cancelled.status, super::AgentRunStatus::Cancelled);
        assert!(
            list_pending_groups(&cancel_first, None)
                .expect("list pending groups after cancellation")
                .is_empty()
        );
        assert!(matches!(
            submit_answers(
                &cancel_first,
                "conversation",
                "group-1",
                0,
                &answers("cancel-first"),
            ),
            Err(UserQuestionError::Conflict(_))
        ));
    }

    #[test]
    fn concurrent_identical_submissions_are_applied_once() {
        let path = temporary_database_path();
        {
            let connection = Connection::open(&path).expect("open concurrent database");
            prepare_connection(&connection);
            run(&connection, "run-1");
            pause_with_questions(
                &connection,
                "conversation",
                "run-1",
                0,
                &json!({"turn": 1}),
                &questions("group-1", "run-1", 1),
            )
            .expect("pause concurrent run");
        }
        let answers = Arc::new(answers("same"));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let path = path.clone();
            let answers = Arc::clone(&answers);
            handles.push(thread::spawn(move || {
                let connection = Connection::open(path).expect("open concurrent connection");
                connection
                    .busy_timeout(std::time::Duration::from_secs(5))
                    .expect("configure concurrent timeout");
                submit_answers(&connection, "conversation", "group-1", 0, &answers)
                    .expect("submit concurrent answers")
                    .idempotent
            }));
        }
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("join answer worker"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|idempotent| !**idempotent).count(), 1);
        assert_eq!(results.iter().filter(|idempotent| **idempotent).count(), 1);
        let _ = fs::remove_file(path);
    }

    fn temporary_database_path() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openchat-user-questions-{timestamp}-{}.sqlite3",
            Uuid::new_v4()
        ))
    }
}
