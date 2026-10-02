use rusqlite::{OptionalExtension, Row, params, types::Type};

use crate::storage::AppStorage;

use super::{
    AssistantMessageWrite, ConversationRoute, NewTitleJob, StoredMessage, unix_time_millis,
};

pub fn conversation_route(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Option<ConversationRoute>> {
    let route = storage
        .connect()?
        .query_row(
            "SELECT connection_id, workspace_id, model_id, title_source
         FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((connection_id, workspace_id, model_id, title_source)) = route else {
        return Ok(None);
    };
    match (connection_id, workspace_id, model_id) {
        (Some(connection_id), Some(workspace_id), Some(model_id)) => Ok(Some(ConversationRoute {
            connection_id,
            workspace_id,
            model_id,
            title_is_automatic: title_source == "automatic",
        })),
        (None, None, None) => Ok(None),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub fn conversation_messages(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Vec<StoredMessage>> {
    let database = storage.connect()?;
    let mut statement = database.prepare(
        "SELECT id, role, content, status, output_tokens, tool_activities FROM messages
         WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
         ORDER BY created_at ASC, id ASC",
    )?;
    let storage_root = storage.root().to_path_buf();
    let rows = statement.query_map([conversation_id], |row| {
        stored_message_from_row(row, &storage_root, conversation_id)
    })?;
    rows.collect()
}

pub fn conversation_messages_from_boundary(
    storage: &AppStorage,
    conversation_id: &str,
    boundary_message_id: &str,
) -> rusqlite::Result<Option<Vec<StoredMessage>>> {
    let database = storage.connect()?;
    let Some(boundary_created_at) = database
        .query_row(
            "SELECT COALESCE(created_at, 0) FROM messages
             WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, boundary_message_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    else {
        return Ok(None);
    };
    let previous_message = database
        .query_row(
            "SELECT COALESCE(created_at, 0), id FROM messages
             WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
               AND (COALESCE(created_at, 0) < ?2
                    OR (COALESCE(created_at, 0) = ?2 AND id < ?3))
             ORDER BY COALESCE(created_at, 0) DESC, id DESC
             LIMIT 1",
            params![conversation_id, boundary_created_at, boundary_message_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let (start_created_at, start_message_id) =
        previous_message.unwrap_or_else(|| (boundary_created_at, boundary_message_id.to_owned()));

    let mut statement = database.prepare(
        "SELECT id, role, content, status, output_tokens, tool_activities FROM messages
         WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
           AND (COALESCE(created_at, 0) > ?2
                OR (COALESCE(created_at, 0) = ?2 AND id >= ?3))
         ORDER BY COALESCE(created_at, 0) ASC, id ASC",
    )?;
    let storage_root = storage.root().to_path_buf();
    let rows = statement.query_map(
        params![conversation_id, start_created_at, start_message_id],
        |row| stored_message_from_row(row, &storage_root, conversation_id),
    )?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map(Some)
}

fn stored_message_from_row(
    row: &Row<'_>,
    storage_root: &std::path::Path,
    conversation_id: &str,
) -> rusqlite::Result<StoredMessage> {
    let tool_activities_json: String = row.get(5)?;
    let tool_activities = serde_json::from_str(&tool_activities_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
    })?;
    let id: String = row.get(0)?;
    let (content, attachments) = crate::attachments::decode_message_content(
        row.get(2)?,
        storage_root,
        conversation_id,
        &id,
    )?;
    Ok(StoredMessage {
        id,
        role: row.get(1)?,
        content,
        status: row.get(3)?,
        output_tokens: row.get(4)?,
        tool_activities,
        attachments,
    })
}

pub fn is_retryable_latest_assistant_message(messages: &[StoredMessage], id: &str) -> bool {
    let Some(last) = messages.last() else {
        return false;
    };
    if last.id != id
        || last.role != "assistant"
        || !matches!(last.status.as_str(), "completed" | "failed" | "stopped")
    {
        return false;
    }
    messages
        .get(messages.len().saturating_sub(2))
        .is_some_and(|previous| previous.role == "user")
}

pub fn apply_generated_title(
    storage: &AppStorage,
    conversation_id: &str,
    title: &str,
) -> rusqlite::Result<bool> {
    let changed = storage.connect()?.execute(
        "UPDATE conversations SET title = ?2, updated_at = MAX(updated_at, ?3)
         WHERE id = ?1 AND title_source = 'automatic'",
        params![conversation_id, title, unix_time_millis()?],
    )?;
    Ok(changed == 1)
}

pub fn save_assistant_message(
    storage: &AppStorage,
    message: AssistantMessageWrite<'_>,
) -> rusqlite::Result<()> {
    let elapsed_microseconds = message
        .elapsed
        .and_then(|elapsed| i64::try_from(elapsed.as_micros()).ok());
    let tokens_per_second = message.output_tokens.and_then(|tokens| {
        let seconds = message.elapsed?.as_secs_f64();
        (seconds > 0.0).then_some(tokens as f64 / seconds)
    });
    let database = storage.connect()?;
    database.execute(
        "INSERT INTO messages (
            id, conversation_id, role, content, created_at, output_tokens,
            tokens_per_second, elapsed_microseconds, status
         ) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(conversation_id, id) DO UPDATE SET
            content = excluded.content,
            output_tokens = excluded.output_tokens,
            tokens_per_second = excluded.tokens_per_second,
            elapsed_microseconds = excluded.elapsed_microseconds,
            status = excluded.status",
        params![
            message.message_id,
            message.conversation_id,
            message.content,
            message.created_at_unix_ms,
            message.output_tokens,
            tokens_per_second,
            elapsed_microseconds,
            message.status,
        ],
    )?;
    database.execute(
        "UPDATE conversations SET updated_at = MAX(updated_at, ?2) WHERE id = ?1",
        params![message.conversation_id, message.created_at_unix_ms],
    )?;
    Ok(())
}

pub fn create_title_job(storage: &AppStorage, job: NewTitleJob<'_>) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "INSERT INTO title_generation_jobs (
            id, conversation_id, connection_id, workspace_id, model_id,
            status, reason_code, created_at_unix_ms, completed_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            job.id,
            job.conversation_id,
            job.connection_id,
            job.workspace_id,
            job.model_id,
            job.status,
            job.reason_code,
            unix_time_millis()?,
            (job.status != "queued")
                .then(unix_time_millis)
                .transpose()?,
        ],
    )?;
    Ok(())
}

pub fn finish_title_job(
    storage: &AppStorage,
    job_id: &str,
    status: &str,
    reason_code: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE title_generation_jobs SET status = ?2, reason_code = ?3,
            completed_at_unix_ms = ?4, attempt_count = attempt_count + 1
         WHERE id = ?1",
        params![job_id, status, reason_code, unix_time_millis()?],
    )?;
    Ok(())
}
