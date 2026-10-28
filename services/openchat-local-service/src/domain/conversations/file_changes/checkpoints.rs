//! Persistent per-tool file checkpoints and conflict-safe workspace restoration.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::{Arc, MutexGuard},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;
use similar::TextDiff;
use uuid::Uuid;

use super::{
    FileChangeDelta, FileSnapshot, MAX_DIFF_LINES, MAX_DIFF_OUTPUT_BYTES, MAX_TEXT_DIFF_BYTES,
    TrackingError, as_text, atomic_write_workspace_file_from_directory,
    capture_file_from_directory, cleanup_pending_snapshot_blobs,
    ensure_no_symlink_path_from_directory, ensure_snapshot_blob, lock_file_changes,
    open_workspace_directory, read_snapshot_blob, remove_workspace_file_from_directory,
    safe_relative_path, sha256, unix_time_millis, validate_change_id, validate_conversation_id,
    workspace_directory_matches_path,
};
use crate::storage::{AppStorage, StorageConnection, StorageTransaction};

const MAX_CHECKPOINTS_PER_CONVERSATION: i64 = 2_000;
const MAX_CHECKPOINT_FILES_PER_EVENT: usize = 5_000;
const MAX_CHECKPOINT_FILES_PER_CONVERSATION: i64 = 50_000;
const MAX_CHECKPOINT_STORAGE_BYTES_PER_CONVERSATION: i64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointSummary {
    pub id: String,
    pub created_at_unix_ms: i64,
    pub tool_name: String,
    pub files: Vec<CheckpointFileSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointFileSummary {
    pub id: String,
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Debug)]
struct CheckpointFile {
    id: String,
    root: String,
    root_identity: Option<String>,
    path: String,
    before_exists: bool,
    before_hash: Option<String>,
    before_blob_hash: Option<String>,
    after_exists: bool,
    after_hash: Option<String>,
    after_blob_hash: Option<String>,
}

#[derive(Clone, Debug)]
struct Checkpoint {
    id: String,
    sequence: i64,
    created_at_unix_ms: i64,
    tool_name: String,
    recovery_available: bool,
    recovered_at_unix_ms: Option<i64>,
    files: Vec<CheckpointFile>,
}

#[derive(Clone, Debug)]
struct FileState {
    exists: bool,
    hash: Option<String>,
    blob_hash: Option<String>,
}

type CheckpointFileHistory =
    BTreeMap<(String, String), Vec<(i64, FileState, FileState, Option<String>)>>;

#[derive(Clone)]
struct RestorePlan {
    root: PathBuf,
    directory: Arc<crate::filesystem::OpenedDirectory>,
    parent_directory: Option<Arc<crate::filesystem::OpenedDirectory>>,
    parent_relative: PathBuf,
    file_name: OsString,
    directory_identity: super::DirectoryIdentity,
    path: String,
    expected_state: FileState,
    current: Option<FileSnapshot>,
    target_exists: bool,
    target_hash: Option<String>,
    target_content: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointRestoreError {
    Conflict,
    AlreadyRestored,
    NotFound,
    NotRecoverable,
    RollbackFailed,
    Unavailable,
}

/// Makes a persisted tool checkpoint eligible for explicit recovery.
pub(crate) fn mark_checkpoint_recovery_available(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint_id: &str,
) -> Result<(), TrackingError> {
    validate_conversation_id(conversation_id)?;
    validate_checkpoint_id(checkpoint_id)?;
    let _guard = lock_file_changes()?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let updated = connection
        .execute(
            "UPDATE conversation_file_checkpoints
             SET recovery_available = 1
             WHERE id = ?1 AND conversation_id = ?2
               AND recovered_at_unix_ms IS NULL",
            params![checkpoint_id, conversation_id],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if updated == 1 {
        Ok(())
    } else {
        Err(TrackingError::StorageUnavailable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CheckpointRecoveryState {
    Available,
    AlreadyRestored,
    Conflict,
}

/// Stores the exact before-and-after content for one tool operation.
pub(crate) fn persist_checkpoint(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    roots: &[(PathBuf, Vec<FileChangeDelta>)],
) -> Result<Option<String>, TrackingError> {
    persist_checkpoint_inner(storage, conversation_id, tool_name, roots, None)
}

/// Persists a checkpoint while the caller holds the file-change lock.
pub(crate) fn persist_checkpoint_locked(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    roots: &[(PathBuf, Vec<FileChangeDelta>)],
) -> Result<Option<String>, TrackingError> {
    persist_checkpoint_inner_locked(storage, conversation_id, tool_name, roots, None)
}

fn persist_checkpoint_inner(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    roots: &[(PathBuf, Vec<FileChangeDelta>)],
    recovered_checkpoint_id: Option<&str>,
) -> Result<Option<String>, TrackingError> {
    let _guard = lock_file_changes()?;
    persist_checkpoint_inner_locked(
        storage,
        conversation_id,
        tool_name,
        roots,
        recovered_checkpoint_id,
    )
}

fn persist_checkpoint_inner_locked(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    roots: &[(PathBuf, Vec<FileChangeDelta>)],
    recovered_checkpoint_id: Option<&str>,
) -> Result<Option<String>, TrackingError> {
    validate_conversation_id(conversation_id)?;
    if tool_name.is_empty() || tool_name.len() > 128 || tool_name.chars().any(char::is_control) {
        return Err(TrackingError::StorageUnavailable);
    }

    let mut files = Vec::new();
    let mut root_directories = Vec::with_capacity(roots.len());
    for (root, deltas) in roots {
        let (directory, canonical_root) = open_workspace_directory(root)?;
        let root_identity = super::directory_identity(&directory)
            .map_err(|_| TrackingError::WorkspaceUnavailable)?;
        let stored_root_identity = root_identity.storage_value()?;
        if deltas
            .iter()
            .any(|delta| delta.root_identity != root_identity)
        {
            return Err(TrackingError::WorkspaceUnavailable);
        }
        root_directories.push((canonical_root.clone(), directory));
        for delta in deltas {
            let relative = safe_relative_path(&delta.path)?;
            let path = relative.to_string_lossy().replace('\\', "/");
            let before_hash = delta.before.as_ref().map(|state| state.hash.as_str());
            let after_hash = delta.after.as_ref().map(|state| state.hash.as_str());
            if before_hash == after_hash {
                continue;
            }
            if files.len() >= MAX_CHECKPOINT_FILES_PER_EVENT {
                return Err(TrackingError::WorkspaceTooLarge);
            }
            let file_id =
                sha256(format!("{}\0{}", canonical_root.to_string_lossy(), path).as_bytes());
            files.push(CheckpointFile {
                id: file_id,
                root: canonical_root.to_string_lossy().into_owned(),
                root_identity: Some(stored_root_identity.clone()),
                path,
                before_exists: delta.before.is_some(),
                before_hash: delta.before.as_ref().map(|state| state.hash.clone()),
                before_blob_hash: delta
                    .before
                    .as_ref()
                    .and_then(|state| state.content.as_deref())
                    .map(|content| ensure_snapshot_blob(storage, content))
                    .transpose()?,
                after_exists: delta.after.is_some(),
                after_hash: delta.after.as_ref().map(|state| state.hash.clone()),
                after_blob_hash: delta
                    .after
                    .as_ref()
                    .and_then(|state| state.content.as_deref())
                    .map(|content| ensure_snapshot_blob(storage, content))
                    .transpose()?,
            });
        }
    }
    if files.is_empty() {
        return Ok(None);
    }
    cleanup_pending_snapshot_blobs(storage)?;
    let id = Uuid::new_v4().simple().to_string();
    let created_at_unix_ms = unix_time_millis()?;
    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let sequence = transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1
             FROM conversation_file_checkpoints WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if sequence > MAX_CHECKPOINTS_PER_CONVERSATION {
        return Err(TrackingError::WorkspaceTooLarge);
    }
    transaction
        .execute(
            "INSERT INTO conversation_file_checkpoints (
                id, conversation_id, sequence, created_at_unix_ms, tool_name
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, conversation_id, sequence, created_at_unix_ms, tool_name],
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    for file in files {
        for hash in [
            file.before_blob_hash.as_deref(),
            file.after_blob_hash.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            mark_snapshot_blob_ready(&transaction, hash)?;
        }
        transaction
            .execute(
                "INSERT INTO conversation_file_checkpoint_files (
                    checkpoint_id, file_id, root, root_identity, path,
                    before_exists, before_hash, before_blob_hash,
                    after_exists, after_hash, after_blob_hash
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    file.id,
                    file.root,
                    file.root_identity,
                    file.path,
                    file.before_exists,
                    file.before_hash,
                    file.before_blob_hash,
                    file.after_exists,
                    file.after_hash,
                    file.after_blob_hash,
                ],
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
    }
    let checkpoint_file_count = transaction
        .query_row(
            "SELECT COUNT(*) FROM conversation_file_checkpoint_files AS files
             JOIN conversation_file_checkpoints AS checkpoints
               ON checkpoints.id = files.checkpoint_id
             WHERE checkpoints.conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if checkpoint_file_count > MAX_CHECKPOINT_FILES_PER_CONVERSATION {
        return Err(TrackingError::WorkspaceTooLarge);
    }
    let snapshot_bytes = transaction
        .query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM (
                SELECT DISTINCT blobs.hash, blobs.size_bytes
                FROM file_change_blobs AS blobs
                WHERE EXISTS (
                    SELECT 1 FROM conversation_file_changes AS changes
                    WHERE changes.conversation_id = ?1
                      AND changes.baseline_blob_hash = blobs.hash
                ) OR EXISTS (
                    SELECT 1 FROM conversation_file_checkpoint_files AS files
                    JOIN conversation_file_checkpoints AS checkpoints
                      ON checkpoints.id = files.checkpoint_id
                    WHERE checkpoints.conversation_id = ?1
                      AND (files.before_blob_hash = blobs.hash OR files.after_blob_hash = blobs.hash)
                )
            )",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if snapshot_bytes > MAX_CHECKPOINT_STORAGE_BYTES_PER_CONVERSATION {
        return Err(TrackingError::WorkspaceTooLarge);
    }
    if let Some(recovered_checkpoint_id) = recovered_checkpoint_id {
        let updated = transaction
            .execute(
                "UPDATE conversation_file_checkpoints
                 SET recovered_at_unix_ms = ?3
                 WHERE id = ?1 AND conversation_id = ?2
                   AND recovered_at_unix_ms IS NULL",
                params![recovered_checkpoint_id, conversation_id, created_at_unix_ms],
            )
            .map_err(|_| TrackingError::StorageUnavailable)?;
        if updated != 1 {
            return Err(TrackingError::StorageUnavailable);
        }
    }
    for (root, directory) in &root_directories {
        if !workspace_directory_matches_path(root, directory)? {
            return Err(TrackingError::WorkspaceUnavailable);
        }
    }
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    Ok(Some(id))
}

/// Returns newest-first checkpoint metadata for the conversation.
pub(crate) fn list_checkpoints(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Vec<CheckpointSummary>, TrackingError> {
    validate_conversation_id(conversation_id)?;
    let _guard = lock_file_changes()?;
    cleanup_pending_snapshot_blobs(storage)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    load_checkpoints(&connection, conversation_id).map(|checkpoints| {
        checkpoints
            .into_iter()
            .rev()
            .map(|checkpoint| CheckpointSummary {
                id: checkpoint.id,
                created_at_unix_ms: checkpoint.created_at_unix_ms,
                tool_name: checkpoint.tool_name,
                files: checkpoint
                    .files
                    .into_iter()
                    .map(|file| CheckpointFileSummary {
                        id: file.id,
                        path: file.path,
                        kind: match (file.before_exists, file.after_exists) {
                            (false, true) => "added",
                            (true, false) => "deleted",
                            _ => "modified",
                        }
                        .to_owned(),
                    })
                    .collect(),
            })
            .collect()
    })
}

/// Reads the exact bounded diff captured for one tool operation and file.
pub(crate) fn read_checkpoint_diff(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint_id: &str,
    file_id: &str,
) -> Result<serde_json::Value, TrackingError> {
    validate_conversation_id(conversation_id)?;
    validate_checkpoint_id(checkpoint_id)?;
    validate_change_id(file_id)?;
    let _guard = lock_file_changes()?;
    cleanup_pending_snapshot_blobs(storage)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let file = load_checkpoint_file(&connection, conversation_id, checkpoint_id, file_id)?
        .ok_or(TrackingError::SnapshotUnavailable)?;
    let before = read_state_content(
        storage.root(),
        file.before_exists,
        file.before_hash.as_deref(),
        file.before_blob_hash.as_deref(),
    )?;
    let after = read_state_content(
        storage.root(),
        file.after_exists,
        file.after_hash.as_deref(),
        file.after_blob_hash.as_deref(),
    )?;
    let (Some(before), Some(after)) = (before, after) else {
        return Ok(json!({"path": file.path, "isBinary": false, "available": false, "diff": ""}));
    };
    let (Some(before_text), Some(after_text)) = (as_text(&before), as_text(&after)) else {
        return Ok(json!({"path": file.path, "isBinary": true, "available": false, "diff": ""}));
    };
    if before_text.len().saturating_add(after_text.len()) > MAX_TEXT_DIFF_BYTES {
        return Ok(json!({"path": file.path, "isBinary": false, "available": false, "diff": ""}));
    }
    let diff = TextDiff::configure()
        .timeout(std::time::Duration::from_millis(200))
        .diff_lines(&before_text, &after_text)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", file.path), &format!("b/{}", file.path))
        .to_string();
    let truncated = diff.lines().count() > MAX_DIFF_LINES || diff.len() > MAX_DIFF_OUTPUT_BYTES;
    let displayed_diff = if truncated {
        let mut output = diff
            .lines()
            .take(MAX_DIFF_LINES)
            .collect::<Vec<_>>()
            .join("\n");
        output.truncate(MAX_DIFF_OUTPUT_BYTES);
        output
    } else {
        diff
    };
    Ok(
        json!({"path": file.path, "isBinary": false, "available": true, "truncated": truncated, "diff": displayed_diff}),
    )
}

/// Restores the complete recorded workspace state after the selected checkpoint.
///
/// Every tracked path is checked against the current recorded head before any
/// file is changed. If a write or persistence step fails, applied files are
/// restored to their pre-operation contents.
pub(crate) fn restore_checkpoint(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint_id: &str,
) -> Result<Vec<super::FileChangeSummary>, CheckpointRestoreError> {
    validate_conversation_id(conversation_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    validate_checkpoint_id(checkpoint_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| CheckpointRestoreError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let connection = storage
        .connect()
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let checkpoints = load_checkpoints(&connection, conversation_id)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    drop(connection);
    let target = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.id == checkpoint_id)
        .ok_or(CheckpointRestoreError::NotFound)?;
    let target_sequence = target.sequence;
    let mut historical_states = CheckpointFileHistory::new();
    for checkpoint in &checkpoints {
        for file in &checkpoint.files {
            let key = (file.root.clone(), file.path.clone());
            let states = historical_states.entry(key).or_default();
            if states
                .first()
                .is_some_and(|(_, _, _, identity)| identity != &file.root_identity)
            {
                return Err(CheckpointRestoreError::Conflict);
            }
            states.push((
                checkpoint.sequence,
                state_before(file),
                state_after(file),
                file.root_identity.clone(),
            ));
        }
    }

    let mut plans = Vec::with_capacity(historical_states.len());
    for ((root_text, path), states) in &historical_states {
        let desired = states
            .iter()
            .rev()
            .find(|(sequence, _, _, _)| *sequence <= target_sequence)
            .map(|(_, _, after, _)| after.clone())
            .or_else(|| {
                states
                    .iter()
                    .find(|(sequence, _, _, _)| *sequence > target_sequence)
                    .map(|(_, before, _, _)| before.clone())
            })
            .or_else(|| states.first().map(|(_, before, _, _)| before.clone()))
            .ok_or(CheckpointRestoreError::Unavailable)?;
        let expected = states
            .last()
            .map(|(_, _, after, _)| after.clone())
            .ok_or(CheckpointRestoreError::Unavailable)?;
        let root_identity = states
            .first()
            .and_then(|(_, _, _, identity)| identity.as_deref())
            .ok_or(CheckpointRestoreError::NotRecoverable)?;
        plans.push(plan_file_restore(
            storage,
            root_text,
            path,
            root_identity,
            &expected,
            &desired,
        )?);
    }

    apply_restore_plans(
        storage,
        conversation_id,
        plans,
        "restore_checkpoint",
        None,
        _guard,
    )
}

/// Returns whether one failed tool operation can still be safely undone.
pub(crate) fn checkpoint_recovery_state(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint_id: &str,
) -> Result<CheckpointRecoveryState, CheckpointRestoreError> {
    validate_conversation_id(conversation_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    validate_checkpoint_id(checkpoint_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| CheckpointRestoreError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let connection = storage
        .connect()
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let checkpoints = load_checkpoints(&connection, conversation_id)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    drop(connection);
    let target = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.id == checkpoint_id)
        .ok_or(CheckpointRestoreError::NotFound)?;
    if target.recovered_at_unix_ms.is_some() {
        return Ok(CheckpointRecoveryState::AlreadyRestored);
    }
    if has_later_checkpoint_for_same_path(&checkpoints, target) {
        return Ok(CheckpointRecoveryState::Conflict);
    }
    recovery_state_for_checkpoint(storage, conversation_id, target)
}

/// Reverts only the file paths changed by one failed tool operation.
///
/// Later operations on the same paths make this operation unavailable. Other
/// paths changed by later operations are preserved.
pub(crate) fn revert_checkpoint_operation(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint_id: &str,
) -> Result<Vec<super::FileChangeSummary>, CheckpointRestoreError> {
    validate_conversation_id(conversation_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    validate_checkpoint_id(checkpoint_id).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let guard = lock_file_changes().map_err(|_| CheckpointRestoreError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let connection = storage
        .connect()
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let checkpoints = load_checkpoints(&connection, conversation_id)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    drop(connection);
    let target = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.id == checkpoint_id)
        .ok_or(CheckpointRestoreError::NotFound)?;
    if target.recovered_at_unix_ms.is_some() {
        return Err(CheckpointRestoreError::AlreadyRestored);
    }
    if has_later_checkpoint_for_same_path(&checkpoints, target) {
        return Err(CheckpointRestoreError::Conflict);
    }
    let recovery_state = recovery_state_for_checkpoint(storage, conversation_id, target)?;
    match recovery_state {
        CheckpointRecoveryState::Available => {}
        CheckpointRecoveryState::AlreadyRestored => {
            return Err(CheckpointRestoreError::AlreadyRestored);
        }
        CheckpointRecoveryState::Conflict => return Err(CheckpointRestoreError::Conflict),
    }
    let mut plans = Vec::with_capacity(target.files.len());
    for file in &target.files {
        plans.push(plan_file_restore(
            storage,
            &file.root,
            &file.path,
            file.root_identity
                .as_deref()
                .ok_or(CheckpointRestoreError::NotRecoverable)?,
            &state_after(file),
            &state_before(file),
        )?);
    }
    if plans.is_empty() {
        return Err(CheckpointRestoreError::NotFound);
    }

    apply_restore_plans(
        storage,
        conversation_id,
        plans,
        "restore_failed_tool_operation",
        Some(checkpoint_id),
        guard,
    )
}

fn recovery_state_for_checkpoint(
    storage: &AppStorage,
    conversation_id: &str,
    checkpoint: &Checkpoint,
) -> Result<CheckpointRecoveryState, CheckpointRestoreError> {
    if checkpoint.recovered_at_unix_ms.is_some() {
        return Ok(CheckpointRecoveryState::AlreadyRestored);
    }
    if !checkpoint.recovery_available {
        return Err(CheckpointRestoreError::NotRecoverable);
    }
    if checkpoint.files.is_empty() {
        return Ok(CheckpointRecoveryState::Conflict);
    }
    let connection = storage
        .connect()
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let entries = super::load_changes(&connection, conversation_id)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    drop(connection);
    let mut all_before = true;
    let mut all_recoverable = true;
    for file in &checkpoint.files {
        let (directory, root) = open_workspace_directory(Path::new(&file.root))
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        if root != file.root {
            return Ok(CheckpointRecoveryState::Conflict);
        }
        let identity = super::directory_identity(&directory)
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        let Some(stored_identity) = file.root_identity.as_deref() else {
            return Err(CheckpointRestoreError::NotRecoverable);
        };
        if !identity
            .matches_storage_value(Some(stored_identity))
            .map_err(|_| CheckpointRestoreError::Unavailable)?
        {
            return Ok(CheckpointRecoveryState::Conflict);
        }
        let relative =
            safe_relative_path(&file.path).map_err(|_| CheckpointRestoreError::Unavailable)?;
        let current = capture_file_from_directory(&directory, &relative)
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        let before = state_before(file);
        let after = state_after(file);
        if (before.exists && before.blob_hash.is_none())
            || current
                .as_ref()
                .is_some_and(|state| state.content.is_none())
        {
            return Err(CheckpointRestoreError::Unavailable);
        }
        let matches_before = matches_state(&current, &before);
        let matches_after = matches_state(&current, &after);
        let change_id = super::change_id(Path::new(&file.root), &file.path);
        let Some(entry) = entries.iter().find(|entry| entry.id == change_id) else {
            return Ok(CheckpointRecoveryState::Conflict);
        };
        let stored_root_identity = identity
            .storage_value()
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        if entry.root != file.root
            || entry.path != file.path
            || entry.root_identity.as_deref() != Some(stored_root_identity.as_str())
        {
            return Ok(CheckpointRecoveryState::Conflict);
        }
        let entry_matches_before = manifest_entry_matches_state(entry, &before);
        let entry_matches_after = manifest_entry_matches_state(entry, &after);
        all_before &= matches_before;
        all_before &= entry_matches_before;
        all_recoverable &=
            (matches_before || matches_after) && (entry_matches_before || entry_matches_after);
    }
    if all_before {
        Ok(CheckpointRecoveryState::AlreadyRestored)
    } else if all_recoverable {
        Ok(CheckpointRecoveryState::Available)
    } else {
        Ok(CheckpointRecoveryState::Conflict)
    }
}

fn has_later_checkpoint_for_same_path(checkpoints: &[Checkpoint], target: &Checkpoint) -> bool {
    let target_paths = target
        .files
        .iter()
        .map(|file| (file.root.as_str(), file.path.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    checkpoints.iter().any(|checkpoint| {
        checkpoint.sequence > target.sequence
            && checkpoint
                .files
                .iter()
                .any(|file| target_paths.contains(&(file.root.as_str(), file.path.as_str())))
    })
}

fn matches_state(current: &Option<FileSnapshot>, expected: &FileState) -> bool {
    current.as_ref().map(|state| state.hash.as_str()) == expected.hash.as_deref()
        && current.is_some() == expected.exists
}

fn plan_file_restore(
    storage: &AppStorage,
    root_text: &str,
    path: &str,
    expected_root_identity: &str,
    expected: &FileState,
    desired: &FileState,
) -> Result<RestorePlan, CheckpointRestoreError> {
    let (directory, root) = open_workspace_directory(Path::new(root_text))
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let directory_identity =
        super::directory_identity(&directory).map_err(|_| CheckpointRestoreError::Unavailable)?;
    if !directory_identity
        .matches_storage_value(Some(expected_root_identity))
        .map_err(|_| CheckpointRestoreError::Unavailable)?
    {
        return Err(CheckpointRestoreError::Conflict);
    }
    let directory = Arc::new(directory);
    let relative = safe_relative_path(path).map_err(|_| CheckpointRestoreError::Unavailable)?;
    let parent_relative = relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let file_name = relative
        .file_name()
        .ok_or(CheckpointRestoreError::Unavailable)?
        .to_os_string();
    let parent_directory = if parent_relative.as_os_str().is_empty() {
        Some(Arc::new(
            directory
                .try_clone()
                .map_err(|_| CheckpointRestoreError::Unavailable)?,
        ))
    } else {
        match crate::filesystem::open_directory_from_directory(directory.as_ref(), &parent_relative)
        {
            Ok(parent) => Some(Arc::new(parent)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err(CheckpointRestoreError::Unavailable),
        }
    };
    let current = match &parent_directory {
        Some(parent) => capture_file_from_directory(parent.as_ref(), Path::new(&file_name)),
        None => capture_file_from_directory(directory.as_ref(), &relative),
    }
    .map_err(|_| CheckpointRestoreError::Unavailable)?;
    if !matches_state(&current, expected) && !matches_state(&current, desired) {
        return Err(CheckpointRestoreError::Conflict);
    }
    let needs_write = !matches_state(&current, desired);
    let target_content = if needs_write && desired.exists {
        let content = desired
            .blob_hash
            .as_deref()
            .map(|hash| read_snapshot_blob(storage.root(), hash))
            .transpose()
            .map_err(|_| CheckpointRestoreError::Unavailable)?
            .ok_or(CheckpointRestoreError::Unavailable)?;
        if desired.hash.as_deref() != Some(sha256(&content).as_str()) {
            return Err(CheckpointRestoreError::Unavailable);
        }
        Some(content)
    } else {
        None
    };
    if needs_write
        && current
            .as_ref()
            .is_some_and(|state| state.content.is_none())
    {
        return Err(CheckpointRestoreError::Unavailable);
    }
    ensure_no_symlink_path_from_directory(&directory, &relative)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    Ok(RestorePlan {
        root,
        directory,
        parent_directory,
        parent_relative,
        file_name,
        directory_identity,
        path: path.to_owned(),
        expected_state: expected.clone(),
        current,
        target_exists: desired.exists,
        target_hash: desired.hash.clone(),
        target_content,
    })
}

fn apply_restore_plans(
    storage: &AppStorage,
    conversation_id: &str,
    mut plans: Vec<RestorePlan>,
    restore_tool_name: &str,
    recovered_checkpoint_id: Option<&str>,
    _file_change_guard: MutexGuard<'static, ()>,
) -> Result<Vec<super::FileChangeSummary>, CheckpointRestoreError> {
    let connection = storage
        .connect()
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    let manifest_entries = super::load_changes(&connection, conversation_id)
        .map_err(|_| CheckpointRestoreError::Unavailable)?;
    drop(connection);
    let mut plan_entries = Vec::with_capacity(plans.len());
    let mut manifest_needs_update = Vec::with_capacity(plans.len());
    for plan in &plans {
        let id = super::change_id(&plan.root, &plan.path);
        let entry = manifest_entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or(CheckpointRestoreError::Conflict)?;
        let stored_root_identity = plan
            .directory_identity
            .storage_value()
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        if entry.root != plan.root.to_string_lossy()
            || entry.path != plan.path
            || entry.root_identity.as_deref() != Some(stored_root_identity.as_str())
        {
            return Err(CheckpointRestoreError::Conflict);
        }
        let entry_matches_expected = manifest_entry_matches_state(&entry, &plan.expected_state);
        let target_state = plan.target_state();
        let entry_matches_target = manifest_entry_matches_state(&entry, &target_state);
        if !entry_matches_expected && !entry_matches_target {
            return Err(CheckpointRestoreError::Conflict);
        }
        plan_entries.push(entry);
        manifest_needs_update.push(
            entry_matches_expected && !file_states_match(&plan.expected_state, &target_state),
        );
    }

    for plan in &plans {
        ensure_plan_root_matches(plan)?;
        ensure_plan_parent_matches(plan)?;
        let current = capture_plan_file(plan)?;
        if current.as_ref().map(|state| state.hash.as_str())
            != plan.current.as_ref().map(|state| state.hash.as_str())
            || current.is_some() != plan.current.is_some()
        {
            return Err(CheckpointRestoreError::Conflict);
        }
    }

    let mut applied: Vec<RestorePlan> = Vec::new();
    for plan in plans.iter_mut().filter(|plan| plan.needs_write()) {
        if let Err(error) =
            ensure_plan_root_matches(plan).and_then(|()| ensure_plan_parent_matches(plan))
        {
            return if rollback_applied(&applied) {
                Err(error)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        let current = match capture_plan_file(plan) {
            Ok(current) => current,
            Err(_) => {
                return if rollback_applied(&applied) {
                    Err(CheckpointRestoreError::Unavailable)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
        };
        if current.as_ref().map(|state| state.hash.as_str())
            != plan.current.as_ref().map(|state| state.hash.as_str())
            || current.is_some() != plan.current.is_some()
        {
            return if rollback_applied(&applied) {
                Err(CheckpointRestoreError::Conflict)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        if plan.target_exists && plan.parent_directory.is_none() {
            if let Err(error) =
                create_restore_plan_parent(plan).and_then(|()| ensure_plan_parent_matches(plan))
            {
                return if rollback_applied(&applied) {
                    Err(error)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
            let current = match capture_plan_file(plan) {
                Ok(current) => current,
                Err(_) => {
                    return if rollback_applied(&applied) {
                        Err(CheckpointRestoreError::Unavailable)
                    } else {
                        Err(CheckpointRestoreError::RollbackFailed)
                    };
                }
            };
            if current.as_ref().map(|state| state.hash.as_str())
                != plan.current.as_ref().map(|state| state.hash.as_str())
                || current.is_some() != plan.current.is_some()
            {
                return if rollback_applied(&applied) {
                    Err(CheckpointRestoreError::Conflict)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
        }
        let result = if plan.target_exists {
            plan.target_content
                .as_deref()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "checkpoint content is unavailable",
                    )
                })
                .and_then(|content| write_restore_plan_file(plan, content))
        } else if plan.current.is_some() {
            remove_restore_plan_file(plan)
        } else {
            Ok(())
        };
        if result.is_err() {
            return if rollback_in_progress(plan, &applied) {
                Err(CheckpointRestoreError::Unavailable)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        let updated = match capture_plan_file(plan) {
            Ok(updated) => updated,
            Err(_) => {
                return if rollback_in_progress(plan, &applied) {
                    Err(CheckpointRestoreError::Unavailable)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
        };
        if updated.as_ref().map(|state| state.hash.as_str()) != plan.target_hash.as_deref()
            || updated.is_some() != plan.target_exists
        {
            return if rollback_in_progress(plan, &applied) {
                Err(CheckpointRestoreError::Conflict)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        if let Err(error) =
            ensure_plan_root_matches(plan).and_then(|()| ensure_plan_parent_matches(plan))
        {
            return if rollback_in_progress(plan, &applied) {
                Err(error)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        applied.push((*plan).clone());
    }

    let mut restore_deltas: BTreeMap<PathBuf, Vec<FileChangeDelta>> = BTreeMap::new();
    let mut tracking_deltas: BTreeMap<PathBuf, Vec<FileChangeDelta>> = BTreeMap::new();
    for (index, plan) in plans.iter().enumerate() {
        let target_state = plan.target_state();
        if file_states_match(&plan.expected_state, &target_state) {
            continue;
        }
        if let Err(error) =
            ensure_plan_root_matches(plan).and_then(|()| ensure_plan_parent_matches(plan))
        {
            return if rollback_applied(&applied) {
                Err(error)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        let after = match capture_plan_file(plan) {
            Ok(after) => after,
            Err(_) => {
                if !rollback_applied(&applied) {
                    return Err(CheckpointRestoreError::RollbackFailed);
                }
                return Err(CheckpointRestoreError::Unavailable);
            }
        };
        if !matches_state(&after, &target_state) {
            return if rollback_applied(&applied) {
                Err(CheckpointRestoreError::Conflict)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        let delta = FileChangeDelta {
            root_identity: plan.directory_identity,
            path: plan.path.clone(),
            before: state_snapshot_metadata(&plan.expected_state)?,
            after: after.clone(),
        };
        restore_deltas
            .entry(plan.root.clone())
            .or_default()
            .push(delta.clone());
        if manifest_needs_update[index] {
            let before = if super::is_entry_approved(&plan_entries[index]) {
                state_snapshot_with_content(storage, &plan.expected_state)?
            } else {
                state_snapshot_metadata(&plan.expected_state)?
            };
            tracking_deltas
                .entry(plan.root.clone())
                .or_default()
                .push(FileChangeDelta { before, ..delta });
        }
    }
    if restore_deltas.is_empty() {
        return Ok(Vec::new());
    }
    for plan in plans
        .iter()
        .filter(|plan| !file_states_match(&plan.expected_state, &plan.target_state()))
    {
        if let Err(error) =
            ensure_plan_root_matches(plan).and_then(|()| ensure_plan_parent_matches(plan))
        {
            return if rollback_applied(&applied) {
                Err(error)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
    }
    let grouped_tracking_deltas = tracking_deltas.into_iter().collect::<Vec<_>>();
    let summaries = if grouped_tracking_deltas.is_empty() {
        Vec::new()
    } else {
        match super::record_deltas_batch_locked(storage, conversation_id, &grouped_tracking_deltas)
        {
            Ok(summaries) => summaries,
            Err(_) => {
                return if rollback_applied(&applied) {
                    Err(CheckpointRestoreError::Unavailable)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
        }
    };
    let checkpoint_roots = restore_deltas.into_iter().collect::<Vec<_>>();
    let recorded_roots = checkpoint_roots
        .iter()
        .map(|(root, _)| root.clone())
        .collect::<Vec<_>>();
    for plan in &applied {
        if let Err(error) =
            ensure_plan_root_matches(plan).and_then(|()| ensure_plan_parent_matches(plan))
        {
            let files_restored = rollback_applied(&applied);
            let tracking_restored =
                repair_tracking(storage, conversation_id, &applied, &recorded_roots);
            return if files_restored && tracking_restored {
                Err(error)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
    }
    let persist_result = persist_checkpoint_inner_locked(
        storage,
        conversation_id,
        restore_tool_name,
        &checkpoint_roots,
        recovered_checkpoint_id,
    );
    if persist_result
        .and_then(|checkpoint_id| checkpoint_id.ok_or(TrackingError::StorageUnavailable))
        .is_err()
    {
        let files_restored = rollback_applied(&applied);
        let tracking_restored =
            repair_tracking(storage, conversation_id, &applied, &recorded_roots);
        return if files_restored && tracking_restored {
            Err(CheckpointRestoreError::Unavailable)
        } else {
            Err(CheckpointRestoreError::RollbackFailed)
        };
    }
    Ok(summaries)
}

impl RestorePlan {
    fn target_state(&self) -> FileState {
        FileState {
            exists: self.target_exists,
            hash: self.target_hash.clone(),
            blob_hash: None,
        }
    }

    fn needs_write(&self) -> bool {
        self.current.as_ref().map(|state| state.hash.as_str()) != self.target_hash.as_deref()
            || self.current.is_some() != self.target_exists
    }
}

fn file_states_match(left: &FileState, right: &FileState) -> bool {
    left.exists == right.exists && left.hash == right.hash
}

fn manifest_entry_matches_state(entry: &super::ManifestEntry, state: &FileState) -> bool {
    entry.expected_exists == state.exists && entry.expected_hash == state.hash
}

fn state_snapshot_metadata(
    state: &FileState,
) -> Result<Option<FileSnapshot>, CheckpointRestoreError> {
    if !state.exists {
        return Ok(None);
    }
    let hash = state
        .hash
        .clone()
        .ok_or(CheckpointRestoreError::NotRecoverable)?;
    Ok(Some(FileSnapshot {
        hash,
        content: None,
    }))
}

fn state_snapshot_with_content(
    storage: &AppStorage,
    state: &FileState,
) -> Result<Option<FileSnapshot>, CheckpointRestoreError> {
    let Some(mut snapshot) = state_snapshot_metadata(state)? else {
        return Ok(None);
    };
    let content = read_state_content(
        storage.root(),
        state.exists,
        state.hash.as_deref(),
        state.blob_hash.as_deref(),
    )
    .map_err(|_| CheckpointRestoreError::Unavailable)?
    .ok_or(CheckpointRestoreError::NotRecoverable)?;
    snapshot.content = Some(content);
    Ok(Some(snapshot))
}

fn ensure_plan_root_matches(plan: &RestorePlan) -> Result<(), CheckpointRestoreError> {
    match super::workspace_directory_matches_path(&plan.root, &plan.directory) {
        Ok(true) => Ok(()),
        Ok(false) => Err(CheckpointRestoreError::Conflict),
        Err(_) => Err(CheckpointRestoreError::Unavailable),
    }
}

fn ensure_plan_parent_matches(plan: &RestorePlan) -> Result<(), CheckpointRestoreError> {
    let Some(parent) = &plan.parent_directory else {
        return Ok(());
    };
    let current_parent = if plan.parent_relative.as_os_str().is_empty() {
        plan.directory
            .try_clone()
            .map_err(|_| CheckpointRestoreError::Unavailable)?
    } else {
        crate::filesystem::open_directory_from_directory(
            plan.directory.as_ref(),
            &plan.parent_relative,
        )
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                CheckpointRestoreError::Conflict
            } else {
                CheckpointRestoreError::Unavailable
            }
        })?
    };
    match crate::filesystem::same_directory_object(&current_parent, parent.as_ref()) {
        Ok(true) => Ok(()),
        Ok(false) => Err(CheckpointRestoreError::Conflict),
        Err(_) => Err(CheckpointRestoreError::Unavailable),
    }
}

fn create_restore_plan_parent(plan: &mut RestorePlan) -> Result<(), CheckpointRestoreError> {
    if plan.parent_directory.is_some() {
        return Ok(());
    }
    let parent = crate::filesystem::create_directory_chain_from_directory(
        plan.directory.as_ref(),
        &plan.parent_relative,
    )
    .map_err(|_| CheckpointRestoreError::Unavailable)?;
    plan.parent_directory = Some(Arc::new(parent));
    Ok(())
}

fn capture_plan_file(plan: &RestorePlan) -> Result<Option<FileSnapshot>, CheckpointRestoreError> {
    match &plan.parent_directory {
        Some(parent) => capture_file_from_directory(parent.as_ref(), Path::new(&plan.file_name)),
        None => capture_file_from_directory(plan.directory.as_ref(), Path::new(&plan.path)),
    }
    .map_err(|_| CheckpointRestoreError::Unavailable)
}

fn write_restore_plan_file(plan: &RestorePlan, content: &[u8]) -> io::Result<()> {
    let parent = plan.parent_directory.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint restore parent capability is unavailable",
        )
    })?;
    atomic_write_workspace_file_from_directory(parent.as_ref(), Path::new(&plan.file_name), content)
}

fn remove_restore_plan_file(plan: &RestorePlan) -> io::Result<()> {
    let parent = plan.parent_directory.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint restore parent capability is unavailable",
        )
    })?;
    remove_workspace_file_from_directory(parent.as_ref(), Path::new(&plan.file_name))
}

fn rollback_applied(plans: &[RestorePlan]) -> bool {
    let mut restored = true;
    for plan in plans.iter().rev() {
        restored &= rollback_plan(plan);
    }
    restored
}

fn rollback_in_progress(plan: &RestorePlan, applied: &[RestorePlan]) -> bool {
    let current_restored = rollback_plan(plan);
    let previous_restored = rollback_applied(applied);
    current_restored && previous_restored
}

fn rollback_plan(plan: &RestorePlan) -> bool {
    let current = match capture_plan_file(plan) {
        Ok(current) => current,
        Err(_) => return false,
    };
    if current.as_ref().map(|state| state.hash.as_str())
        == plan.current.as_ref().map(|state| state.hash.as_str())
        && current.is_some() == plan.current.is_some()
    {
        return true;
    }
    if current.as_ref().map(|state| state.hash.as_str()) != plan.target_hash.as_deref()
        || current.is_some() != plan.target_exists
    {
        return false;
    }
    match &plan.current {
        Some(current) => {
            let Some(content) = current.content.as_deref() else {
                return false;
            };
            if write_restore_plan_file(plan, content).is_err() {
                return false;
            }
            capture_plan_file(plan).is_ok_and(|state| {
                state.as_ref().map(|snapshot| snapshot.hash.as_str()) == Some(current.hash.as_str())
            })
        }
        None => match remove_restore_plan_file(plan) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => true,
            Err(_) => false,
        },
    }
}

fn repair_tracking(
    storage: &AppStorage,
    conversation_id: &str,
    plans: &[RestorePlan],
    recorded_roots: &[PathBuf],
) -> bool {
    let mut grouped: BTreeMap<PathBuf, Vec<FileChangeDelta>> = BTreeMap::new();
    for plan in plans {
        if !recorded_roots.contains(&plan.root) {
            continue;
        }
        if ensure_plan_root_matches(plan).is_err() || ensure_plan_parent_matches(plan).is_err() {
            return false;
        }
        let target = if plan.target_exists {
            plan.target_content.as_ref().map(|content| FileSnapshot {
                hash: sha256(content),
                content: Some(content.clone()),
            })
        } else {
            None
        };
        grouped
            .entry(plan.root.clone())
            .or_default()
            .push(FileChangeDelta {
                root_identity: plan.directory_identity,
                path: plan.path.clone(),
                before: target,
                after: plan.current.clone(),
            });
    }
    // The caller holds the file-change lock while rolling back a failed restore.
    let grouped_deltas = grouped.into_iter().collect::<Vec<_>>();
    super::record_deltas_batch_locked(storage, conversation_id, &grouped_deltas).is_ok()
}

fn state_before(file: &CheckpointFile) -> FileState {
    FileState {
        exists: file.before_exists,
        hash: file.before_hash.clone(),
        blob_hash: file.before_blob_hash.clone(),
    }
}

fn state_after(file: &CheckpointFile) -> FileState {
    FileState {
        exists: file.after_exists,
        hash: file.after_hash.clone(),
        blob_hash: file.after_blob_hash.clone(),
    }
}

fn read_state_content(
    data_root: &Path,
    exists: bool,
    expected_hash: Option<&str>,
    blob_hash: Option<&str>,
) -> Result<Option<Vec<u8>>, TrackingError> {
    if !exists {
        return Ok(Some(Vec::new()));
    }
    let Some(hash) = blob_hash else {
        return Ok(None);
    };
    let content = read_snapshot_blob(data_root, hash)?;
    if expected_hash != Some(sha256(&content).as_str()) {
        return Err(TrackingError::SnapshotUnavailable);
    }
    Ok(Some(content))
}

fn mark_snapshot_blob_ready(
    transaction: &StorageTransaction<'_>,
    hash: &str,
) -> Result<(), TrackingError> {
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
    Ok(())
}

fn validate_checkpoint_id(id: &str) -> Result<(), TrackingError> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(())
}

fn load_checkpoints(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
) -> Result<Vec<Checkpoint>, TrackingError> {
    let mut statement = connection
        .prepare(
            "SELECT checkpoint.id, checkpoint.sequence, checkpoint.created_at_unix_ms,
                    checkpoint.tool_name, checkpoint.recovery_available,
                    checkpoint.recovered_at_unix_ms,
                    file.file_id, file.root, file.root_identity, file.path,
                    file.before_exists, file.before_hash, file.before_blob_hash,
                    file.after_exists, file.after_hash, file.after_blob_hash
             FROM conversation_file_checkpoints AS checkpoint
             LEFT JOIN conversation_file_checkpoint_files AS file
               ON file.checkpoint_id = checkpoint.id
             WHERE checkpoint.conversation_id = ?1
             ORDER BY checkpoint.sequence, file.path, file.root",
        )
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<bool>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<bool>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
            ))
        })
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let mut checkpoints: Vec<Checkpoint> = Vec::new();
    for row in rows {
        let (
            id,
            sequence,
            created_at_unix_ms,
            tool_name,
            recovery_available,
            recovered_at_unix_ms,
            file_id,
            root,
            root_identity,
            path,
            before_exists,
            before_hash,
            before_blob_hash,
            after_exists,
            after_hash,
            after_blob_hash,
        ) = row.map_err(|_| TrackingError::StorageUnavailable)?;
        if checkpoints
            .last()
            .is_none_or(|checkpoint| checkpoint.id != id)
        {
            checkpoints.push(Checkpoint {
                id: id.clone(),
                sequence,
                created_at_unix_ms,
                tool_name,
                recovery_available,
                recovered_at_unix_ms,
                files: Vec::new(),
            });
        }
        if let (Some(file_id), Some(root), Some(path), Some(before_exists), Some(after_exists)) =
            (file_id, root, path, before_exists, after_exists)
        {
            let Some(checkpoint) = checkpoints.last_mut() else {
                return Err(TrackingError::StorageUnavailable);
            };
            checkpoint.files.push(CheckpointFile {
                id: file_id,
                root,
                root_identity,
                path,
                before_exists,
                before_hash,
                before_blob_hash,
                after_exists,
                after_hash,
                after_blob_hash,
            });
        }
    }
    Ok(checkpoints)
}

fn load_checkpoint_file(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    checkpoint_id: &str,
    file_id: &str,
) -> Result<Option<CheckpointFile>, TrackingError> {
    connection
        .query_row(
            "SELECT file.file_id, file.root, file.root_identity, file.path,
                    file.before_exists, file.before_hash, file.before_blob_hash,
                    file.after_exists, file.after_hash, file.after_blob_hash
             FROM conversation_file_checkpoint_files AS file
             JOIN conversation_file_checkpoints AS checkpoint
               ON checkpoint.id = file.checkpoint_id
             WHERE checkpoint.conversation_id = ?1
               AND checkpoint.id = ?2 AND file.file_id = ?3",
            params![conversation_id, checkpoint_id, file_id],
            |row| {
                Ok(CheckpointFile {
                    id: row.get(0)?,
                    root: row.get(1)?,
                    root_identity: row.get(2)?,
                    path: row.get(3)?,
                    before_exists: row.get(4)?,
                    before_hash: row.get(5)?,
                    before_blob_hash: row.get(6)?,
                    after_exists: row.get(7)?,
                    after_hash: row.get(8)?,
                    after_blob_hash: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|_| TrackingError::StorageUnavailable)
}

#[cfg(all(test, windows))]
mod tests {
    use super::super::workspace_root_identity;
    use super::{
        FileState, capture_plan_file, create_restore_plan_parent, plan_file_restore, sha256,
        write_restore_plan_file,
    };
    use crate::storage::AppStorage;
    use std::{fs, path::PathBuf};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-checkpoint-parent-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    #[test]
    fn checkpoint_restore_plan_holds_nested_parent_capability() {
        let temporary = TestDirectory::new();
        let workspace = temporary.0.join("workspace");
        let storage_root = temporary.0.join("storage");
        let nested = workspace.join("nested");
        fs::create_dir_all(&nested).expect("create nested workspace directory");
        fs::create_dir_all(&storage_root).expect("create storage directory");
        fs::write(nested.join("note.txt"), b"current").expect("write workspace file");
        let storage = AppStorage::open_at(storage_root).expect("open isolated storage");
        let root_identity = workspace_root_identity(&workspace)
            .expect("capture workspace root identity")
            .storage_value()
            .expect("serialize workspace root identity");
        let current = FileState {
            exists: true,
            hash: Some(sha256(b"current")),
            blob_hash: None,
        };

        let plan = plan_file_restore(
            &storage,
            workspace.to_str().expect("workspace path is UTF-8"),
            "nested/note.txt",
            &root_identity,
            &current,
            &current,
        )
        .expect("plan checkpoint restore");
        let moved = workspace.join("nested-moved");

        assert!(fs::rename(&nested, &moved).is_err());

        drop(plan);
        fs::rename(&nested, &moved).expect("nested directory is movable after plan is dropped");
    }

    #[test]
    fn checkpoint_restore_plan_retains_a_created_nested_parent() {
        let temporary = TestDirectory::new();
        let workspace = temporary.0.join("workspace");
        let storage_root = temporary.0.join("storage");
        fs::create_dir_all(&workspace).expect("create workspace");
        fs::create_dir_all(&storage_root).expect("create storage directory");
        let storage = AppStorage::open_at(storage_root).expect("open isolated storage");
        let root_identity = workspace_root_identity(&workspace)
            .expect("capture workspace root identity")
            .storage_value()
            .expect("serialize workspace root identity");
        let missing = FileState {
            exists: false,
            hash: None,
            blob_hash: None,
        };
        let mut plan = plan_file_restore(
            &storage,
            workspace.to_str().expect("workspace path is UTF-8"),
            "nested/note.txt",
            &root_identity,
            &missing,
            &missing,
        )
        .expect("plan checkpoint restore with a missing parent");

        create_restore_plan_parent(&mut plan).expect("create and retain nested parent");
        write_restore_plan_file(&plan, b"restored").expect("write through retained parent");
        assert_eq!(
            capture_plan_file(&plan)
                .expect("capture restored file")
                .as_ref()
                .map(|snapshot| snapshot.hash.as_str()),
            Some(sha256(b"restored").as_str())
        );
        let nested = workspace.join("nested");
        let moved = workspace.join("nested-moved");

        assert!(fs::rename(&nested, &moved).is_err());

        drop(plan);
        fs::rename(&nested, &moved).expect("nested directory is movable after plan is dropped");
    }
}
