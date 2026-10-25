//! Agent execution, checkpoints, and extended storage migrations.

use super::repair::schema_table_exists;
use super::triggers::{
    MEMORY_INDEX_TRIGGERS, REDACT_AND_REBUILD_TOOL_INDEX, REDACTED_TOOL_INDEX_TRIGGERS,
};
use super::unix_time_millis;
use rusqlite::Connection;

pub(super) fn apply_agent_migrations(
    connection: &Connection,
    mut current_version: i64,
    target_version: i64,
) -> rusqlite::Result<i64> {
    if current_version < 16 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE agent_runs (
                id TEXT PRIMARY KEY NOT NULL,
                conversation_id TEXT NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                status TEXT NOT NULL CHECK (
                    status IN ('running', 'paused', 'completed', 'cancelled',
                               'interrupted', 'failed')
                ),
                checkpoint_json TEXT CHECK (
                    checkpoint_json IS NULL OR (
                        json_valid(checkpoint_json) = 1
                        AND length(checkpoint_json) <= 262144
                    )
                ),
                checkpoint_revision INTEGER NOT NULL DEFAULT 0
                    CHECK (checkpoint_revision >= 0),
                checkpoint_updated_at_unix_ms INTEGER,
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                finished_at_unix_ms INTEGER,
                CHECK (
                    (checkpoint_json IS NULL AND checkpoint_updated_at_unix_ms IS NULL)
                    OR (checkpoint_json IS NOT NULL AND checkpoint_updated_at_unix_ms IS NOT NULL)
                ),
                CHECK (
                    (status IN ('completed', 'cancelled', 'failed')
                        AND finished_at_unix_ms IS NOT NULL)
                    OR (status NOT IN ('completed', 'cancelled', 'failed')
                        AND finished_at_unix_ms IS NULL)
                )
            );

            CREATE INDEX agent_runs_conversation_idx
                ON agent_runs(conversation_id, created_at_unix_ms DESC);
            CREATE INDEX agent_runs_status_idx
                ON agent_runs(status, updated_at_unix_ms DESC);

            CREATE TABLE pending_question_groups (
                id TEXT PRIMARY KEY NOT NULL,
                run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL CHECK (sequence > 0),
                status TEXT NOT NULL CHECK (
                    status IN ('pending', 'answered', 'cancelled', 'expired')
                ),
                revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
                answers_json TEXT CHECK (
                    answers_json IS NULL OR (
                        json_valid(answers_json) = 1
                        AND length(answers_json) <= 131072
                    )
                ),
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                answered_at_unix_ms INTEGER,
                UNIQUE (run_id, sequence),
                CHECK (
                    (status = 'answered'
                        AND answers_json IS NOT NULL
                        AND answered_at_unix_ms IS NOT NULL)
                    OR (status <> 'answered'
                        AND answers_json IS NULL
                        AND answered_at_unix_ms IS NULL)
                )
            );

            CREATE INDEX pending_question_groups_run_idx
                ON pending_question_groups(run_id, status, sequence);

            CREATE TABLE pending_question_items (
                group_id TEXT NOT NULL
                    REFERENCES pending_question_groups(id) ON DELETE CASCADE,
                item_id TEXT PRIMARY KEY NOT NULL,
                position INTEGER NOT NULL CHECK (position >= 0),
                question_json TEXT NOT NULL CHECK (
                    json_valid(question_json) = 1
                    AND length(question_json) <= 65536
                ),
                UNIQUE (group_id, position)
            );

            CREATE INDEX pending_question_items_group_idx
                ON pending_question_items(group_id, position);

            CREATE TRIGGER agent_runs_status_guard
            BEFORE UPDATE OF status ON agent_runs
            WHEN OLD.status <> NEW.status
                AND NOT (
                    (OLD.status = 'running'
                        AND NEW.status IN ('paused', 'completed', 'cancelled',
                                           'interrupted', 'failed'))
                    OR (OLD.status = 'paused'
                        AND NEW.status IN ('running', 'cancelled', 'interrupted'))
                    OR (OLD.status = 'interrupted'
                        AND NEW.status IN ('running', 'cancelled'))
                )
            BEGIN
                SELECT RAISE(ABORT, 'invalid agent run status transition');
            END;

            CREATE TRIGGER pending_question_groups_status_guard
            BEFORE UPDATE OF status ON pending_question_groups
            WHEN OLD.status <> NEW.status
                AND NOT (
                    OLD.status = 'pending'
                    AND NEW.status IN ('answered', 'cancelled', 'expired')
                )
            BEGIN
                SELECT RAISE(ABORT, 'invalid question group status transition');
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (16, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 16;
    }

    if current_version >= target_version {
        return Ok(current_version);
    }

    if current_version < 17 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "DROP TRIGGER agent_runs_status_guard;

            CREATE TRIGGER agent_runs_status_guard
            BEFORE UPDATE OF status ON agent_runs
            WHEN OLD.status <> NEW.status
                AND NOT (
                    (OLD.status = 'running'
                        AND NEW.status IN ('paused', 'completed', 'cancelled',
                                           'interrupted', 'failed'))
                    OR (OLD.status = 'paused'
                        AND NEW.status IN ('running', 'cancelled', 'interrupted', 'failed'))
                    OR (OLD.status = 'interrupted'
                        AND NEW.status IN ('running', 'cancelled', 'failed'))
                )
            BEGIN
                SELECT RAISE(ABORT, 'invalid agent run status transition');
            END;

            CREATE TRIGGER agent_runs_terminal_pending_guard
            BEFORE UPDATE OF status ON agent_runs
            WHEN NEW.status IN ('completed', 'cancelled', 'failed')
                AND EXISTS (
                    SELECT 1 FROM pending_question_groups
                    WHERE run_id = NEW.id AND status = 'pending'
                )
            BEGIN
                SELECT RAISE(ABORT, 'terminal agent run cannot have pending questions');
            END;

            CREATE TRIGGER pending_question_groups_run_state_guard
            BEFORE INSERT ON pending_question_groups
            WHEN COALESCE(
                (SELECT status FROM agent_runs WHERE id = NEW.run_id), ''
            ) NOT IN ('paused', 'interrupted')
            BEGIN
                SELECT RAISE(ABORT, 'pending questions require a paused or interrupted run');
            END;

            CREATE TRIGGER agent_runs_checkpoint_byte_guard_insert
            BEFORE INSERT ON agent_runs
            WHEN NEW.checkpoint_json IS NOT NULL
                AND length(CAST(NEW.checkpoint_json AS BLOB)) > 262144
            BEGIN
                SELECT RAISE(ABORT, 'agent run checkpoint exceeds its byte limit');
            END;

            CREATE TRIGGER agent_runs_checkpoint_byte_guard_update
            BEFORE UPDATE OF checkpoint_json ON agent_runs
            WHEN NEW.checkpoint_json IS NOT NULL
                AND length(CAST(NEW.checkpoint_json AS BLOB)) > 262144
            BEGIN
                SELECT RAISE(ABORT, 'agent run checkpoint exceeds its byte limit');
            END;

            CREATE TRIGGER pending_question_groups_answers_byte_guard_insert
            BEFORE INSERT ON pending_question_groups
            WHEN NEW.answers_json IS NOT NULL
                AND length(CAST(NEW.answers_json AS BLOB)) > 131072
            BEGIN
                SELECT RAISE(ABORT, 'question answers exceed their byte limit');
            END;

            CREATE TRIGGER pending_question_groups_answers_byte_guard_update
            BEFORE UPDATE OF answers_json ON pending_question_groups
            WHEN NEW.answers_json IS NOT NULL
                AND length(CAST(NEW.answers_json AS BLOB)) > 131072
            BEGIN
                SELECT RAISE(ABORT, 'question answers exceed their byte limit');
            END;

            CREATE TRIGGER pending_question_items_question_byte_guard_insert
            BEFORE INSERT ON pending_question_items
            WHEN length(CAST(NEW.question_json AS BLOB)) > 65536
            BEGIN
                SELECT RAISE(ABORT, 'question item exceeds its byte limit');
            END;

            CREATE TRIGGER pending_question_items_question_byte_guard_update
            BEFORE UPDATE OF question_json ON pending_question_items
            WHEN length(CAST(NEW.question_json AS BLOB)) > 65536
            BEGIN
                SELECT RAISE(ABORT, 'question item exceeds its byte limit');
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (17, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 17;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 18 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "DROP INDEX pending_question_items_group_idx;
            ALTER TABLE pending_question_items
                RENAME TO pending_question_items_v17;

            CREATE TABLE pending_question_items (
                group_id TEXT NOT NULL
                    REFERENCES pending_question_groups(id) ON DELETE CASCADE,
                item_id TEXT NOT NULL,
                position INTEGER NOT NULL CHECK (position >= 0),
                question_json TEXT NOT NULL CHECK (
                    json_valid(question_json) = 1
                    AND length(question_json) <= 65536
                ),
                PRIMARY KEY (group_id, item_id),
                UNIQUE (group_id, position)
            );

            INSERT INTO pending_question_items (group_id, item_id, position, question_json)
            SELECT group_id, item_id, position, question_json
            FROM pending_question_items_v17;

            DROP TABLE pending_question_items_v17;

            CREATE INDEX pending_question_items_group_idx
                ON pending_question_items(group_id, position);

            CREATE TRIGGER pending_question_items_question_byte_guard_insert
            BEFORE INSERT ON pending_question_items
            WHEN length(CAST(NEW.question_json AS BLOB)) > 65536
            BEGIN
                SELECT RAISE(ABORT, 'question item exceeds its byte limit');
            END;

            CREATE TRIGGER pending_question_items_question_byte_guard_update
            BEFORE UPDATE OF question_json ON pending_question_items
            WHEN length(CAST(NEW.question_json AS BLOB)) > 65536
            BEGIN
                SELECT RAISE(ABORT, 'question item exceeds its byte limit');
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (18, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 18;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 19 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_memory_archive_settings (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                included INTEGER NOT NULL CHECK (included IN (0, 1))
            );

            CREATE TABLE conversation_memory_excluded_tools (
                conversation_id TEXT NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                tool_name TEXT NOT NULL CHECK (length(trim(tool_name)) BETWEEN 1 AND 128),
                PRIMARY KEY (conversation_id, tool_name)
            );

            DROP TRIGGER IF EXISTS conversation_memory_message_insert;
            DROP TRIGGER IF EXISTS conversation_memory_message_update;
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
                    (
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
                    )
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
                    (
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
                    )
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

            DROP TRIGGER IF EXISTS conversation_memory_embedding_message_insert;
            DROP TRIGGER IF EXISTS conversation_memory_embedding_message_update;
            CREATE TRIGGER conversation_memory_embedding_message_insert
            AFTER INSERT ON messages
            WHEN NEW.role IN ('user', 'assistant') AND NEW.status = 'completed'
                AND COALESCE((
                    SELECT included FROM conversation_memory_archive_settings
                    WHERE conversation_id = NEW.conversation_id
                ), 1) = 1
            BEGIN
                INSERT OR IGNORE INTO conversation_memory_embedding_pending (
                    conversation_id, message_id
                ) VALUES (NEW.conversation_id, NEW.id);
            END;

            CREATE TRIGGER conversation_memory_embedding_message_update
            AFTER UPDATE OF conversation_id, id, role, content, status, tool_activities ON messages
            BEGIN
                DELETE FROM conversation_memory_embedding_pending
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
                DELETE FROM conversation_memory_embedding_state
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
                DELETE FROM conversation_memory_embeddings
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
                INSERT OR IGNORE INTO conversation_memory_embedding_pending (
                    conversation_id, message_id
                ) SELECT NEW.conversation_id, NEW.id
                    WHERE NEW.role IN ('user', 'assistant') AND NEW.status = 'completed'
                      AND COALESCE((
                          SELECT included FROM conversation_memory_archive_settings
                          WHERE conversation_id = NEW.conversation_id
                      ), 1) = 1;
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (19, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 19;
    }

    if current_version < 20 && target_version >= 20 {
        let transaction = connection.unchecked_transaction()?;
        if !schema_table_exists(&transaction, "conversation_memory_tools_fts")? {
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
        transaction.execute_batch(REDACTED_TOOL_INDEX_TRIGGERS)?;
        transaction.execute_batch(REDACT_AND_REBUILD_TOOL_INDEX)?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (20, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 20;
    }

    if current_version < 21 && target_version >= 21 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE file_change_blobs (
                hash TEXT PRIMARY KEY NOT NULL CHECK (
                    length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'
                ),
                size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 0 AND 16777216),
                state TEXT NOT NULL CHECK (state IN ('writing', 'pending', 'ready')),
                created_at_unix_ms INTEGER NOT NULL
            );

            CREATE TABLE conversation_file_changes (
                conversation_id TEXT NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                id TEXT NOT NULL CHECK (
                    length(id) = 64 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                root TEXT NOT NULL CHECK (length(root) BETWEEN 1 AND 32768),
                path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 32768),
                baseline_exists INTEGER NOT NULL CHECK (baseline_exists IN (0, 1)),
                baseline_hash TEXT CHECK (
                    baseline_hash IS NULL OR (
                        length(baseline_hash) = 64 AND baseline_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ),
                baseline_blob_hash TEXT REFERENCES file_change_blobs(hash),
                expected_exists INTEGER NOT NULL CHECK (expected_exists IN (0, 1)),
                expected_hash TEXT CHECK (
                    expected_hash IS NULL OR (
                        length(expected_hash) = 64 AND expected_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ),
                added_lines INTEGER CHECK (added_lines IS NULL OR added_lines >= 0),
                removed_lines INTEGER CHECK (removed_lines IS NULL OR removed_lines >= 0),
                is_binary INTEGER NOT NULL CHECK (is_binary IN (0, 1)),
                diff_available INTEGER NOT NULL CHECK (diff_available IN (0, 1)),
                status TEXT NOT NULL CHECK (status IN ('active', 'reverted', 'conflict')),
                CHECK (baseline_exists = 0 OR baseline_hash IS NOT NULL),
                CHECK (expected_exists = 0 OR expected_hash IS NOT NULL),
                PRIMARY KEY (conversation_id, id),
                UNIQUE (conversation_id, root, path)
            );

            CREATE INDEX conversation_file_changes_conversation_idx
                ON conversation_file_changes (conversation_id, path);

            CREATE TABLE file_change_legacy_imports (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                imported_at_unix_ms INTEGER NOT NULL
            );

            CREATE TRIGGER conversation_file_changes_blob_delete
            AFTER DELETE ON conversation_file_changes
            WHEN OLD.baseline_blob_hash IS NOT NULL
            BEGIN
                UPDATE file_change_blobs
                   SET state = 'pending'
                 WHERE hash = OLD.baseline_blob_hash
                   AND NOT EXISTS (
                       SELECT 1 FROM conversation_file_changes
                        WHERE baseline_blob_hash = OLD.baseline_blob_hash
                   );
            END;

            CREATE TRIGGER conversation_file_changes_blob_update
            AFTER UPDATE OF baseline_blob_hash ON conversation_file_changes
            WHEN OLD.baseline_blob_hash IS NOT NULL
             AND OLD.baseline_blob_hash IS NOT NEW.baseline_blob_hash
            BEGIN
                UPDATE file_change_blobs
                   SET state = 'pending'
                 WHERE hash = OLD.baseline_blob_hash
                   AND NOT EXISTS (
                       SELECT 1 FROM conversation_file_changes
                        WHERE baseline_blob_hash = OLD.baseline_blob_hash
                   );
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (21, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 21;
    }

    if current_version < 22 && target_version >= 22 {
        let transaction = connection.unchecked_transaction()?;
        let missing_message_index = !schema_table_exists(&transaction, "conversation_memory_fts")?;
        let missing_tool_index =
            !schema_table_exists(&transaction, "conversation_memory_tools_fts")?;

        if missing_message_index {
            transaction.execute_batch(
                "CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );

                INSERT INTO conversation_memory_fts (
                    rowid, conversation_id, message_id, role, scope_token, content
                )
                SELECT message.rowid, message.conversation_id, message.id, message.role,
                       'scope' || lower(hex(CAST(message.conversation_id AS BLOB))),
                       message.content
                  FROM messages AS message
                 WHERE message.role IN ('user', 'assistant')
                   AND message.status = 'completed'
                   AND COALESCE((
                       SELECT included FROM conversation_memory_archive_settings
                        WHERE conversation_id = message.conversation_id
                   ), 1) = 1;",
            )?;
        }

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

        transaction.execute_batch(REDACTED_TOOL_INDEX_TRIGGERS)?;
        if missing_tool_index {
            transaction.execute_batch(REDACT_AND_REBUILD_TOOL_INDEX)?;
        }
        transaction.execute_batch(MEMORY_INDEX_TRIGGERS)?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (22, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 22;
    }

    if current_version < 23 && target_version >= 23 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE chatgpt_models
                ADD COLUMN supports_fast_mode INTEGER NOT NULL DEFAULT 0
                CHECK (supports_fast_mode IN (0, 1));
            UPDATE chatgpt_model_catalog_state
               SET fetched_at_unix_ms = 0;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (23, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 23;
    }

    if current_version < 24 && target_version >= 24 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE usage_events (
                event_id TEXT PRIMARY KEY NOT NULL,
                event_kind TEXT NOT NULL CHECK (event_kind IN ('request', 'legacy_output')),
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                assistant_message_id TEXT,
                provider_id TEXT,
                model_id TEXT,
                reasoning_effort TEXT,
                operation TEXT NOT NULL CHECK (
                    operation IN ('chat', 'tool_follow_up', 'compaction', 'title_generation', 'legacy')
                ),
                status TEXT NOT NULL CHECK (
                    status IN ('pending', 'completed', 'failed', 'cancelled', 'interrupted')
                ),
                fast_requested INTEGER CHECK (fast_requested IS NULL OR fast_requested IN (0, 1)),
                service_tier TEXT,
                input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
                output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
                reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
                cached_input_tokens INTEGER CHECK (
                    cached_input_tokens IS NULL OR cached_input_tokens >= 0
                ),
                cache_write_tokens INTEGER CHECK (
                    cache_write_tokens IS NULL OR cache_write_tokens >= 0
                ),
                reported_cost_nano_usd INTEGER CHECK (
                    reported_cost_nano_usd IS NULL OR reported_cost_nano_usd >= 0
                ),
                cost_source TEXT CHECK (cost_source IS NULL OR cost_source = 'provider_reported'),
                usage_source TEXT NOT NULL CHECK (
                    usage_source IN ('provider_reported', 'unavailable', 'legacy_message_output')
                ),
                started_at_unix_ms INTEGER NOT NULL,
                finished_at_unix_ms INTEGER
            );
            CREATE INDEX usage_events_started_idx
                ON usage_events(started_at_unix_ms DESC, event_id);
            CREATE INDEX usage_events_provider_model_idx
                ON usage_events(provider_id, model_id, started_at_unix_ms DESC);
            CREATE INDEX usage_events_conversation_idx
                ON usage_events(conversation_id, started_at_unix_ms DESC);
            INSERT INTO usage_events (
                event_id, event_kind, conversation_id, assistant_message_id,
                operation, status, output_tokens, usage_source,
                started_at_unix_ms, finished_at_unix_ms
            )
            SELECT
                'legacy-output:' || message.id,
                'legacy_output',
                message.conversation_id,
                message.id,
                'legacy',
                'completed',
                message.output_tokens,
                'legacy_message_output',
                COALESCE(message.created_at, 0),
                COALESCE(message.created_at, 0)
            FROM messages AS message
            WHERE message.role = 'assistant'
              AND message.output_tokens IS NOT NULL
              AND message.output_tokens >= 0
              AND message.status <> 'streaming';",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (24, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 24;
    }

    if current_version < 25 && target_version >= 25 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE models_dev_catalog_cache (
                cache_id INTEGER PRIMARY KEY CHECK (cache_id = 1),
                fetched_at_unix_ms INTEGER NOT NULL CHECK (fetched_at_unix_ms >= 0),
                catalog_json TEXT NOT NULL
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (25, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 25;
    }

    if current_version < 26 && target_version >= 26 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE usage_events ADD COLUMN request_manifest_json TEXT
                CHECK (request_manifest_json IS NULL OR (
                    json_valid(request_manifest_json) = 1
                    AND length(request_manifest_json) <= 32768
                ));",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (26, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 26;
    }

    if current_version < 27 && target_version >= 27 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE usage_events ADD COLUMN run_id TEXT
                REFERENCES agent_runs(id) ON DELETE SET NULL;
            CREATE INDEX usage_events_run_idx
                ON usage_events(run_id, started_at_unix_ms DESC);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (27, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 27;
    }

    if current_version < 28 && target_version >= 28 {
        let has_tags = {
            let mut statement = connection.prepare("PRAGMA table_info(conversations)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "tags")
        };
        let transaction = connection.unchecked_transaction()?;
        if !has_tags {
            transaction.execute_batch(
                "ALTER TABLE conversations ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';",
            )?;
        }
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (28, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 28;
    }

    if current_version < 29 && target_version >= 29 {
        let has_bookmark = {
            let mut statement = connection.prepare("PRAGMA table_info(conversations)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "is_bookmarked")
        };
        let transaction = connection.unchecked_transaction()?;
        if !has_bookmark {
            transaction.execute_batch(
                "ALTER TABLE conversations ADD COLUMN is_bookmarked INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (29, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 29;
    }

    if current_version < 30 && target_version >= 30 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE copilot_oauth_connection (
                id TEXT PRIMARY KEY NOT NULL CHECK (id = 'copilot'),
                github_user_id TEXT NOT NULL,
                login TEXT NOT NULL,
                display_name TEXT,
                credential_reference TEXT NOT NULL UNIQUE,
                auth_status TEXT NOT NULL CHECK (auth_status IN ('active', 'reauth_required')),
                access_expires_at_unix_seconds INTEGER NOT NULL CHECK (access_expires_at_unix_seconds > 0),
                refresh_expires_at_unix_seconds INTEGER,
                connected_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (30, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 30;
    }

    if current_version < 31 && target_version >= 31 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE tool_permission_mode (
                preference_id INTEGER PRIMARY KEY CHECK (preference_id = 1),
                mode TEXT NOT NULL CHECK (
                    mode IN ('require_approval', 'approve_safe_operations', 'full_access')
                )
            );
            CREATE TABLE project_tool_permission_rules (
                project_id TEXT PRIMARY KEY NOT NULL CHECK (length(project_id) BETWEEN 1 AND 128),
                rules_json TEXT NOT NULL CHECK (
                    json_valid(rules_json) = 1
                    AND json_type(rules_json) = 'object'
                    AND length(rules_json) <= 131072
                )
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (31, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 31;
    }

    if current_version < 32 && target_version >= 32 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE copilot_oauth_connection RENAME TO copilot_oauth_connection_v31;
            CREATE TABLE copilot_oauth_connection (
                id TEXT PRIMARY KEY NOT NULL
                    CHECK (length(id) BETWEEN 1 AND 20 AND id NOT GLOB '*[^0-9]*'),
                github_user_id TEXT NOT NULL UNIQUE
                    CHECK (length(github_user_id) BETWEEN 1 AND 20 AND github_user_id NOT GLOB '*[^0-9]*'),
                login TEXT NOT NULL,
                display_name TEXT,
                credential_reference TEXT NOT NULL UNIQUE,
                auth_status TEXT NOT NULL CHECK (auth_status IN ('active', 'reauth_required')),
                access_expires_at_unix_seconds INTEGER NOT NULL CHECK (access_expires_at_unix_seconds > 0),
                refresh_expires_at_unix_seconds INTEGER,
                connected_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                is_selected INTEGER NOT NULL DEFAULT 0 CHECK (is_selected IN (0, 1))
            );
            INSERT INTO copilot_oauth_connection (
                id, github_user_id, login, display_name, credential_reference,
                auth_status, access_expires_at_unix_seconds, refresh_expires_at_unix_seconds,
                connected_at_unix_ms, updated_at_unix_ms, is_selected
            )
            SELECT github_user_id, github_user_id, login, display_name, credential_reference,
                auth_status, access_expires_at_unix_seconds, refresh_expires_at_unix_seconds,
                connected_at_unix_ms, updated_at_unix_ms, 1
            FROM copilot_oauth_connection_v31;
            DROP TABLE copilot_oauth_connection_v31;
            CREATE UNIQUE INDEX copilot_oauth_connection_single_selected
                ON copilot_oauth_connection(is_selected) WHERE is_selected = 1;
            CREATE INDEX copilot_oauth_connection_order
                ON copilot_oauth_connection(connected_at_unix_ms DESC, id);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (32, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 32;
    }

    if current_version < 33 && target_version >= 33 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE pending_chatgpt_credential_deletions (
                credential_reference TEXT PRIMARY KEY NOT NULL
                    CHECK (length(credential_reference) BETWEEN 1 AND 256),
                connection_id TEXT NOT NULL
                    CHECK (length(connection_id) BETWEEN 1 AND 128),
                requested_at_unix_ms INTEGER NOT NULL
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (33, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 33;
    }

    if current_version < 34 && target_version >= 34 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_file_checkpoints (
                id TEXT PRIMARY KEY NOT NULL CHECK (
                    length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                conversation_id TEXT NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL CHECK (sequence > 0),
                created_at_unix_ms INTEGER NOT NULL,
                tool_name TEXT NOT NULL CHECK (length(tool_name) BETWEEN 1 AND 128),
                UNIQUE (conversation_id, sequence)
            );

            CREATE INDEX conversation_file_checkpoints_order
                ON conversation_file_checkpoints (conversation_id, sequence DESC);

            CREATE TABLE conversation_file_checkpoint_files (
                checkpoint_id TEXT NOT NULL
                    REFERENCES conversation_file_checkpoints(id) ON DELETE CASCADE,
                file_id TEXT NOT NULL CHECK (
                    length(file_id) = 64 AND file_id NOT GLOB '*[^0-9a-f]*'
                ),
                root TEXT NOT NULL CHECK (length(root) BETWEEN 1 AND 32768),
                path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 32768),
                before_exists INTEGER NOT NULL CHECK (before_exists IN (0, 1)),
                before_hash TEXT CHECK (
                    before_hash IS NULL OR (
                        length(before_hash) = 64 AND before_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ),
                before_blob_hash TEXT REFERENCES file_change_blobs(hash),
                after_exists INTEGER NOT NULL CHECK (after_exists IN (0, 1)),
                after_hash TEXT CHECK (
                    after_hash IS NULL OR (
                        length(after_hash) = 64 AND after_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ),
                after_blob_hash TEXT REFERENCES file_change_blobs(hash),
                CHECK (before_exists = 1 OR (before_hash IS NULL AND before_blob_hash IS NULL)),
                CHECK (before_exists = 0 OR before_hash IS NOT NULL),
                CHECK (after_exists = 1 OR (after_hash IS NULL AND after_blob_hash IS NULL)),
                CHECK (after_exists = 0 OR after_hash IS NOT NULL),
                PRIMARY KEY (checkpoint_id, file_id),
                UNIQUE (checkpoint_id, root, path)
            );

            CREATE INDEX conversation_file_checkpoint_files_path
                ON conversation_file_checkpoint_files (checkpoint_id, path);

            CREATE TRIGGER conversation_file_checkpoint_files_blob_delete
            AFTER DELETE ON conversation_file_checkpoint_files
            BEGIN
                UPDATE file_change_blobs SET state = 'pending'
                 WHERE hash IN (OLD.before_blob_hash, OLD.after_blob_hash)
                   AND NOT EXISTS (
                       SELECT 1 FROM conversation_file_changes
                        WHERE baseline_blob_hash = file_change_blobs.hash
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM conversation_file_checkpoint_files
                        WHERE before_blob_hash = file_change_blobs.hash
                           OR after_blob_hash = file_change_blobs.hash
                   );
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (34, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 34;
    }

    if current_version < 35 && target_version >= 35 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE local_models (
                id TEXT PRIMARY KEY NOT NULL,
                engine_id TEXT NOT NULL CHECK (engine_id = 'llama_cpp'),
                display_name TEXT NOT NULL,
                model_path TEXT NOT NULL,
                path_kind TEXT NOT NULL CHECK (path_kind IN ('file', 'directory')),
                created_at_unix_ms INTEGER NOT NULL,
                UNIQUE (engine_id, model_path)
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (35, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 35;
    }

    if current_version < 36 && target_version >= 36 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE project_index_state (
                project_id TEXT PRIMARY KEY NOT NULL,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                status TEXT NOT NULL,
                indexed_files INTEGER NOT NULL,
                indexed_bytes INTEGER NOT NULL,
                updated_at_unix_ms INTEGER
            );
            CREATE TABLE project_index_roots (
                project_id TEXT PRIMARY KEY NOT NULL,
                root_path TEXT NOT NULL
            );
            CREATE TABLE project_index_files (
                project_id TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_ns INTEGER NOT NULL,
                size INTEGER NOT NULL,
                content TEXT NOT NULL,
                PRIMARY KEY (project_id, path)
            );
            CREATE VIRTUAL TABLE project_index_fts USING fts5(
                project_id UNINDEXED, path UNINDEXED, content,
                tokenize='unicode61 remove_diacritics 2'
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (36, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 36;
    }

    if current_version < 37 && target_version >= 37 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE generated_images (
                id TEXT PRIMARY KEY NOT NULL CHECK (
                    length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                prompt TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 40000),
                model_id TEXT NOT NULL CHECK (length(model_id) BETWEEN 1 AND 128),
                size TEXT,
                quality TEXT CHECK (
                    quality IS NULL OR quality IN ('low', 'medium', 'high', 'xhigh', 'max', 'auto')
                ),
                background TEXT CHECK (
                    background IS NULL OR background IN ('transparent', 'opaque', 'auto')
                ),
                moderation TEXT CHECK (
                    moderation IS NULL OR moderation IN ('auto', 'low')
                ),
                output_format TEXT NOT NULL CHECK (output_format IN ('png', 'jpeg', 'webp')),
                output_compression INTEGER CHECK (
                    output_compression IS NULL OR (
                        output_compression BETWEEN 0 AND 100
                        AND output_format IN ('jpeg', 'webp')
                    )
                ),
                mime_type TEXT NOT NULL CHECK (mime_type IN ('image/png', 'image/jpeg', 'image/webp')),
                size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 33554432),
                created_at_unix_ms INTEGER NOT NULL
            );
            CREATE INDEX generated_images_created
                ON generated_images (created_at_unix_ms DESC, id ASC);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (37, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 37;
    }

    if current_version < 38 && target_version >= 38 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE generated_images
                ADD COLUMN generation_id TEXT NOT NULL DEFAULT '';
            UPDATE generated_images SET generation_id = id WHERE generation_id = '';
            CREATE INDEX generated_images_generation
                ON generated_images (generation_id, created_at_unix_ms DESC, id ASC);

            CREATE TABLE image_generation_runs (
                id TEXT PRIMARY KEY NOT NULL CHECK (
                    length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                source_kind TEXT NOT NULL CHECK (source_kind IN ('design', 'chat')),
                provider_route TEXT NOT NULL CHECK (
                    provider_route IN ('oauth', 'api_key', 'unknown')
                ),
                connection_id TEXT,
                workspace_id TEXT,
                conversation_id TEXT,
                message_id TEXT,
                prompt TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 40000),
                model_id TEXT NOT NULL CHECK (length(model_id) BETWEEN 1 AND 128),
                size TEXT,
                quality TEXT,
                background TEXT,
                moderation TEXT,
                output_format TEXT,
                output_compression INTEGER,
                requested_count INTEGER NOT NULL CHECK (requested_count BETWEEN 1 AND 3),
                image_count INTEGER NOT NULL DEFAULT 0 CHECK (image_count BETWEEN 0 AND 3),
                status TEXT NOT NULL CHECK (
                    status IN ('preparing', 'generating', 'completed', 'cancelled', 'failed')
                ),
                error_code TEXT,
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );
            CREATE INDEX image_generation_runs_created
                ON image_generation_runs (created_at_unix_ms DESC, id ASC);
            CREATE INDEX image_generation_runs_status
                ON image_generation_runs (status, created_at_unix_ms DESC, id ASC);

            CREATE TABLE image_generation_outputs (
                generation_id TEXT NOT NULL REFERENCES image_generation_runs(id) ON DELETE CASCADE,
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                attachment_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id, attachment_id)
            );
            CREATE INDEX image_generation_outputs_generation
                ON image_generation_outputs (generation_id, conversation_id, message_id);

            CREATE TABLE image_library_annotations (
                source_kind TEXT NOT NULL CHECK (source_kind IN ('design', 'chat')),
                source_key TEXT NOT NULL CHECK (length(source_key) BETWEEN 1 AND 1024),
                is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
                tags_json TEXT NOT NULL DEFAULT '[]',
                PRIMARY KEY (source_kind, source_key)
            );
            CREATE INDEX image_library_annotations_favorites
                ON image_library_annotations (is_favorite, source_kind, source_key);

            CREATE TABLE image_library_hidden_chat_images (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                attachment_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id, attachment_id)
            );

            CREATE TABLE image_collections (
                id TEXT PRIMARY KEY NOT NULL CHECK (
                    length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX image_collections_name
                ON image_collections (name COLLATE NOCASE);

            CREATE TABLE image_collection_items (
                collection_id TEXT NOT NULL REFERENCES image_collections(id) ON DELETE CASCADE,
                source_kind TEXT NOT NULL CHECK (source_kind IN ('design', 'chat')),
                source_key TEXT NOT NULL CHECK (length(source_key) BETWEEN 1 AND 1024),
                PRIMARY KEY (collection_id, source_kind, source_key)
            );
            CREATE INDEX image_collection_items_source
                ON image_collection_items (source_kind, source_key, collection_id);

            CREATE TABLE image_prompt_templates (
                id TEXT PRIMARY KEY NOT NULL CHECK (
                    length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'
                ),
                name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
                prompt TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 10000),
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );
            CREATE INDEX image_prompt_templates_updated
                ON image_prompt_templates (updated_at_unix_ms DESC, id ASC);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (38, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 38;
    }

    if current_version < 39 && target_version >= 39 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE INDEX conversation_file_changes_baseline_blob_idx
                ON conversation_file_changes (baseline_blob_hash);
            CREATE INDEX conversation_file_checkpoint_files_before_blob_idx
                ON conversation_file_checkpoint_files (before_blob_hash);
            CREATE INDEX conversation_file_checkpoint_files_after_blob_idx
                ON conversation_file_checkpoint_files (after_blob_hash);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (39, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 39;
    }

    Ok(current_version)
}
