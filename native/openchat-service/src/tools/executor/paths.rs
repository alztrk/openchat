use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::tools::{canonical_root, safe_relative_path};

use super::{PreparedToolCall, ToolExecutor, ToolOperation, ToolPermissionMode, tool_error};
#[derive(Clone, Copy)]
pub(crate) enum ToolPathScope {
    Project,
    OpenChat,
    Full,
}

impl ToolExecutor {
    pub(super) fn resolve_tool_path(
        &self,
        path: &str,
        directory: bool,
        can_create: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        match self.permission_mode {
            ToolPermissionMode::RequireApproval | ToolPermissionMode::ApproveSafeOperations => {
                self.resolve_approved_scope(path, directory, can_create)
            }
            ToolPermissionMode::FullAccess => self.resolve_full_access(path, directory, can_create),
        }
    }

    fn resolve_approved_scope(
        &self,
        path: &str,
        directory: bool,
        can_create: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        if path.starts_with("desktop:") {
            return Err(tool_error(
                "permission_scope_denied",
                "The Desktop is outside the folders allowed in Onay İste mode.",
            ));
        }
        let (root, relative, scope) = if Path::new(path).is_absolute() {
            let target = match Path::new(path).canonicalize() {
                Ok(target) => target,
                Err(_) if can_create => {
                    let requested_path = Path::new(path);
                    let parent = requested_path.parent().ok_or_else(|| {
                        tool_error("invalid_tool_input", "The file path is invalid.")
                    })?;
                    let canonical_parent = parent.canonicalize().map_err(|_| {
                        tool_error("tool_path_unavailable", "The parent folder is unavailable.")
                    })?;
                    let file_name = requested_path.file_name().ok_or_else(|| {
                        tool_error("invalid_tool_input", "The file path is invalid.")
                    })?;
                    canonical_parent.join(file_name)
                }
                Err(_) => {
                    return Err(tool_error(
                        "tool_path_unavailable",
                        "The requested path is unavailable.",
                    ));
                }
            };
            let project_root = self
                .project_root
                .as_deref()
                .and_then(|root| canonical_root(root).ok());
            let data_root = canonical_root(&self.data_root)
                .map_err(|error| tool_error(error.code, &error.message))?;
            if let Some(root) = project_root.filter(|root| target.starts_with(root)) {
                let relative = target.strip_prefix(&root).map_err(|_| {
                    tool_error(
                        "permission_scope_denied",
                        "The path is outside the allowed folders.",
                    )
                })?;
                (
                    root,
                    relative.to_string_lossy().into_owned(),
                    ToolPathScope::Project,
                )
            } else if target.starts_with(&data_root) {
                let relative = target.strip_prefix(&data_root).map_err(|_| {
                    tool_error(
                        "permission_scope_denied",
                        "The path is outside the allowed folders.",
                    )
                })?;
                (
                    data_root,
                    relative.to_string_lossy().into_owned(),
                    ToolPathScope::OpenChat,
                )
            } else {
                return Err(tool_error(
                    "permission_scope_denied",
                    "Onay İste mode only allows the selected project folder and OpenChat application data folder.",
                ));
            }
        } else {
            let (root, relative, scope) = if let Some(relative) = path.strip_prefix("project:") {
                let Some(root) = self.project_root.as_deref() else {
                    return Err(tool_error(
                        "tool_path_unavailable",
                        "This conversation has no project folder. Use the openchat:/ root instead.",
                    ));
                };
                (root, trim_root_separator(relative), ToolPathScope::Project)
            } else if let Some(relative) = path.strip_prefix("openchat:") {
                (
                    self.data_root.as_path(),
                    trim_root_separator(relative),
                    ToolPathScope::OpenChat,
                )
            } else if let Some(root) = self.project_root.as_deref() {
                (root, path, ToolPathScope::Project)
            } else {
                (self.data_root.as_path(), path, ToolPathScope::OpenChat)
            };
            let root =
                canonical_root(root).map_err(|error| tool_error(error.code, &error.message))?;
            let relative = safe_relative_path(relative)
                .map_err(|error| tool_error(error.code, &error.message))?;
            (root, relative.to_string_lossy().into_owned(), scope)
        };

        let root = canonical_root(root).map_err(|error| tool_error(error.code, &error.message))?;
        let relative_path = safe_relative_path(&relative)
            .map_err(|error| tool_error(error.code, &error.message))?;
        let target_path = match root.join(&relative_path).canonicalize() {
            Ok(path) => path,
            Err(_) if can_create => {
                let full = root.join(&relative_path);
                let parent = full.parent().unwrap_or(&root);
                let canonical_parent = parent.canonicalize().map_err(|_| {
                    tool_error("tool_path_unavailable", "The parent folder is unavailable.")
                })?;
                if !canonical_parent.starts_with(&root) {
                    return Err(tool_error(
                        "permission_scope_denied",
                        "The requested path is outside the allowed folders.",
                    ));
                }
                let file_name = full
                    .file_name()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                canonical_parent.join(file_name)
            }
            Err(_) => {
                return Err(tool_error(
                    "tool_path_unavailable",
                    "The requested path is unavailable.",
                ));
            }
        };
        if !target_path.starts_with(&root)
            || (directory && !target_path.is_dir())
            || (!directory && !can_create && !target_path.is_file())
            || (can_create && target_path.is_dir())
        {
            return Err(tool_error(
                "permission_scope_denied",
                "The requested path is outside the allowed folders or has the wrong type.",
            ));
        }
        Ok((root, relative, target_path, scope))
    }

    fn resolve_full_access(
        &self,
        path: &str,
        directory: bool,
        can_create: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        if let Some(relative) = path.strip_prefix("desktop:/") {
            return resolve_desktop_path(relative, directory);
        }
        let requested = Path::new(path);
        if !requested.is_absolute() {
            return Err(tool_error(
                "invalid_tool_input",
                "Full access mode requires an absolute filesystem path.",
            ));
        }
        let target_path = match requested.canonicalize() {
            Ok(target) => target,
            Err(_) if can_create => {
                let parent = requested
                    .parent()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                let canonical_parent = parent.canonicalize().map_err(|_| {
                    tool_error("tool_path_unavailable", "The parent folder is unavailable.")
                })?;
                let file_name = requested
                    .file_name()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                canonical_parent.join(file_name)
            }
            Err(_) => {
                return Err(tool_error(
                    "tool_path_unavailable",
                    "The requested path is unavailable.",
                ));
            }
        };
        if (directory && !target_path.is_dir())
            || (!directory && !can_create && !target_path.is_file())
            || (can_create && target_path.is_dir())
        {
            return Err(tool_error(
                "invalid_tool_input",
                "The requested path has the wrong type for this tool.",
            ));
        }
        let (root, relative) = if directory {
            (target_path.clone(), String::new())
        } else {
            let Some(parent) = target_path.parent() else {
                return Err(tool_error(
                    "invalid_tool_input",
                    "The file path is invalid.",
                ));
            };
            let Some(file_name) = target_path.file_name() else {
                return Err(tool_error(
                    "invalid_tool_input",
                    "The file path is invalid.",
                ));
            };
            (
                canonical_root(parent).map_err(|error| tool_error(error.code, &error.message))?,
                file_name.to_string_lossy().into_owned(),
            )
        };
        Ok((root, relative, target_path, ToolPathScope::Full))
    }
}

pub(super) fn qualify_output_paths(output: &mut Value, prepared: &PreparedToolCall) {
    if output.get("error").is_some() {
        return;
    }
    match &prepared.operation {
        ToolOperation::List { .. } => qualify_path_array(
            output.get_mut("entries").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
        ),
        ToolOperation::Search { .. } => qualify_path_array(
            output.get_mut("matches").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
        ),
        ToolOperation::Read { .. }
        | ToolOperation::Write { .. }
        | ToolOperation::Edit { .. }
        | ToolOperation::Info => {
            if let Some(path) = output
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                let path = qualify_path(&prepared.root, &path, prepared.scope);
                output["path"] = json!(path);
            }
        }
        ToolOperation::Bash { .. }
        | ToolOperation::SendTerminalInput { .. }
        | ToolOperation::WebSearch { .. }
        | ToolOperation::ReadUrlContent { .. }
        | ToolOperation::DelegateTask { .. }
        | ToolOperation::GitStatus
        | ToolOperation::GitDiff
        | ToolOperation::GitHistory { .. } => {}
    }
}

fn qualify_path_array(entries: Option<&mut Vec<Value>>, root: &Path, scope: ToolPathScope) {
    if let Some(entries) = entries {
        for entry in entries {
            if let Some(path) = entry.get("path").and_then(Value::as_str).map(str::to_owned) {
                let path = qualify_path(root, &path, scope);
                entry["path"] = json!(path);
            }
        }
    }
}

fn qualify_path(root: &Path, relative_path: &str, scope: ToolPathScope) -> String {
    match scope {
        ToolPathScope::Full => root
            .join(relative_path)
            .to_string_lossy()
            .replace('\\', "/"),
        ToolPathScope::Project => {
            let relative_path = relative_path.replace('\\', "/");
            format!("project:/{}", relative_path.trim_start_matches('/'))
        }
        ToolPathScope::OpenChat => {
            let relative_path = relative_path.replace('\\', "/");
            format!("openchat:/{}", relative_path.trim_start_matches('/'))
        }
    }
}

fn trim_root_separator(path: &str) -> &str {
    path.trim_start_matches(['/', '\\'])
}

fn resolve_desktop_path(
    relative: &str,
    directory: bool,
) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
    let desktop_path = dirs::desktop_dir().ok_or_else(|| {
        tool_error(
            "tool_path_unavailable",
            "The current user's Desktop folder could not be located.",
        )
    })?;
    let root =
        canonical_root(desktop_path).map_err(|error| tool_error(error.code, &error.message))?;
    let relative_path =
        safe_relative_path(relative).map_err(|error| tool_error(error.code, &error.message))?;
    let target_path = root.join(relative_path).canonicalize().map_err(|_| {
        tool_error(
            "tool_path_unavailable",
            "The requested Desktop path is unavailable.",
        )
    })?;
    if !target_path.starts_with(&root)
        || (directory && !target_path.is_dir())
        || (!directory && !target_path.is_file())
    {
        return Err(tool_error(
            "permission_scope_denied",
            "The requested path is outside the Desktop folder or has the wrong type.",
        ));
    }
    let relative = target_path
        .strip_prefix(&root)
        .map_err(|_| {
            tool_error(
                "permission_scope_denied",
                "The requested path is outside the Desktop folder.",
            )
        })?
        .to_string_lossy()
        .into_owned();
    Ok((root, relative, target_path, ToolPathScope::Full))
}
