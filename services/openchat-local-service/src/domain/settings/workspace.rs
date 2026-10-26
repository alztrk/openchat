//! Resolves the working directory reported to providers for project-free chats.
//!
//! The renderer stores the user's fallback folder in `app_settings` as a raw
//! string. The value is re-read and re-validated on the service side before it
//! is written into a prompt, so a stale or tampered setting can never inject
//! unvalidated text into a provider request.

use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde_json::Value;

use crate::storage::AppStorage;

/// Renderer-owned setting holding the fallback working directory.
pub(crate) const DEFAULT_WORKSPACE_SETTING: &str = "openchat.defaultWorkspacePath";

/// Upper bound for a workspace path accepted from settings or written to a prompt.
const MAX_WORKSPACE_PATH_BYTES: usize = 4 * 1024;

/// Reads and validates the configured default working directory.
pub(crate) fn default_workspace_path(storage: &AppStorage) -> Option<PathBuf> {
    let raw = {
        let connection = storage.connect().ok()?;
        connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                [DEFAULT_WORKSPACE_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()??
    };
    let candidate = decode_setting_path(&raw)?;
    validated_workspace_path(Path::new(&candidate))
}

/// Resolves the saved project folder for one conversation, then the default.
///
/// Used by background work such as title generation, which only has the
/// conversation identifier and not the full chat send context.
pub(crate) fn conversation_workspace_path(
    storage: &AppStorage,
    conversation_id: &str,
) -> Option<PathBuf> {
    let project_folder = storage.connect().ok().and_then(|connection| {
        connection
            .query_row(
                "SELECT projects.folder_path
                 FROM conversations
                 LEFT JOIN projects ON projects.id = conversations.project_id
                 WHERE conversations.id = ?1",
                [conversation_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
    });
    project_folder
        .and_then(|folder| validated_workspace_path(Path::new(&folder)))
        .or_else(|| default_workspace_path(storage))
}

/// Accepts both the renderer's raw strings and JSON-quoted values.
fn decode_setting_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::String(value)) => {
            let value = value.trim().to_owned();
            (!value.is_empty()).then_some(value)
        }
        _ => Some(trimmed.to_owned()),
    }
}

/// Canonicalizes an absolute directory that is safe to embed in a prompt.
pub(crate) fn validated_workspace_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let display = path.to_str()?;
    if !is_prompt_safe_path(display) {
        return None;
    }
    let canonical = std::fs::canonicalize(path).ok()?;
    canonical.is_dir().then_some(canonical)
}

/// Resolves the primary project root, then the configured default directory.
pub(crate) fn resolve_chat_workspace(
    storage: &AppStorage,
    project_root: Option<&Path>,
    workspace_roots: &[PathBuf],
) -> Option<PathBuf> {
    workspace_roots
        .first()
        .and_then(|root| validated_workspace_path(root))
        .or_else(|| project_root.and_then(validated_workspace_path))
        .or_else(|| default_workspace_path(storage))
}

/// Formats a canonical path without the Windows verbatim prefix.
///
/// `std::fs::canonicalize` returns `\\?\C:\...` on Windows. The prefix is
/// meaningless to a model and only adds noise to the environment block.
pub(crate) fn prompt_workspace_path(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if !is_prompt_safe_path(text) {
        return None;
    }
    Some(strip_verbatim_prefix(text))
}

fn strip_verbatim_prefix(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    text.strip_prefix(r"\\?\").unwrap_or(text).to_owned()
}

/// Rejects paths that could break the `<env>` block or exceed prompt budgets.
fn is_prompt_safe_path(text: &str) -> bool {
    text.len() <= MAX_WORKSPACE_PATH_BYTES
        && !text.chars().any(|ch| {
            ch.is_control() || ch == '<' || ch == '>' || ch == '\u{2028}' || ch == '\u{2029}'
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{decode_setting_path, prompt_workspace_path, validated_workspace_path};

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("openchat-workspace-{label}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory");
        path
    }

    fn test_storage(label: &str) -> crate::storage::AppStorage {
        let root = temporary_directory(label);
        crate::storage::AppStorage::open_at(root.join("data")).expect("open test storage")
    }

    fn initialize_settings_schema(storage: &crate::storage::AppStorage) {
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS app_settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .expect("create settings table");
    }

    #[test]
    fn default_workspace_setting_accepts_raw_and_json_values() {
        let storage = test_storage("settings-raw");
        initialize_settings_schema(&storage);
        let workspace = temporary_directory("settings-raw-value");
        let canonical = fs::canonicalize(&workspace).expect("canonical workspace");
        {
            let connection = storage.connect().expect("connect test storage");
            connection
                .execute(
                    "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)",
                    rusqlite::params![
                        super::DEFAULT_WORKSPACE_SETTING,
                        workspace.to_string_lossy().into_owned()
                    ],
                )
                .expect("save raw setting");
        }
        assert_eq!(super::default_workspace_path(&storage), Some(canonical));

        let quoted = serde_json::to_string(&workspace.to_string_lossy().into_owned())
            .expect("quote workspace path");
        {
            let connection = storage.connect().expect("connect test storage");
            connection
                .execute(
                    "UPDATE app_settings SET value_json = ?1 WHERE key = ?2",
                    rusqlite::params![quoted, super::DEFAULT_WORKSPACE_SETTING],
                )
                .expect("save quoted setting");
        }
        assert_eq!(
            super::default_workspace_path(&storage),
            Some(fs::canonicalize(&workspace).expect("canonical workspace"))
        );
    }

    #[test]
    fn default_workspace_setting_ignores_missing_or_invalid_folders() {
        let storage = test_storage("settings-invalid");
        initialize_settings_schema(&storage);
        assert_eq!(super::default_workspace_path(&storage), None);
        {
            let connection = storage.connect().expect("connect test storage");
            connection
                .execute(
                    "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)",
                    rusqlite::params![
                        super::DEFAULT_WORKSPACE_SETTING,
                        temporary_directory("settings-missing")
                            .join("deleted")
                            .to_string_lossy()
                            .into_owned()
                    ],
                )
                .expect("save missing setting");
        }
        assert_eq!(super::default_workspace_path(&storage), None);
    }

    #[test]
    fn conversation_workspace_prefers_the_saved_project() {
        let storage = test_storage("conversation");
        initialize_settings_schema(&storage);
        let project = temporary_directory("conversation-project");
        let fallback = temporary_directory("conversation-fallback");
        {
            let connection = storage.connect().expect("connect test storage");
            connection
                .execute_batch(
                    "CREATE TABLE conversations (
                        id TEXT PRIMARY KEY NOT NULL,
                        project_id TEXT
                     );
                     CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT);",
                )
                .expect("create project tables");
            connection
                .execute(
                    "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                    rusqlite::params!["project-1", project.to_string_lossy().into_owned()],
                )
                .expect("save project");
            connection
                .execute(
                    "INSERT INTO conversations (id, project_id) VALUES (?1, ?2)",
                    rusqlite::params!["conversation-1", "project-1"],
                )
                .expect("save project conversation");
            connection
                .execute(
                    "INSERT INTO conversations (id, project_id) VALUES (?1, NULL)",
                    rusqlite::params!["conversation-2"],
                )
                .expect("save project-free conversation");
            connection
                .execute(
                    "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)",
                    rusqlite::params![
                        super::DEFAULT_WORKSPACE_SETTING,
                        fallback.to_string_lossy().into_owned()
                    ],
                )
                .expect("save default setting");
        }

        assert_eq!(
            super::conversation_workspace_path(&storage, "conversation-1"),
            Some(fs::canonicalize(&project).expect("canonical project"))
        );
        assert_eq!(
            super::conversation_workspace_path(&storage, "conversation-2"),
            Some(fs::canonicalize(&fallback).expect("canonical fallback"))
        );
        assert_eq!(
            super::conversation_workspace_path(&storage, "missing"),
            Some(fs::canonicalize(&fallback).expect("canonical fallback"))
        );
    }

    #[test]
    fn resolve_chat_workspace_prefers_the_primary_project_root() {
        let storage = test_storage("resolve");
        initialize_settings_schema(&storage);
        let project = temporary_directory("resolve-project");
        let default = temporary_directory("resolve-default");
        {
            let connection = storage.connect().expect("connect test storage");
            connection
                .execute(
                    "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)",
                    rusqlite::params![
                        super::DEFAULT_WORKSPACE_SETTING,
                        default.to_string_lossy().into_owned()
                    ],
                )
                .expect("save default setting");
        }
        assert_eq!(
            super::resolve_chat_workspace(&storage, None, &[project.clone()]),
            Some(fs::canonicalize(&project).expect("canonical project"))
        );
        assert_eq!(
            super::resolve_chat_workspace(&storage, Some(&project), &[]),
            Some(fs::canonicalize(&project).expect("canonical project"))
        );
        assert_eq!(
            super::resolve_chat_workspace(&storage, None, &[]),
            Some(fs::canonicalize(&default).expect("canonical default"))
        );
    }

    #[test]
    fn raw_and_json_encoded_setting_values_are_accepted() {
        assert_eq!(
            decode_setting_path(r"D:\projects\OpenChat").as_deref(),
            Some(r"D:\projects\OpenChat")
        );
        assert_eq!(
            decode_setting_path(r#""D:\\projects\\OpenChat""#).as_deref(),
            Some(r"D:\projects\OpenChat")
        );
        assert_eq!(decode_setting_path("   "), None);
    }

    #[test]
    fn validation_rejects_relative_missing_and_unsafe_paths() {
        assert!(validated_workspace_path(Path::new("relative/folder")).is_none());
        let directory = temporary_directory("valid");
        assert_eq!(
            validated_workspace_path(&directory).expect("canonical directory"),
            fs::canonicalize(&directory).expect("canonical directory")
        );
        assert!(validated_workspace_path(&directory.join("missing")).is_none());
    }

    #[test]
    fn prompt_paths_have_no_verbatim_prefix_or_line_breaks() {
        let directory = temporary_directory("prompt");
        let canonical = fs::canonicalize(&directory).expect("canonical directory");
        let prompt = prompt_workspace_path(&canonical).expect("prompt path");
        assert!(!prompt.starts_with(r"\\?\"));
        assert!(!prompt.contains('\n'));
        assert!(!prompt.contains('\r'));
        assert_eq!(
            super::strip_verbatim_prefix(r"\\?\UNC\server\share\folder"),
            r"\\server\share\folder"
        );
        assert_eq!(
            super::strip_verbatim_prefix(r"\\?\C:\projects"),
            r"C:\projects"
        );
    }
}
