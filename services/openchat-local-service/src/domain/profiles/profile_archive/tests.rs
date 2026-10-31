//! Tests for profile archive export, restore staging, and recovery.

use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};

use rusqlite::{Connection, params};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{
    ARCHIVE_FORMAT, ArchiveFailure, DATABASE_PATH, FileRecord, MAX_DATABASE_BYTES,
    MAX_RESTORE_STATE_BYTES, ProfileManifest, count_rows, export, export_inner, file_record,
    prepare_restore, read_backend_schema_version, read_json, unix_time_millis, write_archive,
};
use crate::storage::AppStorage;

const CHAT_SCHEMA_VERSION: i64 = 11;
const PASSPHRASE: &str = "profile-archive-test-passphrase";
static RESTORE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn encrypted_profile_backup_restores_database_and_attachments_safely() {
    let _guard = RESTORE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source_directory = TestDirectory::new("source");
    let source_key = [0x11; 32];
    let source = seed_profile(&source_directory.0, "source-chat", true, &source_key);
    let local_snapshot_path = add_workspace_file_change(&source);
    assert!(local_snapshot_path.is_file());
    let export_result =
        export(&source, PASSPHRASE.to_owned().into()).expect("export encrypted profile");
    let export_id = export_result["exportId"]
        .as_str()
        .expect("service should return a pending archive identifier");
    let archive_path = source
        .root()
        .join("cache/archive-exports")
        .join(format!("{export_id}.openchatprofilebackup"));
    assert_eq!(export_id.len(), 32);
    assert_eq!(export_result["conversationCount"], 1);
    assert_eq!(export_result["attachmentCount"], 1);
    assert_eq!(export_result["requiresRestart"], false);
    assert_eq!(workspace_file_change_count(&source), 1);
    assert_eq!(workspace_file_blob_count(&source), 1);
    assert!(local_snapshot_path.is_file());

    let destination_directory = TestDirectory::new("destination");
    let destination_key = [0x22; 32];
    let destination = seed_profile(
        &destination_directory.0,
        "existing-chat",
        true,
        &destination_key,
    );
    let invalid_passphrase = prepare_restore(
        &destination,
        archive_path.to_str().expect("archive path is UTF-8"),
        "wrong-profile-passphrase".to_owned().into(),
        CHAT_SCHEMA_VERSION,
    )
    .expect_err("reject an incorrect passphrase");
    assert_eq!(invalid_passphrase.code, "profile_archive_invalid");
    assert_eq!(read_chat_count(&destination), 1);

    let restore_result = prepare_restore(
        &destination,
        archive_path.to_str().expect("archive path is UTF-8"),
        PASSPHRASE.to_owned().into(),
        CHAT_SCHEMA_VERSION,
    )
    .expect("validate and stage profile restore");
    assert_eq!(restore_result["requiresRestart"], true);
    assert_eq!(read_chat_count(&destination), 1);
    drop(destination);

    let restored = AppStorage::open_at_with_key(
        destination_directory.0.clone(),
        crate::storage::DatabaseKey::from_bytes(&destination_key)
            .expect("restore destination database key"),
    )
    .expect("activate staged profile during startup");
    assert_eq!(read_chat_count(&restored), 1);
    assert!(chat_exists(&restored, "source-chat"));
    assert!(!chat_exists(&restored, "existing-chat"));
    let attachment_path = destination_directory
        .0
        .join("attachments/source-chat/source-message/source-attachment.data");
    assert_eq!(
        fs::read(&attachment_path).expect("read restored attachment"),
        b"attachment payload"
    );
    restored
        .initialize_backend_schema()
        .expect("validate restored backend schema");
    assert_eq!(workspace_file_change_count(&restored), 0);
    assert_eq!(workspace_file_blob_count(&restored), 0);
    assert!(
        !destination_directory
            .0
            .join("attachments/snapshots")
            .exists()
    );
    restored
        .commit_pending_profile_restore()
        .expect("commit verified restore");

    let recovery_root = destination_directory.0.join("backups/profile-restores");
    let recovery = fs::read_dir(recovery_root)
        .expect("list preserved profiles")
        .next()
        .expect("the replaced profile is retained")
        .expect("read recovery entry")
        .path();
    assert!(recovery.join("db/openchat.sqlite3").is_file());
    assert!(
        recovery
            .join("attachments/existing-chat/source-message/source-attachment.data")
            .is_file()
    );
}

#[test]
fn unconfirmed_profile_restore_rolls_back_on_the_next_startup() {
    let _guard = RESTORE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source_directory = TestDirectory::new("rollback-source");
    let source_key = [0x13; 32];
    let source = seed_profile(&source_directory.0, "source-chat", false, &source_key);
    let archive_directory = source_directory.0.join("exports");
    fs::create_dir(&archive_directory).expect("create export directory");
    let archive_path = archive_directory.join("profile.openchatprofilebackup");
    export_inner(&source, &archive_path, PASSPHRASE).expect("export profile");

    let destination_directory = TestDirectory::new("rollback-destination");
    let destination_key = [0x23; 32];
    let destination = seed_profile(
        &destination_directory.0,
        "existing-chat",
        false,
        &destination_key,
    );
    prepare_restore(
        &destination,
        archive_path.to_str().expect("archive path is UTF-8"),
        PASSPHRASE.to_owned().into(),
        CHAT_SCHEMA_VERSION,
    )
    .expect("stage profile restore");
    drop(destination);

    let unconfirmed = AppStorage::open_at_with_key(
        destination_directory.0.clone(),
        crate::storage::DatabaseKey::from_bytes(&destination_key)
            .expect("open destination profile key"),
    )
    .expect("activate staged profile");
    assert!(chat_exists(&unconfirmed, "source-chat"));
    drop(unconfirmed);

    let rolled_back = AppStorage::open_at_with_key(
        destination_directory.0.clone(),
        crate::storage::DatabaseKey::from_bytes(&destination_key)
            .expect("reopen destination profile key"),
    )
    .expect("roll back the unconfirmed restore");
    assert!(chat_exists(&rolled_back, "existing-chat"));
    assert!(!chat_exists(&rolled_back, "source-chat"));
    assert!(
        !destination_directory
            .0
            .join("cache/profile-restore/restore.pending")
            .exists()
    );
}

#[test]
fn version_one_archive_is_converted_to_the_destination_key_before_staging() {
    let _guard = RESTORE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source_directory = TestDirectory::new("legacy-archive-source");
    let source_key = [0x31; 32];
    let source = seed_profile(
        &source_directory.0,
        "legacy-source-chat",
        false,
        &source_key,
    );
    let legacy_database_path = source_directory.0.join("legacy-plaintext.sqlite3");
    let source_database = source
        .connect_owned_database()
        .expect("connect to source database");
    source
        .database_key()
        .export_test_plaintext_database(&source_database, &legacy_database_path)
        .expect("export a plaintext v1 archive database fixture");
    drop(source_database);

    let database_record = file_record(DATABASE_PATH, &legacy_database_path, MAX_DATABASE_BYTES)
        .expect("record legacy database file");
    let database = Connection::open_with_flags(
        &legacy_database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("open legacy database copy");
    let manifest = ProfileManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        version: 1,
        created_at_unix_ms: unix_time_millis().expect("read test clock"),
        chat_schema_version: database
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read chat schema version"),
        backend_schema_version: read_backend_schema_version(&database)
            .expect("read backend schema version"),
        conversation_count: count_rows(&database, "conversations").expect("count conversations"),
        message_count: count_rows(&database, "messages").expect("count messages"),
        database: database_record.clone(),
        database_key: None,
        attachments: Vec::<FileRecord>::new(),
    };
    drop(database);

    let archive_directory = source_directory.0.join("exports");
    fs::create_dir(&archive_directory).expect("create archive directory");
    let archive_path = archive_directory.join("legacy.openchatprofilebackup");
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive_path)
        .expect("create legacy archive file");
    let manifest_json =
        Zeroizing::new(serde_json::to_vec(&manifest).expect("serialize legacy archive manifest"));
    write_archive(
        output,
        &manifest_json,
        &database_record,
        &legacy_database_path,
        &[],
        PASSPHRASE,
    )
    .expect("write legacy archive");

    let destination_directory = TestDirectory::new("legacy-archive-destination");
    let destination_key = [0x41; 32];
    let destination = seed_profile(
        &destination_directory.0,
        "existing-chat",
        false,
        &destination_key,
    );
    prepare_restore(
        &destination,
        archive_path.to_str().expect("archive path is UTF-8"),
        PASSPHRASE.to_owned().into(),
        CHAT_SCHEMA_VERSION,
    )
    .expect("stage legacy archive");
    let marker = super::read_marker(
        &destination_directory
            .0
            .join("cache/profile-restore/restore.pending"),
    )
    .expect("read restore marker");
    let stage = super::restore_stage_path(
        &destination_directory.0.join("cache/profile-restore"),
        &marker.stage_id,
    );
    let staged_database = fs::read(stage.join(DATABASE_PATH)).expect("read staged database");
    assert_ne!(&staged_database[..16], b"SQLite format 3\0");
    let mut staged_manifest: ProfileManifest =
        read_json(&stage.join(super::MANIFEST_PATH), super::MAX_MANIFEST_BYTES)
            .expect("read staged manifest");
    assert!(staged_manifest.database_key.is_none());
    let staged_database_path = stage.join(DATABASE_PATH);
    fs::remove_file(&staged_database_path).expect("remove encrypted stage fixture");
    fs::copy(&legacy_database_path, &staged_database_path)
        .expect("restore old plaintext stage fixture");
    staged_manifest.database = database_record.clone();
    fs::write(
        stage.join(super::MANIFEST_PATH),
        serde_json::to_vec(&staged_manifest).expect("serialize old stage manifest"),
    )
    .expect("write old stage manifest");
    drop(destination);

    let restored = crate::storage::AppStorage::open_at_with_key(
        destination_directory.0.clone(),
        crate::storage::DatabaseKey::from_bytes(&destination_key)
            .expect("reopen destination database key"),
    )
    .expect("activate legacy archive restore");
    assert!(chat_exists(&restored, "legacy-source-chat"));
}

#[test]
fn profile_export_does_not_overwrite_an_existing_archive() {
    let source_directory = TestDirectory::new("no-overwrite");
    let source_key = [0x14; 32];
    let source = seed_profile(&source_directory.0, "source-chat", false, &source_key);
    let archive_directory = source_directory.0.join("exports");
    fs::create_dir(&archive_directory).expect("create export directory");
    let archive_path = archive_directory.join("profile.openchatprofilebackup");
    fs::write(&archive_path, b"preserve existing data").expect("create existing file");

    let error = export_inner(&source, &archive_path, PASSPHRASE)
        .expect_err("reject an existing archive path");

    assert!(matches!(error, ArchiveFailure::Conflict));
    assert_eq!(
        fs::read(&archive_path).expect("read existing archive"),
        b"preserve existing data"
    );
    assert_eq!(
        fs::read_dir(&archive_directory)
            .expect("list export directory")
            .count(),
        1,
        "failed exports should not leave partial archives"
    );
}

#[test]
fn startup_removes_orphaned_restore_staging_directories() {
    let directory = TestDirectory::new("orphan-stage");
    let orphan = directory
        .0
        .join("cache/profile-restore/stage-abandoned-stage");
    fs::create_dir_all(&orphan).expect("create orphaned stage");
    fs::write(orphan.join("partial.bin"), b"incomplete").expect("write partial stage");

    let storage = AppStorage::open_at(directory.0.clone()).expect("open local storage");

    assert!(
        !orphan.exists(),
        "orphaned restore data should be reclaimed"
    );
    assert!(storage.database_path().is_file());
}

#[test]
fn restore_state_json_rejects_files_above_its_byte_limit() {
    let directory = TestDirectory::new("restore-state-limit");
    let path = directory.0.join("restore-state.json");
    let valid_json = br#"{"ready":true}"#;
    let mut exact_limit = valid_json.to_vec();
    exact_limit.resize(MAX_RESTORE_STATE_BYTES as usize, b' ');
    fs::write(&path, &exact_limit).expect("write restore state at its byte limit");

    assert_eq!(
        read_json::<serde_json::Value>(&path, MAX_RESTORE_STATE_BYTES)
            .expect("read state at the exact byte limit")["ready"],
        true
    );

    let mut oversized = exact_limit;
    oversized.push(b' ');
    fs::write(&path, oversized).expect("write state one byte over the limit");
    assert!(
        read_json::<serde_json::Value>(&path, MAX_RESTORE_STATE_BYTES).is_err(),
        "oversized restore metadata must be rejected before JSON parsing"
    );
}

fn seed_profile(
    root: &std::path::Path,
    conversation_id: &str,
    with_attachment: bool,
    database_key: &[u8; 32],
) -> AppStorage {
    let database_directory = root.join("db");
    fs::create_dir_all(&database_directory).expect("create database directory");
    let database_path = database_directory.join("openchat.sqlite3");
    let connection = Connection::open(&database_path).expect("create database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
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
                connection_id TEXT,
                workspace_id TEXT,
                api_key_connection_id TEXT,
                provider_id TEXT,
                model_id TEXT,
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                is_archived INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                output_tokens INTEGER,
                tokens_per_second REAL,
                elapsed_microseconds INTEGER,
                reasoning_summaries TEXT NOT NULL DEFAULT '[]',
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                failure_code TEXT,
                PRIMARY KEY (conversation_id, id)
             );
             PRAGMA user_version = 11;",
        )
        .expect("create chat schema");
    connection
        .execute(
            "INSERT INTO conversations (id, title, created_at, updated_at)
             VALUES (?1, ?1, 1, 1)",
            [conversation_id],
        )
        .expect("insert conversation");
    let attachment_bytes = b"attachment payload";
    let content = if with_attachment {
        let attachment = json!({
            "id": "source-attachment",
            "name": "notes.txt",
            "mimeType": "text/plain",
            "sizeBytes": attachment_bytes.len(),
            "kind": "text"
        });
        let envelope = json!({ "content": "attached", "attachments": [attachment] });
        let content = format!("\u{1e}openchat-attachments-v1:{envelope}");
        let attachment_directory = root
            .join("attachments")
            .join(conversation_id)
            .join("source-message");
        fs::create_dir_all(&attachment_directory).expect("create attachment directory");
        fs::write(
            attachment_directory.join("source-attachment.data"),
            attachment_bytes,
        )
        .expect("write attachment fixture");
        content
    } else {
        "plain message".to_owned()
    };
    connection
        .execute(
            "INSERT INTO messages (conversation_id, id, role, content, status, created_at)
             VALUES (?1, ?2, 'user', ?3, 'completed', 1)",
            params![conversation_id, "source-message", content],
        )
        .expect("insert message");
    drop(connection);

    let storage = AppStorage::open_at_with_key(
        root.to_path_buf(),
        crate::storage::DatabaseKey::from_bytes(database_key).expect("create test database key"),
    )
    .expect("open test storage");
    storage
        .initialize_backend_schema()
        .expect("initialize current backend schema");
    storage
}

fn read_chat_count(storage: &AppStorage) -> i64 {
    storage
        .connect()
        .expect("connect to chat database")
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
        .expect("count conversations")
}

fn chat_exists(storage: &AppStorage, conversation_id: &str) -> bool {
    storage
        .connect()
        .expect("connect to chat database")
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("look up conversation")
}

fn add_workspace_file_change(storage: &AppStorage) -> PathBuf {
    let original = b"workspace source snapshot";
    let hash = format!("{:x}", Sha256::digest(original));
    let snapshot_path = storage
        .root()
        .join("attachments/snapshots")
        .join(&hash[..2])
        .join(format!("{hash}.blob"));
    fs::create_dir_all(snapshot_path.parent().expect("snapshot parent"))
        .expect("create snapshot storage");
    fs::write(&snapshot_path, original).expect("write local workspace snapshot");
    let connection = storage.connect().expect("connect to source profile");
    connection
        .execute(
            "INSERT INTO file_change_blobs (
                hash, size_bytes, state, created_at_unix_ms
             ) VALUES (?1, ?2, 'ready', 1)",
            params![hash, original.len() as i64],
        )
        .expect("insert local snapshot metadata");
    connection
        .execute(
            "INSERT INTO conversation_file_changes (
                conversation_id, id, root, path, baseline_exists, baseline_hash,
                baseline_blob_hash, expected_exists, expected_hash, added_lines,
                removed_lines, is_binary, diff_available, status
             ) VALUES (
                'source-chat', ?1, 'C:/workspace', 'private.rs', 1, ?2,
                ?2, 0, NULL, NULL, NULL, 0, 0, 'active'
             )",
            params!["a".repeat(64), hash],
        )
        .expect("insert local workspace change");
    connection
        .execute(
            "INSERT INTO file_change_legacy_imports (conversation_id, imported_at_unix_ms)
             VALUES ('source-chat', 1)",
            [],
        )
        .expect("insert migration marker");
    snapshot_path
}

fn workspace_file_change_count(storage: &AppStorage) -> i64 {
    storage
        .connect()
        .expect("connect to profile database")
        .query_row(
            "SELECT COUNT(*) FROM conversation_file_changes",
            [],
            |row| row.get(0),
        )
        .expect("count workspace file changes")
}

fn workspace_file_blob_count(storage: &AppStorage) -> i64 {
    storage
        .connect()
        .expect("connect to profile database")
        .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
            row.get(0)
        })
        .expect("count workspace snapshots")
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "openchat-profile-archive-{label}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}
