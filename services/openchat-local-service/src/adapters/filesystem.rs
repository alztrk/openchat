//! Capability-based file access shared by project and tool operations.
//!
//! Project contents are untrusted input. Reads stay beneath an opened directory capability and
//! reject hard links, which can otherwise expose a file also named outside the selected root.

#[cfg(windows)]
use std::path::PathBuf;
use std::{
    ffi::OsStr,
    io::{self, Read},
    ops::{Deref, DerefMut},
    path::{Component, Path},
};

use cap_primitives::fs::{FollowSymlinks, open_dir_nofollow};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions},
};

/// An opened directory and every ancestor capability used to reach it.
///
/// On Windows, the directory handles prevent their directory objects from being moved while an
/// operation is using the path. Keeping the full chain alive also prevents an ancestor from being
/// moved out from under a still-open descendant.
pub(crate) struct OpenedDirectory {
    ancestors: Vec<Dir>,
    leaf: Dir,
}

impl OpenedDirectory {
    fn from_dir(leaf: Dir) -> Self {
        Self {
            ancestors: Vec::new(),
            leaf,
        }
    }

    fn try_clone_chain(&self) -> io::Result<Self> {
        let mut ancestors = Vec::with_capacity(self.ancestors.len());
        for ancestor in &self.ancestors {
            ancestors.push(ancestor.try_clone()?);
        }
        Ok(Self {
            ancestors,
            leaf: self.leaf.try_clone()?,
        })
    }

    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        self.try_clone_chain()
    }

    fn push_child(&mut self, child: Dir) {
        self.ancestors
            .push(std::mem::replace(&mut self.leaf, child));
    }
}

impl Deref for OpenedDirectory {
    type Target = Dir;

    fn deref(&self) -> &Self::Target {
        &self.leaf
    }
}

/// Allows path traversal to preserve an existing directory capability chain.
pub(crate) trait DirectoryCapability {
    fn try_clone_chain(&self) -> io::Result<OpenedDirectory>;
}

impl DirectoryCapability for Dir {
    fn try_clone_chain(&self) -> io::Result<OpenedDirectory> {
        Ok(OpenedDirectory::from_dir(self.try_clone()?))
    }
}

impl DirectoryCapability for OpenedDirectory {
    fn try_clone_chain(&self) -> io::Result<OpenedDirectory> {
        OpenedDirectory::try_clone_chain(self)
    }
}

/// A regular file handle that keeps all parent directory capabilities alive until it is dropped.
pub(crate) struct OpenedFile {
    file: File,
    _parent: OpenedDirectory,
}

impl Deref for OpenedFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl DerefMut for OpenedFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

impl Read for OpenedFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl io::Write for OpenedFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut self.file, buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.file)
    }
}

/// Opens a regular file through an already-open directory capability.
pub(crate) fn open_regular_file_from_directory(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<(OpenedFile, Metadata)> {
    let file = open_file_from_directory(directory, relative_path, options)?;
    let metadata = file.metadata()?;
    if metadata_is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(invalid_file_error());
    }
    Ok((file, metadata))
}

/// Opens a candidate file and rejects hard links using the opened handle.
///
/// Callers that enumerate files first must retain their enumeration checks for file type and
/// symlink policy; the capability still confines any path race to the selected root.
pub(crate) fn open_file_from_directory(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<OpenedFile> {
    let (parent, name) = open_parent_directory_from_directory(directory, relative_path)?;
    let mut options = options.clone();
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options)?;
    let metadata = file.metadata()?;
    if metadata_is_reparse_point(&metadata)
        || !metadata.is_file()
        || file_has_multiple_links(&file)?
    {
        return Err(invalid_file_error());
    }
    Ok(OpenedFile {
        file,
        _parent: parent,
    })
}

/// Opens a directory path one component at a time without following symlinks or reparse points.
pub(crate) fn open_directory_from_directory(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &Path,
) -> io::Result<OpenedDirectory> {
    validate_relative_path(relative_path)?;
    let mut current = directory.try_clone_chain()?;
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(invalid_path_error());
        };
        let child = open_directory_child(&current, name)?;
        current.push_child(child);
    }
    Ok(current)
}

/// Creates missing directory components relative to an opened directory capability.
pub(crate) fn create_directory_chain_from_directory(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &Path,
) -> io::Result<OpenedDirectory> {
    validate_relative_path(relative_path)?;
    let mut current = directory.try_clone_chain()?;
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(invalid_path_error());
        };
        match open_directory_child(&current, name) {
            Ok(child) => current.push_child(child),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match current.create_dir(name) {
                    Ok(()) => {}
                    Err(create_error) if create_error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(create_error) => return Err(create_error),
                }
                let child = open_directory_child(&current, name)?;
                current.push_child(child);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

/// Opens an absolute directory path one component at a time without following links.
pub(crate) fn open_absolute_directory_no_follow(path: &Path) -> io::Result<OpenedDirectory> {
    let mut components = path.components();
    #[cfg(windows)]
    let mut current = {
        let Component::Prefix(prefix) = components.next().ok_or_else(invalid_path_error)? else {
            return Err(invalid_path_error());
        };
        if !matches!(components.next(), Some(Component::RootDir)) {
            return Err(invalid_path_error());
        }
        let mut root_path = prefix.as_os_str().to_os_string();
        root_path.push("\\");
        OpenedDirectory::from_dir(Dir::open_ambient_dir(
            PathBuf::from(root_path),
            ambient_authority(),
        )?)
    };
    #[cfg(unix)]
    let mut current = {
        if !matches!(components.next(), Some(Component::RootDir)) {
            return Err(invalid_path_error());
        }
        OpenedDirectory::from_dir(Dir::open_ambient_dir(Path::new("/"), ambient_authority())?)
    };
    #[cfg(not(any(unix, windows)))]
    let mut current = {
        return Dir::open_ambient_dir(path, ambient_authority()).map(OpenedDirectory::from_dir);
    };

    for component in components {
        let Component::Normal(name) = component else {
            return Err(invalid_path_error());
        };
        let child = open_directory_child(&current, name)?;
        current.push_child(child);
    }
    Ok(current)
}

/// Returns the parent capability and final name after no-follow traversal of every parent.
fn open_parent_directory_from_directory<'a>(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &'a Path,
) -> io::Result<(OpenedDirectory, &'a OsStr)> {
    validate_relative_path(relative_path)?;
    let name = relative_path.file_name().ok_or_else(invalid_path_error)?;
    let parent_path = relative_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = match parent_path {
        Some(path) => open_directory_from_directory(directory, path)?,
        None => directory.try_clone_chain()?,
    };
    Ok((parent, name))
}

fn open_directory_child(directory: &Dir, name: &OsStr) -> io::Result<Dir> {
    let parent = directory.try_clone()?.into_std_file();
    let child = open_dir_nofollow(&parent, Path::new(name))?;
    let metadata = child.metadata()?;
    if metadata_is_reparse_point_std(&metadata) || !metadata.is_dir() {
        return Err(invalid_file_error());
    }
    Ok(Dir::from_std_file(child))
}

pub(crate) fn same_directory_object(left: &Dir, right: &Dir) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;

        let left = left.metadata()?;
        let right = right.metadata()?;
        Ok(left.dev() == right.dev() && left.ino() == right.ino())
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };

        let left = left.try_clone()?.into_std_file();
        let right = right.try_clone()?.into_std_file();
        let mut left_information = BY_HANDLE_FILE_INFORMATION::default();
        let mut right_information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: Both cloned files own live Windows handles and the information outputs are writable.
        if unsafe {
            GetFileInformationByHandle(left.as_raw_handle() as HANDLE, &mut left_information)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Both cloned files own live Windows handles and the information outputs are writable.
        if unsafe {
            GetFileInformationByHandle(right.as_raw_handle() as HANDLE, &mut right_information)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(
            left_information.dwVolumeSerialNumber == right_information.dwVolumeSerialNumber
                && left_information.nFileIndexHigh == right_information.nFileIndexHigh
                && left_information.nFileIndexLow == right_information.nFileIndexLow
                && left_information.ftCreationTime.dwHighDateTime
                    == right_information.ftCreationTime.dwHighDateTime
                && left_information.ftCreationTime.dwLowDateTime
                    == right_information.ftCreationTime.dwLowDateTime,
        )
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (left, right);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "workspace directory identity is not supported on this platform",
        ))
    }
}

/// Identifies symlinks and every Windows reparse-point type from opened metadata.
pub(crate) fn metadata_is_reparse_point(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn metadata_is_reparse_point_std(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Opens a regular file below a root path without resolving the file again through an ambient path.
pub(crate) fn open_regular_file_under_root(
    root: &Path,
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<(OpenedFile, Metadata)> {
    let directory = open_absolute_directory_no_follow(root)?;
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
    directory: &(impl DirectoryCapability + ?Sized),
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
fn read_bounded_file(
    file: OpenedFile,
    metadata: Metadata,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
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

fn invalid_path_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "The file path must be relative to its directory capability.",
    )
}

/// Returns the shared error for entries outside the supported regular-file boundary.
fn invalid_file_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "The file must be a regular, independently linked file.",
    )
}
