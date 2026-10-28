//! Content-addressable snapshot blob storage and file-change concurrency locking.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use rusqlite::{TransactionBehavior, params};

use super::types::*;
use super::workspace::{atomic_write, ensure_child_directory, read_bounded_file};
use crate::storage::AppStorage;

static FILE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static FILE_CHANGE_LOCK_WAIT_HOOK: OnceLock<
    Mutex<Option<(std::thread::ThreadId, std::sync::mpsc::SyncSender<()>)>>,
> = OnceLock::new();

/// Serializes file-change operations with synchronous built-in file mutations.
pub(crate) fn lock_file_changes() -> Result<std::sync::MutexGuard<'static, ()>, TrackingError> {
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
    guard.map_err(|_| TrackingError::StorageUnavailable)
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
    if content.len() > MAX_SNAPSHOT_FILE_BYTES as usize {
        return Err(TrackingError::SnapshotUnavailable);
    }
    let hash = sha256(content);
    let path = snapshot_blob_path(storage.root(), &hash, true)?
        .ok_or(TrackingError::StorageUnavailable)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute(
            "INSERT INTO file_change_blobs (hash, size_bytes, state, created_at_unix_ms)
             VALUES (?1, ?2, 'writing', ?3)
             ON CONFLICT(hash) DO UPDATE SET
                state = CASE WHEN file_change_blobs.state = 'pending'
                             THEN 'writing' ELSE file_change_blobs.state END,
                created_at_unix_ms = CASE WHEN file_change_blobs.state = 'pending'
                                          THEN excluded.created_at_unix_ms
                                          ELSE file_change_blobs.created_at_unix_ms END",
            params![
                hash,
                i64::try_from(content.len()).map_err(|_| TrackingError::StorageUnavailable)?,
                unix_time_millis()?,
            ],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let stored_size = connection
        .query_row(
            "SELECT size_bytes FROM file_change_blobs WHERE hash = ?1",
            [&hash],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if usize::try_from(stored_size).ok() != Some(content.len()) {
        return Err(TrackingError::StorageUnavailable);
    }
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
    Ok(hash)
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
    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let stale_writing_before = unix_time_millis()?.saturating_sub(5 * 60 * 1000);
    let hashes = {
        let mut statement = transaction
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
                 )",
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
        statement
            .query_map([stale_writing_before], |row| row.get::<_, String>(0))
            .map_err(|_| TrackingError::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TrackingError::StorageUnavailable)?
    };
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
