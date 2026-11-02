//! MCP tool-call execution and activity emission.

use super::*;

impl ToolExecutor {
    #[cfg(windows)]
    pub(super) async fn execute_mcp_call(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        context: ToolEventContext<'_>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolEventContext {
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
        } = context;
        let Some(registry) = self.mcp_registry.as_ref() else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some(server_id) = registry.server_id_for_tool(&call.name) else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some(tool_name) = registry.tool_name_for_tool(&call.name) else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let Some((server_permission_fingerprint, definition_fingerprint)) =
            registry.permission_fingerprints_for_tool(&call.name)
        else {
            return Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested an MCP tool that is not active for this request.",
                false,
            ));
        };
        let target = format!("MCP server {server_id}, tool {tool_name}");
        let tool_rule = mcp_tool_permission_rule(
            &self.permission_rules,
            server_id,
            server_permission_fingerprint,
            tool_name,
            definition_fingerprint,
        );
        if tool_rule == ToolPermissionRule::Deny {
            let output = tool_error(
                "permission_denied",
                "This MCP tool is denied by the project's permission rules.",
            );
            self.emit_activity(
                storage,
                ToolActivity::denied(call, target, output.clone()),
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
        if tool_rule != ToolPermissionRule::Allow {
            self.emit_activity(
                storage,
                ToolActivity::awaiting_approval(call, target.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            let approved = match permissions
                .request_approval(
                    request_id,
                    &call.name,
                    &target,
                    &call.arguments,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(approved) => approved,
                Err(error) if error.code == "operation_cancelled" => {
                    let output = tool_error("operation_cancelled", "The request was stopped.");
                    self.emit_activity(
                        storage,
                        ToolActivity::cancelled(call, target, output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    let output = tool_error(error.code, &error.message);
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(call, Some(target), output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
            };
            if !approved {
                let output = tool_error("permission_denied", "The user denied this MCP tool call.");
                self.emit_activity(
                    storage,
                    ToolActivity::denied(call, target, output.clone()),
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

        self.emit_activity(
            storage,
            ToolActivity::running(call, Some(target.clone())),
            request_id,
            snapshot,
            events,
        )
        .await?;
        let arguments = match call.arguments.as_object() {
            Some(arguments) => arguments.clone(),
            None => {
                let output = tool_error(
                    "invalid_tool_arguments",
                    "The MCP tool arguments must be a JSON object.",
                );
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, Some(target), output.clone()),
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
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(call, target, output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }
        let call_future =
            registry.call_tool(&call.name, arguments, std::time::Duration::from_secs(120));
        tokio::pin!(call_future);
        let result = loop {
            tokio::select! {
                result = &mut call_future => break result,
                changed = cancellation.changed() => {
                    if changed.is_err() || *cancellation.borrow() {
                        let output = tool_error("operation_cancelled", "The request was stopped.");
                        self.emit_activity(
                            storage,
                            ToolActivity::cancelled(call, target, output),
                            request_id,
                            snapshot,
                            events,
                        ).await?;
                        return Err(crate::permissions::operation_cancelled_error());
                    }
                }
            }
        };
        let mut output = match result {
            Ok(result) if result.get("isError").and_then(Value::as_bool) == Some(true) => {
                let details = result
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| {
                        (item.get("type").and_then(Value::as_str) == Some("text"))
                            .then(|| item.get("text").and_then(Value::as_str))
                            .flatten()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut output = json!({
                    "error": {
                        "code": "mcp_tool_failed",
                        "message": "The MCP server reported a tool error."
                    },
                    "untrusted": true,
                    "result": result
                });
                if !details.is_empty() {
                    output["error"]["details"] =
                        Value::String(details.chars().take(1000).collect());
                }
                output
            }
            Ok(result) => json!({"untrusted": true, "result": result}),
            Err(error) => {
                let mut output =
                    tool_error("mcp_tool_failed", "The MCP tool call failed or timed out.");
                output["error"]["details"] = Value::String(error);
                output["untrusted"] = Value::Bool(true);
                output
            }
        };
        redact_tool_output(&mut output);
        self.emit_activity(
            storage,
            ToolActivity::finished(call, Some(target), output.clone()),
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

    pub(super) async fn emit_activity(
        &self,
        storage: &AppStorage,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let activity = self.prepare_activity(activity, snapshot);
        let activities = self.record_activity(&activity).await;
        crate::chatgpt_store::save_assistant_tool_checkpoint_with_retry(
            storage,
            snapshot,
            &activities,
            "tool_activity_checkpoint_save",
        )
        .await
        .map_err(|error| tool_activity_storage_error(&error))?;
        self.emit_prepared_activity(activity, request_id, snapshot, events)
            .await
    }
}
fn tool_activity_storage_error(error: &rusqlite::Error) -> ServiceError {
    let detail_code = crate::storage::sqlite_error_code(error);
    ServiceError::new(
        "tool_activity_storage_failed",
        "The tool result could not be saved to this conversation.",
        !matches!(
            detail_code,
            "message_content_too_large" | "message_metadata_too_large"
        ),
    )
    .with_details(detail_code)
}
