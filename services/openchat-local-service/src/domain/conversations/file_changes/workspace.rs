//! Filesystem capability and workspace directory operations for change capture.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

use cap_std::fs::{Dir, Metadata as CapabilityMetadata, OpenOptions as CapabilityOpenOptions};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use super::types::*;
use crate::filesystem::OpenedFile as CapabilityFile;

#[cfg(test)]
pub(crate) fn capture_file(
    root: &Path,
    relative_path: &str,
) -> Result<Option<FileSnapshot>, TrackingError> {
    let relative = safe_relative_path(relative_path)?;
    let (directory, _) = open_workspace_directory(root)?;
    capture_file_from_directory(&directory, &relative)
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

pub(crate) fn same_directory_object(left: &Dir, right: &Dir) -> io::Result<bool> {
    crate::filesystem::same_directory_object(left, right)
}

pub(crate) fn directory_identity(directory: &Dir) -> io::Result<DirectoryIdentity> {
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
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
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

pub(crate) fn collect_workspace_files(
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

pub(crate) fn snapshot_file(file: impl Read, size: u64) -> Result<FileSnapshot, TrackingError> {
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

pub(crate) fn snapshot_opened_file(
    file: &mut crate::filesystem::OpenedFile,
) -> Result<FileSnapshot, TrackingError> {
    let size = file
        .metadata()
        .map_err(|_| TrackingError::SnapshotUnavailable)?
        .len();
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TrackingError::SnapshotUnavailable)?;
    snapshot_file(file, size)
}

pub(crate) fn read_bounded_file(path: &Path, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
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

pub(crate) fn open_workspace_file<D>(
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

pub(crate) fn read_workspace_file<D>(
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

pub(crate) fn atomic_write_workspace_file_from_directory<D>(
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

pub(crate) fn is_current_version(entry: &ManifestEntry) -> Result<bool, TrackingError> {
    let (directory, _) = open_workspace_directory(Path::new(&entry.root))?;
    let relative = safe_relative_path(&entry.path)?;
    is_current_version_from_directory(entry, &directory, &relative)
}

pub(crate) fn is_current_version_from_directory(
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

pub(crate) fn ensure_child_directory(
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

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
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
