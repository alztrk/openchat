//! Serializes primary SQLite operations through a bounded FIFO queue.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

use rusqlite::{Connection, ffi};

use super::connection::StorageConnection;

#[cfg(windows)]
use super::broker_connection::BrokerConnection;

const MAX_QUEUED_OPERATIONS: u64 = 64;

#[derive(Clone)]
pub(crate) struct DatabaseQueueHandle {
    backend: Arc<DatabaseQueueBackend>,
}

enum DatabaseQueueBackend {
    Local(Arc<DatabaseQueue>),
    #[cfg(windows)]
    Remote,
}

#[derive(Default)]
struct QueueState {
    next_ticket: u64,
    serving_ticket: u64,
}

/// Owns the primary database connection and its FIFO operation queue.
pub(super) struct DatabaseQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
    connection: Mutex<Connection>,
}

/// A database connection lease that owns its FIFO queue slot until dropped.
pub struct QueuedConnection<'a> {
    connection: MutexGuard<'a, Connection>,
    queue: &'a DatabaseQueue,
    ticket: u64,
    _not_send: PhantomData<Rc<()>>,
}

impl DatabaseQueue {
    fn new(connection: Connection) -> Self {
        Self {
            state: Mutex::new(QueueState::default()),
            ready: Condvar::new(),
            connection: Mutex::new(connection),
        }
    }

    fn connect(&self) -> rusqlite::Result<QueuedConnection<'_>> {
        let ticket = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.next_ticket.wrapping_sub(state.serving_ticket) >= MAX_QUEUED_OPERATIONS {
                return Err(queue_full_error());
            }

            let ticket = state.next_ticket;
            state.next_ticket = state.next_ticket.wrapping_add(1);
            while state.serving_ticket != ticket {
                state = self
                    .ready
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            ticket
        };

        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(QueuedConnection {
            connection,
            queue: self,
            ticket,
            _not_send: PhantomData,
        })
    }

    fn release(&self, ticket: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.serving_ticket == ticket {
            state.serving_ticket = state.serving_ticket.wrapping_add(1);
        }
        self.ready.notify_all();
    }
}

impl DatabaseQueueHandle {
    pub(crate) fn new(connection: Connection) -> Self {
        Self {
            backend: Arc::new(DatabaseQueueBackend::Local(Arc::new(DatabaseQueue::new(
                connection,
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
            DatabaseQueueBackend::Local(queue) => queue.connect().map(StorageConnection::local),
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

impl Drop for QueuedConnection<'_> {
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
