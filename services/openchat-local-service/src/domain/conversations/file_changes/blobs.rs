//! Content-addressable snapshot blob storage and file-change concurrency locking.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

use rusqlite::{TransactionBehavior, params, params_from_iter, types::Value};

use super::types::*;
use super::workspace::{atomic_write, ensure_child_directory, read_bounded_file};
use crate::storage::AppStorage;

static FILE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const FILE_CHANGE_READ_LOCK_TIMEOUT: Duration = Duration::from_secs(4);
#[cfg(windows)]
const FILE_CHANGE_READ_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SNAPSHOT_CLEANUP_BATCH_SIZE: i64 = 32;
pub(crate) const SNAPSHOT_BLOB_BATCH_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const SNAPSHOT_BLOB_BATCH_MAX_COUNT: usize = 32;
#[cfg(windows)]
const FILE_CHANGE_MUTEX_NAME: &str = "Local\\OpenChat.FileChanges.Operation.v1";
#[cfg(test)]
static FILE_CHANGE_LOCK_WAIT_HOOK: OnceLock<
    Mutex<Option<(std::thread::ThreadId, std::sync::mpsc::SyncSender<()>)>>,
> = OnceLock::new();

pub(crate) struct FileChangeLockGuard {
    #[cfg(windows)]
    _shared_guard: SharedFileChangeLock,
    _process_guard: MutexGuard<'static, ()>,
}

#[cfg(windows)]
struct SharedFileChangeLock(HANDLE);

#[cfg(windows)]
impl SharedFileChangeLock {
    fn acquire(timeout_ms: u32) -> Result<Option<Self>, TrackingError> {
        let mut name = FILE_CHANGE_MUTEX_NAME.encode_utf16().collect::<Vec<_>>();
        name.push(0);
        // SAFETY: `name` is NUL-terminated UTF-16 and null attributes request the default DACL.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            // SAFETY: GetLastError has no preconditions and reads thread-local error state.
            let error = unsafe { GetLastError() };
            eprintln!("openchat-file-change-lock-diagnostic: create_failed win32_error={error}");
            return Err(TrackingError::StorageUnavailable);
        }

        // SAFETY: `handle` is a live mutex handle owned by this acquisition attempt.
        match unsafe { WaitForSingleObject(handle, timeout_ms) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(Self(handle))),
            WAIT_TIMEOUT => {
                // SAFETY: The mutex was not acquired, so only its owned handle is closed.
                unsafe { CloseHandle(handle) };
                Ok(None)
            }
            _ => {
                // SAFETY: GetLastError has no preconditions and reads thread-local error state.
                let error = unsafe { GetLastError() };
                // SAFETY: The mutex was not acquired, so only its owned handle is closed.
                unsafe { CloseHandle(handle) };
                eprintln!("openchat-file-change-lock-diagnostic: wait_failed win32_error={error}");
                Err(TrackingError::StorageUnavailable)
            }
        }
    }
}

#[cfg(windows)]
impl Drop for SharedFileChangeLock {
    fn drop(&mut self) {
        // SAFETY: Callers acquire and release this mutex on the same service thread.
        unsafe { ReleaseMutex(self.0) };
        // SAFETY: This guard owns the live handle returned by CreateMutexW.
        unsafe { CloseHandle(self.0) };
    }
}

/// Serializes file-change operations with synchronous built-in file mutations.
pub(crate) fn lock_file_changes() -> Result<FileChangeLockGuard, TrackingError> {
    let lock = FILE_CHANGE_LOCK.get_or_init(|| Mutex::new(()));
    #[cfg(test)]
    let guard = match lock.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::WouldBlock) => {
            notify_file_change_lock_wait_for_test();
            lock.lock()
        }
        Err(std::sync::TryLockError::Poisoned(error)) => Err(error),
    };
    #[cfg(not(test))]
    let guard = lock.lock();
    let process_guard = guard.map_err(|_| TrackingError::StorageUnavailable)?;
    #[cfg(windows)]
    {
        let shared_guard =
            SharedFileChangeLock::acquire(u32::MAX)?.ok_or(TrackingError::StorageUnavailable)?;
        Ok(FileChangeLockGuard {
            _shared_guard: shared_guard,
            _process_guard: process_guard,
        })
    }
    #[cfg(not(windows))]
    {
        Ok(FileChangeLockGuard {
            _process_guard: process_guard,
        })
    }
}

/// Waits briefly for a read-only file-change operation, then returns a retryable busy state.
pub(crate) fn try_lock_file_changes_for_read(
    cancellation: &tokio::sync::watch::Receiver<bool>,
) -> Result<FileChangeLockGuard, TrackingError> {
    let lock = FILE_CHANGE_LOCK.get_or_init(|| Mutex::new(()));
    let started = Instant::now();
    loop {
        if *cancellation.borrow() {
            return Err(TrackingError::Cancelled);
        }
        match lock.try_lock() {
            Ok(process_guard) => {
                #[cfg(windows)]
                {
                    let remaining = FILE_CHANGE_READ_LOCK_TIMEOUT
                        .checked_sub(started.elapsed())
                        .unwrap_or_default();
                    let wait_interval = remaining.min(FILE_CHANGE_READ_LOCK_POLL_INTERVAL);
                    let timeout_ms = u32::try_from(wait_interval.as_millis())
                        .map_err(|_| TrackingError::StorageUnavailable)?;
                    let Some(shared_guard) = SharedFileChangeLock::acquire(timeout_ms)? else {
                        if started.elapsed() < FILE_CHANGE_READ_LOCK_TIMEOUT {
                            continue;
                        }
                        return Err(TrackingError::StorageBusy("file_change_lock_busy"));
                    };
                    return Ok(FileChangeLockGuard {
                        _shared_guard: shared_guard,
                        _process_guard: process_guard,
                    });
                }
                #[cfg(not(windows))]
                {
                    return Ok(FileChangeLockGuard {
                        _process_guard: process_guard,
                    });
                }
            }
            Err(std::sync::TryLockError::WouldBlock)
                if started.elapsed() < FILE_CHANGE_READ_LOCK_TIMEOUT =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(TrackingError::StorageBusy("file_change_lock_busy"));
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(TrackingError::StorageUnavailable);
            }
        }
    }
}

#[cfg(test)]
fn notify_file_change_lock_wait_for_test() {
    let current_thread = std::thread::current().id();
    let hook = FILE_CHANGE_LOCK_WAIT_HOOK.get_or_init(|| Mutex::new(None));
    let mut hook = hook
        .lock()
        .expect("file-change lock test hook must remain available");
    if hook
        .as_ref()
        .is_some_and(|(expected_thread, _)| *expected_thread == current_thread)
        && let Some((_, sender)) = hook.take()
    {
        sender
            .send(())
            .expect("file-change lock wait test must remain connected");
    }
}

#[cfg(test)]
pub(crate) fn set_file_change_lock_wait_hook(
    thread_id: std::thread::ThreadId,
    sender: std::sync::mpsc::SyncSender<()>,
) {
    let hook = FILE_CHANGE_LOCK_WAIT_HOOK.get_or_init(|| Mutex::new(None));
    let mut hook = hook
        .lock()
        .expect("file-change lock test hook must remain available");
    *hook = Some((thread_id, sender));
}

pub(crate) fn ensure_snapshot_blob(
    storage: &AppStorage,
    content: &[u8],
) -> Result<String, TrackingError> {
    ensure_snapshot_blobs(storage, &[content])?
        .into_iter()
        .next()
        .ok_or(TrackingError::StorageUnavailable)
}

/// Registers and writes a bounded group of content-addressed snapshots with one database lease.
pub(crate) fn ensure_snapshot_blobs(
    storage: &AppStorage,
    contents: &[&[u8]],
) -> Result<Vec<String>, TrackingError> {
    if contents.is_empty() {
        return Ok(Vec::new());
    }

    let mut total_bytes = 0usize;
    let mut hashes = Vec::with_capacity(contents.len());
    let mut paths = Vec::with_capacity(contents.len());
    for content in contents {
        if content.len() > MAX_SNAPSHOT_FILE_BYTES as usize {
            return Err(TrackingError::SnapshotUnavailable);
        }
        total_bytes = total_bytes
            .checked_add(content.len())
            .ok_or(TrackingError::SnapshotUnavailable)?;
        if total_bytes > SNAPSHOT_BLOB_BATCH_MAX_BYTES
            || contents.len() > SNAPSHOT_BLOB_BATCH_MAX_COUNT
        {
            return Err(TrackingError::SnapshotUnavailable);
        }
        let hash = sha256(content);
        let path = snapshot_blob_path(storage.root(), &hash, true)?
            .ok_or(TrackingError::StorageUnavailable)?;
        hashes.push(hash);
        paths.push(path);
    }

    let placeholders = (0..contents.len())
        .map(|index| {
            let first = index * 3 + 1;
            format!("(?{first}, ?{}, 'writing', ?{})", first + 1, first + 2)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO file_change_blobs (hash, size_bytes, state, created_at_unix_ms)
         VALUES {placeholders}
         ON CONFLICT(hash) DO UPDATE SET
            state = CASE WHEN file_change_blobs.state = 'pending'
                         THEN 'writing' ELSE file_change_blobs.state END,
            created_at_unix_ms = CASE WHEN file_change_blobs.state = 'pending'
                                      THEN excluded.created_at_unix_ms
                                      ELSE file_change_blobs.created_at_unix_ms END
         RETURNING hash, size_bytes"
    );
    let created_at = unix_time_millis()?;
    let mut values = Vec::with_capacity(contents.len() * 3);
    for (hash, content) in hashes.iter().zip(contents) {
        values.push(Value::Text(hash.clone()));
        values.push(Value::Integer(
            i64::try_from(content.len()).map_err(|_| TrackingError::StorageUnavailable)?,
        ));
        values.push(Value::Integer(created_at));
    }

    let mut connection = storage.connect().map_err(map_storage_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_storage_error)?;
    let mut rows = transaction
        .query(&sql, params_from_iter(values.iter()))
        .map_err(map_storage_error)?;
    let mut stored_sizes = std::collections::HashMap::with_capacity(contents.len());
    while let Some(row) = rows.next().map_err(map_storage_error)? {
        stored_sizes.insert(
            row.get::<_, String>(0).map_err(map_storage_error)?,
            row.get::<_, i64>(1).map_err(map_storage_error)?,
        );
    }
    drop(rows);
    for (hash, content) in hashes.iter().zip(contents) {
        let stored_size = stored_sizes
            .get(hash)
            .copied()
            .ok_or(TrackingError::StorageUnavailable)?;
        if usize::try_from(stored_size).ok() != Some(content.len()) {
            return Err(TrackingError::StorageUnavailable);
        }
    }
    transaction.commit().map_err(map_storage_error)?;
    drop(connection);

    for ((content, hash), path) in contents.iter().zip(&hashes).zip(&paths) {
        write_snapshot_blob(path, content, hash)?;
    }
    Ok(hashes)
}

fn write_snapshot_blob(path: &Path, content: &[u8], hash: &str) -> Result<(), TrackingError> {
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(TrackingError::StorageUnavailable);
        }
        Ok(_) => {
            let stored = read_bounded_file(&path, MAX_SNAPSHOT_FILE_BYTES as usize)
                .map_err(|_| TrackingError::StorageUnavailable)?
                .ok_or(TrackingError::StorageUnavailable)?;
            if sha256(&stored) != hash {
                return Err(TrackingError::StorageUnavailable);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if atomic_write(&path, content).is_err() {
                let concurrent_write_is_valid = fs::symlink_metadata(&path).is_ok_and(|metadata| {
                    !metadata.file_type().is_symlink()
                        && metadata.is_file()
                        && read_bounded_file(&path, MAX_SNAPSHOT_FILE_BYTES as usize)
                            .is_ok_and(|bytes| bytes.is_some_and(|bytes| sha256(&bytes) == hash))
                });
                if !concurrent_write_is_valid {
                    return Err(TrackingError::StorageUnavailable);
                }
            }
        }
        Err(_) => return Err(TrackingError::StorageUnavailable),
    }
    Ok(())
}

pub(crate) fn read_snapshot_blob(data_root: &Path, hash: &str) -> Result<Vec<u8>, TrackingError> {
    validate_change_id(hash).map_err(|_| TrackingError::SnapshotUnavailable)?;
    let path =
        snapshot_blob_path(data_root, hash, false)?.ok_or(TrackingError::SnapshotUnavailable)?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| TrackingError::SnapshotUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrackingError::SnapshotUnavailable);
    }
    let content = read_bounded_file(&path, MAX_SNAPSHOT_FILE_BYTES as usize)
        .map_err(|_| TrackingError::SnapshotUnavailable)?
        .ok_or(TrackingError::SnapshotUnavailable)?;
    if sha256(&content) != hash {
        return Err(TrackingError::SnapshotUnavailable);
    }
    Ok(content)
}

pub(crate) fn snapshot_blob_exists(data_root: &Path, hash: &str) -> bool {
    let Ok(Some(path)) = snapshot_blob_path(data_root, hash, false) else {
        return false;
    };
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.len() <= MAX_SNAPSHOT_FILE_BYTES
    })
}

pub(crate) fn snapshot_blob_path(
    data_root: &Path,
    hash: &str,
    create: bool,
) -> Result<Option<PathBuf>, TrackingError> {
    validate_change_id(hash).map_err(|_| TrackingError::StorageUnavailable)?;
    let root = data_root
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let Some(attachments) = ensure_child_directory(&root, "attachments", create)? else {
        return Ok(None);
    };
    let Some(snapshots) = ensure_child_directory(&attachments, SNAPSHOT_DIRECTORY, create)? else {
        return Ok(None);
    };
    let shard_name = &hash[..SNAPSHOT_SHARD_HEX_LENGTH];
    let Some(shard) = ensure_child_directory(&snapshots, shard_name, create)? else {
        return Ok(None);
    };
    Ok(Some(shard.join(format!("{hash}.blob"))))
}

pub(crate) fn cleanup_pending_snapshot_blobs(storage: &AppStorage) -> Result<(), TrackingError> {
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let stale_writing_before = unix_time_millis()?.saturating_sub(5 * 60 * 1000);
    let hashes = {
        let mut statement = connection
            .prepare(
                "SELECT hash FROM file_change_blobs AS blobs
                 WHERE (state = 'pending' OR (state = 'writing' AND created_at_unix_ms < ?1))
                   AND NOT EXISTS (
                    SELECT 1 FROM conversation_file_changes AS changes
                    WHERE changes.baseline_blob_hash = blobs.hash
                 )
                   AND NOT EXISTS (
                    SELECT 1 FROM conversation_file_checkpoint_files AS files
                    WHERE files.before_blob_hash = blobs.hash OR files.after_blob_hash = blobs.hash
                  )
                 ORDER BY created_at_unix_ms, hash
                 LIMIT ?2",
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
        statement
            .query_map(
                params![stale_writing_before, SNAPSHOT_CLEANUP_BATCH_SIZE],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| TrackingError::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TrackingError::StorageUnavailable)?
    };
    drop(connection);

    let mut deleted_hashes = Vec::with_capacity(hashes.len());
    for hash in hashes {
        if let Some(path) = snapshot_blob_path(storage.root(), &hash, false)? {
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(TrackingError::StorageUnavailable);
                }
                Ok(_) => fs::remove_file(path).map_err(|_| TrackingError::StorageUnavailable)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(TrackingError::StorageUnavailable),
            }
        }
        deleted_hashes.push(hash);
    }
    if deleted_hashes.is_empty() {
        return Ok(());
    }

    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    for hash in deleted_hashes {
        transaction
            .execute(
                "DELETE FROM file_change_blobs WHERE hash = ?1
                 AND (state = 'pending' OR (state = 'writing' AND created_at_unix_ms < ?2))
                 AND NOT EXISTS (
                    SELECT 1 FROM conversation_file_changes
                    WHERE baseline_blob_hash = ?1
                 )
                 AND NOT EXISTS (
                    SELECT 1 FROM conversation_file_checkpoint_files
                    WHERE before_blob_hash = ?1 OR after_blob_hash = ?1
                 )",
                params![hash, stale_writing_before],
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
    }
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)
}

pub(crate) fn read_baseline(
    data_root: &Path,
    entry: &ManifestEntry,
) -> Result<Option<Vec<u8>>, TrackingError> {
    if !entry.baseline_exists {
        return Ok(Some(Vec::new()));
    }
    let Some(hash) = entry.baseline_blob_hash.as_deref() else {
        return Ok(None);
    };
    read_snapshot_blob(data_root, hash).map(Some)
}
