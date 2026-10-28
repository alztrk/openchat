//! Captures and reads file-change snapshots produced during conversation tools.
//!
//! Snapshot blobs are content-addressed under application storage, while
//! manifest paths are validated relative paths and writes use atomic replacement.
use crate::filesystem::OpenedFile as CapabilityFile;
use crate::storage::{AppStorage, StorageConnection, StorageRow, StorageTransaction};
use cap_std::fs::{Dir, Metadata as CapabilityMetadata, OpenOptions as CapabilityOpenOptions};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[path = "file_changes/checkpoints.rs"]
pub(crate) mod checkpoints;
const MANIFEST_VERSION: u8 = 1;
const LEGACY_FILE_CHANGE_DIRECTORY: &str = "file-changes";
const SNAPSHOT_DIRECTORY: &str = "snapshots";
const SNAPSHOT_SHARD_HEX_LENGTH: usize = 2;
const MAX_WORKSPACE_FILES: usize = 20_000;
const MAX_WORKSPACE_DEPTH: usize = 256;
const MAX_WORKSPACE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WORKSPACE_SNAPSHOT_CONTENT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SNAPSHOT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXT_DIFF_BYTES: usize = 1024 * 1024;
const MAX_DIFF_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_DIFF_LINES: usize = 4_000;
const MAX_LEGACY_MANIFEST_BYTES: usize = 32 * 1024 * 1024;
const APPROVED_BASELINE_SENTINEL: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
static FILE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static FILE_CHANGE_LOCK_WAIT_HOOK: OnceLock<
    Mutex<Option<(std::thread::ThreadId, std::sync::mpsc::SyncSender<()>)>>,
> = OnceLock::new();
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
    directory_identity: DirectoryIdentity,
    /// Snapshot entries keyed by validated workspace-relative path.
    pub files: BTreeMap<String, FileSnapshot>,
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity {
    device: u64,
    inode: u64,
}
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity {
    volume_serial: u32,
    file_index_high: u32,
    file_index_low: u32,
    creation_time_high: u32,
    creation_time_low: u32,
}
#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryIdentity;
impl DirectoryIdentity {
    fn storage_value(self) -> Result<String, TrackingError> {
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
    fn matches_storage_value(self, value: Option<&str>) -> Result<bool, TrackingError> {
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
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: u8,
    files: Vec<ManifestEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    id: String,
    root: String,
    #[serde(default)]
    root_identity: Option<String>,
    path: String,
    baseline_exists: bool,
    baseline_hash: Option<String>,
    #[serde(rename = "backupFile", default)]
    baseline_blob_hash: Option<String>,
    expected_exists: bool,
    expected_hash: Option<String>,
    added_lines: Option<usize>,
    removed_lines: Option<usize>,
    is_binary: bool,
    diff_available: bool,
    status: String,
}
/// Captures one regular file after validating its relative path and ancestors.
///
/// Missing files return `Ok(None)`; oversized regular files retain only their
/// digest, while unsafe paths, symlinks, or unreadable files return an error.
#[cfg(test)]
pub(crate) fn capture_file(
    root: &Path,
    relative_path: &str,
) -> Result<Option<FileSnapshot>, TrackingError> {
    let relative = safe_relative_path(relative_path)?;
    let (directory, _) = open_workspace_directory(root)?;
    capture_file_from_directory(&directory, &relative)
}
pub(crate) fn capture_file_with_root_identity_from_directory(
    directory: &crate::filesystem::OpenedDirectory,
    relative_path: &Path,
) -> Result<(DirectoryIdentity, Option<FileSnapshot>), TrackingError> {
    let identity =
        directory_identity(directory).map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let snapshot = capture_file_from_directory(directory, relative_path)?;
    Ok((identity, snapshot))
}
pub(crate) fn capture_file_from_directory<D>(
    directory: &D,
    relative_path: &Path,
) -> Result<Option<FileSnapshot>, TrackingError>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    crate::filesystem::validate_relative_path(relative_path)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    match open_workspace_file(directory, relative_path) {
        Ok((file, metadata)) => snapshot_file(file, metadata.len()).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(TrackingError::SnapshotUnavailable),
    }
}
pub(crate) fn open_workspace_directory(
    root: &Path,
) -> Result<(crate::filesystem::OpenedDirectory, PathBuf), TrackingError> {
    let directory = crate::filesystem::open_absolute_directory_no_follow(root)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let canonical_directory = crate::filesystem::open_absolute_directory_no_follow(&canonical_root)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    if !crate::filesystem::same_directory_object(&directory, &canonical_directory)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?
    {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok((directory, canonical_root))
}
fn same_directory_object(left: &Dir, right: &Dir) -> io::Result<bool> {
    crate::filesystem::same_directory_object(left, right)
}
fn directory_identity(directory: &Dir) -> io::Result<DirectoryIdentity> {
    let file = directory.try_clone()?.into_std_file();
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        Ok(DirectoryIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };
        let mut information = BY_HANDLE_FILE_INFORMATION::default(); // SAFETY: The cloned file owns a live Windows handle and the information output is writable.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(DirectoryIdentity {
            volume_serial: information.dwVolumeSerialNumber,
            file_index_high: information.nFileIndexHigh,
            file_index_low: information.nFileIndexLow,
            creation_time_high: information.ftCreationTime.dwHighDateTime,
            creation_time_low: information.ftCreationTime.dwLowDateTime,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "workspace directory identity is not supported on this platform",
        ))
    }
}
pub(crate) fn workspace_directory_identity(
    directory: &crate::filesystem::OpenedDirectory,
) -> Result<DirectoryIdentity, TrackingError> {
    directory_identity(directory).map_err(|_| TrackingError::WorkspaceUnavailable)
}
/// Returns whether a canonical workspace path still names the opened directory object.
pub(crate) fn workspace_directory_matches_path(
    root: &Path,
    expected: &Dir,
) -> Result<bool, TrackingError> {
    let (actual, canonical_root) = open_workspace_directory(root)?;
    if canonical_root != root {
        return Ok(false);
    }
    same_directory_object(expected, &actual).map_err(|_| TrackingError::WorkspaceUnavailable)
}
#[cfg(test)]
pub(crate) fn workspace_root_identity(root: &Path) -> Result<DirectoryIdentity, TrackingError> {
    let (directory, _) = open_workspace_directory(root)?;
    workspace_directory_identity(&directory)
}
pub(crate) fn ensure_no_symlink_path_from_directory(
    directory: &Dir,
    relative_path: &Path,
) -> Result<(), TrackingError> {
    crate::filesystem::validate_relative_path(relative_path)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let mut current = PathBuf::new();
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(TrackingError::WorkspaceUnavailable);
        };
        current.push(name);
        match directory.symlink_metadata(&current) {
            Ok(metadata) if crate::filesystem::metadata_is_reparse_point(&metadata) => {
                return Err(TrackingError::WorkspaceUnavailable);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(TrackingError::SnapshotUnavailable),
        }
    }
    Ok(())
}
/// Captures a bounded workspace snapshot without following symbolic links.
pub(crate) fn capture_workspace(root: &Path) -> Result<WorkspaceSnapshot, TrackingError> {
    let (directory, root) = open_workspace_directory(root)?;
    let directory_identity =
        directory_identity(&directory).map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let mut files = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut total_snapshot_content_bytes = 0u64;
    collect_workspace_files(
        &directory,
        Path::new(""),
        0,
        &mut files,
        &mut total_bytes,
        &mut total_snapshot_content_bytes,
    )?;
    Ok(WorkspaceSnapshot {
        root,
        directory_identity,
        files,
    })
}
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
        .collect::<std::collections::BTreeSet<_>>();
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
///
/// The database update is transactional; a root mismatch or unavailable storage
/// returns an error without reporting a successful change record.
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
///
/// Every root and relative path is validated before snapshot processing starts.
/// The open directory capabilities remain alive until the single transaction is
/// ready to commit, so a failed validation cannot leave only some roots tracked.
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
            }); // A stale identity cannot provide a baseline, so reuse its path slot only with this delta's captured before-state.
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
        return Ok(
            serde_json::json!({            "path": entry.path,            "isBinary": entry.is_binary,            "available": false,            "diff": "",        }),
        );
    };
    let (Some(old_text), Some(new_text)) = (as_text(&old), as_text(&new)) else {
        return Ok(
            serde_json::json!({            "path": entry.path,            "isBinary": true,            "available": false,            "diff": "",        }),
        );
    };
    if old_text.len().saturating_add(new_text.len()) > MAX_TEXT_DIFF_BYTES {
        return Ok(
            serde_json::json!({            "path": entry.path,            "isBinary": false,            "available": false,            "diff": "",        }),
        );
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
    Ok(
        serde_json::json!({        "path": entry.path,        "isBinary": false,        "available": true,        "truncated": truncated,        "diff": displayed_diff,    }),
    )
}

struct RevertFileTarget {
    directory: crate::filesystem::OpenedDirectory,
    canonical_root: PathBuf,
    parent_directory: Option<crate::filesystem::OpenedDirectory>,
    parent_relative: PathBuf,
    file_name: PathBuf,
    directory_identity: DirectoryIdentity,
}

impl RevertFileTarget {
    fn open(entry: &ManifestEntry) -> Result<Self, RevertError> {
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

    fn capture(&self) -> Result<Option<FileSnapshot>, TrackingError> {
        match &self.parent_directory {
            Some(parent) => capture_file_from_directory(parent, &self.file_name),
            None => Ok(None),
        }
    }

    fn matches_entry(&self, entry: &ManifestEntry) -> Result<bool, TrackingError> {
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

    fn matches_open_paths(&self) -> Result<bool, TrackingError> {
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

    fn write_file(&mut self, content: &[u8]) -> io::Result<()> {
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

    fn remove_file(&self) -> io::Result<()> {
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
fn file_snapshot_matches_entry(snapshot: &Option<FileSnapshot>, entry: &ManifestEntry) -> bool {
    snapshot.is_some() == entry.expected_exists
        && snapshot.as_ref().map(|state| state.hash.as_str()) == entry.expected_hash.as_deref()
}
fn file_states_match(left: &Option<FileSnapshot>, right: &Option<FileSnapshot>) -> bool {
    left.is_some() == right.is_some()
        && left.as_ref().map(|state| state.hash.as_str())
            == right.as_ref().map(|state| state.hash.as_str())
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
fn collect_workspace_files(
    directory: &Dir,
    relative_directory: &Path,
    depth: usize,
    files: &mut BTreeMap<String, FileSnapshot>,
    total_bytes: &mut u64,
    total_snapshot_content_bytes: &mut u64,
) -> Result<(), TrackingError> {
    if depth > MAX_WORKSPACE_DEPTH {
        return Err(TrackingError::WorkspaceTooLarge);
    }
    let entries = directory
        .entries()
        .map_err(|_| TrackingError::SnapshotUnavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| TrackingError::SnapshotUnavailable)?;
        let file_type = entry
            .file_type()
            .map_err(|_| TrackingError::SnapshotUnavailable)?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| TrackingError::SnapshotUnavailable)?;
        if file_type.is_dir() {
            if crate::tools::is_default_ignored_directory(&name) {
                continue;
            }
            let child_directory =
                crate::filesystem::open_directory_from_directory(directory, Path::new(&name))
                    .map_err(|_| TrackingError::SnapshotUnavailable)?;
            collect_workspace_files(
                &child_directory,
                &relative_directory.join(&name),
                depth.saturating_add(1),
                files,
                total_bytes,
                total_snapshot_content_bytes,
            )?;
        } else if file_type.is_file() {
            if files.len() >= MAX_WORKSPACE_FILES {
                return Err(TrackingError::WorkspaceTooLarge);
            }
            let mut options = CapabilityOpenOptions::new();
            options.read(true);
            let (file, metadata) = crate::filesystem::open_regular_file_from_directory(
                directory,
                Path::new(&name),
                &options,
            )
            .map_err(|_| TrackingError::SnapshotUnavailable)?;
            if crate::filesystem::file_has_multiple_links(&file)
                .map_err(|_| TrackingError::SnapshotUnavailable)?
            {
                return Err(TrackingError::SnapshotUnavailable);
            }
            *total_bytes = total_bytes.saturating_add(metadata.len());
            if *total_bytes > MAX_WORKSPACE_BYTES {
                return Err(TrackingError::WorkspaceTooLarge);
            }
            if metadata.len() <= MAX_SNAPSHOT_FILE_BYTES {
                *total_snapshot_content_bytes =
                    total_snapshot_content_bytes.saturating_add(metadata.len());
                if *total_snapshot_content_bytes > MAX_WORKSPACE_SNAPSHOT_CONTENT_BYTES {
                    return Err(TrackingError::WorkspaceTooLarge);
                }
            }
            let snapshot = snapshot_file(file, metadata.len())?;
            let relative_path = relative_directory
                .join(&name)
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative_path, snapshot);
        }
    }
    Ok(())
}
fn snapshot_file(file: impl Read, size: u64) -> Result<FileSnapshot, TrackingError> {
    if size > MAX_WORKSPACE_BYTES {
        return Err(TrackingError::WorkspaceTooLarge);
    }
    let mut hasher = Sha256::new();
    let mut content = (size <= MAX_SNAPSHOT_FILE_BYTES).then(|| Vec::with_capacity(size as usize));
    let mut file = file.take(MAX_WORKSPACE_BYTES + 1);
    let mut bytes_read = 0_u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| TrackingError::SnapshotUnavailable)?;
        if read == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(read as u64);
        if bytes_read > MAX_WORKSPACE_BYTES {
            return Err(TrackingError::WorkspaceTooLarge);
        }
        hasher.update(&buffer[..read]);
        let retained_content_exceeds_size = content
            .as_ref()
            .is_some_and(|bytes| bytes.len().saturating_add(read) > size as usize);
        if retained_content_exceeds_size {
            content = None;
        } else if let Some(bytes) = content.as_mut() {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    Ok(FileSnapshot {
        hash: format!("{:x}", hasher.finalize()),
        content,
    })
}
fn read_bounded_file(path: &Path, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let file = fs::File::open(path)?;
    let mut limited_file = file.take(max_bytes as u64 + 1);
    let mut bytes = Vec::new();
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}
fn open_workspace_file<D>(
    directory: &D,
    relative_path: &Path,
) -> io::Result<(CapabilityFile, CapabilityMetadata)>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    let mut options = CapabilityOpenOptions::new();
    options.read(true);
    crate::filesystem::open_regular_file_from_directory(directory, relative_path, &options)
}
fn read_workspace_file<D>(
    directory: &D,
    relative_path: &Path,
    max_bytes: usize,
) -> io::Result<Option<Vec<u8>>>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    let bytes =
        crate::filesystem::read_bounded_file_from_directory(directory, relative_path, max_bytes)?;
    if bytes.len() > max_bytes {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}
fn atomic_write_workspace_file_from_directory<D>(
    directory: &D,
    relative_path: &Path,
    content: &[u8],
) -> io::Result<()>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    crate::filesystem::validate_relative_path(relative_path)?;
    let parent_path = relative_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = match parent_path {
        Some(parent_path) => {
            crate::filesystem::create_directory_chain_from_directory(directory, parent_path)?
        }
        None => crate::filesystem::DirectoryCapability::try_clone_chain(directory)?,
    };
    let file_name = relative_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The workspace path has no file name.",
        )
    })?;
    let temporary = PathBuf::from(format!(".openchat-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = CapabilityOpenOptions::new();
    options.write(true).create_new(true);
    let mut file = parent.open_with(&temporary, &options)?;
    if let Err(error) = file.write_all(content).and_then(|()| file.sync_all()) {
        drop(file);
        return Err(remove_workspace_temp_after_error(
            &parent, &temporary, error,
        ));
    }
    drop(file);
    parent
        .rename(&temporary, &parent, file_name)
        .map_err(|error| remove_workspace_temp_after_error(&parent, &temporary, error))
}
pub(crate) fn remove_workspace_file_from_directory<D>(
    directory: &D,
    relative_path: &Path,
) -> io::Result<()>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    crate::filesystem::validate_relative_path(relative_path)?;
    let parent_path = relative_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = match parent_path {
        Some(parent_path) => {
            crate::filesystem::open_directory_from_directory(directory, parent_path)?
        }
        None => crate::filesystem::DirectoryCapability::try_clone_chain(directory)?,
    };
    let file_name = relative_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The workspace path has no file name.",
        )
    })?;
    let metadata = parent.symlink_metadata(file_name)?;
    if crate::filesystem::metadata_is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The workspace entry is not a regular file.",
        ));
    }
    parent.remove_file(file_name)
}
fn remove_workspace_temp_after_error(
    directory: &Dir,
    temporary: &Path,
    operation_error: io::Error,
) -> io::Error {
    match directory.remove_file(temporary) {
        Ok(()) => operation_error,
        Err(error) if error.kind() == io::ErrorKind::NotFound => operation_error,
        Err(cleanup_error) => io::Error::other(format!(
            "{operation_error}; temporary file cleanup failed: {cleanup_error}"
        )),
    }
}
fn is_entry_approved(entry: &ManifestEntry) -> bool {
    entry.status == "active" && entry.baseline_hash.as_deref() == Some(APPROVED_BASELINE_SENTINEL)
}
fn summary_for_entry(
    data_root: &Path,
    entry: &ManifestEntry,
    validate_current: bool,
) -> FileChangeSummary {
    let mut status = entry.status.clone();
    let approved = is_entry_approved(entry);
    let expected_matches = !validate_current || is_current_version(entry).unwrap_or(false);
    if validate_current && !expected_matches && status != "reverted" {
        status = "conflict".to_owned();
    }
    let backup_available = !entry.baseline_exists
        || entry
            .baseline_blob_hash
            .as_deref()
            .is_some_and(|hash| snapshot_blob_exists(data_root, hash));
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
fn is_current_version(entry: &ManifestEntry) -> Result<bool, TrackingError> {
    let (directory, _) = open_workspace_directory(Path::new(&entry.root))?;
    let relative = safe_relative_path(&entry.path)?;
    is_current_version_from_directory(entry, &directory, &relative)
}
fn is_current_version_from_directory(
    entry: &ManifestEntry,
    directory: &Dir,
    relative: &Path,
) -> Result<bool, TrackingError> {
    let identity =
        directory_identity(directory).map_err(|_| TrackingError::WorkspaceUnavailable)?;
    if !identity.matches_storage_value(entry.root_identity.as_deref())? {
        return Ok(false);
    }
    match ensure_no_symlink_path_from_directory(directory, relative) {
        Ok(()) => {}
        Err(TrackingError::WorkspaceUnavailable) => return Ok(false),
        Err(error) => return Err(error),
    }
    match open_workspace_file(directory, relative) {
        Ok((file, metadata)) if metadata.len() <= MAX_SNAPSHOT_FILE_BYTES => {
            let current = snapshot_file(file, metadata.len())?;
            Ok(entry.expected_exists && Some(current.hash) == entry.expected_hash)
        }
        Ok((_, metadata)) if metadata.len() > MAX_SNAPSHOT_FILE_BYTES => Ok(false),
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(!entry.expected_exists),
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(false),
        Err(_) => Err(TrackingError::SnapshotUnavailable),
    }
}
fn read_baseline(
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
fn load_changes(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
) -> Result<Vec<ManifestEntry>, TrackingError> {
    let mut statement = connection        .prepare(            "SELECT id, root, root_identity, path, baseline_exists, baseline_hash, baseline_blob_hash,                    expected_exists, expected_hash, added_lines, removed_lines,                    is_binary, diff_available, status             FROM conversation_file_changes             WHERE conversation_id = ?1             ORDER BY path, id",        )        .map_err(|_| TrackingError::StorageUnavailable)?;
    let rows = statement
        .query_map([conversation_id], manifest_entry_from_row)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| TrackingError::StorageUnavailable)
}
fn load_change(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    id: &str,
) -> Result<Option<ManifestEntry>, TrackingError> {
    connection        .query_row(            "SELECT id, root, root_identity, path, baseline_exists, baseline_hash, baseline_blob_hash,                    expected_exists, expected_hash, added_lines, removed_lines,                    is_binary, diff_available, status             FROM conversation_file_changes             WHERE conversation_id = ?1 AND id = ?2",            params![conversation_id, id],            manifest_entry_from_row,        )        .optional()        .map_err(|_| TrackingError::StorageUnavailable)
}
fn manifest_entry_from_row(row: &StorageRow) -> rusqlite::Result<ManifestEntry> {
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
fn store_changes(
    transaction: &StorageTransaction<'_>,
    conversation_id: &str,
    entries: &[ManifestEntry],
) -> Result<(), TrackingError> {
    for entry in entries {
        store_change(transaction, conversation_id, entry)?;
    }
    Ok(())
}
fn store_change(
    transaction: &StorageTransaction<'_>,
    conversation_id: &str,
    entry: &ManifestEntry,
) -> Result<(), TrackingError> {
    if let Some(hash) = entry.baseline_blob_hash.as_deref() {
        transaction            .execute(                "UPDATE file_change_blobs SET state = 'ready' WHERE hash = ?1                 AND state IN ('writing', 'ready')",                [hash],            )            .map_err(|_| TrackingError::StorageUnavailable)?;
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
    transaction        .execute(            "INSERT INTO conversation_file_changes (                conversation_id, id, root, root_identity, path, baseline_exists, baseline_hash,                baseline_blob_hash, expected_exists, expected_hash, added_lines,                removed_lines, is_binary, diff_available, status             ) VALUES (                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15             ) ON CONFLICT(conversation_id, id) DO UPDATE SET                root = excluded.root,                root_identity = excluded.root_identity,                path = excluded.path,                baseline_exists = excluded.baseline_exists,                baseline_hash = excluded.baseline_hash,                baseline_blob_hash = excluded.baseline_blob_hash,                expected_exists = excluded.expected_exists,                expected_hash = excluded.expected_hash,                added_lines = excluded.added_lines,                removed_lines = excluded.removed_lines,                is_binary = excluded.is_binary,                diff_available = excluded.diff_available,                status = excluded.status",            params![                conversation_id,                entry.id,                entry.root,                entry.root_identity,                entry.path,                entry.baseline_exists,                entry.baseline_hash,                entry.baseline_blob_hash,                entry.expected_exists,                entry.expected_hash,                added_lines,                removed_lines,                entry.is_binary,                entry.diff_available,                entry.status,            ],        )        .map_err(|_| TrackingError::StorageUnavailable)?;
    Ok(())
}
fn ensure_snapshot_blob(storage: &AppStorage, content: &[u8]) -> Result<String, TrackingError> {
    if content.len() > MAX_SNAPSHOT_FILE_BYTES as usize {
        return Err(TrackingError::SnapshotUnavailable);
    }
    let hash = sha256(content);
    let path = snapshot_blob_path(storage.root(), &hash, true)?
        .ok_or(TrackingError::StorageUnavailable)?;
    let connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    connection        .execute(            "INSERT INTO file_change_blobs (hash, size_bytes, state, created_at_unix_ms)             VALUES (?1, ?2, 'writing', ?3)             ON CONFLICT(hash) DO UPDATE SET                state = CASE WHEN file_change_blobs.state = 'pending'                             THEN 'writing' ELSE file_change_blobs.state END,                created_at_unix_ms = CASE WHEN file_change_blobs.state = 'pending'                                          THEN excluded.created_at_unix_ms                                          ELSE file_change_blobs.created_at_unix_ms END",            params![                hash,                i64::try_from(content.len()).map_err(|_| TrackingError::StorageUnavailable)?,                unix_time_millis()?,            ],        )        .map_err(|_| TrackingError::StorageUnavailable)?;
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
fn read_snapshot_blob(data_root: &Path, hash: &str) -> Result<Vec<u8>, TrackingError> {
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
fn snapshot_blob_exists(data_root: &Path, hash: &str) -> bool {
    let Ok(Some(path)) = snapshot_blob_path(data_root, hash, false) else {
        return false;
    };
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.len() <= MAX_SNAPSHOT_FILE_BYTES
    })
}
fn snapshot_blob_path(
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
fn cleanup_pending_snapshot_blobs(storage: &AppStorage) -> Result<(), TrackingError> {
    let mut connection = storage
        .connect()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let stale_writing_before = unix_time_millis()?.saturating_sub(5 * 60 * 1000);
    let hashes = {
        let mut statement = transaction            .prepare(                "SELECT hash FROM file_change_blobs AS blobs                 WHERE (state = 'pending' OR (state = 'writing' AND created_at_unix_ms < ?1))                   AND NOT EXISTS (                    SELECT 1 FROM conversation_file_changes AS changes                    WHERE changes.baseline_blob_hash = blobs.hash                 )                   AND NOT EXISTS (                    SELECT 1 FROM conversation_file_checkpoint_files AS files                    WHERE files.before_blob_hash = blobs.hash OR files.after_blob_hash = blobs.hash                 )",            )            .map_err(|_| TrackingError::StorageUnavailable)?;
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
        transaction            .execute(                "DELETE FROM file_change_blobs WHERE hash = ?1                 AND (state = 'pending' OR (state = 'writing' AND created_at_unix_ms < ?2))                 AND NOT EXISTS (                    SELECT 1 FROM conversation_file_changes                    WHERE baseline_blob_hash = ?1                 )                 AND NOT EXISTS (                    SELECT 1 FROM conversation_file_checkpoint_files                    WHERE before_blob_hash = ?1 OR after_blob_hash = ?1                 )",                params![hash, stale_writing_before],            )            .map_err(|_| TrackingError::StorageUnavailable)?;
    }
    transaction
        .commit()
        .map_err(|_| TrackingError::StorageUnavailable)
}
fn unix_time_millis() -> Result<i64, TrackingError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TrackingError::StorageUnavailable)?
        .as_millis();
    i64::try_from(millis).map_err(|_| TrackingError::StorageUnavailable)
}
fn migrate_legacy_conversation(
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
    transaction        .execute(            "INSERT INTO file_change_legacy_imports (conversation_id, imported_at_unix_ms)             VALUES (?1, ?2)",            params![conversation_id, unix_time_millis()?],        )        .map_err(|_| TrackingError::StorageUnavailable)?;
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
fn diff_stats(before: &[u8], after: &[u8]) -> (Option<usize>, Option<usize>, bool, bool) {
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
fn as_text(content: &[u8]) -> Option<String> {
    if content.contains(&0) {
        return None;
    }
    String::from_utf8(content.to_vec()).ok()
}
fn ensure_child_directory(
    parent: &Path,
    child_name: &str,
    create: bool,
) -> Result<Option<PathBuf>, TrackingError> {
    let path = parent.join(child_name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(TrackingError::StorageUnavailable);
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&path).map_err(|_| TrackingError::StorageUnavailable)?;
        }
        Err(_) => return Err(TrackingError::StorageUnavailable),
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    if !canonical.starts_with(parent) {
        return Err(TrackingError::StorageUnavailable);
    }
    Ok(Some(canonical))
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
fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".openchat-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    if let Err(error) = file.write_all(content).and_then(|()| file.sync_all()) {
        drop(file);
        return Err(remove_temp_after_error(&temp_path, error));
    }
    drop(file);
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => Err(remove_temp_after_error(&temp_path, error)),
    }
}
fn remove_temp_after_error(path: &Path, operation_error: io::Error) -> io::Error {
    match fs::remove_file(path) {
        Ok(()) => operation_error,
        Err(error) if error.kind() == io::ErrorKind::NotFound => operation_error,
        Err(cleanup_error) => io::Error::other(format!(
            "{operation_error}; temporary file cleanup failed: {cleanup_error}"
        )),
    }
}
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
fn safe_relative_path(path: &str) -> Result<PathBuf, TrackingError> {
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
fn validate_conversation_id(conversation_id: &str) -> Result<(), TrackingError> {
    if conversation_id.is_empty()
        || conversation_id.len() > 256
        || conversation_id.chars().any(char::is_control)
    {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(())
}
fn validate_change_id(id: &str) -> Result<(), TrackingError> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(())
}
fn change_id(root: &Path, path: &str) -> String {
    let value = format!("{}\0{path}", root.to_string_lossy());
    sha256(value.as_bytes())
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[cfg(test)]
mod tests {
    use super::checkpoints::{
        CheckpointRecoveryState, CheckpointRestoreError, checkpoint_recovery_state,
        mark_checkpoint_recovery_available, persist_checkpoint, restore_checkpoint,
        revert_checkpoint_operation,
    };
    use super::{
        LEGACY_FILE_CHANGE_DIRECTORY, MANIFEST_VERSION, Manifest, ManifestEntry, RevertError,
        SNAPSHOT_DIRECTORY, SNAPSHOT_SHARD_HEX_LENGTH, atomic_write_workspace_file_from_directory,
        capture_file, capture_file_from_directory, capture_workspace, delete_all_changes,
        delete_conversation_changes, ensure_no_symlink_path_from_directory, list_changes,
        open_workspace_directory, open_workspace_file, read_bounded_file, read_diff, record_deltas,
        remove_workspace_file_from_directory, revert_all_changes, revert_change, revert_hunk,
        snapshot_file, workspace_deltas, workspace_directory_matches_path, workspace_root_identity,
    };
    use crate::storage::AppStorage;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    type FileCheckpointChange<'a> = (&'a str, Option<&'a [u8]>, Option<&'a [u8]>);
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new(name: &str) -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "openchat-file-changes-{name}-{}-{id}",
                std::process::id()
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
    struct TestStorage {
        storage: AppStorage,
        _directory: TestDirectory,
    }
    impl TestStorage {
        fn new(name: &str) -> Self {
            let directory = TestDirectory::new(name);
            let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
            storage                .connect()                .expect("connect test storage")                .execute_batch(                    "CREATE TABLE projects (                        id TEXT PRIMARY KEY NOT NULL,                        name TEXT NOT NULL,                        folder_path TEXT NOT NULL,                        created_at INTEGER NOT NULL,                        updated_at INTEGER NOT NULL                    );                    CREATE TABLE conversations (                        id TEXT PRIMARY KEY NOT NULL,                        title TEXT NOT NULL,                        title_source TEXT NOT NULL DEFAULT 'automatic',                        connection_id TEXT,                        workspace_id TEXT,                        api_key_connection_id TEXT,                        provider_id TEXT,                        model_id TEXT,                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,                        is_pinned INTEGER NOT NULL DEFAULT 0,                        created_at INTEGER NOT NULL,                        updated_at INTEGER NOT NULL                    );                    CREATE TABLE messages (                        rowid INTEGER PRIMARY KEY,                        id TEXT NOT NULL,                        conversation_id TEXT NOT NULL                            REFERENCES conversations(id) ON DELETE CASCADE,                        role TEXT NOT NULL,                        content TEXT NOT NULL,                        created_at INTEGER,                        output_tokens INTEGER,                        tokens_per_second REAL,                        elapsed_microseconds INTEGER,                        reasoning_summaries TEXT NOT NULL DEFAULT '[]',                        tool_activities TEXT NOT NULL DEFAULT '[]',                        status TEXT NOT NULL,                        failure_code TEXT,                        UNIQUE (conversation_id, id)                    );",                )                .expect("create desktop chat tables");
            storage
                .initialize_backend_schema()
                .expect("initialize test backend schema");
            Self {
                storage,
                _directory: directory,
            }
        }
        fn add_conversation(&self, conversation_id: &str) {
            ensure_conversation(&self.storage, conversation_id);
        }
    }
    fn ensure_conversation(storage: &AppStorage, conversation_id: &str) {
        storage            .connect()            .expect("connect test storage")            .execute(                "INSERT OR IGNORE INTO conversations (                    id, title, created_at, updated_at                 ) VALUES (?1, ?1, 1, 1)",                [conversation_id],            )            .expect("insert test conversation");
    }
    fn record_file_change(
        root: &std::path::Path,
        storage: &AppStorage,
        conversation_id: &str,
        path: &str,
        old: Option<&[u8]>,
        new: Option<&[u8]>,
    ) -> String {
        ensure_conversation(storage, conversation_id);
        let before = match old {
            Some(content) => {
                fs::create_dir_all(root.join(path).parent().expect("file parent"))
                    .expect("create parent");
                fs::write(root.join(path), content).expect("write old file");
                capture_file(root, path).expect("capture old file")
            }
            None => None,
        };
        match new {
            Some(content) => {
                fs::create_dir_all(root.join(path).parent().expect("file parent"))
                    .expect("create parent");
                fs::write(root.join(path), content).expect("write new file");
            }
            None => {
                let _ = fs::remove_file(root.join(path));
            }
        }
        let after = capture_file(root, path).expect("capture new file");
        let normalized = path.replace('\\', "/");
        let root_identity = workspace_root_identity(root).expect("capture workspace identity");
        let delta = super::FileChangeDelta {
            root_identity,
            path: normalized,
            before,
            after,
        };
        record_deltas(storage, conversation_id, root, vec![delta])
            .expect("record file change")
            .first()
            .expect("one recorded file change")
            .id
            .clone()
    }
    fn record_file_checkpoint(
        root: &std::path::Path,
        storage: &AppStorage,
        conversation_id: &str,
        tool_name: &str,
        changes: &[FileCheckpointChange<'_>],
    ) -> String {
        ensure_conversation(storage, conversation_id);
        let root_identity = workspace_root_identity(root).expect("capture workspace identity");
        let mut deltas = Vec::with_capacity(changes.len());
        for (path, before_content, after_content) in changes {
            let target = root.join(path);
            fs::create_dir_all(target.parent().expect("file parent")).expect("create file parent");
            match before_content {
                Some(content) => fs::write(&target, content).expect("write before file state"),
                None => {
                    let _ = fs::remove_file(&target);
                }
            }
            let before = capture_file(root, path).expect("capture before file state");
            match after_content {
                Some(content) => fs::write(&target, content).expect("write after file state"),
                None => {
                    let _ = fs::remove_file(&target);
                }
            }
            let after = capture_file(root, path).expect("capture after file state");
            deltas.push(super::FileChangeDelta {
                root_identity,
                path: (*path).to_owned(),
                before,
                after,
            });
        }
        record_deltas(storage, conversation_id, root, deltas.clone())
            .expect("record operation file changes");
        persist_checkpoint(
            storage,
            conversation_id,
            tool_name,
            &[(root.to_path_buf(), deltas)],
        )
        .expect("persist operation checkpoint")
        .expect("operation changed files")
    }
    #[test]
    fn multi_root_tracking_rejects_invalid_root_without_committing_other_roots() {
        let first_root = TestDirectory::new("batch-first-root");
        let second_root = TestDirectory::new("batch-second-root");
        let data = TestStorage::new("batch-tracking-data");
        let conversation_id = "batch-tracking-conversation";
        ensure_conversation(&data.storage, conversation_id);
        let first_identity =
            workspace_root_identity(&first_root.0).expect("capture first root identity");
        let second_identity =
            workspace_root_identity(&second_root.0).expect("capture second root identity");
        let first_content = b"tracked from first root".to_vec();
        let second_content = b"invalid second root".to_vec();
        let grouped_deltas = vec![
            (
                first_root.0.clone(),
                vec![super::FileChangeDelta {
                    root_identity: first_identity,
                    path: "first.txt".to_owned(),
                    before: None,
                    after: Some(super::FileSnapshot {
                        hash: super::sha256(&first_content),
                        content: Some(first_content),
                    }),
                }],
            ),
            (
                second_root.0.clone(),
                vec![super::FileChangeDelta {
                    root_identity: first_identity,
                    path: "second.txt".to_owned(),
                    before: None,
                    after: Some(super::FileSnapshot {
                        hash: super::sha256(&second_content),
                        content: Some(second_content),
                    }),
                }],
            ),
        ];
        let result = super::record_deltas_batch(&data.storage, conversation_id, &grouped_deltas);
        assert!(matches!(
            result,
            Err(super::TrackingError::WorkspaceUnavailable)
        ));
        assert!(
            list_changes(&data.storage, conversation_id)
                .expect("list unchanged tracking")
                .is_empty()
        );
        assert_ne!(first_identity, second_identity);
    }
    #[test]
    fn failed_tool_recovery_preserves_later_changes_to_other_files() {
        let root = TestDirectory::new("recovery-workspace");
        let data = TestStorage::new("recovery-data");
        let failed_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "recovery-conversation",
            "bash",
            &[("partial.txt", None, Some(b"partial result\n"))],
        );
        mark_checkpoint_recovery_available(
            &data.storage,
            "recovery-conversation",
            &failed_checkpoint,
        )
        .expect("enable recovery for the failed operation");
        let later_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "recovery-conversation",
            "write_file",
            &[("later.txt", None, Some(b"later result\n"))],
        );
        assert_eq!(
            checkpoint_recovery_state(&data.storage, "recovery-conversation", &later_checkpoint,),
            Err(CheckpointRestoreError::NotRecoverable)
        );
        assert_eq!(
            checkpoint_recovery_state(&data.storage, "recovery-conversation", &failed_checkpoint,)
                .expect("inspect failed operation recovery"),
            CheckpointRecoveryState::Available
        );
        revert_checkpoint_operation(&data.storage, "recovery-conversation", &failed_checkpoint)
            .expect("revert only the failed operation");
        assert!(!root.0.join("partial.txt").exists());
        assert_eq!(
            fs::read_to_string(root.0.join("later.txt")).expect("read later file"),
            "later result\n"
        );
        assert_eq!(
            checkpoint_recovery_state(&data.storage, "recovery-conversation", &failed_checkpoint,)
                .expect("inspect completed recovery"),
            CheckpointRecoveryState::AlreadyRestored
        );
        assert_eq!(            revert_checkpoint_operation(                &data.storage,                "recovery-conversation",                &failed_checkpoint,            )            .expect_err("prevent repeating recovery"),            CheckpointRestoreError::AlreadyRestored        );
    }
    #[test]
    fn failed_tool_recovery_resumes_after_a_partial_filesystem_restore() {
        let root = TestDirectory::new("partial-recovery-workspace");
        let data = TestStorage::new("partial-recovery-data");
        let conversation_id = "partial-recovery-conversation";
        let failed_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            conversation_id,
            "bash",
            &[
                (
                    "first.txt",
                    Some(b"before first\n"),
                    Some(b"partial first\n"),
                ),
                (
                    "second.txt",
                    Some(b"before second\n"),
                    Some(b"partial second\n"),
                ),
            ],
        );
        mark_checkpoint_recovery_available(&data.storage, conversation_id, &failed_checkpoint)
            .expect("enable recovery for the failed operation");

        fs::write(root.0.join("first.txt"), b"before first\n")
            .expect("simulate interruption after restoring the first file");
        assert_eq!(
            checkpoint_recovery_state(&data.storage, conversation_id, &failed_checkpoint)
                .expect("inspect partially restored operation"),
            CheckpointRecoveryState::Available
        );

        let summaries =
            revert_checkpoint_operation(&data.storage, conversation_id, &failed_checkpoint)
                .expect("resume restoring the remaining file");

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            fs::read(root.0.join("first.txt")).expect("read first restored file"),
            b"before first\n"
        );
        assert_eq!(
            fs::read(root.0.join("second.txt")).expect("read second restored file"),
            b"before second\n"
        );
        let changes = list_changes(&data.storage, conversation_id).expect("list restored changes");
        assert!(changes.iter().all(|change| change.status == "reverted"));
        assert_eq!(
            checkpoint_recovery_state(&data.storage, conversation_id, &failed_checkpoint)
                .expect("inspect completed recovery"),
            CheckpointRecoveryState::AlreadyRestored
        );
    }
    #[test]
    fn checkpoint_restore_and_recovery_reject_a_replaced_root_with_matching_contents() {
        let root = TestDirectory::new("replaced-root-checkpoint");
        let data = TestStorage::new("replaced-root-checkpoint-data");
        let checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "replaced-root-checkpoint-conversation",
            "write_file",
            &[("note.txt", None, Some(b"after\n"))],
        );
        mark_checkpoint_recovery_available(
            &data.storage,
            "replaced-root-checkpoint-conversation",
            &checkpoint,
        )
        .expect("enable failed operation recovery");
        let moved_root = root.0.with_extension("original");
        fs::rename(&root.0, &moved_root).expect("move original workspace root");
        fs::create_dir_all(&root.0).expect("create replacement workspace root");
        fs::write(root.0.join("note.txt"), b"after\n").expect("seed matching replacement file");
        assert_eq!(
            restore_checkpoint(
                &data.storage,
                "replaced-root-checkpoint-conversation",
                &checkpoint,
            )
            .expect_err("reject restore into replacement root"),
            CheckpointRestoreError::Conflict
        );
        assert_eq!(
            revert_checkpoint_operation(
                &data.storage,
                "replaced-root-checkpoint-conversation",
                &checkpoint,
            )
            .expect_err("reject recovery into replacement root"),
            CheckpointRestoreError::Conflict
        );
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read replacement file"),
            b"after\n"
        );
        fs::remove_dir_all(moved_root).expect("remove moved original workspace");
    }
    #[test]
    fn identityless_checkpoint_history_cannot_restore_or_recover_files() {
        let root = TestDirectory::new("identityless-checkpoint");
        let data = TestStorage::new("identityless-checkpoint-data");
        let checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "identityless-checkpoint-conversation",
            "write_file",
            &[("note.txt", None, Some(b"after\n"))],
        );
        mark_checkpoint_recovery_available(
            &data.storage,
            "identityless-checkpoint-conversation",
            &checkpoint,
        )
        .expect("enable failed operation recovery");
        data.storage            .connect()            .expect("connect test storage")            .execute(                "UPDATE conversation_file_checkpoint_files                 SET root_identity = NULL WHERE checkpoint_id = ?1",                [&checkpoint],            )            .expect("simulate a legacy identity-less checkpoint");
        assert_eq!(
            checkpoint_recovery_state(
                &data.storage,
                "identityless-checkpoint-conversation",
                &checkpoint,
            ),
            Err(CheckpointRestoreError::NotRecoverable)
        );
        assert_eq!(
            revert_checkpoint_operation(
                &data.storage,
                "identityless-checkpoint-conversation",
                &checkpoint,
            )
            .expect_err("refuse identity-less checkpoint recovery"),
            CheckpointRestoreError::NotRecoverable
        );
        assert_eq!(
            restore_checkpoint(
                &data.storage,
                "identityless-checkpoint-conversation",
                &checkpoint,
            )
            .expect_err("refuse identity-less checkpoint restore"),
            CheckpointRestoreError::NotRecoverable
        );
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read current file"),
            b"after\n"
        );
    }
    #[test]
    fn failed_tool_recovery_keeps_every_file_when_any_path_conflicts() {
        let root = TestDirectory::new("recovery-conflict-workspace");
        let data = TestStorage::new("recovery-conflict-data");
        let failed_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "recovery-conflict-conversation",
            "bash",
            &[
                ("first.txt", None, Some(b"partial first\n")),
                ("second.txt", None, Some(b"partial second\n")),
            ],
        );
        mark_checkpoint_recovery_available(
            &data.storage,
            "recovery-conflict-conversation",
            &failed_checkpoint,
        )
        .expect("enable recovery for the failed operation");
        fs::write(root.0.join("second.txt"), "external change\n")
            .expect("write external conflicting edit");
        assert_eq!(
            checkpoint_recovery_state(
                &data.storage,
                "recovery-conflict-conversation",
                &failed_checkpoint,
            )
            .expect("inspect conflicting recovery"),
            CheckpointRecoveryState::Conflict
        );
        assert_eq!(
            revert_checkpoint_operation(
                &data.storage,
                "recovery-conflict-conversation",
                &failed_checkpoint,
            )
            .expect_err("refuse conflicting recovery"),
            CheckpointRestoreError::Conflict
        );
        assert_eq!(
            fs::read_to_string(root.0.join("first.txt")).expect("read untouched first file"),
            "partial first\n"
        );
        assert_eq!(
            fs::read_to_string(root.0.join("second.txt")).expect("read conflicting file"),
            "external change\n"
        );
    }
    #[test]
    fn failed_tool_recovery_refuses_a_path_touched_by_a_later_tool() {
        let root = TestDirectory::new("recovery-later-tool-workspace");
        let data = TestStorage::new("recovery-later-tool-data");
        let failed_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "recovery-later-tool-conversation",
            "bash",
            &[("shared.txt", Some(b"before\n"), Some(b"partial\n"))],
        );
        mark_checkpoint_recovery_available(
            &data.storage,
            "recovery-later-tool-conversation",
            &failed_checkpoint,
        )
        .expect("enable recovery for the failed operation");
        record_file_checkpoint(
            &root.0,
            &data.storage,
            "recovery-later-tool-conversation",
            "write_file",
            &[("shared.txt", Some(b"partial\n"), Some(b"later result\n"))],
        );
        assert_eq!(
            checkpoint_recovery_state(
                &data.storage,
                "recovery-later-tool-conversation",
                &failed_checkpoint,
            )
            .expect("inspect later tool conflict"),
            CheckpointRecoveryState::Conflict
        );
        assert_eq!(
            revert_checkpoint_operation(
                &data.storage,
                "recovery-later-tool-conversation",
                &failed_checkpoint,
            )
            .expect_err("preserve the later tool result"),
            CheckpointRestoreError::Conflict
        );
        assert_eq!(
            fs::read_to_string(root.0.join("shared.txt")).expect("read later tool result"),
            "later result\n"
        );
    }
    #[test]
    fn restoring_a_workspace_checkpoint_records_the_restore_without_relocking() {
        let root = TestDirectory::new("workspace-restore-workspace");
        let data = TestStorage::new("workspace-restore-data");
        let first_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "workspace-restore-conversation",
            "write_file",
            &[("shared.txt", Some(b"before\n"), Some(b"first\n"))],
        );
        record_file_checkpoint(
            &root.0,
            &data.storage,
            "workspace-restore-conversation",
            "write_file",
            &[("shared.txt", Some(b"first\n"), Some(b"later\n"))],
        );
        restore_checkpoint(
            &data.storage,
            "workspace-restore-conversation",
            &first_checkpoint,
        )
        .expect("restore the selected workspace checkpoint");
        assert_eq!(
            fs::read_to_string(root.0.join("shared.txt")).expect("read restored file"),
            "first\n"
        );
    }
    #[test]
    fn checkpoint_persistence_failure_rolls_back_files_and_tracking() {
        let root = TestDirectory::new("workspace-restore-persistence-failure");
        let data = TestStorage::new("workspace-restore-persistence-failure-data");
        let conversation_id = "workspace-restore-persistence-failure-conversation";
        let first_checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            conversation_id,
            "write_file",
            &[("shared.txt", Some(b"before\n"), Some(b"first\n"))],
        );
        record_file_checkpoint(
            &root.0,
            &data.storage,
            conversation_id,
            "write_file",
            &[("shared.txt", Some(b"first\n"), Some(b"later\n"))],
        );
        data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_restore_checkpoint                 BEFORE INSERT ON conversation_file_checkpoints                 WHEN NEW.tool_name = 'restore_checkpoint'                 BEGIN                    SELECT RAISE(ABORT, 'simulated checkpoint persistence failure');                 END;",            )            .expect("install checkpoint persistence failure");
        assert_eq!(
            restore_checkpoint(&data.storage, conversation_id, &first_checkpoint)
                .expect_err("report checkpoint persistence failure"),
            CheckpointRestoreError::Unavailable
        );
        assert_eq!(
            fs::read(root.0.join("shared.txt")).expect("read rolled-back file"),
            b"later\n",
            "a persistence failure must restore the file contents from before the restore"
        );
        assert_eq!(
            super::checkpoints::list_checkpoints(&data.storage, conversation_id)
                .expect("list persisted checkpoints")
                .len(),
            2,
            "a failed restore must not leave a restore checkpoint"
        );
        let change = list_changes(&data.storage, conversation_id)
            .expect("list restored tracking state")
            .into_iter()
            .find(|change| change.path == "shared.txt")
            .expect("find the tracked file");
        revert_change(&data.storage, conversation_id, &change.id)
            .expect("tracking must match the rolled-back file contents");
        assert_eq!(
            fs::read(root.0.join("shared.txt")).expect("read file after tracked revert"),
            b"before\n",
            "the repaired tracking entry must remain usable after rollback"
        );
    }
    #[test]
    fn restoring_an_already_current_workspace_checkpoint_is_a_successful_noop() {
        let root = TestDirectory::new("workspace-restore-noop-workspace");
        let data = TestStorage::new("workspace-restore-noop-data");
        let checkpoint = record_file_checkpoint(
            &root.0,
            &data.storage,
            "workspace-restore-noop-conversation",
            "write_file",
            &[("shared.txt", Some(b"before\n"), Some(b"current\n"))],
        );
        let summaries = restore_checkpoint(
            &data.storage,
            "workspace-restore-noop-conversation",
            &checkpoint,
        )
        .expect("restoring the current checkpoint is a successful no-op");
        assert!(summaries.is_empty());
        assert_eq!(
            fs::read_to_string(root.0.join("shared.txt")).expect("read unchanged file"),
            "current\n"
        );
    }
    #[test]
    fn tracks_text_changes_and_reverts_to_the_first_version() {
        let root = TestDirectory::new("workspace");
        let data = TestStorage::new("data");
        data.add_conversation("conversation-1");
        fs::write(root.0.join("note.txt"), "first\nsecond\n").expect("write initial file");
        let before = capture_file(&root.0, "note.txt")
            .expect("capture before")
            .expect("initial file exists");
        fs::write(root.0.join("note.txt"), "first\nchanged\nthird\n").expect("write changed file");
        let after = capture_file(&root.0, "note.txt")
            .expect("capture after")
            .expect("changed file exists");
        let root_identity = workspace_root_identity(&root.0).expect("capture workspace identity");
        let summaries = record_deltas(
            &data.storage,
            "conversation-1",
            &root.0,
            vec![super::FileChangeDelta {
                root_identity,
                path: "note.txt".to_owned(),
                before: Some(before),
                after: Some(after),
            }],
        )
        .expect("save change");
        assert_eq!(summaries[0].added_lines, Some(2));
        assert_eq!(summaries[0].removed_lines, Some(1));
        assert!(summaries[0].can_revert);
        assert!(
            !data
                ._directory
                .0
                .join(LEGACY_FILE_CHANGE_DIRECTORY)
                .exists()
        );
        let storage_directory = data._directory.0.clone();
        let TestStorage {
            storage,
            _directory,
        } = data;
        drop(storage);
        let reopened =
            AppStorage::open_at(storage_directory).expect("reopen persistent test storage");
        assert_eq!(
            list_changes(&reopened, "conversation-1")
                .expect("load persisted change history")
                .len(),
            1
        );
        let diff = read_diff(&reopened, "conversation-1", &summaries[0].id).expect("read diff");
        assert!(
            diff["diff"]
                .as_str()
                .is_some_and(|text| text.contains("+third"))
        );
        let changes =
            revert_change(&reopened, "conversation-1", &summaries[0].id).expect("revert change");
        assert_eq!(changes[0].status, "reverted");
        assert_eq!(
            fs::read_to_string(root.0.join("note.txt")).expect("read restored file"),
            "first\nsecond\n"
        );
        drop(reopened);
        drop(_directory);
    }
    #[test]
    fn tracks_created_and_deleted_files_and_detects_external_edits_before_revert() {
        let root = TestDirectory::new("workspace");
        let data = TestStorage::new("data");
        let created_id = record_file_change(
            &root.0,
            &data.storage,
            "conversation-2",
            "new.txt",
            None,
            Some(b"created\n"),
        );
        revert_change(&data.storage, "conversation-2", &created_id).expect("remove created file");
        assert!(!root.0.join("new.txt").exists());
        let deleted_id = record_file_change(
            &root.0,
            &data.storage,
            "conversation-2",
            "gone.txt",
            Some(b"restore me\n"),
            None,
        );
        revert_change(&data.storage, "conversation-2", &deleted_id).expect("restore deleted file");
        assert_eq!(
            fs::read_to_string(root.0.join("gone.txt")).expect("read restored file"),
            "restore me\n"
        );
        let conflict_id = record_file_change(
            &root.0,
            &data.storage,
            "conversation-2",
            "gone.txt",
            Some(b"restore me\n"),
            Some(b"assistant edit\n"),
        );
        fs::write(root.0.join("gone.txt"), "user edit\n").expect("write user edit");
        assert!(matches!(
            revert_change(&data.storage, "conversation-2", &conflict_id),
            Err(RevertError::Conflict)
        ));
        assert_eq!(
            fs::read_to_string(root.0.join("gone.txt")).expect("read conflicted file"),
            "user edit\n"
        );
    }
    #[test]
    fn tracked_revert_restores_file_when_manifest_update_fails() {
        let root = TestDirectory::new("tracked-revert-manifest-failure");
        let data = TestStorage::new("tracked-revert-manifest-failure-data");
        let conversation_id = "tracked-revert-manifest-failure-conversation";
        let id = record_file_change(
            &root.0,
            &data.storage,
            conversation_id,
            "note.txt",
            Some(b"before\n"),
            Some(b"after\n"),
        );
        data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_tracked_revert                 BEFORE UPDATE OF status ON conversation_file_changes                 WHEN NEW.status = 'reverted'                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install tracked revert failure");
        assert_eq!(
            revert_change(&data.storage, conversation_id, &id)
                .expect_err("report manifest failure after restoring the file"),
            RevertError::Unavailable
        );
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
            b"after\n"
        );
        let change = list_changes(&data.storage, conversation_id)
            .expect("read manifest after compensated failure")
            .into_iter()
            .find(|change| change.id == id)
            .expect("find tracked change");
        assert_eq!(change.status, "active");
    }
    #[test]
    fn hunk_revert_restores_file_when_manifest_update_fails() {
        let root = TestDirectory::new("hunk-revert-manifest-failure");
        let data = TestStorage::new("hunk-revert-manifest-failure-data");
        let conversation_id = "hunk-revert-manifest-failure-conversation";
        let original = b"first changed\nsecond\nthird changed\n";
        let id = record_file_change(
            &root.0,
            &data.storage,
            conversation_id,
            "note.txt",
            Some(b"first\nsecond\nthird\n"),
            Some(original),
        );
        data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_hunk_revert                 BEFORE UPDATE OF expected_hash ON conversation_file_changes                 WHEN NEW.expected_hash IS NOT OLD.expected_hash                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install hunk revert failure");
        assert_eq!(
            revert_hunk(&data.storage, conversation_id, &id, 0)
                .expect_err("report hunk manifest failure"),
            RevertError::Unavailable
        );
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
            original
        );
        let change = list_changes(&data.storage, conversation_id)
            .expect("read manifest after compensated failure")
            .into_iter()
            .find(|change| change.id == id)
            .expect("find tracked change");
        assert_eq!(change.status, "active");
    }
    #[test]
    fn revert_all_reports_a_failed_revert_instead_of_suppressing_it() {
        let root = TestDirectory::new("revert-all-manifest-failure");
        let data = TestStorage::new("revert-all-manifest-failure-data");
        let conversation_id = "revert-all-manifest-failure-conversation";
        let id = record_file_change(
            &root.0,
            &data.storage,
            conversation_id,
            "note.txt",
            Some(b"before\n"),
            Some(b"after\n"),
        );
        data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_revert_all                 BEFORE UPDATE OF status ON conversation_file_changes                 WHEN NEW.status = 'reverted'                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install revert-all failure");
        assert_eq!(
            revert_all_changes(&data.storage, conversation_id)
                .expect_err("surface the individual revert failure"),
            RevertError::Unavailable
        );
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
            b"after\n"
        );
        let change = list_changes(&data.storage, conversation_id)
            .expect("read manifest after compensated failure")
            .into_iter()
            .find(|change| change.id == id)
            .expect("find tracked change");
        assert_eq!(change.status, "active");
    }

    #[test]
    fn revert_all_with_no_eligible_changes_returns_the_current_list() {
        let data = TestStorage::new("revert-all-no-changes-data");
        let conversation_id = "revert-all-no-changes-conversation";
        data.add_conversation(conversation_id);

        assert!(
            revert_all_changes(&data.storage, conversation_id)
                .expect("list changes without eligible reverts")
                .is_empty()
        );
    }
    #[test]
    fn new_file_changes_rebase_identityless_history_to_the_captured_before_state() {
        let root = TestDirectory::new("identityless-change-history");
        let data = TestStorage::new("identityless-change-history-data");
        let conversation_id = "identityless-change-history-conversation";
        let id = record_file_change(
            &root.0,
            &data.storage,
            conversation_id,
            "note.txt",
            Some(b"old baseline\n"),
            Some(b"first result\n"),
        );
        data.storage            .connect()            .expect("connect test storage")            .execute(                "UPDATE conversation_file_changes SET root_identity = NULL                 WHERE conversation_id = ?1 AND id = ?2",                rusqlite::params![conversation_id, id],            )            .expect("simulate identity-less legacy history");
        let updated_id = record_file_change(
            &root.0,
            &data.storage,
            conversation_id,
            "note.txt",
            Some(b"first result\n"),
            Some(b"second result\n"),
        );
        assert_eq!(updated_id, id);
        assert!(
            list_changes(&data.storage, conversation_id)
                .expect("list rebased change")
                .first()
                .is_some_and(|change| change.can_revert)
        );
        revert_change(&data.storage, conversation_id, &updated_id)
            .expect("revert against the new captured baseline");
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read rebased file"),
            b"first result\n"
        );
    }
    #[test]
    fn reverting_a_tracked_change_rejects_a_replaced_root_with_matching_contents() {
        let root = TestDirectory::new("replaced-root-change");
        let data = TestStorage::new("replaced-root-change-data");
        let id = record_file_change(
            &root.0,
            &data.storage,
            "replaced-root-change-conversation",
            "note.txt",
            Some(b"before\n"),
            Some(b"after\n"),
        );
        let moved_root = root.0.with_extension("original");
        fs::rename(&root.0, &moved_root).expect("move original workspace root");
        fs::create_dir_all(&root.0).expect("create replacement workspace root");
        fs::write(root.0.join("note.txt"), b"after\n").expect("seed matching replacement file");
        assert!(matches!(
            revert_change(&data.storage, "replaced-root-change-conversation", &id),
            Err(RevertError::Conflict)
        ));
        assert_eq!(
            fs::read(root.0.join("note.txt")).expect("read replacement file"),
            b"after\n"
        );
        fs::remove_dir_all(moved_root).expect("remove moved original workspace");
    }
    #[test]
    fn workspace_snapshot_detects_created_modified_and_deleted_paths() {
        let root = TestDirectory::new("workspace");
        fs::create_dir(root.0.join("target")).expect("create ignored directory");
        fs::write(root.0.join("target/ignored.txt"), "ignored").expect("write ignored file");
        fs::write(root.0.join("keep.txt"), "before").expect("write original");
        fs::write(root.0.join("modify.txt"), "before").expect("write modified file");
        let before = capture_workspace(&root.0).expect("capture before");
        fs::write(root.0.join("modify.txt"), "after").expect("modify file");
        fs::write(root.0.join("new.txt"), "new").expect("create file");
        fs::remove_file(root.0.join("keep.txt")).expect("delete file");
        let after = capture_workspace(&root.0).expect("capture after");
        let deltas = workspace_deltas(&before, &after).expect("compare workspaces");
        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].path, "keep.txt");
        assert_eq!(deltas[1].path, "modify.txt");
        assert_eq!(deltas[2].path, "new.txt");
    }
    #[test]
    fn file_tool_writes_wait_for_checkpoint_lock_while_reads_remain_available() {
        use std::{sync::mpsc, thread, time::Duration};

        let root = TestDirectory::new("file-tool-change-lock");
        fs::write(root.0.join("note.txt"), "before").expect("create workspace file");
        let guard = super::lock_file_changes().expect("hold file-change lock");
        let read = crate::tools::read_file(&root.0, "note.txt", 1, 1)
            .expect("read-only file access remains available");
        assert_eq!(read["lines"][0]["text"], "before");

        let (thread_id_sender, thread_id_receiver) = mpsc::channel();
        let (start_sender, start_receiver) = mpsc::channel();
        let (waiting_sender, waiting_receiver) = mpsc::sync_channel(1);
        let worker_root = root.0.clone();
        let worker = thread::spawn(move || {
            thread_id_sender
                .send(thread::current().id())
                .expect("report writer thread id");
            start_receiver
                .recv()
                .expect("wait for lock-wait observation setup");
            crate::tools::write_file(&worker_root, "note.txt", "after")
        });
        let worker_thread_id = thread_id_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("writer thread starts");
        *super::FILE_CHANGE_LOCK_WAIT_HOOK
            .get_or_init(|| super::Mutex::new(None))
            .lock()
            .expect("install file-change lock wait hook") =
            Some((worker_thread_id, waiting_sender));
        start_sender.send(()).expect("start the writer");
        waiting_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("writer observes the held file-change lock");
        let completed_while_locked = worker.is_finished();
        drop(guard);
        worker
            .join()
            .expect("writer thread exits")
            .expect("write succeeds after the lock is released");
        assert!(
            !completed_while_locked,
            "a file-tool mutation must wait while checkpoint or revert owns the lock"
        );
        assert_eq!(
            fs::read_to_string(root.0.join("note.txt")).expect("read updated file"),
            "after"
        );
    }
    #[test]
    fn file_tool_edits_wait_for_checkpoint_lock() {
        use std::{sync::mpsc, thread, time::Duration};

        let root = TestDirectory::new("file-tool-edit-change-lock");
        fs::write(root.0.join("note.txt"), "before").expect("create workspace file");
        let guard = super::lock_file_changes().expect("hold file-change lock");

        let (thread_id_sender, thread_id_receiver) = mpsc::channel();
        let (start_sender, start_receiver) = mpsc::channel();
        let (waiting_sender, waiting_receiver) = mpsc::sync_channel(1);
        let worker_root = root.0.clone();
        let worker = thread::spawn(move || {
            thread_id_sender
                .send(thread::current().id())
                .expect("report editor thread id");
            start_receiver
                .recv()
                .expect("wait for lock-wait observation setup");
            crate::tools::edit_file(&worker_root, "note.txt", "before", "after")
        });
        let worker_thread_id = thread_id_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("editor thread starts");
        *super::FILE_CHANGE_LOCK_WAIT_HOOK
            .get_or_init(|| super::Mutex::new(None))
            .lock()
            .expect("install file-change lock wait hook") =
            Some((worker_thread_id, waiting_sender));
        start_sender.send(()).expect("start the editor");
        waiting_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("editor observes the held file-change lock");
        let completed_while_locked = worker.is_finished();
        drop(guard);
        worker
            .join()
            .expect("editor thread exits")
            .expect("edit succeeds after the lock is released");

        assert!(
            !completed_while_locked,
            "a file-tool edit must wait while checkpoint or revert owns the lock"
        );
        assert_eq!(
            fs::read_to_string(root.0.join("note.txt")).expect("read edited file"),
            "after"
        );
    }
    #[test]
    fn workspace_snapshot_rejects_a_replacement_root_at_the_same_path() {
        let parent = TestDirectory::new("workspace-root-replacement");
        let root = parent.0.join("workspace");
        let displaced_root = parent.0.join("workspace-displaced");
        fs::create_dir(&root).expect("create workspace root");
        fs::write(root.join("shared.txt"), "same bytes\n").expect("write original file");
        let before = capture_workspace(&root).expect("capture original root");
        fs::rename(&root, &displaced_root).expect("move original root");
        fs::create_dir(&root).expect("create replacement root at the same path");
        fs::write(root.join("shared.txt"), "same bytes\n").expect("seed replacement file");
        let after = capture_workspace(&root).expect("capture replacement root");
        assert!(workspace_deltas(&before, &after).is_err());
    }
    #[test]
    fn file_change_tracking_rejects_a_replaced_root_before_commit() {
        let parent = TestDirectory::new("workspace-change-root-swap");
        let root = parent.0.join("workspace");
        let displaced_root = parent.0.join("workspace-displaced");
        let storage = TestStorage::new("workspace-change-root-swap-data");
        storage.add_conversation("workspace-change-root-swap");
        fs::create_dir(&root).expect("create workspace root");
        fs::write(root.join("shared.txt"), "before\n").expect("write initial file");
        let before = capture_workspace(&root).expect("capture before");
        fs::write(root.join("shared.txt"), "after\n").expect("write changed file");
        let after = capture_workspace(&root).expect("capture after");
        let deltas = workspace_deltas(&before, &after).expect("compare workspace snapshots");
        fs::rename(&root, &displaced_root).expect("move captured workspace root");
        fs::create_dir(&root).expect("create replacement root");
        fs::write(root.join("shared.txt"), "after\n").expect("seed replacement file");
        assert!(
            record_deltas(
                &storage.storage,
                "workspace-change-root-swap",
                &root,
                deltas
            )
            .is_err()
        );
        assert!(
            list_changes(&storage.storage, "workspace-change-root-swap")
                .expect("read unchanged tracking")
                .is_empty()
        );
    }
    #[test]
    fn checkpoint_persistence_rejects_a_replaced_root_before_commit() {
        let parent = TestDirectory::new("workspace-checkpoint-root-swap");
        let root = parent.0.join("workspace");
        let displaced_root = parent.0.join("workspace-displaced");
        let storage = TestStorage::new("workspace-checkpoint-root-swap-data");
        storage.add_conversation("workspace-checkpoint-root-swap");
        fs::create_dir(&root).expect("create workspace root");
        fs::write(root.join("shared.txt"), "before\n").expect("write initial file");
        let before = capture_workspace(&root).expect("capture before");
        fs::write(root.join("shared.txt"), "after\n").expect("write changed file");
        let after = capture_workspace(&root).expect("capture after");
        let deltas = workspace_deltas(&before, &after).expect("compare workspace snapshots");
        fs::rename(&root, &displaced_root).expect("move captured workspace root");
        fs::create_dir(&root).expect("create replacement root");
        fs::write(root.join("shared.txt"), "after\n").expect("seed replacement file");
        assert!(
            persist_checkpoint(
                &storage.storage,
                "workspace-checkpoint-root-swap",
                "bash",
                &[(root, deltas)]
            )
            .is_err()
        );
        assert!(
            super::checkpoints::list_checkpoints(
                &storage.storage,
                "workspace-checkpoint-root-swap"
            )
            .expect("read unchanged checkpoints")
            .is_empty()
        );
    }
    #[test]
    fn restore_root_check_rejects_a_different_directory_object() {
        let parent = TestDirectory::new("workspace-root-identity");
        let expected_root = parent.0.join("expected");
        let other_root = parent.0.join("other");
        fs::create_dir(&expected_root).expect("create expected root");
        fs::create_dir(&other_root).expect("create other root");
        let (expected_directory, expected_canonical_root) =
            open_workspace_directory(&expected_root).expect("open expected root");
        assert!(
            !workspace_directory_matches_path(&other_root, &expected_directory)
                .expect("compare directory identity")
        );
        assert!(
            workspace_directory_matches_path(&expected_canonical_root, &expected_directory)
                .expect("match original directory identity")
        );
    }
    #[test]
    fn file_growth_after_metadata_read_does_not_retain_unbounded_content() {
        let root = TestDirectory::new("workspace");
        let path = root.0.join("growing.txt");
        let content = vec![b'x'; 2 * 1024 * 1024];
        fs::write(&path, &content).expect("write growing file");
        let directory = cap_std::fs::Dir::open_ambient_dir(&root.0, cap_std::ambient_authority())
            .expect("open workspace capability");
        let (file, _) = open_workspace_file(&directory, std::path::Path::new("growing.txt"))
            .expect("open growing file through workspace root");
        let snapshot = snapshot_file(file, 1).expect("snapshot growing file");
        assert_eq!(snapshot.hash, super::sha256(&content));
        assert!(snapshot.content.is_none());
    }
    #[test]
    fn direct_file_capture_rejects_files_over_the_snapshot_read_limit() {
        let root = TestDirectory::new("oversized-snapshot");
        let path = root.0.join("large.bin");
        fs::File::create(&path)
            .expect("create sparse file")
            .set_len(super::MAX_WORKSPACE_BYTES + 1)
            .expect("set sparse file length");
        assert!(matches!(
            capture_file(&root.0, "large.bin"),
            Err(super::TrackingError::WorkspaceTooLarge)
        ));
    }
    #[cfg(unix)]
    fn create_directory_symlink(
        target: &std::path::Path,
        link: &std::path::Path,
    ) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    fn create_directory_symlink(
        target: &std::path::Path,
        link: &std::path::Path,
    ) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn workspace_capability_operations_reject_external_symlinks() {
        let workspace = TestDirectory::new("symlink-workspace");
        let external = TestDirectory::new("symlink-external");
        let external_file = external.0.join("secret.txt");
        fs::write(&external_file, b"keep private").expect("write external file");
        create_directory_symlink(&external.0, &workspace.0.join("outside"))
            .expect("create external directory symlink");
        assert!(capture_file(&workspace.0, "outside/secret.txt").is_err());
        let root_directory =
            cap_std::fs::Dir::open_ambient_dir(&workspace.0, cap_std::ambient_authority())
                .expect("open workspace capability");
        assert!(
            crate::filesystem::read_bounded_file_from_directory(
                &root_directory,
                std::path::Path::new("outside/secret.txt"),
                64,
            )
            .is_err()
        );
        assert!(
            super::atomic_write_workspace_file_from_directory(
                &root_directory,
                std::path::Path::new("outside/secret.txt"),
                b"replace",
            )
            .is_err()
        );
        assert_eq!(
            fs::read(&external_file).expect("read external file"),
            b"keep private"
        );
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn workspace_operations_reject_a_directory_link_added_after_preflight() {
        let workspace = TestDirectory::new("symlink-race-workspace");
        let external = TestDirectory::new("symlink-race-external");
        let external_file = external.0.join("secret.txt");
        fs::write(&external_file, b"keep private").expect("write external file");
        let workspace_link = workspace.0.join("outside");
        fs::create_dir(&workspace_link).expect("create initial workspace directory");
        let root_directory =
            cap_std::fs::Dir::open_ambient_dir(&workspace.0, cap_std::ambient_authority())
                .expect("open workspace capability");
        let relative = std::path::Path::new("outside/secret.txt");

        ensure_no_symlink_path_from_directory(&root_directory, relative)
            .expect("ordinary directory passes the initial path check");
        fs::remove_dir(&workspace_link).expect("remove checked directory");
        create_directory_symlink(&external.0, &workspace_link)
            .expect("replace checked directory with external symlink");

        assert!(capture_file_from_directory(&root_directory, relative).is_err());
        assert!(
            atomic_write_workspace_file_from_directory(&root_directory, relative, b"replace")
                .is_err()
        );
        assert!(remove_workspace_file_from_directory(&root_directory, relative).is_err());
        assert_eq!(
            fs::read(&external_file).expect("read external file after rejected operations"),
            b"keep private"
        );
    }
    #[cfg(windows)]
    #[test]
    fn workspace_directory_handles_block_rename_until_released() {
        let workspace = TestDirectory::new("directory-rename-lock-workspace");
        let nested_path = workspace.0.join("nested");
        let file_path = nested_path.join("inner").join("note.txt");
        fs::create_dir_all(file_path.parent().expect("file parent"))
            .expect("create nested directories");
        fs::write(&file_path, b"workspace content").expect("write nested file");
        let moved_root = workspace.0.with_extension("moved-root");
        let moved_nested = workspace.0.join("nested-moved");

        let (root_directory, _) =
            open_workspace_directory(&workspace.0).expect("open workspace capability");
        assert!(
            fs::rename(&workspace.0, &moved_root).is_err(),
            "an open root capability must prevent moving the workspace root"
        );

        let mut read_options = cap_std::fs::OpenOptions::new();
        read_options.read(true);
        let (opened_file, _) = crate::filesystem::open_regular_file_from_directory(
            &root_directory,
            std::path::Path::new("nested/inner/note.txt"),
            &read_options,
        )
        .expect("open file through nested workspace capabilities");
        assert!(
            fs::rename(&nested_path, &moved_nested).is_err(),
            "an open file must keep every nested parent capability alive"
        );

        drop(opened_file);
        fs::rename(&nested_path, &moved_nested)
            .expect("nested directory can move after its capability is released");
        fs::rename(&moved_nested, &nested_path).expect("restore nested directory path");

        drop(root_directory);
        fs::rename(&workspace.0, &moved_root)
            .expect("workspace root can move after its capability is released");
        fs::rename(&moved_root, &workspace.0).expect("restore workspace root path");
    }
    #[cfg(windows)]
    #[test]
    fn tracked_revert_target_holds_existing_nested_parent_through_write() {
        let workspace = TestDirectory::new("tracked-revert-parent-lock");
        let nested = workspace.0.join("nested");
        fs::create_dir_all(&nested).expect("create nested workspace directory");
        fs::write(nested.join("note.txt"), b"current").expect("write workspace file");
        let root_identity = workspace_root_identity(&workspace.0)
            .expect("capture workspace root identity")
            .storage_value()
            .expect("serialize workspace root identity");
        let entry = ManifestEntry {
            id: "a".repeat(64),
            root: workspace.0.to_string_lossy().into_owned(),
            root_identity: Some(root_identity),
            path: "nested/note.txt".to_owned(),
            baseline_exists: true,
            baseline_hash: Some(super::sha256(b"baseline")),
            baseline_blob_hash: None,
            expected_exists: true,
            expected_hash: Some(super::sha256(b"current")),
            added_lines: None,
            removed_lines: None,
            is_binary: false,
            diff_available: true,
            status: "active".to_owned(),
        };
        let mut target = super::RevertFileTarget::open(&entry).expect("open tracked revert target");
        let moved = workspace.0.join("nested-moved");

        assert!(target.matches_entry(&entry).expect("verify current file"));
        assert!(fs::rename(&nested, &moved).is_err());
        target
            .write_file(b"baseline")
            .expect("write through pinned parent capability");
        assert_eq!(
            fs::read(nested.join("note.txt")).expect("read reverted file"),
            b"baseline"
        );

        drop(target);
        fs::rename(&nested, &moved).expect("nested directory is movable after revert target drops");
        fs::rename(&moved, &nested).expect("restore nested workspace directory");
    }
    #[cfg(windows)]
    #[test]
    fn tracked_revert_target_retains_a_created_nested_parent() {
        let workspace = TestDirectory::new("tracked-revert-created-parent");
        let root_identity = workspace_root_identity(&workspace.0)
            .expect("capture workspace root identity")
            .storage_value()
            .expect("serialize workspace root identity");
        let entry = ManifestEntry {
            id: "b".repeat(64),
            root: workspace.0.to_string_lossy().into_owned(),
            root_identity: Some(root_identity),
            path: "nested/note.txt".to_owned(),
            baseline_exists: false,
            baseline_hash: None,
            baseline_blob_hash: None,
            expected_exists: false,
            expected_hash: None,
            added_lines: None,
            removed_lines: None,
            is_binary: false,
            diff_available: true,
            status: "active".to_owned(),
        };
        let mut target = super::RevertFileTarget::open(&entry).expect("open tracked revert target");
        let nested = workspace.0.join("nested");
        let moved = workspace.0.join("nested-moved");

        assert!(target.matches_entry(&entry).expect("verify missing file"));
        target
            .write_file(b"created")
            .expect("create file through retained parent capability");
        assert_eq!(
            target
                .capture()
                .expect("capture created file")
                .as_ref()
                .and_then(|snapshot| snapshot.content.as_deref()),
            Some(b"created".as_slice())
        );
        assert!(fs::rename(&nested, &moved).is_err());

        drop(target);
        fs::rename(&nested, &moved).expect("created parent is movable after target drops");
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn workspace_root_symlink_is_rejected() {
        let workspace = TestDirectory::new("symlink-root-workspace");
        let external = TestDirectory::new("symlink-root-external");
        let external_file = external.0.join("secret.txt");
        fs::write(&external_file, b"keep private").expect("write external file");
        let workspace_link = workspace.0.join("linked-root");
        create_directory_symlink(&external.0, &workspace_link)
            .expect("create workspace-root symlink");

        assert!(open_workspace_directory(&workspace_link).is_err());
        assert!(capture_workspace(&workspace_link).is_err());
        assert_eq!(
            fs::read(&external_file).expect("read external file after rejected root"),
            b"keep private"
        );
    }
    #[test]
    fn bounded_file_read_rejects_growth_past_the_diff_limit() {
        let root = TestDirectory::new("workspace");
        let path = root.0.join("large.txt");
        fs::write(&path, b"12345").expect("write file");
        let bytes = read_bounded_file(&path, 4).expect("read bounded file");
        assert!(bytes.is_none());
    }
    #[test]
    fn a_reverted_file_can_be_changed_again_from_the_original_baseline() {
        let root = TestDirectory::new("workspace");
        let data = TestStorage::new("data");
        let id = record_file_change(
            &root.0,
            &data.storage,
            "conversation-3",
            "again.txt",
            None,
            Some(b"first\n"),
        );
        revert_change(&data.storage, "conversation-3", &id).expect("revert first edit");
        let second_id = record_file_change(
            &root.0,
            &data.storage,
            "conversation-3",
            "again.txt",
            None,
            Some(b"second\n"),
        );
        assert_eq!(id, second_id);
        let changes = list_changes(&data.storage, "conversation-3").expect("list changes");
        assert_eq!(changes[0].status, "active");
    }
    #[test]
    fn deleting_change_history_removes_only_the_requested_conversation() {
        let root = TestDirectory::new("workspace");
        let data = TestStorage::new("data");
        record_file_change(
            &root.0,
            &data.storage,
            "conversation-to-remove",
            "first.txt",
            None,
            Some(b"first\n"),
        );
        record_file_change(
            &root.0,
            &data.storage,
            "conversation-to-keep",
            "second.txt",
            None,
            Some(b"second\n"),
        );
        delete_conversation_changes(&data.storage, "conversation-to-remove")
            .expect("delete selected conversation changes");
        assert!(
            list_changes(&data.storage, "conversation-to-remove")
                .expect("list deleted conversation changes")
                .is_empty()
        );
        assert_eq!(
            list_changes(&data.storage, "conversation-to-keep")
                .expect("list retained conversation changes")
                .len(),
            1
        );
    }
    #[test]
    fn deleting_all_change_history_is_idempotent() {
        let root = TestDirectory::new("workspace");
        let data = TestStorage::new("data");
        record_file_change(
            &root.0,
            &data.storage,
            "conversation-to-remove",
            "first.txt",
            None,
            Some(b"first\n"),
        );
        delete_all_changes(&data.storage).expect("delete all tracked changes");
        delete_all_changes(&data.storage).expect("repeat deleting all tracked changes");
        assert!(
            list_changes(&data.storage, "conversation-to-remove")
                .expect("list removed changes")
                .is_empty()
        );
    }
    #[test]
    fn legacy_manifests_import_once_and_are_removed_after_commit() {
        let data = TestStorage::new("legacy-import");
        let conversation_id = "legacy-conversation";
        data.add_conversation(conversation_id);
        let workspace = TestDirectory::new("legacy-workspace");
        let original = b"before\n";
        let current = b"after\n";
        fs::write(workspace.0.join("note.txt"), current).expect("write current workspace file");
        let canonical_root = workspace
            .0
            .canonicalize()
            .expect("canonicalize legacy workspace");
        let root_string = canonical_root.to_string_lossy().into_owned();
        let change_id = super::change_id(&canonical_root, "note.txt");
        let legacy_directory = data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .join(super::sha256(conversation_id.as_bytes()));
        let backup_directory = legacy_directory.join("backups");
        fs::create_dir_all(&backup_directory).expect("create legacy backup directory");
        fs::write(
            backup_directory.join(format!("{change_id}.before")),
            original,
        )
        .expect("write legacy baseline");
        let manifest = Manifest {
            version: MANIFEST_VERSION,
            files: vec![ManifestEntry {
                id: change_id.clone(),
                root: root_string,
                root_identity: None,
                path: "note.txt".to_owned(),
                baseline_exists: true,
                baseline_hash: Some(super::sha256(original)),
                baseline_blob_hash: Some(format!("{change_id}.before")),
                expected_exists: true,
                expected_hash: Some(super::sha256(current)),
                added_lines: Some(1),
                removed_lines: Some(1),
                is_binary: false,
                diff_available: true,
                status: "active".to_owned(),
            }],
        };
        fs::write(
            legacy_directory.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("serialize legacy manifest"),
        )
        .expect("write legacy manifest");
        let changes = list_changes(&data.storage, conversation_id).expect("import legacy data");
        assert_eq!(changes.len(), 1);
        assert!(!changes[0].can_revert);
        assert!(!changes[0].diff_available);
        let diff = read_diff(&data.storage, conversation_id, &change_id)
            .expect("read unavailable legacy diff");
        assert_eq!(diff["available"].as_bool(), Some(false));
        assert_eq!(
            revert_change(&data.storage, conversation_id, &change_id)
                .expect_err("refuse an identity-less legacy revert"),
            RevertError::Conflict
        );
        assert_eq!(
            fs::read(workspace.0.join("note.txt")).expect("read the current workspace file"),
            current
        );
        assert!(!legacy_directory.exists());
        assert!(
            !data
                ._directory
                .0
                .join(LEGACY_FILE_CHANGE_DIRECTORY)
                .exists()
        );
        let baseline_hash = super::sha256(original);
        assert!(
            data._directory
                .0
                .join("attachments")
                .join(SNAPSHOT_DIRECTORY)
                .join(&baseline_hash[..SNAPSHOT_SHARD_HEX_LENGTH])
                .join(format!("{baseline_hash}.blob"))
                .is_file()
        );
        assert_eq!(
            list_changes(&data.storage, conversation_id)
                .expect("list imported data again")
                .len(),
            1
        );
    }
    #[test]
    fn malformed_legacy_manifest_is_kept_for_recovery() {
        let data = TestStorage::new("malformed-legacy");
        let conversation_id = "broken-legacy-conversation";
        data.add_conversation(conversation_id);
        let legacy_directory = data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .join(super::sha256(conversation_id.as_bytes()));
        fs::create_dir_all(&legacy_directory).expect("create legacy directory");
        fs::write(legacy_directory.join("manifest.json"), b"not valid json")
            .expect("write malformed legacy manifest");
        assert!(list_changes(&data.storage, conversation_id).is_err());
        assert!(legacy_directory.join("manifest.json").is_file());
        let missing_manifest_conversation = "missing-manifest-conversation";
        data.add_conversation(missing_manifest_conversation);
        let orphaned_backup = data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .join(super::sha256(missing_manifest_conversation.as_bytes()))
            .join("backups")
            .join("retained.before");
        fs::create_dir_all(orphaned_backup.parent().expect("backup parent"))
            .expect("create legacy backup directory");
        fs::write(&orphaned_backup, b"preserve unknown legacy data")
            .expect("write orphaned legacy backup");
        assert!(list_changes(&data.storage, missing_manifest_conversation).is_err());
        assert!(orphaned_backup.is_file());
    }
    #[test]
    fn shared_snapshots_are_reclaimed_only_after_the_last_conversation_is_deleted() {
        let first_workspace = TestDirectory::new("shared-workspace-one");
        let second_workspace = TestDirectory::new("shared-workspace-two");
        let data = TestStorage::new("shared-blobs");
        let first_id = record_file_change(
            &first_workspace.0,
            &data.storage,
            "first-conversation",
            "note.txt",
            Some(b"same baseline\n"),
            Some(b"first edit\n"),
        );
        record_file_change(
            &second_workspace.0,
            &data.storage,
            "second-conversation",
            "note.txt",
            Some(b"same baseline\n"),
            Some(b"second edit\n"),
        );
        let blob_path = data
            ._directory
            .0
            .join("attachments")
            .join(SNAPSHOT_DIRECTORY)
            .join(&super::sha256(b"same baseline\n")[..SNAPSHOT_SHARD_HEX_LENGTH])
            .join(format!("{}.blob", super::sha256(b"same baseline\n")));
        assert!(blob_path.is_file());
        assert_eq!(
            data.storage
                .connect()
                .expect("connect test storage")
                .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count deduplicated blobs"),
            1
        );
        delete_conversation_changes(&data.storage, "first-conversation")
            .expect("delete first conversation history");
        assert!(blob_path.is_file());
        assert!(
            list_changes(&data.storage, "second-conversation")
                .expect("read second conversation")
                .iter()
                .any(|change| change.id != first_id)
        );
        delete_conversation_changes(&data.storage, "second-conversation")
            .expect("delete last conversation history");
        assert!(!blob_path.exists());
        assert_eq!(
            data.storage
                .connect()
                .expect("connect test storage")
                .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count remaining blobs"),
            0
        );
        assert!(
            !data
                ._directory
                .0
                .join(LEGACY_FILE_CHANGE_DIRECTORY)
                .exists()
        );
    }
    #[test]
    fn stale_writing_blob_cleanup_recovers_a_crash_before_record_commit() {
        let data = TestStorage::new("pending-blob");
        let content = b"unreferenced snapshot";
        let hash = super::sha256(content);
        let path = super::snapshot_blob_path(&data._directory.0, &hash, true)
            .expect("create snapshot path")
            .expect("snapshot path should exist");
        fs::write(&path, content).expect("simulate blob written before interrupted record");
        data.storage            .connect()            .expect("connect test storage")            .execute(                "INSERT INTO file_change_blobs (hash, size_bytes, state, created_at_unix_ms)                 VALUES (?1, ?2, 'writing', 1)",                rusqlite::params![hash, content.len() as i64],            )            .expect("persist interrupted blob marker");
        super::cleanup_pending_snapshot_blobs(&data.storage)
            .expect("reclaim interrupted unreferenced snapshot");
        assert!(!path.exists());
        assert_eq!(
            data.storage
                .connect()
                .expect("connect test storage")
                .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count cleaned snapshots"),
            0
        );
    }
    #[test]
    fn conversation_foreign_key_cascade_marks_its_last_blob_for_cleanup() {
        let workspace = TestDirectory::new("cascade-workspace");
        let data = TestStorage::new("cascade-delete");
        record_file_change(
            &workspace.0,
            &data.storage,
            "deleted-conversation",
            "note.txt",
            Some(b"baseline"),
            Some(b"edited"),
        );
        let hash = super::sha256(b"baseline");
        let blob_path = super::snapshot_blob_path(&data._directory.0, &hash, false)
            .expect("locate snapshot")
            .expect("snapshot exists");
        data.storage
            .connect()
            .expect("connect test storage")
            .execute(
                "DELETE FROM conversations WHERE id = 'deleted-conversation'",
                [],
            )
            .expect("delete conversation with cascading change history");
        assert!(blob_path.is_file());
        assert_eq!(
            data.storage
                .connect()
                .expect("connect test storage")
                .query_row(
                    "SELECT state FROM file_change_blobs WHERE hash = ?1",
                    [&hash],
                    |row| row.get::<_, String>(0),
                )
                .expect("read blob cleanup state"),
            "pending"
        );
        super::cleanup_pending_snapshot_blobs(&data.storage)
            .expect("clean orphaned cascade snapshot");
        assert!(!blob_path.exists());
    }
    #[test]
    fn approve_change_and_revert_hunk_update_tracked_state() {
        let workspace = TestDirectory::new("approve-and-hunk-workspace");
        let data = TestStorage::new("approve-and-hunk-data");
        let baseline = b"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
        let edited =
            b"line1-mod\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10-mod\n";
        std::fs::write(workspace.0.join("multi.txt"), edited).expect("write edited file");
        let id = record_file_change(
            &workspace.0,
            &data.storage,
            "hunk-conversation",
            "multi.txt",
            Some(baseline),
            Some(edited),
        ); // Revert only the first hunk (hunk 0), preserving the second hunk at line 10.
        let after_hunk = super::revert_hunk(&data.storage, "hunk-conversation", &id, 0)
            .expect("revert first hunk");
        assert_eq!(after_hunk[0].status, "active");
        assert!(!after_hunk[0].approved);
        let on_disk =
            std::fs::read_to_string(workspace.0.join("multi.txt")).expect("read updated file");
        assert!(on_disk.starts_with("line1\n"));
        assert!(on_disk.ends_with("line10-mod\n")); // Approve the remaining change.
        let after_approve = super::approve_change(&data.storage, "hunk-conversation", &id)
            .expect("approve remaining change");
        assert!(after_approve[0].approved);
        assert!(!after_approve[0].can_revert);
    }
}
