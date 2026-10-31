//! Path resolution, directory handle verification, and filesystem errors.

use super::*;

pub(super) fn resolve_directory_for_listing(
    root: impl AsRef<Path>,
    path: &str,
) -> Result<
    (
        PathBuf,
        PathBuf,
        crate::filesystem::OpenedDirectory,
        crate::filesystem::OpenedDirectory,
        bool,
    ),
    ServiceError,
> {
    let root_path = root.as_ref();
    let initial_root = crate::filesystem::open_absolute_directory_no_follow(root_path)
        .map_err(|error| filesystem_error(&error, "opening the active tool root"))?;
    let root = root_path
        .canonicalize()
        .map_err(|error| filesystem_error(&error, "resolving the active tool root"))?;
    let root_handle = crate::filesystem::open_absolute_directory_no_follow(&root)
        .map_err(|error| filesystem_error(&error, "reopening the active tool root"))?;
    if !crate::filesystem::same_directory_object(&initial_root, &root_handle)
        .map_err(|error| filesystem_error(&error, "validating the active tool root"))?
    {
        return Err(invalid("The active tool root changed while it was opened."));
    }
    let requested = safe_relative_path(path)?;
    let directory = root
        .join(&requested)
        .canonicalize()
        .map_err(|error| filesystem_error(&error, "resolving the requested directory"))?;
    if !directory.starts_with(&root) {
        return Err(invalid(
            "The requested path is outside the active tool root or is not a directory.",
        ));
    }
    let directory_handle = if requested.as_os_str().is_empty() {
        root_handle
            .try_clone()
            .map_err(|error| filesystem_error(&error, "opening the requested directory"))?
    } else {
        crate::filesystem::open_directory_from_directory(&root_handle, &requested)
            .map_err(|error| filesystem_error(&error, "opening the requested directory"))?
    };
    let canonical_directory_handle =
        crate::filesystem::open_absolute_directory_no_follow(&directory)
            .map_err(|error| filesystem_error(&error, "reopening the requested directory"))?;
    if !crate::filesystem::same_directory_object(&directory_handle, &canonical_directory_handle)
        .map_err(|error| filesystem_error(&error, "validating the requested directory"))?
    {
        return Err(invalid(
            "The requested directory changed while it was opened.",
        ));
    }
    let (explicit_hidden, filter_directory_handle) =
        directory_filter_scope(&root_handle, &requested)?;
    if !crate::filesystem::same_directory_object(&directory_handle, &filter_directory_handle)
        .map_err(|error| filesystem_error(&error, "validating the requested directory"))?
    {
        return Err(invalid(
            "The requested directory changed while it was opened.",
        ));
    }
    Ok((
        root,
        directory,
        root_handle,
        directory_handle,
        explicit_hidden,
    ))
}

pub(super) fn directory_filter_scope(
    root: &crate::filesystem::OpenedDirectory,
    relative_path: &Path,
) -> Result<(bool, crate::filesystem::OpenedDirectory), ServiceError> {
    let mut current = root
        .try_clone()
        .map_err(|error| filesystem_error(&error, "opening the requested directory"))?;
    let mut explicit = false;
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(invalid(
                "Tool paths must be relative to the active tool root.",
            ));
        };
        current = crate::filesystem::open_directory_from_directory(&current, Path::new(name))
            .map_err(|error| filesystem_error(&error, "opening the requested directory"))?;
        let name = name.to_string_lossy();
        explicit |= is_default_ignored_directory(&name)
            || opened_directory_is_hidden(&name, &current)
                .map_err(|error| filesystem_error(&error, "reading directory metadata"))?;
    }
    Ok((explicit, current))
}

pub(super) fn opened_directory_is_hidden(name: &str, directory: &Dir) -> io::Result<bool> {
    if name.starts_with('.') {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;

        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        Ok(directory.dir_metadata()?.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = directory;
        Ok(false)
    }
}

pub(super) fn resolve_directory(
    root: impl AsRef<Path>,
    path: &str,
) -> Result<(PathBuf, PathBuf, bool), ServiceError> {
    let root = canonical_root(root)?;
    let requested = safe_relative_path(path)?;
    let directory = root
        .join(&requested)
        .canonicalize()
        .map_err(|error| filesystem_error(&error, "resolving the requested directory"))?;
    if !directory.starts_with(&root) || !directory.is_dir() {
        return Err(invalid(
            "The requested path is outside the active tool root or is not a directory.",
        ));
    }
    let mut relative = PathBuf::new();
    let explicit_hidden = requested.components().any(|component| {
        relative.push(component.as_os_str());
        let Some(name) = component.as_os_str().to_str() else {
            return false;
        };
        is_default_ignored_directory(name)
            || fs::metadata(root.join(&relative)).is_ok_and(|metadata| is_hidden(name, &metadata))
    });
    Ok((root, directory, explicit_hidden))
}

pub(super) fn verify_directory_handles(
    root_path: &Path,
    directory_path: &Path,
    root: &crate::filesystem::OpenedDirectory,
    directory: &crate::filesystem::OpenedDirectory,
) -> Result<(), ServiceError> {
    let current_root = crate::filesystem::open_absolute_directory_no_follow(root_path)
        .map_err(|error| filesystem_error(&error, "reopening the active tool root"))?;
    if !crate::filesystem::same_directory_object(root, &current_root)
        .map_err(|error| filesystem_error(&error, "validating the active tool root"))?
    {
        return Err(invalid("The active tool root changed while it was listed."));
    }
    let current_directory = crate::filesystem::open_absolute_directory_no_follow(directory_path)
        .map_err(|error| filesystem_error(&error, "reopening the requested directory"))?;
    if !crate::filesystem::same_directory_object(directory, &current_directory)
        .map_err(|error| filesystem_error(&error, "validating the requested directory"))?
    {
        return Err(invalid(
            "The requested directory changed while it was listed.",
        ));
    }
    Ok(())
}

/// Opens a regular file relative to a directory capability for the active tool root.
pub(super) fn open_file_under_root(
    root: impl AsRef<Path>,
    path: &str,
    options: &OpenOptions,
) -> Result<
    (
        PathBuf,
        crate::filesystem::OpenedFile,
        cap_std::fs::Metadata,
    ),
    ServiceError,
> {
    let relative = safe_relative_path(path)?;
    let directory = crate::filesystem::open_absolute_directory_no_follow(root.as_ref())
        .map_err(|error| filesystem_error(&error, "opening the active tool root"))?;
    let (file, metadata) =
        crate::filesystem::open_regular_file_from_directory(&directory, &relative, options)
            .map_err(|error| filesystem_error(&error, "opening the requested file"))?;
    Ok((relative, file, metadata))
}

pub(super) fn read_bounded_file_under_root(
    root: &Path,
    relative_path: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    crate::filesystem::read_bounded_file_under_root(root, relative_path, max_bytes)
}

pub(super) fn path_exists_under_root(root: &Path, relative_path: &Path) -> io::Result<bool> {
    let directory = crate::filesystem::open_absolute_directory_no_follow(root)?;
    crate::filesystem::validate_relative_path(relative_path)?;
    match directory.symlink_metadata(relative_path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn read_bounded_file_from_directory(
    directory: &(impl crate::filesystem::DirectoryCapability + ?Sized),
    relative_path: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    crate::filesystem::read_bounded_file_from_directory(directory, relative_path, max_bytes)
}

/// Checks the opened file handle for multiple filesystem names.
///
/// Returns an error if the platform cannot verify the link count, which keeps writes fail-closed.
pub(super) fn has_multiple_links(file: &cap_std::fs::File) -> Result<bool, ServiceError> {
    crate::filesystem::file_has_multiple_links(file)
        .map_err(|error| filesystem_error(&error, "checking the file's link count"))
}

pub(super) fn canonical_root(root: impl AsRef<Path>) -> Result<PathBuf, ServiceError> {
    let root = root.as_ref();
    let directory = crate::filesystem::open_absolute_directory_no_follow(root)
        .map_err(|error| filesystem_error(&error, "opening the active tool root"))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| filesystem_error(&error, "resolving the active tool root"))?;
    let canonical_directory = crate::filesystem::open_absolute_directory_no_follow(&canonical_root)
        .map_err(|error| filesystem_error(&error, "reopening the active tool root"))?;
    if !crate::filesystem::same_directory_object(&directory, &canonical_directory)
        .map_err(|error| filesystem_error(&error, "validating the active tool root"))?
    {
        return Err(invalid("The active tool root changed while it was opened."));
    }
    Ok(canonical_root)
}

pub(super) fn safe_relative_path(path: &str) -> Result<PathBuf, ServiceError> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(
            "Tool paths must be relative to the active tool root.",
        ));
    }
    Ok(path.to_path_buf())
}

pub(super) fn is_hidden(name: &str, metadata: &fs::Metadata) -> bool {
    name.starts_with('.') || windows_hidden(metadata)
}

/// Checks a walker's cached metadata to avoid a second Windows file-system query.
#[allow(dead_code)]
pub(crate) fn walker_entry_is_hidden(name: &str, entry: &ignore::DirEntry) -> Result<bool, ()> {
    if name.starts_with('.') {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        entry
            .metadata()
            .map(|metadata| windows_hidden(&metadata))
            .map_err(|_| ())
    }
    #[cfg(not(windows))]
    {
        let _ = entry;
        Ok(false)
    }
}

pub(crate) fn is_default_ignored_directory(name: &str) -> bool {
    [
        ".git",
        ".gradle",
        ".next",
        ".nuxt",
        "build",
        "coverage",
        "DerivedData",
        "dist",
        "node_modules",
        "Pods",
        "target",
        "vendor",
    ]
    .iter()
    .any(|ignored| name.eq_ignore_ascii_case(ignored))
}

#[cfg(windows)]
pub(super) fn windows_hidden(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
pub(super) fn windows_hidden(_metadata: &fs::Metadata) -> bool {
    false
}

pub(super) fn invalid(message: &str) -> ServiceError {
    ServiceError::new("invalid_tool_input", message, false)
}

pub(super) fn unavailable(message: &str) -> ServiceError {
    ServiceError::new("tool_path_unavailable", message, false)
}

pub(super) fn file_change_lock_unavailable() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "File changes could not be coordinated safely.",
        true,
    )
}

pub(crate) fn filesystem_error_code(error: &io::Error) -> &'static str {
    #[cfg(windows)]
    match error.raw_os_error() {
        Some(32) => return "filesystem_file_locked",
        Some(39 | 112 | 1295) => return "filesystem_storage_full",
        Some(2 | 3) => return "filesystem_not_found",
        Some(5) => return "filesystem_permission_denied",
        Some(183) => return "filesystem_already_exists",
        _ => {}
    }

    match error.kind() {
        io::ErrorKind::PermissionDenied => "filesystem_permission_denied",
        io::ErrorKind::NotFound => "filesystem_not_found",
        io::ErrorKind::AlreadyExists => "filesystem_already_exists",
        io::ErrorKind::StorageFull => "filesystem_storage_full",
        io::ErrorKind::InvalidInput => "filesystem_invalid_path",
        io::ErrorKind::InvalidData => "filesystem_invalid_data",
        _ => "filesystem_io_error",
    }
}

pub(super) fn filesystem_error(error: &io::Error, operation: &str) -> ServiceError {
    ServiceError::new(
        filesystem_error_code(error),
        format!("The operating system reported an error while {operation}."),
        false,
    )
    .with_details(format!("{operation}: {error}"))
}
