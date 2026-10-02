use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::storage::AppStorage;

#[derive(Clone, Debug)]
pub struct ConversationContextState {
    pub compaction_kind: Option<String>,
    pub compaction_payload: Option<String>,
    pub compaction_provider_id: Option<String>,
    pub compaction_connection_id: Option<String>,
    pub compaction_workspace_id: Option<String>,
    pub compaction_model_id: Option<String>,
    pub compacted_through_message_id: Option<String>,
    pub last_prompt_tokens: Option<i64>,
    pub last_prompt_message_id: Option<String>,
    pub last_prompt_provider_id: Option<String>,
    pub last_prompt_model_id: Option<String>,
    pub last_prompt_connection_id: Option<String>,
    pub last_prompt_workspace_id: Option<String>,
}

pub struct PromptUsage<'a> {
    pub input_tokens: i64,
    pub last_message_id: &'a str,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub connection_id: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
}

pub fn load_conversation_context_state(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Option<ConversationContextState>> {
    storage
        .connect()?
        .query_row(
            "SELECT compaction_kind, compaction_payload, compaction_provider_id,
                    compaction_connection_id, compaction_workspace_id,
                    compaction_model_id, compacted_through_message_id,
                    last_prompt_tokens, last_prompt_message_id,
                    last_prompt_provider_id, last_prompt_model_id,
                    last_prompt_connection_id, last_prompt_workspace_id
             FROM conversation_context_state WHERE conversation_id = ?1",
            [conversation_id],
            |row| {
                Ok(ConversationContextState {
                    compaction_kind: row.get(0)?,
                    compaction_payload: row.get(1)?,
                    compaction_provider_id: row.get(2)?,
                    compaction_connection_id: row.get(3)?,
                    compaction_workspace_id: row.get(4)?,
                    compaction_model_id: row.get(5)?,
                    compacted_through_message_id: row.get(6)?,
                    last_prompt_tokens: row.get(7)?,
                    last_prompt_message_id: row.get(8)?,
                    last_prompt_provider_id: row.get(9)?,
                    last_prompt_model_id: row.get(10)?,
                    last_prompt_connection_id: row.get(11)?,
                    last_prompt_workspace_id: row.get(12)?,
                })
            },
        )
        .optional()
}

pub fn save_compaction_state(
    storage: &AppStorage,
    conversation_id: &str,
    state: &ConversationContextState,
) -> rusqlite::Result<()> {
    let Some(kind) = state.compaction_kind.as_deref() else {
        return Err(rusqlite::Error::InvalidQuery);
    };
    let Some(payload) = state.compaction_payload.as_deref() else {
        return Err(rusqlite::Error::InvalidQuery);
    };
    let Some(through_message_id) = state.compacted_through_message_id.as_deref() else {
        return Err(rusqlite::Error::InvalidQuery);
    };
    let updated_at = unix_time_millis()?;
    storage.connect()?.execute(
        "INSERT INTO conversation_context_state (
            conversation_id, compaction_kind, compaction_payload,
            compaction_provider_id, compaction_connection_id,
            compaction_workspace_id, compaction_model_id,
            compacted_through_message_id, last_prompt_tokens,
            last_prompt_message_id, last_prompt_provider_id,
            last_prompt_model_id, last_prompt_connection_id,
            last_prompt_workspace_id, updated_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, NULL, NULL, NULL, ?9)
         ON CONFLICT(conversation_id) DO UPDATE SET
            compaction_kind = excluded.compaction_kind,
            compaction_payload = excluded.compaction_payload,
            compaction_provider_id = excluded.compaction_provider_id,
            compaction_connection_id = excluded.compaction_connection_id,
            compaction_workspace_id = excluded.compaction_workspace_id,
            compaction_model_id = excluded.compaction_model_id,
            compacted_through_message_id = excluded.compacted_through_message_id,
            last_prompt_tokens = NULL,
            last_prompt_message_id = NULL,
            last_prompt_provider_id = NULL,
            last_prompt_model_id = NULL,
            last_prompt_connection_id = NULL,
            last_prompt_workspace_id = NULL,
            updated_at_unix_ms = excluded.updated_at_unix_ms",
        params![
            conversation_id,
            kind,
            payload,
            state.compaction_provider_id,
            state.compaction_connection_id,
            state.compaction_workspace_id,
            state.compaction_model_id,
            through_message_id,
            updated_at,
        ],
    )?;
    Ok(())
}

pub fn reset_conversation_context(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    let connection = storage.connect()?;
    reset_conversation_context_from_connection(&connection, conversation_id)
}

fn reset_conversation_context_from_connection(
    connection: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    Ok(connection.execute(
        "DELETE FROM conversation_context_state WHERE conversation_id = ?1",
        [conversation_id],
    )? > 0)
}

pub fn save_prompt_usage(
    storage: &AppStorage,
    conversation_id: &str,
    usage: PromptUsage<'_>,
) -> rusqlite::Result<()> {
    if usage.input_tokens < 0 || usage.last_message_id.is_empty() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let updated_at = unix_time_millis()?;
    storage.connect()?.execute(
        "INSERT INTO conversation_context_state (
            conversation_id, last_prompt_tokens, last_prompt_message_id,
            last_prompt_provider_id, last_prompt_model_id,
            last_prompt_connection_id, last_prompt_workspace_id,
            updated_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(conversation_id) DO UPDATE SET
            last_prompt_tokens = excluded.last_prompt_tokens,
            last_prompt_message_id = excluded.last_prompt_message_id,
            last_prompt_provider_id = excluded.last_prompt_provider_id,
            last_prompt_model_id = excluded.last_prompt_model_id,
            last_prompt_connection_id = excluded.last_prompt_connection_id,
            last_prompt_workspace_id = excluded.last_prompt_workspace_id,
            updated_at_unix_ms = excluded.updated_at_unix_ms",
        params![
            conversation_id,
            usage.input_tokens,
            usage.last_message_id,
            usage.provider_id,
            usage.model_id,
            usage.connection_id,
            usage.workspace_id,
            updated_at,
        ],
    )?;
    Ok(())
}

fn unix_time_millis() -> rusqlite::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::reset_conversation_context_from_connection;

    #[test]
    fn resetting_context_removes_only_the_selected_conversation_state() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE conversation_context_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    compaction_payload TEXT,
                    last_prompt_tokens INTEGER
                );
                INSERT INTO conversation_context_state
                    (conversation_id, compaction_payload, last_prompt_tokens)
                VALUES ('selected', 'summary', 1200), ('other', 'keep', 800);",
            )
            .expect("create context state");

        assert!(
            reset_conversation_context_from_connection(&connection, "selected")
                .expect("reset selected conversation context")
        );
        assert!(
            !reset_conversation_context_from_connection(&connection, "missing")
                .expect("reset missing conversation context")
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT compaction_payload FROM conversation_context_state
                     WHERE conversation_id = 'other'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("read unrelated context state"),
            "keep"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_context_state",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count context states"),
            1
        );
    }
}
