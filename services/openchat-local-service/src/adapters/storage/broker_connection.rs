#![cfg(windows)]

//! `rusqlite`-shaped access to the isolated storage broker.
//!
//! The facade intentionally owns result data. A remote row cannot borrow a SQLite statement, so
//! callers that migrate to this module must use [`BrokerRow`] in their row callbacks.

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{
    Connection, Error, Params, Result, TransactionBehavior, ffi,
    types::{FromSql, FromSqlError, Value, ValueRef},
};
use sha2::{Digest, Sha256};

use super::{
    ipc_protocol::{
        LegacyImportStatus, MAX_STORAGE_COLUMNS, MAX_STORAGE_FRAME_BYTES, MAX_STORAGE_IMPORT_BYTES,
        MAX_STORAGE_PARAMETERS, MAX_STORAGE_ROWS, MAX_STORAGE_SNAPSHOT_BYTES,
        MAX_STORAGE_SQL_BYTES, MAX_STORAGE_TRANSFER_CHUNK_BYTES, StorageRequest, StorageResponse,
        StorageValue, TransactionBehavior as ProtocolTransactionBehavior,
    },
    remote_connection::{RemoteStorageConnection, RemoteStorageError},
};

const MAX_BROKER_OPERATIONS: usize = 64;
const REMOTE_DATABASE_QUEUE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
static BROKER_OPERATION_QUEUE: RemoteDatabaseQueue = RemoteDatabaseQueue {
    state: Mutex::new(RemoteQueueState {
        next_ticket: 0,
        waiting_tickets: VecDeque::new(),
        active_ticket: None,
        active_since: None,
    }),
    ready: Condvar::new(),
};

/// A serialized connection to the Windows storage broker.
#[derive(Clone)]
pub(crate) struct BrokerConnection {
    remote: RemoteStorageConnection,
    autocommit: Arc<AtomicBool>,
    _lease: Arc<RemoteDatabaseLease>,
}

struct RemoteDatabaseQueue {
    state: Mutex<RemoteQueueState>,
    ready: Condvar,
}

struct RemoteQueueState {
    next_ticket: u64,
    waiting_tickets: VecDeque<u64>,
    active_ticket: Option<u64>,
    active_since: Option<Instant>,
}

struct RemoteDatabaseLease {
    queue: &'static RemoteDatabaseQueue,
    ticket: u64,
}

impl BrokerConnection {
    /// Connects to the protected local storage broker.
    pub(crate) fn connect() -> Result<Self> {
        let lease = acquire_remote_database_lease()?;
        let remote = RemoteStorageConnection::connect().map_err(remote_error)?;
        Ok(Self::from_remote(remote, lease))
    }

    fn from_remote(remote: RemoteStorageConnection, lease: Arc<RemoteDatabaseLease>) -> Self {
        Self {
            remote,
            autocommit: Arc::new(AtomicBool::new(true)),
            _lease: lease,
        }
    }

    /// Executes one parameterized statement through the broker.
    pub(crate) fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        self.prepare(sql)?.execute(params)
    }

    /// Executes a query expected to return at least one row.
    pub(crate) fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&BrokerRow) -> Result<T>,
    {
        self.prepare(sql)?.query_row(params, f)
    }

    /// Executes a batch of SQL statements through the broker.
    pub(crate) fn execute_batch(&self, sql: &str) -> Result<()> {
        validate_sql(sql)?;
        let request = StorageRequest::ExecuteBatch {
            sql: sql.to_owned(),
        };
        ensure_request_frame(&request)?;
        match self.request(request)? {
            StorageResponse::Unit => {
                self.update_autocommit_from_batch(sql);
                Ok(())
            }
            _ => Err(invalid_response()),
        }
    }

    /// Creates a broker statement. Preparation validates only the bounded SQL contract; SQLite
    /// preparation itself remains owned by the service.
    pub(crate) fn prepare(&self, sql: &str) -> Result<BrokerStatement> {
        validate_sql(sql)?;
        Ok(BrokerStatement {
            connection: self.clone(),
            sql: sql.to_owned(),
        })
    }

    /// Begins a deferred transaction on the broker connection.
    pub(crate) fn transaction(&self) -> Result<BrokerTransaction> {
        self.transaction_with_behavior(TransactionBehavior::Deferred)
    }

    /// Begins a transaction with the requested SQLite behavior.
    pub(crate) fn transaction_with_behavior(
        &self,
        behavior: TransactionBehavior,
    ) -> Result<BrokerTransaction> {
        if !self.is_autocommit() {
            return Err(sqlite_failure(
                ffi::SQLITE_ERROR,
                "A storage transaction is already active.",
            ));
        }
        match self.request(StorageRequest::Begin {
            behavior: protocol_transaction_behavior(behavior)?,
        })? {
            StorageResponse::Unit => {
                self.autocommit.store(false, Ordering::Release);
                Ok(BrokerTransaction {
                    connection: self.clone(),
                    active: true,
                })
            }
            _ => Err(invalid_response()),
        }
    }

    /// Returns the rowid reported by the last successful insert on this broker connection.
    pub(crate) fn last_insert_rowid(&self) -> Result<i64> {
        match self.request(StorageRequest::LastInsertRowId) {
            Ok(StorageResponse::Integer { value }) => Ok(value),
            Ok(_) => Err(invalid_response()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn vault_read_map(&self) -> Result<crate::secrets::SecretMap> {
        match self.request(StorageRequest::VaultReadMap)? {
            StorageResponse::VaultMap { secrets } if secrets.is_within_limits() => Ok(secrets),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn vault_write_map(&self, secrets: crate::secrets::SecretMap) -> Result<()> {
        if !secrets.is_within_limits() {
            return Err(sqlite_failure(
                ffi::SQLITE_TOOBIG,
                "The provider secret map exceeds its size limit.",
            ));
        }
        match self.request(StorageRequest::VaultWriteMap { secrets })? {
            StorageResponse::VaultUpdated => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn vault_get(&self, key: &str) -> Result<Option<zeroize::Zeroizing<String>>> {
        match self.request(StorageRequest::VaultGet {
            key: key.to_owned(),
        })? {
            StorageResponse::VaultMap { secrets } if secrets.is_within_limits() => Ok(secrets
                .get(key)
                .map(|value| zeroize::Zeroizing::new(value.to_owned()))),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn vault_set(&self, key: &str, value: &str) -> Result<()> {
        let mut secrets = crate::secrets::SecretMap::default();
        secrets.insert(key.to_owned(), value.to_owned());
        match self.request(StorageRequest::VaultSet { secrets })? {
            StorageResponse::VaultUpdated => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn vault_delete(&self, key: &str) -> Result<()> {
        match self.request(StorageRequest::VaultDelete {
            key: key.to_owned(),
        })? {
            StorageResponse::VaultUpdated => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn vault_has(&self, key: &str) -> Result<bool> {
        match self.request(StorageRequest::VaultHas {
            key: key.to_owned(),
        })? {
            StorageResponse::VaultContains { exists } => Ok(exists),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn legacy_import_status(&self) -> Result<LegacyImportStatus> {
        match self.request(StorageRequest::LegacyImportStatus)? {
            StorageResponse::LegacyImportStatus { status } => Ok(status),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn commit_pending_profile_restore(&self) -> Result<()> {
        match self.request(StorageRequest::CommitPendingProfileRestore)? {
            StorageResponse::Unit => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn complete_empty_legacy_import(
        &self,
        secrets: crate::secrets::SecretMap,
        preserve_legacy_source: bool,
    ) -> Result<()> {
        match self.request(StorageRequest::CompleteEmptyLegacyImport {
            secrets,
            preserve_legacy_source,
        })? {
            StorageResponse::LegacyImportCompleted => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn import_legacy_snapshot(
        &self,
        path: &Path,
        secrets: crate::secrets::SecretMap,
    ) -> Result<()> {
        let mut source = File::open(path).map_err(|_| invalid_path())?;
        let total_bytes = source.metadata().map_err(|_| invalid_path())?.len();
        if total_bytes == 0 || total_bytes > MAX_STORAGE_IMPORT_BYTES {
            return Err(sqlite_failure(
                ffi::SQLITE_TOOBIG,
                "The local database exceeds the migration transfer limit.",
            ));
        }

        let mut hasher = Sha256::new();
        let mut hash_buffer = vec![0u8; MAX_STORAGE_TRANSFER_CHUNK_BYTES];
        loop {
            let bytes_read = source.read(&mut hash_buffer).map_err(|_| invalid_path())?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&hash_buffer[..bytes_read]);
        }
        let digest = format!("{:x}", hasher.finalize());
        source
            .seek(SeekFrom::Start(0))
            .map_err(|_| invalid_path())?;

        let import_id = match self.request(StorageRequest::BeginLegacyImport {
            total_bytes,
            sha256: digest,
        })? {
            StorageResponse::LegacyImportStarted { import_id } => import_id,
            _ => return Err(invalid_response()),
        };

        let mut offset = 0u64;
        let mut buffer = vec![0u8; MAX_STORAGE_TRANSFER_CHUNK_BYTES];
        while offset < total_bytes {
            let bytes_read = source.read(&mut buffer).map_err(|_| invalid_path())?;
            if bytes_read == 0 {
                return Err(invalid_response());
            }
            let data = STANDARD.encode(&buffer[..bytes_read]);
            let next_offset = match self.request(StorageRequest::WriteLegacyImportChunk {
                import_id: import_id.clone(),
                offset,
                data,
            })? {
                StorageResponse::LegacyImportChunkWritten { next_offset } => next_offset,
                _ => return Err(invalid_response()),
            };
            offset = offset
                .checked_add(bytes_read as u64)
                .filter(|expected| *expected == next_offset)
                .ok_or_else(invalid_response)?;
        }

        match self.request(StorageRequest::CommitLegacyImport { import_id, secrets })? {
            StorageResponse::LegacyImportCompleted => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) fn create_verified_snapshot(&self, destination: &Path) -> Result<()> {
        let parent = destination.parent().ok_or_else(invalid_path)?;
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid_path)?;
        let partial_path =
            parent.join(format!(".{name}.{}.partial", uuid::Uuid::new_v4().simple()));
        let mut partial = PartialSnapshot::new(partial_path.clone());
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)
            .map_err(|_| invalid_path())?;

        let (snapshot_id, total_bytes, expected_sha256) =
            match self.request(StorageRequest::BeginSnapshot)? {
                StorageResponse::SnapshotStarted {
                    snapshot_id,
                    total_bytes,
                    sha256,
                } if !snapshot_id.is_empty()
                    && snapshot_id.len() <= 64
                    && total_bytes > 0
                    && total_bytes <= MAX_STORAGE_SNAPSHOT_BYTES
                    && valid_sha256(&sha256) =>
                {
                    (snapshot_id, total_bytes, sha256)
                }
                _ => return Err(invalid_response()),
            };

        let transfer_result = (|| {
            let mut destination_file = OpenOptions::new()
                .write(true)
                .open(&partial_path)
                .map_err(|_| invalid_path())?;
            let mut hasher = Sha256::new();
            let mut offset = 0u64;
            loop {
                let (chunk_offset, data, complete) =
                    match self.request(StorageRequest::ReadSnapshotChunk {
                        snapshot_id: snapshot_id.clone(),
                        offset,
                    })? {
                        StorageResponse::SnapshotChunk {
                            offset,
                            data,
                            complete,
                        } => (offset, data, complete),
                        _ => return Err(invalid_response()),
                    };
                if chunk_offset != offset {
                    return Err(invalid_response());
                }
                let chunk = STANDARD.decode(data).map_err(|_| invalid_response())?;
                if chunk.is_empty() && !complete {
                    return Err(invalid_response());
                }
                if chunk.len() > MAX_STORAGE_TRANSFER_CHUNK_BYTES {
                    return Err(invalid_response());
                }
                offset = offset
                    .checked_add(chunk.len() as u64)
                    .filter(|next| *next <= total_bytes)
                    .ok_or_else(invalid_response)?;
                destination_file
                    .write_all(&chunk)
                    .map_err(|_| invalid_path())?;
                hasher.update(&chunk);
                if complete {
                    break;
                }
                if offset >= total_bytes {
                    return Err(invalid_response());
                }
            }
            let actual_sha256 = format!("{:x}", hasher.finalize());
            if offset != total_bytes || actual_sha256 != expected_sha256 {
                return Err(invalid_response());
            }
            destination_file.sync_all().map_err(|_| invalid_path())
        })();
        let end_result = self.end_snapshot(&snapshot_id);
        transfer_result?;
        end_result?;
        fs::rename(&partial_path, destination).map_err(|_| invalid_path())?;
        partial.disarm();
        Ok(())
    }

    fn end_snapshot(&self, snapshot_id: &str) -> Result<()> {
        match self.request(StorageRequest::EndSnapshot {
            snapshot_id: snapshot_id.to_owned(),
        })? {
            StorageResponse::SnapshotEnded => Ok(()),
            _ => Err(invalid_response()),
        }
    }

    /// Returns the locally tracked transaction state for this broker connection.
    pub(crate) fn is_autocommit(&self) -> bool {
        self.autocommit.load(Ordering::Acquire)
    }

    fn request(&self, request: StorageRequest) -> Result<StorageResponse> {
        self.remote.request(request).map_err(remote_error)
    }

    fn update_autocommit_from_batch(&self, sql: &str) {
        let mut state = self.is_autocommit();
        for keyword in sql_keywords(sql) {
            match keyword.as_str() {
                "BEGIN" => state = false,
                "COMMIT" | "END" | "ROLLBACK" => state = true,
                _ => {}
            }
        }
        self.autocommit.store(state, Ordering::Release);
    }

    fn finish_transaction(&self, commit: bool) -> Result<()> {
        let request = if commit {
            StorageRequest::Commit
        } else {
            StorageRequest::Rollback
        };
        match self.request(request)? {
            StorageResponse::Unit => {
                self.autocommit.store(true, Ordering::Release);
                Ok(())
            }
            _ => Err(invalid_response()),
        }
    }
}

fn acquire_remote_database_lease() -> Result<Arc<RemoteDatabaseLease>> {
    let mut state = BROKER_OPERATION_QUEUE
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let active_operation_count =
        state.waiting_tickets.len() + state.active_ticket.is_some() as usize;
    if active_operation_count >= MAX_BROKER_OPERATIONS {
        return Err(rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_BUSY),
            Some("database operation queue is full".to_owned()),
        ));
    }

    let ticket = state.next_ticket;
    state.next_ticket = state.next_ticket.wrapping_add(1);
    state.waiting_tickets.push_back(ticket);
    let waiting_since = Instant::now();

    loop {
        if state.active_ticket.is_none() && state.waiting_tickets.front() == Some(&ticket) {
            let _ = state.waiting_tickets.pop_front();
            state.active_ticket = Some(ticket);
            state.active_since = Some(Instant::now());
            return Ok(Arc::new(RemoteDatabaseLease {
                queue: &BROKER_OPERATION_QUEUE,
                ticket,
            }));
        }

        let remaining = REMOTE_DATABASE_QUEUE_WAIT_TIMEOUT.saturating_sub(waiting_since.elapsed());
        if remaining.is_zero() {
            state
                .waiting_tickets
                .retain(|queued_ticket| *queued_ticket != ticket);
            let active_ticket = state.active_ticket;
            let queue_head_ticket = state.waiting_tickets.front().copied();
            let active_lease_ms = state
                .active_since
                .map(|active_since| active_since.elapsed().as_millis())
                .unwrap_or_default();
            let wait_ms = waiting_since.elapsed().as_millis();
            let queue_depth = state.waiting_tickets.len() + state.active_ticket.is_some() as usize;
            BROKER_OPERATION_QUEUE.ready.notify_all();
            drop(state);
            eprintln!(
                "storage_database_queue_wait_timed_out ticket={ticket} active_ticket={active_ticket:?} queue_head_ticket={queue_head_ticket:?} queue_depth={queue_depth} wait_ms={wait_ms} active_lease_ms={active_lease_ms}"
            );
            return Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_BUSY),
                Some("database operation queue wait timed out".to_owned()),
            ));
        }

        let (next_state, _) = BROKER_OPERATION_QUEUE
            .ready
            .wait_timeout(state, remaining)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next_state;
    }
}

impl Drop for RemoteDatabaseLease {
    fn drop(&mut self) {
        let mut state = self
            .queue
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active_ticket == Some(self.ticket) {
            state.active_ticket = None;
            state.active_since = None;
        }
        self.queue.ready.notify_all();
    }
}

struct PartialSnapshot {
    path: std::path::PathBuf,
    active: bool,
}

impl PartialSnapshot {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for PartialSnapshot {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// A prepared remote statement.
pub(crate) struct BrokerStatement {
    connection: BrokerConnection,
    sql: String,
}

impl BrokerStatement {
    /// Executes the statement and returns its changed-row count.
    pub(crate) fn execute<P: Params>(&self, params: P) -> Result<usize> {
        let parameters = encode_parameters(&self.sql, params)?;
        let request = StorageRequest::Execute {
            sql: self.sql.clone(),
            parameters,
        };
        ensure_request_frame(&request)?;
        match self.connection.request(request)? {
            StorageResponse::Executed { changed_rows, .. } => Ok(changed_rows),
            _ => Err(invalid_response()),
        }
    }

    /// Executes the statement as an owned row stream.
    pub(crate) fn query<P: Params>(&self, params: P) -> Result<BrokerRows> {
        let parameters = encode_parameters(&self.sql, params)?;
        let request = StorageRequest::Query {
            sql: self.sql.clone(),
            parameters,
        };
        ensure_request_frame(&request)?;
        let response = self.connection.request(request)?;
        BrokerRows::from_response(response)
    }

    /// Executes the statement and maps its first row.
    pub(crate) fn query_row<T, P, F>(&self, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&BrokerRow) -> Result<T>,
    {
        let mut rows = self.query(params)?;
        let row = rows.next()?.ok_or(Error::QueryReturnedNoRows)?;
        f(row)
    }
}

/// An owned row stream returned by a broker query.
pub(crate) struct BrokerRows {
    columns: Arc<[String]>,
    rows: Vec<Vec<StorageValue>>,
    next_index: usize,
    current: Option<BrokerRow>,
}

impl BrokerRows {
    fn from_response(response: StorageResponse) -> Result<Self> {
        let StorageResponse::Rows { columns, rows } = response else {
            return Err(invalid_response());
        };
        if columns.len() > MAX_STORAGE_COLUMNS || rows.len() > MAX_STORAGE_ROWS {
            return Err(sqlite_failure(
                ffi::SQLITE_TOOBIG,
                "The storage query result exceeded its size limit.",
            ));
        }
        for row in &rows {
            if row.len() != columns.len() {
                return Err(invalid_response());
            }
            for value in row {
                storage_value_to_sql_value(value)?;
            }
        }
        Ok(Self {
            columns: Arc::from(columns.into_boxed_slice()),
            rows,
            next_index: 0,
            current: None,
        })
    }

    /// Returns the next owned row, borrowing it until the next call.
    pub(crate) fn next(&mut self) -> Result<Option<&BrokerRow>> {
        if self.next_index >= self.rows.len() {
            self.current = None;
            return Ok(None);
        }
        let values = std::mem::take(&mut self.rows[self.next_index]);
        self.next_index += 1;
        self.current = Some(BrokerRow {
            columns: Arc::clone(&self.columns),
            values,
        });
        Ok(self.current.as_ref())
    }
}

/// An owned result row with SQLite-compatible typed extraction.
pub(crate) struct BrokerRow {
    columns: Arc<[String]>,
    values: Vec<StorageValue>,
}

impl BrokerRow {
    /// Reads one column using a numeric index or case-insensitive column name.
    pub(crate) fn get<I, T>(&self, index: I) -> Result<T>
    where
        I: BrokerRowIndex,
        T: FromSql,
    {
        let index = index.resolve(&self.columns)?;
        let value = self
            .values
            .get(index)
            .ok_or(Error::InvalidColumnIndex(index))?;
        let sql_value = storage_value_to_sql_value(value)?;
        let data_type = sql_value.data_type();
        T::column_result(ValueRef::from(&sql_value)).map_err(|error| match error {
            FromSqlError::InvalidType => {
                Error::InvalidColumnType(index, self.columns[index].clone(), data_type)
            }
            FromSqlError::OutOfRange(value) => Error::IntegralValueOutOfRange(index, value),
            FromSqlError::Utf8Error(error) => Error::Utf8Error(index, error),
            other => Error::FromSqlConversionFailure(index, data_type, Box::new(other)),
        })
    }

    /// Returns the number of columns in the row.
    pub(crate) fn column_count(&self) -> usize {
        self.values.len()
    }

    /// Returns a column name by numeric index.
    pub(crate) fn column_name(&self, index: usize) -> Result<&str> {
        self.columns
            .get(index)
            .map(String::as_str)
            .ok_or(Error::InvalidColumnIndex(index))
    }
}

/// Resolves the two row index forms supported by `rusqlite`.
pub(crate) trait BrokerRowIndex {
    fn resolve(&self, columns: &[String]) -> Result<usize>;
}

impl BrokerRowIndex for usize {
    fn resolve(&self, columns: &[String]) -> Result<usize> {
        if *self < columns.len() {
            Ok(*self)
        } else {
            Err(Error::InvalidColumnIndex(*self))
        }
    }
}

impl BrokerRowIndex for &str {
    fn resolve(&self, columns: &[String]) -> Result<usize> {
        columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(self))
            .ok_or_else(|| Error::InvalidColumnName((*self).to_owned()))
    }
}

/// A transaction whose broker connection is rolled back when dropped while active.
pub(crate) struct BrokerTransaction {
    connection: BrokerConnection,
    active: bool,
}

impl BrokerTransaction {
    /// Executes one statement inside the transaction.
    pub(crate) fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        self.connection.prepare(sql)?.execute(params)
    }

    /// Executes a batch inside the transaction.
    pub(crate) fn execute_batch(&self, sql: &str) -> Result<()> {
        self.connection.execute_batch(sql)
    }

    /// Prepares a statement for use inside the transaction.
    pub(crate) fn prepare(&self, sql: &str) -> Result<BrokerStatement> {
        self.connection.prepare(sql)
    }

    /// Commits the transaction.
    pub(crate) fn commit(mut self) -> Result<()> {
        if self.active {
            self.connection.finish_transaction(true)?;
            self.active = false;
        }
        Ok(())
    }

    /// Rolls back the transaction.
    pub(crate) fn rollback(mut self) -> Result<()> {
        if self.active {
            self.connection.finish_transaction(false)?;
            self.active = false;
        }
        Ok(())
    }

    /// Returns the rowid reported by the last successful insert.
    pub(crate) fn last_insert_rowid(&self) -> Result<i64> {
        self.connection.last_insert_rowid()
    }
}

impl Drop for BrokerTransaction {
    fn drop(&mut self) {
        if self.active && self.connection.finish_transaction(false).is_ok() {
            self.active = false;
        }
    }
}

fn encode_parameters<P: Params>(sql: &str, params: P) -> Result<Vec<StorageValue>> {
    validate_sql(sql)?;
    let markers = parameter_markers(sql)?;
    let marker_sql = markers.join(", ");
    let discovery_sql = if marker_sql.is_empty() {
        "SELECT 1".to_owned()
    } else {
        format!("SELECT {marker_sql}")
    };

    let probe_connection = Connection::open_in_memory().map_err(|_| probe_error())?;
    let parameter_count = probe_connection
        .prepare(&discovery_sql)
        .map_err(|_| probe_error())?
        .parameter_count();
    if parameter_count > MAX_STORAGE_PARAMETERS {
        return Err(sqlite_failure(
            ffi::SQLITE_TOOBIG,
            "The storage request has too many parameters.",
        ));
    }
    validate_numbered_markers(&markers)?;

    if parameter_count == 0 {
        let mut statement = probe_connection
            .prepare("SELECT 1")
            .map_err(|_| probe_error())?;
        let mut rows = statement.query(params)?;
        if rows.next()?.is_none() {
            return Err(probe_error());
        }
        return Ok(Vec::new());
    }

    let index_sql = (1..=parameter_count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let expanded_sql = format!("SELECT {marker_sql}, {index_sql}");
    let mut statement = probe_connection
        .prepare(&expanded_sql)
        .map_err(|_| probe_error())?;
    if statement.parameter_count() != parameter_count {
        return Err(probe_error());
    }
    let mut rows = statement.query(params)?;
    let row = rows.next()?.ok_or_else(probe_error)?;
    let offset = markers.len();
    let mut values = Vec::with_capacity(parameter_count);
    for index in 0..parameter_count {
        let value = row
            .get::<_, Value>(offset + index)
            .map_err(|_| probe_error())?;
        values.push(StorageValue::try_from(value).map_err(|_| probe_error())?);
    }
    Ok(values)
}

fn parameter_markers(sql: &str) -> Result<Vec<String>> {
    let bytes = sql.as_bytes();
    let mut markers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[index];
                skip_quoted(bytes, &mut index, quote);
            }
            b'[' => skip_bracket_identifier(bytes, &mut index),
            b'-' if bytes.get(index + 1) == Some(&b'-') => skip_line_comment(bytes, &mut index),
            b'/' if bytes.get(index + 1) == Some(&b'*') => skip_block_comment(bytes, &mut index),
            b'?' => {
                let start = index;
                index += 1;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                markers.push(sql[start..index].to_owned());
            }
            b':' | b'@' | b'$' => {
                let start = index;
                if let Some(end) = scan_named_marker(bytes, index) {
                    index = end;
                    markers.push(sql[start..end].to_owned());
                } else {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    Ok(markers)
}

fn validate_numbered_markers(markers: &[String]) -> Result<()> {
    for marker in markers {
        let Some(number) = marker.strip_prefix('?').filter(|value| !value.is_empty()) else {
            continue;
        };
        let index = number.parse::<usize>().map_err(|_| probe_error())?;
        if index == 0 || index > MAX_STORAGE_PARAMETERS {
            return Err(sqlite_failure(
                ffi::SQLITE_TOOBIG,
                "The storage request has too many parameters.",
            ));
        }
    }
    Ok(())
}

fn scan_named_marker(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    if !bytes.get(index).is_some_and(is_parameter_name_start) {
        return None;
    }
    index += 1;
    while bytes.get(index).is_some_and(is_parameter_name_continue) {
        index += 1;
    }
    if bytes[start] == b'$' {
        loop {
            if bytes.get(index..index + 2) != Some(b"::")
                || !bytes.get(index + 2).is_some_and(is_parameter_name_start)
            {
                break;
            }
            index += 3;
            while bytes.get(index).is_some_and(is_parameter_name_continue) {
                index += 1;
            }
        }
        if bytes.get(index) == Some(&b'(') {
            index += 1;
            while index < bytes.len() && bytes[index] != b')' {
                index += 1;
            }
            if index < bytes.len() {
                index += 1;
            }
        }
    }
    Some(index)
}

fn is_parameter_name_start(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}

fn is_parameter_name_continue(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}

fn skip_quoted(bytes: &[u8], index: &mut usize, quote: u8) {
    *index += 1;
    while *index < bytes.len() {
        if bytes[*index] == quote {
            *index += 1;
            if bytes.get(*index) == Some(&quote) {
                *index += 1;
            } else {
                break;
            }
        } else {
            *index += 1;
        }
    }
}

fn skip_bracket_identifier(bytes: &[u8], index: &mut usize) {
    *index += 1;
    while *index < bytes.len() {
        if bytes[*index] == b']' {
            *index += 1;
            break;
        }
        *index += 1;
    }
}

fn skip_line_comment(bytes: &[u8], index: &mut usize) {
    *index += 2;
    while *index < bytes.len() && bytes[*index] != b'\n' {
        *index += 1;
    }
}

fn skip_block_comment(bytes: &[u8], index: &mut usize) {
    *index += 2;
    while *index + 1 < bytes.len() {
        if bytes[*index] == b'*' && bytes[*index + 1] == b'/' {
            *index += 2;
            return;
        }
        *index += 1;
    }
    *index = bytes.len();
}

fn sql_keywords(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut keywords = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[index];
                skip_quoted(bytes, &mut index, quote);
            }
            b'[' => skip_bracket_identifier(bytes, &mut index),
            b'-' if bytes.get(index + 1) == Some(&b'-') => skip_line_comment(bytes, &mut index),
            b'/' if bytes.get(index + 1) == Some(&b'*') => skip_block_comment(bytes, &mut index),
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                keywords.push(sql[start..index].to_ascii_uppercase());
            }
            _ => index += 1,
        }
    }
    keywords
}

fn storage_value_to_sql_value(value: &StorageValue) -> Result<Value> {
    Value::try_from(value.clone()).map_err(|_| probe_error())
}

fn validate_sql(sql: &str) -> Result<()> {
    if sql.is_empty() {
        return Err(Error::InvalidQuery);
    }
    if sql.len() > MAX_STORAGE_SQL_BYTES {
        return Err(sqlite_failure(
            ffi::SQLITE_TOOBIG,
            "The storage SQL exceeded its size limit.",
        ));
    }
    Ok(())
}

fn ensure_request_frame(request: &StorageRequest) -> Result<()> {
    let frame = serde_json::to_vec(request).map_err(|_| probe_error())?;
    if frame.len() > MAX_STORAGE_FRAME_BYTES {
        return Err(sqlite_failure(
            ffi::SQLITE_TOOBIG,
            "The storage request exceeded its frame limit.",
        ));
    }
    Ok(())
}

fn protocol_transaction_behavior(
    behavior: TransactionBehavior,
) -> Result<ProtocolTransactionBehavior> {
    match behavior {
        TransactionBehavior::Deferred => Ok(ProtocolTransactionBehavior::Deferred),
        TransactionBehavior::Immediate => Ok(ProtocolTransactionBehavior::Immediate),
        TransactionBehavior::Exclusive => Ok(ProtocolTransactionBehavior::Exclusive),
        _ => Err(sqlite_failure(
            ffi::SQLITE_MISUSE,
            "The requested transaction behavior is unsupported.",
        )),
    }
}

fn remote_error(error: RemoteStorageError) -> Error {
    match error {
        RemoteStorageError::Transport | RemoteStorageError::UnexpectedEndOfStream => {
            sqlite_failure(ffi::SQLITE_IOERR, "The storage broker is unavailable.")
        }
        RemoteStorageError::FrameTooLarge => sqlite_failure(
            ffi::SQLITE_TOOBIG,
            "The storage request exceeded its frame limit.",
        ),
        RemoteStorageError::InvalidFrame | RemoteStorageError::InvalidResponse => sqlite_failure(
            ffi::SQLITE_MISUSE,
            "The storage broker response is invalid.",
        ),
        RemoteStorageError::ProtocolVersionMismatch => sqlite_failure(
            ffi::SQLITE_MISMATCH,
            "The storage broker protocol is unsupported.",
        ),
        RemoteStorageError::TimedOut => sqlite_failure(
            ffi::SQLITE_IOERR,
            "The storage broker did not respond in time.",
        ),
        RemoteStorageError::ServerRejected { code } => {
            sqlite_failure(code, sqlite_code_message(code))
        }
        RemoteStorageError::Poisoned => sqlite_failure(
            ffi::SQLITE_MISUSE,
            "The storage broker connection is unavailable.",
        ),
    }
}

fn sqlite_code_message(code: i32) -> &'static str {
    match code {
        ffi::SQLITE_BUSY | ffi::SQLITE_LOCKED => "The database is busy.",
        ffi::SQLITE_CORRUPT | ffi::SQLITE_NOTADB => "The database failed an integrity check.",
        ffi::SQLITE_CANTOPEN | ffi::SQLITE_PERM | ffi::SQLITE_IOERR => {
            "The protected database is unavailable."
        }
        ffi::SQLITE_FULL => "The database storage is full.",
        ffi::SQLITE_TOOBIG => "The database request exceeded its size limit.",
        ffi::SQLITE_MISUSE => "The database request is invalid.",
        _ => "The database operation failed.",
    }
}

fn sqlite_failure(code: i32, message: &'static str) -> Error {
    Error::SqliteFailure(ffi::Error::new(code), Some(message.to_owned()))
}

fn invalid_response() -> Error {
    sqlite_failure(
        ffi::SQLITE_MISUSE,
        "The storage broker response is invalid.",
    )
}

fn invalid_path() -> Error {
    sqlite_failure(
        ffi::SQLITE_CANTOPEN,
        "The legacy database snapshot is unavailable.",
    )
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn probe_error() -> Error {
    sqlite_failure(
        ffi::SQLITE_MISUSE,
        "The storage parameter binding request is invalid.",
    )
}
