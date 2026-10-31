//! Database operations for agent runs, checkpoints, and user question lifecycle.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, Params, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::Value;

use super::super::connection::{StorageConnection, StorageRow, StorageRows, StorageTransaction};
use super::types::*;

pub(crate) trait StorageRead {
    fn query<P: Params>(&self, sql: &str, params: P) -> rusqlite::Result<StorageRows>;

    fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> rusqlite::Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> rusqlite::Result<T>;
}

impl StorageRead for StorageConnection<'_> {
    fn query<P: Params>(&self, sql: &str, params: P) -> rusqlite::Result<StorageRows> {
        StorageConnection::query(self, sql, params)
    }

    fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> rusqlite::Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> rusqlite::Result<T>,
    {
        StorageConnection::query_row(self, sql, params, f)
    }
}

impl StorageRead for StorageTransaction<'_> {
    fn query<P: Params>(&self, sql: &str, params: P) -> rusqlite::Result<StorageRows> {
        StorageTransaction::query(self, sql, params)
    }

    fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> rusqlite::Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> rusqlite::Result<T>,
    {
        StorageTransaction::query_row(self, sql, params, f)
    }
}

#[cfg(test)]
pub(crate) fn create_run(
    connection: &StorageConnection,
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

/// Creates a run and its first validated checkpoint in one transaction.
///
/// This prevents a failed checkpoint write from leaving a non-resumable running row.
///
/// # Errors
/// Returns validation, clock, or database errors without exposing a partial run.
pub(crate) fn create_run_with_checkpoint(
    connection: &mut StorageConnection,
    new_run: &NewAgentRun,
    checkpoint: &Value,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(&new_run.run_id, "run id")?;
    validate_identifier(&new_run.conversation_id, "conversation id")?;
    let checkpoint_json = validate_checkpoint(checkpoint)?;
    let now = unix_time_millis()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let result = transaction.execute(
        "INSERT INTO agent_runs (
            id, conversation_id, status, checkpoint_json, checkpoint_revision,
            checkpoint_updated_at_unix_ms, created_at_unix_ms, updated_at_unix_ms
        ) VALUES (?1, ?2, 'running', ?3, 1, ?4, ?4, ?4)",
        params![
            new_run.run_id,
            new_run.conversation_id,
            checkpoint_json,
            now
        ],
    );
    match result {
        Ok(_) => {}
        Err(error) if is_constraint_error(&error) => {
            return Err(UserQuestionError::Conflict(
                "the run id is already in use or the conversation does not exist".to_owned(),
            ));
        }
        Err(error) => return Err(error.into()),
    }
    let run = load_run(&transaction, &new_run.run_id)?;
    transaction.commit()?;
    Ok(run)
}

pub(crate) fn load_run_for_conversation(
    connection: &StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let run = load_run(connection, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    Ok(run)
}

pub(crate) fn list_subagent_runs_for_conversation(
    connection: &StorageConnection,
    conversation_id: &str,
    requested_limit: usize,
) -> Result<Vec<AgentRun>, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    if !(1..=100).contains(&requested_limit) {
        return Err(UserQuestionError::InvalidInput(
            "subagent run limit is outside the supported range".to_owned(),
        ));
    }
    let limit = i64::try_from(requested_limit)
        .map_err(|_| UserQuestionError::InvalidInput("subagent run limit is invalid".to_owned()))?;
    let run_ids = {
        let mut statement = connection.prepare(
            "SELECT id
             FROM agent_runs
             WHERE conversation_id = ?1
               AND json_extract(checkpoint_json, '$.runKind') = 'subagent'
             ORDER BY updated_at_unix_ms DESC, id DESC
             LIMIT ?2",
        )?;
        statement
            .query_map(rusqlite::params![conversation_id, limit], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    run_ids
        .into_iter()
        .map(|run_id| load_run_for_conversation(connection, conversation_id, &run_id))
        .collect()
}

pub(crate) fn list_run_summaries(
    connection: &StorageConnection,
    requested_limit: usize,
) -> Result<Vec<AgentRunSummary>, UserQuestionError> {
    if !(1..=50).contains(&requested_limit) {
        return Err(UserQuestionError::InvalidInput(
            "run summary limit is outside the supported range".to_owned(),
        ));
    }
    let limit = i64::try_from(requested_limit)
        .map_err(|_| UserQuestionError::InvalidInput("run summary limit is invalid".to_owned()))?;
    let rows = {
        let mut statement = connection.prepare(
            "SELECT runs.id, runs.conversation_id, conversations.title,
                    runs.status, runs.updated_at_unix_ms
             FROM agent_runs AS runs
             INNER JOIN conversations ON conversations.id = runs.conversation_id
             WHERE runs.status IN ('running', 'paused', 'interrupted')
                OR json_extract(runs.checkpoint_json, '$.runKind') = 'subagent'
             ORDER BY CASE runs.status WHEN 'running' THEN 0 ELSE 1 END,
                      runs.updated_at_unix_ms DESC, runs.id
             LIMIT ?1",
        )?;
        statement
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    rows.into_iter()
        .map(
            |(run_id, conversation_id, conversation_title, status, updated_at_unix_ms)| {
                let checkpoint = load_run(connection, &run_id)?.checkpoint;
                let provider_id = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("providerId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let model_id = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("modelId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let run_kind = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("runKind"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let parent_run_id = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("parentRunId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let objective = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("objective"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let progress_phase = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("progress"))
                    .and_then(|progress| progress.get("phase"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let progress_tool_name = checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.get("progress"))
                    .and_then(|progress| progress.get("toolName"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Ok(AgentRunSummary {
                    run_id,
                    conversation_id,
                    conversation_title,
                    status: AgentRunStatus::parse(status)?,
                    updated_at_unix_ms,
                    provider_id,
                    model_id,
                    run_kind,
                    parent_run_id,
                    objective,
                    progress_phase,
                    progress_tool_name,
                })
            },
        )
        .collect()
}

pub(crate) fn latest_active_goal_run<Q: StorageRead + ?Sized>(
    connection: &Q,
    conversation_id: &str,
) -> Result<Option<AgentRun>, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    let mut rows = connection.query(
        "SELECT id
         FROM agent_runs
         WHERE conversation_id = ?1
           AND status IN ('running', 'paused', 'interrupted')
         ORDER BY updated_at_unix_ms DESC, id DESC",
        [conversation_id],
    )?;
    let mut run_ids = Vec::new();
    while let Some(row) = rows.next()? {
        run_ids.push(row.get::<_, String>(0)?);
    }
    for run_id in run_ids {
        let run = load_run(connection, &run_id)?;
        if run
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.get("goal"))
            .is_some()
        {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

pub(crate) enum GoalActivation {
    Started(AgentRun),
    ActiveGoalExists,
}

pub(crate) fn activate_goal_run(
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
    expected_revision: i64,
    checkpoint: &Value,
) -> Result<GoalActivation, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    validate_non_negative(expected_revision, "checkpoint revision")?;
    if checkpoint.get("goal").and_then(Value::as_object).is_none() {
        return Err(UserQuestionError::InvalidInput(
            "goal checkpoint is missing".to_owned(),
        ));
    }
    let checkpoint_json = validate_checkpoint(checkpoint)?;
    let now = unix_time_millis()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    if run.checkpoint_revision != expected_revision {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }
    if run.status != AgentRunStatus::Running {
        return Err(invalid_run_transition(run.status, "start a goal"));
    }
    if run
        .checkpoint
        .as_ref()
        .and_then(|value| value.get("goal"))
        .is_some()
        || latest_active_goal_run(&transaction, conversation_id)?.is_some()
    {
        return Ok(GoalActivation::ActiveGoalExists);
    }
    let changed = transaction.execute(
        "UPDATE agent_runs
         SET checkpoint_json = ?1,
             checkpoint_revision = checkpoint_revision + 1,
             checkpoint_updated_at_unix_ms = ?2,
             updated_at_unix_ms = ?2
         WHERE id = ?3
           AND conversation_id = ?4
           AND checkpoint_revision = ?5
           AND status = 'running'",
        params![
            checkpoint_json,
            now,
            run_id,
            conversation_id,
            expected_revision
        ],
    )?;
    if changed != 1 {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }
    transaction.commit()?;
    load_run(connection, run_id).map(GoalActivation::Started)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GoalRunTransition {
    KeepRunning,
    Pause,
    Cancel,
}

pub(crate) fn update_goal_run(
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
    expected_revision: i64,
    checkpoint: &Value,
    transition: GoalRunTransition,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    validate_non_negative(expected_revision, "checkpoint revision")?;
    let checkpoint_json = validate_checkpoint(checkpoint)?;
    let now = unix_time_millis()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    if run.checkpoint_revision != expected_revision {
        return Err(UserQuestionError::StaleRevision {
            expected: expected_revision,
            actual: run.checkpoint_revision,
        });
    }
    if run
        .checkpoint
        .as_ref()
        .and_then(|value| value.get("goal"))
        .is_none()
    {
        return Err(UserQuestionError::Conflict(
            "the agent run is not a goal".to_owned(),
        ));
    }

    let (next_status, finished_at) = match transition {
        GoalRunTransition::KeepRunning => {
            if run.status != AgentRunStatus::Running {
                return Err(invalid_run_transition(run.status, "update goal progress"));
            }
            (AgentRunStatus::Running, None)
        }
        GoalRunTransition::Pause => {
            if !matches!(
                run.status,
                AgentRunStatus::Running | AgentRunStatus::Paused | AgentRunStatus::Interrupted
            ) {
                return Err(invalid_run_transition(run.status, "pause goal"));
            }
            (AgentRunStatus::Paused, None)
        }
        GoalRunTransition::Cancel => {
            if !matches!(
                run.status,
                AgentRunStatus::Running | AgentRunStatus::Paused | AgentRunStatus::Interrupted
            ) {
                return Err(invalid_run_transition(run.status, "stop goal"));
            }
            (AgentRunStatus::Cancelled, Some(now))
        }
    };
    let changed = transaction.execute(
        "UPDATE agent_runs
         SET status = ?1,
             checkpoint_json = ?2,
             checkpoint_revision = checkpoint_revision + 1,
             checkpoint_updated_at_unix_ms = ?3,
             updated_at_unix_ms = ?3,
             finished_at_unix_ms = CASE WHEN ?4 IS NULL THEN finished_at_unix_ms ELSE ?4 END
         WHERE id = ?5
           AND checkpoint_revision = ?6
           AND status IN ('running', 'paused', 'interrupted')",
        params![
            next_status.as_str(),
            checkpoint_json,
            now,
            finished_at,
            run_id,
            expected_revision,
        ],
    )?;
    if changed == 0 {
        return Err(UserQuestionError::Conflict(
            "the goal changed before its update could be saved".to_owned(),
        ));
    }
    if transition == GoalRunTransition::Cancel {
        transaction.execute(
            "UPDATE pending_question_groups
             SET status = 'cancelled',
                 revision = revision + 1,
                 updated_at_unix_ms = ?1
             WHERE run_id = ?2 AND status = 'pending'",
            params![now, run_id],
        )?;
    }
    transaction.commit()?;
    load_run(connection, run_id)
}

pub(crate) fn next_question_sequence(
    connection: &StorageConnection,
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
    connection: &mut StorageConnection,
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
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
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
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: Option<&str>,
) -> Result<Vec<PendingQuestionGroup>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
) -> Result<Vec<PendingQuestionGroup>, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    group_id: &str,
    expected_revision: i64,
    answers: &[QuestionAnswer],
) -> Result<SubmittedAnswers, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(group_id, "question group id")?;
    validate_non_negative(expected_revision, "question group revision")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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

pub(crate) fn mark_run_paused(
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let run = load_run(&transaction, run_id)?;
    ensure_run_conversation(&run, conversation_id)?;
    match run.status {
        AgentRunStatus::Paused => {
            transaction.commit()?;
            Ok(run)
        }
        AgentRunStatus::Running => {
            let now = unix_time_millis()?;
            transaction.execute(
                "UPDATE agent_runs SET status = 'paused', updated_at_unix_ms = ?1
                 WHERE id = ?2 AND status = 'running'",
                params![now, run_id],
            )?;
            transaction.commit()?;
            load_run(connection, run_id)
        }
        status => Err(invalid_run_transition(status, "pause")),
    }
}

pub(crate) fn mark_run_cancelled(
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: &str,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    validate_identifier(conversation_id, "conversation id")?;
    validate_identifier(run_id, "run id")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: Option<&str>,
) -> Result<Vec<AgentRun>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
    connection: &mut StorageConnection,
    conversation_id: Option<&str>,
) -> Result<Vec<AgentRun>, UserQuestionError> {
    if let Some(conversation_id) = conversation_id {
        validate_identifier(conversation_id, "conversation id")?;
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
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
    transaction: &StorageTransaction<'_>,
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

pub(crate) fn load_run<Q: StorageRead + ?Sized>(
    connection: &Q,
    run_id: &str,
) -> Result<AgentRun, UserQuestionError> {
    let row = connection
        .query_row(
            "SELECT id, conversation_id, status, checkpoint_json, checkpoint_revision
             FROM agent_runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((run_id, conversation_id, status, checkpoint_json, checkpoint_revision)) = row else {
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
        run_id,
        conversation_id,
        status: AgentRunStatus::parse(status)?,
        checkpoint,
        checkpoint_revision,
    })
}

fn load_question_group<Q: StorageRead + ?Sized>(
    connection: &Q,
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

    let mut item_rows = connection.query(
        "SELECT item_id, question_json
         FROM pending_question_items
         WHERE group_id = ?1
         ORDER BY position",
        [&group_id],
    )?;
    let mut questions = Vec::new();
    while let Some(item_row) = item_rows.next()? {
        let item_id = item_row.get::<_, String>(0)?;
        let json = item_row.get::<_, String>(1)?;
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
    if let Some(max_length) = item.max_length
        && (max_length == 0 || max_length > MAX_TEXT_BYTES)
    {
        return Err(UserQuestionError::InvalidInput(
            "question text length constraint is outside the allowed range".to_owned(),
        ));
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
        QuestionKind::Choice | QuestionKind::MultiChoice if item.options.len() < 2 => {
            Err(UserQuestionError::InvalidInput(
                "choice questions must provide at least two options".to_owned(),
            ))
        }
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
            (QuestionKind::MultiChoice, QuestionAnswerValue::MultiChoice { option_ids }) => {
                let mut selected_ids = HashSet::with_capacity(option_ids.len());
                if option_ids.is_empty() || option_ids.len() > question.options.len() {
                    return Err(UserQuestionError::InvalidInput(
                        "a multiple-choice answer must select at least one available option"
                            .to_owned(),
                    ));
                }
                for option_id in option_ids {
                    if !selected_ids.insert(option_id.as_str())
                        || !question
                            .options
                            .iter()
                            .any(|option| option.id == *option_id)
                    {
                        return Err(UserQuestionError::InvalidInput(
                            "a multiple-choice answer contains a duplicate or unknown option"
                                .to_owned(),
                        ));
                    }
                }
            }
            (QuestionKind::Text, QuestionAnswerValue::Text { text }) => {
                validate_text(text, "answer", true)?;
                if let Some(max_length) = question.max_length
                    && text.chars().count() > max_length
                {
                    return Err(UserQuestionError::InvalidInput(
                        "answer exceeds the question text limit".to_owned(),
                    ));
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
    for answer in &mut canonical_answers {
        let Some(question) = questions
            .iter()
            .find(|question| question.id == answer.question_id)
        else {
            continue;
        };
        if let QuestionAnswerValue::MultiChoice { option_ids } = &mut answer.value {
            option_ids.sort_by_key(|option_id| {
                question
                    .options
                    .iter()
                    .position(|option| option.id == *option_id)
                    .unwrap_or(usize::MAX)
            });
        }
    }
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
