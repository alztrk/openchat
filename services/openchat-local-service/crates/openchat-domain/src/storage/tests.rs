use std::{fs, path::PathBuf};

use rusqlite::Connection;
use uuid::Uuid;

use super::{
    AppStorage, CopilotCatalogFilterSummary, MAX_DIAGNOSTIC_LOG_BYTES,
    MINIMUM_SQLITE_VERSION_NUMBER, database_guard, database_key::DatabaseKey,
    disable_trusted_schema, sqlite_error_code, validate_sqlite_runtime,
};

#[test]
fn open_rejects_corrupt_existing_database_before_wal_or_schema_writes() {
    let directory = TestDirectory::new();
    let database_directory = directory.0.join("db");
    fs::create_dir_all(&database_directory).expect("create database directory");
    let database_path = database_directory.join("openchat.sqlite3");
    let original = b"not an sqlite database";
    fs::write(&database_path, original).expect("write corrupt fixture");

    let error = match AppStorage::open_at(directory.0.clone()) {
        Ok(_) => panic!("corrupt database must stop startup"),
        Err(error) => error,
    };

    assert_eq!(sqlite_error_code(&error), "sqlite_cannot_open");
    assert_eq!(
        fs::read(&database_path).expect("read database fixture"),
        original
    );
    assert!(!database_directory.join("openchat.sqlite3-wal").exists());
}

#[test]
fn startup_encrypts_retained_plaintext_pre_migration_backups() {
    let directory = TestDirectory::new();
    let backup_directory = directory.0.join("db/backups");
    fs::create_dir_all(&backup_directory).expect("create backup directory");
    let backup_path = backup_directory.join("openchat.sqlite3.pre-migration-legacy.backup");
    let backup = Connection::open(&backup_path).expect("create legacy plaintext backup");
    backup
        .execute_batch(
            "CREATE TABLE backup_probe (value TEXT NOT NULL);
                 INSERT INTO backup_probe VALUES ('preserved');",
        )
        .expect("write backup fixture");
    drop(backup);

    let storage = AppStorage::open_at(directory.0.clone()).expect("open isolated profile");

    let header = fs::read(&backup_path).expect("read converted backup");
    assert_ne!(&header[..16], b"SQLite format 3\0");
    let converted = DatabaseKey::isolated_test_key()
        .open(&backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("open converted backup with profile key");
    assert_eq!(
        converted
            .query_row("SELECT value FROM backup_probe", [], |row| row
                .get::<_, String>(0))
            .expect("read converted backup data"),
        "preserved"
    );
    drop(converted);
    drop(storage);
}

#[cfg(windows)]
#[test]
fn legacy_profile_database_sources_are_encrypted_without_creating_missing_files() {
    let directory = TestDirectory::new();
    let key = DatabaseKey::isolated_test_key();
    let legacy_paths = [
        directory.0.join("db/openchat.sqlite3"),
        directory.0.join("db/local_models.sqlite3"),
    ];
    for path in &legacy_paths {
        fs::create_dir_all(path.parent().expect("legacy database parent"))
            .expect("create legacy database parent");
        let database = Connection::open(path).expect("create legacy plaintext database");
        database
            .execute_batch(
                "CREATE TABLE retained_probe (value TEXT NOT NULL);
                     INSERT INTO retained_probe VALUES ('preserved');",
            )
            .expect("write legacy database fixture");
    }

    for path in &legacy_paths {
        super::protect_legacy_database_file(path, &key).expect("protect retained legacy database");
        let bytes = fs::read(path).expect("read protected legacy database");
        assert_ne!(&bytes[..16], b"SQLite format 3\0");
        let database = key
            .open(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open protected legacy database");
        assert_eq!(
            database
                .query_row("SELECT value FROM retained_probe", [], |row| row
                    .get::<_, String>(0))
                .expect("read preserved legacy value"),
            "preserved"
        );
    }
    let missing_path = directory.0.join("cache/missing.sqlite3");
    super::protect_legacy_database_file(&missing_path, &key)
        .expect("ignore an absent legacy source");
    assert!(!missing_path.exists());
}

#[test]
fn app_connections_disable_trusted_schema() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");
    let connection = storage.connect().expect("open configured connection");
    let trusted_schema_enabled = connection
        .query_row("PRAGMA trusted_schema", [], |row| row.get::<_, bool>(0))
        .expect("read trusted schema setting");

    assert!(!trusted_schema_enabled);
}

#[test]
fn trusted_schema_blocks_non_innocuous_functions_in_triggers() {
    let connection = Connection::open_in_memory().expect("open in-memory database");
    disable_trusted_schema(&connection).expect("disable trusted schema");
    connection
        .create_scalar_function(
            "openchat_test_external_function",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |_| Ok("untrusted"),
        )
        .expect("register non-innocuous function");
    connection
        .execute_batch(
            "CREATE TABLE source (value TEXT NOT NULL);
                 CREATE TRIGGER invoke_external AFTER INSERT ON source
                 BEGIN
                     SELECT openchat_test_external_function();
                 END;",
        )
        .expect("create trigger");

    assert!(
        connection
            .execute("INSERT INTO source VALUES ('value')", [])
            .is_err()
    );
}

#[test]
fn pending_chat_migration_gets_one_verified_backup_before_drift_opens() {
    let directory = TestDirectory::new();
    let database_directory = directory.0.join("db");
    fs::create_dir_all(&database_directory).expect("create database directory");
    let database_path = database_directory.join("openchat.sqlite3");
    let mut initial = Connection::open(&database_path).expect("create existing database");
    initial
        .execute_batch(
            "CREATE TABLE migration_probe (value TEXT NOT NULL);
                 INSERT INTO migration_probe VALUES ('preserved');
                 CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    folder_path TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    title_source TEXT NOT NULL DEFAULT 'automatic',
                    is_pinned INTEGER NOT NULL DEFAULT 0,
                    connection_id TEXT,
                    workspace_id TEXT,
                    project_id TEXT,
                    provider_id TEXT,
                    api_key_connection_id TEXT,
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
                    status TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, id)
                 );
                 CREATE TABLE model_favorites (
                    provider_id TEXT NOT NULL,
                    model_id TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    favorited_at INTEGER NOT NULL,
                    source_connection_id TEXT,
                    PRIMARY KEY (provider_id, model_id)
                 );
                 PRAGMA user_version = 9;",
        )
        .expect("prepare database one chat migration behind");
    super::migrate_chat_schema(&mut initial).expect("prepare chat schema for backend migration");
    super::initialize_schema(&initial, 17).expect("prepare backend schema at version 17");
    initial
        .pragma_update(None, "user_version", 9)
        .expect("mark chat migration pending");
    drop(initial);

    let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");

    assert!(
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare chat migration")
    );
    assert!(
        !storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("avoid duplicate startup backup")
    );

    let backups = fs::read_dir(database_directory.join("backups"))
        .expect("list migration backups")
        .collect::<Result<Vec<_>, _>>()
        .expect("read migration backup entries");
    assert_eq!(backups.len(), 1);
    let test_key = DatabaseKey::isolated_test_key();
    let backup = test_key
        .open(
            &backups[0].path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open verified backup");
    let user_version =
        database_guard::chat_schema_version(&backup).expect("read backup schema version");
    let value = backup
        .query_row("SELECT value FROM migration_probe", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("read preserved user data");

    assert_eq!(user_version, 9);
    assert_eq!(value, "preserved");
    database_guard::verify_integrity(&backup).expect("backup passes integrity check");
}

#[test]
fn diagnostic_log_stays_bounded_and_records_only_safe_runtime_fields() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
    let log_path = directory.0.join("logs").join("openchat-service.log");
    fs::write(&log_path, vec![b'x'; MAX_DIAGNOSTIC_LOG_BYTES as usize])
        .expect("fill diagnostic log to its size limit");

    storage
        .log_local_engine_event(
            "runtime_started",
            None,
            Some(std::time::Duration::from_millis(25)),
        )
        .expect("write safe runtime diagnostic");

    let entry = fs::read_to_string(&log_path).expect("read diagnostic log");
    assert!(entry.len() < 256);
    assert!(entry.contains("component=local_engine event=runtime_started"));
    assert!(entry.contains("duration_ms=25"));
    assert!(!entry.contains("modelPath"));
    assert!(!entry.contains("prompt"));
}

#[test]
fn rpc_failure_diagnostic_records_only_operation_and_error_code() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");

    storage
        .log_rpc_failure("chat.send", "provider_credentials_missing")
        .expect("write RPC failure diagnostic");

    let log_path = directory.0.join("logs").join("openchat-service.log");
    let entry = fs::read_to_string(log_path).expect("read diagnostic log");
    assert!(entry.contains("component=rpc event=request_failed"));
    assert!(entry.contains("operation=chat.send code=provider_credentials_missing"));
    assert!(!entry.contains("prompt"));
    assert!(!entry.contains("apiKey"));
}

#[test]
fn sqlite_error_diagnostics_use_stable_codes_without_error_details() {
    let busy_error = rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
        Some("private database detail".to_owned()),
    );

    assert_eq!(sqlite_error_code(&busy_error), "sqlite_busy");
    assert_eq!(
        sqlite_error_code(&rusqlite::Error::InvalidQuery),
        "invalid_query"
    );
}

#[test]
fn copilot_catalog_diagnostic_rejects_unbounded_stage_values() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");

    assert!(
        storage
            .log_copilot_catalog_failure("model_data", "invalid_provider_response")
            .is_ok()
    );
    assert!(
        storage
            .log_copilot_catalog_failure("private user data", "invalid_provider_response")
            .is_err()
    );

    let log_path = directory.0.join("logs").join("openchat-service.log");
    let entry = fs::read_to_string(log_path).expect("read diagnostic log");
    assert!(entry.contains("component=copilot event=model_catalog_failed"));
    assert!(entry.contains("stage=model_data code=invalid_provider_response"));
    assert!(!entry.contains("private user data"));
}

#[test]
fn copilot_catalog_filter_diagnostic_records_only_bounded_aggregate_counts() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
    let summary = CopilotCatalogFilterSummary {
        total: 8,
        policy_blocked: 1,
        non_chat: 2,
        picker_disabled: 3,
        invalid_id: 1,
        auto_candidates: 1,
    };

    storage
        .log_copilot_catalog_filter_summary(&summary)
        .expect("write bounded catalog summary");
    let log_path = directory.0.join("logs").join("openchat-service.log");
    let entry = fs::read_to_string(&log_path).expect("read diagnostic log");
    assert!(entry.contains(
            "component=copilot event=model_catalog_filter_summary total=8 policy_blocked=1 non_chat=2 picker_disabled=3 invalid_id=1 auto_candidates=1"
        ));
    assert!(
        storage
            .log_copilot_catalog_filter_summary(&CopilotCatalogFilterSummary {
                total: 1025,
                policy_blocked: 0,
                non_chat: 0,
                picker_disabled: 0,
                invalid_id: 0,
                auto_candidates: 0,
            })
            .is_err()
    );
}

#[test]
fn database_runtime_diagnostic_records_engine_and_schema_versions_without_paths() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");

    storage
        .log_database_runtime(10)
        .expect("write database runtime diagnostic");

    let log_path = directory.0.join("logs").join("openchat-service.log");
    let entry = fs::read_to_string(log_path).expect("read diagnostic log");
    assert!(entry.contains("component=database event=runtime_initialized"));
    assert!(entry.contains(&format!(
        "sqlite_version={} backend_schema_version={} chat_schema_version=10",
        rusqlite::version(),
        storage.schema_version(),
    )));
    assert!(!entry.contains(directory.0.to_string_lossy().as_ref()));
}

#[test]
fn bundled_sqlite_runtime_includes_the_wal_reset_fix() {
    let runtime_version = rusqlite::version_number();
    assert!(
        runtime_version >= MINIMUM_SQLITE_VERSION_NUMBER,
        "Rust SQLite runtime {} is below the required 3.51.3 minimum",
        rusqlite::version()
    );
    validate_sqlite_runtime(runtime_version, rusqlite::version())
        .expect("production runtime guard accepts the bundled engine");
}

#[test]
fn sqlite_runtime_guard_rejects_versions_before_opening_database() {
    let error = validate_sqlite_runtime(3_051_002, "3.51.2")
        .expect_err("SQLite 3.51.2 still contains the WAL-reset race");

    assert!(error.to_string().contains("sqlite_runtime_unsupported"));
    assert!(error.to_string().contains("3.51.2"));
    validate_sqlite_runtime(MINIMUM_SQLITE_VERSION_NUMBER, "3.51.3")
        .expect("the fixed SQLite version is supported");
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("openchat-storage-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}
