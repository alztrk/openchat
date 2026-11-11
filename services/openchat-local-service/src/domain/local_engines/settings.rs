//! Stores and validates user-selected local inference engine settings.
//!
//! Configured executable paths are canonicalized and checked against supported
//! engine binaries before settings are returned to runtime code.

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
const MAX_SETTINGS_BYTES: u64 = 32 * 1024;
const MAX_CUSTOM_ARGUMENTS_LENGTH: usize = 2048;

fn default_version() -> u8 {
    1
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalEngineSettings {
    #[serde(default = "default_version")]
    pub(crate) version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) llama_server_executable_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) gpu_layers: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) threads: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) threads_batch: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) context_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) batch_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ubatch_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) flash_attention: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mlock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) no_mmap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parallel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) custom_arguments: Option<String>,
}

/// Reads the validated saved llama-server path, if one is configured.
pub(crate) fn llama_server_executable_path(
    storage_root: &Path,
) -> Result<Option<PathBuf>, ServiceError> {
    let settings = read_settings(storage_root)?;
    Ok(settings.llama_server_executable_path)
}

/// Formats the settings as a JSON object suitable for RPC responses.
pub(crate) fn settings_json(storage_root: &Path) -> Result<serde_json::Value, ServiceError> {
    let settings = read_settings(storage_root)?;
    let available = settings
        .llama_server_executable_path
        .as_deref()
        .and_then(validate_saved_executable_path)
        .is_some();
    Ok(json!({
        "llamaServerExecutablePath": settings.llama_server_executable_path.as_ref().and_then(|path| path.to_str()),
        "llamaServerExecutableAvailable": available,
        "gpuLayers": settings.gpu_layers,
        "threads": settings.threads,
        "threadsBatch": settings.threads_batch,
        "contextSize": settings.context_size,
        "batchSize": settings.batch_size,
        "ubatchSize": settings.ubatch_size,
        "flashAttention": settings.flash_attention,
        "mlock": settings.mlock,
        "noMmap": settings.no_mmap,
        "parallel": settings.parallel,
        "customArguments": settings.custom_arguments,
    }))
}

/// Saves or clears the configured llama-server executable path.
///
/// A supplied path must be absolute, canonical, and name a supported executable.
pub(crate) fn set_llama_server_executable_path(
    storage_root: &Path,
    path: Option<&str>,
) -> Result<serde_json::Value, ServiceError> {
    let executable_path = match path {
        Some(path) => Some(validate_executable_path(path)?),
        None => None,
    };
    let mut settings = read_settings(storage_root)?;
    settings.llama_server_executable_path = executable_path.clone();
    write_settings(storage_root, &settings)?;

    Ok(json!({
        "path": executable_path.as_ref().and_then(|path| path.to_str()),
        "available": executable_path.as_ref().is_some_and(|path| path.is_file()),
    }))
}

/// Updates user-configurable local engine parameters.
pub(crate) fn update_settings(
    storage_root: &Path,
    params: &serde_json::Value,
) -> Result<serde_json::Value, ServiceError> {
    let mut settings = read_settings(storage_root)?;

    if let Some(path_val) = params.get("llamaServerExecutablePath") {
        settings.llama_server_executable_path = match path_val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) if s.trim().is_empty() => None,
            serde_json::Value::String(s) => Some(validate_executable_path(s)?),
            _ => return Err(invalid_settings_error("Invalid executable path parameter.")),
        };
    }

    if let Some(val) = params.get("gpuLayers") {
        settings.gpu_layers = parse_bounded_u32(val, 0, 999, "gpuLayers")?;
    }

    if let Some(val) = params.get("threads") {
        settings.threads = parse_bounded_u32(val, 1, 256, "threads")?;
    }

    if let Some(val) = params.get("threadsBatch") {
        settings.threads_batch = parse_bounded_u32(val, 1, 256, "threadsBatch")?;
    }

    if let Some(val) = params.get("contextSize") {
        settings.context_size = parse_bounded_u32(val, 512, 2_000_000, "contextSize")?;
    }

    if let Some(val) = params.get("batchSize") {
        settings.batch_size = parse_bounded_u32(val, 1, 131_072, "batchSize")?;
    }

    if let Some(val) = params.get("ubatchSize") {
        settings.ubatch_size = parse_bounded_u32(val, 1, 131_072, "ubatchSize")?;
    }

    if let Some(val) = params.get("parallel") {
        settings.parallel = parse_bounded_u32(val, 1, 64, "parallel")?;
    }

    if let Some(val) = params.get("flashAttention") {
        settings.flash_attention = match val {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(b) => Some(*b),
            _ => {
                return Err(invalid_settings_error(
                    "flashAttention must be boolean or null.",
                ));
            }
        };
    }

    if let Some(val) = params.get("mlock") {
        settings.mlock = match val {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(b) => Some(*b),
            _ => return Err(invalid_settings_error("mlock must be boolean or null.")),
        };
    }

    if let Some(val) = params.get("noMmap") {
        settings.no_mmap = match val {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(b) => Some(*b),
            _ => return Err(invalid_settings_error("noMmap must be boolean or null.")),
        };
    }

    if let Some(val) = params.get("customArguments") {
        settings.custom_arguments = match val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) if s.trim().is_empty() => None,
            serde_json::Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.len() > MAX_CUSTOM_ARGUMENTS_LENGTH {
                    return Err(invalid_settings_error(
                        "Custom arguments string is too long.",
                    ));
                }
                if trimmed.contains('\n')
                    || trimmed.contains('\r')
                    || trimmed.contains(';')
                    || trimmed.contains('&')
                    || trimmed.contains('|')
                    || trimmed.contains('`')
                    || trimmed.contains('$')
                {
                    return Err(invalid_settings_error(
                        "Custom arguments contain unsupported or dangerous shell characters.",
                    ));
                }
                Some(trimmed.to_owned())
            }
            _ => {
                return Err(invalid_settings_error(
                    "customArguments must be a string or null.",
                ));
            }
        };
    }

    write_settings(storage_root, &settings)?;
    settings_json(storage_root)
}

fn parse_bounded_u32(
    val: &serde_json::Value,
    min: u32,
    max: u32,
    _field_name: &str,
) -> Result<Option<u32>, ServiceError> {
    match val {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(num) => {
            let Some(int_val) = num.as_u64() else {
                return Err(invalid_settings_error(
                    "Numeric value must be a positive integer.",
                ));
            };
            let u32_val = u32::try_from(int_val)
                .map_err(|_| invalid_settings_error("Integer out of bounds."))?;
            if u32_val < min || u32_val > max {
                return Err(invalid_settings_error("Value exceeds allowed range."));
            }
            Ok(Some(u32_val))
        }
        _ => Err(invalid_settings_error("Expected integer or null.")),
    }
}

/// Revalidates a stored executable path before it is used by the runtime.
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

pub(crate) fn read_settings(storage_root: &Path) -> Result<LocalEngineSettings, ServiceError> {
    let path = storage_root.join(SETTINGS_FILE);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LocalEngineSettings {
                version: 1,
                ..Default::default()
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

fn invalid_settings_error(message: &'static str) -> ServiceError {
    ServiceError::new("invalid_request_params", message, false)
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

    use serde_json::json;
    use uuid::Uuid;

    use super::{
        llama_server_executable_path, read_settings, set_llama_server_executable_path,
        settings_json, update_settings, validate_saved_executable_path,
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
    fn updates_and_persists_manual_hardware_and_runtime_settings() {
        let temporary = TestDirectory::new();
        let params = json!({
            "gpuLayers": 33,
            "threads": 8,
            "threadsBatch": 12,
            "contextSize": 16384,
            "batchSize": 1024,
            "ubatchSize": 512,
            "flashAttention": true,
            "mlock": true,
            "noMmap": false,
            "parallel": 2,
            "customArguments": "--numa --split-mode row",
        });

        let updated =
            update_settings(&temporary.0, &params).expect("settings update should succeed");
        assert_eq!(updated["gpuLayers"], 33);
        assert_eq!(updated["threads"], 8);
        assert_eq!(updated["threadsBatch"], 12);
        assert_eq!(updated["contextSize"], 16384);
        assert_eq!(updated["batchSize"], 1024);
        assert_eq!(updated["ubatchSize"], 512);
        assert_eq!(updated["flashAttention"], true);
        assert_eq!(updated["mlock"], true);
        assert_eq!(updated["noMmap"], false);
        assert_eq!(updated["parallel"], 2);
        assert_eq!(updated["customArguments"], "--numa --split-mode row");

        let loaded = read_settings(&temporary.0).expect("settings should load from disk");
        assert_eq!(loaded.gpu_layers, Some(33));
        assert_eq!(loaded.threads, Some(8));
        assert_eq!(loaded.flash_attention, Some(true));
        assert_eq!(
            loaded.custom_arguments.as_deref(),
            Some("--numa --split-mode row")
        );

        // Test clearing a setting with null
        let clear_params = json!({
            "gpuLayers": null,
            "customArguments": null,
        });
        let cleared =
            update_settings(&temporary.0, &clear_params).expect("clearing should succeed");
        assert_eq!(cleared["gpuLayers"], serde_json::Value::Null);
        assert_eq!(cleared["customArguments"], serde_json::Value::Null);
        // Preserved remaining
        assert_eq!(cleared["threads"], 8);
        assert_eq!(cleared["flashAttention"], true);

        let json_representation = settings_json(&temporary.0).expect("settings json should format");
        assert_eq!(json_representation["threads"], 8);
    }

    #[test]
    fn rejects_out_of_bounds_or_dangerous_custom_settings() {
        let temporary = TestDirectory::new();

        // Threads too high
        let error = update_settings(&temporary.0, &json!({"threads": 500}))
            .expect_err("threads > 256 should be rejected");
        assert_eq!(error.code, "invalid_request_params");

        // Context size too small
        let error = update_settings(&temporary.0, &json!({"contextSize": 100}))
            .expect_err("contextSize < 512 should be rejected");
        assert_eq!(error.code, "invalid_request_params");

        // Shell injection characters in custom arguments
        let error = update_settings(&temporary.0, &json!({"customArguments": "--foo; rm -rf /"}))
            .expect_err("semicolon should be rejected");
        assert_eq!(error.code, "invalid_request_params");

        let error = update_settings(
            &temporary.0,
            &json!({"customArguments": "--foo && echo bar"}),
        )
        .expect_err("ampersand should be rejected");
        assert_eq!(error.code, "invalid_request_params");
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
