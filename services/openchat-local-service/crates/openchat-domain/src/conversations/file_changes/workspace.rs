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

/// Keeps a checked workspace entry protected until its tracking update completes.
pub(crate) struct WorkspaceMutationGuard {
    #[cfg(windows)]
    file: CapabilityFile,
    #[cfg(windows)]
    delete_pending: bool,
}

impl WorkspaceMutationGuard {
    pub(crate) fn is_delete_pending(&self) -> bool {
        #[cfg(windows)]
        {
            self.delete_pending
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

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
        // SAFETY: `file` owns a live Windows handle and `information` is a writable output.
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

#[cfg(any(not(windows), test))]
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

/// Compares and changes one workspace file while retaining the Windows file handle through the
/// caller's tracking transaction. Windows writes in place under a restrictive share mode; other
/// platforms use an atomic publish after comparison but cannot exclude uncoordinated writers.
pub(crate) fn write_workspace_file_if_unchanged<D>(
    directory: &D,
    relative_path: &Path,
    expected: Option<&FileSnapshot>,
    content: &[u8],
    mutation_guard: &mut Option<WorkspaceMutationGuard>,
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
        None => directory.try_clone_chain()?,
    };
    let file_name = relative_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The workspace path has no file name.",
        )
    })?;

    #[cfg(windows)]
    {
        write_workspace_file_windows(&parent, file_name, expected, content, mutation_guard)
    }
    #[cfg(not(windows))]
    {
        let _ = mutation_guard;
        ensure_workspace_snapshot_matches(&parent, Path::new(file_name), expected)?;
        atomic_write_workspace_file_from_directory(&parent, Path::new(file_name), content)
    }
}

#[cfg(any(not(windows), test))]
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

/// Marks the file identified by its checked handle for deletion and keeps that handle alive until
/// the caller's tracking transaction completes. On Windows the delete-on-close disposition is
/// finalized when the guard is released, reserving the path during compensation and persistence.
pub(crate) fn remove_workspace_file_if_unchanged<D>(
    directory: &D,
    relative_path: &Path,
    expected: &FileSnapshot,
    mutation_guard: &mut Option<WorkspaceMutationGuard>,
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
        None => directory.try_clone_chain()?,
    };
    let file_name = relative_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The workspace path has no file name.",
        )
    })?;

    #[cfg(windows)]
    {
        remove_workspace_file_windows(&parent, file_name, expected, mutation_guard)
    }
    #[cfg(not(windows))]
    {
        let _ = mutation_guard;
        ensure_workspace_snapshot_matches(&parent, Path::new(file_name), Some(expected))?;
        remove_workspace_file_from_directory(&parent, Path::new(file_name))
    }
}

#[cfg(not(windows))]
fn ensure_workspace_snapshot_matches<D>(
    directory: &D,
    file_name: &Path,
    expected: Option<&FileSnapshot>,
) -> io::Result<()>
where
    D: crate::filesystem::DirectoryCapability + ?Sized,
{
    let current = capture_file_from_directory(directory, file_name)
        .map_err(|_| io::Error::other("The workspace file could not be compared."))?;
    if current.as_ref().map(|snapshot| snapshot.hash.as_str())
        != expected.map(|snapshot| snapshot.hash.as_str())
    {
        return Err(workspace_mutation_conflict());
    }
    Ok(())
}

fn workspace_mutation_conflict() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The workspace file changed before the guarded mutation.",
    )
}

#[cfg(windows)]
fn write_workspace_file_windows(
    parent: &crate::filesystem::OpenedDirectory,
    file_name: &std::ffi::OsStr,
    expected: Option<&FileSnapshot>,
    content: &[u8],
    mutation_guard: &mut Option<WorkspaceMutationGuard>,
) -> io::Result<()> {
    let mut created_file = false;
    if mutation_guard.is_none() {
        match expected {
            Some(expected) => {
                let mut file = open_workspace_mutation_file(parent, file_name)
                    .map_err(map_workspace_lock_error)?;
                let current = snapshot_guarded_file(&mut file)?;
                if !workspace_snapshots_equal(Some(&current), Some(expected)) {
                    return Err(workspace_mutation_conflict());
                }
                if expected.content.is_none() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "The workspace file is too large to safely roll back an in-place write.",
                    ));
                }
                ensure_workspace_handle_matches(parent, file_name, &file)?;
                *mutation_guard = Some(WorkspaceMutationGuard {
                    file,
                    delete_pending: false,
                });
            }
            None => {
                let file = create_workspace_mutation_file(parent, Path::new(file_name))
                    .map_err(map_workspace_publish_error)?;
                ensure_workspace_handle_matches(parent, file_name, &file)?;
                *mutation_guard = Some(WorkspaceMutationGuard {
                    file,
                    delete_pending: false,
                });
                created_file = true;
            }
        }
    }

    let guard = mutation_guard
        .as_mut()
        .ok_or_else(|| io::Error::other("The workspace mutation handle is unavailable."))?;
    if guard.delete_pending {
        if expected.is_some() {
            return Err(workspace_mutation_conflict());
        }
        set_file_delete_disposition(&guard.file, false)?;
        guard.delete_pending = false;
    } else if !created_file {
        let current = snapshot_guarded_file(&mut guard.file)?;
        if !workspace_snapshots_equal(Some(&current), expected) {
            return Err(workspace_mutation_conflict());
        }
    } else if expected.is_some() {
        return Err(workspace_mutation_conflict());
    }
    ensure_workspace_handle_matches(parent, file_name, &guard.file)?;

    let previous_content =
        if created_file {
            None
        } else {
            Some(snapshot_guarded_file(&mut guard.file)?.content.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "The workspace file is too large to safely roll back an in-place write.",
            )
        })?)
        };
    if let Err(write_error) = write_workspace_handle(&mut guard.file, content) {
        let rollback_result = match previous_content.as_deref() {
            Some(previous) => write_workspace_handle(&mut guard.file, previous),
            None => set_file_delete_disposition(&guard.file, true).map(|()| {
                guard.delete_pending = true;
            }),
        };
        return match rollback_result {
            Ok(()) => Err(write_error),
            Err(rollback_error) => Err(io::Error::other(format!(
                "{write_error}; restoring the workspace file after the failed write also failed: {rollback_error}"
            ))),
        };
    }
    ensure_workspace_handle_matches(parent, file_name, &guard.file)
}

#[cfg(windows)]
fn remove_workspace_file_windows(
    parent: &crate::filesystem::OpenedDirectory,
    file_name: &std::ffi::OsStr,
    expected: &FileSnapshot,
    mutation_guard: &mut Option<WorkspaceMutationGuard>,
) -> io::Result<()> {
    if mutation_guard.is_none() {
        let mut file =
            open_workspace_mutation_file(parent, file_name).map_err(map_workspace_lock_error)?;
        let current = snapshot_guarded_file(&mut file)?;
        if !workspace_snapshots_equal(Some(&current), Some(expected)) {
            return Err(workspace_mutation_conflict());
        }
        if current.content.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The workspace file is too large to safely roll back a delete.",
            ));
        }
        ensure_workspace_handle_matches(parent, file_name, &file)?;
        *mutation_guard = Some(WorkspaceMutationGuard {
            file,
            delete_pending: false,
        });
    }

    let guard = mutation_guard
        .as_mut()
        .ok_or_else(|| io::Error::other("The workspace mutation handle is unavailable."))?;
    if guard.delete_pending {
        return Ok(());
    }
    let current = snapshot_guarded_file(&mut guard.file)?;
    if !workspace_snapshots_equal(Some(&current), Some(expected)) {
        return Err(workspace_mutation_conflict());
    }
    if current.content.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The workspace file is too large to safely roll back a delete.",
        ));
    }
    ensure_workspace_handle_matches(parent, file_name, &guard.file)?;
    set_file_delete_disposition(&guard.file, true)?;
    guard.delete_pending = true;
    Ok(())
}

#[cfg(windows)]
fn map_workspace_lock_error(error: io::Error) -> io::Error {
    use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;

    if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) {
        workspace_mutation_conflict()
    } else if error.kind() == io::ErrorKind::NotFound {
        workspace_mutation_conflict()
    } else {
        error
    }
}

#[cfg(windows)]
fn map_workspace_publish_error(error: io::Error) -> io::Error {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};

    if matches!(
        error.raw_os_error(),
        Some(code)
            if code == ERROR_ALREADY_EXISTS as i32 || code == ERROR_FILE_EXISTS as i32
    ) {
        workspace_mutation_conflict()
    } else {
        error
    }
}

#[cfg(windows)]
fn write_workspace_handle(file: &mut CapabilityFile, content: &[u8]) -> io::Result<()> {
    use std::io::Seek;

    file.seek(std::io::SeekFrom::Start(0))?;
    file.write_all(content)?;
    file.set_len(content.len() as u64)?;
    file.sync_all()
}

#[cfg(windows)]
fn ensure_workspace_handle_matches(
    parent: &crate::filesystem::OpenedDirectory,
    file_name: &std::ffi::OsStr,
    file: &CapabilityFile,
) -> io::Result<()> {
    match crate::filesystem::opened_file_matches_directory_entry(parent, Path::new(file_name), file)
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(workspace_mutation_conflict()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(workspace_mutation_conflict()),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn workspace_snapshots_equal(
    current: Option<&FileSnapshot>,
    expected: Option<&FileSnapshot>,
) -> bool {
    current.map(|snapshot| snapshot.hash.as_str())
        == expected.map(|snapshot| snapshot.hash.as_str())
}

#[cfg(windows)]
fn snapshot_guarded_file(file: &mut CapabilityFile) -> io::Result<FileSnapshot> {
    snapshot_opened_file(file).map_err(|_| {
        io::Error::other("The workspace file could not be compared through its handle.")
    })
}

#[cfg(windows)]
fn open_workspace_mutation_file(
    parent: &crate::filesystem::OpenedDirectory,
    file_name: &std::ffi::OsStr,
) -> io::Result<CapabilityFile> {
    use cap_std::fs::OpenOptionsExt;
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::{DELETE, FILE_SHARE_READ},
    };

    let mut options = CapabilityOpenOptions::new();
    options.read(true).write(true);
    options.share_mode(FILE_SHARE_READ);
    options.access_mode(GENERIC_READ | GENERIC_WRITE | DELETE);
    crate::filesystem::open_file_from_directory(parent, Path::new(file_name), &options)
}

#[cfg(windows)]
fn create_workspace_mutation_file(
    parent: &crate::filesystem::OpenedDirectory,
    relative_path: &Path,
) -> io::Result<CapabilityFile> {
    use cap_std::fs::OpenOptionsExt;
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::{DELETE, FILE_SHARE_READ},
    };

    let mut options = CapabilityOpenOptions::new();
    options.read(true).write(true).create_new(true);
    options.share_mode(FILE_SHARE_READ);
    options.access_mode(GENERIC_READ | GENERIC_WRITE | DELETE);
    crate::filesystem::open_file_from_directory(parent, relative_path, &options)
}

#[cfg(windows)]
fn set_file_delete_disposition(file: &CapabilityFile, delete: bool) -> io::Result<()> {
    use std::{mem::size_of, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: delete };
    // SAFETY: the file owns a live handle and `disposition` is a correctly sized input structure.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as _,
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
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

#[cfg(all(test, windows))]
mod windows_mutation_tests {
    use super::{
        CapabilityOpenOptions, WorkspaceMutationGuard, capture_file_from_directory,
        open_workspace_mutation_file, remove_workspace_file_if_unchanged, snapshot_guarded_file,
        write_workspace_file_if_unchanged,
    };
    use std::{
        fs::{self, OpenOptions},
        path::{Path, PathBuf},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-workspace-mutation-{}-{}",
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
    fn checked_in_place_write_holds_the_path_through_tracking_persistence() {
        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        let moved_path = temporary.0.join("moved-note.txt");
        fs::write(&path, b"before").expect("write initial file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let expected = capture_file_from_directory(&root, Path::new("note.txt"))
            .expect("capture initial workspace file")
            .expect("initial file exists");

        let mut file = open_workspace_mutation_file(&root, Path::new("note.txt").as_os_str())
            .expect("lock the compared workspace file");
        let locked_snapshot = snapshot_guarded_file(&mut file).expect("hash locked file");
        assert_eq!(locked_snapshot.hash, expected.hash);
        assert!(
            OpenOptions::new().write(true).open(&path).is_err(),
            "a writer must not open after the compare handle is acquired"
        );
        assert!(
            fs::rename(&path, &moved_path).is_err(),
            "the checked path must not be renamed before publication"
        );

        let mut mutation_guard = Some(WorkspaceMutationGuard {
            file,
            delete_pending: false,
        });
        write_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            Some(&expected),
            b"after",
            &mut mutation_guard,
        )
        .expect("write the checked file through its retained handle");
        assert_eq!(fs::read(&path).expect("read updated file"), b"after");
        assert!(
            OpenOptions::new().write(true).open(&path).is_err(),
            "the new file must stay protected until tracking persistence ends"
        );
        assert!(
            fs::rename(&path, &moved_path).is_err(),
            "the new path must stay protected until tracking persistence ends"
        );

        drop(mutation_guard);
        fs::rename(&path, &moved_path).expect("replacement path unlocks after the guard drops");
    }

    #[test]
    fn checked_delete_reserves_the_path_until_the_guard_is_released() {
        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        fs::write(&path, b"before").expect("write initial file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let expected = capture_file_from_directory(&root, Path::new("note.txt"))
            .expect("capture initial workspace file")
            .expect("initial file exists");
        let file = open_workspace_mutation_file(&root, Path::new("note.txt").as_os_str())
            .expect("open the target with delete access");
        drop(file);
        let mut mutation_guard = None;

        remove_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            &expected,
            &mut mutation_guard,
        )
        .expect("mark the checked handle for deletion");
        assert!(
            mutation_guard
                .as_ref()
                .is_some_and(WorkspaceMutationGuard::is_delete_pending),
            "the checked delete must remain pending while its handle is retained"
        );
        assert!(
            OpenOptions::new().write(true).open(&path).is_err(),
            "a new writer must not open the file while its delete is pending"
        );
        assert!(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .is_err(),
            "the pending delete must reserve the original name"
        );

        drop(mutation_guard);
        assert!(
            !path.exists(),
            "the file is removed when the guard is released"
        );
    }

    #[test]
    fn failed_delete_transaction_can_restore_through_the_retained_handle() {
        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        fs::write(&path, b"before").expect("write initial file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let expected = capture_file_from_directory(&root, Path::new("note.txt"))
            .expect("capture initial workspace file")
            .expect("initial file exists");
        let mut mutation_guard = None;

        remove_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            &expected,
            &mut mutation_guard,
        )
        .expect("mark the checked handle for deletion");
        write_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            None,
            b"restored",
            &mut mutation_guard,
        )
        .expect("restore the file before releasing its pending delete handle");
        assert_eq!(fs::read(&path).expect("read restored file"), b"restored");

        drop(mutation_guard);
        assert_eq!(
            fs::read(&path).expect("read after guard release"),
            b"restored"
        );
    }

    #[test]
    fn checked_guard_blocks_posix_replace_until_tracking_finishes() {
        use cap_std::fs::OpenOptionsExt;
        use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
        use windows_sys::Wdk::Storage::FileSystem::{
            FILE_RENAME_POSIX_SEMANTICS, FILE_RENAME_REPLACE_IF_EXISTS,
        };
        use windows_sys::Win32::Storage::FileSystem::{
            FileRenameInfoEx, SetFileInformationByHandle,
        };

        #[repr(C)]
        struct RenameInformationExHeader {
            flags: u32,
            root_directory: windows_sys::Win32::Foundation::HANDLE,
            file_name_length: u32,
        }

        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        let replacement_path = temporary.0.join("replacement.txt");
        fs::write(&path, b"before").expect("write initial file");
        fs::write(&replacement_path, b"external").expect("write replacement file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let target_guard = open_workspace_mutation_file(&root, Path::new("note.txt").as_os_str())
            .expect("lock the target file");
        let mut options = CapabilityOpenOptions::new();
        options.read(true).write(true);
        options.access_mode(
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE
                | windows_sys::Win32::Storage::FileSystem::DELETE,
        );
        let replacement = crate::filesystem::open_file_from_directory(
            &root,
            Path::new("replacement.txt"),
            &options,
        )
        .expect("open the replacement file with rename sharing");
        let wide_name: Vec<u16> = path.as_os_str().encode_wide().collect();
        let file_name_length = (wide_name.len() * std::mem::size_of::<u16>()) as u32;
        let file_name_offset = std::mem::offset_of!(RenameInformationExHeader, file_name_length)
            + std::mem::size_of::<u32>();
        let buffer_length = (file_name_offset + wide_name.len() * 2).max(24);
        let mut buffer = vec![0_u64; buffer_length.div_ceil(8)];
        let header = RenameInformationExHeader {
            flags: FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS,
            root_directory: std::ptr::null_mut(),
            file_name_length,
        };
        let buffer_pointer = buffer.as_mut_ptr().cast::<u8>();
        // SAFETY: the u64-backed buffer is aligned and large enough for the header and UTF-16 name.
        unsafe {
            std::ptr::write(buffer_pointer.cast::<RenameInformationExHeader>(), header);
            std::ptr::copy_nonoverlapping(
                wide_name.as_ptr(),
                buffer_pointer.add(file_name_offset).cast::<u16>(),
                wide_name.len(),
            );
        }
        // SAFETY: `replacement` owns a live handle and the input buffer remains valid for the call.
        let result = unsafe {
            SetFileInformationByHandle(
                replacement.as_raw_handle() as _,
                FileRenameInfoEx,
                buffer_pointer.cast(),
                buffer_length as u32,
            )
        };
        assert_eq!(result, 0, "the guarded POSIX replacement must be denied");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION as i32)
        );
        assert_eq!(fs::read(&path).expect("read unchanged path"), b"before");
        drop(target_guard);
        // SAFETY: the source handle and input buffer are still live and valid.
        let result = unsafe {
            SetFileInformationByHandle(
                replacement.as_raw_handle() as _,
                FileRenameInfoEx,
                buffer_pointer.cast(),
                buffer_length as u32,
            )
        };
        assert_ne!(
            result, 0,
            "the POSIX replacement should be allowed after the guard closes"
        );
        assert_eq!(fs::read(&path).expect("read replaced path"), b"external");
    }

    #[test]
    fn pending_delete_guard_blocks_posix_replace_until_tracking_finishes() {
        use cap_std::fs::OpenOptionsExt;
        use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
        use windows_sys::Wdk::Storage::FileSystem::{
            FILE_RENAME_POSIX_SEMANTICS, FILE_RENAME_REPLACE_IF_EXISTS,
        };
        use windows_sys::Win32::Storage::FileSystem::{
            FileRenameInfoEx, SetFileInformationByHandle,
        };

        #[repr(C)]
        struct RenameInformationExHeader {
            flags: u32,
            root_directory: windows_sys::Win32::Foundation::HANDLE,
            file_name_length: u32,
        }

        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        let replacement_path = temporary.0.join("replacement.txt");
        fs::write(&path, b"before").expect("write initial file");
        fs::write(&replacement_path, b"external").expect("write replacement file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let expected = capture_file_from_directory(&root, Path::new("note.txt"))
            .expect("capture initial workspace file")
            .expect("initial file exists");
        let mut options = CapabilityOpenOptions::new();
        options.read(true).write(true);
        options.access_mode(
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE
                | windows_sys::Win32::Storage::FileSystem::DELETE,
        );
        let replacement = crate::filesystem::open_file_from_directory(
            &root,
            Path::new("replacement.txt"),
            &options,
        )
        .expect("open the replacement file with rename sharing");
        let mut mutation_guard = None;
        remove_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            &expected,
            &mut mutation_guard,
        )
        .expect("mark the checked file for deletion");

        let wide_name: Vec<u16> = path.as_os_str().encode_wide().collect();
        let file_name_length = (wide_name.len() * std::mem::size_of::<u16>()) as u32;
        let file_name_offset = std::mem::offset_of!(RenameInformationExHeader, file_name_length)
            + std::mem::size_of::<u32>();
        let buffer_length = (file_name_offset + wide_name.len() * 2).max(24);
        let mut buffer = vec![0_u64; buffer_length.div_ceil(8)];
        let header = RenameInformationExHeader {
            flags: FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS,
            root_directory: std::ptr::null_mut(),
            file_name_length,
        };
        let buffer_pointer = buffer.as_mut_ptr().cast::<u8>();
        // SAFETY: the u64-backed buffer is aligned and large enough for the header and UTF-16 name.
        unsafe {
            std::ptr::write(buffer_pointer.cast::<RenameInformationExHeader>(), header);
            std::ptr::copy_nonoverlapping(
                wide_name.as_ptr(),
                buffer_pointer.add(file_name_offset).cast::<u16>(),
                wide_name.len(),
            );
        }
        // SAFETY: `replacement` owns a live handle and the input buffer remains valid for the call.
        let result = unsafe {
            SetFileInformationByHandle(
                replacement.as_raw_handle() as _,
                FileRenameInfoEx,
                buffer_pointer.cast(),
                buffer_length as u32,
            )
        };
        assert_eq!(result, 0, "a pending delete must reserve the guarded path");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION as i32)
        );
        drop(mutation_guard);
        // SAFETY: the source handle and input buffer are still live and valid.
        let result = unsafe {
            SetFileInformationByHandle(
                replacement.as_raw_handle() as _,
                FileRenameInfoEx,
                buffer_pointer.cast(),
                buffer_length as u32,
            )
        };
        assert_ne!(
            result, 0,
            "the replacement should proceed after the pending-delete guard closes"
        );
        assert_eq!(fs::read(&path).expect("read replacement path"), b"external");
    }

    #[test]
    fn an_already_open_writer_makes_the_checked_mutation_fail_before_changes() {
        let temporary = TestDirectory::new();
        let path = temporary.0.join("note.txt");
        fs::write(&path, b"before").expect("write initial file");
        let root = crate::filesystem::open_absolute_directory_no_follow(&temporary.0)
            .expect("open workspace capability");
        let expected = capture_file_from_directory(&root, Path::new("note.txt"))
            .expect("capture initial workspace file")
            .expect("initial file exists");
        let writer = OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open competing writer before the mutation");
        let mut mutation_guard = None;

        let error = write_workspace_file_if_unchanged(
            &root,
            Path::new("note.txt"),
            Some(&expected),
            b"replacement",
            &mut mutation_guard,
        )
        .expect_err("refuse a target with a writer already open");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(mutation_guard.is_none());
        assert_eq!(fs::read(&path).expect("read unchanged file"), b"before");
        drop(writer);
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
