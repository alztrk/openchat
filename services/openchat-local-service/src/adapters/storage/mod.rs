//! Owns the local SQLite database, schema lifecycle, diagnostics, and persistence operations.
//!
//! Startup checks SQLite compatibility and database integrity before migrations; schema changes
//! use verified backups, and diagnostic records are size-bounded and credential-redacted.

use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rusqlite::{
    Connection, OpenFlags, OptionalExtension,
    backup::{Backup, StepResult},
    ffi,
};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

#[cfg(windows)]
use broker_connection::BrokerConnection;
#[cfg(windows)]
use ipc_protocol::LegacyImportStatus;
#[cfg(windows)]
use remote_connection::RemoteStorageError;

const MAX_DIAGNOSTIC_LOG_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_ENTRY_BYTES: usize = 4096;
const MINIMUM_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
pub(crate) const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(windows)]
pub(crate) const STORAGE_DATABASE_LOCK_TIMEOUT: Duration = Duration::from_secs(7);
#[cfg(windows)]
pub(crate) const STORAGE_BROKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(16);
#[cfg(windows)]
pub(crate) const STORAGE_BROKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(windows)]
const LEGACY_IMPORT_JOURNAL_CONTENT: &[u8] = b"OpenChat legacy storage import v1\n";
#[cfg(windows)]
const LEGACY_IMPORT_JOURNAL_NAME: &str = "storage-import.in-progress";
#[cfg(windows)]
const LEGACY_IMPORT_ROLLBACK_NAME: &str = "storage-import-rollback.sqlite3";

#[cfg(windows)]
mod broker_connection;
mod connection;
mod database_guard;
mod database_key;
mod database_queue;
pub(crate) mod ipc_protocol;
#[cfg(windows)]
mod remote_connection;
mod schema;
pub(crate) mod tool_index_redaction;
pub(crate) mod user_questions;
pub(crate) use connection::{StorageConnection, StorageRow, StorageTransaction};
pub(crate) use database_key::DatabaseKey;
pub(crate) use database_queue::{DatabaseQueueHandle, QueuedConnection};
use schema::{INITIAL_SCHEMA_VERSION, SCHEMA_VERSION, initialize_schema};

pub(crate) fn sqlite_error_code(error: &rusqlite::Error) -> &'static str {
    match error {
        rusqlite::Error::SqliteFailure(_, message)
            if message
                .as_deref()
                .is_some_and(|message| message == "database operation queue is full") =>
        {
            "database_queue_full"
        }
        rusqlite::Error::SqliteFailure(_, message)
            if message
                .as_deref()
                .is_some_and(|message| message == "database operation queue wait timed out") =>
        {
            "database_queue_wait_timeout"
        }
        rusqlite::Error::SqliteFailure(_, message)
            if message.as_deref().is_some_and(|message| {
                message == "The storage broker did not respond in time."
            }) =>
        {
            "storage_broker_timeout"
        }
        rusqlite::Error::SqliteFailure(_, message)
            if message
                .as_deref()
                .is_some_and(|message| message == "chat message content limit exceeded") =>
        {
            "message_content_too_large"
        }
        rusqlite::Error::SqliteFailure(_, message)
            if message
                .as_deref()
                .is_some_and(|message| message == "chat message metadata limit exceeded") =>
        {
            "message_metadata_too_large"
        }
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_BUSY_SNAPSHOT =>
        {
            "sqlite_busy_snapshot"
        }
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_BUSY_TIMEOUT =>
        {
            "sqlite_busy_timeout"
        }
        rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
            rusqlite::ErrorCode::DatabaseBusy => "sqlite_busy",
            rusqlite::ErrorCode::DatabaseLocked => "sqlite_locked",
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                "sqlite_corrupt"
            }
            rusqlite::ErrorCode::CannotOpen => "sqlite_cannot_open",
            rusqlite::ErrorCode::PermissionDenied => "sqlite_permission_denied",
            rusqlite::ErrorCode::SystemIoFailure => "sqlite_io_error",
            rusqlite::ErrorCode::DiskFull => "sqlite_disk_full",
            rusqlite::ErrorCode::SchemaChanged => "sqlite_schema_changed",
            _ => "sqlite_failure",
        },
        rusqlite::Error::InvalidQuery => "invalid_query",
        rusqlite::Error::InvalidColumnType(_, _, _) => "invalid_column_type",
        rusqlite::Error::InvalidColumnIndex(_) => "invalid_column_index",
        rusqlite::Error::InvalidColumnName(_) => "invalid_column_name",
        rusqlite::Error::QueryReturnedNoRows => "query_returned_no_rows",
        rusqlite::Error::QueryReturnedMoreThanOneRow => "query_returned_multiple_rows",
        rusqlite::Error::InvalidParameterName(_) => "invalid_parameter_name",
        rusqlite::Error::InvalidParameterCount(_, _) => "invalid_parameter_count",
        rusqlite::Error::SqlInputError { .. } => "sql_input_error",
        rusqlite::Error::FromSqlConversionFailure(_, _, _) => "stored_value_invalid",
        rusqlite::Error::ToSqlConversionFailure(source) => {
            #[cfg(windows)]
            if let Some(error) = source.downcast_ref::<RemoteStorageError>() {
                return error.diagnostic_code();
            }
            #[cfg(not(windows))]
            let _ = source;
            "storage_conversion_failed"
        }
        _ => "storage_operation_failed",
    }
}

pub(crate) fn is_retryable_storage_error_code(code: &str) -> bool {
    matches!(
        code,
        "sqlite_busy"
            | "sqlite_busy_snapshot"
            | "sqlite_busy_timeout"
            | "sqlite_locked"
            | "sqlite_io_error"
            | "sqlite_5"
            | "sqlite_517"
            | "database_queue_full"
            | "database_queue_wait_timeout"
            | "storage_broker_timeout"
            | "storage_broker_transport_unavailable"
            | "storage_broker_unexpected_eof"
    )
}

/// Carries the bounded usage fields permitted in a provider diagnostic entry.
pub(crate) struct ProviderUsageDiagnostic<'a> {
    pub(crate) provider_id: &'a str,
    pub(crate) model_id: &'a str,
    pub(crate) operation: &'a str,
    pub(crate) prompt_tokens: Option<i64>,
    pub(crate) completion_tokens: Option<i64>,
    pub(crate) cached_tokens: Option<i64>,
    pub(crate) cache_write_tokens: Option<i64>,
    pub(crate) cache_discount: Option<f64>,
    pub(crate) estimated_prompt_tokens: i64,
    pub(crate) image_inputs: i64,
}

/// Contains only bounded metadata for one ChatGPT response stream diagnostic.
pub(crate) struct ChatGptStreamDiagnostic<'a> {
    pub(crate) event: &'static str,
    pub(crate) stream_id: &'a str,
    pub(crate) attempt: Option<usize>,
    pub(crate) retry_count: Option<usize>,
    pub(crate) retry_delay_ms: Option<u128>,
    pub(crate) continuation_count: Option<usize>,
    pub(crate) status: Option<u16>,
    pub(crate) http_version: Option<&'static str>,
    pub(crate) content_encoding: Option<&'static str>,
    pub(crate) transfer_encoding: Option<&'static str>,
    pub(crate) content_length_header_bytes: Option<u64>,
    pub(crate) elapsed_ms: Option<u128>,
    pub(crate) body_chunks_received: Option<usize>,
    pub(crate) decoded_body_bytes_received: Option<usize>,
    pub(crate) first_chunk_ms: Option<u128>,
    pub(crate) last_chunk_idle_ms: Option<u128>,
    pub(crate) error_code: Option<&'static str>,
    pub(crate) reqwest_error_class: Option<&'static str>,
    pub(crate) source_error_class: Option<&'static str>,
    pub(crate) source_io_kind: Option<&'static str>,
    pub(crate) retryable: Option<bool>,
    pub(crate) completion_event_seen: Option<bool>,
    pub(crate) partial_text_available: Option<bool>,
}

impl<'a> ChatGptStreamDiagnostic<'a> {
    pub(crate) fn new(event: &'static str, stream_id: &'a str) -> Self {
        Self {
            event,
            stream_id,
            attempt: None,
            retry_count: None,
            retry_delay_ms: None,
            continuation_count: None,
            status: None,
            http_version: None,
            content_encoding: None,
            transfer_encoding: None,
            content_length_header_bytes: None,
            elapsed_ms: None,
            body_chunks_received: None,
            decoded_body_bytes_received: None,
            first_chunk_ms: None,
            last_chunk_idle_ms: None,
            error_code: None,
            reqwest_error_class: None,
            source_error_class: None,
            source_io_kind: None,
            retryable: None,
            completion_event_seen: None,
            partial_text_available: None,
        }
    }
}

/// Contains aggregate Copilot model-catalog filter counts without provider payload data.
pub(crate) struct CopilotCatalogFilterSummary {
    pub(crate) total: usize,
    pub(crate) policy_blocked: usize,
    pub(crate) non_chat: usize,
    pub(crate) picker_disabled: usize,
    pub(crate) invalid_id: usize,
    pub(crate) auto_candidates: usize,
}

/// Owns a user's local OpenChat database and its verified migration/restore state.
pub struct AppStorage {
    root: PathBuf,
    image_files_root: PathBuf,
    database_path: PathBuf,
    database_key: Arc<database_key::DatabaseKey>,
    database_had_data_on_open: bool,
    schema_version: AtomicI64,
    migration_backup_created: AtomicBool,
    migration_backup_lock: Mutex<()>,
    diagnostics_lock: Mutex<()>,
    database_queue: DatabaseQueueHandle,
    profile_restore_pending_validation: bool,
}

/// Applies chat schema migrations through the service-owned storage boundary.
///
/// # Errors
/// Returns an error if the migration cannot safely complete.
pub fn migrate_chat_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    schema::chat_migrations::migrate_chat_schema(connection)
}

impl AppStorage {
    /// Opens the current user's local OpenChat database after platform and integrity checks.
    ///
    /// # Errors
    /// Returns a SQLite or filesystem error if the database cannot be opened safely.
    pub fn open() -> rusqlite::Result<Self> {
        let root = local_app_data_root()?;
        #[cfg(windows)]
        {
            Self::open_brokered(root, false)
        }
        #[cfg(not(windows))]
        {
            Self::open_at(root)
        }
    }

    /// Opens an empty AppContainer profile only after the user explicitly chose to preserve an
    /// inaccessible legacy Windows profile without importing it.
    pub(crate) fn open_with_empty_legacy_profile() -> rusqlite::Result<Self> {
        let root = local_app_data_root()?;
        #[cfg(windows)]
        {
            Self::open_brokered(root, true)
        }
        #[cfg(not(windows))]
        {
            Self::open_at(root)
        }
    }

    #[cfg(windows)]
    fn open_brokered(
        root: PathBuf,
        start_with_empty_legacy_profile: bool,
    ) -> rusqlite::Result<Self> {
        ensure_supported_sqlite_runtime()?;
        let database_directory = root.join("db");
        let logs_directory = root.join("logs");
        let cache_directory = root.join("cache");
        fs::create_dir_all(&database_directory).map_err(filesystem_error)?;
        fs::create_dir_all(&logs_directory).map_err(filesystem_error)?;
        fs::create_dir_all(&cache_directory).map_err(filesystem_error)?;

        let database_path = database_directory.join("openchat.sqlite3");
        let broker = BrokerConnection::connect()?;
        let database_key = Arc::new(
            database_key::read_stored_database_key()?.ok_or(rusqlite::Error::InvalidQuery)?,
        );
        let image_files_root = appcontainer_image_files_root()?;
        let import_status = broker.legacy_import_status()?;
        let mut preserve_legacy_sources = matches!(
            import_status,
            LegacyImportStatus::CompletedWithLegacySourcePreserved
        );
        match import_status {
            LegacyImportStatus::PendingEmpty => {
                let protected_legacy_profile = if start_with_empty_legacy_profile {
                    false
                } else {
                    legacy_protected_storage_may_contain_profile_data().map_err(|_| {
                        rusqlite::Error::SqliteFailure(
                            ffi::Error::new(ffi::SQLITE_CANTOPEN),
                            Some(
                                "legacy_profile_check_failed: The previous OpenChat storage state could not be checked safely."
                                    .to_owned(),
                            ),
                        )
                    })?
                };
                if start_with_empty_legacy_profile {
                    broker
                        .complete_empty_legacy_import(crate::secrets::SecretMap::default(), true)?;
                    preserve_legacy_sources = true;
                } else if protected_legacy_profile {
                    protect_legacy_profile_databases(&database_directory, &database_key)?;
                    return Err(rusqlite::Error::SqliteFailure(
                        ffi::Error::new(ffi::SQLITE_CANTOPEN),
                        Some(
                            "legacy_profile_conflict: The previous OpenChat storage profile may contain newer data. Choose a separate empty profile to continue without importing it."
                                .to_owned(),
                        ),
                    ));
                } else {
                    let legacy_database_exists = legacy_database_exists(&database_path)?;
                    let (secrets, legacy_secret_path) =
                        crate::secrets::read_for_storage_migration(&database_path)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    if legacy_database_exists {
                        let snapshot_path = cache_directory.join(format!(
                            "storage-import-{}.sqlite3",
                            uuid::Uuid::new_v4().simple()
                        ));
                        let result = (|| {
                            create_legacy_import_snapshot(
                                &database_path,
                                &snapshot_path,
                                &database_key,
                            )?;
                            broker.import_legacy_snapshot(&snapshot_path, secrets)
                        })();
                        let _ = fs::remove_file(&snapshot_path);
                        result?;
                    } else {
                        broker.complete_empty_legacy_import(secrets, false)?;
                    }
                    crate::secrets::finish_storage_migration(legacy_secret_path.as_deref())
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                }
            }
            LegacyImportStatus::Completed => {
                crate::secrets::finish_completed_storage_migration()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
            }
            LegacyImportStatus::CompletedWithLegacySourcePreserved => {}
            LegacyImportStatus::ExistingData => {
                if legacy_database_exists(&database_path)? {
                    protect_legacy_profile_databases(&database_directory, &database_key)?;
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
        }

        if !preserve_legacy_sources {
            migrate_legacy_local_models(
                &database_directory.join("local_models.sqlite3"),
                &broker,
                &database_key,
            )?;
        }
        protect_legacy_profile_databases(&database_directory, &database_key)?;

        let schema_version = broker.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
            [],
            |row| row.get(0),
        )?;
        let database_queue = DatabaseQueueHandle::remote();
        Ok(Self {
            root,
            image_files_root,
            database_path,
            database_key,
            database_had_data_on_open: false,
            schema_version: AtomicI64::new(schema_version),
            migration_backup_created: AtomicBool::new(true),
            migration_backup_lock: Mutex::new(()),
            diagnostics_lock: Mutex::new(()),
            database_queue,
            profile_restore_pending_validation: false,
        })
    }

    /// Opens storage at a supplied root, recovering any interrupted profile restore first.
    ///
    /// # Errors
    /// Returns a SQLite or filesystem error if recovery or database initialization fails.
    pub fn open_at(root: PathBuf) -> rusqlite::Result<Self> {
        #[cfg(test)]
        {
            return Self::open_at_with_key(root, database_key::DatabaseKey::isolated_test_key());
        }
        #[cfg(all(feature = "desktop-profile-test-support", not(test)))]
        {
            let key = database_key::read_or_create_desktop_test_key(&root)?;
            return Self::open_at_with_key(root, key);
        }
        #[cfg(all(not(test), not(feature = "desktop-profile-test-support")))]
        {
            let _ = root;
            Err(rusqlite::Error::InvalidQuery)
        }
    }

    pub(crate) fn open_at_with_key(
        root: PathBuf,
        key: database_key::DatabaseKey,
    ) -> rusqlite::Result<Self> {
        let key = Arc::new(key);
        ensure_supported_sqlite_runtime()?;
        #[cfg(windows)]
        recover_interrupted_legacy_import(&root, &key)?;
        let profile_restore_pending_validation =
            crate::profile_archive::activate_pending_restore(&root, &key)?;
        match Self::open_active_root(root.clone(), key.clone()) {
            Ok(mut storage) => {
                storage.profile_restore_pending_validation = profile_restore_pending_validation;
                Ok(storage)
            }
            Err(error) => {
                if profile_restore_pending_validation {
                    crate::profile_archive::recover_unconfirmed_restore(&root, &key)?;
                }
                Err(error)
            }
        }
    }

    fn open_active_root(
        root: PathBuf,
        database_key: Arc<database_key::DatabaseKey>,
    ) -> rusqlite::Result<Self> {
        let database_directory = root.join("db");
        let logs_directory = root.join("logs");
        let cache_directory = root.join("cache");

        fs::create_dir_all(&database_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&logs_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&cache_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        convert_retained_pre_migration_backups(&database_directory, &database_key)?;

        let database_path = database_directory.join("openchat.sqlite3");
        let database_had_data_on_open = match fs::metadata(&database_path) {
            Ok(metadata) => metadata.len() > 0,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(error)));
            }
        };
        let (connection, _) = database_key.open_or_migrate(&database_path)?;
        disable_trusted_schema(&connection)?;
        connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

        if database_had_data_on_open {
            database_guard::verify_integrity(&connection)?;
        }
        let mut migration_backup_created = false;
        if database_had_data_on_open
            && database_guard::backend_schema_version(&connection)? < INITIAL_SCHEMA_VERSION
        {
            database_guard::create_verified_backup(&connection, &database_path, &database_key)?;
            migration_backup_created = true;
        }

        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -64000;
             PRAGMA mmap_size = 268435456;
             PRAGMA wal_autocheckpoint = 1000;",
        )?;
        let schema_version = initialize_schema(&connection, INITIAL_SCHEMA_VERSION)?;
        ensure_service_metadata(&connection)?;
        let database_queue = DatabaseQueueHandle::new(connection, database_key.clone());
        Ok(Self {
            image_files_root: root.clone(),
            root,
            database_path,
            database_key,
            database_had_data_on_open,
            schema_version: AtomicI64::new(schema_version),
            migration_backup_created: AtomicBool::new(migration_backup_created),
            migration_backup_lock: Mutex::new(()),
            diagnostics_lock: Mutex::new(()),
            database_queue,
            profile_restore_pending_validation: false,
        })
    }

    /// Returns the canonical root used for this storage instance.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the verified profile root used for persisted image files.
    pub(crate) fn image_files_root(&self) -> &Path {
        &self.image_files_root
    }

    /// Returns the SQLite database path owned by this storage instance.
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub(crate) fn database_key(&self) -> &database_key::DatabaseKey {
        &self.database_key
    }

    /// Returns the per-profile cache directory used by semantic memory indexes.
    pub fn semantic_memory_cache_directory(&self) -> PathBuf {
        self.root.join("cache").join("semantic-memory")
    }

    /// Returns the backend schema version currently initialized in this process.
    pub fn schema_version(&self) -> i64 {
        self.schema_version.load(Ordering::Acquire)
    }

    /// Appends bounded OAuth lifecycle metadata without storing authorization codes or tokens.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub fn log_oauth_event(
        &self,
        event: &'static str,
        address: Option<&'static str>,
        port: Option<u16>,
        code: Option<&'static str>,
        status: Option<u16>,
    ) -> io::Result<()> {
        self.append_diagnostic("oauth", event, |file| {
            if let Some(address) = address {
                write!(file, " address={address}")?;
            }
            if let Some(port) = port {
                write!(file, " port={port}")?;
            }
            if let Some(code) = code {
                write!(file, " code={code}")?;
            }
            if let Some(status) = status {
                write!(file, " status={status}")?;
            }
            Ok(())
        })
    }

    /// Appends bounded ChatGPT operation metadata without request or response content.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub fn log_chatgpt_event(
        &self,
        event: &'static str,
        operation: &'static str,
        status: Option<u16>,
        code: Option<&'static str>,
        duration_ms: Option<u128>,
        item_count: Option<usize>,
    ) -> io::Result<()> {
        self.append_diagnostic("chatgpt", event, |file| {
            write!(file, " operation={operation}")?;
            if let Some(status) = status {
                write!(file, " status={status}")?;
            }
            if let Some(code) = code {
                write!(file, " code={code}")?;
            }
            if let Some(duration_ms) = duration_ms {
                write!(file, " duration_ms={duration_ms}")?;
            }
            if let Some(item_count) = item_count {
                write!(file, " item_count={item_count}")?;
            }
            Ok(())
        })
    }

    /// Records a bounded ChatGPT storage retry or failure without request data.
    ///
    /// # Errors
    /// Returns an I/O error if diagnostic fields are invalid or the log cannot be updated.
    pub(crate) fn log_chatgpt_storage_diagnostic(
        &self,
        event: &'static str,
        operation: &'static str,
        code: &'static str,
        attempt: usize,
        delay_ms: Option<u128>,
    ) -> io::Result<()> {
        if !matches!(event, "storage_retry_scheduled" | "storage_failed")
            || !is_safe_diagnostic_code(operation)
            || !is_safe_diagnostic_code(code)
            || attempt == 0
            || attempt > 8
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ChatGPT storage diagnostic fields are invalid",
            ));
        }

        self.append_diagnostic("chatgpt", event, |entry| {
            write!(
                entry,
                " operation={operation} code={code} attempt={attempt}"
            )?;
            if let Some(delay_ms) = delay_ms {
                write!(entry, " delay_ms={delay_ms}")?;
            }
            Ok(())
        })
    }

    /// Appends a correlated response-stream diagnostic without provider content or credentials.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated or the stream ID is invalid.
    pub(crate) fn log_chatgpt_stream_diagnostic(
        &self,
        diagnostic: ChatGptStreamDiagnostic<'_>,
    ) -> io::Result<()> {
        if diagnostic.stream_id.len() != 32
            || !diagnostic
                .stream_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ChatGPT stream diagnostic ID is invalid",
            ));
        }

        self.append_diagnostic("chatgpt", diagnostic.event, |file| {
            write!(
                file,
                " operation=responses stream_id={}",
                diagnostic.stream_id
            )?;
            if let Some(attempt) = diagnostic.attempt {
                write!(file, " attempt={attempt}")?;
            }
            if let Some(retry_count) = diagnostic.retry_count {
                write!(file, " retry_count={retry_count}")?;
            }
            if let Some(retry_delay_ms) = diagnostic.retry_delay_ms {
                write!(file, " retry_delay_ms={retry_delay_ms}")?;
            }
            if let Some(continuation_count) = diagnostic.continuation_count {
                write!(file, " continuation_count={continuation_count}")?;
            }
            if let Some(status) = diagnostic.status {
                write!(file, " status={status}")?;
            }
            if let Some(http_version) = diagnostic.http_version {
                write!(file, " http_version={http_version}")?;
            }
            if let Some(content_encoding) = diagnostic.content_encoding {
                write!(file, " content_encoding={content_encoding}")?;
            }
            if let Some(transfer_encoding) = diagnostic.transfer_encoding {
                write!(file, " transfer_encoding={transfer_encoding}")?;
            }
            if let Some(content_length_header_bytes) = diagnostic.content_length_header_bytes {
                write!(
                    file,
                    " content_length_header_bytes={content_length_header_bytes}"
                )?;
            }
            if let Some(elapsed_ms) = diagnostic.elapsed_ms {
                write!(file, " elapsed_ms={elapsed_ms}")?;
            }
            if let Some(body_chunks_received) = diagnostic.body_chunks_received {
                write!(file, " body_chunks_received={body_chunks_received}")?;
            }
            if let Some(decoded_body_bytes_received) = diagnostic.decoded_body_bytes_received {
                write!(
                    file,
                    " decoded_body_bytes_received={decoded_body_bytes_received}"
                )?;
            }
            if let Some(first_chunk_ms) = diagnostic.first_chunk_ms {
                write!(file, " first_chunk_ms={first_chunk_ms}")?;
            }
            if let Some(last_chunk_idle_ms) = diagnostic.last_chunk_idle_ms {
                write!(file, " last_chunk_idle_ms={last_chunk_idle_ms}")?;
            }
            if let Some(error_code) = diagnostic.error_code {
                write!(file, " code={error_code}")?;
            }
            if let Some(reqwest_error_class) = diagnostic.reqwest_error_class {
                write!(file, " reqwest_error_class={reqwest_error_class}")?;
            }
            if let Some(source_error_class) = diagnostic.source_error_class {
                write!(file, " source_error_class={source_error_class}")?;
            }
            if let Some(source_io_kind) = diagnostic.source_io_kind {
                write!(file, " source_io_kind={source_io_kind}")?;
            }
            if let Some(retryable) = diagnostic.retryable {
                write!(file, " retryable={retryable}")?;
            }
            if let Some(completion_event_seen) = diagnostic.completion_event_seen {
                write!(file, " completion_event_seen={completion_event_seen}")?;
            }
            if let Some(partial_text_available) = diagnostic.partial_text_available {
                write!(file, " partial_text_available={partial_text_available}")?;
            }
            Ok(())
        })
    }

    /// Records a failed RPC using a fixed operation label, stable error code, and class.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_rpc_failure(
        &self,
        operation: &'static str,
        code: &'static str,
    ) -> io::Result<()> {
        self.log_rpc_failure_with_detail(operation, code, None)
    }

    /// Records a failed RPC and an optional stable diagnostic code from the service error.
    ///
    /// Detail values are written only when they match the bounded diagnostic-code format.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_rpc_failure_with_detail(
        &self,
        operation: &'static str,
        code: &'static str,
        detail_code: Option<&str>,
    ) -> io::Result<()> {
        let category = crate::error::ErrorCategory::from_code(code).as_str();
        self.append_diagnostic("rpc", "request_failed", |entry| {
            write!(
                entry,
                " operation={operation} code={code} category={category}"
            )?;
            if let Some(detail_code) = detail_code.filter(|value| is_safe_diagnostic_code(value)) {
                write!(entry, " detail_code={detail_code}")?;
            }
            Ok(())
        })
    }

    /// Records a failed RPC with bounded class and correlation metadata.
    ///
    /// # Errors
    /// Returns an I/O error if the operation metadata is invalid or the diagnostic log cannot be updated.
    pub(crate) fn log_rpc_failure_with_context(
        &self,
        operation: &str,
        code: &str,
        category: &str,
        correlation_id: &str,
        diagnostic_code: Option<&str>,
    ) -> io::Result<()> {
        if !is_safe_diagnostic_operation(operation)
            || !is_safe_diagnostic_code(code)
            || !is_safe_diagnostic_code(category)
            || !is_correlation_id(correlation_id)
            || diagnostic_code.is_some_and(|value| !is_safe_diagnostic_code(value))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RPC failure diagnostic metadata is invalid.",
            ));
        }

        self.append_diagnostic("rpc", "request_failed", |entry| {
            write!(
                entry,
                " operation={operation} code={code} category={category} correlation_id={correlation_id}"
            )?;
            if let Some(diagnostic_code) = diagnostic_code {
                write!(entry, " diagnostic_code={diagnostic_code}")?;
            }
            Ok(())
        })
    }

    /// Records a message-save failure using only a fixed stage and bounded error codes.
    ///
    /// # Errors
    /// Returns an I/O error if the stage or codes are invalid or the diagnostic log cannot be updated.
    pub(crate) fn log_chat_message_save_failure(
        &self,
        stage: &'static str,
        code: &'static str,
        detail_code: Option<&str>,
    ) -> io::Result<()> {
        if !matches!(
            stage,
            "attachment_staging"
                | "database_connect"
                | "transaction_begin"
                | "message_insert"
                | "conversation_update"
                | "attachment_commit"
                | "transaction_commit"
        ) || !is_safe_diagnostic_code(code)
            || detail_code.is_some_and(|value| !is_safe_diagnostic_code(value))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chat storage diagnostic fields are invalid",
            ));
        }

        self.append_diagnostic("chat_storage", "message_save_failed", |entry| {
            write!(entry, " stage={stage} code={code}")?;
            if let Some(detail_code) = detail_code {
                write!(entry, " detail_code={detail_code}")?;
            }
            Ok(())
        })
    }

    /// Records a failed file-change list request without paths or conversation IDs.
    pub(crate) fn log_file_changes_list_failure(
        &self,
        stage: &'static str,
        code: &'static str,
        elapsed_ms: u128,
    ) -> io::Result<()> {
        self.append_diagnostic("file_changes", "list_failed", |entry| {
            write!(entry, " stage={stage} code={code} elapsed_ms={elapsed_ms}")
        })
    }

    /// Records slow file-change reads using bounded aggregate timing data only.
    pub(crate) fn log_file_changes_list_timing(
        &self,
        entry_count: usize,
        lock_wait_ms: u128,
        database_ms: u128,
        validation_ms: u128,
    ) -> io::Result<()> {
        self.append_diagnostic("file_changes", "list_slow", |entry| {
            write!(
                entry,
                " entries={entry_count} lock_wait_ms={lock_wait_ms} database_ms={database_ms} validation_ms={validation_ms}"
            )
        })
    }

    /// Records why a tool checkpoint exceeded the same limits used by message storage RPCs.
    pub(crate) fn log_rejected_assistant_checkpoint(
        &self,
        reason: &'static str,
        content_bytes: usize,
        metadata_bytes: usize,
        activity_count: usize,
    ) -> io::Result<()> {
        if !matches!(reason, "content_limit" | "metadata_limit") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "assistant checkpoint diagnostic fields are invalid",
            ));
        }
        self.append_diagnostic("chat_storage", "assistant_checkpoint_rejected", |entry| {
            write!(
                entry,
                " reason={reason} content_bytes={content_bytes} metadata_bytes={metadata_bytes} activity_count={activity_count}"
            )
        })
    }

    /// Records file-change scans cancelled by the caller using aggregate counts and timings only.
    pub(crate) fn log_file_changes_list_cancelled(
        &self,
        stage: &'static str,
        entry_count: usize,
        validated_count: usize,
        lock_wait_ms: u128,
        database_ms: u128,
        validation_ms: u128,
    ) -> io::Result<()> {
        if !matches!(
            stage,
            "before_start" | "lock" | "after_manifest" | "validation"
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file-change cancellation diagnostic fields are invalid",
            ));
        }
        self.append_diagnostic("file_changes", "list_cancelled", |entry| {
            write!(
                entry,
                " stage={stage} entries={entry_count} validated={validated_count} lock_wait_ms={lock_wait_ms} database_ms={database_ms} validation_ms={validation_ms}"
            )
        })
    }

    /// Records a bounded image-library storage failure without paths or image contents.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated or the stage is invalid.
    pub(crate) fn log_image_library_failure(
        &self,
        stage: &'static str,
        error_kind: Option<io::ErrorKind>,
    ) -> io::Result<()> {
        if !matches!(
            stage,
            "root_canonicalize"
                | "directory_inspect"
                | "directory_create"
                | "directory_reinspect"
                | "directory_missing"
                | "directory_canonicalize"
                | "directory_validation"
                | "file_create"
                | "file_write"
                | "file_sync"
                | "file_metadata"
                | "file_validation"
                | "file_canonicalize"
                | "file_path_validation"
                | "read_metadata_validation"
                | "file_open"
                | "file_signature_read"
                | "file_signature_validation"
                | "file_seek"
                | "read_length_conversion"
                | "file_read"
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "image-library diagnostic stage is invalid",
            ));
        }
        let error_kind =
            error_kind.map_or_else(|| "validation".to_owned(), |kind| format!("{kind:?}"));
        self.append_diagnostic("image_library", "storage_failed", |entry| {
            write!(entry, " stage={stage} error_kind={error_kind}")
        })
    }

    /// Records that non-critical token metadata could not be saved after a completed response.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_context_usage_save_failure(&self) -> io::Result<()> {
        self.append_diagnostic("context_usage", "metadata_save_failed", |entry| {
            write!(entry, " code=context_metadata_write_failed")
        })
    }

    /// Records the fixed failure stage for Copilot catalog parsing without response contents.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_copilot_catalog_failure(
        &self,
        stage: &'static str,
        code: &'static str,
    ) -> io::Result<()> {
        if !matches!(
            stage,
            "user_metadata"
                | "api_endpoint"
                | "model_http"
                | "model_body"
                | "model_data"
                | "model_filter"
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Copilot catalog diagnostic stage is invalid",
            ));
        }
        self.append_diagnostic("copilot", "model_catalog_failed", |entry| {
            write!(entry, " stage={stage} code={code}")
        })
    }

    /// Records bounded aggregate model-filter counts without model names or response contents.
    ///
    /// # Errors
    /// Returns an I/O error if the counts are out of bounds or the diagnostic log cannot be updated.
    pub(crate) fn log_copilot_catalog_filter_summary(
        &self,
        summary: &CopilotCatalogFilterSummary,
    ) -> io::Result<()> {
        let counts = [
            summary.total,
            summary.policy_blocked,
            summary.non_chat,
            summary.picker_disabled,
            summary.invalid_id,
            summary.auto_candidates,
        ];
        if counts.iter().any(|count| *count > 1024) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Copilot catalog filter counts are out of bounds",
            ));
        }
        self.append_diagnostic("copilot", "model_catalog_filter_summary", |entry| {
            write!(
                entry,
                " total={} policy_blocked={} non_chat={} picker_disabled={} invalid_id={} auto_candidates={}",
                summary.total,
                summary.policy_blocked,
                summary.non_chat,
                summary.picker_disabled,
                summary.invalid_id,
                summary.auto_candidates,
            )
        })
    }

    /// Records bounded local-engine lifecycle metadata without model or credential contents.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_local_engine_event(
        &self,
        event: &'static str,
        code: Option<&'static str>,
        duration: Option<Duration>,
    ) -> io::Result<()> {
        self.append_diagnostic("local_engine", event, |entry| {
            if let Some(code) = code {
                write!(entry, " code={code}")?;
            }
            if let Some(duration) = duration {
                write!(entry, " duration_ms={}", duration.as_millis())?;
            }
            Ok(())
        })
    }

    /// Records SQLite and schema versions after database initialization.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub fn log_database_runtime(&self, chat_schema_version: i64) -> io::Result<()> {
        self.append_diagnostic("database", "runtime_initialized", |entry| {
            write!(
                entry,
                " sqlite_version={} backend_schema_version={} chat_schema_version={chat_schema_version}",
                rusqlite::version(),
                self.schema_version(),
            )
        })
    }

    /// Records validated token counts and route identifiers for usage statistics.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_provider_request_usage(
        &self,
        diagnostic: ProviderUsageDiagnostic<'_>,
    ) -> io::Result<()> {
        let ProviderUsageDiagnostic {
            provider_id,
            model_id,
            operation,
            prompt_tokens,
            completion_tokens,
            cached_tokens,
            cache_write_tokens,
            cache_discount,
            estimated_prompt_tokens,
            image_inputs,
        } = diagnostic;
        if !matches!(provider_id, "cerebras" | "mistral" | "openrouter") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "provider usage diagnostics are not enabled for this provider",
            ));
        }
        let operation = match operation {
            "chat" | "tool_follow_up" | "compaction" => operation,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "provider usage diagnostic operation is invalid",
                ));
            }
        };
        let provider = diagnostic_identifier(provider_id, 32);
        let model = diagnostic_identifier(model_id, 128);
        if provider.is_empty()
            || model.is_empty()
            || [
                prompt_tokens,
                completion_tokens,
                cached_tokens,
                cache_write_tokens,
            ]
            .into_iter()
            .flatten()
            .any(|tokens| tokens < 0)
            || estimated_prompt_tokens < 0
            || image_inputs < 0
            || cache_discount.is_some_and(|discount| !discount.is_finite())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "provider usage diagnostic values are invalid",
            ));
        }

        let event = if prompt_tokens.is_some()
            || completion_tokens.is_some()
            || cached_tokens.is_some()
            || cache_write_tokens.is_some()
            || cache_discount.is_some()
        {
            "usage_reported"
        } else {
            "usage_unavailable"
        };
        self.append_diagnostic("provider_usage", event, |entry| {
            write!(
                entry,
                " provider={provider} model={model} operation={operation} estimated_prompt_tokens={estimated_prompt_tokens} image_inputs={image_inputs}"
            )?;
            if let Some(prompt_tokens) = prompt_tokens {
                write!(entry, " prompt_tokens={prompt_tokens}")?;
            }
            if let Some(completion_tokens) = completion_tokens {
                write!(entry, " completion_tokens={completion_tokens}")?;
            }
            if let Some(cached_tokens) = cached_tokens {
                write!(entry, " cached_tokens={cached_tokens}")?;
            }
            if let Some(cache_write_tokens) = cache_write_tokens {
                write!(entry, " cache_write_tokens={cache_write_tokens}")?;
            }
            if let Some(cache_discount) = cache_discount {
                write!(entry, " cache_discount={cache_discount}")?;
            }
            Ok(())
        })
    }

    fn append_diagnostic(
        &self,
        component: &'static str,
        event: &'static str,
        write_fields: impl FnOnce(&mut Vec<u8>) -> io::Result<()>,
    ) -> io::Result<()> {
        let _guard = self
            .diagnostics_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis();
        let mut entry = Vec::new();
        let process_id = std::process::id();
        let build_profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        write!(
            entry,
            "timestamp_unix_ms={timestamp} pid={process_id} version={} build_profile={build_profile} component={component} event={event}",
            env!("CARGO_PKG_VERSION")
        )?;
        write_fields(&mut entry)?;
        writeln!(entry)?;
        if entry.len() > MAX_DIAGNOSTIC_ENTRY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "service diagnostic entry exceeded its size limit",
            ));
        }

        let log_path = self.root.join("logs").join("openchat-service.log");
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(log_path)?;
        let entry_size = u64::try_from(entry.len()).map_err(io::Error::other)?;
        if file.metadata()?.len().saturating_add(entry_size) > MAX_DIAGNOSTIC_LOG_BYTES {
            file.set_len(0)?;
        }
        file.seek(SeekFrom::End(0))?;
        file.write_all(&entry)?;
        file.flush()
    }

    /// Applies backend schema migrations after checking and backing up existing data.
    ///
    /// # Errors
    /// Returns a SQLite or backup error if integrity checks, backup creation, or migration fails.
    pub fn initialize_backend_schema(&self) -> rusqlite::Result<i64> {
        if self.database_queue.is_remote() {
            let connection = self.database_queue.connect()?;
            connection.verify_integrity()?;
            let chat_schema_exists = connection.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'conversations'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            let schema_version = connection.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            let chat_schema_version =
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
            if !chat_schema_exists
                || schema_version < SCHEMA_VERSION
                || chat_schema_version != crate::CHAT_SCHEMA_VERSION
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
            self.schema_version.store(schema_version, Ordering::Release);
            if self.log_database_runtime(chat_schema_version).is_err() {
                eprintln!("database_runtime_diagnostic_write_failed");
            }
            return Ok(schema_version);
        }

        let connection = self.database_queue.connect_local()?;
        if database_guard::backend_schema_version(&connection)? < SCHEMA_VERSION {
            self.create_migration_backup_if_needed(&connection)?;
        }
        let chat_schema_exists = connection.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'conversations'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if !chat_schema_exists {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let schema_version = initialize_schema(&connection, SCHEMA_VERSION)?;
        ensure_service_metadata(&connection)?;
        self.schema_version.store(schema_version, Ordering::Release);
        let chat_schema_version = database_guard::chat_schema_version(&connection)?;
        if self.log_database_runtime(chat_schema_version).is_err() {
            eprintln!("database_runtime_diagnostic_write_failed");
        }
        Ok(schema_version)
    }

    /// Commits a profile restore after the caller confirms restored state is valid.
    ///
    /// # Errors
    /// Returns a SQLite or filesystem error if the restore journal cannot be committed.
    pub fn commit_pending_profile_restore(&self) -> rusqlite::Result<()> {
        #[cfg(windows)]
        if self.database_queue.is_remote() {
            return BrokerConnection::connect()?.commit_pending_profile_restore();
        }
        if self.profile_restore_pending_validation {
            crate::profile_archive::commit_pending_restore(&self.root)?;
        }
        Ok(())
    }

    /// Verifies the chat schema and creates a migration backup when an update is pending.
    ///
    /// Returns whether a new backup was created.
    ///
    /// # Errors
    /// Returns a SQLite or backup error for an invalid target version or unsafe database state.
    pub fn prepare_chat_schema_migration(&self, target_version: i64) -> rusqlite::Result<bool> {
        if target_version != crate::CHAT_SCHEMA_VERSION {
            return Err(rusqlite::Error::InvalidQuery);
        }

        if self.database_queue.is_remote() {
            let connection = self.database_queue.connect()?;
            connection.verify_integrity()?;
            let chat_schema_version =
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
            let backend_version = connection.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            if chat_schema_version != target_version || backend_version < SCHEMA_VERSION {
                return Err(rusqlite::Error::InvalidQuery);
            }
            self.schema_version
                .store(backend_version, Ordering::Release);
            return Ok(false);
        }

        let mut connection = self.database_queue.connect_local()?;
        database_guard::verify_integrity(&connection)?;
        let current_chat_version = database_guard::chat_schema_version(&connection)?;
        if current_chat_version > target_version {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let backend_version = database_guard::backend_schema_version(&connection)?;
        let migration_pending = current_chat_version < target_version
            || backend_version < SCHEMA_VERSION
            || schema::chat_migrations::needs_repair(&connection)?
            || schema::repair::needs_repair(&connection, backend_version)?;
        if !migration_pending {
            schema::chat_migrations::migrate_chat_schema(&mut connection)?;
            return Ok(false);
        }

        let backup_created = self.create_migration_backup_if_needed(&connection)?;
        schema::chat_migrations::migrate_chat_schema(&mut connection)?;
        let backend_version = initialize_schema(&connection, SCHEMA_VERSION)?;
        ensure_service_metadata(&connection)?;
        self.schema_version
            .store(backend_version, Ordering::Release);
        let now = schema::unix_time_millis()?;
        connection.execute("DELETE FROM ephemeral_cache WHERE expires_at <= ?1", [now])?;
        connection.execute("PRAGMA optimize", [])?;
        database_guard::verify_integrity(&connection)?;
        Ok(backup_created)
    }

    /// Acquires the next operation lease on the primary SQLite storage.
    ///
    /// The queue slot stays occupied until the returned connection is dropped. Callers must
    /// release it before awaiting network, provider, or other long-running work.
    ///
    /// # Errors
    /// Returns a SQLite error if the connection or required SQL functions cannot be initialized.
    pub(crate) fn connect(&self) -> rusqlite::Result<StorageConnection<'_>> {
        self.database_queue.connect()
    }

    /// Acquires the concrete SQLite connection for service-owned maintenance and pipe handling.
    pub(crate) fn connect_owned_database(&self) -> rusqlite::Result<QueuedConnection<'_>> {
        self.database_queue.connect_local()
    }

    #[cfg(windows)]
    pub(crate) fn create_owned_database_snapshot(
        &self,
        destination: &Path,
    ) -> rusqlite::Result<()> {
        let connection = self.database_queue.connect_local()?;
        database_guard::create_verified_snapshot(&connection, destination, &self.database_key)
    }

    pub(crate) fn database_queue_handle(&self) -> DatabaseQueueHandle {
        self.database_queue.clone()
    }

    #[cfg(windows)]
    pub(crate) fn legacy_import_status(&self) -> rusqlite::Result<LegacyImportStatus> {
        let connection = self.database_queue.connect_local()?;
        ensure_service_metadata(&connection)?;
        legacy_import_status_from_connection(&connection)
    }

    #[cfg(windows)]
    pub(crate) fn complete_empty_legacy_import(
        &self,
        secrets: crate::secrets::SecretMap,
        preserve_legacy_source: bool,
    ) -> rusqlite::Result<()> {
        if self.legacy_import_status()? != LegacyImportStatus::PendingEmpty {
            return Err(rusqlite::Error::InvalidQuery);
        }
        secrets
            .write_to_storage(self)
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        verify_imported_secrets(self, &secrets)?;
        let connection = self.database_queue.connect_local()?;
        mark_legacy_import_completed(&connection, preserve_legacy_source)
    }

    #[cfg(windows)]
    pub(crate) fn import_legacy_snapshot(
        &self,
        snapshot_path: &Path,
        secrets: crate::secrets::SecretMap,
    ) -> rusqlite::Result<()> {
        if self.legacy_import_status()? != LegacyImportStatus::PendingEmpty {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let source = self
            .database_key
            .open(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        disable_trusted_schema(&source)?;
        validate_legacy_import_snapshot(&source)?;

        let rollback_path = self.root.join("cache").join(LEGACY_IMPORT_ROLLBACK_NAME);
        {
            let current = self.database_queue.connect_local()?;
            database_guard::create_verified_snapshot(&current, &rollback_path, &self.database_key)?;
        }
        write_legacy_import_journal(&self.root)?;

        let import_result = (|| {
            {
                let mut destination = self.database_queue.connect_local()?;
                copy_database(&source, &mut destination)?;
                database_guard::verify_integrity(&destination)?;
                ensure_service_metadata(&destination)?;
            }
            drop(source);
            self.prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)?;
            self.initialize_backend_schema()?;
            secrets
                .write_to_storage(self)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            verify_imported_secrets(self, &secrets)?;
            let connection = self.database_queue.connect_local()?;
            mark_legacy_import_completed(&connection, false)
        })();

        if let Err(error) = import_result {
            restore_database_snapshot(&rollback_path, self)?;
            self.schema_version.store(SCHEMA_VERSION, Ordering::Release);
            finish_legacy_import_journal(&self.root)?;
            return Err(error);
        }
        finish_legacy_import_journal(&self.root)?;
        Ok(())
    }

    fn create_migration_backup_if_needed(&self, connection: &Connection) -> rusqlite::Result<bool> {
        if !self.database_had_data_on_open || self.migration_backup_created.load(Ordering::Acquire)
        {
            return Ok(false);
        }

        let _guard = self
            .migration_backup_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.migration_backup_created.load(Ordering::Acquire) {
            return Ok(false);
        }

        let _backup_path = database_guard::create_verified_backup(
            connection,
            &self.database_path,
            &self.database_key,
        )?;
        self.migration_backup_created.store(true, Ordering::Release);
        Ok(true)
    }
}

fn is_safe_diagnostic_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_safe_diagnostic_operation(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn is_correlation_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(windows)]
fn legacy_protected_storage_may_contain_profile_data() -> rusqlite::Result<bool> {
    let program_data = env::var_os("PROGRAMDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(legacy_protected_storage_check_error)?;
    let protected_root = program_data.join("OpenChat").join("Storage");
    match fs::symlink_metadata(protected_root) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(true),
        Err(_) => Err(legacy_protected_storage_check_error()),
    }
}

#[cfg(windows)]
fn legacy_database_exists(database_path: &Path) -> rusqlite::Result<bool> {
    match fs::symlink_metadata(database_path) {
        Ok(metadata)
            if metadata.is_file()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 =>
        {
            Ok(metadata.len() > 0)
        }
        Ok(_) => Err(rusqlite::Error::InvalidPath(database_path.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(filesystem_error(error)),
    }
}

#[cfg(windows)]
fn legacy_protected_storage_check_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_CANTOPEN),
        Some("The previous OpenChat storage state could not be checked safely.".to_owned()),
    )
}

#[cfg(windows)]
struct LegacyLocalModel {
    id: String,
    engine_id: String,
    display_name: String,
    model_path: String,
    path_kind: String,
    created_at_unix_ms: i64,
}

#[cfg(windows)]
fn protect_legacy_database_file(
    path: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(filesystem_error(error)),
    };
    if !metadata.is_file()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.len() < 16
    {
        return Err(rusqlite::Error::InvalidPath(path.to_path_buf()));
    }
    let (database, _) = database_key.open_or_migrate(path)?;
    database_guard::verify_integrity(&database)
}

#[cfg(windows)]
fn migrate_legacy_local_models(
    legacy_path: &Path,
    broker: &BrokerConnection,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let completed = broker
        .query_row(
            "SELECT value FROM openchat_storage_metadata
             WHERE key = 'legacy_local_models_import_completed'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match completed.as_deref() {
        Some("1") => return Ok(()),
        Some(_) => return Err(rusqlite::Error::InvalidQuery),
        None => {}
    }

    let mut models = Vec::new();
    match fs::symlink_metadata(legacy_path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || metadata.len() < 16
            {
                return Err(rusqlite::Error::InvalidPath(legacy_path.to_path_buf()));
            }
            let (source, _) = database_key.open_or_migrate(legacy_path)?;
            disable_trusted_schema(&source)?;
            database_guard::verify_integrity(&source)?;
            let has_catalog = source.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'local_models'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if has_catalog {
                let mut statement = source.prepare(
                    "SELECT id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
                     FROM local_models ORDER BY id",
                )?;
                let mut rows = statement.query([])?;
                let mut total_bytes = 0usize;
                while let Some(row) = rows.next()? {
                    if models.len() >= 100_000 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let model = LegacyLocalModel {
                        id: row.get(0)?,
                        engine_id: row.get(1)?,
                        display_name: row.get(2)?,
                        model_path: row.get(3)?,
                        path_kind: row.get(4)?,
                        created_at_unix_ms: row.get(5)?,
                    };
                    total_bytes = total_bytes
                        .checked_add(model.id.len())
                        .and_then(|size| size.checked_add(model.engine_id.len()))
                        .and_then(|size| size.checked_add(model.display_name.len()))
                        .and_then(|size| size.checked_add(model.model_path.len()))
                        .and_then(|size| size.checked_add(model.path_kind.len()))
                        .filter(|size| *size <= 16 * 1024 * 1024)
                        .ok_or(rusqlite::Error::InvalidQuery)?;
                    if !valid_legacy_local_model(&model) {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    models.push(model);
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(filesystem_error(error)),
    }

    let transaction = broker.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for model in models {
        transaction.execute(
            "INSERT INTO local_models (
                id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(engine_id, model_path) DO NOTHING",
            rusqlite::params![
                model.id,
                model.engine_id,
                model.display_name,
                model.model_path,
                model.path_kind,
                model.created_at_unix_ms,
            ],
        )?;
    }
    transaction.execute(
        "INSERT INTO openchat_storage_metadata (key, value)
         VALUES ('legacy_local_models_import_completed', '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

#[cfg(windows)]
fn valid_legacy_local_model(model: &LegacyLocalModel) -> bool {
    !model.id.is_empty()
        && model.id.len() <= 128
        && !model.id.chars().any(char::is_control)
        && model.engine_id == "llama_cpp"
        && !model.display_name.trim().is_empty()
        && model.display_name.len() <= 512
        && !model.display_name.chars().any(char::is_control)
        && Path::new(&model.model_path).is_absolute()
        && model.model_path.len() <= 32_768
        && !model.model_path.chars().any(char::is_control)
        && matches!(model.path_kind.as_str(), "file" | "directory")
        && model.created_at_unix_ms > 0
}

fn ensure_service_metadata(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS openchat_storage_metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
         );",
    )
}

#[cfg(windows)]
fn legacy_import_status_from_connection(
    connection: &Connection,
) -> rusqlite::Result<LegacyImportStatus> {
    let imported = connection
        .query_row(
            "SELECT value FROM openchat_storage_metadata
             WHERE key = 'legacy_import_completed'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match imported.as_deref() {
        Some("1") => {
            let preserved = connection
                .query_row(
                    "SELECT value FROM openchat_storage_metadata
                     WHERE key = 'legacy_source_preserved'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            return match preserved.as_deref() {
                Some("1") => Ok(LegacyImportStatus::CompletedWithLegacySourcePreserved),
                None => Ok(LegacyImportStatus::Completed),
                Some(_) => Err(rusqlite::Error::InvalidQuery),
            };
        }
        Some(_) => return Err(rusqlite::Error::InvalidQuery),
        None => {}
    }

    let tables = [
        "conversations",
        "messages",
        "projects",
        "workspaces",
        "secure_vault",
        "chatgpt_connections",
        "chatgpt_workspaces",
        "copilot_oauth_connection",
        "app_settings",
        "personal_memory_facts",
        "usage_events",
        "model_favorites",
        "tool_permission_mode",
        "project_tool_permission_rules",
    ];
    for table in tables {
        let table_exists = connection.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
            )",
            [table],
            |row| row.get::<_, bool>(0),
        )?;
        if table_exists {
            let has_rows = connection.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} LIMIT 1)"),
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if has_rows {
                return Ok(LegacyImportStatus::ExistingData);
            }
        }
    }
    Ok(LegacyImportStatus::PendingEmpty)
}

#[cfg(windows)]
fn mark_legacy_import_completed(
    connection: &Connection,
    preserve_legacy_source: bool,
) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT INTO openchat_storage_metadata (key, value)
         VALUES ('legacy_import_completed', '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    if preserve_legacy_source {
        transaction.execute(
            "INSERT INTO openchat_storage_metadata (key, value)
             VALUES ('legacy_source_preserved', '1')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
    }
    transaction.commit()
}

#[cfg(windows)]
fn validate_legacy_import_snapshot(source: &Connection) -> rusqlite::Result<()> {
    database_guard::verify_integrity(source)?;
    let chat_schema_version = database_guard::chat_schema_version(source)?;
    if chat_schema_version > crate::CHAT_SCHEMA_VERSION
        || database_guard::backend_schema_version(source)? > SCHEMA_VERSION
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let has_conversations = source.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'conversations'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_conversations {
        return Err(rusqlite::Error::InvalidQuery);
    }
    {
        let mut foreign_keys = source.prepare("PRAGMA foreign_key_check")?;
        if foreign_keys.exists([])? {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    let vault_exists = source.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'secure_vault'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if vault_exists {
        let old_dpapi_values = source.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM secure_vault WHERE substr(key, 1, 8) != 'keyring:'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if old_dpapi_values {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn create_legacy_import_snapshot(
    source_path: &Path,
    snapshot_path: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&source)?;
    database_guard::verify_plaintext_integrity(&source)?;
    let chat_schema_version = database_guard::chat_schema_version(&source)?;
    if chat_schema_version > crate::CHAT_SCHEMA_VERSION
        || database_guard::backend_schema_version(&source)? > SCHEMA_VERSION
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    {
        let mut foreign_keys = source.prepare("PRAGMA foreign_key_check")?;
        if foreign_keys.exists([])? {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    database_key.export_plaintext_database(source_path, snapshot_path)?;
    drop(source);

    let snapshot = database_key.open(snapshot_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    disable_trusted_schema(&snapshot)?;
    let vault_exists = snapshot.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'secure_vault'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if vault_exists {
        snapshot.execute(
            "DELETE FROM secure_vault WHERE substr(key, 1, 8) != 'keyring:'",
            [],
        )?;
    }
    database_guard::verify_integrity(&snapshot)
}

#[cfg(windows)]
fn verify_imported_secrets(
    storage: &AppStorage,
    expected: &crate::secrets::SecretMap,
) -> rusqlite::Result<()> {
    let stored = crate::secrets::SecretMap::read_from_storage(storage)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if stored.has_same_entries_as(expected) {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

#[cfg(windows)]
fn restore_database_snapshot(snapshot_path: &Path, storage: &AppStorage) -> rusqlite::Result<()> {
    let source = storage
        .database_key
        .open(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&source)?;
    let mut destination = storage.database_queue.connect_local()?;
    copy_database(&source, &mut destination)?;
    database_guard::verify_integrity(&destination)?;
    ensure_service_metadata(&destination)
}

#[cfg(windows)]
fn write_legacy_import_journal(root: &Path) -> rusqlite::Result<()> {
    let cache_directory = root.join("cache");
    fs::create_dir_all(&cache_directory).map_err(filesystem_error)?;
    let journal_path = cache_directory.join(LEGACY_IMPORT_JOURNAL_NAME);
    let partial_path = journal_path.with_extension("in-progress.partial");
    remove_internal_file(&partial_path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial_path)
        .map_err(filesystem_error)?;
    file.write_all(LEGACY_IMPORT_JOURNAL_CONTENT)
        .map_err(filesystem_error)?;
    file.sync_all().map_err(filesystem_error)?;
    drop(file);
    fs::rename(&partial_path, &journal_path).map_err(filesystem_error)
}

#[cfg(windows)]
fn convert_interrupted_legacy_import_snapshots(
    cache_directory: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let entries = match fs::read_dir(cache_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(filesystem_error(error)),
    };
    for entry in entries {
        let entry = entry.map_err(filesystem_error)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("storage-import-") || !name.ends_with(".sqlite3") {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(filesystem_error)?;
        if !metadata.is_file()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || metadata.len() < 16
        {
            return Err(rusqlite::Error::InvalidPath(path));
        }
        let (connection, _) = database_key.open_or_migrate(&path)?;
        database_guard::verify_integrity(&connection)?;
    }
    Ok(())
}

#[cfg(windows)]
fn recover_interrupted_legacy_import(
    root: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let cache_directory = root.join("cache");
    convert_interrupted_legacy_import_snapshots(&cache_directory, database_key)?;
    let journal_path = cache_directory.join(LEGACY_IMPORT_JOURNAL_NAME);
    let rollback_path = cache_directory.join(LEGACY_IMPORT_ROLLBACK_NAME);
    let journal_metadata = match fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            remove_internal_file(&rollback_path)?;
            remove_internal_file(&journal_path.with_extension("in-progress.partial"))?;
            remove_recovery_database_files(root)?;
            return Ok(());
        }
        Err(error) => return Err(filesystem_error(error)),
    };
    if !journal_metadata.is_file()
        || journal_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || journal_metadata.len() as usize != LEGACY_IMPORT_JOURNAL_CONTENT.len()
    {
        return Err(rusqlite::Error::InvalidPath(journal_path));
    }
    let journal = fs::read(&journal_path).map_err(filesystem_error)?;
    if journal != LEGACY_IMPORT_JOURNAL_CONTENT {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let rollback_metadata = fs::symlink_metadata(&rollback_path).map_err(filesystem_error)?;
    if !rollback_metadata.is_file()
        || rollback_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(rusqlite::Error::InvalidPath(rollback_path));
    }
    if rollback_metadata.len() < 16 {
        return Err(rusqlite::Error::InvalidPath(rollback_path));
    }
    let (rollback, _) = database_key.open_or_migrate(&rollback_path)?;
    disable_trusted_schema(&rollback)?;
    database_guard::verify_integrity(&rollback)?;
    drop(rollback);

    let database_path = root.join("db").join("openchat.sqlite3");
    if !legacy_import_completed_in_database(&database_path, database_key)? {
        restore_snapshot_file(&rollback_path, &database_path, database_key)?;
    }
    finish_legacy_import_journal(root)
}

#[cfg(windows)]
fn legacy_import_completed_in_database(
    database_path: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<bool> {
    let metadata = match fs::symlink_metadata(database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(filesystem_error(error)),
    };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(rusqlite::Error::InvalidPath(database_path.to_path_buf()));
    }
    if metadata.len() < 16 {
        return Err(rusqlite::Error::InvalidPath(database_path.to_path_buf()));
    }
    let (connection, _) = database_key.open_or_migrate(database_path)?;
    disable_trusted_schema(&connection)?;
    database_guard::verify_integrity(&connection)?;
    let metadata_table_exists = connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema
             WHERE type = 'table' AND name = 'openchat_storage_metadata'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !metadata_table_exists {
        return Ok(false);
    }
    let value = connection
        .query_row(
            "SELECT value FROM openchat_storage_metadata
             WHERE key = 'legacy_import_completed'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match value.as_deref() {
        Some("1") => Ok(true),
        None => Ok(false),
        Some(_) => Err(rusqlite::Error::InvalidQuery),
    }
}

#[cfg(windows)]
fn restore_snapshot_file(
    snapshot_path: &Path,
    database_path: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let database_directory = database_path
        .parent()
        .ok_or_else(|| rusqlite::Error::InvalidPath(database_path.to_path_buf()))?;
    fs::create_dir_all(database_directory).map_err(filesystem_error)?;
    let recovery_path = database_directory.join("openchat.sqlite3.import-recovery");
    remove_internal_file(&recovery_path)?;
    remove_internal_file(&recovery_path.with_extension("backup.partial"))?;

    let source = database_key.open(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&source)?;
    database_guard::create_verified_snapshot(&source, &recovery_path, database_key)?;
    drop(source);

    remove_sqlite_sidecar(&database_path.with_extension("sqlite3-wal"))?;
    remove_sqlite_sidecar(&database_path.with_extension("sqlite3-shm"))?;
    remove_internal_file(database_path)?;
    fs::rename(&recovery_path, database_path).map_err(filesystem_error)?;

    let restored = database_key.open(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&restored)?;
    database_guard::verify_integrity(&restored)
}

#[cfg(windows)]
fn remove_sqlite_sidecar(path: &Path) -> rusqlite::Result<()> {
    remove_internal_file(path)
}

#[cfg(windows)]
fn remove_internal_file(path: &Path) -> rusqlite::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 =>
        {
            fs::remove_file(path).map_err(filesystem_error)
        }
        Ok(_) => Err(rusqlite::Error::InvalidPath(path.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem_error(error)),
    }
}

#[cfg(windows)]
fn remove_recovery_database_files(root: &Path) -> rusqlite::Result<()> {
    let database_directory = root.join("db");
    let recovery_path = database_directory.join("openchat.sqlite3.import-recovery");
    remove_internal_file(&recovery_path)?;
    remove_internal_file(&recovery_path.with_extension("backup.partial"))
}

#[cfg(windows)]
fn finish_legacy_import_journal(root: &Path) -> rusqlite::Result<()> {
    let cache_directory = root.join("cache");
    remove_internal_file(&cache_directory.join(LEGACY_IMPORT_JOURNAL_NAME))?;
    remove_internal_file(&cache_directory.join(LEGACY_IMPORT_ROLLBACK_NAME))?;
    remove_internal_file(
        &cache_directory
            .join(LEGACY_IMPORT_JOURNAL_NAME)
            .with_extension("in-progress.partial"),
    )?;
    remove_recovery_database_files(root)
}

#[cfg(windows)]
fn copy_database(
    source: &Connection,
    destination: &mut QueuedConnection<'_>,
) -> rusqlite::Result<()> {
    let backup = Backup::new(source, destination)?;
    let started_at = Instant::now();
    loop {
        match backup.step(128)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                if started_at.elapsed() >= Duration::from_secs(30) {
                    return Err(rusqlite::Error::SqliteFailure(
                        ffi::Error::new(ffi::SQLITE_BUSY),
                        Some("database snapshot operation timed out".to_owned()),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
    }
}

fn filesystem_error(error: io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

fn diagnostic_identifier(value: &str, max_length: usize) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/' | ':')
        })
        .take(max_length)
        .collect()
}

fn ensure_supported_sqlite_runtime() -> rusqlite::Result<()> {
    validate_sqlite_runtime(rusqlite::version_number(), rusqlite::version())
}

fn validate_sqlite_runtime(version_number: i32, version: &str) -> rusqlite::Result<()> {
    if version_number >= MINIMUM_SQLITE_VERSION_NUMBER {
        return Ok(());
    }

    Err(rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_ERROR),
        Some(format!(
            "sqlite_runtime_unsupported: OpenChat requires SQLite 3.51.3 or newer; linked runtime is {version} ({version_number})."
        )),
    ))
}

#[cfg(windows)]
fn protect_legacy_profile_databases(
    database_directory: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    protect_legacy_database_file(&database_directory.join("openchat.sqlite3"), database_key)?;
    protect_legacy_database_file(
        &database_directory.join("local_models.sqlite3"),
        database_key,
    )?;
    convert_retained_pre_migration_backups(database_directory, database_key)
}

fn convert_retained_pre_migration_backups(
    database_directory: &Path,
    database_key: &database_key::DatabaseKey,
) -> rusqlite::Result<()> {
    let backup_directory = database_directory.join("backups");
    let entries = match fs::read_dir(&backup_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(filesystem_error(error)),
    };
    for entry in entries {
        let entry = entry.map_err(filesystem_error)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("openchat.sqlite3.pre-migration-") || !name.ends_with(".backup") {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(filesystem_error)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() < 16 {
            return Err(rusqlite::Error::InvalidPath(path));
        }
        let (connection, _) = database_key.open_or_migrate(&path)?;
        database_guard::verify_integrity(&connection)?;
    }
    Ok(())
}

/// Runs the startup integrity check against an already opened database connection.
pub(crate) fn verify_database_integrity(connection: &Connection) -> rusqlite::Result<()> {
    database_guard::verify_integrity(connection)
}

/// Verifies an operation through the primary storage abstraction.
pub(crate) fn verify_storage_integrity(connection: &StorageConnection<'_>) -> rusqlite::Result<()> {
    connection.verify_integrity()
}

pub(crate) fn verify_plaintext_database_integrity(connection: &Connection) -> rusqlite::Result<()> {
    database_guard::verify_plaintext_integrity(connection)
}

/// Disables execution of non-innocuous application functions from database schema objects.
///
/// # Errors
/// Returns a SQLite error if the setting cannot be applied or verified.
pub(crate) fn disable_trusted_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.pragma_update(None, "trusted_schema", false)?;
    let trusted_schema_enabled =
        connection.pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))?;
    if trusted_schema_enabled {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

/// Creates an operation-level snapshot through the primary storage abstraction.
pub(crate) fn create_verified_storage_snapshot(
    source: &StorageConnection<'_>,
    destination: &Path,
) -> rusqlite::Result<()> {
    source.create_verified_snapshot(destination)
}

/// Returns the latest initialized backend schema version for status reporting.
pub(crate) fn backend_schema_version() -> i64 {
    SCHEMA_VERSION
}

fn local_app_data_root() -> rusqlite::Result<PathBuf> {
    #[cfg(windows)]
    {
        let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(|| {
            rusqlite::Error::InvalidPath(
                "LOCALAPPDATA is not available for the current Windows user".into(),
            )
        })?;
        Ok(PathBuf::from(local_app_data).join("OpenChat"))
    }

    #[cfg(not(windows))]
    {
        let _ = env::var_os("LOCALAPPDATA");
        Err(rusqlite::Error::InvalidPath(
            "The OpenChat local service currently supports Windows only".into(),
        ))
    }
}

#[cfg(windows)]
fn appcontainer_image_files_root() -> rusqlite::Result<PathBuf> {
    let storage_root = crate::sandbox::storage_broker_data_root().map_err(filesystem_error)?;
    let profile_root = storage_root.parent().ok_or_else(|| {
        rusqlite::Error::InvalidPath("The OpenChat storage profile is unavailable.".into())
    })?;
    let canonical_profile_root = fs::canonicalize(profile_root).map_err(filesystem_error)?;
    let metadata = fs::symlink_metadata(&storage_root).map_err(filesystem_error)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || !metadata.is_dir() {
        return Err(rusqlite::Error::InvalidPath(
            "The OpenChat image storage root is unsafe.".into(),
        ));
    }
    let canonical_storage_root = fs::canonicalize(&storage_root).map_err(filesystem_error)?;
    if !canonical_storage_root.starts_with(&canonical_profile_root) {
        return Err(rusqlite::Error::InvalidPath(
            "The OpenChat image storage root is outside its profile.".into(),
        ));
    }
    Ok(canonical_storage_root)
}

#[cfg(test)]
mod tests;
