use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_json::to_vec_pretty;

const MAX_DIAGNOSTICS_BYTES: usize = 16 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportDiagnostics {
    format_version: u8,
    application: String,
    generated_at_utc: String,
    runtime: SupportRuntime,
    storage: SupportStorage,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SupportRuntime {
    platform: String,
    app_version: String,
    runtime: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SupportStorage {
    history_status: String,
    service_status: String,
    database_schema_version: u8,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDataError {
    code: &'static str,
    message: &'static str,
}

impl LocalDataError {
    fn invalid() -> Self {
        Self {
            code: "invalid_request_params",
            message: "The support diagnostics data is invalid.",
        }
    }

    fn write_failed() -> Self {
        Self {
            code: "diagnostics_export_failed",
            message: "The support diagnostics file could not be written.",
        }
    }
}

#[tauri::command]
pub fn export_support_diagnostics(
    path: String,
    diagnostics: SupportDiagnostics,
) -> Result<(), LocalDataError> {
    validate_diagnostics(&diagnostics)?;
    let contents = to_vec_pretty(&diagnostics).map_err(|_| LocalDataError::invalid())?;
    if contents.len() > MAX_DIAGNOSTICS_BYTES {
        return Err(LocalDataError::invalid());
    }

    let output = PathBuf::from(path);
    if !output.is_absolute()
        || !output
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err(LocalDataError::invalid());
    }
    let parent = output.parent().ok_or_else(LocalDataError::invalid)?;
    if !parent.is_dir() {
        return Err(LocalDataError::write_failed());
    }
    if let Ok(metadata) = fs::symlink_metadata(&output) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LocalDataError::invalid());
        }
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output)
        .map_err(|_| LocalDataError::write_failed())?;
    file.write_all(&contents)
        .and_then(|()| file.flush())
        .map_err(|_| LocalDataError::write_failed())
}

fn validate_diagnostics(value: &SupportDiagnostics) -> Result<(), LocalDataError> {
    if value.format_version != 1
        || value.application != "OpenChat"
        || value.generated_at_utc.len() > 40
        || !value.generated_at_utc.ends_with('Z')
        || value.runtime.platform != "windows"
        || value.runtime.runtime != "Tauri"
        || value.runtime.app_version.is_empty()
        || value.runtime.app_version.len() > 64
        || !value
            .runtime
            .app_version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        || (value.storage.history_status != "available"
            && value.storage.history_status != "unavailable")
        || value.storage.service_status != "ready"
        || !(1..=100).contains(&value.storage.database_schema_version)
    {
        return Err(LocalDataError::invalid());
    }
    Ok(())
}
