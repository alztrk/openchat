//! SQLite persistence, delta recording transactions, and legacy manifest migration.

use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::blobs::{
    cleanup_pending_snapshot_blobs, ensure_snapshot_blob, lock_file_changes, read_snapshot_blob,
};
use super::diff::diff_stats;
use super::types::*;
use super::workspace::{
    directory_identity, ensure_child_directory, open_workspace_directory, read_bounded_file,
    workspace_directory_matches_path,
};
use crate::storage::{AppStorage, StorageConnection, StorageRow, StorageTransaction};

/// Compares snapshots from the same root and returns paths whose contents differ.
pub(crate) fn workspace_deltas(
    before: &WorkspaceSnapshot,
    after: &WorkspaceSnapshot,
) -> Result<Vec<FileChangeDelta>, TrackingError> {
    if before.root != after.root || before.directory_identity != after.directory_identity {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    let paths = before
        .files
        .keys()
        .chain(after.files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    Ok(paths
        .into_iter()
        .filter_map(|path| {
            let old = before.files.get(&path);
            let new = after.files.get(&path);
            let changed = old.map(|state| &state.hash) != new.map(|state| &state.hash);
            changed.then(|| FileChangeDelta {
                root_identity: before.directory_identity,
                path,
                before: old.cloned(),
                after: new.cloned(),
            })
        })
        .collect())
}

/// Persists tracked deltas and their baseline blobs for one conversation.
#[cfg(test)]
pub(crate) fn record_deltas(
    storage: &AppStorage,
    conversation_id: &str,
    root: &Path,
    deltas: Vec<FileChangeDelta>,
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    if deltas.is_empty() {
        return Ok(Vec::new());
    }
    let _guard = lock_file_changes()?;
    record_deltas_locked(storage, conversation_id, root, deltas)
}

/// Persists deltas while the caller holds the file-change lock.
#[cfg(test)]
pub(crate) fn record_deltas_locked(
    storage: &AppStorage,
    conversation_id: &str,
    root: &Path,
    deltas: Vec<FileChangeDelta>,
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    record_deltas_batch_locked(storage, conversation_id, &[(root.to_path_buf(), deltas)])
}

/// Persists deltas from multiple workspace roots under one file-change lock.
pub(crate) fn record_deltas_batch(
    storage: &AppStorage,
    conversation_id: &str,
    grouped_deltas: &[(PathBuf, Vec<FileChangeDelta>)],
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    let _guard = lock_file_changes()?;
    record_deltas_batch_locked(storage, conversation_id, grouped_deltas)
}

/// Persists deltas from multiple workspace roots in one manifest transaction.
pub(crate) fn record_deltas_batch_locked(
    storage: &AppStorage,
    conversation_id: &str,
    grouped_deltas: &[(PathBuf, Vec<FileChangeDelta>)],
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    let grouped_deltas = grouped_deltas
        .iter()
        .filter(|(_, deltas)| !deltas.is_empty())
        .collect::<Vec<_>>();
    if grouped_deltas.is_empty() {
        return Ok(Vec::new());
    }
    validate_conversation_id(conversation_id)?;
    let mut workspaces = Vec::with_capacity(grouped_deltas.len());
    for (requested_root, deltas) in grouped_deltas {
        let (directory, root) = open_workspace_directory(requested_root)?;
        let root_identity =
            directory_identity(&directory).map_err(|_| TrackingError::WorkspaceUnavailable)?;
        root_identity.storage_value()?;
        if deltas
            .iter()
            .any(|delta| delta.root_identity != root_identity)
        {
            return Err(TrackingError::WorkspaceUnavailable);
        }
        for delta in deltas {
            safe_relative_path(&delta.path)?;
        }
        workspaces.push((root, directory, root_identity, deltas));
    }
    cleanup_pending_snapshot_blobs(storage)?;
    migrate_legacy_conversation(storage, conversation_id)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let mut entries = load_changes(&connection, conversation_id)?;
    drop(connection);
    let delta_count = workspaces
        .iter()
        .map(|(_, _, _, deltas)| deltas.len())
        .sum();
    let mut changed_summaries = Vec::with_capacity(delta_count);
    for (root, _, root_identity, deltas) in &workspaces {
        let stored_root_identity = root_identity.storage_value()?;
        for delta in deltas.iter() {
            let relative = safe_relative_path(&delta.path)?;
            let normalized = relative.to_string_lossy().replace('\\', "/");
            let id = change_id(root, &normalized);
            let before_hash = delta.before.as_ref().map(|state| state.hash.as_str());
            let after_hash = delta.after.as_ref().map(|state| state.hash.as_str());
            if before_hash == after_hash {
                continue;
            }
            let existing_index = entries.iter().position(|entry| entry.id == id);
            let reusable_index = existing_index.filter(|index| {
                entries[*index].root_identity.as_deref() == Some(stored_root_identity.as_str())
            });
            let (baseline_exists, baseline_hash, baseline_blob_hash, was_conflicted) =
                if let Some(index) = reusable_index
                    && !is_entry_approved(&entries[index])
                {
                    let existing = &entries[index];
                    let expected = existing.expected_hash.as_deref();
                    let baseline_is_current = if existing.baseline_exists {
                        before_hash == existing.baseline_hash.as_deref()
                    } else {
                        delta.before.is_none()
                    };
                    let matches_expected = before_hash == expected
                        && delta.before.is_some() == existing.expected_exists;
                    (
                        existing.baseline_exists,
                        existing.baseline_hash.clone(),
                        existing.baseline_blob_hash.clone(),
                        existing.status == "conflict"
                            || (!matches_expected && !baseline_is_current),
                    )
                } else {
                    let baseline_exists = delta.before.is_some();
                    let baseline_hash = delta.before.as_ref().map(|state| state.hash.clone());
                    let baseline_blob_hash = match delta
                        .before
                        .as_ref()
                        .and_then(|state| state.content.as_deref())
                    {
                        Some(content) => Some(ensure_snapshot_blob(storage, content)?),
                        None => None,
                    };
                    (baseline_exists, baseline_hash, baseline_blob_hash, false)
                };
            let baseline = if baseline_exists {
                match baseline_blob_hash.as_deref() {
                    Some(hash) => Some(read_snapshot_blob(storage.root(), hash)?),
                    None => None,
                }
            } else {
                Some(Vec::new())
            };
            let current = delta
                .after
                .as_ref()
                .and_then(|state| state.content.clone())
                .or_else(|| delta.after.is_none().then(Vec::new));
            let (added_lines, removed_lines, is_binary, diff_available) =
                match (baseline.as_deref(), current.as_deref()) {
                    (Some(old), Some(new)) => diff_stats(old, new),
                    _ => (None, None, false, false),
                };
            let status = if was_conflicted {
                "conflict".to_owned()
            } else if baseline_exists == delta.after.is_some()
                && baseline_hash.as_deref() == after_hash
            {
                "reverted".to_owned()
            } else {
                "active".to_owned()
            };
            let entry = ManifestEntry {
                id: id.clone(),
                root: root.to_string_lossy().into_owned(),
                root_identity: Some(stored_root_identity.clone()),
                path: normalized,
                baseline_exists,
                baseline_hash,
                baseline_blob_hash,
                expected_exists: delta.after.is_some(),
                expected_hash: delta.after.as_ref().map(|state| state.hash.clone()),
                added_lines,
                removed_lines,
                is_binary,
                diff_available,
                status,
            };
            let summary = summary_for_entry(storage.root(), &entry, false);
            if let Some(index) = existing_index {
                entries[index] = entry;
            } else {
                entries.push(entry);
            }
            changed_summaries.push(summary);
        }
    }
    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    store_changes(&transaction, conversation_id, &entries)?;
    for (root, directory, _, _) in &workspaces {
        if !workspace_directory_matches_path(root, directory)? {
            return Err(TrackingError::WorkspaceUnavailable);
        }
    }
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    Ok(changed_summaries)
}

/// Lists tracked file changes after importing any eligible legacy records.
pub(crate) fn list_changes(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    validate_conversation_id(conversation_id)?;
    let _guard = lock_file_changes()?;
    cleanup_pending_snapshot_blobs(storage)?;
    migrate_legacy_conversation(storage, conversation_id)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    Ok(load_changes(&connection, conversation_id)?
        .iter()
        .map(|entry| summary_for_entry(storage.root(), entry, true))
        .collect())
}

pub(crate) fn load_changes(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
) -> Result<Vec<ManifestEntry>, TrackingError> {
    let mut statement = connection
        .prepare(
            "SELECT id, root, root_identity, path, baseline_exists, baseline_hash, baseline_blob_hash,
                    expected_exists, expected_hash, added_lines, removed_lines,
                    is_binary, diff_available, status
             FROM conversation_file_changes
             WHERE conversation_id = ?1
             ORDER BY path, id",
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let rows = statement
        .query_map([conversation_id], manifest_entry_from_row)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| TrackingError::StorageUnavailable)
}

pub(crate) fn load_change(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    id: &str,
) -> Result<Option<ManifestEntry>, TrackingError> {
    connection
        .query_row(
            "SELECT id, root, root_identity, path, baseline_exists, baseline_hash, baseline_blob_hash,
                    expected_exists, expected_hash, added_lines, removed_lines,
                    is_binary, diff_available, status
             FROM conversation_file_changes
             WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, id],
            manifest_entry_from_row,
        )
        .optional()
        .map_err(|_| TrackingError::StorageUnavailable)
}

pub(crate) fn manifest_entry_from_row(row: &StorageRow) -> rusqlite::Result<ManifestEntry> {
    Ok(ManifestEntry {
        id: row.get(0)?,
        root: row.get(1)?,
        root_identity: row.get(2)?,
        path: row.get(3)?,
        baseline_exists: row.get(4)?,
        baseline_hash: row.get(5)?,
        baseline_blob_hash: row.get(6)?,
        expected_exists: row.get(7)?,
        expected_hash: row.get(8)?,
        added_lines: row
            .get::<_, Option<i64>>(9)?
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
        removed_lines: row
            .get::<_, Option<i64>>(10)?
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
        is_binary: row.get(11)?,
        diff_available: row.get(12)?,
        status: row.get(13)?,
    })
}

pub(crate) fn store_changes(
    transaction: &StorageTransaction<'_>,
    conversation_id: &str,
    entries: &[ManifestEntry],
) -> Result<(), TrackingError> {
    for entry in entries {
        store_change(transaction, conversation_id, entry)?;
    }
    Ok(())
}

pub(crate) fn store_change(
    transaction: &StorageTransaction<'_>,
    conversation_id: &str,
    entry: &ManifestEntry,
) -> Result<(), TrackingError> {
    if let Some(hash) = entry.baseline_blob_hash.as_deref() {
        transaction
            .execute(
                "UPDATE file_change_blobs SET state = 'ready' WHERE hash = ?1
                 AND state IN ('writing', 'ready')",
                [hash],
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
        if transaction
            .query_row(
                "SELECT state = 'ready' FROM file_change_blobs WHERE hash = ?1",
                [hash],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(|_| TrackingError::StorageUnavailable)?
            != Some(true)
        {
            return Err(TrackingError::StorageUnavailable);
        }
    }
    let added_lines = entry
        .added_lines
        .map(i64::try_from)
        .transpose()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let removed_lines = entry
        .removed_lines
        .map(i64::try_from)
        .transpose()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    transaction
        .execute(
            "INSERT INTO conversation_file_changes (
                conversation_id, id, root, root_identity, path, baseline_exists, baseline_hash,
                baseline_blob_hash, expected_exists, expected_hash, added_lines,
                removed_lines, is_binary, diff_available, status
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
             ) ON CONFLICT(conversation_id, id) DO UPDATE SET
                root = excluded.root,
                root_identity = excluded.root_identity,
                path = excluded.path,
                baseline_exists = excluded.baseline_exists,
                baseline_hash = excluded.baseline_hash,
                baseline_blob_hash = excluded.baseline_blob_hash,
                expected_exists = excluded.expected_exists,
                expected_hash = excluded.expected_hash,
                added_lines = excluded.added_lines,
                removed_lines = excluded.removed_lines,
                is_binary = excluded.is_binary,
                diff_available = excluded.diff_available,
                status = excluded.status",
            params![
                conversation_id,
                entry.id,
                entry.root,
                entry.root_identity,
                entry.path,
                entry.baseline_exists,
                entry.baseline_hash,
                entry.baseline_blob_hash,
                entry.expected_exists,
                entry.expected_hash,
                added_lines,
                removed_lines,
                entry.is_binary,
                entry.diff_available,
                entry.status,
            ],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    Ok(())
}

/// Deletes tracked file changes for one conversation and cleans unused blobs.
pub(crate) fn delete_conversation_changes(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<(), TrackingError> {
    validate_conversation_id(conversation_id)?;
    let _guard = lock_file_changes()?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute(
            "DELETE FROM conversation_file_changes WHERE conversation_id = ?1",
            [conversation_id],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute(
            "DELETE FROM conversation_file_checkpoints WHERE conversation_id = ?1",
            [conversation_id],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute(
            "DELETE FROM file_change_legacy_imports WHERE conversation_id = ?1",
            [conversation_id],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    drop(connection);
    remove_legacy_conversation_directory(storage.root(), conversation_id)?;
    cleanup_pending_snapshot_blobs(storage)
}

/// Deletes all change tracking metadata and cleans unused baseline blobs.
pub(crate) fn delete_all_changes(storage: &AppStorage) -> Result<(), TrackingError> {
    let _guard = lock_file_changes()?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute("DELETE FROM conversation_file_changes", [])
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute("DELETE FROM conversation_file_checkpoints", [])
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection
        .execute("DELETE FROM file_change_legacy_imports", [])
        .map_err(|_| TrackingError::StorageUnavailable)?;
    drop(connection);
    let root = storage
        .root()
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if let Some(directory) = ensure_child_directory(&root, LEGACY_FILE_CHANGE_DIRECTORY, false)? {
        fs::remove_dir_all(directory).map_err(|_| TrackingError::StorageUnavailable)?;
    }
    cleanup_pending_snapshot_blobs(storage)
}

pub(crate) fn migrate_legacy_conversation(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<(), TrackingError> {
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let conversation_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let directory = legacy_conversation_directory(storage.root(), conversation_id, false)?;
    if !conversation_exists {
        if directory.is_some() {
            remove_legacy_conversation_directory(storage.root(), conversation_id)?;
        }
        return Ok(());
    }
    let imported = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM file_change_legacy_imports WHERE conversation_id = ?1)",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    drop(connection);
    if imported {
        if directory.is_some() {
            remove_legacy_conversation_directory(storage.root(), conversation_id)?;
        }
        return Ok(());
    }
    let Some(directory) = directory else {
        return Ok(());
    };
    let manifest = read_manifest(&directory)?;
    let mut imported_entries = Vec::with_capacity(manifest.files.len());
    for mut entry in manifest.files {
        validate_change_id(&entry.id)?;
        let root = PathBuf::from(&entry.root)
            .canonicalize()
            .map_err(|_| TrackingError::StorageUnavailable)?;
        let path = safe_relative_path(&entry.path)?;
        let normalized = path.to_string_lossy().replace('\\', "/");
        if entry.id != change_id(&root, &normalized)
            || (entry.baseline_exists && entry.baseline_hash.is_none())
            || (entry.expected_exists && entry.expected_hash.is_none())
            || !matches!(entry.status.as_str(), "active" | "reverted" | "conflict")
        {
            return Err(TrackingError::StorageUnavailable);
        }
        if let Some(backup_name) = entry.baseline_blob_hash.take() {
            if !entry.baseline_exists || backup_name != format!("{}.before", entry.id) {
                return Err(TrackingError::StorageUnavailable);
            }
            let content = read_legacy_backup(&directory, &backup_name)?;
            if entry.baseline_hash.as_deref() != Some(sha256(&content).as_str()) {
                return Err(TrackingError::StorageUnavailable);
            }
            entry.baseline_blob_hash = Some(ensure_snapshot_blob(storage, &content)?);
        }
        entry.root = root.to_string_lossy().into_owned();
        entry.path = normalized;
        entry.diff_available &= entry.root_identity.is_some();
        imported_entries.push(entry);
    }
    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    for entry in &imported_entries {
        store_change(&transaction, conversation_id, entry)?;
    }
    transaction
        .execute(
            "INSERT INTO file_change_legacy_imports (conversation_id, imported_at_unix_ms)
             VALUES (?1, ?2)",
            params![conversation_id, unix_time_millis()?],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    remove_legacy_conversation_directory(storage.root(), conversation_id)
}

fn read_legacy_backup(directory: &Path, name: &str) -> Result<Vec<u8>, TrackingError> {
    let backups = ensure_child_directory(directory, "backups", false)?
        .ok_or(TrackingError::StorageUnavailable)?;
    let path = backups.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| TrackingError::StorageUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrackingError::StorageUnavailable);
    }
    read_bounded_file(&path, MAX_SNAPSHOT_FILE_BYTES as usize)
        .map_err(|_| TrackingError::StorageUnavailable)?
        .ok_or(TrackingError::StorageUnavailable)
}

fn legacy_conversation_directory(
    data_root: &Path,
    conversation_id: &str,
    create: bool,
) -> Result<Option<PathBuf>, TrackingError> {
    let root = data_root
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let Some(base) = ensure_child_directory(&root, LEGACY_FILE_CHANGE_DIRECTORY, create)? else {
        return Ok(None);
    };
    ensure_child_directory(&base, &sha256(conversation_id.as_bytes()), create)
}

fn remove_legacy_conversation_directory(
    data_root: &Path,
    conversation_id: &str,
) -> Result<(), TrackingError> {
    if let Some(directory) = legacy_conversation_directory(data_root, conversation_id, false)? {
        fs::remove_dir_all(directory).map_err(|_| TrackingError::StorageUnavailable)?;
    }
    remove_legacy_root_if_empty(data_root)?;
    Ok(())
}

fn remove_legacy_root_if_empty(data_root: &Path) -> Result<(), TrackingError> {
    let root = data_root
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if let Some(directory) = ensure_child_directory(&root, LEGACY_FILE_CHANGE_DIRECTORY, false)? {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::AlreadyExists
                        | io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(_) => return Err(TrackingError::StorageUnavailable),
        }
    }
    Ok(())
}

fn read_manifest(directory: &Path) -> Result<Manifest, TrackingError> {
    let path = directory.join("manifest.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(TrackingError::StorageUnavailable)
        }
        Ok(_) => {
            let bytes = read_bounded_file(&path, MAX_LEGACY_MANIFEST_BYTES)
                .map_err(|_| TrackingError::StorageUnavailable)?
                .ok_or(TrackingError::StorageUnavailable)?;
            let manifest: Manifest =
                serde_json::from_slice(&bytes).map_err(|_| TrackingError::StorageUnavailable)?;
            if manifest.version != MANIFEST_VERSION {
                return Err(TrackingError::StorageUnavailable);
            }
            Ok(manifest)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let backups = ensure_child_directory(directory, "backups", false)?;
            if let Some(backups) = backups {
                let mut entries =
                    fs::read_dir(backups).map_err(|_| TrackingError::StorageUnavailable)?;
                if entries
                    .next()
                    .transpose()
                    .map_err(|_| TrackingError::StorageUnavailable)?
                    .is_some()
                {
                    return Err(TrackingError::StorageUnavailable);
                }
            }
            Ok(Manifest {
                version: MANIFEST_VERSION,
                files: Vec::new(),
            })
        }
        Err(_) => Err(TrackingError::StorageUnavailable),
    }
}
