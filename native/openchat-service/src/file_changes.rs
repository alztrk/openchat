use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};

const MANIFEST_VERSION: u8 = 1;
const MAX_WORKSPACE_FILES: usize = 20_000;
const MAX_WORKSPACE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WORKSPACE_SNAPSHOT_CONTENT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SNAPSHOT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXT_DIFF_BYTES: usize = 1024 * 1024;
const MAX_DIFF_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_DIFF_LINES: usize = 4_000;

static FILE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileChangeSummary {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub added_lines: Option<usize>,
    pub removed_lines: Option<usize>,
    pub is_binary: bool,
    pub diff_available: bool,
    pub status: String,
    pub can_revert: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FileSnapshot {
    pub hash: String,
    pub content: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceSnapshot {
    pub root: PathBuf,
    pub files: BTreeMap<String, FileSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct FileChangeDelta {
    pub path: String,
    pub before: Option<FileSnapshot>,
    pub after: Option<FileSnapshot>,
}

#[derive(Debug)]
pub(crate) enum TrackingError {
    WorkspaceUnavailable,
    WorkspaceTooLarge,
    SnapshotUnavailable,
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
    path: String,
    baseline_exists: bool,
    baseline_hash: Option<String>,
    backup_file: Option<String>,
    expected_exists: bool,
    expected_hash: Option<String>,
    added_lines: Option<usize>,
    removed_lines: Option<usize>,
    is_binary: bool,
    diff_available: bool,
    status: String,
}

pub(crate) fn capture_file(
    root: &Path,
    relative_path: &str,
) -> Result<Option<FileSnapshot>, TrackingError> {
    let root = root
        .canonicalize()
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let relative = safe_relative_path(relative_path)?;
    let path = root.join(relative);
    ensure_no_symlink_path(&root, &path)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(TrackingError::SnapshotUnavailable)
        }
        Ok(metadata) if metadata.is_file() => snapshot_file(&path, metadata.len()).map(Some),
        Ok(_) => Err(TrackingError::SnapshotUnavailable),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(TrackingError::SnapshotUnavailable),
    }
}

pub(crate) fn capture_workspace(root: &Path) -> Result<WorkspaceSnapshot, TrackingError> {
    let root = root
        .canonicalize()
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    if !root.is_dir() {
        return Err(TrackingError::WorkspaceUnavailable);
    }

    let mut files = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut total_snapshot_content_bytes = 0u64;
    collect_workspace_files(
        &root,
        &root,
        &mut files,
        &mut total_bytes,
        &mut total_snapshot_content_bytes,
    )?;
    Ok(WorkspaceSnapshot { root, files })
}

pub(crate) fn workspace_deltas(
    before: &WorkspaceSnapshot,
    after: &WorkspaceSnapshot,
) -> Result<Vec<FileChangeDelta>, TrackingError> {
    if before.root != after.root {
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
                path,
                before: old.cloned(),
                after: new.cloned(),
            })
        })
        .collect())
}

pub(crate) fn record_deltas(
    data_root: &Path,
    conversation_id: &str,
    root: &Path,
    deltas: Vec<FileChangeDelta>,
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    if deltas.is_empty() {
        return Ok(Vec::new());
    }
    validate_conversation_id(conversation_id)?;
    let root = root
        .canonicalize()
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let _guard = lock_file_changes()?;
    let directory = ensure_conversation_directory(data_root, conversation_id, true)?
        .ok_or(TrackingError::StorageUnavailable)?;
    ensure_child_directory(&directory, "backups", true)?
        .ok_or(TrackingError::StorageUnavailable)?;
    let mut manifest = read_manifest(&directory)?;
    if manifest.version == 0 {
        manifest.version = MANIFEST_VERSION;
    }

    let mut changed_summaries = Vec::with_capacity(deltas.len());
    for delta in deltas {
        let relative = safe_relative_path(&delta.path)?;
        let normalized = relative.to_string_lossy().replace('\\', "/");
        let id = change_id(&root, &normalized);
        let before_hash = delta.before.as_ref().map(|state| state.hash.as_str());
        let after_hash = delta.after.as_ref().map(|state| state.hash.as_str());
        if before_hash == after_hash {
            continue;
        }

        let existing_index = manifest.files.iter().position(|entry| entry.id == id);
        let (baseline_exists, baseline_hash, backup_file, was_conflicted) =
            if let Some(index) = existing_index {
                let existing = &manifest.files[index];
                let expected = existing.expected_hash.as_deref();
                let baseline_is_current = if existing.baseline_exists {
                    before_hash == existing.baseline_hash.as_deref()
                } else {
                    delta.before.is_none()
                };
                let matches_expected =
                    before_hash == expected && delta.before.is_some() == existing.expected_exists;
                (
                    existing.baseline_exists,
                    existing.baseline_hash.clone(),
                    existing.backup_file.clone(),
                    existing.status == "conflict" || (!matches_expected && !baseline_is_current),
                )
            } else {
                let baseline_exists = delta.before.is_some();
                let baseline_hash = delta.before.as_ref().map(|state| state.hash.clone());
                let backup_file = if baseline_exists
                    && delta
                        .before
                        .as_ref()
                        .is_some_and(|state| state.content.is_some())
                {
                    let name = format!("{id}.before");
                    let content = delta
                        .before
                        .as_ref()
                        .and_then(|state| state.content.as_deref())
                        .ok_or(TrackingError::SnapshotUnavailable)?;
                    atomic_write(&directory.join("backups").join(&name), content)
                        .map_err(|_| TrackingError::StorageUnavailable)?;
                    Some(name)
                } else {
                    None
                };
                (baseline_exists, baseline_hash, backup_file, false)
            };

        let baseline = if baseline_exists {
            match backup_file.as_deref() {
                Some(name) => Some(
                    read_bounded_file(
                        &directory.join("backups").join(name),
                        MAX_SNAPSHOT_FILE_BYTES as usize,
                    )
                    .map_err(|_| TrackingError::StorageUnavailable)?
                    .ok_or(TrackingError::StorageUnavailable)?,
                ),
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
                _ => (None, None, true, false),
            };
        let status = if was_conflicted {
            "conflict".to_owned()
        } else if baseline_exists == delta.after.is_some() && baseline_hash.as_deref() == after_hash
        {
            "reverted".to_owned()
        } else {
            "active".to_owned()
        };
        let entry = ManifestEntry {
            id: id.clone(),
            root: root.to_string_lossy().into_owned(),
            path: normalized,
            baseline_exists,
            baseline_hash,
            backup_file,
            expected_exists: delta.after.is_some(),
            expected_hash: delta.after.map(|state| state.hash),
            added_lines,
            removed_lines,
            is_binary,
            diff_available,
            status,
        };
        let summary = summary_for_entry(data_root, conversation_id, &entry, false);
        if let Some(index) = existing_index {
            manifest.files[index] = entry;
        } else {
            manifest.files.push(entry);
        }
        changed_summaries.push(summary);
    }

    write_manifest(&directory, &manifest)?;
    Ok(changed_summaries)
}

pub(crate) fn list_changes(
    data_root: &Path,
    conversation_id: &str,
) -> Result<Vec<FileChangeSummary>, TrackingError> {
    validate_conversation_id(conversation_id)?;
    let _guard = lock_file_changes()?;
    let Some(directory) = ensure_conversation_directory(data_root, conversation_id, false)? else {
        return Ok(Vec::new());
    };
    let manifest = read_manifest(&directory)?;
    Ok(manifest
        .files
        .iter()
        .map(|entry| summary_for_entry(data_root, conversation_id, entry, true))
        .collect())
}

pub(crate) fn read_diff(
    data_root: &Path,
    conversation_id: &str,
    id: &str,
) -> Result<serde_json::Value, TrackingError> {
    validate_conversation_id(conversation_id)?;
    validate_change_id(id)?;
    let _guard = lock_file_changes()?;
    let directory = ensure_conversation_directory(data_root, conversation_id, false)?
        .ok_or(TrackingError::SnapshotUnavailable)?;
    let manifest = read_manifest(&directory)?;
    let entry = manifest
        .files
        .iter()
        .find(|entry| entry.id == id)
        .ok_or(TrackingError::SnapshotUnavailable)?;
    let old = read_baseline(&directory, entry)?;
    let current_path = resolve_record_path(entry)?;
    let new = match fs::symlink_metadata(&current_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(TrackingError::SnapshotUnavailable);
        }
        Ok(metadata) if metadata.len() > MAX_SNAPSHOT_FILE_BYTES => None,
        Ok(_) => read_bounded_file(&current_path, MAX_SNAPSHOT_FILE_BYTES as usize)
            .map_err(|_| TrackingError::SnapshotUnavailable)?,
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

pub(crate) fn revert_change(
    data_root: &Path,
    conversation_id: &str,
    id: &str,
) -> Result<Vec<FileChangeSummary>, RevertError> {
    validate_conversation_id(conversation_id).map_err(|_| RevertError::Unavailable)?;
    validate_change_id(id).map_err(|_| RevertError::Unavailable)?;
    let _guard = lock_file_changes().map_err(|_| RevertError::Unavailable)?;
    let directory = ensure_conversation_directory(data_root, conversation_id, false)
        .map_err(|_| RevertError::Unavailable)?
        .ok_or(RevertError::Unavailable)?;
    let mut manifest = read_manifest(&directory).map_err(|_| RevertError::Unavailable)?;
    let index = manifest
        .files
        .iter()
        .position(|entry| entry.id == id)
        .ok_or(RevertError::Unavailable)?;
    let entry = &manifest.files[index];
    if entry.status == "reverted" {
        return Err(RevertError::AlreadyReverted);
    }
    if entry.status == "conflict"
        || !is_current_version(entry).map_err(|_| RevertError::Unavailable)?
    {
        manifest.files[index].status = "conflict".to_owned();
        write_manifest(&directory, &manifest).map_err(|_| RevertError::Unavailable)?;
        return Err(RevertError::Conflict);
    }

    let baseline_exists = entry.baseline_exists;
    let baseline_hash = entry.baseline_hash.clone();
    let target = resolve_record_path(entry).map_err(|_| RevertError::Unavailable)?;
    if baseline_exists {
        let content = read_baseline(&directory, entry)
            .map_err(|_| RevertError::Unavailable)?
            .ok_or(RevertError::Unavailable)?;
        if sha256(&content) != entry.baseline_hash.as_deref().unwrap_or_default() {
            return Err(RevertError::Unavailable);
        }
        let parent = target.parent().ok_or(RevertError::Unavailable)?;
        fs::create_dir_all(parent).map_err(|_| RevertError::Unavailable)?;
        ensure_no_symlink_path(Path::new(&entry.root), &target)
            .map_err(|_| RevertError::Unavailable)?;
        atomic_write(&target, &content).map_err(|_| RevertError::Unavailable)?;
    } else {
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(RevertError::Conflict);
            }
            Ok(_) => fs::remove_file(&target).map_err(|_| RevertError::Unavailable)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(RevertError::Unavailable),
        }
    }

    manifest.files[index].status = "reverted".to_owned();
    manifest.files[index].expected_exists = baseline_exists;
    manifest.files[index].expected_hash = baseline_hash;
    write_manifest(&directory, &manifest).map_err(|_| RevertError::Unavailable)?;
    Ok(manifest
        .files
        .iter()
        .map(|entry| summary_for_entry(data_root, conversation_id, entry, true))
        .collect())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RevertError {
    AlreadyReverted,
    Conflict,
    Unavailable,
}

pub(crate) fn delete_conversation_changes(
    data_root: &Path,
    conversation_id: &str,
) -> Result<(), TrackingError> {
    validate_conversation_id(conversation_id)?;
    let _guard = lock_file_changes()?;
    let Some(directory) = ensure_conversation_directory(data_root, conversation_id, false)? else {
        return Ok(());
    };
    fs::remove_dir_all(directory).map_err(|_| TrackingError::StorageUnavailable)
}

pub(crate) fn delete_all_changes(data_root: &Path) -> Result<(), TrackingError> {
    let _guard = lock_file_changes()?;
    let root = data_root
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let Some(directory) = ensure_child_directory(&root, "file-changes", false)? else {
        return Ok(());
    };
    fs::remove_dir_all(directory).map_err(|_| TrackingError::StorageUnavailable)
}

fn collect_workspace_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, FileSnapshot>,
    total_bytes: &mut u64,
    total_snapshot_content_bytes: &mut u64,
) -> Result<(), TrackingError> {
    let entries = fs::read_dir(directory).map_err(|_| TrackingError::SnapshotUnavailable)?;
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
            collect_workspace_files(
                root,
                &entry.path(),
                files,
                total_bytes,
                total_snapshot_content_bytes,
            )?;
        } else if file_type.is_file() {
            if files.len() >= MAX_WORKSPACE_FILES {
                return Err(TrackingError::WorkspaceTooLarge);
            }
            let metadata = entry
                .metadata()
                .map_err(|_| TrackingError::SnapshotUnavailable)?;
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
            let file = snapshot_file(&entry.path(), metadata.len())?;
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| TrackingError::SnapshotUnavailable)?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, file);
        }
    }
    Ok(())
}

fn snapshot_file(path: &Path, size: u64) -> Result<FileSnapshot, TrackingError> {
    let mut file = fs::File::open(path).map_err(|_| TrackingError::SnapshotUnavailable)?;
    let mut hasher = Sha256::new();
    let mut content = (size <= MAX_SNAPSHOT_FILE_BYTES).then(|| Vec::with_capacity(size as usize));
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| TrackingError::SnapshotUnavailable)?;
        if read == 0 {
            break;
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

fn summary_for_entry(
    data_root: &Path,
    conversation_id: &str,
    entry: &ManifestEntry,
    validate_current: bool,
) -> FileChangeSummary {
    let mut status = entry.status.clone();
    let expected_matches = !validate_current || is_current_version(entry).unwrap_or(false);
    if validate_current && !expected_matches && status != "reverted" {
        status = "conflict".to_owned();
    }
    let backup_available = !entry.baseline_exists
        || entry.backup_file.as_deref().is_some_and(|name| {
            name == format!("{}.before", entry.id)
                && safe_backup_path(data_root, conversation_id, name).is_some_and(|path| {
                    fs::symlink_metadata(path).is_ok_and(|metadata| {
                        !metadata.file_type().is_symlink() && metadata.is_file()
                    })
                })
        });
    let can_revert = status == "active" && expected_matches && backup_available;
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
    }
}

fn is_current_version(entry: &ManifestEntry) -> Result<bool, TrackingError> {
    let path = resolve_record_path(entry)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Ok(false),
        Ok(metadata) if metadata.len() <= MAX_SNAPSHOT_FILE_BYTES => {
            let current = snapshot_file(&path, metadata.len())?;
            Ok(entry.expected_exists && Some(current.hash) == entry.expected_hash)
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(!entry.expected_exists),
        Err(_) => Err(TrackingError::SnapshotUnavailable),
    }
}

fn resolve_record_path(entry: &ManifestEntry) -> Result<PathBuf, TrackingError> {
    let root = PathBuf::from(&entry.root)
        .canonicalize()
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let relative = safe_relative_path(&entry.path)?;
    let path = root.join(relative);
    ensure_no_symlink_path(&root, &path)?;
    if !path.starts_with(&root) {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    Ok(path)
}

fn ensure_no_symlink_path(root: &Path, path: &Path) -> Result<(), TrackingError> {
    if !path.starts_with(root) {
        return Err(TrackingError::WorkspaceUnavailable);
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| TrackingError::WorkspaceUnavailable)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(TrackingError::WorkspaceUnavailable);
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TrackingError::WorkspaceUnavailable);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(TrackingError::SnapshotUnavailable),
        }
    }
    Ok(())
}

fn read_baseline(
    directory: &Path,
    entry: &ManifestEntry,
) -> Result<Option<Vec<u8>>, TrackingError> {
    if !entry.baseline_exists {
        return Ok(Some(Vec::new()));
    }
    let Some(name) = entry.backup_file.as_deref() else {
        return Ok(None);
    };
    if name != format!("{}.before", entry.id) {
        return Err(TrackingError::SnapshotUnavailable);
    }
    let backups = ensure_child_directory(directory, "backups", false)?
        .ok_or(TrackingError::SnapshotUnavailable)?;
    let path = backups.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| TrackingError::SnapshotUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrackingError::SnapshotUnavailable);
    }
    read_bounded_file(&path, MAX_SNAPSHOT_FILE_BYTES as usize)
        .map_err(|_| TrackingError::SnapshotUnavailable)?
        .map(Some)
        .ok_or(TrackingError::SnapshotUnavailable)
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

fn safe_backup_path(data_root: &Path, conversation_id: &str, backup_name: &str) -> Option<PathBuf> {
    let id = backup_name.strip_suffix(".before")?;
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let directory = ensure_conversation_directory(data_root, conversation_id, false).ok()??;
    let backups = ensure_child_directory(&directory, "backups", false).ok()??;
    Some(backups.join(backup_name))
}

fn ensure_conversation_directory(
    data_root: &Path,
    conversation_id: &str,
    create: bool,
) -> Result<Option<PathBuf>, TrackingError> {
    let root = data_root
        .canonicalize()
        .map_err(|_| TrackingError::StorageUnavailable)?;
    let Some(base) = ensure_child_directory(&root, "file-changes", create)? else {
        return Ok(None);
    };
    ensure_child_directory(&base, &sha256(conversation_id.as_bytes()), create)
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
            let bytes = fs::read(path).map_err(|_| TrackingError::StorageUnavailable)?;
            let manifest: Manifest =
                serde_json::from_slice(&bytes).map_err(|_| TrackingError::StorageUnavailable)?;
            if manifest.version != MANIFEST_VERSION {
                return Err(TrackingError::StorageUnavailable);
            }
            Ok(manifest)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Manifest {
            version: MANIFEST_VERSION,
            files: Vec::new(),
        }),
        Err(_) => Err(TrackingError::StorageUnavailable),
    }
}

fn write_manifest(directory: &Path, manifest: &Manifest) -> Result<(), TrackingError> {
    let serialized = serde_json::to_vec(manifest).map_err(|_| TrackingError::StorageUnavailable)?;
    atomic_write(&directory.join("manifest.json"), &serialized)
        .map_err(|_| TrackingError::StorageUnavailable)
}

fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".openchat-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp_path, content)?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn lock_file_changes() -> Result<std::sync::MutexGuard<'static, ()>, TrackingError> {
    FILE_CHANGE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| TrackingError::StorageUnavailable)
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
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        RevertError, capture_file, capture_workspace, delete_all_changes,
        delete_conversation_changes, list_changes, read_bounded_file, read_diff, record_deltas,
        revert_change, snapshot_file, workspace_deltas,
    };

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

    fn record_file_change(
        root: &std::path::Path,
        data_root: &std::path::Path,
        conversation_id: &str,
        path: &str,
        old: Option<&[u8]>,
        new: Option<&[u8]>,
    ) -> String {
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
        let delta = super::FileChangeDelta {
            path: normalized,
            before,
            after,
        };
        record_deltas(data_root, conversation_id, root, vec![delta])
            .expect("record file change")
            .first()
            .expect("one recorded file change")
            .id
            .clone()
    }

    #[test]
    fn tracks_text_changes_and_reverts_to_the_first_version() {
        let root = TestDirectory::new("workspace");
        let data = TestDirectory::new("data");
        fs::write(root.0.join("note.txt"), "first\nsecond\n").expect("write initial file");
        let before = capture_file(&root.0, "note.txt")
            .expect("capture before")
            .expect("initial file exists");
        fs::write(root.0.join("note.txt"), "first\nchanged\nthird\n").expect("write changed file");
        let after = capture_file(&root.0, "note.txt")
            .expect("capture after")
            .expect("changed file exists");
        let summaries = record_deltas(
            &data.0,
            "conversation-1",
            &root.0,
            vec![super::FileChangeDelta {
                path: "note.txt".to_owned(),
                before: Some(before),
                after: Some(after),
            }],
        )
        .expect("save change");
        assert_eq!(summaries[0].added_lines, Some(2));
        assert_eq!(summaries[0].removed_lines, Some(1));
        assert!(summaries[0].can_revert);
        let diff = read_diff(&data.0, "conversation-1", &summaries[0].id).expect("read diff");
        assert!(
            diff["diff"]
                .as_str()
                .is_some_and(|text| text.contains("+third"))
        );

        let changes =
            revert_change(&data.0, "conversation-1", &summaries[0].id).expect("revert change");
        assert_eq!(changes[0].status, "reverted");
        assert_eq!(
            fs::read_to_string(root.0.join("note.txt")).expect("read restored file"),
            "first\nsecond\n"
        );
    }

    #[test]
    fn tracks_created_and_deleted_files_and_detects_external_edits_before_revert() {
        let root = TestDirectory::new("workspace");
        let data = TestDirectory::new("data");
        let created_id = record_file_change(
            &root.0,
            &data.0,
            "conversation-2",
            "new.txt",
            None,
            Some(b"created\n"),
        );
        revert_change(&data.0, "conversation-2", &created_id).expect("remove created file");
        assert!(!root.0.join("new.txt").exists());

        let deleted_id = record_file_change(
            &root.0,
            &data.0,
            "conversation-2",
            "gone.txt",
            Some(b"restore me\n"),
            None,
        );
        revert_change(&data.0, "conversation-2", &deleted_id).expect("restore deleted file");
        assert_eq!(
            fs::read_to_string(root.0.join("gone.txt")).expect("read restored file"),
            "restore me\n"
        );

        let conflict_id = record_file_change(
            &root.0,
            &data.0,
            "conversation-2",
            "gone.txt",
            Some(b"restore me\n"),
            Some(b"assistant edit\n"),
        );
        fs::write(root.0.join("gone.txt"), "user edit\n").expect("write user edit");
        assert!(matches!(
            revert_change(&data.0, "conversation-2", &conflict_id),
            Err(RevertError::Conflict)
        ));
        assert_eq!(
            fs::read_to_string(root.0.join("gone.txt")).expect("read conflicted file"),
            "user edit\n"
        );
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
    fn file_growth_after_metadata_read_does_not_retain_unbounded_content() {
        let root = TestDirectory::new("workspace");
        let path = root.0.join("growing.txt");
        let content = vec![b'x'; 2 * 1024 * 1024];
        fs::write(&path, &content).expect("write growing file");

        let snapshot = snapshot_file(&path, 1).expect("snapshot growing file");

        assert_eq!(snapshot.hash, super::sha256(&content));
        assert!(snapshot.content.is_none());
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
        let data = TestDirectory::new("data");
        let id = record_file_change(
            &root.0,
            &data.0,
            "conversation-3",
            "again.txt",
            None,
            Some(b"first\n"),
        );
        revert_change(&data.0, "conversation-3", &id).expect("revert first edit");
        let second_id = record_file_change(
            &root.0,
            &data.0,
            "conversation-3",
            "again.txt",
            None,
            Some(b"second\n"),
        );
        assert_eq!(id, second_id);
        let changes = list_changes(&data.0, "conversation-3").expect("list changes");
        assert_eq!(changes[0].status, "active");
    }

    #[test]
    fn deleting_change_history_removes_only_the_requested_conversation() {
        let root = TestDirectory::new("workspace");
        let data = TestDirectory::new("data");
        record_file_change(
            &root.0,
            &data.0,
            "conversation-to-remove",
            "first.txt",
            None,
            Some(b"first\n"),
        );
        record_file_change(
            &root.0,
            &data.0,
            "conversation-to-keep",
            "second.txt",
            None,
            Some(b"second\n"),
        );

        delete_conversation_changes(&data.0, "conversation-to-remove")
            .expect("delete selected conversation changes");

        assert!(
            list_changes(&data.0, "conversation-to-remove")
                .expect("list deleted conversation changes")
                .is_empty()
        );
        assert_eq!(
            list_changes(&data.0, "conversation-to-keep")
                .expect("list retained conversation changes")
                .len(),
            1
        );
    }

    #[test]
    fn deleting_all_change_history_is_idempotent() {
        let root = TestDirectory::new("workspace");
        let data = TestDirectory::new("data");
        record_file_change(
            &root.0,
            &data.0,
            "conversation-to-remove",
            "first.txt",
            None,
            Some(b"first\n"),
        );

        delete_all_changes(&data.0).expect("delete all tracked changes");
        delete_all_changes(&data.0).expect("repeat deleting all tracked changes");

        assert!(
            list_changes(&data.0, "conversation-to-remove")
                .expect("list removed changes")
                .is_empty()
        );
    }
}
