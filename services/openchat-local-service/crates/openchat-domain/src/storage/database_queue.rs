//! Serializes primary SQLite operations through a bounded FIFO queue.

use std::{
    collections::VecDeque,
    marker::PhantomData,
    ops::{Deref, DerefMut},
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

/// Owns the primary database connection and its FIFO operation queue.
pub(super) struct DatabaseQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
    connection: Mutex<Connection>,
    key: Arc<DatabaseKey>,
}

/// A database connection lease that owns its FIFO queue slot until dropped.
pub struct QueuedConnection<'a> {
    connection: MutexGuard<'a, Connection>,
    // Release the SQLite connection before allowing the next FIFO ticket to proceed.
    _lease: DatabaseQueueLease<'a>,
    _not_send: PhantomData<Rc<()>>,
}

struct DatabaseQueueLease<'a> {
    queue: &'a DatabaseQueue,
    ticket: u64,
}

impl DatabaseQueue {
    fn new(connection: Connection, key: Arc<DatabaseKey>) -> Self {
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
        }
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
    pub(crate) fn new(connection: Connection, key: Arc<DatabaseKey>) -> Self {
        Self {
            backend: Arc::new(DatabaseQueueBackend::Local(Arc::new(DatabaseQueue::new(
                connection, key,
            )))),
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

    pub(crate) fn connect_local(&self) -> rusqlite::Result<QueuedConnection<'_>> {
        match self.backend.as_ref() {
            DatabaseQueueBackend::Local(queue) => queue.connect(),
            #[cfg(windows)]
            DatabaseQueueBackend::Remote => Err(rusqlite::Error::InvalidQuery),
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

/// Holds an SQLite exclusive lock for the lifetime of the service-owned connection.
///
/// SQLite-aware processes outside this service cannot open the primary database while this
/// connection remains alive. The process must retain the same connection for that lock to hold.
pub(super) fn acquire_exclusive_lock(connection: &Connection) -> rusqlite::Result<()> {
    connection
        .pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(map_lock_error)?;
    let locking_mode = connection
        .pragma_query_value(None, "locking_mode", |row| row.get::<_, String>(0))
        .map_err(map_lock_error)?;
    if !locking_mode.eq_ignore_ascii_case("exclusive") {
        return Err(rusqlite::Error::InvalidQuery);
    }

    connection
        .execute_batch("BEGIN EXCLUSIVE; COMMIT;")
        .map_err(map_lock_error)
}

fn map_lock_error(error: rusqlite::Error) -> rusqlite::Error {
    if matches!(
        &error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    ) {
        return rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_BUSY),
            Some(
                "Could not acquire the primary database lock; close other applications using this database and restart OpenChat."
                    .to_owned(),
            ),
        );
    }

    error
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

impl Drop for DatabaseQueueLease<'_> {
    fn drop(&mut self) {
        self.queue.release(self.ticket);
    }
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
mod benchmark_tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::{Duration, Instant},
    };

    use rusqlite::Connection;

    use super::super::database_key::DatabaseKey;
    use super::DatabaseQueueHandle;

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
