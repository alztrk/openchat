//! Creates, validates, and transactionally restores encrypted whole-profile backups.
//!
//! Database and attachment entries are bounded and hashed; restore state is journaled so an
//! interrupted activation can be validated, completed, or recovered without silently accepting
//! an incomplete profile.

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use std::{
    iter,
    sync::{Mutex, OnceLock},
};

use age::{Decryptor, scrypt::Identity};
use age::{Encryptor, secrecy::SecretString};
use rusqlite::Connection;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::Archive;
use tar::{Builder, EntryType, Header};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    attachment_crypto::{AttachmentCipher, attachment_associated_data},
    attachments::{
        AttachmentMetadata, message_attachment_metadata, validate_portable_attachment_content,
    },
    protocol::ServiceError,
    storage::AppStorage,
};

#[path = "profile_archive/restore.rs"]
mod restore;
#[path = "profile_archive/writing.rs"]
mod writing;

use restore::*;
pub(crate) use restore::{
    activate_pending_restore, commit_pending_restore, recover_unconfirmed_restore,
};
use writing::*;

const ARCHIVE_FORMAT: &str = "openchat-profile";
const ARCHIVE_VERSION: u32 = 2;
const RESTORE_STATE_VERSION: u32 = 1;
const ARCHIVE_EXTENSION: &str = "openchatprofilebackup";
const MANIFEST_PATH: &str = "manifest.json";
const DATABASE_PATH: &str = "database/openchat.sqlite3";
const MIN_PASSPHRASE_CHARACTERS: usize = 12;
const MAX_PASSPHRASE_BYTES: usize = 512;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_DATABASE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: u64 = 24 * 1024 * 1024 * 1024;
const MAX_ATTACHMENTS: usize = 100_000;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024 * 1024;
const MAX_RESTORE_STATE_BYTES: u64 = 4 * 1024;
#[cfg(not(any(test, feature = "desktop-profile-test-support")))]
const DEFAULT_AGE_SCRYPT_LOG_N: u8 = 18;
#[cfg(any(test, feature = "desktop-profile-test-support"))]
const DEFAULT_AGE_SCRYPT_LOG_N: u8 = 10;
const MAX_AGE_SCRYPT_LOG_N: u8 = 18;
const PROFILE_ARCHIVE_ROOT: &str = "profile-archives";
const RESTORE_ROOT: &str = "profile-restore";
const RESTORE_PENDING_FILE: &str = "restore.pending";
const RESTORE_APPLYING_FILE: &str = "restore.applying.json";
const RESTORE_AWAITING_FILE: &str = "restore.awaiting.json";
const RESTORE_COMMITTED_FILE: &str = "restore.committed.json";
const RECOVERY_DIRECTORY: &str = "profile-restores";
static RESTORE_OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileManifest {
    format: String,
    version: u32,
    created_at_unix_ms: i64,
    chat_schema_version: i64,
    backend_schema_version: i64,
    conversation_count: u64,
    message_count: u64,
    database: FileRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    database_key: Option<Vec<u8>>,
    attachments: Vec<FileRecord>,
}

impl Drop for ProfileManifest {
    fn drop(&mut self) {
        if let Some(key) = &mut self.database_key {
            key.zeroize();
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileRecord {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreMarker {
    version: u32,
    stage_id: String,
    chat_schema_version: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreJournal {
    version: u32,
    stage_id: String,
    recovery_id: String,
    phase: RestorePhase,
    had_database_directory: bool,
    had_attachments_directory: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RestorePhase {
    Applying,
    AwaitingValidation,
    Committed,
}

#[derive(Debug, Clone, Copy)]
enum ArchiveFailure {
    InvalidRequest,
    InvalidArchive,
    InvalidPassphrase,
    NotFound,
    Conflict,
    Busy,
    Storage,
    LimitExceeded,
    UnsupportedSchema,
}

impl From<rusqlite::Error> for ArchiveFailure {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}

impl From<io::Error> for ArchiveFailure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::NotFound
        } else if error.kind() == io::ErrorKind::AlreadyExists {
            Self::Conflict
        } else {
            Self::Storage
        }
    }
}

struct AttachmentSource {
    record: FileRecord,
    path: PathBuf,
    associated_data: Vec<u8>,
}

struct TemporaryDirectory {
    path: PathBuf,
    active: bool,
}

impl TemporaryDirectory {
    fn create(parent: &Path, label: &str) -> Result<Self, ArchiveFailure> {
        fs::create_dir_all(parent).map_err(|_| ArchiveFailure::Storage)?;
        let path = parent.join(format!("{label}-{}", Uuid::new_v4().simple()));
        fs::create_dir(&path).map_err(|_| ArchiveFailure::Storage)?;
        Ok(Self { path, active: true })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn preserve(&mut self) {
        self.active = false;
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct PartialOutput {
    path: PathBuf,
    active: bool,
}

impl PartialOutput {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for PartialOutput {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn service_error(failure: ArchiveFailure) -> ServiceError {
    match failure {
        ArchiveFailure::InvalidRequest => ServiceError::new(
            "invalid_request_params",
            "The profile backup request is invalid.",
            false,
        ),
        ArchiveFailure::InvalidArchive => ServiceError::new(
            "profile_archive_invalid",
            "The encrypted profile backup is invalid or its passphrase is incorrect.",
            false,
        ),
        ArchiveFailure::InvalidPassphrase => ServiceError::new(
            "profile_archive_passphrase_invalid",
            "Use a passphrase with at least 12 characters and no more than 512 bytes.",
            false,
        ),
        ArchiveFailure::NotFound => ServiceError::new(
            "profile_archive_not_found",
            "The selected profile backup could not be found.",
            false,
        ),
        ArchiveFailure::Conflict => ServiceError::new(
            "profile_archive_conflict",
            "A profile backup is already waiting to be restored, or the output file exists.",
            false,
        ),
        ArchiveFailure::Busy => ServiceError::new(
            "profile_archive_busy",
            "Another profile backup operation is in progress.",
            true,
        ),
        ArchiveFailure::Storage => ServiceError::new(
            "profile_archive_storage_failed",
            "The profile backup could not be read, written, or safely restored.",
            true,
        ),
        ArchiveFailure::LimitExceeded => ServiceError::new(
            "profile_archive_limit_exceeded",
            "The profile backup exceeds a supported size or item limit.",
            false,
        ),
        ArchiveFailure::UnsupportedSchema => ServiceError::new(
            "profile_archive_schema_unsupported",
            "This profile backup was created by a newer OpenChat database schema.",
            false,
        ),
    }
}

pub(crate) fn export(
    storage: &AppStorage,
    passphrase: Zeroizing<String>,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    let pending = crate::pending_archive_export::PendingArchiveExport::begin(
        storage.root(),
        ARCHIVE_EXTENSION,
    )?;
    let result = export_inner(storage, pending.archive_path(), passphrase.as_str())
        .map_err(service_error)?;
    pending.finish(result)
}

pub(crate) fn prepare_restore(
    storage: &AppStorage,
    path: &str,
    passphrase: Zeroizing<String>,
    current_chat_schema_version: i64,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    if !(1..=100).contains(&current_chat_schema_version) {
        return Err(service_error(ArchiveFailure::InvalidRequest));
    }
    prepare_restore_inner(
        storage,
        storage.root(),
        Path::new(path),
        passphrase.as_str(),
        current_chat_schema_version,
    )
    .map_err(service_error)
}

#[cfg(feature = "storage-broker-test-support")]
pub(crate) fn create_broker_test_archive(
    source_root: &Path,
    archive_path: &Path,
    conversation_id: &str,
    passphrase: Zeroizing<String>,
) -> io::Result<()> {
    if conversation_id.is_empty()
        || conversation_id.len() > 128
        || !conversation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The storage broker test conversation identifier is invalid.",
        ));
    }
    validate_passphrase(&passphrase).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The test passphrase is invalid.",
        )
    })?;

    let database_directory = source_root.join("db");
    fs::create_dir_all(&database_directory)?;
    let database_path = database_directory.join("openchat.sqlite3");
    drop(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&database_path)?,
    );

    let database = Connection::open(&database_path).map_err(io::Error::other)?;
    database
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
        .map_err(io::Error::other)?;
    database
        .execute(
            "INSERT INTO conversations (id, title, created_at, updated_at)
             VALUES (?1, 'Restored broker profile', 1, 1)",
            [conversation_id],
        )
        .map_err(io::Error::other)?;

    let attachment_bytes = b"broker archive attachment";
    let attachment = json!({
        "id": "broker-archive-attachment",
        "name": "archive.txt",
        "mimeType": "text/plain",
        "sizeBytes": attachment_bytes.len(),
        "kind": "text"
    });
    let envelope = json!({
        "content": "profile archive integration fixture",
        "attachments": [attachment]
    });
    let message_content = format!("\u{1e}openchat-attachments-v1:{envelope}");
    database
        .execute(
            "INSERT INTO messages (conversation_id, id, role, content, status, created_at)
             VALUES (?1, 'broker-archive-message', 'user', ?2, 'completed', 1)",
            rusqlite::params![conversation_id, message_content],
        )
        .map_err(io::Error::other)?;
    drop(database);

    let storage_key = crate::storage::DatabaseKey::generate().map_err(io::Error::other)?;
    let storage = AppStorage::open_at_with_key(source_root.to_path_buf(), storage_key)
        .map_err(io::Error::other)?;
    storage
        .initialize_backend_schema()
        .map_err(io::Error::other)?;
    let attachment_directory = source_root
        .join("attachments")
        .join(conversation_id)
        .join("broker-archive-message");
    fs::create_dir_all(&attachment_directory)?;
    let attachment_cipher = AttachmentCipher::from_profile_key(storage.database_key())?;
    let attachment_context = attachment_associated_data(Path::new(&format!(
        "{conversation_id}/broker-archive-message/broker-archive-attachment.data"
    )))?;
    attachment_cipher.write_new_file(
        &attachment_directory.join("broker-archive-attachment.data"),
        &attachment_context,
        attachment_bytes,
    )?;

    export_inner(&storage, archive_path, &passphrase)
        .map(|_| ())
        .map_err(|_| io::Error::other("The encrypted test profile archive could not be created."))
}

#[cfg(feature = "storage-broker-test-support")]
pub(crate) fn stage_broker_test_restore(
    destination_root: &Path,
    archive_path: &Path,
    database_key: &[u8],
    passphrase: Zeroizing<String>,
) -> io::Result<()> {
    let archive_path = archive_path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The test profile archive path is not valid UTF-8.",
        )
    })?;
    let database_key =
        crate::storage::DatabaseKey::from_bytes(database_key).map_err(io::Error::other)?;
    let storage = AppStorage::open_at_with_key(destination_root.to_path_buf(), database_key)
        .map_err(io::Error::other)?;
    prepare_restore(
        &storage,
        archive_path,
        passphrase,
        crate::CHAT_SCHEMA_VERSION,
    )
    .map(|_| ())
    .map_err(|error| {
        io::Error::other(format!(
            "The encrypted test profile archive could not be staged ({}).",
            error.code
        ))
    })
}

fn export_inner(
    storage: &AppStorage,
    target: &Path,
    passphrase: &str,
) -> Result<Value, ArchiveFailure> {
    let target = validate_output_path(target)?;
    ensure_destination_is_new(&target)?;
    let source = storage.connect().map_err(|_| ArchiveFailure::Storage)?;
    crate::storage::verify_storage_integrity(&source).map_err(|_| ArchiveFailure::Storage)?;
    let archive_temp = TemporaryDirectory::create(
        &storage.root().join("cache").join(PROFILE_ARCHIVE_ROOT),
        "export",
    )?;
    let local_database_snapshot_path = archive_temp.path().join("openchat.local.sqlite3");
    crate::storage::create_verified_storage_snapshot(&source, &local_database_snapshot_path)
        .map_err(|_| ArchiveFailure::Storage)?;
    drop(source);
    strip_workspace_file_changes(&local_database_snapshot_path, storage.database_key())?;

    let archive_database_key =
        crate::storage::DatabaseKey::generate().map_err(|_| ArchiveFailure::Storage)?;
    let database_snapshot_path = archive_temp.path().join("openchat.sqlite3");
    storage
        .database_key()
        .export_encrypted_database(
            &local_database_snapshot_path,
            &database_snapshot_path,
            &archive_database_key,
        )
        .map_err(|_| ArchiveFailure::Storage)?;

    let database = archive_database_key
        .open(
            &database_snapshot_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|_| ArchiveFailure::Storage)?;
    crate::storage::disable_trusted_schema(&database).map_err(|_| ArchiveFailure::Storage)?;
    storage::verify_database_integrity(&database).map_err(|_| ArchiveFailure::Storage)?;
    verify_foreign_keys(&database)?;
    let chat_schema_version = database
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|_| ArchiveFailure::Storage)?;
    let backend_schema_version = read_backend_schema_version(&database)?;
    let conversation_count = count_rows(&database, "conversations")?;
    let message_count = count_rows(&database, "messages")?;
    let database_record = file_record(DATABASE_PATH, &database_snapshot_path, MAX_DATABASE_BYTES)?;
    let attachments = collect_attachments(&database, storage.root(), storage.database_key())?;
    drop(database);

    let attachment_bytes = attachments
        .iter()
        .try_fold(0_u64, |total, attachment| {
            total.checked_add(attachment.record.size_bytes)
        })
        .ok_or(ArchiveFailure::LimitExceeded)?;
    if attachments.len() > MAX_ATTACHMENTS || attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let manifest = ProfileManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        version: ARCHIVE_VERSION,
        created_at_unix_ms: unix_time_millis().map_err(|_| ArchiveFailure::Storage)?,
        chat_schema_version,
        backend_schema_version,
        conversation_count,
        message_count,
        database: database_record.clone(),
        database_key: Some(archive_database_key.as_bytes().to_vec()),
        attachments: attachments
            .iter()
            .map(|attachment| attachment.record.clone())
            .collect(),
    };
    validate_manifest(&manifest)?;
    let manifest_json =
        Zeroizing::new(serde_json::to_vec(&manifest).map_err(|_| ArchiveFailure::Storage)?);
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }

    let partial_path = target.with_file_name(format!(
        ".{}.{}.partial",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ArchiveFailure::InvalidRequest)?,
        Uuid::new_v4().simple()
    ));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial_path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ArchiveFailure::Conflict
            } else {
                ArchiveFailure::Storage
            }
        })?;
    let mut partial = PartialOutput::new(partial_path);
    write_archive(
        output,
        &manifest_json,
        &database_record,
        &database_snapshot_path,
        &attachments,
        storage.database_key(),
        passphrase,
    )?;
    ensure_destination_is_new(&target)?;
    fs::rename(partial.path.as_path(), &target).map_err(map_destination_error)?;
    partial.disarm();
    Ok(archive_summary(&manifest, false))
}

fn strip_workspace_file_changes(
    database_path: &Path,
    database_key: &crate::storage::DatabaseKey,
) -> Result<(), ArchiveFailure> {
    let database = database_key
        .open(database_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| ArchiveFailure::Storage)?;
    crate::storage::disable_trusted_schema(&database).map_err(|_| ArchiveFailure::Storage)?;
    crate::storage::tool_index_redaction::register_sqlite_function(&database)
        .map_err(|_| ArchiveFailure::Storage)?;
    let mut present = 0;
    for table in [
        "conversation_file_changes",
        "conversation_file_checkpoints",
        "conversation_file_checkpoint_files",
        "file_change_blobs",
        "file_change_legacy_imports",
    ] {
        present += database
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                )",
                [table],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| ArchiveFailure::Storage)? as usize;
    }
    if present == 0 {
        return Ok(());
    }
    if present != 5 {
        return Err(ArchiveFailure::Storage);
    }
    database
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             BEGIN IMMEDIATE;
             DELETE FROM conversation_file_changes;
             DELETE FROM conversation_file_checkpoints;
             DELETE FROM file_change_legacy_imports;
             DELETE FROM file_change_blobs;
             COMMIT;",
        )
        .map_err(|_| ArchiveFailure::Storage)
}

fn prepare_restore_inner(
    storage: &AppStorage,
    restore_base: &Path,
    archive_path: &Path,
    passphrase: &str,
    current_chat_schema_version: i64,
) -> Result<Value, ArchiveFailure> {
    let restore_root = restore_base.join("cache").join(RESTORE_ROOT);
    fs::create_dir_all(&restore_root).map_err(|_| ArchiveFailure::Storage)?;
    let _lock = RESTORE_OPERATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .try_lock()
        .map_err(|_| ArchiveFailure::Busy)?;
    if restore_pending_path(&restore_root).exists()
        || restore_journal_paths(&restore_root)
            .iter()
            .any(|path| path.exists())
    {
        return Err(ArchiveFailure::Conflict);
    }
    let mut stage = TemporaryDirectory::create(&restore_root, "stage")?;
    let stage_id = stage
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("stage-"))
        .filter(|id| safe_identifier(id))
        .ok_or(ArchiveFailure::Storage)?
        .to_owned();
    let manifest = read_archive_to_stage(
        archive_path,
        passphrase,
        stage.path(),
        current_chat_schema_version,
        storage.database_key(),
    )?;
    let manifest_path = stage.path().join(MANIFEST_PATH);
    let manifest_bytes =
        Zeroizing::new(serde_json::to_vec(&manifest).map_err(|_| ArchiveFailure::Storage)?);
    write_new_bytes(&manifest_path, &manifest_bytes)?;
    let marker = RestoreMarker {
        version: RESTORE_STATE_VERSION,
        stage_id,
        chat_schema_version: current_chat_schema_version,
    };
    write_new_json(&restore_pending_path(&restore_root), &marker)?;
    stage.preserve();
    Ok(archive_summary(&manifest, true))
}

fn collect_attachments(
    database: &Connection,
    storage_root: &Path,
    profile_key: &crate::storage::DatabaseKey,
) -> Result<Vec<AttachmentSource>, ArchiveFailure> {
    let attachments_root = storage_root.join("attachments");
    let canonical_root = match fs::canonicalize(&attachments_root) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ArchiveFailure::Storage),
    };
    let cipher =
        AttachmentCipher::from_profile_key(profile_key).map_err(|_| ArchiveFailure::Storage)?;
    let mut statement = database
        .prepare("SELECT conversation_id, id, content FROM messages ORDER BY rowid")
        .map_err(|_| ArchiveFailure::Storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| ArchiveFailure::Storage)?;
    let mut paths = BTreeMap::new();
    for row in rows {
        let (conversation_id, message_id, content) = row.map_err(|_| ArchiveFailure::Storage)?;
        let metadata =
            message_attachment_metadata(&content).map_err(|_| ArchiveFailure::Storage)?;
        for attachment in metadata {
            let archive_path = format!(
                "attachments/{conversation_id}/{message_id}/{}.data",
                attachment.id
            );
            if paths.contains_key(&archive_path) {
                return Err(ArchiveFailure::InvalidArchive);
            }
            let source = attachments_root
                .join(&conversation_id)
                .join(&message_id)
                .join(format!("{}.data", attachment.id));
            let file_metadata =
                fs::symlink_metadata(&source).map_err(|_| ArchiveFailure::Storage)?;
            if !file_metadata.is_file() || file_metadata.file_type().is_symlink() {
                return Err(ArchiveFailure::Storage);
            }
            let canonical = fs::canonicalize(&source).map_err(|_| ArchiveFailure::Storage)?;
            if !canonical.starts_with(&canonical_root) {
                return Err(ArchiveFailure::Storage);
            }
            let associated_data = attachment_associated_data(Path::new(&format!(
                "{conversation_id}/{message_id}/{}.data",
                attachment.id
            )))
            .map_err(|_| ArchiveFailure::Storage)?;
            let bytes = cipher
                .read_file(&canonical, &associated_data, attachment.size_bytes)
                .map_err(|_| ArchiveFailure::Storage)?;
            validate_portable_attachment_content(&attachment, &bytes)
                .map_err(|_| ArchiveFailure::Storage)?;
            let record = FileRecord {
                path: archive_path.clone(),
                size_bytes: bytes.len() as u64,
                sha256: digest_hex(&Sha256::digest(&bytes)),
            };
            paths.insert(
                archive_path,
                AttachmentSource {
                    record,
                    path: canonical,
                    associated_data,
                },
            );
            if paths.len() > MAX_ATTACHMENTS {
                return Err(ArchiveFailure::LimitExceeded);
            }
        }
    }
    Ok(paths.into_values().collect())
}

mod storage {
    pub(super) use crate::storage::{
        backend_schema_version, verify_database_integrity, verify_plaintext_database_integrity,
    };
}

#[cfg(test)]
#[path = "profile_archive/tests.rs"]
mod tests;
