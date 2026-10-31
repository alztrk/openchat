//! Owns short-lived archive files that are waiting for a native save dialog.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, TryLockError},
    time::{Duration, SystemTime},
};

use serde_json::Value;
use uuid::Uuid;

use crate::protocol::ServiceError;

const ARCHIVE_EXPORT_DIRECTORY: &str = "archive-exports";
const MARKER_CONTENT_PREFIX: &str = "openchat-archive-export-v1\n";
const PENDING_EXPORT_TTL: Duration = Duration::from_secs(6 * 60 * 60);

static EXPORT_LOCK: Mutex<()> = Mutex::new(());

/// A temporary encrypted archive reserved until the Tauri host presents a save dialog.
pub(crate) struct PendingArchiveExport {
    export_id: String,
    archive_path: PathBuf,
    marker_path: PathBuf,
    pending_marker_path: PathBuf,
    retained: bool,
}

impl PendingArchiveExport {
    /// Reserves one archive destination under the application data directory.
    pub(crate) fn begin(
        storage_root: &Path,
        extension: &'static str,
    ) -> Result<Self, ServiceError> {
        if !is_supported_extension(extension) {
            return Err(storage_failed());
        }

        let _guard = match EXPORT_LOCK.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Err(export_busy()),
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
        };
        let root = fs::canonicalize(storage_root).map_err(|_| storage_failed())?;
        let cache_directory = storage_root.join("cache");
        ensure_directory(&cache_directory)?;
        let export_directory = cache_directory.join(ARCHIVE_EXPORT_DIRECTORY);
        ensure_directory(&export_directory)?;
        let canonical_export_directory =
            fs::canonicalize(&export_directory).map_err(|_| storage_failed())?;
        if !canonical_export_directory.starts_with(&root) {
            return Err(storage_failed());
        }

        clean_expired_exports(&canonical_export_directory)?;
        if has_live_export(&canonical_export_directory)? {
            return Err(export_busy());
        }

        let export_id = Uuid::new_v4().simple().to_string();
        let archive_path = canonical_export_directory.join(format!("{export_id}.{extension}"));
        let marker_path = canonical_export_directory.join(format!("{export_id}.working"));
        let pending_marker_path = canonical_export_directory.join(format!("{export_id}.pending"));
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker_path)
            .map_err(|_| storage_failed())?;
        if marker
            .write_all(format!("{MARKER_CONTENT_PREFIX}{extension}\n").as_bytes())
            .and_then(|()| marker.sync_all())
            .is_err()
        {
            let _ = fs::remove_file(&marker_path);
            return Err(storage_failed());
        }

        Ok(Self {
            export_id,
            archive_path,
            marker_path,
            pending_marker_path,
            retained: false,
        })
    }

    /// Returns the reserved file path used by the existing encrypted archive writer.
    pub(crate) fn archive_path(&self) -> &Path {
        &self.archive_path
    }

    /// Marks the completed file ready for one native save operation and returns its identifier.
    pub(crate) fn finish(mut self, mut result: Value) -> Result<Value, ServiceError> {
        let Some(object) = result.as_object_mut() else {
            return Err(storage_failed());
        };
        if fs::symlink_metadata(&self.marker_path)
            .map(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
            .unwrap_or(true)
        {
            return Err(storage_failed());
        }
        fs::rename(&self.marker_path, &self.pending_marker_path).map_err(|_| storage_failed())?;
        object.insert("exportId".to_owned(), Value::String(self.export_id.clone()));
        self.marker_path = self.pending_marker_path.clone();
        self.retained = true;
        Ok(result)
    }
}

impl Drop for PendingArchiveExport {
    fn drop(&mut self) {
        if !self.retained {
            let _ = fs::remove_file(&self.archive_path);
            let _ = fs::remove_file(&self.marker_path);
            let _ = fs::remove_file(&self.pending_marker_path);
        }
    }
}

fn ensure_directory(path: &Path) -> Result<(), ServiceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(storage_failed()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| storage_failed())
        }
        Err(_) => Err(storage_failed()),
    }
}

fn clean_expired_exports(directory: &Path) -> Result<(), ServiceError> {
    let entries = fs::read_dir(directory).map_err(|_| storage_failed())?;
    for entry in entries {
        let entry = entry.map_err(|_| storage_failed())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some((export_id, state)) = name.rsplit_once('.') else {
            continue;
        };
        if !is_export_id(export_id) || !matches!(state, "working" | "pending" | "consuming") {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| storage_failed())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(storage_failed());
        }
        let age = SystemTime::now()
            .duration_since(metadata.modified().map_err(|_| storage_failed())?)
            .unwrap_or_default();
        if age <= PENDING_EXPORT_TTL {
            continue;
        }

        for extension in ["openchatbackup", "openchatprofilebackup"] {
            remove_regular_file(&directory.join(format!("{export_id}.{extension}")))?;
        }
        fs::remove_file(entry.path()).map_err(|_| storage_failed())?;
    }
    Ok(())
}

fn has_live_export(directory: &Path) -> Result<bool, ServiceError> {
    for entry in fs::read_dir(directory).map_err(|_| storage_failed())? {
        let entry = entry.map_err(|_| storage_failed())?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((export_id, state)) = name.rsplit_once('.') else {
            continue;
        };
        if is_export_id(export_id) && matches!(state, "working" | "pending" | "consuming") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn remove_regular_file(path: &Path) -> Result<(), ServiceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            fs::remove_file(path).map_err(|_| storage_failed())
        }
        Ok(_) => Err(storage_failed()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(storage_failed()),
    }
}

fn is_supported_extension(extension: &str) -> bool {
    matches!(extension, "openchatbackup" | "openchatprofilebackup")
}

fn is_export_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn export_busy() -> ServiceError {
    ServiceError::new(
        "archive_export_busy",
        "Another archive is being created or is waiting for its save dialog.",
        true,
    )
}

fn storage_failed() -> ServiceError {
    ServiceError::new(
        "archive_export_storage_failed",
        "The encrypted archive could not be staged safely.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{PENDING_EXPORT_TTL, PendingArchiveExport, is_export_id, is_supported_extension};
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-pending-export-{}", Uuid::new_v4()));
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
    fn export_ids_and_extensions_are_strictly_bounded() {
        assert!(is_export_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_export_id("../0123456789abcdef0123456789ab"));
        assert!(!is_export_id("0123456789ABCDEF0123456789ABCDEF"));
        assert!(is_supported_extension("openchatbackup"));
        assert!(is_supported_extension("openchatprofilebackup"));
        assert!(!is_supported_extension("json"));
    }

    #[test]
    fn pending_export_is_staged_once_until_the_native_save_consumes_it() {
        let directory = TestDirectory::new();
        let export = PendingArchiveExport::begin(&directory.0, "openchatbackup")
            .expect("reserve a pending export");
        let expected_directory =
            fs::canonicalize(directory.0.join("cache").join("archive-exports"))
                .expect("canonicalize staging directory");
        assert_eq!(
            export.archive_path().parent(),
            Some(expected_directory.as_path())
        );
        assert!(is_export_id(&export.export_id));

        fs::write(export.archive_path(), b"encrypted archive").expect("write staged archive");
        let export_id = export.export_id.clone();
        let result = export
            .finish(serde_json::json!({"conversationCount": 1}))
            .expect("mark archive ready");
        assert_eq!(result["exportId"], export_id);
        assert!(
            expected_directory
                .join(format!("{export_id}.pending"))
                .is_file()
        );

        let second_export = match PendingArchiveExport::begin(&directory.0, "openchatbackup") {
            Ok(_) => panic!("only one archive can wait for a native save"),
            Err(error) => error,
        };
        assert_eq!(second_export.code, "archive_export_busy");

        fs::remove_file(expected_directory.join(format!("{export_id}.pending")))
            .expect("remove pending marker");
        fs::remove_file(expected_directory.join(format!("{export_id}.openchatbackup")))
            .expect("remove staged archive");
    }

    #[test]
    fn pending_export_ttl_is_bounded_to_six_hours() {
        assert_eq!(PENDING_EXPORT_TTL.as_secs(), 6 * 60 * 60);
    }
}
