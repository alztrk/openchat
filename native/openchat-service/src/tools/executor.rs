use super::{
    MAX_TOOL_CALLS_PER_TURN, MAX_TOOL_ROUNDS, edit_file, get_file_info, list_files, read_file,
    search_files, write_file,
};
use crate::{
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ToolActivity, ToolCall, ToolResult},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

mod paths;
mod validation;
use paths::qualify_output_paths;
pub(crate) use validation::{PreparedToolCall, ToolOperation};

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

            if !matches!(prepared.operation, ToolOperation::Bash { .. }) {
                match self.resolve_tool_path(
                    &prepared.requested_path,
                    prepared.operation.targets_directory(),
                    prepared.operation.can_create_file(),
                ) {
                    Ok((root, relative_path, target, scope)) if target == approved_target => {
                        prepared.root = root;
                        prepared.relative_path = relative_path;
                        prepared.target_path = target;
                        prepared.scope = scope;
                    }
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
                }
            }
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
        let mut output = execute_model_tool(&prepared);
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

fn execute_model_tool(prepared: &PreparedToolCall) -> Value {
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
        ToolOperation::Bash { .. } => Ok(json!({
            "output": "",
            "exit": 0,
            "truncated": false
        })),
        ToolOperation::Info => get_file_info(&prepared.root, &prepared.relative_path),
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
