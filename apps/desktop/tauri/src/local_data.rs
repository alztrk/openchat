//! Validates and exports the privacy-limited support diagnostics bundle.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::to_vec_pretty;

use crate::{
    file_ops::replace_file_atomically,
    request_bounds::{deserialize_bounded_text, BoundedText},
    save_dialog::{select_save_file, SaveFileDialogError},
};

const MAX_DIAGNOSTICS_BYTES: usize = 16 * 1024;
const MAXIMUM_DIAGNOSTICS_APPLICATION_BYTES: usize = 8;
const MAXIMUM_DIAGNOSTICS_TIMESTAMP_BYTES: usize = 40;
const MAXIMUM_DIAGNOSTICS_PLATFORM_BYTES: usize = 7;
const MAXIMUM_DIAGNOSTICS_VERSION_BYTES: usize = 64;
const MAXIMUM_DIAGNOSTICS_RUNTIME_BYTES: usize = 5;
const MAXIMUM_DIAGNOSTICS_SERVICE_STATUS_BYTES: usize = 5;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Versioned, privacy-limited application health information for support requests.
pub struct SupportDiagnostics {
    format_version: u8,
    #[serde(deserialize_with = "deserialize_diagnostics_application")]
    application: String,
    #[serde(deserialize_with = "deserialize_diagnostics_timestamp")]
    generated_at_utc: String,
    runtime: SupportRuntime,
    storage: SupportStorage,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Runtime identity fields included in a diagnostics export.
struct SupportRuntime {
    #[serde(deserialize_with = "deserialize_diagnostics_platform")]
    platform: String,
    #[serde(deserialize_with = "deserialize_diagnostics_version")]
    app_version: String,
    #[serde(deserialize_with = "deserialize_diagnostics_runtime")]
    runtime: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Storage availability summary included without conversation or credential data.
struct SupportStorage {
    history_status: HistoryStorageStatus,
    #[serde(deserialize_with = "deserialize_diagnostics_service_status")]
    service_status: String,
    database_schema_version: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Allowlisted history states that the renderer may include in support diagnostics.
enum HistoryStorageStatus {
    Loading,
    Available,
    Unavailable,
    Corrupt,
    BackupUnavailable,
}

/// Deserializes the fixed application label within its accepted byte length.
fn deserialize_diagnostics_application<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_APPLICATION_BYTES>(deserializer)
}

/// Deserializes the UTC timestamp within its existing 40-byte validation limit.
fn deserialize_diagnostics_timestamp<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_TIMESTAMP_BYTES>(deserializer)
}

/// Deserializes the supported operating-system label within its accepted byte length.
fn deserialize_diagnostics_platform<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_PLATFORM_BYTES>(deserializer)
}

/// Deserializes the application version within its existing 64-byte validation limit.
fn deserialize_diagnostics_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_VERSION_BYTES>(deserializer)
}

/// Deserializes the fixed runtime label within its accepted byte length.
fn deserialize_diagnostics_runtime<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_RUNTIME_BYTES>(deserializer)
}

/// Deserializes the fixed service status within its accepted byte length.
fn deserialize_diagnostics_service_status<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIAGNOSTICS_SERVICE_STATUS_BYTES>(deserializer)
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
/// Safe error contract returned by support diagnostics commands.
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
/// Validates a diagnostics bundle, asks where to save it, then publishes the JSON file.
///
/// Cancellation returns `Ok(false)`. Invalid input and filesystem failures use stable,
/// non-sensitive errors; the output is size-limited and replaces the selected path only
/// after the complete staging file has been written and synchronized.
///
/// # Parameters
/// - `window`: Parent window for the native save dialog.
/// - `title`: Localized dialog title, limited to 128 UTF-8 bytes during argument deserialization.
/// - `diagnostics`: Versioned health fields collected by the frontend.
///
/// # Returns
/// `Ok(true)` when saved, `Ok(false)` when cancelled, or a validated command error.
pub async fn export_support_diagnostics(
    window: tauri::WebviewWindow,
    title: BoundedText<128>,
    diagnostics: SupportDiagnostics,
) -> Result<bool, LocalDataError> {
    let title = title.into_inner();
    validate_diagnostics(&diagnostics)?;
    let contents = to_vec_pretty(&diagnostics).map_err(|_| LocalDataError::invalid())?;
    if contents.len() > MAX_DIAGNOSTICS_BYTES
        || title.is_empty()
        || title.len() > 128
        || title.chars().any(char::is_control)
    {
        return Err(LocalDataError::invalid());
    }

    let Some(output) = select_save_file(
        &window,
        "openchat-support-diagnostics.json".to_owned(),
        title,
        "JSON",
        "json",
    )
    .await
    .map_err(|error| match error {
        SaveFileDialogError::Unavailable => LocalDataError::write_failed(),
    })?
    else {
        return Ok(false);
    };
    if !is_valid_diagnostics_export_path(&output) {
        return Err(LocalDataError::invalid());
    }
    let parent = output.parent().ok_or_else(LocalDataError::invalid)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| LocalDataError::write_failed())?;
    if !canonical_parent.is_dir() {
        return Err(LocalDataError::write_failed());
    }
    let file_name = output.file_name().ok_or_else(LocalDataError::invalid)?;
    let destination = canonical_parent.join(file_name);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(LocalDataError::invalid());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(LocalDataError::write_failed()),
    }

    write_diagnostics_atomically(&destination, &contents)?;
    Ok(true)
}

/// Publishes completed bytes by replacing the destination with a same-directory staging file.
///
/// A unique file created with `create_new` prevents collisions. Failed writes leave the
/// destination untouched, and the cleanup guard removes an unpublished staging file.
fn write_diagnostics_atomically(destination: &Path, contents: &[u8]) -> Result<(), LocalDataError> {
    let parent = destination.parent().ok_or_else(LocalDataError::invalid)?;
    let staging = parent.join(format!(
        ".openchat-support-diagnostics-{}.tmp",
        uuid::Uuid::new_v4()
    ));
    let mut cleanup = StagingFileCleanup {
        path: staging.clone(),
        retained: false,
    };
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(contents)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        return Err(LocalDataError::write_failed());
    }
    replace_file_atomically(&staging, destination).map_err(|_| LocalDataError::write_failed())?;
    cleanup.retained = true;
    Ok(())
}

/// Removes a staging file when the replace operation did not publish it.
struct StagingFileCleanup {
    path: std::path::PathBuf,
    retained: bool,
}

/// Cleans up temporary output after errors or early returns.
impl Drop for StagingFileCleanup {
    fn drop(&mut self) {
        if !self.retained {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Accepts only an absolute destination with a JSON extension.
fn is_valid_diagnostics_export_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

/// Enforces the supported schema and excludes unapproved diagnostic content.
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
        || value.storage.service_status != "ready"
        || !(1..=100).contains(&value.storage.database_schema_version)
    {
        return Err(LocalDataError::invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        is_valid_diagnostics_export_path, validate_diagnostics, write_diagnostics_atomically,
        HistoryStorageStatus, SupportDiagnostics, SupportRuntime, SupportStorage,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-support-diagnostics-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated diagnostics directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn diagnostics_export_requires_an_absolute_json_path() {
        let absolute_json_path = std::env::temp_dir().join("diagnostics.json");
        let absolute_markdown_path = std::env::temp_dir().join("diagnostics.md");
        assert!(is_valid_diagnostics_export_path(&absolute_json_path));
        assert!(!is_valid_diagnostics_export_path(Path::new(
            "diagnostics.json"
        )));
        assert!(!is_valid_diagnostics_export_path(&absolute_markdown_path));
    }

    #[test]
    fn diagnostics_accept_all_supported_history_states() {
        for history_status in [
            HistoryStorageStatus::Loading,
            HistoryStorageStatus::Available,
            HistoryStorageStatus::Unavailable,
            HistoryStorageStatus::Corrupt,
            HistoryStorageStatus::BackupUnavailable,
        ] {
            let diagnostics = SupportDiagnostics {
                format_version: 1,
                application: "OpenChat".to_owned(),
                generated_at_utc: "2026-10-18T12:00:00.000Z".to_owned(),
                runtime: SupportRuntime {
                    platform: "windows".to_owned(),
                    app_version: "0.1.8".to_owned(),
                    runtime: "Tauri".to_owned(),
                },
                storage: SupportStorage {
                    history_status,
                    service_status: "ready".to_owned(),
                    database_schema_version: 1,
                },
            };

            assert!(validate_diagnostics(&diagnostics).is_ok());
        }
    }

    #[test]
    fn diagnostics_deserialization_preserves_renderer_shape_and_bounds_version() {
        let diagnostics = serde_json::json!({
            "formatVersion": 1,
            "application": "OpenChat",
            "generatedAtUtc": "2026-10-18T12:00:00.000Z",
            "runtime": {
                "platform": "windows",
                "appVersion": "0.1.8",
                "runtime": "Tauri"
            },
            "storage": {
                "historyStatus": "available",
                "serviceStatus": "ready",
                "databaseSchemaVersion": 18
            }
        });
        let parsed = serde_json::from_value::<SupportDiagnostics>(diagnostics.clone())
            .expect("renderer diagnostics payload should deserialize");
        assert!(validate_diagnostics(&parsed).is_ok());

        let mut oversized = diagnostics;
        oversized["runtime"]["appVersion"] = serde_json::Value::String("1".repeat(65));
        assert!(serde_json::from_value::<SupportDiagnostics>(oversized).is_err());
    }

    #[test]
    fn diagnostics_history_states_match_the_renderer_wire_values() {
        for (wire_value, expected) in [
            ("loading", HistoryStorageStatus::Loading),
            ("available", HistoryStorageStatus::Available),
            ("unavailable", HistoryStorageStatus::Unavailable),
            ("corrupt", HistoryStorageStatus::Corrupt),
            ("backupUnavailable", HistoryStorageStatus::BackupUnavailable),
        ] {
            let encoded = serde_json::to_string(&expected).expect("serialize history state");
            let decoded = serde_json::from_str::<HistoryStorageStatus>(&encoded)
                .expect("deserialize history state");

            assert_eq!(encoded, format!("\"{wire_value}\""));
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn diagnostics_reject_unknown_history_states() {
        let result = serde_json::from_str::<SupportDiagnostics>(
            r#"{"formatVersion":1,"application":"OpenChat","generatedAtUtc":"2026-10-18T12:00:00.000Z","runtime":{"platform":"windows","appVersion":"0.1.8","runtime":"Tauri"},"storage":{"historyStatus":"unknown","serviceStatus":"ready","databaseSchemaVersion":1}}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn diagnostics_export_replaces_an_existing_file_with_complete_contents() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("diagnostics.json");
        fs::write(&destination, b"old contents").expect("write existing diagnostics file");

        write_diagnostics_atomically(&destination, b"new complete contents")
            .expect("replace diagnostics file atomically");

        assert_eq!(
            fs::read(&destination).expect("read replaced diagnostics file"),
            b"new complete contents"
        );
    }
}
