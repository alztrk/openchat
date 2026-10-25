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
        Mutex,
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

const MAX_DIAGNOSTIC_LOG_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_ENTRY_BYTES: usize = 4096;
const MINIMUM_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
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
mod database_queue;
pub(crate) mod ipc_protocol;
#[cfg(windows)]
mod remote_connection;
mod schema;
pub(crate) mod tool_index_redaction;
pub(crate) mod user_questions;
pub(crate) use connection::{StorageConnection, StorageRow, StorageTransaction};
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
        rusqlite::Error::FromSqlConversionFailure(_, _, _) => "stored_value_invalid",
        rusqlite::Error::ToSqlConversionFailure(_) => "storage_conversion_failed",
        _ => "storage_operation_failed",
    }
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
    database_path: PathBuf,
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
                            create_legacy_import_snapshot(&database_path, &snapshot_path)?;
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
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
        }

        if !preserve_legacy_sources {
            migrate_legacy_local_models(&database_directory.join("local_models.sqlite3"), &broker)?;
            migrate_legacy_project_index(&cache_directory.join("project-index.sqlite3"), &broker)?;
        }

        let schema_version = broker.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
            [],
            |row| row.get(0),
        )?;
        let database_queue = DatabaseQueueHandle::remote();
        Ok(Self {
            root,
            database_path,
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
        ensure_supported_sqlite_runtime()?;
        #[cfg(windows)]
        recover_interrupted_legacy_import(&root)?;
        let profile_restore_pending_validation =
            crate::profile_archive::activate_pending_restore(&root)?;
        match Self::open_active_root(root.clone()) {
            Ok(mut storage) => {
                storage.profile_restore_pending_validation = profile_restore_pending_validation;
                Ok(storage)
            }
            Err(error) => {
                if profile_restore_pending_validation {
                    crate::profile_archive::recover_unconfirmed_restore(&root)?;
                }
                Err(error)
            }
        }
    }

    fn open_active_root(root: PathBuf) -> rusqlite::Result<Self> {
        let database_directory = root.join("db");
        let logs_directory = root.join("logs");
        let cache_directory = root.join("cache");

        fs::create_dir_all(&database_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&logs_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&cache_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

        let database_path = database_directory.join("openchat.sqlite3");
        let database_had_data_on_open = match fs::metadata(&database_path) {
            Ok(metadata) => metadata.len() > 0,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(error)));
            }
        };
        let connection = Connection::open_with_flags(
            &database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        disable_trusted_schema(&connection)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        database_queue::acquire_exclusive_lock(&connection)?;

        if database_had_data_on_open {
            database_guard::verify_integrity(&connection)?;
        }
        let mut migration_backup_created = false;
        if database_had_data_on_open
            && database_guard::backend_schema_version(&connection)? < INITIAL_SCHEMA_VERSION
        {
            database_guard::create_verified_backup(&connection, &database_path)?;
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
        let database_queue = DatabaseQueueHandle::new(connection);
        Ok(Self {
            root,
            database_path,
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

    /// Returns the SQLite database path owned by this storage instance.
    pub fn database_path(&self) -> &Path {
        &self.database_path
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

    /// Records a failed RPC using only a fixed operation label and stable error code.
    ///
    /// # Errors
    /// Returns an I/O error if the diagnostic log cannot be updated.
    pub(crate) fn log_rpc_failure(
        &self,
        operation: &'static str,
        code: &'static str,
    ) -> io::Result<()> {
        self.append_diagnostic("rpc", "request_failed", |entry| {
            write!(entry, " operation={operation} code={code}")
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
        write!(
            entry,
            "timestamp_unix_ms={timestamp} component={component} event={event}"
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
        database_guard::create_verified_snapshot(&connection, destination)
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

        let source = Connection::open_with_flags(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        disable_trusted_schema(&source)?;
        validate_legacy_import_snapshot(&source)?;

        let rollback_path = self.root.join("cache").join(LEGACY_IMPORT_ROLLBACK_NAME);
        {
            let current = self.database_queue.connect_local()?;
            database_guard::create_verified_snapshot(&current, &rollback_path)?;
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
            if let Err(restore_error) = restore_database_snapshot(&rollback_path, self) {
                return Err(restore_error);
            }
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

        let _backup_path = database_guard::create_verified_backup(connection, &self.database_path)?;
        self.migration_backup_created.store(true, Ordering::Release);
        Ok(true)
    }
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
struct LegacyProjectIndexState {
    project_id: String,
    enabled: bool,
}

#[cfg(windows)]
fn migrate_legacy_local_models(
    legacy_path: &Path,
    broker: &BrokerConnection,
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
            if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(rusqlite::Error::InvalidPath(legacy_path.to_path_buf()));
            }
            let source =
                Connection::open_with_flags(legacy_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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
fn migrate_legacy_project_index(
    legacy_path: &Path,
    broker: &BrokerConnection,
) -> rusqlite::Result<()> {
    let completed = broker
        .query_row(
            "SELECT value FROM openchat_storage_metadata
             WHERE key = 'legacy_project_index_import_completed'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match completed.as_deref() {
        Some("1") => return Ok(()),
        Some(_) => return Err(rusqlite::Error::InvalidQuery),
        None => {}
    }

    let mut states = Vec::new();
    match fs::symlink_metadata(legacy_path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(rusqlite::Error::InvalidPath(legacy_path.to_path_buf()));
            }
            let source =
                Connection::open_with_flags(legacy_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            disable_trusted_schema(&source)?;
            database_guard::verify_integrity(&source)?;
            let has_state = source.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'project_index_state'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if has_state {
                let mut statement = source.prepare(
                    "SELECT project_id, enabled FROM project_index_state ORDER BY project_id",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok(LegacyProjectIndexState {
                        project_id: row.get(0)?,
                        enabled: row.get(1)?,
                    })
                })?;
                for row in rows {
                    if states.len() >= 100_000 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let state = row?;
                    if state.project_id.is_empty()
                        || state.project_id.len() > 128
                        || state.project_id.chars().any(char::is_control)
                    {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    states.push(state);
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(filesystem_error(error)),
    }

    let transaction = broker.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for state in states {
        transaction.execute(
            "INSERT INTO project_index_state (
                project_id, enabled, status, indexed_files, indexed_bytes, updated_at_unix_ms
             ) VALUES (?1, ?2, ?3, 0, 0, NULL)
             ON CONFLICT(project_id) DO NOTHING",
            rusqlite::params![
                state.project_id,
                state.enabled,
                if state.enabled { "stale" } else { "disabled" },
            ],
        )?;
    }
    transaction.execute(
        "INSERT INTO openchat_storage_metadata (key, value)
         VALUES ('legacy_project_index_import_completed', '1')
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
fn create_legacy_import_snapshot(source_path: &Path, snapshot_path: &Path) -> rusqlite::Result<()> {
    let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&source)?;
    database_guard::verify_integrity(&source)?;
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
    database_guard::create_verified_snapshot(&source, snapshot_path)?;
    drop(source);

    let snapshot = Connection::open_with_flags(snapshot_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
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
    let source = Connection::open_with_flags(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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
fn recover_interrupted_legacy_import(root: &Path) -> rusqlite::Result<()> {
    let cache_directory = root.join("cache");
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
    let rollback = Connection::open_with_flags(&rollback_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&rollback)?;
    database_guard::verify_integrity(&rollback)?;
    drop(rollback);

    let database_path = root.join("db").join("openchat.sqlite3");
    if !legacy_import_completed_in_database(&database_path)? {
        restore_snapshot_file(&rollback_path, &database_path)?;
    }
    finish_legacy_import_journal(root)
}

#[cfg(windows)]
fn legacy_import_completed_in_database(database_path: &Path) -> rusqlite::Result<bool> {
    let metadata = match fs::symlink_metadata(database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(filesystem_error(error)),
    };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(rusqlite::Error::InvalidPath(database_path.to_path_buf()));
    }
    let connection = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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
fn restore_snapshot_file(snapshot_path: &Path, database_path: &Path) -> rusqlite::Result<()> {
    let database_directory = database_path
        .parent()
        .ok_or_else(|| rusqlite::Error::InvalidPath(database_path.to_path_buf()))?;
    fs::create_dir_all(database_directory).map_err(filesystem_error)?;
    let recovery_path = database_directory.join("openchat.sqlite3.import-recovery");
    remove_internal_file(&recovery_path)?;
    remove_internal_file(&recovery_path.with_extension("backup.partial"))?;

    let source = Connection::open_with_flags(snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    disable_trusted_schema(&source)?;
    database_guard::create_verified_snapshot(&source, &recovery_path)?;
    drop(source);

    remove_sqlite_sidecar(&database_path.with_extension("sqlite3-wal"))?;
    remove_sqlite_sidecar(&database_path.with_extension("sqlite3-shm"))?;
    remove_internal_file(database_path)?;
    fs::rename(&recovery_path, database_path).map_err(filesystem_error)?;

    let restored = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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

/// Runs the startup integrity check against an already opened database connection.
pub(crate) fn verify_database_integrity(connection: &Connection) -> rusqlite::Result<()> {
    database_guard::verify_integrity(connection)
}

/// Verifies an operation through the primary storage abstraction.
pub(crate) fn verify_storage_integrity(connection: &StorageConnection<'_>) -> rusqlite::Result<()> {
    connection.verify_integrity()
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rusqlite::Connection;
    use uuid::Uuid;

    use super::{
        AppStorage, CopilotCatalogFilterSummary, MAX_DIAGNOSTIC_LOG_BYTES,
        MINIMUM_SQLITE_VERSION_NUMBER, database_guard, disable_trusted_schema, sqlite_error_code,
        validate_sqlite_runtime,
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

        assert!(database_guard::is_corrupt_database_error(&error));
        assert_eq!(
            fs::read(&database_path).expect("read database fixture"),
            original
        );
        assert!(!database_directory.join("openchat.sqlite3-wal").exists());
    }

    #[test]
    fn app_connections_disable_trusted_schema() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");
        let connection = storage.connect().expect("open configured connection");
        let trusted_schema_enabled = connection
            .pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))
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
        let initial = Connection::open(&database_path).expect("create existing database");
        initial
            .execute_batch(
                "CREATE TABLE openchat_backend_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_unix_ms INTEGER NOT NULL
                 );
                 INSERT INTO openchat_backend_migrations VALUES (17, 1);
                 CREATE TABLE migration_probe (value TEXT NOT NULL);
                 INSERT INTO migration_probe VALUES ('preserved');
                 PRAGMA user_version = 9;",
            )
            .expect("prepare database one chat migration behind");
        drop(initial);

        let storage = AppStorage::open_at(directory.0.clone()).expect("open storage");

        assert!(
            storage
                .prepare_chat_schema_migration(10)
                .expect("prepare chat migration")
        );
        assert!(
            !storage
                .prepare_chat_schema_migration(10)
                .expect("avoid duplicate startup backup")
        );

        let backups = fs::read_dir(database_directory.join("backups"))
            .expect("list migration backups")
            .collect::<Result<Vec<_>, _>>()
            .expect("read migration backup entries");
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(backups[0].path()).expect("open verified backup");
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
            let path =
                std::env::temp_dir().join(format!("openchat-storage-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
