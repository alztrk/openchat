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

const SCHEMA_VERSION: i64 = 8;
const INITIAL_SCHEMA_VERSION: i64 = 2;

pub struct AppStorage {
    root: PathBuf,
    database_path: PathBuf,
    schema_version: AtomicI64,
    diagnostics_lock: Mutex<()>,
}

impl AppStorage {
    pub fn open() -> rusqlite::Result<Self> {
        let root = local_app_data_root()?;
        let database_directory = root.join("db");
        let logs_directory = root.join("logs");
        let cache_directory = root.join("cache");

        fs::create_dir_all(&database_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&logs_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        fs::create_dir_all(&cache_directory)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

        let database_path = database_directory.join("zihora.sqlite3");
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
        let log_path = self.root.join("logs").join("zihora-service.log");
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
        Ok(PathBuf::from(local_app_data).join("Zihora"))
    }

    #[cfg(not(windows))]
    {
        let _ = env::var_os("LOCALAPPDATA");
        Err(rusqlite::Error::InvalidPath(
            "The Zihora local service currently supports Windows only".into(),
        ))
    }
}

fn initialize_schema(connection: &Connection, target_version: i64) -> rusqlite::Result<i64> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS zihora_backend_migrations (
            version INTEGER PRIMARY KEY,
            applied_at_unix_ms INTEGER NOT NULL
        );",
    )?;

    let mut current_version = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM zihora_backend_migrations",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if current_version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if current_version >= target_version {
        return Ok(current_version);
    }

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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (1, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 1;
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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (2, ?1)",
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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (3, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 3;
    }

    if current_version < 4 {
        let transaction = connection.unchecked_transaction()?;
        transaction
            .execute_batch("ALTER TABLE chatgpt_models ADD COLUMN default_reasoning_level TEXT;")?;
        transaction.execute(
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (4, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (5, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
        current_version = 5;
    }

    if current_version < 6 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE chatgpt_models
                ADD COLUMN supports_reasoning_summary_parameter INTEGER NOT NULL DEFAULT 1
                CHECK (supports_reasoning_summary_parameter IN (0, 1));",
        )?;
        transaction.execute(
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (6, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (7, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
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
            "INSERT INTO zihora_backend_migrations (version, applied_at_unix_ms) VALUES (8, ?1)",
            [unix_time_millis()?],
        )?;
        transaction.commit()?;
    }

    Ok(SCHEMA_VERSION)
}

fn unix_time_millis() -> rusqlite::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}
