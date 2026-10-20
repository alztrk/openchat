//! Persists conversation routes, messages, title jobs, and tool checkpoints.
//!
//! Database transactions keep assistant messages and associated tool state
//! consistent when a provider stream is interrupted or completed.

use rusqlite::{OptionalExtension, Row, params, types::Type};

use crate::{
    provider_schema::{ChatStreamSnapshot, ToolActivity},
    storage::AppStorage,
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

pub fn is_custom_title(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
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

pub fn should_generate_automatic_title(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    let database = storage.connect()?;
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
    let existing_content = database
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
            content,
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

pub fn save_assistant_tool_checkpoint(
    storage: &AppStorage,
    snapshot: &ChatStreamSnapshot,
    activities: &[ToolActivity],
) -> rusqlite::Result<()> {
    let activities_json = serde_json::to_string(activities)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let database = storage.connect()?;
    let existing_content = database
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
    database.execute(
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

        assert!(!is_custom_title(&storage, conv_id).expect("is_custom"));
        // 0 completed assistant turns: should not generate
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should gen"));

        // Add 1 completed assistant turn
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

        // 1 completed assistant turn with default title: should generate
        assert!(should_generate_automatic_title(&storage, conv_id).expect("should gen"));

        // Apply generated title
        let applied = apply_generated_title(&storage, conv_id, "Cargo Configuration").expect("apply");
        assert!(applied);

        let current_title: String = conn
            .query_row("SELECT title FROM conversations WHERE id = ?1", params![conv_id], |row| row.get(0))
            .expect("query title");
        assert_eq!(current_title, "Cargo Configuration");

        // Now add second turn
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

        // Title is already "Cargo Configuration" (not default, not user prompt), turn count is 2: should NOT re-generate
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should not gen"));

        // If user manually edited the title, title_source becomes 'manual'
        conn.execute(
            "UPDATE conversations SET title = 'My Custom Title', title_source = 'manual' WHERE id = ?1",
            params![conv_id],
        )
        .expect("update manual");

        assert!(is_custom_title(&storage, conv_id).expect("is_custom"));
        assert!(!should_generate_automatic_title(&storage, conv_id).expect("should not gen"));

        // Automated titling must NEVER overwrite manual/custom title
        let overwritten = apply_generated_title(&storage, conv_id, "Another Auto Title").expect("apply");
        assert!(!overwritten);

        let final_title: String = conn
            .query_row("SELECT title FROM conversations WHERE id = ?1", params![conv_id], |row| row.get(0))
            .expect("query title");
        assert_eq!(final_title, "My Custom Title");
    }
}
