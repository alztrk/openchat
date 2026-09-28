use super::{
    MAX_LIST_OFFSET, MAX_READ_LINES, MAX_TOOL_ARGUMENT_BYTES, MAX_TOOL_CALLS_PER_TURN,
    MAX_TOOL_ROUNDS, canonical_root, get_file_info, invalid, list_files, read_file,
    safe_relative_path, search_files,
};
use crate::{
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ToolActivity, ToolCall, ToolResult},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolPermissionMode {
    RequireApproval,
    FullAccess,
}

impl ToolPermissionMode {
    pub fn from_rpc(value: Option<&str>) -> Result<Self, ServiceError> {
        match value {
            None | Some("require_approval") => Ok(Self::RequireApproval),
            Some("full_access") => Ok(Self::FullAccess),
            Some(_) => Err(ServiceError::new(
                "invalid_tool_permission_mode",
                "The selected tool permission mode is invalid.",
                false,
            )),
        }
    }
}

#[derive(Clone, Copy)]
enum ToolOperation {
    List,
    Search,
    Read,
    Info,
}

#[derive(Clone, Copy)]
enum ToolPathScope {
    Project,
    OpenChat,
    Full,
}

struct PreparedToolCall {
    root: PathBuf,
    call: ToolCall,
    target_path: PathBuf,
    scope: ToolPathScope,
    operation: ToolOperation,
}

pub struct ToolExecutor {
    project_root: Option<PathBuf>,
    data_root: PathBuf,
    permission_mode: ToolPermissionMode,
    rounds: usize,
    calls: usize,
}

pub(crate) fn tool_call_limit_error() -> ServiceError {
    ServiceError::new(
        "tool_iteration_limit",
        "The response reached the file-tool call limit. Start a new request to continue.",
        false,
    )
}

impl ToolExecutor {
    pub fn new(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
    ) -> Self {
        Self {
            project_root: project_root.map(Path::to_path_buf),
            data_root: data_root.to_path_buf(),
            permission_mode,
            rounds: 0,
            calls: 0,
        }
    }

    pub fn begin_round(&mut self, calls: &[ToolCall]) -> Result<(), ServiceError> {
        if calls.is_empty() {
            return Ok(());
        }
        if self.rounds >= MAX_TOOL_ROUNDS
            || self.calls.saturating_add(calls.len()) > MAX_TOOL_CALLS_PER_TURN
        {
            return Err(tool_call_limit_error());
        }
        self.rounds += 1;
        self.calls += calls.len();
        Ok(())
    }

    pub async fn execute_call(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<ToolResult, ServiceError> {
        let mut prepared = match self.prepare_call(call) {
            Ok(prepared) => prepared,
            Err(output) => {
                self.send_activity(call, None, output.clone(), request_id, snapshot, events)
                    .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }
        };

        if self.permission_mode == ToolPermissionMode::RequireApproval {
            let approved_target = prepared.target_path.clone();
            let target_path = prepared.target_path.to_string_lossy().into_owned();
            self.emit_activity(
                call,
                ToolActivity::awaiting_approval(call, target_path.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            let approved = match permissions
                .request_approval(
                    request_id,
                    &call.name,
                    &target_path,
                    &call.arguments,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(approved) => approved,
                Err(error) if error.code == "operation_cancelled" => {
                    let output = tool_error("operation_cancelled", "The request was stopped.");
                    self.send_cancelled_activity(
                        call,
                        target_path,
                        output,
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    self.send_activity(
                        call,
                        Some(target_path),
                        tool_error(error.code, &error.message),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
            };
            if !approved {
                let output = tool_error(
                    "permission_denied",
                    "The user denied this tool call. Do not retry the same operation unless the user asks.",
                );
                self.send_denied_activity(
                    call,
                    target_path,
                    output.clone(),
                    request_id,
                    snapshot,
                    events,
                )
                .await?;
                return Ok(ToolResult {
                    call_id: call.id.clone(),
                    output,
                });
            }

            prepared = match self.prepare_call(call) {
                Ok(prepared) if prepared.target_path == approved_target => prepared,
                Ok(_) => {
                    let output = tool_error(
                        "permission_target_changed",
                        "The requested path changed after approval. Request permission again.",
                    );
                    self.send_denied_activity(
                        call,
                        target_path,
                        output.clone(),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Ok(ToolResult {
                        call_id: call.id.clone(),
                        output,
                    });
                }
                Err(output) => {
                    self.send_activity(
                        call,
                        Some(target_path),
                        output.clone(),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Ok(ToolResult {
                        call_id: call.id.clone(),
                        output,
                    });
                }
            };
        }

        self.send_running_activity(
            call,
            prepared.target_path.to_string_lossy().into_owned(),
            request_id,
            snapshot,
            events,
        )
        .await?;
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.send_cancelled_activity(
                call,
                prepared.target_path.to_string_lossy().into_owned(),
                output,
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }
        let mut output = execute_model_tool(&prepared.root, &prepared.call);
        qualify_output_paths(&mut output, &prepared);
        self.send_activity(
            call,
            Some(prepared.target_path.to_string_lossy().into_owned()),
            output.clone(),
            request_id,
            snapshot,
            events,
        )
        .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    fn prepare_call(&self, call: &ToolCall) -> Result<PreparedToolCall, Value> {
        let serialized_size = serde_json::to_vec(&call.arguments)
            .map_err(|_| tool_error("invalid_tool_input", "The tool arguments are invalid."))?
            .len();
        if serialized_size > MAX_TOOL_ARGUMENT_BYTES {
            return Err(tool_error(
                "invalid_tool_input",
                "The tool arguments exceed the supported size.",
            ));
        }
        let Some(arguments) = call.arguments.as_object() else {
            return Err(tool_error(
                "invalid_tool_input",
                "The tool arguments must be a JSON object.",
            ));
        };
        let (operation, directory) = match call.name.as_str() {
            "list_files" if only_keys(&call.arguments, &["path", "offset", "limit"]) => {
                if usize_argument(&call.arguments, "offset")
                    .is_some_and(|value| value <= MAX_LIST_OFFSET)
                    && usize_argument(&call.arguments, "limit").is_some_and(|value| value > 0)
                {
                    (ToolOperation::List, true)
                } else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The file listing arguments are invalid.",
                    ));
                }
            }
            "search_files"
                if only_keys(
                    &call.arguments,
                    &["path", "query", "includeHidden", "offset", "limit"],
                ) && string_argument(&call.arguments, "query").is_some()
                    && bool_argument(&call.arguments, "includeHidden").is_some()
                    && usize_argument(&call.arguments, "offset")
                        .is_some_and(|value| value <= 1_000_000)
                    && usize_argument(&call.arguments, "limit").is_some_and(|value| value > 0)
                    && string_argument(&call.arguments, "query").is_some_and(|query| {
                        !query.is_empty()
                            && query.len() <= 4096
                            && !query.chars().any(char::is_control)
                    }) =>
            {
                (ToolOperation::Search, true)
            }
            "read_file"
                if only_keys(&call.arguments, &["path", "startLine", "lineCount"])
                    && usize_argument(&call.arguments, "startLine")
                        .is_some_and(|line| line > 0)
                    && usize_argument(&call.arguments, "lineCount")
                        .is_some_and(|count| (1..=MAX_READ_LINES).contains(&count)) =>
            {
                (ToolOperation::Read, false)
            }
            "get_file_info" if only_keys(&call.arguments, &["path"]) => {
                (ToolOperation::Info, false)
            }
            _ => {
                return Err(tool_error(
                    "invalid_tool_input",
                    "The requested file tool or its arguments are invalid.",
                ));
            }
        };
        let Some(path) = arguments.get("path").and_then(Value::as_str) else {
            return Err(tool_error(
                "invalid_tool_input",
                "The requested path is invalid.",
            ));
        };

        let resolved = match self.permission_mode {
            ToolPermissionMode::RequireApproval => self.resolve_approved_scope(path, directory),
            ToolPermissionMode::FullAccess => self.resolve_full_access(path, directory),
        }?;
        let (root, relative, target_path, scope) = resolved;
        let mut normalized_call = call.clone();
        if let Some(arguments) = normalized_call.arguments.as_object_mut() {
            arguments.insert("path".to_owned(), json!(relative));
        }
        Ok(PreparedToolCall {
            root,
            call: normalized_call,
            target_path,
            scope,
            operation,
        })
    }

    fn resolve_approved_scope(
        &self,
        path: &str,
        directory: bool,
    ) -> Result<(PathBuf, String, PathBuf, ToolPathScope), Value> {
        if path.starts_with("desktop:") {
            return Err(tool_error(
                "permission_scope_denied",
                "The Desktop is outside the folders allowed in Onay İste mode.",
            ));
        }
        let (root, relative, scope) = if Path::new(path).is_absolute() {
            let target = Path::new(path).canonicalize().map_err(|_| {
                tool_error(
                    "tool_path_unavailable",
                    "The requested path is unavailable.",
                )
            })?;
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
        let target_path = root.join(&relative_path).canonicalize().map_err(|_| {
            tool_error(
                "tool_path_unavailable",
                "The requested path is unavailable.",
            )
        })?;
        if !target_path.starts_with(&root)
            || (directory && !target_path.is_dir())
            || (!directory && !target_path.is_file())
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
        let target_path = requested.canonicalize().map_err(|_| {
            tool_error(
                "tool_path_unavailable",
                "The requested path is unavailable.",
            )
        })?;
        if (directory && !target_path.is_dir()) || (!directory && !target_path.is_file()) {
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

    async fn send_running_activity(
        &self,
        call: &ToolCall,
        target_path: String,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        self.emit_activity(
            call,
            ToolActivity::running(call, Some(target_path)),
            request_id,
            snapshot,
            events,
        )
        .await
    }

    async fn send_denied_activity(
        &self,
        call: &ToolCall,
        target_path: String,
        output: Value,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        self.emit_activity(
            call,
            ToolActivity::denied(call, target_path, output),
            request_id,
            snapshot,
            events,
        )
        .await
    }

    async fn send_cancelled_activity(
        &self,
        call: &ToolCall,
        target_path: String,
        output: Value,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        self.emit_activity(
            call,
            ToolActivity::cancelled(call, target_path, output),
            request_id,
            snapshot,
            events,
        )
        .await
    }

    async fn send_activity(
        &self,
        call: &ToolCall,
        target_path: Option<String>,
        output: Value,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        self.emit_activity(
            call,
            ToolActivity::finished(call, target_path, output),
            request_id,
            snapshot,
            events,
        )
        .await
    }

    async fn emit_activity(
        &self,
        _call: &ToolCall,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let event = ChatStreamEvent::ToolActivityUpdated {
            snapshot: snapshot.clone(),
            activity,
        }
        .into_rpc(request_id.clone());
        events.send(&event).await.map_err(|_| protocol_error())
    }
}

fn execute_model_tool(root: &Path, call: &ToolCall) -> Value {
    let arguments = match serde_json::to_vec(&call.arguments) {
        Ok(arguments) if arguments.len() <= MAX_TOOL_ARGUMENT_BYTES => &call.arguments,
        _ => {
            return tool_error(
                "invalid_tool_input",
                "The tool arguments exceed the supported size.",
            );
        }
    };
    if !arguments.is_object() {
        return tool_error(
            "invalid_tool_input",
            "The tool arguments must be a JSON object.",
        );
    }
    let result = match call.name.as_str() {
        "list_files" if only_keys(arguments, &["path", "offset", "limit"]) => {
            match (
                string_argument(arguments, "path"),
                usize_argument(arguments, "offset"),
                usize_argument(arguments, "limit"),
            ) {
                (Some(path), Some(offset), Some(limit)) => list_files(root, path, offset, limit),
                _ => Err(invalid("The file listing arguments are invalid.")),
            }
        }
        "search_files"
            if only_keys(
                arguments,
                &["path", "query", "includeHidden", "offset", "limit"],
            ) =>
        {
            match (
                string_argument(arguments, "path"),
                string_argument(arguments, "query"),
                bool_argument(arguments, "includeHidden"),
                usize_argument(arguments, "offset"),
                usize_argument(arguments, "limit"),
            ) {
                (Some(path), Some(query), Some(include_hidden), Some(offset), Some(limit)) => {
                    search_files(root, path, query, include_hidden, offset, limit)
                }
                _ => Err(invalid("The file search arguments are invalid.")),
            }
        }
        "read_file" if only_keys(arguments, &["path", "startLine", "lineCount"]) => {
            match (
                string_argument(arguments, "path"),
                usize_argument(arguments, "startLine"),
                usize_argument(arguments, "lineCount"),
            ) {
                (Some(path), Some(start_line), Some(line_count)) => {
                    read_file(root, path, start_line, line_count)
                }
                _ => Err(invalid("The file read arguments are invalid.")),
            }
        }
        "get_file_info" if only_keys(arguments, &["path"]) => {
            match string_argument(arguments, "path") {
                Some(path) => get_file_info(root, path),
                None => Err(invalid("The file information arguments are invalid.")),
            }
        }
        _ => Err(invalid(
            "The requested file tool or its arguments are invalid.",
        )),
    };

    match result {
        Ok(value) => value,
        Err(error) => json!({
            "error": {
                "code": error.code,
                "message": error.message
            }
        }),
    }
}

fn tool_error(code: &str, message: &str) -> Value {
    json!({"error": {"code": code, "message": message}})
}

fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_protocol_failed",
        "The local service could not send the tool activity update.",
        true,
    )
}

fn qualify_output_paths(output: &mut Value, prepared: &PreparedToolCall) {
    if output.get("error").is_some() {
        return;
    }
    match prepared.operation {
        ToolOperation::List => qualify_path_array(
            output.get_mut("entries").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
        ),
        ToolOperation::Search => qualify_path_array(
            output.get_mut("matches").and_then(Value::as_array_mut),
            &prepared.root,
            prepared.scope,
        ),
        ToolOperation::Read | ToolOperation::Info => {
            if let Some(path) = output
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                let path = qualify_path(&prepared.root, &path, prepared.scope);
                output["path"] = json!(path);
            }
        }
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
    path.trim_start_matches(|character| character == '/' || character == '\\')
}

fn only_keys(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|arguments| arguments.keys().all(|key| allowed.contains(&key.as_str())))
}

fn string_argument<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

fn bool_argument(value: &Value, name: &str) -> Option<bool> {
    value.get(name).and_then(Value::as_bool)
}

fn usize_argument(value: &Value, name: &str) -> Option<usize> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
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
