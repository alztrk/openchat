//! Text difference computation, hunk extraction, and diff inspection for tracked changes.

use std::{io, path::Path, time::Duration};

use similar::{ChangeTag, TextDiff};

use super::blobs::{cleanup_pending_snapshot_blobs, lock_file_changes, read_baseline};
use super::store::{load_change, migrate_legacy_conversation};
use super::types::*;
use super::workspace::{
    directory_identity, ensure_no_symlink_path_from_directory, open_workspace_directory,
    read_workspace_file,
};
use crate::storage::AppStorage;

pub(crate) fn diff_stats(
    before: &[u8],
    after: &[u8],
) -> (Option<usize>, Option<usize>, bool, bool) {
    let (Some(before), Some(after)) = (as_text(before), as_text(after)) else {
        return (None, None, true, false);
    };
    if before.len().saturating_add(after.len()) > MAX_TEXT_DIFF_BYTES {
        return (None, None, false, false);
    }
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(200))
        .diff_lines(&before, &after);
    let mut added = 0usize;
    let mut removed = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    (Some(added), Some(removed), false, true)
}

/// Reads a bounded text diff against the captured baseline for one change ID.
///
/// Binary, missing, oversized, or otherwise unavailable content is represented
/// as a non-available diff; unsafe paths and storage failures return an error.
pub(crate) fn read_diff(
    storage: &AppStorage,
    conversation_id: &str,
    id: &str,
) -> Result<serde_json::Value, TrackingError> {
    validate_conversation_id(conversation_id)?;
    validate_change_id(id)?;
    let _guard = lock_file_changes()?;
    cleanup_pending_snapshot_blobs(storage)?;
    migrate_legacy_conversation(storage, conversation_id)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let entry =
        load_change(&connection, conversation_id, id)?.ok_or(TrackingError::SnapshotUnavailable)?;
    if !entry.diff_available || entry.root_identity.is_none() {
        return Ok(serde_json::json!({
            "path": entry.path,
            "isBinary": entry.is_binary,
            "available": false,
            "diff": "",
        }));
    }
    let old = read_baseline(storage.root(), &entry)?;
    let (directory, _) = open_workspace_directory(Path::new(&entry.root))?;
    let identity =
        directory_identity(&directory).map_err(|_| TrackingError::WorkspaceUnavailable)?;
    if !identity.matches_storage_value(entry.root_identity.as_deref())? {
        return Err(TrackingError::SnapshotUnavailable);
    }
    let relative = safe_relative_path(&entry.path)?;
    ensure_no_symlink_path_from_directory(&directory, &relative)?;
    let new = match read_workspace_file(&directory, &relative, MAX_SNAPSHOT_FILE_BYTES as usize) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => return Err(TrackingError::SnapshotUnavailable),
    };
    let (Some(old), Some(new)) = (old, new) else {
        return Ok(serde_json::json!({
            "path": entry.path,
            "isBinary": entry.is_binary,
            "available": false,
            "diff": "",
        }));
    };
    let (Some(old_text), Some(new_text)) = (as_text(&old), as_text(&new)) else {
        return Ok(serde_json::json!({
            "path": entry.path,
            "isBinary": true,
            "available": false,
            "diff": "",
        }));
    };
    if old_text.len().saturating_add(new_text.len()) > MAX_TEXT_DIFF_BYTES {
        return Ok(serde_json::json!({
            "path": entry.path,
            "isBinary": false,
            "available": false,
            "diff": "",
        }));
    }
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(200))
        .diff_lines(&old_text, &new_text)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", entry.path), &format!("b/{}", entry.path))
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
    Ok(serde_json::json!({
        "path": entry.path,
        "isBinary": false,
        "available": true,
        "truncated": truncated,
        "diff": displayed_diff,
    }))
}
