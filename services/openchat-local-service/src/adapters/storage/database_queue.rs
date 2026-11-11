//! Serializes primary SQLite operations through a bounded FIFO queue and provides
//! a pooled secondary connection mechanism for concurrent read-only WAL operations.

use std::{
    collections::VecDeque,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use rusqlite::{Connection, ffi};

use super::{connection::StorageConnection, database_key::DatabaseKey};

#[cfg(windows)]
use super::broker_connection::BrokerConnection;

const MAX_QUEUED_OPERATIONS: usize = 64;
const DATABASE_QUEUE_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_READER_POOL_SIZE: usize = 4;

#[derive(Clone)]
pub(crate) struct DatabaseQueueHandle {
    backend: Arc<DatabaseQueueBackend>,
}

enum DatabaseQueueBackend {
    Local(Arc<DatabaseQueue>),
    #[cfg(windows)]
    Remote,
}

struct QueueState {
    next_ticket: u64,
    waiting_tickets: VecDeque<u64>,
    active_ticket: Option<u64>,
    active_since: Option<Instant>,
}

/// Owns the primary database connection, its FIFO operation queue, and
/// a pooled set of secondary read-only connections for WAL concurrency.
pub(super) struct DatabaseQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
    connection: Mutex<Connection>,
    key: Arc<DatabaseKey>,
    database_path: Option<PathBuf>,
    reader_pool: Mutex<Vec<Connection>>,
}

/// A database connection lease that owns its FIFO queue slot until dropped.
pub struct QueuedConnection<'a> {
    connection: MutexGuard<'a, Connection>,
    // Release the SQLite connection before allowing the next FIFO ticket to proceed.
    _lease: DatabaseQueueLease<'a>,
    _not_send: PhantomData<Rc<()>>,
}

/// A pooled secondary read-only connection that operates concurrently alongside
/// active write transactions in SQLite WAL mode.
pub struct PooledReaderConnection<'a> {
    queue: &'a DatabaseQueue,
    connection: Option<Connection>,
    _not_send: PhantomData<Rc<()>>,
}

struct DatabaseQueueLease<'a> {
    queue: &'a DatabaseQueue,
    ticket: u64,
}

impl DatabaseQueue {
    #[allow(dead_code)]
    pub(super) fn new(connection: Connection, key: Arc<DatabaseKey>) -> Self {
        Self::new_with_path(connection, key, None)
    }

    pub(super) fn new_with_path(
        connection: Connection,
        key: Arc<DatabaseKey>,
        database_path: Option<PathBuf>,
    ) -> Self {
        Self {
            state: Mutex::new(QueueState {
                next_ticket: 0,
                waiting_tickets: VecDeque::new(),
                active_ticket: None,
                active_since: None,
            }),
            ready: Condvar::new(),
            connection: Mutex::new(connection),
            key,
            database_path,
            reader_pool: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn has_reader_pool(&self) -> bool {
        self.database_path.is_some()
    }

    pub(super) fn connect_read(&self) -> rusqlite::Result<PooledReaderConnection<'_>> {
        let path = match &self.database_path {
            Some(path) => path,
            None => return Err(rusqlite::Error::InvalidQuery),
        };

        let pooled_connection = {
            let mut pool = self.reader_pool.lock().unwrap_or_else(|p| p.into_inner());
            pool.pop()
        };

        let connection = match pooled_connection {
            Some(conn) => conn,
            None => open_reader_connection(path, &self.key)?,
        };

        Ok(PooledReaderConnection::new(self, connection))
    }

    fn return_reader(&self, connection: Connection) {
        let mut pool = self.reader_pool.lock().unwrap_or_else(|p| p.into_inner());
        if pool.len() < MAX_READER_POOL_SIZE {
            pool.push(connection);
        }
    }

    pub(super) fn clear_readers(&self) {
        let mut pool = self.reader_pool.lock().unwrap_or_else(|p| p.into_inner());
        pool.clear();
    }

    fn connect(&self) -> rusqlite::Result<QueuedConnection<'_>> {
        let ticket = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let active_operation_count =
                state.waiting_tickets.len() + state.active_ticket.is_some() as usize;
            if active_operation_count >= MAX_QUEUED_OPERATIONS {
                return Err(queue_full_error());
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
                    break ticket;
                }

                let remaining = DATABASE_QUEUE_WAIT_TIMEOUT.saturating_sub(waiting_since.elapsed());
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
                    let queue_depth =
                        state.waiting_tickets.len() + state.active_ticket.is_some() as usize;
                    self.ready.notify_all();
                    drop(state);
                    eprintln!(
                        "openchat-storage-broker-diagnostic: local_database_queue_wait_timed_out ticket={ticket} active_ticket={active_ticket:?} queue_head_ticket={queue_head_ticket:?} queue_depth={queue_depth} wait_ms={wait_ms} active_lease_ms={active_lease_ms}"
                    );
                    return Err(queue_wait_timed_out_error());
                }

                let (next_state, _) = self
                    .ready
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state = next_state;
            }
        };

        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(QueuedConnection {
            connection,
            _lease: DatabaseQueueLease {
                queue: self,
                ticket,
            },
            _not_send: PhantomData,
        })
    }

    fn release(&self, ticket: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active_ticket == Some(ticket) {
            state.active_ticket = None;
            state.active_since = None;
        }
        self.ready.notify_all();
    }
}

impl DatabaseQueueHandle {
    #[allow(dead_code)]
    pub(crate) fn new(connection: Connection, key: Arc<DatabaseKey>) -> Self {
        Self::new_with_path(connection, key, None)
    }

    pub(crate) fn new_with_path(
        connection: Connection,
        key: Arc<DatabaseKey>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            backend: Arc::new(DatabaseQueueBackend::Local(Arc::new(
                DatabaseQueue::new_with_path(connection, key, path),
            ))),
        }
    }

    #[cfg(windows)]
    pub(crate) fn remote() -> Self {
        Self {
            backend: Arc::new(DatabaseQueueBackend::Remote),
        }
    }

    pub(crate) fn connect(&self) -> rusqlite::Result<StorageConnection<'_>> {
        match self.backend.as_ref() {
            DatabaseQueueBackend::Local(queue) => queue
                .connect()
                .map(|connection| StorageConnection::local(connection, &queue.key)),
            #[cfg(windows)]
            DatabaseQueueBackend::Remote => {
                BrokerConnection::connect().map(StorageConnection::remote)
            }
        }
    }

    pub(crate) fn connect_read(&self) -> rusqlite::Result<StorageConnection<'_>> {
        match self.backend.as_ref() {
            DatabaseQueueBackend::Local(queue) => {
                if queue.has_reader_pool() {
                    queue
                        .connect_read()
                        .map(|connection| StorageConnection::reader(connection, &queue.key))
                } else {
                    queue
                        .connect()
                        .map(|connection| StorageConnection::local(connection, &queue.key))
                }
            }
            #[cfg(windows)]
            DatabaseQueueBackend::Remote => {
                BrokerConnection::connect().map(StorageConnection::remote)
            }
        }
    }

    pub(crate) fn connect_local(&self) -> rusqlite::Result<QueuedConnection<'_>> {
        match self.backend.as_ref() {
            DatabaseQueueBackend::Local(queue) => queue.connect(),
            #[cfg(windows)]
            DatabaseQueueBackend::Remote => Err(rusqlite::Error::InvalidQuery),
        }
    }

    pub(crate) fn clear_readers(&self) {
        if let DatabaseQueueBackend::Local(queue) = self.backend.as_ref() {
            queue.clear_readers();
        }
    }

    pub(crate) fn is_remote(&self) -> bool {
        #[cfg(windows)]
        {
            matches!(self.backend.as_ref(), DatabaseQueueBackend::Remote)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

impl Deref for QueuedConnection<'_> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for QueuedConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl<'a> PooledReaderConnection<'a> {
    fn new(queue: &'a DatabaseQueue, connection: Connection) -> Self {
        Self {
            queue,
            connection: Some(connection),
            _not_send: PhantomData,
        }
    }
}

impl Deref for PooledReaderConnection<'_> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection
            .as_ref()
            .expect("reader connection exists until dropped")
    }
}

impl DerefMut for PooledReaderConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection
            .as_mut()
            .expect("reader connection exists until dropped")
    }
}

impl Drop for PooledReaderConnection<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            self.queue.return_reader(connection);
        }
    }
}

impl Drop for DatabaseQueueLease<'_> {
    fn drop(&mut self) {
        self.queue.release(self.ticket);
    }
}

fn open_reader_connection(path: &Path, key: &DatabaseKey) -> rusqlite::Result<Connection> {
    let connection = key.open(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    super::disable_trusted_schema(&connection)?;
    super::tool_index_redaction::register_sqlite_function(&connection)?;
    super::schema::register_search_normalization(&connection)?;
    connection.busy_timeout(super::SQLITE_BUSY_TIMEOUT)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -32000;
         PRAGMA mmap_size = 268435456;",
    )?;
    Ok(connection)
}

fn queue_full_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_BUSY),
        Some("database operation queue is full".to_owned()),
    )
}

fn queue_wait_timed_out_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_BUSY),
        Some("database operation queue wait timed out".to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Barrier},
        thread,
        time::{Duration, Instant},
    };

    use rusqlite::Connection;
    use uuid::Uuid;

    use super::super::database_key::DatabaseKey;
    use super::DatabaseQueueHandle;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("openchat-queue-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn concurrent_wal_reader_executes_while_writer_is_held() {
        let directory = TestDirectory::new();
        let db_path = directory.0.join("test_wal.sqlite3");
        let key = Arc::new(DatabaseKey::generate().expect("generate test key"));

        let (primary, _) = key.open_or_migrate(&db_path).expect("open primary");
        primary
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE items (id TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO items VALUES ('k1', 'v1');",
            )
            .expect("init WAL table");

        let queue = DatabaseQueueHandle::new_with_path(primary, key, Some(db_path));

        // Acquire writer connection and intentionally keep it alive without dropping
        let writer = queue.connect().expect("acquire writer connection");

        // Concurrent connect_read must succeed and query WAL data without being blocked by writer!
        let reader = queue.connect_read().expect("acquire concurrent reader");
        let value: String = reader
            .query_row("SELECT value FROM items WHERE id = 'k1'", [], |row| {
                row.get(0)
            })
            .expect("read from WAL reader");
        assert_eq!(value, "v1");

        // Subsequent pooled read reuses reader and succeeds
        let reader2 = queue.connect_read().expect("acquire second reader");
        let count: i64 = reader2
            .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
            .expect("count rows from second reader");
        assert_eq!(count, 1);

        drop(reader);
        drop(reader2);
        drop(writer);
    }

    #[test]
    #[ignore = "manual local database queue contention benchmark"]
    fn benchmark_concurrent_local_database_queue_wait() {
        let key = Arc::new(DatabaseKey::isolated_test_key());
        let connection = Connection::open_in_memory().expect("open in-memory benchmark database");
        key.apply(&connection).expect("key benchmark database");
        let queue = DatabaseQueueHandle::new(connection, key);
        let worker_count = 12;
        let barrier = Arc::new(Barrier::new(worker_count + 1));
        let active_lease = queue
            .connect_local()
            .expect("hold the database queue before workers start");
        let workers = (0..worker_count)
            .map(|_| {
                let queue = queue.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let started = Instant::now();
                    let connection = queue
                        .connect_local()
                        .expect("acquire a queued benchmark connection");
                    let wait = started.elapsed();
                    thread::sleep(Duration::from_millis(2));
                    drop(connection);
                    wait
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        thread::sleep(Duration::from_millis(10));
        drop(active_lease);
        let mut waits = workers
            .into_iter()
            .map(|worker| worker.join().expect("queue benchmark worker completes"))
            .collect::<Vec<_>>();
        waits.sort_unstable();

        eprintln!(
            "local_database_queue workers={worker_count} hold_ms=2 p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            waits[(waits.len() - 1) * 50 / 100].as_secs_f64() * 1000.0,
            waits[(waits.len() - 1) * 95 / 100].as_secs_f64() * 1000.0,
            waits.last().map_or(0.0, |wait| wait.as_secs_f64() * 1000.0),
        );
    }
}
