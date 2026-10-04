use super::{MAX_TOOL_CALLS_PER_TURN, MAX_TOOL_ROUNDS};
use crate::{
    chatgpt::{ChatGptService, ImageGenerationAuth, ImageGenerationRequest, ImageQuality},
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ToolActivity, ToolCall, ToolResult},
    storage::AppStorage,
    user_question_broker::UserQuestionBroker,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tokio::sync::{Mutex, watch};

mod execution;
mod paths;
mod validation;
pub(crate) use execution::execute_model_tool;
use paths::qualify_output_paths;
pub(crate) use validation::{PreparedToolCall, ToolOperation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolPermissionMode {
    RequireApproval,
    FullAccess,
}

pub(crate) enum ImageGenerationContext<'a> {
    ChatGptOAuth {
        service: &'a ChatGptService,
        connection_id: &'a str,
        workspace_id: &'a str,
        turn_id: Option<&'a str>,
    },
    ApiKey {
        service: &'a ChatGptService,
        api_key: &'a str,
    },
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
    current_round_id: Option<String>,
    tool_activities: Mutex<Vec<ToolActivity>>,
    allowed_tool_names: HashSet<String>,
}

enum PendingFileChangeCapture {
    File {
        path: String,
        before:
            Result<Option<crate::file_changes::FileSnapshot>, crate::file_changes::TrackingError>,
    },
    Workspace {
        before: Result<crate::file_changes::WorkspaceSnapshot, crate::file_changes::TrackingError>,
    },
    None,
}

pub(crate) fn tool_call_limit_error() -> ServiceError {
    ServiceError::new(
        "tool_iteration_limit",
        "The response reached the file-tool call limit. Start a new request to continue.",
        false,
    )
}

impl ToolExecutor {
    #[cfg(test)]
    pub fn new(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
    ) -> Self {
        Self::with_allowed_tool_names(
            project_root,
            data_root,
            permission_mode,
            crate::tools::definitions()
                .into_iter()
                .map(|tool| tool.name.to_owned()),
        )
    }

    pub(crate) fn with_allowed_tool_names(
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
        allowed_tool_names: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            project_root: project_root.map(Path::to_path_buf),
            data_root: data_root.to_path_buf(),
            permission_mode,
            rounds: 0,
            calls: 0,
            current_round_id: None,
            tool_activities: Mutex::new(Vec::new()),
            allowed_tool_names: allowed_tool_names.into_iter().collect(),
        }
    }

    fn validate_tool_name(&self, tool_name: &str) -> Result<(), ServiceError> {
        if self.allowed_tool_names.contains(tool_name) {
            Ok(())
        } else {
            Err(ServiceError::new(
                "invalid_provider_response",
                "The provider requested a tool that was not included in the request.",
                false,
            ))
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
        self.current_round_id = Some(format!("round-{}", self.rounds));
        Ok(())
    }

    #[cfg(test)]
    pub async fn execute_call(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
        storage: &AppStorage,
        run_id: &str,
        provider_id: &str,
        user_questions: &UserQuestionBroker,
    ) -> Result<ToolResult, ServiceError> {
        self.execute_call_with_image_context(
            call,
            permissions,
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
            run_id,
            provider_id,
            user_questions,
            None,
        )
        .await
    }

    pub async fn execute_call_with_image_context(
        &self,
        call: &ToolCall,
        permissions: &ToolPermissionBroker,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
        storage: &AppStorage,
        run_id: &str,
        provider_id: &str,
        user_questions: &UserQuestionBroker,
        image_generation: Option<&ImageGenerationContext<'_>>,
    ) -> Result<ToolResult, ServiceError> {
        self.validate_tool_name(&call.name)?;
        if call.name == "generate_image" {
            return self
                .execute_image_call(
                    call,
                    request_id,
                    snapshot,
                    events,
                    cancellation,
                    storage,
                    image_generation,
                )
                .await;
        }
        if call.name == "ask_user" {
            let questions = match call
                .arguments
                .get("questions")
                .ok_or_else(|| question_arguments_error())
                .and_then(crate::user_question_broker::parse_questions)
            {
                Ok(questions) => questions,
                Err(error) => {
                    let output = tool_error(error.code, &error.message);
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(call, None, output.clone()),
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
            let waiting_activity =
                self.prepare_activity(ToolActivity::waiting_for_user(call), snapshot);
            let tool_activities = self.record_activity(&waiting_activity).await;
            let answers = match user_questions
                .ask(
                    storage,
                    run_id,
                    provider_id,
                    request_id,
                    &snapshot.conversation_id,
                    &snapshot.message_id,
                    &call.id,
                    &call.name,
                    &call.arguments,
                    questions,
                    snapshot,
                    &waiting_activity,
                    &tool_activities,
                    events,
                    cancellation,
                )
                .await
            {
                Ok(answers) => answers,
                Err(error) => {
                    if error.code == "operation_cancelled" {
                        self.emit_activity(
                            storage,
                            ToolActivity::cancelled(
                                call,
                                String::new(),
                                tool_error(error.code, &error.message),
                            ),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                    } else {
                        self.emit_activity(
                            storage,
                            ToolActivity::finished(
                                call,
                                None,
                                tool_error(error.code, &error.message),
                            ),
                            request_id,
                            snapshot,
                            events,
                        )
                        .await?;
                    }
                    return Err(error);
                }
            };
            let output = serde_json::to_value(answers).map_err(|_| {
                ServiceError::new(
                    "question_answer_invalid",
                    "The submitted answer could not be read. Ask the user again.",
                    false,
                )
            })?;
            let output = json!({"answers": output});
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
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

        let mut prepared = match self.prepare_call(call) {
            Ok(prepared) => prepared,
            Err(output) => {
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
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

        if self.permission_mode == ToolPermissionMode::RequireApproval {
            let approved_target = prepared.target_path.clone();
            let target_path = prepared.target_path.to_string_lossy().into_owned();
            self.emit_activity(
                storage,
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
                    self.emit_activity(
                        storage,
                        ToolActivity::cancelled(call, target_path, output),
                        request_id,
                        snapshot,
                        events,
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    self.emit_activity(
                        storage,
                        ToolActivity::finished(
                            call,
                            Some(target_path),
                            tool_error(error.code, &error.message),
                        ),
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
                self.emit_activity(
                    storage,
                    ToolActivity::denied(call, target_path, output.clone()),
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

            let is_non_fs = matches!(
                prepared.operation,
                ToolOperation::Bash { .. }
                    | ToolOperation::SendTerminalInput { .. }
                    | ToolOperation::WebSearch { .. }
                    | ToolOperation::ReadUrlContent { .. }
            );
            if !is_non_fs {
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
                        self.emit_activity(
                            storage,
                            ToolActivity::denied(call, target_path, output.clone()),
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
                        self.emit_activity(
                            storage,
                            ToolActivity::finished(call, Some(target_path), output.clone()),
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

        self.emit_activity(
            storage,
            ToolActivity::running(
                call,
                Some(prepared.target_path.to_string_lossy().into_owned()),
            ),
            request_id,
            snapshot,
            events,
        )
        .await?;
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(
                    call,
                    prepared.target_path.to_string_lossy().into_owned(),
                    output,
                ),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }
        let file_change_capture = capture_file_change_before(&prepared);
        let mut output = execute_model_tool(&prepared).await;
        qualify_output_paths(&mut output, &prepared);
        let (file_changes, file_changes_error) = finish_file_change_capture(
            storage.root(),
            &snapshot.conversation_id,
            &prepared,
            file_change_capture,
        );
        let mut activity = ToolActivity::finished(
            call,
            Some(prepared.target_path.to_string_lossy().into_owned()),
            output.clone(),
        );
        activity.file_changes = file_changes;
        activity.file_changes_error = file_changes_error;
        self.emit_activity(storage, activity, request_id, snapshot, events)
            .await?;
        Ok(ToolResult {
            call_id: call.id.clone(),
            output,
        })
    }

    async fn emit_activity(
        &self,
        storage: &AppStorage,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let activity = self.prepare_activity(activity, snapshot);
        let activities = self.record_activity(&activity).await;
        crate::chatgpt_store::save_assistant_tool_checkpoint(storage, snapshot, &activities)
            .map_err(|_| tool_activity_storage_error())?;
        self.emit_prepared_activity(activity, request_id, snapshot, events)
            .await
    }

    async fn execute_image_call(
        &self,
        call: &ToolCall,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
        storage: &AppStorage,
        image_generation: Option<&ImageGenerationContext<'_>>,
    ) -> Result<ToolResult, ServiceError> {
        let request = match parse_image_request(&call.arguments) {
            Ok(request) => request,
            Err(error) => {
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
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
        let Some(image_generation) = image_generation else {
            let output = tool_error(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider connection.",
            );
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Ok(ToolResult {
                call_id: call.id.clone(),
                output,
            });
        };

        self.emit_activity(
            storage,
            ToolActivity::running(call, None),
            request_id,
            snapshot,
            events,
        )
        .await?;
        if *cancellation.borrow() {
            let output = tool_error("operation_cancelled", "The request was stopped.");
            self.emit_activity(
                storage,
                ToolActivity::cancelled(call, String::new(), output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }

        let result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                service,
                connection_id,
                workspace_id,
                turn_id,
            } => {
                service
                    .generate_image(
                        &request,
                        ImageGenerationAuth::ChatGptOAuth {
                            connection_id,
                            workspace_id,
                        },
                        *turn_id,
                        cancellation,
                    )
                    .await
            }
            ImageGenerationContext::ApiKey { service, api_key } => {
                service
                    .generate_image(
                        &request,
                        ImageGenerationAuth::ApiKey(api_key),
                        None,
                        cancellation,
                    )
                    .await
            }
        };
        let result = match result {
            Ok(result) => result,
            Err(error) if is_image_cancellation(&error) => {
                self.emit_activity(
                    storage,
                    ToolActivity::cancelled(
                        call,
                        String::new(),
                        tool_error(error.code, &error.message),
                    ),
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
                    ToolActivity::finished(call, None, output.clone()),
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
                ToolActivity::cancelled(call, String::new(), output),
                request_id,
                snapshot,
                events,
            )
            .await?;
            return Err(crate::permissions::operation_cancelled_error());
        }

        let metadata = &result.metadata;
        let images = result
            .outputs
            .iter()
            .map(|image| (image.mime_type.as_str(), image.bytes.as_slice()))
            .collect::<Vec<_>>();
        let attachments = match crate::attachments::write_generated_images(
            storage.root(),
            &snapshot.conversation_id,
            &snapshot.message_id,
            &images,
        ) {
            Ok(attachments) => attachments,
            Err(error) => {
                let output = tool_error(error.code, &error.message);
                self.emit_activity(
                    storage,
                    ToolActivity::finished(call, None, output.clone()),
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
        if crate::attachments::append_assistant_attachments(
            storage,
            &snapshot.conversation_id,
            &snapshot.message_id,
            &attachments,
        )
        .is_err()
        {
            let cleanup_result = crate::attachments::delete_generated_images(
                storage.root(),
                &snapshot.conversation_id,
                &snapshot.message_id,
                &attachments,
            );
            let error = if cleanup_result.is_err() {
                image_storage_cleanup_error()
            } else {
                image_storage_persistence_error()
            };
            let output = tool_error(error.code, &error.message);
            self.emit_activity(
                storage,
                ToolActivity::finished(call, None, output.clone()),
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

        let mut output = json!({
            "attachments": attachments,
            "created": metadata.created_unix_seconds,
            "generation": {
                "background": metadata.background,
                "quality": metadata.quality,
                "size": metadata.size.as_deref(),
            },
        });
        if let Some(request_id) = metadata.imagegen_request_id.as_deref() {
            output["imageRequestId"] = Value::String(request_id.to_owned());
        }
        self.emit_activity(
            storage,
            ToolActivity::finished(call, None, output.clone()),
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

    fn prepare_activity(
        &self,
        mut activity: ToolActivity,
        snapshot: &ChatStreamSnapshot,
    ) -> ToolActivity {
        activity.round_id.clone_from(&self.current_round_id);
        activity.assistant_text_before_byte_offset = Some(snapshot.content.len());
        activity
    }

    async fn record_activity(&self, activity: &ToolActivity) -> Vec<ToolActivity> {
        let mut activities = self.tool_activities.lock().await;
        if let Some(existing) = activities
            .iter_mut()
            .find(|existing| existing.call_id == activity.call_id)
        {
            *existing = activity.clone();
        } else {
            activities.push(activity.clone());
        }
        activities.clone()
    }

    async fn emit_prepared_activity(
        &self,
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

fn capture_file_change_before(prepared: &PreparedToolCall) -> PendingFileChangeCapture {
    match &prepared.operation {
        ToolOperation::Write { .. } | ToolOperation::Edit { .. } => {
            PendingFileChangeCapture::File {
                path: prepared.relative_path.clone(),
                before: crate::file_changes::capture_file(&prepared.root, &prepared.relative_path),
            }
        }
        ToolOperation::Bash { .. } | ToolOperation::SendTerminalInput { .. } => {
            PendingFileChangeCapture::Workspace {
                before: crate::file_changes::capture_workspace(&prepared.root),
            }
        }
        _ => PendingFileChangeCapture::None,
    }
}

fn finish_file_change_capture(
    data_root: &Path,
    conversation_id: &str,
    prepared: &PreparedToolCall,
    capture: PendingFileChangeCapture,
) -> (Vec<crate::file_changes::FileChangeSummary>, Option<String>) {
    let deltas = match capture {
        PendingFileChangeCapture::None => return (Vec::new(), None),
        PendingFileChangeCapture::File { path, before } => {
            let Ok(before) = before else {
                return (Vec::new(), Some("snapshot_unavailable".to_owned()));
            };
            let after = match crate::file_changes::capture_file(&prepared.root, &path) {
                Ok(after) => after,
                Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
            };
            let before_hash = before.as_ref().map(|file| file.hash.as_str());
            let after_hash = after.as_ref().map(|file| file.hash.as_str());
            if before_hash == after_hash {
                Vec::new()
            } else {
                vec![crate::file_changes::FileChangeDelta {
                    path,
                    before,
                    after,
                }]
            }
        }
        PendingFileChangeCapture::Workspace { before } => {
            let Ok(before) = before else {
                return (
                    Vec::new(),
                    Some("workspace_snapshot_unavailable".to_owned()),
                );
            };
            let after = match crate::file_changes::capture_workspace(&prepared.root) {
                Ok(after) => after,
                Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
            };
            match crate::file_changes::workspace_deltas(&before, &after) {
                Ok(deltas) => deltas,
                Err(error) => return (Vec::new(), Some(tracking_error_code(error).to_owned())),
            }
        }
    };
    match crate::file_changes::record_deltas(data_root, conversation_id, &prepared.root, deltas) {
        Ok(changes) => (changes, None),
        Err(error) => (Vec::new(), Some(tracking_error_code(error).to_owned())),
    }
}

fn tracking_error_code(error: crate::file_changes::TrackingError) -> &'static str {
    match error {
        crate::file_changes::TrackingError::WorkspaceUnavailable => "workspace_unavailable",
        crate::file_changes::TrackingError::WorkspaceTooLarge => "workspace_too_large",
        crate::file_changes::TrackingError::SnapshotUnavailable => "snapshot_unavailable",
        crate::file_changes::TrackingError::StorageUnavailable => "storage_unavailable",
    }
}

#[cfg(test)]
mod policy_tests {
    use std::path::Path;

    use super::{ToolExecutor, ToolPermissionMode};

    #[test]
    fn rejects_calls_that_were_not_advertised_to_the_model() {
        let executor = ToolExecutor::with_allowed_tool_names(
            None,
            Path::new("."),
            ToolPermissionMode::FullAccess,
            ["read_file".to_owned()],
        );

        assert!(executor.validate_tool_name("read_file").is_ok());
        assert_eq!(
            executor
                .validate_tool_name("ask_user")
                .expect_err("unadvertised question tool must be rejected")
                .code,
            "invalid_provider_response"
        );
    }
}

fn question_arguments_error() -> ServiceError {
    ServiceError::new(
        "question_data_invalid",
        "The AI sent an invalid question. Ask it to try again.",
        false,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateImageArguments {
    prompt: String,
    model: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    count: Option<u8>,
}

fn parse_image_request(arguments: &Value) -> Result<ImageGenerationRequest, ServiceError> {
    let arguments = serde_json::from_value::<GenerateImageArguments>(arguments.clone())
        .map_err(|_| image_request_invalid())?;
    let quality = match arguments.quality.as_deref() {
        None => None,
        Some("low") => Some(ImageQuality::Low),
        Some("medium") => Some(ImageQuality::Medium),
        Some("high") => Some(ImageQuality::High),
        Some("xhigh") => Some(ImageQuality::XHigh),
        Some("max") => Some(ImageQuality::Max),
        Some("auto") => Some(ImageQuality::Auto),
        Some(_) => return Err(image_request_invalid()),
    };
    let background = match arguments.background.as_deref() {
        None => None,
        Some("transparent") => Some(crate::chatgpt::ImageBackground::Transparent),
        Some("opaque") => Some(crate::chatgpt::ImageBackground::Opaque),
        Some("auto") => Some(crate::chatgpt::ImageBackground::Auto),
        Some(_) => return Err(image_request_invalid()),
    };
    Ok(ImageGenerationRequest {
        prompt: arguments.prompt,
        model: arguments.model.unwrap_or_else(|| "gpt-image-2".to_owned()),
        size: arguments.size,
        quality,
        background,
        output_format: None,
        count: arguments.count.unwrap_or(1),
    })
}

fn image_request_invalid() -> ServiceError {
    ServiceError::new(
        "image_request_invalid",
        "The image generation request is invalid.",
        false,
    )
}

fn tool_activity_storage_error() -> ServiceError {
    ServiceError::new(
        "tool_activity_storage_failed",
        "The tool result could not be saved to this conversation.",
        true,
    )
}

fn image_storage_persistence_error() -> ServiceError {
    ServiceError::new(
        "image_storage_unavailable",
        "The generated image was received but could not be attached to the response.",
        true,
    )
}

fn image_storage_cleanup_error() -> ServiceError {
    ServiceError::new(
        "image_storage_unavailable",
        "The generated image could not be safely finalized locally.",
        true,
    )
}

fn is_image_cancellation(error: &ServiceError) -> bool {
    matches!(error.code, "operation_cancelled" | "request_cancelled")
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
