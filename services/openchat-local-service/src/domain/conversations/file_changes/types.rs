//! Core data types, errors, constants, and basic helpers for file change tracking.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MANIFEST_VERSION: u8 = 1;
pub(crate) const LEGACY_FILE_CHANGE_DIRECTORY: &str = "file-changes";
pub(crate) const SNAPSHOT_DIRECTORY: &str = "snapshots";
pub(crate) const SNAPSHOT_SHARD_HEX_LENGTH: usize = 2;
pub(crate) const MAX_WORKSPACE_FILES: usize = 20_000;
pub(crate) const MAX_WORKSPACE_DEPTH: usize = 256;
pub(crate) const MAX_WORKSPACE_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_WORKSPACE_SNAPSHOT_CONTENT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_SNAPSHOT_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_TEXT_DIFF_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_DIFF_OUTPUT_BYTES: usize = 256 * 1024;
pub(crate) const MAX_DIFF_LINES: usize = 4_000;
pub(crate) const MAX_LEGACY_MANIFEST_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const APPROVED_BASELINE_SENTINEL: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// UI-facing summary of one tracked project file change.
pub(crate) struct FileChangeSummary {
    /// Stable identifier used to request a diff or revert this change.
    pub id: String,
    /// Validated project-relative file path.
    pub path: String,
    /// Change category, such as added, modified, or deleted.
    pub kind: String,
    /// Number of added text lines when available.
    pub added_lines: Option<usize>,
    /// Number of removed text lines when available.
    pub removed_lines: Option<usize>,
    /// Whether the change contains binary content.
    pub is_binary: bool,
    /// Whether a bounded textual diff can be read.
    pub diff_available: bool,
    /// Current tracking state, including conflict and reverted states.
    pub status: String,
    /// Whether reverting can be offered for this state.
    pub can_revert: bool,
    /// Whether the user has explicitly approved this file change.
    #[serde(default)]
    pub approved: bool,
}

#[derive(Clone, Debug)]
/// Hash and optional bounded file bytes captured for change comparison.
pub(crate) struct FileSnapshot {
    /// SHA-256 digest of the captured file content.
    pub hash: String,
    /// File content when the file is within the snapshot size limit.
    pub content: Option<Vec<u8>>,
}

#[derive(Clone)]
/// Captured file states for one canonical workspace root.
pub(crate) struct WorkspaceSnapshot {
    /// Canonical project root from which the relative file paths were collected.
    pub root: PathBuf,
    pub(crate) directory_identity: DirectoryIdentity,
    /// Snapshot entries keyed by validated workspace-relative path.
    pub files: BTreeMap<String, FileSnapshot>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity {
    pub(crate) volume_serial: u32,
    pub(crate) file_index_high: u32,
    pub(crate) file_index_low: u32,
    pub(crate) creation_time_high: u32,
    pub(crate) creation_time_low: u32,
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity;

impl DirectoryIdentity {
    pub(crate) fn storage_value(self) -> Result<String, TrackingError> {
        #[cfg(unix)]
        {
            Ok(format!("unix-v1:{:016x}:{:016x}", self.device, self.inode))
        }
        #[cfg(windows)]
        {
            Ok(format!(
                "windows-v1:{:08x}:{:08x}:{:08x}:{:08x}:{:08x}",
                self.volume_serial,
                self.file_index_high,
                self.file_index_low,
                self.creation_time_high,
                self.creation_time_low
            ))
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = self;
            Err(TrackingError::WorkspaceUnavailable)
        }
    }

    pub(crate) fn matches_storage_value(self, value: Option<&str>) -> Result<bool, TrackingError> {
        Ok(value == Some(self.storage_value()?.as_str()))
    }
}

#[derive(Clone, Debug)]
/// Before-and-after state for a single changed workspace file.
pub(crate) struct FileChangeDelta {
    /// OS identity of the workspace root from which both file states were captured.
    pub root_identity: DirectoryIdentity,
    /// Validated workspace-relative path.
    pub path: String,
    /// Baseline state, or `None` when the file did not exist before.
    pub before: Option<FileSnapshot>,
    /// Current state, or `None` when the file has been deleted.
    pub after: Option<FileSnapshot>,
}

#[derive(Debug)]
/// Failure categories for file-change capture, storage, and recovery.
pub(crate) enum TrackingError {
    /// The workspace root or a tracked file could not be safely accessed.
    WorkspaceUnavailable,
    /// The workspace exceeded a configured scan or snapshot limit.
    WorkspaceTooLarge,
    /// A required baseline or snapshot could not be read or verified.
    SnapshotUnavailable,
    /// The local tracking database could not be read or updated.
    StorageUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Failure categories for a user-requested file change revert.
pub(crate) enum RevertError {
    /// The change was already reverted.
    AlreadyReverted,
    /// The current file no longer matches the recorded version.
    Conflict,
    /// The file-change update committed, but its response state could not be loaded.
    AppliedButResponseUnavailable,
    /// A file was changed, but restoring it after a later failure was unsuccessful.
    RollbackFailed,
    /// The baseline, path, or local storage could not be safely accessed.
    Unavailable,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Manifest {
    pub(crate) version: u8,
    pub(crate) files: Vec<ManifestEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestEntry {
    pub(crate) id: String,
    pub(crate) root: String,
    #[serde(default)]
    pub(crate) root_identity: Option<String>,
    pub(crate) path: String,
    pub(crate) baseline_exists: bool,
    pub(crate) baseline_hash: Option<String>,
    #[serde(rename = "backupFile", default)]
    pub(crate) baseline_blob_hash: Option<String>,
    pub(crate) expected_exists: bool,
    pub(crate) expected_hash: Option<String>,
    pub(crate) added_lines: Option<usize>,
    pub(crate) removed_lines: Option<usize>,
    pub(crate) is_binary: bool,
    pub(crate) diff_available: bool,
    pub(crate) status: String,
}

pub(crate) fn is_entry_approved(entry: &ManifestEntry) -> bool {
    entry.status == "active" && entry.baseline_hash.as_deref() == Some(APPROVED_BASELINE_SENTINEL)
}

pub(crate) fn summary_for_entry(
    data_root: &Path,
    entry: &ManifestEntry,
    validate_current: bool,
) -> FileChangeSummary {
    let mut status = entry.status.clone();
    let approved = is_entry_approved(entry);
    let expected_matches =
        !validate_current || super::workspace::is_current_version(entry).unwrap_or(false);
    if validate_current && !expected_matches && status != "reverted" {
        status = "conflict".to_owned();
    }
    let backup_available = !entry.baseline_exists
        || entry
            .baseline_blob_hash
            .as_deref()
            .is_some_and(|hash| super::blobs::snapshot_blob_exists(data_root, hash));
    let can_revert = !approved && status == "active" && expected_matches && backup_available;
    let kind = if !entry.baseline_exists {
        "added"
    } else if !entry.expected_exists {
        "deleted"
    } else {
        "modified"
    };
    FileChangeSummary {
        id: entry.id.clone(),
        path: entry.path.clone(),
        kind: kind.to_owned(),
        added_lines: entry.added_lines,
        removed_lines: entry.removed_lines,
        is_binary: entry.is_binary,
        diff_available: entry.diff_available,
        status,
        can_revert,
        approved,
    }
}

pub(crate) fn file_snapshot_matches_entry(
    snapshot: &Option<FileSnapshot>,
    entry: &ManifestEntry,
) -> bool {
    snapshot.is_some() == entry.expected_exists
        && snapshot.as_ref().map(|state| state.hash.as_str()) == entry.expected_hash.as_deref()
}

pub(crate) fn file_states_match(left: &Option<FileSnapshot>, right: &Option<FileSnapshot>) -> bool {
    left.is_some() == right.is_some()
        && left.as_ref().map(|state| state.hash.as_str())
            == right.as_ref().map(|state| state.hash.as_str())
}

pub(crate) fn as_text(content: &[u8]) -> Option<String> {
    if content.contains(&0) {
        return None;
    }
    String::from_utf8(content.to_vec()).ok()
}

pub(crate) fn safe_relative_path(path: &str) -> Result<PathBuf, TrackingError> {
    let relative = Path::new(path);
    if path.trim().is_empty()
        || relative.is_absolute()
        || path.contains(':')
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(name) if !name.is_empty()))
    {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(relative.to_path_buf())
}

pub(crate) fn validate_conversation_id(conversation_id: &str) -> Result<(), TrackingError> {
    if conversation_id.is_empty()
        || conversation_id.len() > 256
        || conversation_id.chars().any(char::is_control)
    {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(())
}

pub(crate) fn validate_change_id(id: &str) -> Result<(), TrackingError> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(())
}

pub(crate) fn change_id(root: &Path, path: &str) -> String {
    let value = format!("{}\0{path}", root.to_string_lossy());
    sha256(value.as_bytes())
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn unix_time_millis() -> Result<i64, TrackingError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TrackingError::StorageUnavailable)?
        .as_millis();
    i64::try_from(millis).map_err(|_| TrackingError::StorageUnavailable)
}
