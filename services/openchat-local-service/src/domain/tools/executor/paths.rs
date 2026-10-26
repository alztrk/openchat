//! Resolves tool-supplied paths against the active permission scope.
//!
//! Approved modes bind paths to canonical project or application-data roots; full-access mode
//! is handled separately so path authorization follows the selected permission policy.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::tools::{canonical_root, filesystem_error_code, safe_relative_path};

use super::{PreparedToolCall, ToolExecutor, ToolOperation, ToolPermissionMode, tool_error};
/// Filesystem root class used to enforce the selected tool permission mode.
#[derive(Clone, Copy)]
pub(crate) enum ToolPathScope {
    Project,
    OpenChat,
    Full,
}

impl ToolExecutor {
    /// Resolves and authorizes a tool path before the operation touches the filesystem.
    ///
    /// `directory` selects whether the target must be a directory, while `can_create` permits
    /// an absent leaf only when its existing parent can be canonicalized inside the allowed root.
    pub(super) fn resolve_tool_path(
        &self,
        path: &str,
        directory: bool,
        can_create: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        match self.permission_mode {
            ToolPermissionMode::Plan
            | ToolPermissionMode::RequireApproval
            | ToolPermissionMode::ApproveSafeOperations => {
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
        let requested_path = Path::new(path);
        if !requested_path.is_absolute() {
            return Err(tool_error(
                "invalid_tool_input",
                "Use an absolute filesystem path. Relative paths and path aliases are not accepted.",
            ));
        }
        let target = match requested_path.canonicalize() {
            Ok(target) => target,
            Err(error) if can_create && error.kind() == std::io::ErrorKind::NotFound => {
                let parent = requested_path
                    .parent()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                let canonical_parent = parent.canonicalize().map_err(|error| {
                    filesystem_path_error(&error, "resolving the parent folder")
                })?;
                let file_name = requested_path
                    .file_name()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                canonical_parent.join(file_name)
            }
            Err(error) => {
                return Err(filesystem_path_error(
                    &error,
                    "resolving the requested path",
                ));
            }
        };
        let data_root = canonical_root(&self.data_root).map_err(service_tool_error)?;
        let (root, relative, scope) = if let Some(root) =
            self.project_roots.iter().find_map(|root| {
                canonical_root(root)
                    .ok()
                    .filter(|canonical| target.starts_with(canonical))
            }) {
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
                "The selected permission mode only allows the selected project folder and OpenChat application data folder.",
            ));
        };

        let root = canonical_root(root).map_err(service_tool_error)?;
        let relative_path = safe_relative_path(&relative).map_err(service_tool_error)?;
        let target_path = match root.join(&relative_path).canonicalize() {
            Ok(path) => path,
            Err(error) if can_create && error.kind() == std::io::ErrorKind::NotFound => {
                let full = root.join(&relative_path);
                let parent = full.parent().unwrap_or(&root);
                let canonical_parent = parent.canonicalize().map_err(|error| {
                    filesystem_path_error(&error, "resolving the parent folder")
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
            Err(error) => {
                return Err(filesystem_path_error(
                    &error,
                    "resolving the requested path",
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
        let requested = Path::new(path);
        if !requested.is_absolute() {
            return Err(tool_error(
                "invalid_tool_input",
                "Use an absolute filesystem path. Relative paths and path aliases are not accepted.",
            ));
        }
        let requested = requested.to_path_buf();
        let target_path = match requested.canonicalize() {
            Ok(target) => target,
            Err(error) if can_create && error.kind() == std::io::ErrorKind::NotFound => {
                let parent = requested
                    .parent()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                let canonical_parent = parent.canonicalize().map_err(|error| {
                    filesystem_path_error(&error, "resolving the parent folder")
                })?;
                let file_name = requested
                    .file_name()
                    .ok_or_else(|| tool_error("invalid_tool_input", "The file path is invalid."))?;
                canonical_parent.join(file_name)
            }
            Err(error) => {
                return Err(filesystem_path_error(
                    &error,
                    "resolving the requested path",
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
                canonical_root(parent).map_err(service_tool_error)?,
                file_name.to_string_lossy().into_owned(),
            )
        };
        Ok((root, relative, target_path, ToolPathScope::Full))
    }
}

fn filesystem_path_error(error: &std::io::Error, operation: &str) -> Value {
    let mut output = tool_error(
        filesystem_error_code(error),
        "The operating system reported an error while resolving the path.",
    );
    output["error"]["details"] = Value::String(format!("{operation}: {error}"));
    output
}

fn service_tool_error(error: crate::protocol::ServiceError) -> Value {
    let mut output = tool_error(error.code, &error.message);
    if let Some(details) = error.details {
        output["error"]["details"] = Value::String(details);
    }
    output
}

pub(super) fn qualify_output_paths(
    output: &mut Value,
    prepared: &PreparedToolCall,
    _project_roots: &[PathBuf],
) {
    if output.get("error").is_some() {
        return;
    }
    match &prepared.operation {
        ToolOperation::List { .. } => qualify_path_array(
            output.get_mut("entries").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
            _project_roots,
        ),
        ToolOperation::Search { .. } => qualify_path_array(
            output.get_mut("matches").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
            _project_roots,
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
        | ToolOperation::ListSubagentModels
        | ToolOperation::GetSubagentResult { .. }
        | ToolOperation::CancelSubagent { .. }
        | ToolOperation::SaveMemory { .. }
        | ToolOperation::SaveProjectMemory { .. }
        | ToolOperation::ProjectTool { .. } => {}
    }
}

fn qualify_path_array(
    entries: Option<&mut Vec<Value>>,
    root: &Path,
    scope: ToolPathScope,
    _project_roots: &[PathBuf],
) {
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
        ToolPathScope::Full | ToolPathScope::Project | ToolPathScope::OpenChat => {
            root.join(relative_path).to_string_lossy().into_owned()
        }
    }
}
