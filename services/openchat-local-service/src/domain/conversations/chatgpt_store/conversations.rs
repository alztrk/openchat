//! Persists conversation routes, messages, title jobs, and tool checkpoints.
//!
//! Database transactions keep assistant messages and associated tool state
//! consistent when a provider stream is interrupted or completed.

use rusqlite::{OptionalExtension, params, types::Type};

use crate::{
    provider_schema::{ChatStreamSnapshot, ToolActivity},
    storage::{AppStorage, StorageConnection, StorageRow},
};

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

#[allow(dead_code)]
pub fn conversation_title_is_automatic(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    storage.connect()?.query_row(
        "SELECT title_source = 'automatic' FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    )
}

pub fn is_custom_title(storage: &AppStorage, conversation_id: &str) -> rusqlite::Result<bool> {
    storage.connect()?.query_row(
        "SELECT title_source != 'automatic' FROM conversations WHERE id = ?1",
        params![conversation_id],
        |row| row.get(0),
    )
}

pub fn is_default_title(title: &str) -> bool {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "new conversation"
        | "start a new conversation"
        | "new chat"
        | "untitled"
        | "untitled conversation"
        | "chat"
        | "yeni sohbet"
        | "yeni bir sohbet"
        | "yeni bir sohbet baslat"
        | "yeni bir sohbet başlat"
        | "yeni konuşma"
        | "yeni konusma"
        | "başlıksız"
        | "başlıksız sohbet"
        | "basliksiz"
        | "basliksiz sohbet"
        | "neue unterhaltung"
        | "neue unterhaltung starten"
        | "neuer chat"
        | "ohne titel"
        | "nueva conversación"
        | "nueva conversacion"
        | "iniciar una conversación nueva"
        | "iniciar una conversacion nueva"
        | "nuevo chat"
        | "sin título"
        | "sin titulo"
        | "nouvelle conversation"
        | "démarrer une nouvelle conversation"
        | "demarrer une nouvelle conversation"
        | "nouveau chat"
        | "sans titre" => true,
        _ => is_date_title(trimmed),
    }
}

fn is_date_title(text: &str) -> bool {
    let s = text.trim();
    if (8..=32).contains(&s.len()) {
        let has_separator = s.contains('-') || s.contains('/') || s.contains('.');
        let is_numeric_date = s
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | '/' | '.' | ':' | ' ' | 'T'));
        if has_separator && is_numeric_date {
            let digit_count = s.chars().filter(|c| c.is_ascii_digit()).count();
            if digit_count >= 6 {
                return true;
            }
        }
    }
    false
}

pub fn should_generate_automatic_title_on_connection(
    database: &StorageConnection<'_>,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    let row = database
        .query_row(
            "SELECT title, title_source FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((title, title_source)) = row else {
        return Ok(false);
    };

    if title_source != "automatic" {
        return Ok(false);
    }

    let completed_assistant_turns: i64 = database.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'completed'",
        params![conversation_id],
        |row| row.get(0),
    )?;

    if !(1..=2).contains(&completed_assistant_turns) {
        return Ok(false);
    }

    if is_default_title(&title) {
        return Ok(true);
    }

    let first_user_message: Option<String> = database
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND role = 'user' ORDER BY created_at ASC, id ASC LIMIT 1",
            params![conversation_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(content) = first_user_message {
        let first_line = content.lines().next().unwrap_or("").trim();
        let prompt_snippet = if first_line.is_empty() {
            content.trim()
        } else {
            first_line
        };
        let prompt_snippet = prompt_snippet.chars().take(500).collect::<String>();
        if title.trim() == prompt_snippet.trim() {
            return Ok(true);
        }
    }

    if completed_assistant_turns == 1 {
        return Ok(true);
    }

    Ok(false)
}

pub fn should_generate_automatic_title(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    let database = storage.connect()?;
    should_generate_automatic_title_on_connection(&database, conversation_id)
}

pub fn conversation_messages(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Vec<StoredMessage>> {
    let rows = {
        let database = storage.connect()?;
        let mut statement = database.prepare(
            "SELECT id, role, content, status, output_tokens, tool_activities FROM messages
             WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
             ORDER BY created_at ASC, id ASC",
        )?;
        statement
            .query_map([conversation_id], stored_message_row_from_sql)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    rows.into_iter()
        .map(|row| stored_message_from_values(row, storage.root(), conversation_id))
        .collect()
}

pub fn conversation_messages_from_boundary(
    storage: &AppStorage,
    conversation_id: &str,
    boundary_message_id: &str,
) -> rusqlite::Result<Option<Vec<StoredMessage>>> {
    let rows = {
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
        let (start_created_at, start_message_id) = previous_message
            .unwrap_or_else(|| (boundary_created_at, boundary_message_id.to_owned()));

        let mut statement = database.prepare(
            "SELECT id, role, content, status, output_tokens, tool_activities FROM messages
             WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
               AND (COALESCE(created_at, 0) > ?2
                    OR (COALESCE(created_at, 0) = ?2 AND id >= ?3))
             ORDER BY COALESCE(created_at, 0) ASC, id ASC",
        )?;
        statement
            .query_map(
                params![conversation_id, start_created_at, start_message_id],
                stored_message_row_from_sql,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    rows.into_iter()
        .map(|row| stored_message_from_values(row, storage.root(), conversation_id))
        .collect::<rusqlite::Result<Vec<_>>>()
        .map(Some)
}

type StoredMessageRow = (String, String, String, String, Option<i64>, String);

fn stored_message_row_from_sql(row: &StorageRow) -> rusqlite::Result<StoredMessageRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn stored_message_from_values(
    row: StoredMessageRow,
    storage_root: &std::path::Path,
    conversation_id: &str,
) -> rusqlite::Result<StoredMessage> {
    let (id, role, stored_content, status, output_tokens, tool_activities_json) = row;
    let tool_activities = serde_json::from_str(&tool_activities_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
    })?;
    let (content, attachments) = crate::attachments::decode_message_content(
        stored_content,
        storage_root,
        conversation_id,
        &id,
    )?;
    Ok(StoredMessage {
        id,
        role,
        content,
        status,
        output_tokens,
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

pub fn save_assistant_message_on_connection(
    database: &mut StorageConnection<'_>,
    message: AssistantMessageWrite<'_>,
) -> rusqlite::Result<()> {
    validate_assistant_message_content(message.content)?;
    let elapsed_microseconds = message
        .elapsed
        .and_then(|elapsed| i64::try_from(elapsed.as_micros()).ok());
    let tokens_per_second = message.output_tokens.and_then(|tokens| {
        let seconds = message.elapsed?.as_secs_f64();
        (seconds > 0.0).then_some(tokens as f64 / seconds)
    });
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let existing_content = transaction
        .query_row(
            "SELECT content FROM messages
             WHERE conversation_id = ?1 AND id = ?2 AND role = 'assistant'",
            params![message.conversation_id, message.message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let content = crate::attachments::merge_content_preserving_attachments(
        existing_content.as_deref(),
        message.content,
    )
    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    validate_assistant_message_content(&content)?;
    transaction.execute(
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
            content,
            message.created_at_unix_ms,
            message.output_tokens,
            tokens_per_second,
            elapsed_microseconds,
            message.status,
        ],
    )?;
    transaction.execute(
        "UPDATE conversations SET updated_at = MAX(updated_at, ?2) WHERE id = ?1",
        params![message.conversation_id, message.created_at_unix_ms],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn save_assistant_message(
    storage: &AppStorage,
    message: AssistantMessageWrite<'_>,
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    save_assistant_message_on_connection(&mut database, message)
}

pub fn save_assistant_tool_checkpoint(
    storage: &AppStorage,
    snapshot: &ChatStreamSnapshot,
    activities: &[ToolActivity],
) -> rusqlite::Result<()> {
    let metadata_limit = crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_METADATA_BYTES;
    let (activities_json, persisted_activity_count) =
        serialize_bounded_tool_activities(activities, metadata_limit)?;
    if let Err(error) = validate_assistant_message_content(&snapshot.content) {
        log_rejected_tool_checkpoint(
            storage,
            "content_limit",
            snapshot.content.len(),
            activities_json.len(),
            persisted_activity_count,
        );
        return Err(error);
    }
    let mut database = storage.connect()?;
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let existing_content = transaction
        .query_row(
            "SELECT content FROM messages
             WHERE conversation_id = ?1 AND id = ?2 AND role = 'assistant'",
            params![snapshot.conversation_id, snapshot.message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let content = crate::attachments::merge_content_preserving_attachments(
        existing_content.as_deref(),
        &snapshot.content,
    )
    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    if let Err(error) = validate_assistant_message_content(&content) {
        log_rejected_tool_checkpoint(
            storage,
            "content_limit",
            content.len(),
            activities_json.len(),
            persisted_activity_count,
        );
        return Err(error);
    }
    transaction.execute(
        "INSERT INTO messages (
            id, conversation_id, role, content, created_at, tool_activities, status
         ) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, 'streaming')
         ON CONFLICT(conversation_id, id) DO UPDATE SET
            content = excluded.content,
            created_at = COALESCE(messages.created_at, excluded.created_at),
            tool_activities = excluded.tool_activities,
            status = 'streaming'",
        params![
            snapshot.message_id,
            snapshot.conversation_id,
            content,
            snapshot.created_at_unix_ms,
            activities_json,
        ],
    )?;
    transaction.commit()
}

fn serialize_bounded_tool_activities(
    activities: &[ToolActivity],
    maximum_bytes: usize,
) -> rusqlite::Result<(String, usize)> {
    let mut encoded_activities = Vec::new();
    let mut encoded_bytes = 2usize;

    for activity in activities.iter().rev() {
        let mut encoded = serde_json::to_string(activity)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let separator_bytes = usize::from(!encoded_activities.is_empty());
        if encoded_bytes
            .saturating_add(separator_bytes)
            .saturating_add(encoded.len())
            > maximum_bytes
        {
            let mut compacted = activity.clone();
            compacted.output = None;
            compacted.file_changes.clear();
            compacted.file_changes_error = None;
            encoded = serde_json::to_string(&compacted)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            if encoded_bytes
                .saturating_add(separator_bytes)
                .saturating_add(encoded.len())
                > maximum_bytes
            {
                compacted.arguments = serde_json::Value::Object(Default::default());
                compacted.target_path = None;
                encoded = serde_json::to_string(&compacted)
                    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            }
        }

        let next_bytes = encoded_bytes
            .saturating_add(separator_bytes)
            .saturating_add(encoded.len());
        if next_bytes > maximum_bytes {
            break;
        }
        encoded_bytes = next_bytes;
        encoded_activities.push(encoded);
    }

    encoded_activities.reverse();
    let mut output = String::with_capacity(encoded_bytes);
    output.push('[');
    for (index, activity) in encoded_activities.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(activity);
    }
    output.push(']');
    Ok((output, encoded_activities.len()))
}

fn message_size_limit_error(message: &'static str) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
        Some(message.to_owned()),
    )
}

fn validate_assistant_message_content(content: &str) -> rusqlite::Result<()> {
    if content.len() > crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES {
        return Err(message_size_limit_error(
            "chat message content limit exceeded",
        ));
    }
    Ok(())
}

fn log_rejected_tool_checkpoint(
    storage: &AppStorage,
    reason: &'static str,
    content_bytes: usize,
    metadata_bytes: usize,
    activity_count: usize,
) {
    if storage
        .log_rejected_assistant_checkpoint(reason, content_bytes, metadata_bytes, activity_count)
        .is_err()
    {
        eprintln!("assistant_checkpoint_diagnostic_write_failed");
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(name: &str) -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "openchat-conv-test-{name}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn storage_with_assistant_attachment(name: &str) -> (TestDirectory, AppStorage, String) {
        let directory = TestDirectory::new(name);
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        let existing_content = format!(
            "{}{}",
            crate::conversation_limits::MESSAGE_CONTENT_PREFIX,
            serde_json::json!({
                "content": "",
                "attachments": [{
                    "id": "attachment-1",
                    "name": "note.txt",
                    "mimeType": "text/plain",
                    "sizeBytes": 1,
                    "kind": "text"
                }]
            })
        );
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tokens_per_second REAL,
                    elapsed_microseconds INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    status TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, id)
                );",
            )
            .expect("create assistant message test schema");
        connection
            .execute(
                "INSERT INTO conversations (id, title, created_at, updated_at)
                 VALUES ('conversation-1', 'Test', 1, 1)",
                [],
            )
            .expect("insert test conversation");
        connection
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, status)
                 VALUES ('assistant-1', 'conversation-1', 'assistant', ?1, 1, 'streaming')",
                [&existing_content],
            )
            .expect("insert assistant attachment message");
        drop(connection);
        (directory, storage, existing_content)
    }

    fn over_limit_merged_content(existing_content: &str) -> String {
        let empty_merge =
            crate::attachments::merge_content_preserving_attachments(Some(existing_content), "")
                .expect("merge empty content with attachment envelope");
        let content_length =
            crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES - empty_merge.len() + 1;
        "x".repeat(content_length)
    }

    fn assert_content_size_limit_error(error: rusqlite::Error) {
        assert!(matches!(
            error,
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == rusqlite::ErrorCode::TooBig
        ));
    }

    #[test]
    fn assistant_message_save_rejects_oversized_merged_attachment_content() {
        let (_directory, storage, existing_content) =
            storage_with_assistant_attachment("assistant-message-limit");
        let content = over_limit_merged_content(&existing_content);
        assert!(content.len() < crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES);

        let error = save_assistant_message(
            &storage,
            AssistantMessageWrite {
                conversation_id: "conversation-1",
                message_id: "assistant-1",
                content: &content,
                status: "completed",
                created_at_unix_ms: 2,
                output_tokens: None,
                elapsed: None,
            },
        )
        .expect_err("the merged assistant content must exceed the limit");
        assert_content_size_limit_error(error);

        let persisted_content: String = storage
            .connect()
            .expect("connect after rejected message")
            .query_row(
                "SELECT content FROM messages WHERE conversation_id = 'conversation-1' AND id = 'assistant-1'",
                [],
                |row| row.get(0),
            )
            .expect("read unchanged assistant content");
        assert_eq!(persisted_content, existing_content);
    }

    #[test]
    fn assistant_checkpoint_rejects_oversized_merged_attachment_content() {
        let (_directory, storage, existing_content) =
            storage_with_assistant_attachment("assistant-checkpoint-limit");
        let content = over_limit_merged_content(&existing_content);
        assert!(content.len() < crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES);
        let snapshot = ChatStreamSnapshot::new("conversation-1", "assistant-1", &content, 2);

        let error = save_assistant_tool_checkpoint(&storage, &snapshot, &[])
            .expect_err("the merged checkpoint content must exceed the limit");
        assert_content_size_limit_error(error);

        let persisted_content: String = storage
            .connect()
            .expect("connect after rejected checkpoint")
            .query_row(
                "SELECT content FROM messages WHERE conversation_id = 'conversation-1' AND id = 'assistant-1'",
                [],
                |row| row.get(0),
            )
            .expect("read unchanged assistant content");
        assert_eq!(persisted_content, existing_content);
    }

    #[test]
    fn assistant_message_content_limit_is_inclusive_at_the_byte_boundary() {
        let content = "x".repeat(crate::conversation_limits::MAXIMUM_CHAT_MESSAGE_CONTENT_BYTES);
        assert!(validate_assistant_message_content(&content).is_ok());
        let over_limit = format!("{content}x");
        assert_content_size_limit_error(
            validate_assistant_message_content(&over_limit)
                .expect_err("one byte over the limit must fail"),
        );
    }

    #[test]
    fn test_is_default_title() {
        assert!(is_default_title(""));
        assert!(is_default_title("   "));
        assert!(is_default_title("New Conversation"));
        assert!(is_default_title("new conversation"));
        assert!(is_default_title("Start a new conversation"));
        assert!(is_default_title("New Chat"));
        assert!(is_default_title("Yeni sohbet"));
        assert!(is_default_title("yeni sohbet"));
        assert!(is_default_title("Yeni bir sohbet"));
        assert!(is_default_title("Neue Unterhaltung"));
        assert!(is_default_title("Nueva conversación"));
        assert!(is_default_title("Nouvelle conversation"));
        assert!(is_default_title("Untitled"));
        assert!(is_default_title("Başlıksız"));
        assert!(is_default_title("2026-10-20"));
        assert!(is_default_title("2026-10-20 14:30:00"));
        assert!(is_default_title("20.10.2026"));
        assert!(is_default_title("10/20/2026"));

        assert!(!is_default_title("Rust Cargo Setup Guide"));
        assert!(!is_default_title("Fixing SQLite connection pool"));
        assert!(!is_default_title("Alican's notes"));
    }

    #[test]
    fn test_automatic_titling_lifecycle() {
        let directory = TestDirectory::new("titling-lifecycle");
        let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");
        let conn = storage.connect().expect("connect");

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                title_source TEXT NOT NULL DEFAULT 'automatic',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER,
                output_tokens INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                PRIMARY KEY (conversation_id, id)
            );",
        )
        .expect("create test schema");

        let conv_id = "test-conv-1";
        conn.execute(
            "INSERT INTO conversations (id, title, title_source, created_at, updated_at)
             VALUES (?1, 'New Conversation', 'automatic', 1000, 1000)",
            params![conv_id],
        )
        .expect("insert conv");
        drop(conn);

        assert!(!is_custom_title(&storage, conv_id).expect("is_custom"));
        // 0 completed assistant turns: should not generate
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should gen"));

        // Add 1 completed assistant turn
        let conn = storage.connect().expect("connect");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, status, created_at)
             VALUES ('msg-u1', ?1, 'user', 'How do I configure cargo?', 'completed', 1010)",
            params![conv_id],
        )
        .expect("insert user msg");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, status, created_at)
             VALUES ('msg-a1', ?1, 'assistant', 'Here is how to configure cargo...', 'completed', 1020)",
            params![conv_id],
        )
        .expect("insert assistant msg");
        drop(conn);

        // 1 completed assistant turn with default title: should generate
        assert!(should_generate_automatic_title(&storage, conv_id).expect("should gen"));

        // Apply generated title
        let applied =
            apply_generated_title(&storage, conv_id, "Cargo Configuration").expect("apply");
        assert!(applied);

        let current_title: String = storage
            .connect()
            .expect("connect")
            .query_row(
                "SELECT title FROM conversations WHERE id = ?1",
                params![conv_id],
                |row| row.get(0),
            )
            .expect("query title");
        assert_eq!(current_title, "Cargo Configuration");

        // Now add second turn
        let conn = storage.connect().expect("connect");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, status, created_at)
             VALUES ('msg-u2', ?1, 'user', 'What about dependencies?', 'completed', 1030)",
            params![conv_id],
        )
        .expect("insert user msg 2");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, status, created_at)
             VALUES ('msg-a2', ?1, 'assistant', 'Dependencies are added under [dependencies]...', 'completed', 1040)",
            params![conv_id],
        )
        .expect("insert assistant msg 2");
        drop(conn);

        // Title is already "Cargo Configuration" (not default, not user prompt), turn count is 2: should NOT re-generate
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should not gen"));

        // If user manually edited the title, title_source becomes 'manual'
        let conn = storage.connect().expect("connect");
        conn.execute(
            "UPDATE conversations SET title = 'My Custom Title', title_source = 'manual' WHERE id = ?1",
            params![conv_id],
        )
        .expect("update manual");
        drop(conn);

        assert!(is_custom_title(&storage, conv_id).expect("is_custom"));
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should not gen"));

        // Automated titling must NEVER overwrite manual/custom title
        let overwritten =
            apply_generated_title(&storage, conv_id, "Another Auto Title").expect("apply");
        assert!(!overwritten);

        let final_title: String = storage
            .connect()
            .expect("connect")
            .query_row(
                "SELECT title FROM conversations WHERE id = ?1",
                params![conv_id],
                |row| row.get(0),
            )
            .expect("query title");
        assert_eq!(final_title, "My Custom Title");
    }
}
