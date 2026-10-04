use rusqlite::{Connection, OptionalExtension, params};

const MAX_ARCHIVE_TOOLS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveIndexSettings {
    pub included: bool,
    pub tools: Vec<ArchiveIndexTool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveIndexTool {
    pub name: String,
    pub included: bool,
}

pub(super) fn load(
    connection: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<ArchiveIndexSettings> {
    let included = archive_enabled(connection, conversation_id)?;
    let mut statement = connection.prepare(
        "SELECT discovered.tool_name,
                NOT EXISTS (
                    SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                    WHERE excluded.conversation_id = ?1
                      AND excluded.tool_name = discovered.tool_name
                )
         FROM (
             SELECT DISTINCT json_extract(activity.value, '$.name') AS tool_name
             FROM messages AS message
             JOIN json_each(
                 CASE WHEN json_valid(message.tool_activities)
                      THEN message.tool_activities ELSE '[]' END
             ) AS activity
             WHERE message.conversation_id = ?1
               AND message.role = 'assistant' AND message.status = 'completed'
               AND json_type(activity.value, '$.output') IS NOT NULL
               AND json_extract(activity.value, '$.status')
                   IN ('completed', 'failed', 'denied', 'cancelled')
               AND json_type(activity.value, '$.name') = 'text'
               AND length(trim(json_extract(activity.value, '$.name'))) BETWEEN 1 AND 128
         ) AS discovered
         ORDER BY discovered.tool_name COLLATE NOCASE, discovered.tool_name
         LIMIT ?2",
    )?;
    let tools = statement
        .query_map(params![conversation_id, MAX_ARCHIVE_TOOLS as i64], |row| {
            Ok(ArchiveIndexTool {
                name: row.get(0)?,
                included: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ArchiveIndexSettings { included, tools })
}

pub(super) fn set_conversation_included(
    connection: &mut Connection,
    conversation_id: &str,
    included: bool,
) -> rusqlite::Result<(ArchiveIndexSettings, bool)> {
    let transaction = connection.transaction()?;
    require_conversation(&transaction, conversation_id)?;
    let was_included = archive_enabled(&transaction, conversation_id)?;
    if was_included != included {
        if included {
            transaction.execute(
                "DELETE FROM conversation_memory_archive_settings WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_backfill WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO conversation_memory_embedding_pending (
                    conversation_id, message_id
                 )
                 SELECT conversation_id, id FROM messages
                 WHERE conversation_id = ?1
                   AND role IN ('user', 'assistant') AND status = 'completed'",
                [conversation_id],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO conversation_memory_archive_settings (conversation_id, included)
                 VALUES (?1, 0)
                 ON CONFLICT(conversation_id) DO UPDATE SET included = excluded.included",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_fts WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_tools_fts WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_index_state WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_tool_index_state WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_pending WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embeddings WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_state WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_backfill WHERE conversation_id = ?1",
                [conversation_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_namespaces WHERE conversation_id = ?1",
                [conversation_id],
            )?;
        }
    }
    transaction.commit()?;
    Ok((load(connection, conversation_id)?, was_included != included))
}

pub(super) fn set_tool_included(
    connection: &mut Connection,
    conversation_id: &str,
    tool_name: &str,
    included: bool,
) -> rusqlite::Result<(ArchiveIndexSettings, bool)> {
    let tool_name = tool_name.trim();
    if tool_name.is_empty() || tool_name.chars().count() > 128 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let transaction = connection.transaction()?;
    require_conversation(&transaction, conversation_id)?;
    let tool_exists = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1
             FROM messages AS message
             JOIN json_each(
                 CASE WHEN json_valid(message.tool_activities)
                      THEN message.tool_activities ELSE '[]' END
             ) AS activity
             WHERE message.conversation_id = ?1
               AND message.role = 'assistant' AND message.status = 'completed'
               AND json_type(activity.value, '$.output') IS NOT NULL
               AND json_extract(activity.value, '$.status')
                   IN ('completed', 'failed', 'denied', 'cancelled')
               AND json_extract(activity.value, '$.name') = ?2
         )",
        params![conversation_id, tool_name],
        |row| row.get::<_, bool>(0),
    )?;
    if !tool_exists {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let was_included = transaction
        .query_row(
            "SELECT 1 FROM conversation_memory_excluded_tools
             WHERE conversation_id = ?1 AND tool_name = ?2",
            params![conversation_id, tool_name],
            |_| Ok(()),
        )
        .optional()?
        .is_none();

    if was_included != included {
        if included {
            transaction.execute(
                "DELETE FROM conversation_memory_excluded_tools
                 WHERE conversation_id = ?1 AND tool_name = ?2",
                params![conversation_id, tool_name],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO conversation_memory_excluded_tools (conversation_id, tool_name)
                 VALUES (?1, ?2)",
                params![conversation_id, tool_name],
            )?;
        }

        if archive_enabled(&transaction, conversation_id)? {
            rebuild_tool_index(&transaction, conversation_id)?;
            requeue_tool_embeddings(&transaction, conversation_id, tool_name)?;
        }
    }
    transaction.commit()?;
    Ok((load(connection, conversation_id)?, was_included != included))
}

fn archive_enabled(connection: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            "SELECT included FROM conversation_memory_archive_settings
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()
        .map(|included| included.unwrap_or(true))
}

fn require_conversation(connection: &Connection, conversation_id: &str) -> rusqlite::Result<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
        [conversation_id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(rusqlite::Error::QueryReturnedNoRows)
    }
}

fn rebuild_tool_index(connection: &Connection, conversation_id: &str) -> rusqlite::Result<()> {
    crate::storage::tool_index_redaction::register_sqlite_function(connection)?;
    connection.execute(
        "DELETE FROM conversation_memory_tools_fts WHERE conversation_id = ?1",
        [conversation_id],
    )?;
    connection.execute(
        "INSERT INTO conversation_memory_tools_fts (
            rowid, conversation_id, message_id, role, scope_token, content
         )
         SELECT message.rowid, message.conversation_id, message.id, message.role,
                'scope' || lower(hex(CAST(message.conversation_id AS BLOB))),
                openchat_redact_credentials((
                    SELECT group_concat(
                        COALESCE(json_extract(activity.value, '$.name'), '') || ' ' ||
                        COALESCE(json_extract(activity.value, '$.arguments'), '') || ' ' ||
                        COALESCE(json_extract(activity.value, '$.output'), ''),
                        char(10)
                    )
                    FROM json_each(
                        CASE WHEN json_valid(message.tool_activities)
                             THEN message.tool_activities ELSE '[]' END
                    ) AS activity
                    WHERE json_type(activity.value, '$.output') IS NOT NULL
                      AND json_extract(activity.value, '$.status')
                          IN ('completed', 'failed', 'denied', 'cancelled')
                      AND NOT EXISTS (
                          SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                          WHERE excluded.conversation_id = message.conversation_id
                            AND excluded.tool_name = COALESCE(
                                json_extract(activity.value, '$.name'), ''
                            )
                      )
                ))
         FROM messages AS message
         WHERE message.conversation_id = ?1
           AND message.role = 'assistant' AND message.status = 'completed'
           AND json_type(
               CASE WHEN json_valid(message.tool_activities)
                    THEN message.tool_activities ELSE '[]' END
           ) = 'array'
           AND EXISTS (
               SELECT 1 FROM json_each(
                   CASE WHEN json_valid(message.tool_activities)
                        THEN message.tool_activities ELSE '[]' END
               ) AS activity
               WHERE json_type(activity.value, '$.output') IS NOT NULL
                 AND json_extract(activity.value, '$.status')
                     IN ('completed', 'failed', 'denied', 'cancelled')
                 AND NOT EXISTS (
                     SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                     WHERE excluded.conversation_id = message.conversation_id
                       AND excluded.tool_name = COALESCE(
                           json_extract(activity.value, '$.name'), ''
                       )
                 )
           )",
        [conversation_id],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO conversation_memory_tool_index_state (
            conversation_id, backfilled_at_unix_ms
         ) VALUES (?1, 0)",
        [conversation_id],
    )?;
    Ok(())
}

fn requeue_tool_embeddings(
    connection: &Connection,
    conversation_id: &str,
    tool_name: &str,
) -> rusqlite::Result<()> {
    let matches_tool = "EXISTS (
        SELECT 1 FROM json_each(
            CASE WHEN json_valid(message.tool_activities)
                 THEN message.tool_activities ELSE '[]' END
        ) AS activity
        WHERE json_type(activity.value, '$.output') IS NOT NULL
          AND json_extract(activity.value, '$.status')
              IN ('completed', 'failed', 'denied', 'cancelled')
          AND json_extract(activity.value, '$.name') = ?2
    )";
    for table in [
        "conversation_memory_embedding_pending",
        "conversation_memory_embedding_state",
        "conversation_memory_embeddings",
    ] {
        let sql = format!(
            "DELETE FROM {table} WHERE conversation_id = ?1 AND message_id IN (
                SELECT message.id FROM messages AS message
                WHERE message.conversation_id = ?1 AND message.role = 'assistant'
                  AND message.status = 'completed' AND {matches_tool}
            )"
        );
        connection.execute(&sql, params![conversation_id, tool_name])?;
    }
    connection.execute(
        "INSERT OR IGNORE INTO conversation_memory_embedding_pending (
            conversation_id, message_id
         )
         SELECT message.conversation_id, message.id
         FROM messages AS message
         WHERE message.conversation_id = ?1 AND message.role = 'assistant'
           AND message.status = 'completed'
           AND EXISTS (
               SELECT 1 FROM json_each(
                   CASE WHEN json_valid(message.tool_activities)
                        THEN message.tool_activities ELSE '[]' END
               ) AS activity
               WHERE json_type(activity.value, '$.output') IS NOT NULL
                 AND json_extract(activity.value, '$.status')
                     IN ('completed', 'failed', 'denied', 'cancelled')
                 AND json_extract(activity.value, '$.name') = ?2
           )",
        params![conversation_id, tool_name],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{load, set_conversation_included, set_tool_included};

    fn connection_with_archive() -> Connection {
        let connection = Connection::open_in_memory().expect("open archive settings database");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
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
                 CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                 );
                 CREATE TABLE conversation_memory_excluded_tools (
                    conversation_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, tool_name)
                 );
                 CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                    scope_token, content
                 );
                 CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                    scope_token, content
                 );
                 CREATE TABLE conversation_memory_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                 );
                 CREATE TABLE conversation_memory_tool_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                 );
                 CREATE TABLE conversation_memory_embedding_pending (
                    conversation_id TEXT NOT NULL, message_id TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, message_id)
                 );
                 CREATE TABLE conversation_memory_embeddings (
                    conversation_id TEXT NOT NULL, message_id TEXT NOT NULL,
                    chunk_index INTEGER NOT NULL, start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL, model_id TEXT NOT NULL,
                    content_hash TEXT NOT NULL, vector BLOB NOT NULL
                 );
                 CREATE TABLE conversation_memory_embedding_state (
                    conversation_id TEXT NOT NULL, message_id TEXT NOT NULL,
                    model_id TEXT NOT NULL, content_hash TEXT NOT NULL,
                    chunk_count INTEGER NOT NULL
                 );
                 CREATE TABLE conversation_memory_embedding_backfill (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    cursor_created_at INTEGER, cursor_message_id TEXT
                 );
                 CREATE TABLE conversation_memory_embedding_namespaces (
                    conversation_id TEXT PRIMARY KEY NOT NULL
                 );
                 INSERT INTO conversations (id) VALUES ('conversation-a');",
            )
            .expect("create archive preference schema");
        connection
    }

    #[test]
    fn conversation_exclusion_clears_derived_data_and_reenable_queues_history() {
        let mut connection = connection_with_archive();
        connection
            .execute_batch(
                "INSERT INTO messages
                    (conversation_id, id, role, content, status, created_at)
                 VALUES
                    ('conversation-a', 'user-1', 'user', 'hello', 'completed', 1),
                    ('conversation-a', 'assistant-1', 'assistant', 'answer', 'completed', 2);
                 INSERT INTO conversation_memory_fts
                    (conversation_id, message_id, role, scope_token, content)
                 VALUES ('conversation-a', 'user-1', 'user', 'scope', 'hello');
                 INSERT INTO conversation_memory_tools_fts
                    (conversation_id, message_id, role, scope_token, content)
                 VALUES ('conversation-a', 'assistant-1', 'assistant', 'scope', 'tool result');
                 INSERT INTO conversation_memory_index_state VALUES ('conversation-a', 1);
                 INSERT INTO conversation_memory_tool_index_state VALUES ('conversation-a', 1);
                 INSERT INTO conversation_memory_embedding_pending VALUES ('conversation-a', 'user-1');
                 INSERT INTO conversation_memory_embeddings VALUES
                    ('conversation-a', 'user-1', 0, 0, 5, 'model', 'hash', zeroblob(4));
                 INSERT INTO conversation_memory_embedding_state VALUES
                    ('conversation-a', 'user-1', 'model', 'hash', 1);
                 INSERT INTO conversation_memory_embedding_backfill
                    (conversation_id) VALUES ('conversation-a');
                 INSERT INTO conversation_memory_embedding_namespaces VALUES ('conversation-a');",
            )
            .expect("seed derived archive data");

        let (excluded, changed) =
            set_conversation_included(&mut connection, "conversation-a", false)
                .expect("exclude conversation from archive indexing");
        assert!(changed);
        assert!(!excluded.included);
        for table in [
            "conversation_memory_fts",
            "conversation_memory_tools_fts",
            "conversation_memory_index_state",
            "conversation_memory_tool_index_state",
            "conversation_memory_embedding_pending",
            "conversation_memory_embeddings",
            "conversation_memory_embedding_state",
            "conversation_memory_embedding_backfill",
            "conversation_memory_embedding_namespaces",
        ] {
            let count = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count cleared archive data");
            assert_eq!(count, 0, "{table} should be cleared");
        }

        let (included, changed) =
            set_conversation_included(&mut connection, "conversation-a", true)
                .expect("reinclude conversation in archive indexing");
        assert!(changed);
        assert!(included.included);
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_embedding_pending
                     WHERE conversation_id = 'conversation-a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count requeued messages"),
            2
        );
        assert!(
            load(&connection, "conversation-a")
                .expect("load restored settings")
                .included
        );
    }

    #[test]
    fn tool_exclusion_rebuilds_search_index_and_requeues_assistant_embedding() {
        let mut connection = connection_with_archive();
        connection
            .execute(
                "INSERT INTO messages
                    (conversation_id, id, role, content, status, created_at, tool_activities)
                 VALUES (?1, ?2, 'assistant', 'assistant response', 'completed', 1, ?3)",
                rusqlite::params![
                    "conversation-a",
                    "assistant-1",
                    r#"[{"name":"read","arguments":{"path":"private.json"},"output":{"content":"readsecretword"},"status":"completed"},{"name":"write","arguments":{},"output":{"content":"writevisibleword"},"status":"completed"}]"#,
                ],
            )
            .expect("insert tool activity message");
        connection
            .execute(
                "INSERT INTO conversation_memory_embedding_state
                    VALUES ('conversation-a', 'assistant-1', 'model', 'hash', 1)",
                [],
            )
            .expect("insert assistant embedding state");
        connection
            .execute(
                "INSERT INTO conversation_memory_embeddings
                    VALUES ('conversation-a', 'assistant-1', 0, 0, 18, 'model', 'hash', zeroblob(4))",
                [],
            )
            .expect("insert assistant embedding");

        let (settings, changed) =
            set_tool_included(&mut connection, "conversation-a", "read", false)
                .expect("exclude read tool");
        assert!(changed);
        assert!(settings.included);
        assert_eq!(settings.tools.len(), 2);
        assert!(
            settings
                .tools
                .iter()
                .find(|tool| tool.name == "read")
                .is_some_and(|tool| !tool.included)
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_tools_fts
                     WHERE conversation_memory_tools_fts MATCH 'content : \"readsecretword\"'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("search excluded tool output"),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_tools_fts
                     WHERE conversation_memory_tools_fts MATCH 'content : \"writevisibleword\"'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("search included tool output"),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_embedding_pending
                     WHERE conversation_id = 'conversation-a' AND message_id = 'assistant-1'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count requeued assistant message"),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_embeddings",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count removed embeddings"),
            0
        );
    }
}
