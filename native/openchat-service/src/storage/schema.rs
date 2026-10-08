use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

pub(super) const SCHEMA_VERSION: i64 = 29;
pub(super) const INITIAL_SCHEMA_VERSION: i64 = 2;

const REDACTED_TOOL_INDEX_TRIGGERS: &str = r#"
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

const REDACT_AND_REBUILD_TOOL_INDEX: &str = r#"
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

const MEMORY_INDEX_TRIGGERS: &str = r#"
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

pub(super) fn initialize_schema(
    connection: &Connection,
    target_version: i64,
) -> rusqlite::Result<i64> {
    super::tool_index_redaction::register_sqlite_function(connection)?;
    if !(1..=SCHEMA_VERSION).contains(&target_version) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let migrations_table_exists = schema_table_exists(connection, "openchat_backend_migrations")?;
    let mut current_version = if migrations_table_exists {
        connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?
    } else {
        0
    };
    if current_version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if current_version >= target_version {
        if target_version >= SCHEMA_VERSION && current_version >= 13 {
            let indexes_repaired =
                repair_missing_memory_indexes_and_triggers(connection, current_version)?;
            if indexes_repaired {
                let transaction = connection.unchecked_transaction()?;
                transaction.execute_batch(REDACTED_TOOL_INDEX_TRIGGERS)?;
                transaction.execute_batch(MEMORY_INDEX_TRIGGERS)?;
                transaction.commit()?;
            }
        }
        return Ok(current_version);
    }

    if current_version >= 13 {
        repair_missing_memory_indexes_and_triggers(connection, current_version)?;
    }

    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS openchat_backend_migrations (
            version INTEGER PRIMARY KEY,
            applied_at_unix_ms INTEGER NOT NULL
        );",
    )?;

    if current_version == 0 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
        "CREATE TABLE chatgpt_connections (
            id TEXT PRIMARY KEY NOT NULL,
            external_user_id TEXT,
            email TEXT,
            display_name TEXT,
            plan_type TEXT,
            credential_reference TEXT NOT NULL UNIQUE,
            auth_status TEXT NOT NULL CHECK (auth_status IN ('active', 'reauth_required', 'revoked')),
            created_at_unix_ms INTEGER NOT NULL,
            updated_at_unix_ms INTEGER NOT NULL,
            last_authenticated_at_unix_ms INTEGER
        );

        CREATE TABLE chatgpt_workspaces (
            id TEXT PRIMARY KEY NOT NULL,
            connection_id TEXT NOT NULL REFERENCES chatgpt_connections(id) ON DELETE CASCADE,
            external_workspace_id TEXT NOT NULL,
            display_name TEXT,
            plan_type TEXT,
            is_selected INTEGER NOT NULL DEFAULT 0 CHECK (is_selected IN (0, 1)),
            fetched_at_unix_ms INTEGER NOT NULL,
            UNIQUE (connection_id, id),
            UNIQUE (connection_id, external_workspace_id)
        );

        CREATE TABLE chatgpt_models (
            connection_id TEXT NOT NULL REFERENCES chatgpt_connections(id) ON DELETE CASCADE,
            workspace_id TEXT NOT NULL REFERENCES chatgpt_workspaces(id) ON DELETE CASCADE,
            model_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT,
            context_window INTEGER,
            reasoning_levels_json TEXT,
            is_available INTEGER NOT NULL CHECK (is_available IN (0, 1)),
            fetched_at_unix_ms INTEGER NOT NULL,
            client_version TEXT NOT NULL,
            PRIMARY KEY (connection_id, workspace_id, model_id),
            FOREIGN KEY (connection_id, workspace_id)
                REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE CASCADE
        );

        CREATE TABLE chatgpt_usage_snapshots (
            id TEXT PRIMARY KEY NOT NULL,
            connection_id TEXT NOT NULL REFERENCES chatgpt_connections(id) ON DELETE CASCADE,
            workspace_id TEXT NOT NULL REFERENCES chatgpt_workspaces(id) ON DELETE CASCADE,
            fetched_at_unix_ms INTEGER NOT NULL,
            freshness TEXT NOT NULL CHECK (freshness IN ('current', 'stale', 'unavailable')),
            ordinary_usage_allowed INTEGER CHECK (
                ordinary_usage_allowed IS NULL OR ordinary_usage_allowed IN (0, 1)
            ),
            reset_credit_count INTEGER CHECK (reset_credit_count IS NULL OR reset_credit_count >= 0),
            reset_credit_details_state TEXT NOT NULL CHECK (
                reset_credit_details_state IN ('available', 'omitted', 'unavailable')
            ),
            FOREIGN KEY (connection_id, workspace_id)
                REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE CASCADE
        );

        CREATE TABLE chatgpt_usage_buckets (
            snapshot_id TEXT NOT NULL REFERENCES chatgpt_usage_snapshots(id) ON DELETE CASCADE,
            limit_id TEXT NOT NULL,
            used_percent REAL CHECK (used_percent IS NULL OR used_percent >= 0),
            window_seconds INTEGER CHECK (window_seconds IS NULL OR window_seconds >= 0),
            reset_at_unix_ms INTEGER,
            PRIMARY KEY (snapshot_id, limit_id)
        );

        CREATE TABLE chatgpt_reset_credits (
            snapshot_id TEXT NOT NULL REFERENCES chatgpt_usage_snapshots(id) ON DELETE CASCADE,
            external_credit_id TEXT NOT NULL,
            reset_type TEXT,
            status TEXT,
            granted_at_unix_ms INTEGER,
            expires_at_unix_ms INTEGER,
            title TEXT,
            description TEXT,
            PRIMARY KEY (snapshot_id, external_credit_id)
        );

        CREATE TABLE title_generation_jobs (
            id TEXT PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            connection_id TEXT NOT NULL REFERENCES chatgpt_connections(id) ON DELETE RESTRICT,
            workspace_id TEXT NOT NULL REFERENCES chatgpt_workspaces(id) ON DELETE RESTRICT,
            model_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (
                status IN ('queued', 'skipped', 'running', 'completed', 'failed', 'cancelled')
            ),
            reason_code TEXT,
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
            created_at_unix_ms INTEGER NOT NULL,
            completed_at_unix_ms INTEGER,
            FOREIGN KEY (connection_id, workspace_id)
                REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX chatgpt_workspaces_connection_idx
            ON chatgpt_workspaces(connection_id, is_selected);
        CREATE UNIQUE INDEX chatgpt_single_selected_workspace_idx
            ON chatgpt_workspaces(connection_id) WHERE is_selected = 1;
        CREATE INDEX chatgpt_models_workspace_idx
            ON chatgpt_models(connection_id, workspace_id, is_available);
        CREATE INDEX chatgpt_usage_connection_idx
            ON chatgpt_usage_snapshots(connection_id, workspace_id, fetched_at_unix_ms DESC);
        CREATE INDEX title_jobs_conversation_idx
            ON title_generation_jobs(conversation_id, created_at_unix_ms DESC);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (1, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 1;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 2 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE chatgpt_connections
                ADD COLUMN is_selected INTEGER NOT NULL DEFAULT 0
                CHECK (is_selected IN (0, 1));
            CREATE UNIQUE INDEX chatgpt_single_selected_connection_idx
                ON chatgpt_connections(is_selected) WHERE is_selected = 1;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (2, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 2;
    }

    if current_version >= target_version {
        return Ok(current_version);
    }

    if current_version < 3 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "DROP INDEX title_jobs_conversation_idx;
            ALTER TABLE title_generation_jobs RENAME TO title_generation_jobs_v2;
            CREATE TABLE title_generation_jobs (
                id TEXT PRIMARY KEY NOT NULL,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                connection_id TEXT NOT NULL REFERENCES chatgpt_connections(id) ON DELETE RESTRICT,
                workspace_id TEXT NOT NULL REFERENCES chatgpt_workspaces(id) ON DELETE RESTRICT,
                model_id TEXT,
                status TEXT NOT NULL CHECK (
                    status IN ('queued', 'skipped', 'running', 'completed', 'failed', 'cancelled')
                ),
                reason_code TEXT,
                attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
                created_at_unix_ms INTEGER NOT NULL,
                completed_at_unix_ms INTEGER,
                FOREIGN KEY (connection_id, workspace_id)
                    REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE RESTRICT
            );
            INSERT INTO title_generation_jobs (
                id, conversation_id, connection_id, workspace_id, model_id, status,
                reason_code, attempt_count, created_at_unix_ms, completed_at_unix_ms
            ) SELECT id, conversation_id, connection_id, workspace_id, model_id, status,
                     reason_code, attempt_count, created_at_unix_ms, completed_at_unix_ms
              FROM title_generation_jobs_v2;
            DROP TABLE title_generation_jobs_v2;
            CREATE INDEX title_jobs_conversation_idx
                ON title_generation_jobs(conversation_id, created_at_unix_ms DESC);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (3, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 3;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 4 {
        let transaction = connection.unchecked_transaction()?;
        transaction
            .execute_batch("ALTER TABLE chatgpt_models ADD COLUMN default_reasoning_level TEXT;")?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (4, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 4;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 5 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE chatgpt_title_preferences (
                preference_id INTEGER PRIMARY KEY CHECK (preference_id = 1),
                connection_id TEXT,
                workspace_id TEXT,
                updated_at_unix_ms INTEGER NOT NULL,
                CHECK (
                    (connection_id IS NULL AND workspace_id IS NULL)
                    OR connection_id IS NOT NULL
                ),
                FOREIGN KEY (connection_id, workspace_id)
                    REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE SET NULL
            );
            INSERT INTO chatgpt_title_preferences (
                preference_id, connection_id, workspace_id, updated_at_unix_ms
            ) VALUES (1, NULL, NULL, 0);",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (5, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 5;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 6 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE chatgpt_models
                ADD COLUMN supports_reasoning_summary_parameter INTEGER NOT NULL DEFAULT 1
                CHECK (supports_reasoning_summary_parameter IN (0, 1));",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (6, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 6;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 7 {
        let has_provider_id = {
            let mut statement = connection.prepare("PRAGMA table_info(conversations)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "provider_id")
        };
        let transaction = connection.unchecked_transaction()?;
        if !has_provider_id {
            transaction.execute_batch("ALTER TABLE conversations ADD COLUMN provider_id TEXT;")?;
        }
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (7, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 7;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 8 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE chatgpt_model_catalog_state (
                connection_id TEXT NOT NULL,
                workspace_id TEXT NOT NULL,
                fetched_at_unix_ms INTEGER NOT NULL,
                client_version TEXT NOT NULL,
                PRIMARY KEY (connection_id, workspace_id),
                FOREIGN KEY (connection_id, workspace_id)
                    REFERENCES chatgpt_workspaces(connection_id, id) ON DELETE CASCADE
            );
            CREATE TABLE opencode_model_catalog (
                catalog_id INTEGER PRIMARY KEY CHECK (catalog_id = 1),
                fetched_at_unix_ms INTEGER NOT NULL,
                models_json TEXT NOT NULL
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (8, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 8;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 9 {
        let has_api_key_connection_id = {
            let mut statement = connection.prepare("PRAGMA table_info(conversations)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "api_key_connection_id")
        };
        let transaction = connection.unchecked_transaction()?;
        if !has_api_key_connection_id {
            transaction.execute_batch(
                "ALTER TABLE conversations ADD COLUMN api_key_connection_id TEXT;",
            )?;
        }
        transaction.execute_batch(
            "CREATE TABLE openai_api_model_catalog (
                api_key_hash TEXT PRIMARY KEY,
                fetched_at_unix_ms INTEGER NOT NULL,
                models_json TEXT NOT NULL
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (9, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 9;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 10 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE compatible_provider_model_catalog (
                provider_id TEXT NOT NULL,
                api_key_hash TEXT NOT NULL,
                fetched_at_unix_ms INTEGER NOT NULL,
                models_json TEXT NOT NULL,
                PRIMARY KEY (provider_id, api_key_hash)
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (10, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 10;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 11 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_context_state (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                compaction_kind TEXT CHECK (
                    compaction_kind IS NULL
                    OR compaction_kind IN ('responses_checkpoint', 'summary')
                ),
                compaction_payload TEXT,
                compaction_provider_id TEXT,
                compaction_connection_id TEXT,
                compaction_workspace_id TEXT,
                compaction_model_id TEXT,
                compacted_through_message_id TEXT,
                last_prompt_tokens INTEGER CHECK (
                    last_prompt_tokens IS NULL OR last_prompt_tokens >= 0
                ),
                last_prompt_message_id TEXT,
                last_prompt_provider_id TEXT,
                last_prompt_model_id TEXT,
                updated_at_unix_ms INTEGER NOT NULL,
                CHECK (
                    (compaction_kind IS NULL AND compaction_payload IS NULL
                        AND compacted_through_message_id IS NULL)
                    OR (compaction_kind IS NOT NULL AND compaction_payload IS NOT NULL
                        AND compacted_through_message_id IS NOT NULL)
                )
            );",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (11, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 11;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 12 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE conversation_context_state
                 ADD COLUMN last_prompt_connection_id TEXT;
             ALTER TABLE conversation_context_state
                 ADD COLUMN last_prompt_workspace_id TEXT;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (12, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 12;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 13 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_memory_index_state (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                backfilled_at_unix_ms INTEGER NOT NULL
            );

            CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                conversation_id UNINDEXED,
                message_id UNINDEXED,
                role UNINDEXED,
                scope_token,
                content,
                tokenize = 'unicode61 remove_diacritics 2'
            );

            CREATE TRIGGER conversation_memory_message_insert
            AFTER INSERT ON messages
            WHEN NEW.role IN ('user', 'assistant') AND NEW.status = 'completed'
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
                WHERE NEW.role IN ('user', 'assistant') AND NEW.status = 'completed';
            END;

            CREATE TRIGGER conversation_memory_message_delete
            AFTER DELETE ON messages
            BEGIN
                DELETE FROM conversation_memory_fts WHERE rowid = OLD.rowid;
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (13, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 13;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 14 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_memory_tool_index_state (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                backfilled_at_unix_ms INTEGER NOT NULL
            );

            CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                conversation_id UNINDEXED,
                message_id UNINDEXED,
                role UNINDEXED,
                scope_token,
                content,
                tokenize = 'unicode61 remove_diacritics 2'
            );

            CREATE TRIGGER conversation_memory_tool_insert
            AFTER INSERT ON messages
            WHEN NEW.role = 'assistant'
                AND NEW.status = 'completed'
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
                    )
                WHERE NEW.role = 'assistant'
                  AND NEW.status = 'completed'
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
                  );
            END;

            CREATE TRIGGER conversation_memory_tool_delete
            AFTER DELETE ON messages
            BEGIN
                DELETE FROM conversation_memory_tools_fts WHERE rowid = OLD.rowid;
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (14, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 14;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

    if current_version < 15 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE conversation_memory_embedding_namespaces (
                namespace INTEGER PRIMARY KEY AUTOINCREMENT
                    CHECK (namespace BETWEEN 1 AND 16777215),
                conversation_id TEXT NOT NULL UNIQUE
                    REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE TABLE conversation_memory_embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT
                    CHECK (id BETWEEN 1 AND 1099511627775),
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
                start_byte INTEGER NOT NULL CHECK (start_byte >= 0),
                end_byte INTEGER NOT NULL CHECK (end_byte >= start_byte),
                model_id TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                vector BLOB NOT NULL CHECK (length(vector) = 384),
                UNIQUE (conversation_id, message_id, chunk_index),
                FOREIGN KEY (conversation_id, message_id)
                    REFERENCES messages(conversation_id, id) ON DELETE CASCADE
            );

            CREATE INDEX conversation_memory_embeddings_conversation_idx
                ON conversation_memory_embeddings(conversation_id, id);

            CREATE TABLE conversation_memory_embedding_state (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                chunk_count INTEGER NOT NULL CHECK (chunk_count >= 0),
                PRIMARY KEY (conversation_id, message_id),
                FOREIGN KEY (conversation_id, message_id)
                    REFERENCES messages(conversation_id, id) ON DELETE CASCADE
            );

            CREATE TABLE conversation_memory_embedding_pending (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id),
                FOREIGN KEY (conversation_id, message_id)
                    REFERENCES messages(conversation_id, id) ON DELETE CASCADE
            );

            CREATE TABLE conversation_memory_embedding_backfill (
                conversation_id TEXT PRIMARY KEY NOT NULL
                    REFERENCES conversations(id) ON DELETE CASCADE,
                cursor_created_at INTEGER,
                cursor_message_id TEXT
            );

            CREATE TABLE conversation_memory_semantic_index_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
                indexed_generation INTEGER CHECK (
                    indexed_generation IS NULL OR indexed_generation >= 0
                )
            );
            INSERT INTO conversation_memory_semantic_index_state (id, generation)
                VALUES (1, 0);

            CREATE TRIGGER conversation_memory_embedding_insert
            AFTER INSERT ON conversation_memory_embeddings
            BEGIN
                UPDATE conversation_memory_semantic_index_state
                SET generation = generation + 1 WHERE id = 1;
            END;

            CREATE TRIGGER conversation_memory_embedding_update
            AFTER UPDATE ON conversation_memory_embeddings
            BEGIN
                UPDATE conversation_memory_semantic_index_state
                SET generation = generation + 1 WHERE id = 1;
            END;

            CREATE TRIGGER conversation_memory_embedding_delete
            AFTER DELETE ON conversation_memory_embeddings
            BEGIN
                UPDATE conversation_memory_semantic_index_state
                SET generation = generation + 1 WHERE id = 1;
            END;

            CREATE TRIGGER conversation_memory_embedding_message_insert
            AFTER INSERT ON messages
            WHEN NEW.role IN ('user', 'assistant') AND NEW.status = 'completed'
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
                    WHERE NEW.role IN ('user', 'assistant') AND NEW.status = 'completed';
            END;

            CREATE TRIGGER conversation_memory_embedding_message_delete
            AFTER DELETE ON messages
            BEGIN
                DELETE FROM conversation_memory_embedding_pending
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
                DELETE FROM conversation_memory_embedding_state
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
                DELETE FROM conversation_memory_embeddings
                    WHERE conversation_id = OLD.conversation_id AND message_id = OLD.id;
            END;",
        )?;
        transaction.execute(
            "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms) VALUES (15, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 15;
        if current_version >= target_version {
            return Ok(current_version);
        }
    }

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

    Ok(current_version)
}

fn schema_table_exists(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
        )",
        [name],
        |row| row.get(0),
    )
}

fn schema_trigger_exists(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'trigger' AND name = ?1
        )",
        [name],
        |row| row.get(0),
    )
}

fn repair_missing_memory_indexes_and_triggers(
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

    use super::{SCHEMA_VERSION, initialize_schema};

    fn memory_index_repair_fixture(version: i64) -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
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
            .expect("create host conversation schema");
        initialize_schema(&connection, version).expect("initialize fixture schema");
        connection
            .execute("INSERT INTO conversations (id) VALUES ('conversation')", [])
            .expect("insert fixture conversation");
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at, tool_activities)
                 VALUES
                    ('user-message', 'conversation', 'user', 'repairmessageindexmarker', 'completed', 1, '[]'),
                    ('assistant-message', 'conversation', 'assistant', 'Completed tool call.', 'completed', 2,
                     '[{\"name\":\"read_file\",\"status\":\"completed\",\"arguments\":\"repairtoolindexmarker\",\"output\":\"done\"}]')",
                [],
            )
            .expect("insert fixture messages");
        connection
    }

    fn drop_memory_indexes_and_triggers(connection: &Connection) {
        connection
            .execute_batch(
                "DROP TRIGGER IF EXISTS conversation_memory_message_insert;
                DROP TRIGGER IF EXISTS conversation_memory_message_update;
                DROP TRIGGER IF EXISTS conversation_memory_message_delete;
                DROP TRIGGER IF EXISTS conversation_memory_tool_insert;
                DROP TRIGGER IF EXISTS conversation_memory_tool_update;
                DROP TRIGGER IF EXISTS conversation_memory_tool_delete;
                DROP TABLE conversation_memory_fts;
                DROP TABLE conversation_memory_tools_fts;",
            )
            .expect("remove derived memory indexes and triggers");
    }

    fn drop_memory_triggers(connection: &Connection) {
        connection
            .execute_batch(
                "DROP TRIGGER IF EXISTS conversation_memory_message_insert;
                DROP TRIGGER IF EXISTS conversation_memory_message_update;
                DROP TRIGGER IF EXISTS conversation_memory_message_delete;
                DROP TRIGGER IF EXISTS conversation_memory_tool_insert;
                DROP TRIGGER IF EXISTS conversation_memory_tool_update;
                DROP TRIGGER IF EXISTS conversation_memory_tool_delete;",
            )
            .expect("remove derived memory index triggers");
    }

    #[test]
    fn upgrades_conversation_bookmarks_without_duplicating_the_drift_column() {
        let connection = memory_index_repair_fixture(SCHEMA_VERSION - 1);
        initialize_schema(&connection, SCHEMA_VERSION).expect("add conversation bookmarks");
        let is_bookmarked = connection
            .query_row(
                "SELECT is_bookmarked FROM conversations WHERE id = 'conversation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read default conversation bookmark state");
        assert_eq!(is_bookmarked, 0);

        initialize_schema(&connection, SCHEMA_VERSION)
            .expect("reopen current schema without duplicate migration");

        let frontend_migrated = memory_index_repair_fixture(SCHEMA_VERSION - 1);
        frontend_migrated
            .execute_batch(
                "ALTER TABLE conversations ADD COLUMN is_bookmarked INTEGER NOT NULL DEFAULT 0;",
            )
            .expect("simulate the Drift-owned bookmark migration");
        initialize_schema(&frontend_migrated, SCHEMA_VERSION)
            .expect("recognize the existing Drift bookmark column");
    }

    fn assert_memory_indexes_rebuilt(connection: &Connection) {
        let message_count = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_fts
                 WHERE conversation_memory_fts MATCH 'content : \"repairmessageindexmarker\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("find rebuilt message index entry");
        assert_eq!(message_count, 1);

        let tool_count = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"repairtoolindexmarker\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("find rebuilt tool index entry");
        assert_eq!(tool_count, 1);

        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                 VALUES ('new-message', 'conversation', 'user', 'postmigrationindexmarker', 'completed', 3)",
                [],
            )
            .expect("insert message after index repair");
        let new_message_count = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_fts
                 WHERE conversation_memory_fts MATCH 'content : \"postmigrationindexmarker\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("check the restored message trigger");
        assert_eq!(new_message_count, 1);
    }

    #[test]
    fn upgrades_version_seventeen_when_derived_memory_indexes_are_missing() {
        let connection = memory_index_repair_fixture(17);
        drop_memory_indexes_and_triggers(&connection);

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION)
                .expect("upgrade and rebuild missing memory indexes"),
            SCHEMA_VERSION
        );
        assert_memory_indexes_rebuilt(&connection);
    }

    #[test]
    fn repairs_missing_memory_indexes_from_version_twenty_one() {
        let connection = memory_index_repair_fixture(21);
        drop_memory_indexes_and_triggers(&connection);

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION)
                .expect("repair missing indexes in an otherwise current schema"),
            SCHEMA_VERSION
        );
        assert_memory_indexes_rebuilt(&connection);
    }

    #[test]
    fn repairs_missing_memory_triggers_on_current_schema_startup() {
        let connection = memory_index_repair_fixture(SCHEMA_VERSION);
        drop_memory_triggers(&connection);

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION)
                .expect("repair missing memory index triggers"),
            SCHEMA_VERSION
        );
        assert_memory_indexes_rebuilt(&connection);
    }

    #[test]
    fn schema_initialization_stops_at_each_requested_target() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch("CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);")
            .expect("create the host conversation table");

        assert_eq!(
            initialize_schema(&connection, 1).expect("apply initial schema only"),
            1
        );
        let version_after_initial = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read initial migration version");
        assert_eq!(version_after_initial, 1);

        assert_eq!(
            initialize_schema(&connection, 4).expect("apply schema through version four"),
            4
        );
        let version_after_upgrade = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read upgraded migration version");
        assert_eq!(version_after_upgrade, 4);
    }

    #[test]
    fn question_item_ids_are_unique_within_each_group_and_existing_items_survive_upgrade() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );",
            )
            .expect("create host conversation schema");
        initialize_schema(&connection, 17).expect("apply schema through question item migration");
        connection
            .execute("INSERT INTO conversations (id) VALUES ('conversation')", [])
            .expect("insert question fixture conversation");
        connection
            .execute(
                "INSERT INTO agent_runs (
                    id, conversation_id, status, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES ('run', 'conversation', 'paused', 0, 0)",
                [],
            )
            .expect("insert paused question fixture run");
        connection
            .execute(
                "INSERT INTO pending_question_groups (
                    id, run_id, sequence, status, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES ('first', 'run', 1, 'pending', 0, 0),
                          ('second', 'run', 2, 'pending', 0, 0)",
                [],
            )
            .expect("insert question fixture groups");
        connection
            .execute(
                "INSERT INTO pending_question_items (group_id, item_id, position, question_json)
                 VALUES ('first', 'mode', 0, '{\"id\":\"mode\"}')",
                [],
            )
            .expect("insert legacy question item");

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION)
                .expect("apply question identity migration"),
            SCHEMA_VERSION
        );
        connection
            .execute(
                "INSERT INTO pending_question_items (group_id, item_id, position, question_json)
                 VALUES ('second', 'mode', 0, '{\"id\":\"mode\"}')",
                [],
            )
            .expect("allow matching item ids in separate question groups");

        let item_count = connection
            .query_row("SELECT COUNT(*) FROM pending_question_items", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count migrated question items");
        assert_eq!(item_count, 2);
        let original_json = connection
            .query_row(
                "SELECT question_json FROM pending_question_items
                 WHERE group_id = 'first' AND item_id = 'mode'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read preserved question item");
        assert_eq!(original_json, "{\"id\":\"mode\"}");
        assert!(connection
            .execute(
                "INSERT INTO pending_question_items (group_id, item_id, position, question_json)
                 VALUES ('first', 'mode', 1, '{\"id\":\"mode\"}')",
                [],
            )
            .is_err());
    }

    #[test]
    fn compaction_and_archive_migrations_upgrade_version_ten_without_losing_messages() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );
                CREATE TABLE chatgpt_models (model_id TEXT);
                CREATE TABLE chatgpt_model_catalog_state (fetched_at_unix_ms INTEGER);
                INSERT INTO conversations (id) VALUES ('conversation');
                INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                VALUES
                    ('old-user', 'conversation', 'user', 'Keep this original question.', 'completed', 1),
                    ('old-assistant', 'conversation', 'assistant', 'Keep this original answer.', 'completed', 2);",
            )
            .expect("create version ten schema and historical messages");
        for version in 1..=10 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record version ten migration history");
        }

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION).expect("upgrade version ten schema"),
            SCHEMA_VERSION
        );

        let saved_messages = connection
            .query_row("SELECT COUNT(*) FROM messages", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count preserved messages");
        assert_eq!(saved_messages, 2);
        let migration_version = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read backend migration version");
        assert_eq!(migration_version, SCHEMA_VERSION);

        let context_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_context_state
                 WHERE conversation_id = 'conversation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count old conversation context state");
        assert_eq!(context_rows, 0);

        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                 VALUES ('new-message', 'conversation', 'assistant', 'new indexed response', 'completed', 3)",
                [],
            )
            .expect("insert message after migration");
        let indexed_rows = connection
            .query_row("SELECT COUNT(*) FROM conversation_memory_fts", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count post-migration index rows");
        assert_eq!(indexed_rows, 1);
    }

    #[test]
    fn failed_tool_archive_migration_rolls_back_partial_schema_changes() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]'
                );
                CREATE TRIGGER conversation_memory_tool_insert
                AFTER INSERT ON messages
                BEGIN
                    SELECT 1;
                END;",
            )
            .expect("create version thirteen schema with a conflicting trigger");
        for version in 1..14 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record version thirteen migration history");
        }

        initialize_schema(&connection, SCHEMA_VERSION)
            .expect_err("conflicting trigger must abort the tool archive migration");
        for table in [
            "conversation_memory_tool_index_state",
            "conversation_memory_tools_fts",
        ] {
            let exists = connection
                .query_row(
                    "SELECT EXISTS (
                        SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                    )",
                    [table],
                    |row| row.get::<_, bool>(0),
                )
                .expect("check rolled-back migration table");
            assert!(!exists, "partial table {table} survived the rollback");
        }
        let migration_version = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read migration version after failure");
        assert_eq!(migration_version, 13);
    }

    #[test]
    fn failed_user_question_migration_rolls_back_partial_schema_changes() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE migration_trigger_conflict (id INTEGER);
                CREATE TRIGGER agent_runs_status_guard
                AFTER INSERT ON migration_trigger_conflict
                BEGIN
                    SELECT 1;
                END;",
            )
            .expect("create migration trigger fixture");
        for version in 1..=15 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record version fifteen migration history");
        }

        initialize_schema(&connection, SCHEMA_VERSION)
            .expect_err("conflicting trigger must abort the user question migration");
        for table in [
            "agent_runs",
            "pending_question_groups",
            "pending_question_items",
        ] {
            let exists = connection
                .query_row(
                    "SELECT EXISTS (
                        SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                    )",
                    [table],
                    |row| row.get::<_, bool>(0),
                )
                .expect("check rolled-back user question table");
            assert!(!exists, "partial table {table} survived the rollback");
        }
        let migration_version = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read migration version after user question failure");
        assert_eq!(migration_version, 15);
    }

    #[test]
    fn failed_user_question_hardening_migration_rolls_back_trigger_changes() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);",
            )
            .expect("create hardening migration fixture");
        for version in 1..=15 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record version fifteen migration history");
        }
        initialize_schema(&connection, 16).expect("apply user question migration");
        connection
            .execute_batch(
                "CREATE TABLE migration_trigger_conflict (id INTEGER);
                 CREATE TRIGGER agent_runs_checkpoint_byte_guard_insert
                 AFTER INSERT ON migration_trigger_conflict
                 BEGIN
                     SELECT 1;
                 END;",
            )
            .expect("create hardening trigger conflict");

        initialize_schema(&connection, SCHEMA_VERSION)
            .expect_err("conflicting hardening trigger must abort the migration");
        let migration_version = connection
            .query_row(
                "SELECT MAX(version) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read migration version after hardening failure");
        assert_eq!(migration_version, 16);
        let status_trigger_count = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'trigger' AND name = 'agent_runs_status_guard'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("check restored status trigger");
        assert_eq!(status_trigger_count, 1);
        let byte_trigger_sql = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger' AND name = 'agent_runs_checkpoint_byte_guard_insert'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read conflicting byte trigger");
        assert!(byte_trigger_sql.contains("migration_trigger_conflict"));
    }

    #[test]
    fn memory_fts_migration_indexes_new_completed_messages_and_tracks_updates() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );
                CREATE TABLE chatgpt_models (model_id TEXT);
                CREATE TABLE chatgpt_model_catalog_state (fetched_at_unix_ms INTEGER);
                CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                INSERT INTO conversations (id) VALUES ('conversation');
                INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                VALUES
                    ('before-migration', 'conversation', 'user', 'historical term', 'completed', 1);",
            )
            .expect("create pre-migration schema");
        for version in 1..13 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record prior migration");
        }

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION).expect("apply FTS migration"),
            SCHEMA_VERSION
        );
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                 VALUES ('new-message', 'conversation', 'assistant', 'live term', 'completed', 2)",
                [],
            )
            .expect("insert completed message");

        let indexed_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_fts
                 WHERE conversation_memory_fts.conversation_id = 'conversation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count newly indexed messages");
        assert_eq!(indexed_rows, 1);

        connection
            .execute(
                "UPDATE messages SET tool_activities = ?2 WHERE id = ?1",
                [
                    "new-message",
                    r#"[{"callId":"call-1","name":"read","arguments":{"path":"config"},"output":{"content":"toolquartz detail"},"status":"completed"}]"#,
                ],
            )
            .expect("index completed tool activity");
        let tool_matches = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"toolquartz\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("search indexed tool result");
        assert_eq!(tool_matches, 1);

        connection
            .execute(
                "UPDATE messages SET tool_activities = ?2 WHERE id = ?1",
                [
                    "new-message",
                    r#"[{"callId":"call-1","name":"read","arguments":{"path":"config"},"output":{"content":"toolzircon detail"},"status":"completed"}]"#,
                ],
            )
            .expect("update indexed tool activity");
        let revised_tool_matches = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"toolzircon\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("search revised tool result");
        assert_eq!(revised_tool_matches, 1);

        connection
            .execute(
                "UPDATE messages SET content = 'revised term' WHERE id = 'new-message'",
                [],
            )
            .expect("update indexed message");
        let revised_matches = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_fts
                 WHERE conversation_memory_fts MATCH 'content : \"revised\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("search revised content");
        assert_eq!(revised_matches, 1);

        connection
            .execute(
                "UPDATE messages SET status = 'failed' WHERE id = 'new-message'",
                [],
            )
            .expect("mark message incomplete");
        let remaining_rows = connection
            .query_row("SELECT COUNT(*) FROM conversation_memory_fts", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count remaining index rows");
        assert_eq!(remaining_rows, 0);
    }

    #[test]
    fn semantic_memory_migration_tracks_vectors_and_invalidates_edited_sources() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );
                CREATE TABLE chatgpt_models (model_id TEXT);
                CREATE TABLE chatgpt_model_catalog_state (fetched_at_unix_ms INTEGER);
                CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                    scope_token, content
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                    scope_token, content
                );
                INSERT INTO conversations (id) VALUES ('conversation-a');",
            )
            .expect("create schema fourteen fixture");
        for version in 1..=14 {
            connection
                .execute(
                    "INSERT INTO openchat_backend_migrations (version, applied_at_unix_ms)
                     VALUES (?1, 0)",
                    [version],
                )
                .expect("record prior schema version");
        }

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION).expect("apply semantic memory schema"),
            SCHEMA_VERSION
        );
        connection
            .execute(
                "INSERT INTO messages (conversation_id, id, role, content, status, created_at)
                 VALUES ('conversation-a', 'message-a', 'user', 'Original message.', 'completed', 1)",
                [],
            )
            .expect("insert completed historical message");
        let pending_count = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embedding_pending",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count queued embeddings");
        assert_eq!(pending_count, 1);

        connection
            .execute(
                "INSERT INTO conversation_memory_embedding_namespaces (conversation_id)
                 VALUES ('conversation-a')",
                [],
            )
            .expect("create conversation namespace");
        connection
            .execute(
                "INSERT INTO conversation_memory_embeddings (
                    conversation_id, message_id, chunk_index, start_byte, end_byte,
                    model_id, content_hash, vector
                 ) VALUES ('conversation-a', 'message-a', 0, 0, 16, 'model', 'hash', zeroblob(384))",
                [],
            )
            .expect("insert derived embedding");
        connection
            .execute(
                "INSERT INTO conversation_memory_embedding_state (
                    conversation_id, message_id, model_id, content_hash, chunk_count
                 ) VALUES ('conversation-a', 'message-a', 'model', 'hash', 1)",
                [],
            )
            .expect("record derived embedding state");
        connection
            .execute("DELETE FROM conversation_memory_embedding_pending", [])
            .expect("clear queued embedding after recording derived state");
        let generation = connection
            .query_row(
                "SELECT generation FROM conversation_memory_semantic_index_state WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read derived index generation");
        assert_eq!(generation, 2);

        connection
            .execute(
                "UPDATE messages SET tool_activities = '[]'
                 WHERE conversation_id = 'conversation-a' AND id = 'message-a'",
                [],
            )
            .expect("invalidate embeddings when tool activities change");
        let remaining_tool_change_embeddings = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embeddings",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count embeddings after tool update");
        assert_eq!(remaining_tool_change_embeddings, 0);
        let pending_after_tool_update = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embedding_pending",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count queued embeddings after tool update");
        assert_eq!(pending_after_tool_update, 1);

        connection
            .execute(
                "UPDATE messages SET content = 'Edited source message.'
                 WHERE conversation_id = 'conversation-a' AND id = 'message-a'",
                [],
            )
            .expect("invalidate embeddings when source changes");
        let remaining_embeddings = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embeddings",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count invalidated embeddings");
        assert_eq!(remaining_embeddings, 0);
        let source_content = connection
            .query_row(
                "SELECT content FROM messages WHERE conversation_id = 'conversation-a' AND id = 'message-a'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read source message after indexing");
        assert_eq!(source_content, "Edited source message.");
    }

    #[test]
    fn archive_index_controls_gate_new_message_and_tool_indexes() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );
                INSERT INTO conversations (id) VALUES ('conversation-a'), ('conversation-b');",
            )
            .expect("create host schema");
        assert_eq!(
            initialize_schema(&connection, 18).expect("apply schema through version 18"),
            18
        );
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                 VALUES ('prior', 'conversation-a', 'user', 'priorarchiveword', 'completed', 1)",
                [],
            )
            .expect("insert a pre-control completed message");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_fts
                     WHERE conversation_id = 'conversation-a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count previously indexed messages"),
            1
        );

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION).expect("apply archive index controls"),
            SCHEMA_VERSION
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_fts
                     WHERE conversation_id = 'conversation-a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("preserve existing archive entries by default"),
            1
        );

        connection
            .execute(
                "INSERT INTO conversation_memory_archive_settings (conversation_id, included)
                 VALUES ('conversation-a', 0)",
                [],
            )
            .expect("exclude one conversation");
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                 VALUES ('excluded', 'conversation-a', 'user', 'neverindexword', 'completed', 2)",
                [],
            )
            .expect("insert a message into the excluded conversation");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_fts
                     WHERE conversation_id = 'conversation-a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("count rows for excluded conversation"),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_memory_embedding_pending
                     WHERE conversation_id = 'conversation-a' AND message_id = 'excluded'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("check semantic pending rows for excluded conversation"),
            0
        );

        connection
            .execute(
                "INSERT INTO conversation_memory_excluded_tools (conversation_id, tool_name)
                 VALUES ('conversation-b', 'read')",
                [],
            )
            .expect("exclude one tool");
        connection
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, status, created_at, tool_activities
                 ) VALUES (
                    'tools', 'conversation-b', 'assistant', 'assistanttext', 'completed', 3,
                    '[{\"name\":\"read\",\"arguments\":{},\"output\":{\"content\":\"readsecretword\"},\"status\":\"completed\"},{\"name\":\"write\",\"arguments\":{},\"output\":{\"content\":\"writevisibleword\"},\"status\":\"completed\"}]'
                 )",
                [],
            )
            .expect("insert completed activity for included and excluded tools");
        let excluded_tool_hits = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"readsecretword\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("search excluded tool output");
        let included_tool_hits = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"writevisibleword\"'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("search included tool output");
        assert_eq!(excluded_tool_hits, 0);
        assert_eq!(included_tool_hits, 1);
    }

    #[test]
    fn credential_redaction_migration_rebuilds_derived_indexes_only() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );
                INSERT INTO conversations (id) VALUES ('archive-a');",
            )
            .expect("create host schema");
        assert_eq!(
            initialize_schema(&connection, 18).expect("apply schema through version 18"),
            18
        );
        assert_eq!(
            initialize_schema(&connection, 19).expect("apply schema through version 19"),
            19
        );

        let activities = r#"[{"name":"read","arguments":{"api_key":"argument-secret"},"output":{"content":"visiblearchiveword sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"},"status":"completed"}]"#;
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at, tool_activities)
                 VALUES ('message-a', 'archive-a', 'assistant', 'assistant text', 'completed', 1, ?1)",
                [activities],
            )
            .expect("insert a message indexed by the pre-redaction trigger");
        connection
            .execute(
                "INSERT INTO conversation_memory_embedding_namespaces (conversation_id)
                 VALUES ('archive-a')",
                [],
            )
            .expect("create semantic namespace");
        connection
            .execute(
                "INSERT INTO conversation_memory_embeddings (
                    conversation_id, message_id, chunk_index, start_byte, end_byte,
                    model_id, content_hash, vector
                 ) VALUES ('archive-a', 'message-a', 0, 0, 12, 'model', 'hash', zeroblob(384))",
                [],
            )
            .expect("create a vector that was derived from the raw tool activity");
        connection
            .execute(
                "INSERT INTO conversation_memory_embedding_state (
                    conversation_id, message_id, model_id, content_hash, chunk_count
                 ) VALUES ('archive-a', 'message-a', 'model', 'hash', 1)",
                [],
            )
            .expect("record the old semantic source");

        assert_eq!(
            initialize_schema(&connection, SCHEMA_VERSION)
                .expect("apply credential redaction migration"),
            SCHEMA_VERSION
        );
        let indexed_content: String = connection
            .query_row(
                "SELECT content FROM conversation_memory_tools_fts WHERE message_id = 'message-a'",
                [],
                |row| row.get(0),
            )
            .expect("read rebuilt tool index");
        assert!(indexed_content.contains("visiblearchiveword"));
        assert!(indexed_content.contains("[REDACTED]"));
        assert!(!indexed_content.contains("argument-secret"));
        assert!(!indexed_content.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"));

        let source_activities: String = connection
            .query_row(
                "SELECT tool_activities FROM messages WHERE id = 'message-a'",
                [],
                |row| row.get(0),
            )
            .expect("read original source activity");
        assert!(source_activities.contains("argument-secret"));
        assert!(source_activities.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"));

        let derived_embeddings: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embeddings
                 WHERE conversation_id = 'archive-a' AND message_id = 'message-a'",
                [],
                |row| row.get(0),
            )
            .expect("count cleared stale vectors");
        assert_eq!(derived_embeddings, 0);
        let queued: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_memory_embedding_pending
                 WHERE conversation_id = 'archive-a' AND message_id = 'message-a'",
                [],
                |row| row.get(0),
            )
            .expect("count requeued sanitized vectors");
        assert_eq!(queued, 1);

        let updated_activities = r#"[{"name":"read","arguments":{},"output":{"client_secret":"new-secret","content":"newvisibleword"},"status":"completed"}]"#;
        connection
            .execute(
                "UPDATE messages SET tool_activities = ?1 WHERE id = 'message-a'",
                [updated_activities],
            )
            .expect("exercise redacted update trigger");
        let updated_index: String = connection
            .query_row(
                "SELECT content FROM conversation_memory_tools_fts WHERE message_id = 'message-a'",
                [],
                |row| row.get(0),
            )
            .expect("read updated tool index");
        assert!(updated_index.contains("newvisibleword"));
        assert!(updated_index.contains("[REDACTED]"));
        assert!(!updated_index.contains("new-secret"));

        let inserted_activities = r#"[{"name":"read","arguments":{},"output":{"content":"insertedvisibleword","api_key":"inserted-secret"},"status":"completed"}]"#;
        connection
            .execute(
                "INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at, tool_activities)
                 VALUES ('message-b', 'archive-a', 'assistant', 'assistant text', 'completed', 2, ?1)",
                [inserted_activities],
            )
            .expect("exercise redacted insert trigger");
        let inserted_index: String = connection
            .query_row(
                "SELECT content FROM conversation_memory_tools_fts WHERE message_id = 'message-b'",
                [],
                |row| row.get(0),
            )
            .expect("read inserted tool index");
        assert!(inserted_index.contains("insertedvisibleword"));
        assert!(inserted_index.contains("[REDACTED]"));
        assert!(!inserted_index.contains("inserted-secret"));
    }
}
