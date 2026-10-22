//! Dispatches prepared tool operations to filesystem, terminal, web, and project handlers.
//!
//! The execution owner conversation is supplied by the trusted RPC stream context rather than
//! model arguments, and terminal sessions stay scoped to that conversation.

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

/// Executes a prepared model tool call and scopes interactive terminal access to its conversation.
///
/// The conversation identifier must come from the trusted stream snapshot, never tool arguments.
pub(crate) async fn execute_model_tool(
    prepared: &PreparedToolCall,
    owner_conversation_id: &str,
) -> Value {
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
                    .execute(
                        cmd,
                        &prepared.root,
                        owner_conversation_id,
                        *timeout_seconds,
                        *wait_ms,
                    )
                    .await
                {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "command_failed", "message": err}}),
                }
            } else if let Some(tid) = terminal_id {
                if action.as_deref() == Some("kill") {
                    match manager.kill_session(tid, owner_conversation_id).await {
                        Ok(val) => return val,
                        Err(err) => {
                            return json!({"error": {"code": "terminal_error", "message": err}});
                        }
                    }
                } else if action.as_deref() == Some("read") || (input.is_none() && action.is_none())
                {
                    match manager
                        .read_output_of(tid, owner_conversation_id, *wait_ms)
                        .await
                    {
                        Ok(val) => return val,
                        Err(err) => {
                            return json!({"error": {"code": "terminal_error", "message": err}});
                        }
                    }
                } else {
                    let input_str = input.as_deref().unwrap_or("");
                    match manager
                        .send_input_to(tid, owner_conversation_id, input_str, *wait_ms)
                        .await
                    {
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
                match manager
                    .kill_session(terminal_id, owner_conversation_id)
                    .await
                {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "terminal_error", "message": err}}),
                }
            } else if action.as_deref() == Some("read") || (input.is_none() && action.is_none()) {
                match manager
                    .read_output_of(terminal_id, owner_conversation_id, *wait_ms)
                    .await
                {
                    Ok(val) => return val,
                    Err(err) => return json!({"error": {"code": "terminal_error", "message": err}}),
                }
            } else {
                let input_str = input.as_deref().unwrap_or("");
                match manager
                    .send_input_to(terminal_id, owner_conversation_id, input_str, *wait_ms)
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
        ToolOperation::DelegateTask { .. } => {
            return json!({"error": {
                "code": "subagent_unavailable",
                "message": "Delegated analysis could not be started in this execution context."
            }});
        }
        ToolOperation::ListSubagentModels
        | ToolOperation::GetSubagentResult { .. }
        | ToolOperation::CancelSubagent { .. }
        | ToolOperation::SaveMemory { .. } => {
            return json!({
                "error": {
                    "code": "tool_execution_unavailable",
                    "message": "This tool must run through the permission-aware executor."
                }
            });
        }
        #[cfg(windows)]
        ToolOperation::ProjectTool { .. } => {
            return json!({"error": {
                "code": "project_tool_unavailable",
                "message": "Configured project tools must run through the approved sandboxed executor."
            }});
        }
        ToolOperation::GitStatus => {
            return git_inspection_result(crate::git_inspection::status(&prepared.root).await);
        }
        ToolOperation::GitDiff => {
            return git_inspection_result(crate::git_inspection::diff(&prepared.root).await);
        }
        ToolOperation::GitHistory { limit } => {
            return git_inspection_result(
                crate::git_inspection::history(&prepared.root, *limit).await,
            );
        }
        ToolOperation::Info => get_file_info(&prepared.root, &prepared.relative_path),
    };

    match result {
        Ok(value) => value,
        Err(error) => service_error(error),
    }
}

fn git_inspection_result(
    result: Result<Value, crate::git_inspection::GitInspectionError>,
) -> Value {
    match result {
        Ok(value) => value,
        Err(crate::git_inspection::GitInspectionError::NotRepository) => json!({
            "error": {
                "code": "not_git_repository",
                "message": "The attached project folder is not a Git repository."
            }
        }),
        Err(crate::git_inspection::GitInspectionError::TimedOut) => json!({
            "error": {
                "code": "git_operation_timed_out",
                "message": "The Git inspection command exceeded its time limit."
            }
        }),
        Err(crate::git_inspection::GitInspectionError::InvalidInput) => json!({
            "error": {
                "code": "invalid_tool_input",
                "message": "The Git inspection arguments are invalid."
            }
        }),
        Err(crate::git_inspection::GitInspectionError::Unavailable) => json!({
            "error": {
                "code": "git_unavailable",
                "message": "Git inspection could not be completed. Confirm Git is installed and try again."
            }
        }),
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
    use crate::tools::terminal::TerminalSessionManager;
    use std::path::PathBuf;

    #[tokio::test]
    async fn execute_command_runs_echo_successfully() {
        let temp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "openchat-terminal-test-{}",
                uuid::Uuid::new_v4().simple()
            ));
        std::fs::create_dir_all(&temp_dir).expect("create an isolated terminal test folder");
        let prepared = PreparedToolCall {
            root: temp_dir.clone(),
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

        const OWNER_CONVERSATION_ID: &str = "execution-tool-test";
        let mut result = execute_model_tool(&prepared, OWNER_CONVERSATION_ID).await;
        for _ in 0..5 {
            if result["is_running"] != true {
                break;
            }
            let terminal_id = result["terminal_id"]
                .as_str()
                .expect("running command should expose its terminal session");
            result = TerminalSessionManager::global()
                .read_output_of(terminal_id, OWNER_CONVERSATION_ID, Some(1_000))
                .await
                .expect("echo command should remain readable");
        }
        if result["is_running"] == true
            && let Some(terminal_id) = result["terminal_id"].as_str()
        {
            TerminalSessionManager::global()
                .kill_session(terminal_id, OWNER_CONVERSATION_ID)
                .await
                .expect("timed-out echo session should be terminated");
        }
        assert_ne!(result["is_running"], true, "echo process did not finish");
        assert_eq!(result["exit_code"], 0, "command result: {result}");
        let output = result["output"].as_str().unwrap_or("");
        assert!(output.contains("hello_openchat"));
        std::fs::remove_dir_all(temp_dir).expect("remove the isolated terminal test folder");
    }
}
