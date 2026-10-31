//! SQLite FTS and credential redaction triggers for message and tool memory indexes.

pub(super) const REDACTED_TOOL_INDEX_TRIGGERS: &str = r#"
    DROP TRIGGER IF EXISTS conversation_memory_tool_insert;
    DROP TRIGGER IF EXISTS conversation_memory_tool_update;
    CREATE TRIGGER conversation_memory_tool_insert
    AFTER INSERT ON messages
    WHEN NEW.role = 'assistant'
        AND NEW.status = 'completed'
        AND COALESCE((
            SELECT included FROM conversation_memory_archive_settings
            WHERE conversation_id = NEW.conversation_id
        ), 1) = 1
        AND json_type(
            CASE WHEN json_valid(NEW.tool_activities)
                 THEN NEW.tool_activities ELSE '[]' END
        ) = 'array'
        AND EXISTS (
            SELECT 1 FROM json_each(
                CASE WHEN json_valid(NEW.tool_activities)
                     THEN NEW.tool_activities ELSE '[]' END
            ) AS activity
            WHERE json_type(activity.value, '$.output') IS NOT NULL
              AND json_extract(activity.value, '$.status')
                  IN ('completed', 'failed', 'denied', 'cancelled')
              AND NOT EXISTS (
                  SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                  WHERE excluded.conversation_id = NEW.conversation_id
                    AND excluded.tool_name = COALESCE(
                        json_extract(activity.value, '$.name'), ''
                    )
              )
        )
    BEGIN
        INSERT INTO conversation_memory_tools_fts (
            rowid, conversation_id, message_id, role, scope_token, content
        ) VALUES (
            NEW.rowid, NEW.conversation_id, NEW.id, NEW.role,
            'scope' || lower(hex(CAST(NEW.conversation_id AS BLOB))),
            openchat_redact_credentials((
                SELECT group_concat(
                    COALESCE(json_extract(activity.value, '$.name'), '') || ' ' ||
                    COALESCE(json_extract(activity.value, '$.arguments'), '') || ' ' ||
                    COALESCE(json_extract(activity.value, '$.output'), ''),
                    char(10)
                )
                FROM json_each(
                    CASE WHEN json_valid(NEW.tool_activities)
                         THEN NEW.tool_activities ELSE '[]' END
                ) AS activity
                WHERE json_type(activity.value, '$.output') IS NOT NULL
                  AND json_extract(activity.value, '$.status')
                      IN ('completed', 'failed', 'denied', 'cancelled')
                  AND NOT EXISTS (
                      SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                      WHERE excluded.conversation_id = NEW.conversation_id
                        AND excluded.tool_name = COALESCE(
                            json_extract(activity.value, '$.name'), ''
                        )
                  )
            ))
        );
    END;

    CREATE TRIGGER conversation_memory_tool_update
    AFTER UPDATE OF conversation_id, id, role, status, tool_activities ON messages
    BEGIN
        DELETE FROM conversation_memory_tools_fts WHERE rowid = OLD.rowid;
        INSERT INTO conversation_memory_tools_fts (
            rowid, conversation_id, message_id, role, scope_token, content
        )
        SELECT
            NEW.rowid, NEW.conversation_id, NEW.id, NEW.role,
            'scope' || lower(hex(CAST(NEW.conversation_id AS BLOB))),
            openchat_redact_credentials((
                SELECT group_concat(
                    COALESCE(json_extract(activity.value, '$.name'), '') || ' ' ||
                    COALESCE(json_extract(activity.value, '$.arguments'), '') || ' ' ||
                    COALESCE(json_extract(activity.value, '$.output'), ''),
                    char(10)
                )
                FROM json_each(
                    CASE WHEN json_valid(NEW.tool_activities)
                         THEN NEW.tool_activities ELSE '[]' END
                ) AS activity
                WHERE json_type(activity.value, '$.output') IS NOT NULL
                  AND json_extract(activity.value, '$.status')
                      IN ('completed', 'failed', 'denied', 'cancelled')
                  AND NOT EXISTS (
                      SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                      WHERE excluded.conversation_id = NEW.conversation_id
                        AND excluded.tool_name = COALESCE(
                            json_extract(activity.value, '$.name'), ''
                        )
                  )
            ))
        WHERE NEW.role = 'assistant'
          AND NEW.status = 'completed'
          AND COALESCE((
              SELECT included FROM conversation_memory_archive_settings
              WHERE conversation_id = NEW.conversation_id
          ), 1) = 1
          AND json_type(
              CASE WHEN json_valid(NEW.tool_activities)
                   THEN NEW.tool_activities ELSE '[]' END
          ) = 'array'
          AND EXISTS (
              SELECT 1 FROM json_each(
                  CASE WHEN json_valid(NEW.tool_activities)
                       THEN NEW.tool_activities ELSE '[]' END
              ) AS activity
              WHERE json_type(activity.value, '$.output') IS NOT NULL
                AND json_extract(activity.value, '$.status')
                    IN ('completed', 'failed', 'denied', 'cancelled')
                AND NOT EXISTS (
                    SELECT 1 FROM conversation_memory_excluded_tools AS excluded
                    WHERE excluded.conversation_id = NEW.conversation_id
                      AND excluded.tool_name = COALESCE(
                          json_extract(activity.value, '$.name'), ''
                      )
                )
          );
    END;
"#;

pub(super) const REDACT_AND_REBUILD_TOOL_INDEX: &str = r#"
    CREATE TEMP TABLE conversation_memory_redaction_affected AS
        SELECT conversation_id, message_id FROM conversation_memory_tools_fts;

    DELETE FROM conversation_memory_embeddings
    WHERE EXISTS (
        SELECT 1 FROM conversation_memory_redaction_affected AS affected
        WHERE affected.conversation_id = conversation_memory_embeddings.conversation_id
          AND affected.message_id = conversation_memory_embeddings.message_id
    );
    DELETE FROM conversation_memory_embedding_state
    WHERE EXISTS (
        SELECT 1 FROM conversation_memory_redaction_affected AS affected
        WHERE affected.conversation_id = conversation_memory_embedding_state.conversation_id
          AND affected.message_id = conversation_memory_embedding_state.message_id
    );

    DELETE FROM conversation_memory_tools_fts;
    INSERT INTO conversation_memory_tools_fts (
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
    WHERE message.role = 'assistant'
      AND message.status = 'completed'
      AND COALESCE((
          SELECT included FROM conversation_memory_archive_settings
          WHERE conversation_id = message.conversation_id
      ), 1) = 1
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
      );

    INSERT OR IGNORE INTO conversation_memory_embedding_pending (conversation_id, message_id)
        SELECT conversation_id, message_id FROM conversation_memory_tools_fts;
    DROP TABLE conversation_memory_redaction_affected;
    INSERT INTO conversation_memory_tools_fts(conversation_memory_tools_fts) VALUES ('optimize');
    UPDATE conversation_memory_semantic_index_state
       SET generation = generation + 1, indexed_generation = NULL
     WHERE id = 1;
"#;

pub(super) const MEMORY_INDEX_TRIGGERS: &str = r#"
    DROP TRIGGER IF EXISTS conversation_memory_message_insert;
    DROP TRIGGER IF EXISTS conversation_memory_message_update;
    DROP TRIGGER IF EXISTS conversation_memory_message_delete;
    CREATE TRIGGER conversation_memory_message_insert
    AFTER INSERT ON messages
    WHEN NEW.role IN ('user', 'assistant')
        AND NEW.status = 'completed'
        AND COALESCE((
            SELECT included FROM conversation_memory_archive_settings
            WHERE conversation_id = NEW.conversation_id
        ), 1) = 1
    BEGIN
        INSERT INTO conversation_memory_fts (
            rowid, conversation_id, message_id, role, scope_token, content
        ) VALUES (
            NEW.rowid, NEW.conversation_id, NEW.id, NEW.role,
            'scope' || lower(hex(CAST(NEW.conversation_id AS BLOB))), NEW.content
        );
    END;

    CREATE TRIGGER conversation_memory_message_update
    AFTER UPDATE OF conversation_id, id, role, content, status ON messages
    WHEN OLD.status = 'completed' OR NEW.status = 'completed'
    BEGIN
        DELETE FROM conversation_memory_fts WHERE rowid = OLD.rowid;
        INSERT INTO conversation_memory_fts (
            rowid, conversation_id, message_id, role, scope_token, content
        ) SELECT
            NEW.rowid, NEW.conversation_id, NEW.id, NEW.role,
            'scope' || lower(hex(CAST(NEW.conversation_id AS BLOB))), NEW.content
        WHERE NEW.role IN ('user', 'assistant') AND NEW.status = 'completed'
          AND COALESCE((
              SELECT included FROM conversation_memory_archive_settings
              WHERE conversation_id = NEW.conversation_id
          ), 1) = 1;
    END;

    CREATE TRIGGER conversation_memory_message_delete
    AFTER DELETE ON messages
    BEGIN
        DELETE FROM conversation_memory_fts WHERE rowid = OLD.rowid;
    END;

    DROP TRIGGER IF EXISTS conversation_memory_tool_delete;
    CREATE TRIGGER conversation_memory_tool_delete
    AFTER DELETE ON messages
    BEGIN
        DELETE FROM conversation_memory_tools_fts WHERE rowid = OLD.rowid;
    END;
"#;
