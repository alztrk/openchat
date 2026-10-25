//! Persistent per-tool file checkpoints and conflict-safe workspace restoration.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;
use similar::TextDiff;
use uuid::Uuid;

use super::{
    FileChangeDelta, FileSnapshot, MAX_DIFF_LINES, MAX_DIFF_OUTPUT_BYTES, MAX_TEXT_DIFF_BYTES,
    TrackingError, as_text, atomic_write, capture_file, cleanup_pending_snapshot_blobs,
    ensure_no_symlink_path, ensure_snapshot_blob, lock_file_changes, read_snapshot_blob,
    safe_relative_path, sha256, unix_time_millis, validate_change_id, validate_conversation_id,
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
    files: Vec<CheckpointFile>,
}

#[derive(Clone, Debug)]
struct FileState {
    exists: bool,
    hash: Option<String>,
    blob_hash: Option<String>,
}

#[derive(Clone, Debug)]
struct RestorePlan {
    root: PathBuf,
    path: String,
    current: Option<FileSnapshot>,
    target_exists: bool,
    target_hash: Option<String>,
    target_content: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointRestoreError {
    Conflict,
    NotFound,
    RollbackFailed,
    Unavailable,
}

/// Stores the exact before-and-after content for one tool operation.
pub(crate) fn persist_checkpoint(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    roots: &[(PathBuf, Vec<FileChangeDelta>)],
) -> Result<(), TrackingError> {
    validate_conversation_id(conversation_id)?;
    if tool_name.is_empty() || tool_name.len() > 128 || tool_name.chars().any(char::is_control) {
        return Err(TrackingError::StorageUnavailable);
    }

    let mut files = Vec::new();
    for (root, deltas) in roots {
        let canonical_root = root
            .canonicalize()
            .map_err(|_| TrackingError::WorkspaceUnavailable)?;
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
        return Ok(());
    }
    let _guard = lock_file_changes()?;
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
                    checkpoint_id, file_id, root, path,
                    before_exists, before_hash, before_blob_hash,
                    after_exists, after_hash, after_blob_hash
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    file.id,
                    file.root,
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
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)
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
    let target = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.id == checkpoint_id)
        .ok_or(CheckpointRestoreError::NotFound)?;
    let target_sequence = target.sequence;
    let mut historical_states: BTreeMap<(String, String), Vec<(i64, FileState, FileState)>> =
        BTreeMap::new();
    for checkpoint in &checkpoints {
        for file in &checkpoint.files {
            let key = (file.root.clone(), file.path.clone());
            historical_states.entry(key).or_default().push((
                checkpoint.sequence,
                state_before(file),
                state_after(file),
            ));
        }
    }

    let mut plans = Vec::with_capacity(historical_states.len());
    for ((root_text, path), states) in &historical_states {
        let desired = states
            .iter()
            .rev()
            .find(|(sequence, _, _)| *sequence <= target_sequence)
            .map(|(_, _, after)| after.clone())
            .or_else(|| {
                states
                    .iter()
                    .find(|(sequence, _, _)| *sequence > target_sequence)
                    .map(|(_, before, _)| before.clone())
            })
            .or_else(|| states.first().map(|(_, before, _)| before.clone()))
            .ok_or(CheckpointRestoreError::Unavailable)?;
        let expected = states
            .last()
            .map(|(_, _, after)| after.clone())
            .ok_or(CheckpointRestoreError::Unavailable)?;
        let root = PathBuf::from(root_text)
            .canonicalize()
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        let current = capture_file(&root, path).map_err(|_| CheckpointRestoreError::Unavailable)?;
        if current.as_ref().map(|state| state.hash.as_str()) != expected.hash.as_deref()
            || current.is_some() != expected.exists
        {
            return Err(CheckpointRestoreError::Conflict);
        }
        let needs_write = current.as_ref().map(|state| state.hash.as_str())
            != desired.hash.as_deref()
            || current.is_some() != desired.exists;
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
        let target_path =
            root.join(safe_relative_path(path).map_err(|_| CheckpointRestoreError::Unavailable)?);
        ensure_no_symlink_path(&root, &target_path)
            .map_err(|_| CheckpointRestoreError::Unavailable)?;
        plans.push(RestorePlan {
            root,
            path: path.clone(),
            current,
            target_exists: desired.exists,
            target_hash: desired.hash,
            target_content,
        });
    }

    let mut applied = Vec::new();
    for plan in plans.iter().filter(|plan| plan.needs_write()) {
        let current = match capture_file(&plan.root, &plan.path) {
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
        let target = plan.root.join(&plan.path);
        let result = if plan.target_exists {
            plan.target_content
                .as_deref()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "checkpoint content is unavailable",
                    )
                })
                .and_then(|content| atomic_write(&target, content))
        } else if plan.current.is_some() {
            fs::remove_file(&target)
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
        let updated = match capture_file(&plan.root, &plan.path) {
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
            return if rollback_applied(&applied) {
                Err(CheckpointRestoreError::Conflict)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        applied.push(plan.clone());
    }

    let mut restore_deltas: BTreeMap<PathBuf, Vec<FileChangeDelta>> = BTreeMap::new();
    for plan in &plans {
        if !plan.needs_write() {
            continue;
        }
        let after = match capture_file(&plan.root, &plan.path) {
            Ok(after) => after,
            Err(_) => {
                if !rollback_applied(&applied) {
                    return Err(CheckpointRestoreError::RollbackFailed);
                }
                return Err(CheckpointRestoreError::Unavailable);
            }
        };
        if after.as_ref().map(|state| state.hash.as_str()) != plan.target_hash.as_deref()
            || after.is_some() != plan.target_exists
        {
            return if rollback_applied(&applied) {
                Err(CheckpointRestoreError::Conflict)
            } else {
                Err(CheckpointRestoreError::RollbackFailed)
            };
        }
        restore_deltas
            .entry(plan.root.clone())
            .or_default()
            .push(FileChangeDelta {
                path: plan.path.clone(),
                before: plan.current.clone(),
                after,
            });
    }
    drop(_guard);
    let mut summaries = Vec::new();
    let mut recorded_roots = Vec::new();
    for (root, deltas) in &restore_deltas {
        match super::record_deltas(storage, conversation_id, root, deltas.clone()) {
            Ok(updated) => {
                summaries.extend(updated);
                recorded_roots.push(root.clone());
            }
            Err(_) => {
                let files_restored = rollback_applied(&applied);
                let tracking_restored =
                    repair_tracking(storage, conversation_id, &applied, &recorded_roots);
                return if files_restored && tracking_restored {
                    Err(CheckpointRestoreError::Unavailable)
                } else {
                    Err(CheckpointRestoreError::RollbackFailed)
                };
            }
        }
    }
    let checkpoint_roots = restore_deltas
        .iter()
        .map(|(root, deltas)| (root.clone(), deltas.clone()))
        .collect::<Vec<_>>();
    if persist_checkpoint(
        storage,
        conversation_id,
        "restore_checkpoint",
        &checkpoint_roots,
    )
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
    fn needs_write(&self) -> bool {
        self.current.as_ref().map(|state| state.hash.as_str()) != self.target_hash.as_deref()
            || self.current.is_some() != self.target_exists
    }
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
    let target = plan.root.join(&plan.path);
    let current = match capture_file(&plan.root, &plan.path) {
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
            if atomic_write(&target, content).is_err() {
                return false;
            }
            capture_file(&plan.root, &plan.path).is_ok_and(|state| {
                state.as_ref().map(|snapshot| snapshot.hash.as_str()) == Some(current.hash.as_str())
            })
        }
        None => match fs::remove_file(&target) {
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
                path: plan.path.clone(),
                before: target,
                after: plan.current.clone(),
            });
    }
    for (root, deltas) in grouped {
        if super::record_deltas(storage, conversation_id, &root, deltas).is_err() {
            return false;
        }
    }
    true
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
                    checkpoint.tool_name, file.file_id, file.root, file.path,
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
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<bool>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<bool>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
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
            file_id,
            root,
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
            .map_or(true, |checkpoint| checkpoint.id != id)
        {
            checkpoints.push(Checkpoint {
                id: id.clone(),
                sequence,
                created_at_unix_ms,
                tool_name,
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
            "SELECT file.file_id, file.root, file.path,
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
                    path: row.get(2)?,
                    before_exists: row.get(3)?,
                    before_hash: row.get(4)?,
                    before_blob_hash: row.get(5)?,
                    after_exists: row.get(6)?,
                    after_hash: row.get(7)?,
                    after_blob_hash: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(|_| TrackingError::StorageUnavailable)
}
