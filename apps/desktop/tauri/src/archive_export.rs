//! Exposes the one-time native save operation for staged encrypted archives.

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{
    file_ops::publish_file_without_replacing,
    local_service::LocalServiceState,
    request_bounds::BoundedText,
    save_dialog::{select_save_file, SaveFileDialogError},
};
use serde::Serialize;
use tauri::State;

const MAX_CONVERSATION_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_PROFILE_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MARKER_CONTENT_PREFIX: &str = "openchat-archive-export-v1\n";
const MAXIMUM_ARCHIVE_KIND_BYTES: usize = 12;
const MAXIMUM_EXPORT_ID_BYTES: usize = 32;
const MAXIMUM_ARCHIVE_FILE_NAME_BYTES: usize = 255;
const MAXIMUM_DIALOG_TITLE_BYTES: usize = 128;

/// Error returned when a pending archive cannot be saved safely.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveExportError {
    code: &'static str,
    message: &'static str,
}

impl ArchiveExportError {
    /// Builds an error for invalid staged data or unsupported caller input.
    fn invalid() -> Self {
        Self {
            code: "archive_export_invalid",
            message: "The pending archive or selected destination is invalid.",
        }
    }

    /// Builds a generic error that does not reveal local path or I/O details.
    fn unavailable() -> Self {
        Self {
            code: "archive_export_unavailable",
            message: "The encrypted archive could not be saved safely.",
        }
    }

    /// Builds an error when the selected destination already exists.
    fn conflict() -> Self {
        Self {
            code: "archive_export_conflict",
            message: "A file already exists at the selected destination.",
        }
    }
}

#[derive(Clone, Copy)]
/// Archive family used to select the permitted extension, filter, and size limit.
enum ArchiveKind {
    Conversation,
    Profile,
}

impl ArchiveKind {
    /// Parses the IPC archive kind using the supported stable names.
    fn parse(value: &str) -> Result<Self, ArchiveExportError> {
        match value {
            "conversation" => Ok(Self::Conversation),
            "profile" => Ok(Self::Profile),
            _ => Err(ArchiveExportError::invalid()),
        }
    }

    /// Returns the required destination extension for this archive family.
    fn extension(self) -> &'static str {
        match self {
            Self::Conversation => "openchatbackup",
            Self::Profile => "openchatprofilebackup",
        }
    }

    /// Returns the localized-save-dialog filter label key text.
    fn filter_name(self) -> &'static str {
        match self {
            Self::Conversation => "OpenChat conversation archive",
            Self::Profile => "OpenChat profile backup",
        }
    }

    /// Returns the maximum permitted staged archive size.
    fn max_bytes(self) -> u64 {
        match self {
            Self::Conversation => MAX_CONVERSATION_ARCHIVE_BYTES,
            Self::Profile => MAX_PROFILE_ARCHIVE_BYTES,
        }
    }
}

/// Opens a native save dialog and consumes an archive staged by the local service.
///
/// The staged marker, source file, archive kind, filename, and destination are validated before
/// copying. The source is streamed through a bounded buffer into a newly created partial file;
/// an existing destination is preserved. Cancelling the dialog consumes and cleans up the
/// pending export, and I/O errors are returned without exposing local paths. Renderer text fields
/// are bounded to 12, 32, 255, and 128 UTF-8 bytes respectively during argument deserialization.
///
/// # Errors
/// Returns a validation, dialog, or storage error when the pending export cannot be safely saved.
#[tauri::command]
pub(crate) async fn save_pending_archive_export(
    window: tauri::WebviewWindow,
    state: State<'_, LocalServiceState>,
    kind: BoundedText<MAXIMUM_ARCHIVE_KIND_BYTES>,
    export_id: BoundedText<MAXIMUM_EXPORT_ID_BYTES>,
    file_name: BoundedText<MAXIMUM_ARCHIVE_FILE_NAME_BYTES>,
    title: BoundedText<MAXIMUM_DIALOG_TITLE_BYTES>,
) -> Result<bool, ArchiveExportError> {
    let kind = kind.into_inner();
    let export_id = export_id.into_inner();
    let file_name = file_name.into_inner();
    let title = title.into_inner();
    let kind = ArchiveKind::parse(&kind)?;
    if !is_export_id(&export_id)
        || !is_safe_file_name(&file_name, kind.extension())
        || title.is_empty()
        || title.len() > 128
        || title.chars().any(char::is_control)
    {
        return Err(ArchiveExportError::invalid());
    }

    let directory = pending_export_directory(state.storage_root_override.as_deref())?;
    let source = directory.join(format!("{export_id}.{}", kind.extension()));
    let pending_marker = directory.join(format!("{export_id}.pending"));
    let consuming_marker = directory.join(format!("{export_id}.consuming"));
    validate_pending_export(&directory, &source, &pending_marker, kind)?;
    fs::rename(&pending_marker, &consuming_marker)
        .map_err(|_| ArchiveExportError::unavailable())?;
    let cleanup = PendingExportCleanup {
        source: source.clone(),
        marker: consuming_marker,
    };

    let Some(destination) = select_save_file(
        &window,
        file_name,
        title,
        kind.filter_name(),
        kind.extension(),
    )
    .await
    .map_err(|error| match error {
        SaveFileDialogError::Unavailable => ArchiveExportError::unavailable(),
    })?
    else {
        return Ok(false);
    };
    let destination = canonical_destination(&destination, kind.extension())?;
    let export_id_for_copy = export_id.clone();
    let copy_result = tauri::async_runtime::spawn_blocking(move || {
        let _cleanup = cleanup;
        copy_archive(&source, &destination, &export_id_for_copy, kind.max_bytes())
    })
    .await
    .map_err(|_| ArchiveExportError::unavailable())?;
    copy_result?;
    Ok(true)
}

/// Resolves the app-managed staging directory and rejects paths outside the canonical app root.
fn pending_export_directory(
    storage_root_override: Option<&Path>,
) -> Result<PathBuf, ArchiveExportError> {
    let canonical_root = match storage_root_override {
        Some(storage_root) => {
            validate_directory(storage_root)?;
            fs::canonicalize(storage_root).map_err(|_| ArchiveExportError::unavailable())?
        }
        None => {
            let local_app_data =
                env::var_os("LOCALAPPDATA").ok_or_else(ArchiveExportError::unavailable)?;
            let app_data_root = PathBuf::from(local_app_data).join("OpenChat");
            fs::canonicalize(app_data_root).map_err(|_| ArchiveExportError::unavailable())?
        }
    };
    let cache = canonical_root.join("cache");
    let directory = cache.join("archive-exports");
    validate_directory(&cache)?;
    validate_directory(&directory)?;
    let canonical_directory =
        fs::canonicalize(&directory).map_err(|_| ArchiveExportError::unavailable())?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(ArchiveExportError::invalid());
    }
    Ok(canonical_directory)
}

/// Requires an existing real directory rather than a symlink or reparse-backed directory.
fn validate_directory(path: &Path) -> Result<(), ArchiveExportError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ArchiveExportError::unavailable())?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(ArchiveExportError::invalid());
    }
    Ok(())
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Confirms the service-created marker and staged archive match the requested kind and limits.
fn validate_pending_export(
    directory: &Path,
    source: &Path,
    marker: &Path,
    kind: ArchiveKind,
) -> Result<(), ArchiveExportError> {
    let marker_metadata =
        fs::symlink_metadata(marker).map_err(|_| ArchiveExportError::invalid())?;
    if !marker_metadata.is_file() || marker_metadata.file_type().is_symlink() {
        return Err(ArchiveExportError::invalid());
    }
    let marker_contents =
        fs::read_to_string(marker).map_err(|_| ArchiveExportError::unavailable())?;
    let expected_marker = format!("{MARKER_CONTENT_PREFIX}{}\n", kind.extension());
    if marker_contents != expected_marker {
        return Err(ArchiveExportError::invalid());
    }

    let source_metadata =
        fs::symlink_metadata(source).map_err(|_| ArchiveExportError::invalid())?;
    if !source_metadata.is_file() || source_metadata.file_type().is_symlink() {
        return Err(ArchiveExportError::invalid());
    }
    let canonical_source =
        fs::canonicalize(source).map_err(|_| ArchiveExportError::unavailable())?;
    if canonical_source.parent() != Some(directory) {
        return Err(ArchiveExportError::invalid());
    }
    let size = source_metadata.len();
    if size == 0 || size > kind.max_bytes() {
        return Err(ArchiveExportError::invalid());
    }
    Ok(())
}

/// Resolves the destination parent while preserving the user-selected filename.
fn canonical_destination(path: &Path, extension: &str) -> Result<PathBuf, ArchiveExportError> {
    if !path.is_absolute()
        || path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_none_or(|actual| !actual.eq_ignore_ascii_case(extension))
    {
        return Err(ArchiveExportError::invalid());
    }
    let parent = path.parent().ok_or_else(ArchiveExportError::invalid)?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| ArchiveExportError::unavailable())?;
    if !canonical_parent.is_dir() {
        return Err(ArchiveExportError::unavailable());
    }
    let file_name = path.file_name().ok_or_else(ArchiveExportError::invalid)?;
    Ok(canonical_parent.join(file_name))
}

/// Copies one validated archive without replacing existing files or exceeding its size limit.
fn copy_archive(
    source: &Path,
    destination: &Path,
    export_id: &str,
    max_bytes: u64,
) -> Result<(), ArchiveExportError> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err(ArchiveExportError::conflict());
    }
    let source_file = open_archive_source(source)?;
    let source_size = source_file
        .metadata()
        .map_err(|_| ArchiveExportError::unavailable())?
        .len();
    if source_size == 0 || source_size > max_bytes {
        return Err(ArchiveExportError::invalid());
    }

    let partial = destination.with_file_name(format!(".openchat-{export_id}.partial"));
    let _partial_cleanup = PartialFileCleanup {
        path: partial.clone(),
    };
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ArchiveExportError::conflict()
            } else {
                ArchiveExportError::unavailable()
            }
        })?;
    let copied = io::copy(
        &mut source_file.take(max_bytes.saturating_add(1)),
        &mut output,
    )
    .map_err(|_| ArchiveExportError::unavailable())?;
    if copied != source_size || copied > max_bytes {
        return Err(ArchiveExportError::unavailable());
    }
    output
        .sync_all()
        .map_err(|_| ArchiveExportError::unavailable())?;
    drop(output);
    publish_file_without_replacing(&partial, destination).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            ArchiveExportError::conflict()
        } else {
            ArchiveExportError::unavailable()
        }
    })?;
    Ok(())
}

#[cfg(windows)]
/// Opens the source without following a Windows reparse point.
fn open_archive_source(path: &Path) -> Result<File, ArchiveExportError> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    };

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| ArchiveExportError::unavailable())?;
    let metadata = file
        .metadata()
        .map_err(|_| ArchiveExportError::unavailable())?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ArchiveExportError::invalid());
    }
    Ok(file)
}

#[cfg(not(windows))]
/// Opens the source and verifies the opened handle refers to a regular file.
fn open_archive_source(path: &Path) -> Result<File, ArchiveExportError> {
    let file = File::open(path).map_err(|_| ArchiveExportError::unavailable())?;
    if !file
        .metadata()
        .map_err(|_| ArchiveExportError::unavailable())?
        .is_file()
    {
        return Err(ArchiveExportError::invalid());
    }
    Ok(file)
}

/// Accepts only the lowercase hexadecimal IDs generated by the staging service.
fn is_export_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Rejects path syntax and names that do not use the archive's required extension.
fn is_safe_file_name(file_name: &str, extension: &str) -> bool {
    let path = Path::new(file_name);
    !file_name.is_empty()
        && file_name.len() <= 255
        && !file_name.starts_with('.')
        && !file_name.chars().any(char::is_control)
        && !file_name.contains(['/', '\\', ':'])
        && path.components().count() == 1
        && path.file_name().and_then(std::ffi::OsStr::to_str) == Some(file_name)
        && path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

/// Removes a staged archive after a save attempt, including cancellation and errors.
struct PendingExportCleanup {
    source: PathBuf,
    marker: PathBuf,
}

impl Drop for PendingExportCleanup {
    /// Removes both the staged payload and its single-use marker.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.marker);
    }
}

/// Removes the temporary partial file after a publish attempt or any earlier failure.
struct PartialFileCleanup {
    path: PathBuf,
}

impl Drop for PartialFileCleanup {
    /// Deletes only the temporary partial file owned by this export attempt.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        copy_archive, is_export_id, is_safe_file_name, pending_export_directory,
        ArchiveExportError, MAX_CONVERSATION_ARCHIVE_BYTES,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "openchat-tauri-archive-export-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn archive_ids_and_names_reject_paths_and_wrong_extensions() {
        assert!(is_export_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_export_id("../0123456789abcdef0123456789ab"));
        assert!(!is_export_id("0123456789ABCDEF0123456789ABCDEF"));
        assert!(is_safe_file_name(
            "OpenChat-123.openchatbackup",
            "openchatbackup"
        ));
        assert!(!is_safe_file_name(
            "..\\OpenChat.openchatbackup",
            "openchatbackup"
        ));
        assert!(!is_safe_file_name("OpenChat.json", "openchatbackup"));
    }

    #[test]
    fn pending_export_directory_uses_the_isolated_storage_root_when_supplied() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        let staging = storage_root.join("cache").join("archive-exports");
        fs::create_dir_all(&staging).expect("create isolated archive staging directory");

        let resolved = pending_export_directory(Some(&storage_root))
            .expect("resolve staging under the isolated profile");

        assert_eq!(
            resolved,
            fs::canonicalize(staging).expect("canonicalize staging")
        );
        assert!(resolved
            .starts_with(fs::canonicalize(storage_root).expect("canonicalize storage root")));
    }

    #[test]
    fn archive_copy_is_streamed_and_does_not_replace_an_existing_file() {
        let directory = TestDirectory::new();
        let source = directory.0.join("source.openchatbackup");
        let destination = directory.0.join("saved.openchatbackup");
        fs::write(&source, b"encrypted archive").expect("write staged test archive");

        copy_archive(
            &source,
            &destination,
            "0123456789abcdef0123456789abcdef",
            MAX_CONVERSATION_ARCHIVE_BYTES,
        )
        .expect("copy staged archive");
        assert_eq!(
            fs::read(&destination).expect("read saved archive"),
            b"encrypted archive"
        );

        let conflict = copy_archive(
            &source,
            &destination,
            "fedcba9876543210fedcba9876543210",
            MAX_CONVERSATION_ARCHIVE_BYTES,
        )
        .expect_err("existing destinations must be preserved");
        assert_eq!(conflict.code, ArchiveExportError::conflict().code);
        assert_eq!(
            fs::read(&destination).expect("read preserved file"),
            b"encrypted archive"
        );
    }
}
