use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags};

mod schema;
pub(crate) mod user_questions;
use schema::{INITIAL_SCHEMA_VERSION, SCHEMA_VERSION, initialize_schema};

pub struct AppStorage {
    root: PathBuf,
    database_path: PathBuf,
    schema_version: AtomicI64,
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
        let connection = Connection::open_with_flags(
            &database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let schema_version = initialize_schema(&connection, INITIAL_SCHEMA_VERSION)?;
        drop(connection);

        Ok(Self {
            root,
            database_path,
            schema_version: AtomicI64::new(schema_version),
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

    fn append_diagnostic(
        &self,
        component: &'static str,
        event: &'static str,
        write_fields: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
    ) -> io::Result<()> {
        let _guard = self
            .diagnostics_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis();
        let log_path = self.root.join("logs").join("openchat-service.log");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        write!(
            file,
            "timestamp_unix_ms={timestamp} component={component} event={event}"
        )?;
        write_fields(&mut file)?;
        writeln!(file)?;
        file.flush()
    }

    pub fn initialize_backend_schema(&self) -> rusqlite::Result<i64> {
        let connection = self.connect()?;
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

    pub fn connect(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open_with_flags(
            &self.database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
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
