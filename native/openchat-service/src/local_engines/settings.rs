use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::protocol::ServiceError;

const SETTINGS_FILE: &str = "settings/local-engines.json";
const MAX_SETTINGS_BYTES: u64 = 8 * 1024;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalEngineSettings {
    version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    llama_server_executable_path: Option<PathBuf>,
}

pub(crate) fn llama_server_executable_path(
    storage_root: &Path,
) -> Result<Option<PathBuf>, ServiceError> {
    let settings = read_settings(storage_root)?;
    Ok(settings.llama_server_executable_path)
}

pub(crate) fn set_llama_server_executable_path(
    storage_root: &Path,
    path: Option<&str>,
) -> Result<serde_json::Value, ServiceError> {
    let executable_path = match path {
        Some(path) => Some(validate_executable_path(path)?),
        None => None,
    };
    let settings = LocalEngineSettings {
        version: 1,
        llama_server_executable_path: executable_path.clone(),
    };
    write_settings(storage_root, &settings)?;

    Ok(json!({
        "path": executable_path.as_ref().and_then(|path| path.to_str()),
        "available": executable_path.as_ref().is_some_and(|path| path.is_file()),
    }))
}

pub(crate) fn validate_saved_executable_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let canonical_path = path.canonicalize().ok()?;
    (canonical_path.is_file() && is_expected_executable_name(&canonical_path))
        .then_some(canonical_path)
}

fn validate_executable_path(path: &str) -> Result<PathBuf, ServiceError> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(invalid_executable_path_error());
    }
    let canonical_path = path
        .canonicalize()
        .map_err(|_| invalid_executable_path_error())?;
    if !canonical_path.is_file() || !is_expected_executable_name(&canonical_path) {
        return Err(invalid_executable_path_error());
    }
    Ok(canonical_path)
}

fn is_expected_executable_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    #[cfg(windows)]
    let expected = "llama-server.exe";
    #[cfg(not(windows))]
    let expected = "llama-server";
    name.eq_ignore_ascii_case(expected)
}

fn read_settings(storage_root: &Path) -> Result<LocalEngineSettings, ServiceError> {
    let path = storage_root.join(SETTINGS_FILE);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LocalEngineSettings {
                version: 1,
                llama_server_executable_path: None,
            });
        }
        Err(_) => return Err(settings_error()),
    };
    let metadata = file.metadata().map_err(|_| settings_error())?;
    if !metadata.is_file() || metadata.len() > MAX_SETTINGS_BYTES {
        return Err(settings_error());
    }
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut contents)
        .map_err(|_| settings_error())?;
    if contents.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(settings_error());
    }
    let settings =
        serde_json::from_slice::<LocalEngineSettings>(&contents).map_err(|_| settings_error())?;
    if settings.version != 1 {
        return Err(settings_error());
    }
    Ok(settings)
}

fn write_settings(storage_root: &Path, settings: &LocalEngineSettings) -> Result<(), ServiceError> {
    let settings_path = storage_root.join(SETTINGS_FILE);
    let parent = settings_path.parent().ok_or_else(settings_error)?;
    fs::create_dir_all(parent).map_err(|_| settings_error())?;

    let temporary_path = parent.join(format!(".local-engines-{}.tmp", Uuid::new_v4()));
    let contents = serde_json::to_vec(settings).map_err(|_| settings_error())?;
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|_| settings_error())?;
        file.write_all(&contents).map_err(|_| settings_error())?;
        file.sync_all().map_err(|_| settings_error())?;
        fs::rename(&temporary_path, &settings_path).map_err(|_| settings_error())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn invalid_executable_path_error() -> ServiceError {
    ServiceError::new(
        "local_engine_executable_path_invalid",
        "Choose an existing llama-server executable for this device.",
        false,
    )
}

fn settings_error() -> ServiceError {
    ServiceError::new(
        "local_engine_settings_unavailable",
        "Local engine settings could not be read or saved.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::{
        llama_server_executable_path, set_llama_server_executable_path,
        validate_saved_executable_path,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-local-engine-settings-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test root should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stores_and_clears_the_selected_llama_server_path() {
        let temporary = TestDirectory::new();
        let executable = temporary.0.join(if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        });
        fs::write(&executable, b"test executable").expect("fixture should be written");

        let saved = set_llama_server_executable_path(&temporary.0, executable.to_str())
            .expect("the existing llama-server executable should be saved");
        assert_eq!(saved["available"], true);
        assert_eq!(
            llama_server_executable_path(&temporary.0)
                .expect("settings should load")
                .as_deref(),
            Some(
                executable
                    .canonicalize()
                    .expect("path should canonicalize")
                    .as_path()
            )
        );

        set_llama_server_executable_path(&temporary.0, None)
            .expect("the executable path should be cleared");
        assert_eq!(
            llama_server_executable_path(&temporary.0).expect("settings should load"),
            None
        );
    }

    #[test]
    fn rejects_paths_that_are_not_llama_server_executables() {
        let temporary = TestDirectory::new();
        let other_file = temporary.0.join("other.exe");
        fs::write(&other_file, b"not llama-server").expect("fixture should be written");

        let error = set_llama_server_executable_path(&temporary.0, other_file.to_str())
            .expect_err("an unrelated executable must be rejected");
        assert_eq!(error.code, "local_engine_executable_path_invalid");
    }

    #[test]
    fn a_removed_saved_executable_is_reported_as_unavailable() {
        let temporary = TestDirectory::new();
        let executable = temporary.0.join(if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        });
        fs::write(&executable, b"test executable").expect("fixture should be written");
        let canonical = executable.canonicalize().expect("path should canonicalize");
        assert!(validate_saved_executable_path(&canonical).is_some());

        fs::remove_file(&executable).expect("fixture should be removed");
        assert!(validate_saved_executable_path(&canonical).is_none());
    }
}
