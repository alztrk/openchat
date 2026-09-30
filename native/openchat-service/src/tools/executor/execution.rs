use serde_json::{Value, json};

use crate::{
    protocol::ServiceError,
    tools::{
        edit_file, get_file_info, list_files, read_file, search_files,
        terminal::TerminalSessionManager,
        web_search::{execute_read_url, execute_web_search},
        write_file,
    },
};

use super::{PreparedToolCall, ToolOperation};

pub(crate) async fn execute_model_tool(prepared: &PreparedToolCall) -> Value {
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
        ToolOperation::Bash {
            command,
            terminal_id,
            input,
            action,
            timeout_seconds,
            wait_ms,
        } => {
            let manager = TerminalSessionManager::global();
            if let Some(cmd) = command {
                match manager
                    .execute(cmd, &prepared.root, *timeout_seconds, *wait_ms)
                    .await
                {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "command_failed", "message": err}}),
                }
            } else if let Some(tid) = terminal_id {
                if action.as_deref() == Some("kill") {
                    match manager.kill_session(tid).await {
                        Ok(val) => return val,
                        Err(err) => {
                            return json!({"error": {"code": "terminal_error", "message": err}});
                        }
                    }
                } else if action.as_deref() == Some("read") || (input.is_none() && action.is_none())
                {
                    match manager.read_output_of(tid, *wait_ms).await {
                        Ok(val) => return val,
                        Err(err) => {
                            return json!({"error": {"code": "terminal_error", "message": err}});
                        }
                    }
                } else {
                    let input_str = input.as_deref().unwrap_or("");
                    match manager.send_input_to(tid, input_str, *wait_ms).await {
                        Ok(val) => return val,
                        Err(err) => {
                            return json!({"error": {"code": "terminal_error", "message": err}});
                        }
                    }
                }
            } else {
                return json!({
                    "error": {
                        "code": "invalid_tool_input",
                        "message": "Either command or terminal_id is required."
                    }
                });
            }
        }
        ToolOperation::SendTerminalInput {
            terminal_id,
            input,
            action,
            wait_ms,
        } => {
            let manager = TerminalSessionManager::global();
            if action.as_deref() == Some("kill") {
                match manager.kill_session(terminal_id).await {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "terminal_error", "message": err}}),
                }
            } else if action.as_deref() == Some("read") || (input.is_none() && action.is_none()) {
                match manager.read_output_of(terminal_id, *wait_ms).await {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "terminal_error", "message": err}}),
                }
            } else {
                let input_str = input.as_deref().unwrap_or("");
                match manager
                    .send_input_to(terminal_id, input_str, *wait_ms)
                    .await
                {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "terminal_error", "message": err}}),
                }
            }
        }
        ToolOperation::WebSearch { query, limit } => {
            match execute_web_search(query, *limit).await {
                Ok(val) => return val,
                Err(err) => return json!({"error": {"code": "search_failed", "message": err}}),
            }
        }
        ToolOperation::ReadUrlContent { url, max_chars } => {
            match execute_read_url(url, *max_chars).await {
                Ok(val) => return val,
                Err(err) => return json!({"error": {"code": "read_url_failed", "message": err}}),
            }
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
    use crate::tools::executor::{PreparedToolCall, ToolOperation, paths::ToolPathScope};
    use std::path::PathBuf;

    #[tokio::test]
    async fn execute_command_runs_echo_successfully() {
        let temp_dir = std::env::temp_dir();
        let prepared = PreparedToolCall {
            root: temp_dir,
            relative_path: String::new(),
            requested_path: "echo hello_openchat".to_owned(),
            target_path: PathBuf::new(),
            scope: ToolPathScope::Project,
            operation: ToolOperation::Bash {
                command: Some("echo hello_openchat".to_owned()),
                terminal_id: None,
                input: None,
                action: None,
                timeout_seconds: Some(10),
                wait_ms: Some(3000),
            },
        };

        let result = execute_model_tool(&prepared).await;
        assert_eq!(result["exit_code"], 0);
        let output = result["output"].as_str().unwrap_or("");
        assert!(output.contains("hello_openchat"));
    }
}
