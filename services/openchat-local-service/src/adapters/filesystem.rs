//! Capability-based file access shared by project and tool operations.
//!
//! Project contents are untrusted input. Reads stay beneath an opened directory capability and
//! reject hard links, which can otherwise expose a file also named outside the selected root.

use std::{
    io::{self, Read},
    path::{Component, Path},
};

use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions},
};

/// Opens a regular file through an already-open directory capability.
pub(crate) fn open_regular_file_from_directory(
    directory: &Dir,
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<(File, Metadata)> {
    validate_relative_path(relative_path)?;
    let path_metadata = directory.symlink_metadata(relative_path)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(invalid_file_error());
    }

    let file = open_file_from_directory(directory, relative_path, options)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid_file_error());
    }
    Ok((file, metadata))
}

/// Opens a candidate file and rejects hard links using the opened handle.
///
/// Callers that enumerate files first must retain their enumeration checks for file type and
/// symlink policy; the capability still confines any path race to the selected root.
pub(crate) fn open_file_from_directory(
    directory: &Dir,
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<File> {
    validate_relative_path(relative_path)?;
    let file = directory.open_with(relative_path, options)?;
    if file_has_multiple_links(&file)? {
        return Err(invalid_file_error());
    }
    Ok(file)
}

/// Opens a regular file below a root path without resolving the file again through an ambient path.
pub(crate) fn open_regular_file_under_root(
    root: &Path,
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<(File, Metadata)> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    open_regular_file_from_directory(&directory, relative_path, options)
}

/// Reads at most `max_bytes + 1` from a regular file below the selected root.
pub(crate) fn read_bounded_file_under_root(
    root: &Path,
    relative_path: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    let (file, metadata) = open_regular_file_under_root(root, relative_path, &options)?;
    read_bounded_file(file, metadata, max_bytes)
}

/// Reads at most `max_bytes + 1` from a regular file inside an opened directory capability.
pub(crate) fn read_bounded_file_from_directory(
    directory: &Dir,
    relative_path: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    let (file, metadata) = open_regular_file_from_directory(directory, relative_path, &options)?;
    read_bounded_file(file, metadata, max_bytes)
}

/// Checks the opened handle so path replacement cannot hide an existing hard link.
pub(crate) fn file_has_multiple_links(file: &File) -> io::Result<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };

        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `file` owns a live Windows file handle and `information` is a writable output structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(information.nNumberOfLinks > 1)
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(file.metadata()?.nlink() > 1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Ok(true)
    }
}

/// Reads a size-limited prefix from an already-open regular file.
///
/// The extra byte distinguishes a file that exceeded the limit from one that fit exactly.
fn read_bounded_file(file: File, metadata: Metadata, max_bytes: usize) -> io::Result<Vec<u8>> {
    let max_bytes = u64::try_from(max_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The file size limit exceeds the supported range.",
        )
    })?;
    let read_limit = max_bytes.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The file size limit exceeds the supported range.",
        )
    })?;
    let capacity = metadata.len().min(max_bytes) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(read_limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Rejects empty, absolute, and traversing paths before capability-relative filesystem access.
pub(crate) fn validate_relative_path(relative_path: &Path) -> io::Result<()> {
    if relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The file path must be relative to its directory capability.",
        ));
    }
    Ok(())
}

/// Returns the shared error for entries outside the supported regular-file boundary.
fn invalid_file_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "The file must be a regular, independently linked file.",
    )
}
