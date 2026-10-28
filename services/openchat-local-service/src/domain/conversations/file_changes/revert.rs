//! Safe file restoration, partial hunk reverts, conflict detection, and change approvals.

use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use similar::TextDiff;

use super::blobs::{cleanup_pending_snapshot_blobs, lock_file_changes, read_baseline};
use super::diff::diff_stats;
use super::store::{
    list_changes, load_change, load_changes, migrate_legacy_conversation, store_change,
    store_changes,
};
use super::types::*;
use super::workspace::{
    atomic_write_workspace_file_from_directory, capture_file_from_directory, directory_identity,
    is_current_version, open_workspace_directory, remove_workspace_file_from_directory,
    same_directory_object, workspace_directory_matches_path,
};
use crate::storage::{AppStorage, StorageConnection};

pub(crate) struct RevertFileTarget {
    pub(crate) directory: crate::filesystem::OpenedDirectory,
    pub(crate) canonical_root: PathBuf,
    pub(crate) parent_directory: Option<crate::filesystem::OpenedDirectory>,
    pub(crate) parent_relative: PathBuf,
    pub(crate) file_name: PathBuf,
    pub(crate) directory_identity: DirectoryIdentity,
}

impl RevertFileTarget {
    pub(crate) fn open(entry: &ManifestEntry) -> Result<Self, RevertError> {
        let (directory, canonical_root) = open_workspace_directory(Path::new(&entry.root))
            .map_err(|_| RevertError::Unavailable)?;
        let directory_identity =
            directory_identity(&directory).map_err(|_| RevertError::Unavailable)?;
        let relative = safe_relative_path(&entry.path).map_err(|_| RevertError::Unavailable)?;
        let parent_relative = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let file_name = relative
            .file_name()
            .ok_or(RevertError::Unavailable)?
            .to_os_string()
            .into();
        let parent_directory = if parent_relative.as_os_str().is_empty() {
            Some(
                directory
                    .try_clone()
                    .map_err(|_| RevertError::Unavailable)?,
            )
        } else {
            match crate::filesystem::open_directory_from_directory(&directory, &parent_relative) {
                Ok(parent) => Some(parent),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(_) => return Err(RevertError::Unavailable),
            }
        };
        Ok(Self {
            directory,
            canonical_root,
            parent_directory,
            parent_relative,
            file_name,
            directory_identity,
        })
    }

    pub(crate) fn capture(&self) -> Result<Option<FileSnapshot>, TrackingError> {
        match &self.parent_directory {
            Some(parent) => capture_file_from_directory(parent, &self.file_name),
            None => Ok(None),
        }
    }

    pub(crate) fn matches_entry(&self, entry: &ManifestEntry) -> Result<bool, TrackingError> {
        if !self
            .directory_identity
            .matches_storage_value(entry.root_identity.as_deref())?
            || !self.matches_open_paths()?
        {
            return Ok(false);
        }
        let current = self.capture()?;
        if current
            .as_ref()
            .is_some_and(|snapshot| snapshot.content.is_none())
        {
            return Ok(false);
        }
        Ok(file_snapshot_matches_entry(&current, entry))
    }

    pub(crate) fn matches_open_paths(&self) -> Result<bool, TrackingError> {
        if !workspace_directory_matches_path(&self.canonical_root, &self.directory)? {
            return Ok(false);
        }
        let current_parent = if self.parent_relative.as_os_str().is_empty() {
            self.directory
                .try_clone()
                .map_err(|_| TrackingError::WorkspaceUnavailable)?
        } else {
            match crate::filesystem::open_directory_from_directory(
                &self.directory,
                &self.parent_relative,
            ) {
                Ok(parent) => parent,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(self.parent_directory.is_none());
                }
                Err(_) => return Err(TrackingError::WorkspaceUnavailable),
            }
        };
        match &self.parent_directory {
            Some(expected) => same_directory_object(&current_parent, expected)
                .map_err(|_| TrackingError::WorkspaceUnavailable),
            None => Ok(false),
        }
    }

    pub(crate) fn write_file(&mut self, content: &[u8]) -> io::Result<()> {
        if self.parent_directory.is_none() {
            let parent = crate::filesystem::create_directory_chain_from_directory(
                &self.directory,
                &self.parent_relative,
            )?;
            self.parent_directory = Some(parent);
        }
        let parent = self.parent_directory.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "The workspace file parent is unavailable.",
            )
        })?;
        match self.matches_open_paths() {
            Ok(true) => {}
            Ok(false) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "The workspace file parent changed during the revert.",
                ));
            }
            Err(_) => {
                return Err(io::Error::other(
                    "The workspace file parent could not be verified.",
                ));
            }
        }
        atomic_write_workspace_file_from_directory(parent, &self.file_name, content)
    }

    pub(crate) fn remove_file(&self) -> io::Result<()> {
        let parent = self.parent_directory.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "The workspace file parent is missing.",
            )
        })?;
        match self.matches_open_paths() {
            Ok(true) => {}
            Ok(false) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "The workspace file parent changed during the revert.",
                ));
            }
            Err(_) => {
                return Err(io::Error::other(
                    "The workspace file parent could not be verified.",
                ));
            }
        }
        remove_workspace_file_from_directory(parent, &self.file_name)
    }
}

/// Restores a tracked file to its verified baseline if it has not changed since.
///
/// A concurrent modification returns `RevertError::Conflict` and does not
/// overwrite the newer content.
pub(crate) fn revert_change(
    storage: &AppStorage,
    conversation_id: &str,
    id: &str,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    validate_change_id(id).map_err(|_| RevertError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| RevertError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| RevertError::Unavailable)?;
    migrate_legacy_conversation(storage, conversation_id).map_err(|_| RevertError::Unavailable)?;
    let mut connection = storage.connect().map_err(|_| RevertError::Unavailable)?;
    let mut entry = load_change(&connection, conversation_id, id)
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    if entry.status == "reverted" {
        return Err(RevertError::AlreadyReverted);
    }
    if entry.status == "conflict" {
        entry.status = "conflict".to_owned();
        let transaction = connection
            .transaction()
            .map_err(|_| RevertError::Unavailable)?;
        store_change(&transaction, conversation_id, &entry)
            .map_err(|_| RevertError::Unavailable)?;
        transaction.commit().map_err(|_| RevertError::Unavailable)?;
        return Err(RevertError::Conflict);
    }
    let mut target = RevertFileTarget::open(&entry)?;
    if !target
        .matches_entry(&entry)
        .map_err(|_| RevertError::Unavailable)?
    {
        mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
        return Err(RevertError::Conflict);
    }
    let original = target.capture().map_err(|_| RevertError::Unavailable)?;
    if !file_snapshot_matches_entry(&original, &entry) {
        mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
        return Err(RevertError::Conflict);
    }
    let baseline_exists = entry.baseline_exists;
    let baseline_hash = entry.baseline_hash.clone();
    let baseline_snapshot;
    if baseline_exists {
        let content = read_baseline(storage.root(), &entry)
            .map_err(|_| RevertError::Unavailable)?
            .ok_or(RevertError::Unavailable)?;
        if sha256(&content) != entry.baseline_hash.as_deref().unwrap_or_default() {
            return Err(RevertError::Unavailable);
        }
        baseline_snapshot = Some(FileSnapshot {
            hash: sha256(&content),
            content: Some(content.clone()),
        });
        if !target
            .matches_entry(&entry)
            .map_err(|_| RevertError::Unavailable)?
        {
            mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
            return Err(RevertError::Conflict);
        }
        target.write_file(&content).map_err(|_| {
            if rollback_file_change_if_matches(&mut target, &original, &baseline_snapshot) {
                RevertError::Unavailable
            } else {
                RevertError::RollbackFailed
            }
        })?;
    } else {
        baseline_snapshot = None;
        if !target
            .matches_entry(&entry)
            .map_err(|_| RevertError::Unavailable)?
        {
            mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
            return Err(RevertError::Conflict);
        }
        match target.remove_file() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                return Err(RevertError::Conflict);
            }
            Err(_) => {
                return Err(
                    if rollback_file_change_if_matches(&mut target, &original, &baseline_snapshot) {
                        RevertError::Unavailable
                    } else {
                        RevertError::RollbackFailed
                    },
                );
            }
        }
    }
    entry.status = "reverted".to_owned();
    entry.expected_exists = baseline_exists;
    entry.expected_hash = baseline_hash;
    persist_file_change_or_restore(
        &mut connection,
        conversation_id,
        &entry,
        &mut target,
        &original,
        &baseline_snapshot,
    )?;
    drop(connection);
    let connection = storage
        .connect()
        .map_err(|_| RevertError::AppliedButResponseUnavailable)?;
    load_changes(&connection, conversation_id)
        .map_err(|_| RevertError::AppliedButResponseUnavailable)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| summary_for_entry(storage.root(), entry, true))
                .collect()
        })
}

/// Marks one active file change as approved by locking its current state.
pub(crate) fn approve_change(
    storage: &AppStorage,
    conversation_id: &str,
    id: &str,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    validate_change_id(id).map_err(|_| RevertError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| RevertError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| RevertError::Unavailable)?;
    migrate_legacy_conversation(storage, conversation_id).map_err(|_| RevertError::Unavailable)?;
    let mut connection = storage.connect().map_err(|_| RevertError::Unavailable)?;
    let mut entry = load_change(&connection, conversation_id, id)
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    if entry.status != "active" {
        return Err(RevertError::Unavailable);
    }
    if !is_current_version(&entry).map_err(|_| RevertError::Unavailable)? {
        entry.status = "conflict".to_owned();
        let transaction = connection
            .transaction()
            .map_err(|_| RevertError::Unavailable)?;
        store_change(&transaction, conversation_id, &entry)
            .map_err(|_| RevertError::Unavailable)?;
        transaction.commit().map_err(|_| RevertError::Unavailable)?;
        return Err(RevertError::Conflict);
    }
    entry.baseline_hash = Some(APPROVED_BASELINE_SENTINEL.to_owned());
    let transaction = connection
        .transaction()
        .map_err(|_| RevertError::Unavailable)?;
    store_change(&transaction, conversation_id, &entry).map_err(|_| RevertError::Unavailable)?;
    transaction.commit().map_err(|_| RevertError::Unavailable)?;
    drop(connection);
    let connection = storage
        .connect()
        .map_err(|_| RevertError::AppliedButResponseUnavailable)?;
    load_changes(&connection, conversation_id)
        .map_err(|_| RevertError::AppliedButResponseUnavailable)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| summary_for_entry(storage.root(), entry, true))
                .collect()
        })
}

/// Marks all active, non-conflicting file changes in a conversation as approved.
pub(crate) fn approve_all_changes(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| RevertError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| RevertError::Unavailable)?;
    migrate_legacy_conversation(storage, conversation_id).map_err(|_| RevertError::Unavailable)?;
    let mut connection = storage.connect().map_err(|_| RevertError::Unavailable)?;
    let mut entries =
        load_changes(&connection, conversation_id).map_err(|_| RevertError::Unavailable)?;
    for entry in &mut entries {
        if entry.status == "active"
            && !is_entry_approved(entry)
            && is_current_version(entry).unwrap_or(false)
        {
            entry.baseline_hash = Some(APPROVED_BASELINE_SENTINEL.to_owned());
        }
    }
    let transaction = connection
        .transaction()
        .map_err(|_| RevertError::Unavailable)?;
    store_changes(&transaction, conversation_id, &entries).map_err(|_| RevertError::Unavailable)?;
    transaction.commit().map_err(|_| RevertError::Unavailable)?;
    Ok(entries
        .iter()
        .map(|entry| summary_for_entry(storage.root(), entry, true))
        .collect())
}

/// Reverts all active, non-approved file changes in a conversation back to baseline.
pub(crate) fn revert_all_changes(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    let connection = storage.connect().map_err(|_| RevertError::Unavailable)?;
    let entries =
        load_changes(&connection, conversation_id).map_err(|_| RevertError::Unavailable)?;
    drop(connection);
    let had_revertable_changes = entries
        .iter()
        .any(|entry| entry.status == "active" && !is_entry_approved(entry));
    let mut first_error = None;
    for entry in &entries {
        if entry.status == "active"
            && !is_entry_approved(entry)
            && let Err(error) = revert_change(storage, conversation_id, &entry.id)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    list_changes(storage, conversation_id).map_err(|_| {
        if had_revertable_changes {
            RevertError::AppliedButResponseUnavailable
        } else {
            RevertError::Unavailable
        }
    })
}

/// Reverts a single diff hunk (0-indexed) of a modified text file back to its baseline lines.
pub(crate) fn revert_hunk(
    storage: &AppStorage,
    conversation_id: &str,
    id: &str,
    hunk_index: usize,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    validate_change_id(id).map_err(|_| RevertError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| RevertError::Unavailable)?;
    cleanup_pending_snapshot_blobs(storage).map_err(|_| RevertError::Unavailable)?;
    migrate_legacy_conversation(storage, conversation_id).map_err(|_| RevertError::Unavailable)?;
    let mut connection = storage.connect().map_err(|_| RevertError::Unavailable)?;
    let mut entry = load_change(&connection, conversation_id, id)
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    if entry.status != "active" || is_entry_approved(&entry) || !entry.expected_exists {
        return Err(RevertError::Unavailable);
    }
    let mut target = RevertFileTarget::open(&entry)?;
    if !target
        .matches_entry(&entry)
        .map_err(|_| RevertError::Unavailable)?
    {
        mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
        return Err(RevertError::Conflict);
    }
    let baseline_bytes = read_baseline(storage.root(), &entry)
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    let original = target
        .capture()
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    if !file_snapshot_matches_entry(&Some(original.clone()), &entry) {
        mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
        return Err(RevertError::Conflict);
    }
    let current_bytes = original
        .content
        .as_deref()
        .ok_or(RevertError::Unavailable)?;
    let (Some(old_text), Some(new_text)) = (as_text(&baseline_bytes), as_text(current_bytes))
    else {
        return Err(RevertError::Unavailable);
    };
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(200))
        .diff_lines(&old_text, &new_text);
    let groups = diff.grouped_ops(3);
    let Some(target_group) = groups.get(hunk_index) else {
        return Err(RevertError::Unavailable);
    };
    let old_lines: Vec<&str> = old_text.split_inclusive('\n').collect();
    let new_lines: Vec<&str> = new_text.split_inclusive('\n').collect();
    let mut group_new_start = None;
    let mut group_new_end = 0usize;
    let mut group_old_lines: Vec<&str> = Vec::new();
    for op in target_group {
        if group_new_start.is_none() {
            group_new_start = Some(op.new_range().start);
        }
        group_new_end = op.new_range().end;
        for idx in op.old_range() {
            if let Some(line) = old_lines.get(idx) {
                group_old_lines.push(*line);
            }
        }
    }
    let Some(replace_start) = group_new_start else {
        return Err(RevertError::Unavailable);
    };
    let mut updated_text = String::with_capacity(new_text.len());
    for line in &new_lines[..replace_start.min(new_lines.len())] {
        updated_text.push_str(line);
    }
    for line in &group_old_lines {
        updated_text.push_str(line);
    }
    for line in &new_lines[group_new_end.min(new_lines.len())..] {
        updated_text.push_str(line);
    }
    let updated_bytes = updated_text.as_bytes();
    let updated_snapshot = Some(FileSnapshot {
        hash: sha256(updated_bytes),
        content: Some(updated_bytes.to_vec()),
    });
    if !target
        .matches_entry(&entry)
        .map_err(|_| RevertError::Unavailable)?
    {
        mark_revert_conflict(&mut connection, conversation_id, &mut entry)?;
        return Err(RevertError::Conflict);
    }
    target.write_file(updated_bytes).map_err(|_| {
        if rollback_file_change_if_matches(&mut target, &Some(original.clone()), &updated_snapshot)
        {
            RevertError::Unavailable
        } else {
            RevertError::RollbackFailed
        }
    })?;
    let new_hash = sha256(updated_bytes);
    let (added, removed, is_binary, diff_available) = diff_stats(&baseline_bytes, updated_bytes);
    let mut applied_snapshot = updated_snapshot;
    entry.expected_hash = Some(new_hash.clone());
    entry.added_lines = added;
    entry.removed_lines = removed;
    entry.is_binary = is_binary;
    entry.diff_available = diff_available;
    if entry.baseline_exists && entry.baseline_hash.as_deref() == Some(new_hash.as_str()) {
        entry.status = "reverted".to_owned();
    } else if !entry.baseline_exists && updated_bytes.is_empty() {
        if !revert_target_matches_snapshot(&target, &applied_snapshot).unwrap_or(false)
            || target.remove_file().is_err()
        {
            return Err(
                if rollback_file_change_if_matches(
                    &mut target,
                    &Some(original.clone()),
                    &applied_snapshot,
                ) {
                    RevertError::Unavailable
                } else {
                    RevertError::RollbackFailed
                },
            );
        }
        applied_snapshot = None;
        entry.expected_exists = false;
        entry.expected_hash = None;
        entry.status = "reverted".to_owned();
    }
    persist_file_change_or_restore(
        &mut connection,
        conversation_id,
        &entry,
        &mut target,
        &Some(original),
        &applied_snapshot,
    )?;
    drop(connection);
    let connection = storage
        .connect()
        .map_err(|_| RevertError::AppliedButResponseUnavailable)?;
    load_changes(&connection, conversation_id)
        .map_err(|_| RevertError::AppliedButResponseUnavailable)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| summary_for_entry(storage.root(), entry, true))
                .collect()
        })
}

fn revert_target_matches_snapshot(
    target: &RevertFileTarget,
    expected: &Option<FileSnapshot>,
) -> Result<bool, TrackingError> {
    if !target.matches_open_paths()? {
        return Ok(false);
    }
    Ok(file_states_match(&target.capture()?, expected))
}

fn rollback_file_change_if_matches(
    target: &mut RevertFileTarget,
    original: &Option<FileSnapshot>,
    applied: &Option<FileSnapshot>,
) -> bool {
    if !target.matches_open_paths().unwrap_or(false) {
        return false;
    }
    let current = match target.capture() {
        Ok(current) => current,
        Err(_) => return false,
    };
    if file_states_match(&current, original) {
        return true;
    }
    if !file_states_match(&current, applied) {
        return false;
    }
    let restore_result = match original {
        Some(snapshot) => match snapshot.content.as_deref() {
            Some(content) => target.write_file(content),
            None => return false,
        },
        None => match target.remove_file() {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    };
    restore_result.is_ok() && revert_target_matches_snapshot(target, original).unwrap_or(false)
}

fn mark_revert_conflict(
    connection: &mut StorageConnection<'_>,
    conversation_id: &str,
    entry: &mut ManifestEntry,
) -> Result<(), RevertError> {
    entry.status = "conflict".to_owned();
    let transaction = connection
        .transaction()
        .map_err(|_| RevertError::Unavailable)?;
    store_change(&transaction, conversation_id, entry).map_err(|_| RevertError::Unavailable)?;
    transaction.commit().map_err(|_| RevertError::Unavailable)
}

fn persist_file_change_or_restore(
    connection: &mut StorageConnection<'_>,
    conversation_id: &str,
    entry: &ManifestEntry,
    target: &mut RevertFileTarget,
    original: &Option<FileSnapshot>,
    applied: &Option<FileSnapshot>,
) -> Result<(), RevertError> {
    let persist_result = (|| {
        let transaction = connection
            .transaction()
            .map_err(|_| RevertError::Unavailable)?;
        store_change(&transaction, conversation_id, entry).map_err(|_| RevertError::Unavailable)?;
        transaction.commit().map_err(|_| RevertError::Unavailable)
    })();
    match persist_result {
        Ok(()) => Ok(()),
        Err(error) if rollback_file_change_if_matches(target, original, applied) => Err(error),
        Err(_) => Err(RevertError::RollbackFailed),
    }
}
