use rusqlite::{Connection, OptionalExtension, params};

use super::unix_time_millis;

pub(super) fn ensure_archived_memory_indexes(
    connection: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<()> {
    if !archive_indexing_enabled(connection, conversation_id)? {
        return Ok(());
    }

    let is_backfilled = connection
        .query_row(
            "SELECT 1 FROM conversation_memory_index_state WHERE conversation_id = ?1",
            [conversation_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !is_backfilled {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO conversation_memory_fts (
                rowid, conversation_id, message_id, role, scope_token, content
             )
             SELECT rowid, conversation_id, id, role,
                    'scope' || lower(hex(CAST(conversation_id AS BLOB))), content
             FROM messages
             WHERE conversation_id = ?1
               AND role IN ('user', 'assistant')
               AND status = 'completed'
               AND NOT EXISTS (
                    SELECT 1 FROM conversation_memory_fts AS indexed
                    WHERE indexed.rowid = messages.rowid
               )",
            [conversation_id],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO conversation_memory_index_state (
                conversation_id, backfilled_at_unix_ms
             ) VALUES (?1, ?2)",
            params![conversation_id, unix_time_millis()?],
        )?;
        transaction.commit()?;
    }

    let is_tool_index_backfilled = connection
        .query_row(
            "SELECT 1 FROM conversation_memory_tool_index_state WHERE conversation_id = ?1",
            [conversation_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !is_tool_index_backfilled {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO conversation_memory_tools_fts (
                rowid, conversation_id, message_id, role, scope_token, content
             )
             SELECT message.rowid, message.conversation_id, message.id, message.role,
                    'scope' || lower(hex(CAST(message.conversation_id AS BLOB))),
                    (
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
                    )
             FROM messages AS message
             WHERE message.conversation_id = ?1
               AND message.role = 'assistant'
               AND message.status = 'completed'
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
               )
               AND NOT EXISTS (
                    SELECT 1 FROM conversation_memory_tools_fts AS indexed
                    WHERE indexed.rowid = message.rowid
               )",
            [conversation_id],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO conversation_memory_tool_index_state (
                conversation_id, backfilled_at_unix_ms
             ) VALUES (?1, ?2)",
            params![conversation_id, unix_time_millis()?],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

pub(super) fn archive_indexing_enabled(
    connection: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
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
