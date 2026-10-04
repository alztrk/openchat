use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags};

const MAX_DIAGNOSTIC_LOG_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_ENTRY_BYTES: usize = 4096;

mod database_guard;
mod schema;
pub(crate) mod user_questions;
use schema::{INITIAL_SCHEMA_VERSION, SCHEMA_VERSION, initialize_schema};

pub struct AppStorage {
    root: PathBuf,
    database_path: PathBuf,
    database_had_data_on_open: bool,
    schema_version: AtomicI64,
    migration_backup_created: AtomicBool,
    migration_backup_lock: Mutex<()>,
    diagnostics_lock: Mutex<()>,
}

impl AppStorage {
    pub fn open() -> rusqlite::Result<Self> {
        let root = local_app_data_root()?;
        Self::open_at(root)
    }

    pub fn open_at(root: PathBuf) -> rusqlite::Result<Self> {
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
        connection.busy_timeout(Duration::from_secs(5))?;

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

        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let schema_version = initialize_schema(&connection, INITIAL_SCHEMA_VERSION)?;
        drop(connection);

        Ok(Self {
            root,
            database_path,
            database_had_data_on_open,
            schema_version: AtomicI64::new(schema_version),
            migration_backup_created: AtomicBool::new(migration_backup_created),
            migration_backup_lock: Mutex::new(()),
            diagnostics_lock: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn semantic_memory_cache_directory(&self) -> PathBuf {
        self.root.join("cache").join("semantic-memory")
    }

    pub fn schema_version(&self) -> i64 {
        self.schema_version.load(Ordering::Acquire)
    }

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

    pub fn initialize_backend_schema(&self) -> rusqlite::Result<i64> {
        let connection = self.connect()?;
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
        self.schema_version.store(schema_version, Ordering::Release);
        Ok(schema_version)
    }

    pub fn prepare_chat_schema_migration(&self, target_version: i64) -> rusqlite::Result<bool> {
        if !(1..=100).contains(&target_version) {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let connection = self.connect()?;
        database_guard::verify_integrity(&connection)?;
        let current_chat_version = database_guard::chat_schema_version(&connection)?;
        if current_chat_version > target_version {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let migration_pending = current_chat_version < target_version
            || database_guard::backend_schema_version(&connection)? < SCHEMA_VERSION;
        if !migration_pending {
            return Ok(false);
        }

        self.create_migration_backup_if_needed(&connection)
    }

    pub fn connect(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open_with_flags(
            &self.database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
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

    use super::{AppStorage, MAX_DIAGNOSTIC_LOG_BYTES, database_guard};

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
