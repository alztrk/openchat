use serde_json::Value;
use std::path::PathBuf;

use super::{ToolExecutor, paths::ToolPathScope, tool_error};
use crate::{
    provider_schema::ToolCall,
    tools::{MAX_LIST_OFFSET, MAX_READ_LINES, MAX_TOOL_ARGUMENT_BYTES},
};

#[derive(Debug)]
pub(crate) enum ToolOperation {
    List {
        offset: usize,
        limit: usize,
    },
    Search {
        query: String,
        include_hidden: bool,
        offset: usize,
        limit: usize,
    },
    Read {
        start_line: usize,
        line_count: usize,
    },
    Write {
        content: String,
    },
    Edit {
        old_string: String,
        new_string: String,
    },
    Bash {
        command: Option<String>,
        terminal_id: Option<String>,
        input: Option<String>,
        action: Option<String>,
        timeout_seconds: Option<u64>,
        wait_ms: Option<u64>,
    },
    SendTerminalInput {
        terminal_id: String,
        input: Option<String>,
        action: Option<String>,
        wait_ms: Option<u64>,
    },
    WebSearch {
        query: String,
        limit: Option<usize>,
    },
    ReadUrlContent {
        url: String,
        max_chars: Option<usize>,
    },
    DelegateTask {
        task: String,
        context: Option<String>,
    },
    GitStatus,
    GitDiff,
    GitHistory {
        limit: usize,
    },
    Info,
}

impl ToolOperation {
    pub(crate) fn is_safe_for_auto_approval(&self) -> bool {
        matches!(
            self,
            Self::List { .. }
                | Self::Search { .. }
                | Self::Read { .. }
                | Self::GitStatus
                | Self::GitDiff
                | Self::GitHistory { .. }
                | Self::Info
        )
    }

    pub(crate) fn targets_directory(&self) -> bool {
        matches!(self, Self::List { .. } | Self::Search { .. })
    }

    pub(crate) fn can_create_file(&self) -> bool {
        matches!(self, Self::Write { .. })
    }
}

pub(crate) struct PreparedToolCall {
    pub(crate) root: PathBuf,
    pub(crate) relative_path: String,
    pub(crate) requested_path: String,
    pub(crate) target_path: PathBuf,
    pub(crate) scope: ToolPathScope,
    pub(crate) operation: ToolOperation,
}

impl ToolExecutor {
    pub(crate) fn prepare_call(&self, call: &ToolCall) -> Result<PreparedToolCall, Value> {
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
        let operation = match call.name.as_str() {
            "delegate_task" => {
                if !only_keys(&call.arguments, &["task", "context"]) {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The delegated task arguments are invalid.",
                    ));
                }
                let Some(task) = string_argument(&call.arguments, "task") else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The delegated task is required.",
                    ));
                };
                let context = string_argument(&call.arguments, "context");
                if task.trim().is_empty()
                    || task.len() > 4000
                    || context.is_some_and(|value| value.len() > 8000)
                {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The delegated task or context exceeds the supported limits.",
                    ));
                }
                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                return Ok(PreparedToolCall {
                    root: root.clone(),
                    relative_path: String::new(),
                    requested_path: "delegated analysis".to_owned(),
                    target_path: root,
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::DelegateTask {
                        task: task.to_owned(),
                        context: context.map(str::to_owned),
                    },
                });
            }
            "run_project_task" => {
                if !only_keys(&call.arguments, &["task"]) {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The project task arguments are invalid.",
                    ));
                }
                let Some(requested_task) = string_argument(&call.arguments, "task") else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The project task id is required.",
                    ));
                };
                let Some(root) = self.project_root.clone() else {
                    return Err(tool_error(
                        "project_required",
                        "Named project tasks require an attached project.",
                    ));
                };
                let task = crate::tools::project_tasks::load_project_task(&root, requested_task)
                    .map_err(|error| tool_error(&error.code, &error.message))?;
                let command = task.command;
                return Ok(PreparedToolCall {
                    root: root.clone(),
                    relative_path: String::new(),
                    requested_path: task.id,
                    target_path: root,
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::Bash {
                        command: Some(command),
                        terminal_id: None,
                        input: None,
                        action: None,
                        timeout_seconds: Some(task.timeout_seconds),
                        wait_ms: None,
                    },
                });
            }
            "bash" | "execute_command" => {
                let command = string_argument(&call.arguments, "command").map(str::to_owned);
                let terminal_id = string_argument(&call.arguments, "terminal_id")
                    .or_else(|| string_argument(&call.arguments, "terminalId"))
                    .map(str::to_owned);
                let input = string_argument(&call.arguments, "input").map(str::to_owned);
                let action = string_argument(&call.arguments, "action").map(str::to_owned);
                let timeout_seconds = u64_argument(&call.arguments, "timeout_seconds")
                    .or_else(|| u64_argument(&call.arguments, "timeoutSeconds"));
                let wait_ms = u64_argument(&call.arguments, "wait_ms")
                    .or_else(|| u64_argument(&call.arguments, "waitMs"));

                if command.is_none() && terminal_id.is_none() {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "Either command or terminal_id is required.",
                    ));
                }

                if let Some(cmd) = &command
                    && cmd.trim().is_empty()
                {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The shell command cannot be empty.",
                    ));
                }

                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                let requested = command.as_deref().or(terminal_id.as_deref()).unwrap_or("");
                return Ok(PreparedToolCall {
                    root,
                    relative_path: String::new(),
                    requested_path: requested.to_owned(),
                    target_path: PathBuf::from(requested),
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::Bash {
                        command,
                        terminal_id,
                        input,
                        action,
                        timeout_seconds,
                        wait_ms,
                    },
                });
            }
            "send_terminal_input" => {
                let terminal_id = string_argument(&call.arguments, "terminal_id")
                    .or_else(|| string_argument(&call.arguments, "terminalId"));
                let Some(terminal_id) = terminal_id else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The terminal_id is required.",
                    ));
                };
                let input = string_argument(&call.arguments, "input").map(str::to_owned);
                let action = string_argument(&call.arguments, "action").map(str::to_owned);
                let wait_ms = u64_argument(&call.arguments, "wait_ms")
                    .or_else(|| u64_argument(&call.arguments, "waitMs"));

                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                return Ok(PreparedToolCall {
                    root,
                    relative_path: String::new(),
                    requested_path: terminal_id.to_owned(),
                    target_path: PathBuf::from(terminal_id),
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::SendTerminalInput {
                        terminal_id: terminal_id.to_owned(),
                        input,
                        action,
                        wait_ms,
                    },
                });
            }
            "web_search" => {
                let Some(query) = string_argument(&call.arguments, "query") else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The search query is required.",
                    ));
                };
                if query.trim().is_empty() {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The search query cannot be empty.",
                    ));
                }
                let limit = usize_argument(&call.arguments, "limit");
                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                return Ok(PreparedToolCall {
                    root,
                    relative_path: String::new(),
                    requested_path: query.to_owned(),
                    target_path: PathBuf::from(query),
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::WebSearch {
                        query: query.to_owned(),
                        limit,
                    },
                });
            }
            "read_url_content" | "read_url" => {
                let Some(url) = string_argument(&call.arguments, "url") else {
                    return Err(tool_error("invalid_tool_input", "The URL is required."));
                };
                if url.trim().is_empty() {
                    return Err(tool_error("invalid_tool_input", "The URL cannot be empty."));
                }
                let max_chars = usize_argument(&call.arguments, "max_chars")
                    .or_else(|| usize_argument(&call.arguments, "maxChars"))
                    .or_else(|| usize_argument(&call.arguments, "max_length"));
                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                return Ok(PreparedToolCall {
                    root,
                    relative_path: String::new(),
                    requested_path: url.to_owned(),
                    target_path: PathBuf::from(url),
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::ReadUrlContent {
                        url: url.to_owned(),
                        max_chars,
                    },
                });
            }
            "git_status" | "git_diff" | "git_history" => {
                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                let operation = match call.name.as_str() {
                    "git_status" => ToolOperation::GitStatus,
                    "git_diff" => ToolOperation::GitDiff,
                    "git_history" => {
                        let limit = usize_argument(&call.arguments, "limit").unwrap_or(20);
                        if !(1..=100).contains(&limit) {
                            return Err(tool_error(
                                "invalid_tool_input",
                                "The Git history limit must be between 1 and 100.",
                            ));
                        }
                        ToolOperation::GitHistory { limit }
                    }
                    _ => {
                        return Err(tool_error(
                            "invalid_tool_input",
                            "The Git operation is invalid.",
                        ));
                    }
                };
                return Ok(PreparedToolCall {
                    root: root.clone(),
                    relative_path: String::new(),
                    requested_path: ".".to_owned(),
                    target_path: root.clone(),
                    scope: ToolPathScope::Project,
                    operation,
                });
            }
            "list_files" | "glob" | "list_directory" => {
                let offset = usize_argument(&call.arguments, "offset").unwrap_or(0);
                let limit = usize_argument(&call.arguments, "limit").unwrap_or(100);
                if offset > MAX_LIST_OFFSET || limit == 0 {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The file listing arguments are invalid.",
                    ));
                }
                ToolOperation::List { offset, limit }
            }
            "search_files" | "grep" => {
                let query = string_argument(&call.arguments, "query")
                    .or_else(|| string_argument(&call.arguments, "pattern"));
                let Some(query) = query else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The search query or pattern is required.",
                    ));
                };
                if query.is_empty() || query.len() > 4096 || query.chars().any(char::is_control) {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The requested file tool or its arguments are invalid.",
                    ));
                }
                let include_hidden =
                    bool_argument(&call.arguments, "includeHidden").unwrap_or(false);
                let offset = usize_argument(&call.arguments, "offset").unwrap_or(0);
                let limit = usize_argument(&call.arguments, "limit").unwrap_or(40);
                if offset > 1_000_000 || limit == 0 {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The search offset or limit is invalid.",
                    ));
                }
                ToolOperation::Search {
                    query: query.to_owned(),
                    include_hidden,
                    offset,
                    limit,
                }
            }
            "read" | "read_file" => {
                let start_line = usize_argument(&call.arguments, "startLine")
                    .or_else(|| usize_argument(&call.arguments, "offset"))
                    .unwrap_or(1)
                    .max(1);
                let line_count = usize_argument(&call.arguments, "lineCount")
                    .or_else(|| usize_argument(&call.arguments, "limit"))
                    .unwrap_or(MAX_READ_LINES)
                    .clamp(1, MAX_READ_LINES);
                ToolOperation::Read {
                    start_line,
                    line_count,
                }
            }
            "write" | "write_file" => {
                let Some(content) = string_argument(&call.arguments, "content") else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The file content is required for writing.",
                    ));
                };
                ToolOperation::Write {
                    content: content.to_owned(),
                }
            }
            "edit" | "edit_file" => {
                let old_string = string_argument(&call.arguments, "oldString")
                    .or_else(|| string_argument(&call.arguments, "old_string"));
                let new_string = string_argument(&call.arguments, "newString")
                    .or_else(|| string_argument(&call.arguments, "new_string"));
                let (Some(old_string), Some(new_string)) = (old_string, new_string) else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The old and new strings are required for editing.",
                    ));
                };
                ToolOperation::Edit {
                    old_string: old_string.to_owned(),
                    new_string: new_string.to_owned(),
                }
            }
            "get_file_info" => ToolOperation::Info,
            _ => {
                return Err(tool_error(
                    "invalid_tool_input",
                    "The requested file tool or its arguments are invalid.",
                ));
            }
        };
        let path = arguments
            .get("path")
            .or_else(|| arguments.get("filePath"))
            .and_then(Value::as_str);
        let path = match path {
            Some(path) => path,
            None if operation.targets_directory() => ".",
            None => {
                return Err(tool_error(
                    "invalid_tool_input",
                    "The requested path is invalid.",
                ));
            }
        };

        let (root, relative, target_path, scope) = self.resolve_tool_path(
            path,
            operation.targets_directory(),
            operation.can_create_file(),
        )?;
        Ok(PreparedToolCall {
            root,
            relative_path: relative,
            requested_path: path.to_owned(),
            target_path,
            scope,
            operation,
        })
    }
}

#[allow(dead_code)]
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

fn u64_argument(value: &Value, name: &str) -> Option<u64> {
    value.get(name).and_then(Value::as_u64)
}
