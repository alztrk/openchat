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
        if self.security_scan_scope_only {
            return self.resolve_security_scan_scope(path, directory, can_create);
        }
        match self.permission_mode {
            ToolPermissionMode::Plan
            | ToolPermissionMode::RequireApproval
            | ToolPermissionMode::ApproveSafeOperations => {
                self.resolve_approved_scope(path, directory, can_create)
            }
            ToolPermissionMode::FullAccess => self.resolve_full_access(path, directory, can_create),
        }
    }

    fn resolve_security_scan_scope(
        &self,
        path: &str,
        directory: bool,
        can_create: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        if can_create {
            return Err(tool_error(
                "permission_scope_denied",
                "Security scans are read-only and cannot create or change files.",
            ));
        }
        let requested_path = Path::new(path);
        if !requested_path.is_absolute() {
            return Err(tool_error(
                "invalid_tool_input",
                "Use an absolute path inside a selected security scan folder.",
            ));
        }
        let target_path = requested_path
            .canonicalize()
            .map_err(|error| filesystem_path_error(&error, "resolving the requested path"))?;
        let Some(root) = self
            .project_roots
            .iter()
            .filter_map(|root| canonical_root(root).ok())
            .find(|root| target_path.starts_with(root))
        else {
            return Err(tool_error(
                "permission_scope_denied",
                "Security scan tools can access only the selected project folders.",
            ));
        };
        if (directory && !target_path.is_dir()) || (!directory && !target_path.is_file()) {
            return Err(tool_error(
                "invalid_tool_input",
                "The requested path has the wrong type for this security scan tool.",
            ));
        }
        let relative = target_path
            .strip_prefix(&root)
            .map_err(|_| {
                tool_error(
                    "permission_scope_denied",
                    "Security scan tools can access only the selected project folders.",
                )
            })?
            .to_string_lossy()
            .into_owned();
        safe_relative_path(&relative).map_err(service_tool_error)?;
        Ok((root, relative, target_path, ToolPathScope::Project))
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::super::{ToolExecutor, ToolPermissionMode, paths::ToolPathScope};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-security-scan-paths-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    #[test]
    fn security_scan_paths_are_limited_to_selected_roots_for_every_permission_mode() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        let data_root = directory.0.join("application-data");
        let outside_root = directory.0.join("outside-project");
        fs::create_dir_all(project_root.join("src")).expect("create project source folder");
        fs::create_dir_all(&data_root).expect("create application data folder");
        fs::create_dir_all(&outside_root).expect("create outside folder");
        let project_file = project_root.join("src/main.rs");
        let data_file = data_root.join("private.txt");
        let outside_file = outside_root.join("private.txt");
        fs::write(&project_file, "fn main() {}\n").expect("create project file");
        fs::write(&data_file, "application data\n").expect("create application data file");
        fs::write(&outside_file, "outside project\n").expect("create outside file");
        let canonical_project = fs::canonicalize(project_root).expect("canonical project root");

        for permission_mode in [
            ToolPermissionMode::Plan,
            ToolPermissionMode::RequireApproval,
            ToolPermissionMode::ApproveSafeOperations,
            ToolPermissionMode::FullAccess,
        ] {
            let executor = ToolExecutor::with_allowed_tool_names(
                Some(&canonical_project),
                &data_root,
                permission_mode,
                [
                    "read_file".to_owned(),
                    "write_file".to_owned(),
                    "execute_command".to_owned(),
                ],
            )
            .with_workspace_roots(vec![canonical_project.clone()])
            .with_security_scan_scope(true, false);

            assert_eq!(executor.permission_mode, ToolPermissionMode::FullAccess);
            assert_eq!(
                executor.allowed_tool_names,
                crate::security_scan::allowed_tool_names(false)
            );
            assert!(
                executor
                    .resolve_tool_path(&project_file.to_string_lossy(), false, false)
                    .is_ok()
            );
            assert!(
                executor
                    .resolve_tool_path(&data_file.to_string_lossy(), false, false)
                    .is_err()
            );
            assert!(
                executor
                    .resolve_tool_path(&outside_file.to_string_lossy(), false, false)
                    .is_err()
            );
            assert!(
                executor
                    .resolve_tool_path(&project_file.to_string_lossy(), false, true)
                    .is_err()
            );
        }
    }

    #[test]
    fn approved_file_tools_accept_secondary_roots_and_reject_unlisted_folders() {
        let directory = TestDirectory::new();
        let primary_root = directory.0.join("primary");
        let secondary_root = directory.0.join("secondary");
        let outside_root = directory.0.join("outside");
        let data_root = directory.0.join("application-data");
        for root in [&primary_root, &secondary_root, &outside_root, &data_root] {
            fs::create_dir_all(root).expect("create workspace fixture root");
        }
        let secondary_file = secondary_root.join("src/lib.rs");
        fs::create_dir_all(secondary_file.parent().expect("secondary source parent"))
            .expect("create secondary source folder");
        fs::write(&secondary_file, "pub fn secondary() {}\n")
            .expect("write secondary workspace file");
        let outside_file = outside_root.join("private.txt");
        fs::write(&outside_file, "outside workspace\n").expect("write unlisted file");

        let primary_root = fs::canonicalize(primary_root).expect("canonicalize primary root");
        let secondary_root = fs::canonicalize(secondary_root).expect("canonicalize secondary root");
        let data_root = fs::canonicalize(data_root).expect("canonicalize data root");
        let executor = ToolExecutor::with_allowed_tool_names(
            Some(&primary_root),
            &data_root,
            ToolPermissionMode::RequireApproval,
            ["read_file".to_owned()],
        )
        .with_workspace_roots(vec![primary_root.clone(), secondary_root.clone()]);

        let (root, relative, target, scope) = executor
            .resolve_tool_path(&secondary_file.to_string_lossy(), false, false)
            .expect("secondary workspace file should be in project scope");
        assert_eq!(root, secondary_root);
        assert_eq!(
            relative,
            PathBuf::from("src").join("lib.rs").to_string_lossy()
        );
        assert!(target.starts_with(&root));
        assert!(target.is_file());
        assert!(matches!(scope, ToolPathScope::Project));
        assert!(
            executor
                .resolve_tool_path(&outside_file.to_string_lossy(), false, false)
                .is_err()
        );
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
        ToolOperation::CodeSearch { .. } => {
            qualify_path_array(
                output.get_mut("matches").and_then(Value::as_array_mut),
                &prepared.root,
                prepared.scope,
                _project_roots,
            );
            qualify_path_array(
                output.get_mut("symbols").and_then(Value::as_array_mut),
                &prepared.root,
                prepared.scope,
                _project_roots,
            );
        }
        ToolOperation::LspQuery { .. } => qualify_path_array(
            output.get_mut("locations").and_then(Value::as_array_mut),
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
