//! Repairs missing derived memory indexes, FTS tables, and SQLite triggers.

use rusqlite::Connection;
use super::SCHEMA_VERSION;

pub(super) fn schema_table_exists(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
        )",
        [name],
        |row| row.get(0),
    )
}

pub(super) fn schema_trigger_exists(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'trigger' AND name = ?1
        )",
        [name],
        |row| row.get(0),
    )
}

pub(super) fn repair_missing_memory_indexes_and_triggers(
    connection: &Connection,
    backend_version: i64,
) -> rusqlite::Result<bool> {
    if backend_version < 13 || !schema_table_exists(connection, "messages")? {
        return Ok(false);
    }
    let missing_message_index = !schema_table_exists(connection, "conversation_memory_fts")?;
    let missing_tool_index =
        backend_version >= 14 && !schema_table_exists(connection, "conversation_memory_tools_fts")?;
    let missing_memory_triggers = if backend_version >= SCHEMA_VERSION {
        let mut missing = false;
        for name in [
            "conversation_memory_message_insert",
            "conversation_memory_message_update",
            "conversation_memory_message_delete",
            "conversation_memory_tool_insert",
            "conversation_memory_tool_update",
            "conversation_memory_tool_delete",
        ] {
            if !schema_trigger_exists(connection, name)? {
                missing = true;
                break;
            }
        }
        missing
    } else {
        false
    };
    if !missing_message_index && !missing_tool_index && !missing_memory_triggers {
        return Ok(false);
    }

    let has_archive_settings =
        schema_table_exists(connection, "conversation_memory_archive_settings")?;
    let has_excluded_tools = schema_table_exists(connection, "conversation_memory_excluded_tools")?;
    let archive_filter = if has_archive_settings {
        "AND COALESCE((SELECT included FROM conversation_memory_archive_settings WHERE conversation_id = message.conversation_id), 1) = 1"
    } else {
        ""
    };
    let excluded_tool_filter = if has_excluded_tools {
        "AND NOT EXISTS (SELECT 1 FROM conversation_memory_excluded_tools AS excluded WHERE excluded.conversation_id = message.conversation_id AND excluded.tool_name = COALESCE(json_extract(activity.value, '$.name'), ''))"
    } else {
        ""
    };

    let transaction = connection.unchecked_transaction()?;
    if missing_tool_index {
        transaction.execute_batch(
            "CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                conversation_id UNINDEXED,
                message_id UNINDEXED,
                role UNINDEXED,
                scope_token,
                content,
                tokenize = 'unicode61 remove_diacritics 2'
            );",
        )?;
    }
    if missing_message_index {
        transaction.execute_batch(
            "CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                conversation_id UNINDEXED,
                message_id UNINDEXED,
                role UNINDEXED,
                scope_token,
                content,
                tokenize = 'unicode61 remove_diacritics 2'
            );",
        )?;
    }

    if missing_message_index {
        let rebuild_message_index = format!(
            "INSERT INTO conversation_memory_fts (
                rowid, conversation_id, message_id, role, scope_token, content
            )
            SELECT message.rowid, message.conversation_id, message.id, message.role,
                   'scope' || lower(hex(CAST(message.conversation_id AS BLOB))),
                   message.content
              FROM messages AS message
             WHERE message.role IN ('user', 'assistant')
               AND message.status = 'completed'
               {archive_filter};"
        );
        transaction.execute_batch(&rebuild_message_index)?;
    }

    if missing_tool_index {
        let rebuild_tool_index = format!(
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
                         {excluded_tool_filter}
                   ))
              FROM messages AS message
             WHERE message.role = 'assistant'
               AND message.status = 'completed'
               AND EXISTS (
                   SELECT 1
                     FROM json_each(
                         CASE WHEN json_valid(message.tool_activities)
                              THEN message.tool_activities ELSE '[]' END
                     ) AS activity
                    WHERE json_type(activity.value, '$.output') IS NOT NULL
                      AND json_extract(activity.value, '$.status')
                          IN ('completed', 'failed', 'denied', 'cancelled')
                      {excluded_tool_filter}
               )
               {archive_filter};"
        );
        transaction.execute_batch(&rebuild_tool_index)?;
    }
    transaction.commit()?;
    Ok(true)
}
