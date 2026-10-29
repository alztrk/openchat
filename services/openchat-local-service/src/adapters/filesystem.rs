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

/// Opens a regular file for mutation while denying concurrent writes and replacement on Windows.
///
/// The returned handle still permits other readers. On Windows, callers must retain it through
/// their complete compare, mutation, and tracking sequence; the restrictive share mode is scoped
/// to the handle lifetime. Other platforms retain their existing capability and no-follow checks.
pub(crate) fn open_mutation_file_from_directory(
    directory: &(impl DirectoryCapability + ?Sized),
    relative_path: &Path,
    options: &OpenOptions,
) -> io::Result<(OpenedFile, Metadata)> {
    let mut options = options.clone();
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

        options.share_mode(FILE_SHARE_READ);
    }
    open_regular_file_from_directory(directory, relative_path, &options)
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
                let child = create_or_open_directory_child(&current, name)?;
                current.push_child(child);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

fn create_or_open_directory_child(directory: &Dir, name: &OsStr) -> io::Result<Dir> {
    #[cfg(windows)]
    {
        match create_directory_child_windows(directory, name) {
            Ok(child) => Ok(child),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                open_directory_child(directory, name)
            }
            Err(error) => Err(error),
        }
    }
    #[cfg(not(windows))]
    {
        match directory.create_dir(name) {
            Ok(()) => open_directory_child(directory, name),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                open_directory_child(directory, name)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(windows)]
fn create_directory_child_windows(directory: &Dir, name: &OsStr) -> io::Result<Dir> {
    use std::{
        mem::size_of,
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle},
        },
        ptr::{null, null_mut},
    };
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                FILE_CREATE, FILE_DIRECTORY_FILE, FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
            },
        },
        Win32::{
            Foundation::{
                HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError,
                STATUS_OBJECT_NAME_COLLISION, UNICODE_STRING,
            },
            Storage::FileSystem::{
                FILE_ATTRIBUTE_DIRECTORY, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
            },
            System::IO::IO_STATUS_BLOCK,
        },
    };

    let mut name: Vec<u16> = name.encode_wide().collect();
    let name_length = name
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u16::try_from(length).ok())
        .filter(|length| *length > 0)
        .filter(|_| !name.contains(&0))
        .ok_or_else(invalid_path_error)?;
    let mut object_name = UNICODE_STRING {
        Length: name_length,
        MaximumLength: name_length,
        Buffer: name.as_mut_ptr(),
    };
    let parent = directory.try_clone()?.into_std_file();
    let object_attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle() as HANDLE,
        ObjectName: &mut object_name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: null(),
        SecurityQualityOfService: null(),
    };
    let mut handle: HANDLE = null_mut();
    let mut io_status = IO_STATUS_BLOCK::default();
    // SAFETY: The parent handle and all pointers remain valid for the synchronous call. The
    // one-component name was validated, FILE_CREATE prevents opening an existing entry, and
    // OBJ_DONT_REPARSE rejects a reparse point that races with the initial lookup.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &object_attributes,
            &mut io_status,
            null(),
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            null(),
            0,
        )
    };
    if status < 0 {
        if status == STATUS_OBJECT_NAME_COLLISION {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "The workspace directory already exists.",
            ));
        }
        // SAFETY: NtCreateFile returns a documented NTSTATUS value that RtlNtStatusToDosError maps.
        let error_code = unsafe { RtlNtStatusToDosError(status) };
        return Err(io::Error::from_raw_os_error(error_code as i32));
    }
    if handle.is_null() {
        return Err(io::Error::other(
            "Windows returned no handle for a created workspace directory.",
        ));
    }

    // SAFETY: NtCreateFile succeeded and transferred ownership of this handle to the caller.
    let child = unsafe { std::fs::File::from_raw_handle(handle) };
    let metadata = child.metadata()?;
    if metadata_is_reparse_point_std(&metadata) || !metadata.is_dir() {
        return Err(invalid_file_error());
    }
    Ok(Dir::from_std_file(child))
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

/// Checks that a workspace-relative entry still names the opened file object.
pub(crate) fn opened_file_matches_directory_entry<D>(
    directory: &D,
    relative_path: &Path,
    expected: &OpenedFile,
) -> io::Result<bool>
where
    D: DirectoryCapability + ?Sized,
{
    let mut options = OpenOptions::new();
    options.read(true);
    let (current, _) = open_regular_file_from_directory(directory, relative_path, &options)?;
    same_file_object(&expected.file, &current.file)
}

fn same_file_object(left: &File, right: &File) -> io::Result<bool> {
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

        let mut left_information = BY_HANDLE_FILE_INFORMATION::default();
        let mut right_information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: Both files own live handles and the information outputs are writable.
        if unsafe {
            GetFileInformationByHandle(left.as_raw_handle() as HANDLE, &mut left_information)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Both files own live handles and the information outputs are writable.
        if unsafe {
            GetFileInformationByHandle(right.as_raw_handle() as HANDLE, &mut right_information)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(
            left_information.dwVolumeSerialNumber == right_information.dwVolumeSerialNumber
                && left_information.nFileIndexHigh == right_information.nFileIndexHigh
                && left_information.nFileIndexLow == right_information.nFileIndexLow,
        )
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (left, right);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "workspace file identity is not supported on this platform",
        ))
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-filesystem-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&path).expect("create isolated filesystem test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("remove isolated filesystem test directory");
        }
    }

    #[test]
    fn created_directory_chain_is_opened_before_its_path_can_be_replaced() {
        let temporary = TestDirectory::new();
        let root_path = temporary.0.join("workspace");
        std::fs::create_dir(&root_path).expect("create workspace root");
        let root = open_absolute_directory_no_follow(&root_path).expect("open workspace root");
        let created = create_directory_chain_from_directory(&root, Path::new("nested/leaf"))
            .expect("create nested directories");
        let nested = root_path.join("nested");
        let moved = root_path.join("moved");

        assert!(std::fs::rename(&nested, &moved).is_err());
        drop(created);
        std::fs::rename(&nested, &moved).expect("rename after releasing created directory handles");
    }

    #[test]
    fn create_collision_opens_only_a_valid_existing_directory() {
        let temporary = TestDirectory::new();
        let root_path = temporary.0.join("workspace");
        std::fs::create_dir(&root_path).expect("create workspace root");
        std::fs::create_dir(root_path.join("existing")).expect("create existing directory");
        std::fs::write(root_path.join("file"), b"content").expect("create non-directory entry");
        let root = open_absolute_directory_no_follow(&root_path).expect("open workspace root");

        let existing = create_or_open_directory_child(&root, OsStr::new("existing"))
            .expect("open the existing directory after create collision");
        assert!(
            existing
                .try_clone()
                .expect("clone existing directory handle")
                .into_std_file()
                .metadata()
                .expect("read existing metadata")
                .is_dir()
        );
        assert!(create_or_open_directory_child(&root, OsStr::new("file")).is_err());
    }

    #[test]
    fn create_only_rejects_a_reparse_point_added_after_the_initial_lookup() {
        use std::os::windows::fs::symlink_dir;

        let temporary = TestDirectory::new();
        let root_path = temporary.0.join("workspace");
        let external_path = temporary.0.join("external");
        std::fs::create_dir(&root_path).expect("create workspace root");
        std::fs::create_dir(&external_path).expect("create external directory");
        let root = open_absolute_directory_no_follow(&root_path).expect("open workspace root");
        let external_target = external_path.join("created-through-link");
        symlink_dir(&external_target, root_path.join("redirect"))
            .expect("create directory symlink after opening workspace root");

        assert!(create_directory_child_windows(&root, OsStr::new("redirect")).is_err());
        assert!(!external_target.exists());
    }
}
