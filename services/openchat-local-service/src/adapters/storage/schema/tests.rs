//! Migration, rollback, and data integrity tests for SQLite schema versions.

use super::*;
use rusqlite::Connection;

const BOOKMARK_COLUMN_MIGRATION_VERSION: i64 = 29;

#[test]
fn migration_33_adds_durable_chatgpt_credential_cleanup_queue() {
    let connection = memory_index_repair_fixture(32);

    assert_eq!(
        initialize_schema(&connection, 33).expect("add credential cleanup queue"),
        33
    );
    connection
        .execute(
            "INSERT INTO pending_chatgpt_credential_deletions (
                connection_id, credential_reference, requested_at_unix_ms
             ) VALUES ('connection-1', 'credential-reference', 1)",
            [],
        )
        .expect("persist pending credential cleanup");
    assert_eq!(
        connection
            .query_row(
                "SELECT credential_reference FROM pending_chatgpt_credential_deletions
                 WHERE connection_id = 'connection-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read pending cleanup"),
        "credential-reference"
    );
}

#[test]
fn latest_schema_recreates_missing_conversation_context_state() {
    let connection = memory_index_repair_fixture(SCHEMA_VERSION);
    connection
        .execute_batch("DROP TABLE conversation_context_state")
        .expect("simulate a missing migration table");

    assert_eq!(
        initialize_schema(&connection, SCHEMA_VERSION).expect("repair current schema"),
        SCHEMA_VERSION
    );
    connection
        .execute(
            "INSERT INTO conversation_context_state (
                conversation_id, last_prompt_tokens, last_prompt_message_id,
                last_prompt_provider_id, last_prompt_model_id,
                last_prompt_connection_id, last_prompt_workspace_id, updated_at_unix_ms
             ) VALUES ('conversation', 17, 'assistant-message', 'chatgpt', 'model',
                       'connection', 'workspace', 1)",
            [],
        )
        .expect("write usage metadata into repaired table");

    let route = connection
        .query_row(
            "SELECT last_prompt_tokens, last_prompt_connection_id, last_prompt_workspace_id
             FROM conversation_context_state WHERE conversation_id = 'conversation'",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .expect("read usage metadata from repaired table");
    assert_eq!(route, (17, "connection".to_owned(), "workspace".to_owned()));
}

#[test]
fn migration_40_indexes_usage_event_export_order() {
    let connection = memory_index_repair_fixture(39);

    assert_eq!(
        initialize_schema(&connection, SCHEMA_VERSION).expect("add usage event export index"),
        SCHEMA_VERSION
    );

    let plan = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT event_id FROM usage_events
             WHERE started_at_unix_ms >= ?1 AND started_at_unix_ms < ?2
             ORDER BY started_at_unix_ms DESC, event_id DESC
             LIMIT ?3 OFFSET ?4",
        )
        .expect("prepare usage event export plan")
        .query_map([0_i64, 100, 1000, 0], |row| row.get::<_, String>(3))
        .expect("explain usage event export query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("read usage event export query plan");

    assert!(
        plan.iter()
            .any(|detail| detail.contains("usage_events_started_desc_event_id_desc_idx")),
        "usage event export did not select the new index: {plan:?}"
    );
    assert!(
        plan.iter().all(|detail| !detail.contains("TEMP B-TREE")),
        "usage event export still sorts into a temporary B-tree: {plan:?}"
    );
}

#[test]
fn migration_43_indexes_usage_event_filter_options() {
    let connection = memory_index_repair_fixture(SCHEMA_VERSION - 1);

    assert_eq!(
        initialize_schema(&connection, SCHEMA_VERSION).expect("add filter option indexes"),
        SCHEMA_VERSION
    );

    for (sql, expected_index) in [
        (
            "SELECT DISTINCT model_id FROM usage_events
             WHERE event_kind = 'request' AND model_id IS NOT NULL ORDER BY model_id",
            "usage_events_model_filter_options_idx",
        ),
        (
            "SELECT DISTINCT COALESCE(reasoning_effort, 'unspecified')
             FROM usage_events WHERE event_kind = 'request' ORDER BY 1",
            "usage_events_reasoning_filter_options_idx",
        ),
    ] {
        let plan = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("prepare filter options query plan")
            .query_map([], |row| row.get::<_, String>(3))
            .expect("explain filter options query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("read filter options query plan");

        assert!(
            plan.iter().any(|detail| detail.contains(expected_index)),
            "filter options did not select {expected_index}: {plan:?}"
        );
        assert!(
            plan.iter().all(|detail| !detail.contains("TEMP B-TREE")),
            "filter options still sorts into a temporary B-tree: {plan:?}"
        );
    }
}

#[test]
fn migration_44_creates_the_project_symbol_lookup_index() {
    let connection = memory_index_repair_fixture(42);
    connection
        .execute(
            "INSERT INTO project_index_files (project_id, path, modified_ns, size, content)
             VALUES ('project-1', 'src/lib.rs', 1, 0, '')",
            [],
        )
        .expect("store an existing indexed project file");
    connection
        .execute(
            "INSERT INTO project_index_state
                (project_id, enabled, status, indexed_files, indexed_bytes)
             VALUES ('project-1', 1, 'current', 1, 0)",
            [],
        )
        .expect("store an existing project index state");

    assert_eq!(
        initialize_schema(&connection, 44).expect("add project symbol index"),
        44
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT symbols_indexed FROM project_index_files
                 WHERE project_id = 'project-1' AND path = 'src/lib.rs'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .expect("read the symbol index state"),
        false
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM project_index_state WHERE project_id = 'project-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read the migrated index status"),
        "stale"
    );
    connection
        .execute(
            "INSERT INTO project_index_symbols
                (project_id, path, name, kind, start_line, end_line)
             VALUES ('project-1', 'src/lib.rs', 'launch', 'function', 4, 8)",
            [],
        )
        .expect("store an indexed symbol");

    assert_eq!(
        connection
            .query_row(
                "SELECT name, start_line, end_line FROM project_index_symbols
                 WHERE project_id = 'project-1' AND name = 'launch'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("read indexed symbol"),
        ("launch".to_owned(), 4, 8)
    );
}

#[test]
fn migration_45_adds_checkpoint_recovery_state() {
    let connection = memory_index_repair_fixture(44);
    let checkpoint_columns = |connection: &Connection| {
        let mut statement = connection
            .prepare("PRAGMA table_info(conversation_file_checkpoints)")
            .expect("inspect checkpoint columns");
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("read checkpoint columns")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect checkpoint columns")
    };

    let before_columns = checkpoint_columns(&connection);
    assert!(
        !before_columns
            .iter()
            .any(|column| column == "recovery_available")
    );
    assert!(
        !before_columns
            .iter()
            .any(|column| column == "recovered_at_unix_ms")
    );
    assert_eq!(
        initialize_schema(&connection, 45).expect("add checkpoint recovery state"),
        45
    );
    let after_columns = checkpoint_columns(&connection);
    assert!(
        after_columns
            .iter()
            .any(|column| column == "recovery_available")
    );
    assert!(
        after_columns
            .iter()
            .any(|column| column == "recovered_at_unix_ms")
    );
}

#[test]
fn migration_46_versions_project_symbols_without_reindexing_existing_languages() {
    let connection = memory_index_repair_fixture(45);
    connection
        .execute_batch(
            "INSERT INTO project_index_files
                (project_id, path, modified_ns, size, content, symbols_indexed)
             VALUES
                ('project-1', 'src/lib.rs', 1, 0, '', 1),
                ('project-1', 'src/App.dart', 1, 0, '', 1);
             INSERT INTO project_index_state
                (project_id, enabled, status, indexed_files, indexed_bytes)
             VALUES ('project-1', 1, 'current', 2, 0);",
        )
        .expect("store pre-migration project index data");

    assert_eq!(
        initialize_schema(&connection, 46).expect("version project symbol extraction"),
        46
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT symbol_version FROM project_index_files
                 WHERE project_id = 'project-1' AND path = 'src/lib.rs'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read the existing language symbol version"),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT symbol_version FROM project_index_files
                 WHERE project_id = 'project-1' AND path = 'src/App.dart'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read the new grammar symbol version"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM project_index_state WHERE project_id = 'project-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read the migrated project index state"),
        "stale"
    );
}

#[test]
fn migration_48_preserves_legacy_file_records_without_root_identity() {
    let connection = memory_index_repair_fixture(47);
    connection
        .execute_batch(
            "INSERT INTO conversation_file_changes (
                conversation_id, id, root, path, baseline_exists, expected_exists,
                is_binary, diff_available, status
             ) VALUES (
                'conversation', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                'C:/workspace', 'note.txt', 0, 0, 0, 1, 'active'
             );
             INSERT INTO conversation_file_checkpoints (
                id, conversation_id, sequence, created_at_unix_ms, tool_name
             ) VALUES ('bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'conversation', 1, 1, 'write_file');
             INSERT INTO conversation_file_checkpoint_files (
                checkpoint_id, file_id, root, path,
                before_exists, after_exists
             ) VALUES (
                'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                'C:/workspace', 'note.txt', 0, 0
             );",
        )
        .expect("insert legacy file tracking records");

    assert_eq!(
        initialize_schema(&connection, 48).expect("add stored workspace identities"),
        48
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT root_identity FROM conversation_file_changes
                 WHERE conversation_id = 'conversation'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("read legacy tracked change identity"),
        None
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT root_identity FROM conversation_file_checkpoint_files
                 WHERE checkpoint_id = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("read legacy checkpoint identity"),
        None
    );
}

#[test]
fn migration_49_marks_existing_permission_settings_as_unconfirmed() {
    let connection = memory_index_repair_fixture(48);
    connection
        .execute_batch(
            "INSERT INTO tool_permission_mode (preference_id, mode)
             VALUES (1, 'full_access');
             INSERT INTO project_tool_permission_rules (project_id, rules_json)
             VALUES ('project-1', '{\"read_file\":\"allow\"}');",
        )
        .expect("insert legacy elevated permission settings");

    assert_eq!(
        initialize_schema(&connection, 49).expect("add permission confirmation provenance"),
        49
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT mode, native_confirmed FROM tool_permission_mode
                 WHERE preference_id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
            )
            .expect("read preserved legacy global policy"),
        ("full_access".to_owned(), false)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT rules_json, native_confirmed FROM project_tool_permission_rules
                 WHERE project_id = 'project-1'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
            )
            .expect("read preserved legacy project policy"),
        ("{\"read_file\":\"allow\"}".to_owned(), false)
    );
}

#[test]
fn migration_50_adds_nullable_first_output_latency() {
    let connection = memory_index_repair_fixture(49);
    connection
        .execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, operation, status,
                usage_source, started_at_unix_ms
             ) VALUES ('existing-request', 'request', 'conversation', 'chat',
                       'completed', 'unavailable', 1)",
            [],
        )
        .expect("insert a pre-migration usage event");

    assert_eq!(
        initialize_schema(&connection, 50).expect("add first output latency"),
        50
    );

    let historical_latency = connection
        .query_row(
            "SELECT first_output_latency_ms FROM usage_events
             WHERE event_id = 'existing-request'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .expect("read historical event latency");
    assert_eq!(historical_latency, None);

    assert!(
        connection
            .execute(
                "UPDATE usage_events SET first_output_latency_ms = -1
             WHERE event_id = 'existing-request'",
                [],
            )
            .is_err()
    );

    let migration_version = connection
        .query_row(
            "SELECT MAX(version) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("read migration version");
    assert_eq!(migration_version, 50);
}

#[test]
fn migration_51_adds_nullable_usage_attribution() {
    let connection = memory_index_repair_fixture(50);
    connection
        .execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, operation, status,
                usage_source, started_at_unix_ms
             ) VALUES ('existing-request', 'request', 'conversation', 'chat',
                       'completed', 'unavailable', 1)",
            [],
        )
        .expect("insert a pre-migration usage event");

    assert_eq!(
        initialize_schema(&connection, 51).expect("add usage attribution"),
        51
    );

    let attribution = connection
        .query_row(
            "SELECT connection_id, workspace_id, project_id, parent_run_id
             FROM usage_events WHERE event_id = 'existing-request'",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .expect("read historical usage attribution");
    assert_eq!(attribution, (None, None, None, None));

    let migration_version = connection
        .query_row(
            "SELECT MAX(version) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("read migration version");
    assert_eq!(migration_version, 51);
}

#[test]
fn migration_52_adds_nullable_provider_processing_time() {
    let connection = memory_index_repair_fixture(51);
    connection
        .execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, operation, status,
                usage_source, started_at_unix_ms
             ) VALUES ('existing-request', 'request', 'conversation', 'chat',
                       'completed', 'unavailable', 1)",
            [],
        )
        .expect("insert a pre-migration usage event");

    assert_eq!(
        initialize_schema(&connection, 52).expect("add provider processing time"),
        52
    );

    let historical_processing_time = connection
        .query_row(
            "SELECT provider_processing_ms, provider_queue_time_ms,
                    provider_input_processing_ms, provider_output_processing_ms
             FROM usage_events
             WHERE event_id = 'existing-request'",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .expect("read historical provider timing fields");
    assert_eq!(historical_processing_time, (None, None, None, None));

    for column in [
        "provider_processing_ms",
        "provider_queue_time_ms",
        "provider_input_processing_ms",
        "provider_output_processing_ms",
    ] {
        assert!(
            connection
                .execute(
                    &format!(
                        "UPDATE usage_events SET {column} = -1
                     WHERE event_id = 'existing-request'"
                    ),
                    [],
                )
                .is_err()
        );
    }

    let migration_version = connection
        .query_row(
            "SELECT MAX(version) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("read migration version");
    assert_eq!(migration_version, 52);
}

#[test]
fn migration_53_adds_nullable_bounded_provider_completion_reason() {
    let connection = memory_index_repair_fixture(52);
    connection
        .execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, operation, status,
                usage_source, started_at_unix_ms
             ) VALUES ('existing-request', 'request', 'conversation', 'chat',
                       'completed', 'unavailable', 1)",
            [],
        )
        .expect("insert a pre-migration usage event");

    assert_eq!(
        initialize_schema(&connection, 53).expect("add completion reason"),
        53
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT completion_reason FROM usage_events
                 WHERE event_id = 'existing-request'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("read historical completion reason"),
        None
    );
    connection
        .execute(
            "UPDATE usage_events SET completion_reason = 'tool_calls'
             WHERE event_id = 'existing-request'",
            [],
        )
        .expect("store a bounded completion reason");

    let too_long_reason = "x".repeat(65);
    for invalid_reason in [
        "reason with spaces",
        "reason/with/slashes",
        too_long_reason.as_str(),
    ] {
        assert!(
            connection
                .execute(
                    "UPDATE usage_events SET completion_reason = ?1
                     WHERE event_id = 'existing-request'",
                    [invalid_reason],
                )
                .is_err(),
            "invalid reason should be rejected: {invalid_reason}"
        );
    }
}

#[test]
fn migration_54_adds_bounded_provider_rate_limit_observations() {
    let connection = memory_index_repair_fixture(53);
    connection
        .execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, operation, status,
                usage_source, started_at_unix_ms
             ) VALUES ('existing-request', 'request', 'conversation', 'chat',
                       'completed', 'unavailable', 1)",
            [],
        )
        .expect("insert a pre-migration usage event");

    assert_eq!(
        initialize_schema(&connection, 54).expect("add provider rate-limit observations"),
        54
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT provider_rate_limits_json FROM usage_events
                 WHERE event_id = 'existing-request'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("read historical provider rate-limit observation"),
        None
    );
    connection
        .execute(
            "UPDATE usage_events SET provider_rate_limits_json = ?1
             WHERE event_id = 'existing-request'",
            [r#"{"observedAtUnixMs":1,"requestLimit":10}"#],
        )
        .expect("store a bounded provider rate-limit observation");
    assert!(
        connection
            .execute(
                "UPDATE usage_events SET provider_rate_limits_json = ?1
             WHERE event_id = 'existing-request'",
                ["not-json"],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE usage_events SET provider_rate_limits_json = ?1
             WHERE event_id = 'existing-request'",
                ["x".repeat(2049)],
            )
            .is_err()
    );
}

#[test]
fn migration_55_disables_public_web_research_for_existing_security_scans() {
    let connection = memory_index_repair_fixture(54);
    assert_eq!(
        initialize_schema(&connection, 55).expect("add security scan web research consent"),
        55
    );
    let default_value = connection
        .query_row(
            "SELECT dflt_value FROM pragma_table_info('security_scans')
             WHERE name = 'public_web_research'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .expect("read security scan consent default");
    assert_eq!(default_value.as_deref(), Some("0"));
}

#[test]
fn migration_56_adds_a_separate_security_scan_attempt_id() {
    let connection = memory_index_repair_fixture(55);
    assert_eq!(
        initialize_schema(&connection, 56).expect("add security scan attempt identity"),
        56
    );
    let attempt_column = connection
        .query_row(
            "SELECT type, dflt_value FROM pragma_table_info('security_scans')
             WHERE name = 'active_attempt_id'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("read scan attempt column");
    assert_eq!(attempt_column, ("TEXT".to_owned(), None));
}

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
fn migration_31_adds_service_owned_tool_permission_settings() {
    let connection = memory_index_repair_fixture(30);
    initialize_schema(&connection, 31).expect("add tool permission settings");

    let migrated_version = connection
        .query_row(
            "SELECT MAX(version) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("read migration version");
    assert_eq!(migrated_version, 31);

    connection
        .execute(
            "INSERT INTO tool_permission_mode (preference_id, mode) VALUES (1, 'full_access')",
            [],
        )
        .expect("persist a supported global mode");
    assert!(
        connection
            .execute(
                "INSERT INTO project_tool_permission_rules (project_id, rules_json)
                 VALUES ('project-1', '{\"read_file\":\"deny\"}')",
                [],
            )
            .is_ok()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO project_tool_permission_rules (project_id, rules_json)
                 VALUES ('project-2', '[]')",
                [],
            )
            .is_err()
    );
}

#[test]
fn migration_32_preserves_and_selects_the_existing_copilot_connection() {
    let connection = memory_index_repair_fixture(31);
    connection
        .execute(
            "INSERT INTO copilot_oauth_connection (
                id, github_user_id, login, display_name, credential_reference, auth_status,
                access_expires_at_unix_seconds, refresh_expires_at_unix_seconds,
                connected_at_unix_ms, updated_at_unix_ms
             ) VALUES ('copilot', '123456', 'migration-user', NULL, '123456:generation', 'active',
                       1_800_000_000, NULL, 100, 200)",
            [],
        )
        .expect("insert the pre-migration Copilot connection");

    initialize_schema(&connection, SCHEMA_VERSION).expect("upgrade the Copilot connection schema");

    let migrated_connection = connection
        .query_row(
            "SELECT id, github_user_id, login, auth_status, is_selected
             FROM copilot_oauth_connection",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .expect("read the migrated Copilot connection");

    assert_eq!(
        migrated_connection,
        (
            "123456".to_owned(),
            "123456".to_owned(),
            "migration-user".to_owned(),
            "active".to_owned(),
            1,
        )
    );
}

#[test]
fn upgrades_conversation_bookmarks_without_duplicating_the_drift_column() {
    let version_before_bookmark_column = BOOKMARK_COLUMN_MIGRATION_VERSION - 1;
    let connection = memory_index_repair_fixture(version_before_bookmark_column);
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

    let frontend_migrated = memory_index_repair_fixture(version_before_bookmark_column);
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
        initialize_schema(&connection, SCHEMA_VERSION).expect("apply question identity migration"),
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
    assert!(
        connection
            .execute(
                "INSERT INTO pending_question_items (group_id, item_id, position, question_json)
                 VALUES ('first', 'mode', 1, '{\"id\":\"mode\"}')",
                [],
            )
            .is_err()
    );
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
