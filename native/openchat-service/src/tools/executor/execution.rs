use serde_json::{Value, json};

use crate::{
    protocol::ServiceError,
    tools::{edit_file, get_file_info, list_files, read_file, search_files, write_file},
};

use super::{PreparedToolCall, ToolOperation};

pub(super) fn execute_model_tool(prepared: &PreparedToolCall) -> Value {
    let result = match &prepared.operation {
        ToolOperation::List { offset, limit } => {
            list_files(&prepared.root, &prepared.relative_path, *offset, *limit)
        }
        ToolOperation::Search {
            query,
            include_hidden,
            offset,
            limit,
        } => search_files(
            &prepared.root,
            &prepared.relative_path,
            query,
            *include_hidden,
            *offset,
            *limit,
        ),
        ToolOperation::Read {
            start_line,
            line_count,
        } => read_file(
            &prepared.root,
            &prepared.relative_path,
            *start_line,
            *line_count,
        ),
        ToolOperation::Write { content } => {
            write_file(&prepared.root, &prepared.relative_path, content)
        }
        ToolOperation::Edit {
            old_string,
            new_string,
        } => edit_file(
            &prepared.root,
            &prepared.relative_path,
            old_string,
            new_string,
        ),
        ToolOperation::Bash { .. } => {
            return json!({
                "error": {
                    "code": "tool_not_supported",
                    "message": "Command execution is not supported by this client."
                }
            });
        }
        ToolOperation::Info => get_file_info(&prepared.root, &prepared.relative_path),
    };

    match result {
        Ok(value) => value,
        Err(error) => service_error(error),
    }
}

fn service_error(error: ServiceError) -> Value {
    json!({
        "error": {
            "code": error.code,
            "message": error.message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::execute_model_tool;
    use crate::tools::executor::{
        PreparedToolCall, ToolOperation,
        paths::ToolPathScope,
    };
    use std::path::PathBuf;

    #[test]
    fn unsupported_command_returns_an_error_instead_of_fake_success() {
        let prepared = PreparedToolCall {
            root: PathBuf::new(),
            relative_path: String::new(),
            requested_path: "echo hi".to_owned(),
            target_path: PathBuf::new(),
            scope: ToolPathScope::Project,
            operation: ToolOperation::Bash {
                command: "echo hi".to_owned(),
            },
        };

        let result = execute_model_tool(&prepared);

        assert_eq!(result["error"]["code"], "tool_not_supported");
    }
}
