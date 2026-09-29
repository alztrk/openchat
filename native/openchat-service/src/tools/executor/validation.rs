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
        #[allow(dead_code)]
        command: String,
    },
    Info,
}

impl ToolOperation {
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
            "bash" | "execute_command" => {
                let Some(command) = string_argument(&call.arguments, "command") else {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The shell command is invalid.",
                    ));
                };
                if command.trim().is_empty() {
                    return Err(tool_error(
                        "invalid_tool_input",
                        "The shell command cannot be empty.",
                    ));
                }
                let root = self
                    .project_root
                    .clone()
                    .unwrap_or_else(|| self.data_root.clone());
                return Ok(PreparedToolCall {
                    root,
                    relative_path: String::new(),
                    requested_path: command.to_owned(),
                    target_path: PathBuf::from(command),
                    scope: ToolPathScope::Project,
                    operation: ToolOperation::Bash {
                        command: command.to_owned(),
                    },
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
