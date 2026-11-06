//! Image generation tool-call execution and request parsing.

use super::*;

impl ToolExecutor {
    pub(super) async fn execute_image_call(
        &self,
        call: &ToolCall,
        context: ToolEventContext<'_>,
        image_generation: Option<&ImageGenerationContext<'_>>,
    ) -> Result<ToolResult, ServiceError> {
        let ToolEventContext {
            request_id,
            snapshot,
            events,
            cancellation,
            storage,
        } = context;
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

        let run_result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                connection_id,
                workspace_id,
                ..
            } => crate::image_library::begin_generation(
                storage,
                &request,
                crate::image_library::ImageGenerationOrigin {
                    source_kind: "chat",
                    provider_route: "oauth",
                    connection_id: Some(connection_id),
                    workspace_id: Some(workspace_id),
                    conversation_id: Some(&snapshot.conversation_id),
                    message_id: Some(&snapshot.message_id),
                },
            ),
            ImageGenerationContext::ApiKey { .. } => crate::image_library::begin_generation(
                storage,
                &request,
                crate::image_library::ImageGenerationOrigin {
                    source_kind: "chat",
                    provider_route: "api_key",
                    connection_id: None,
                    workspace_id: None,
                    conversation_id: Some(&snapshot.conversation_id),
                    message_id: Some(&snapshot.message_id),
                },
            ),
            ImageGenerationContext::CompatibleApiKey { .. }
            | ImageGenerationContext::LocalEndpoint { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider route.",
                false,
            )),
        };
        let generation_id = match run_result {
            Ok(id) => id,
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

        let result = match image_generation {
            ImageGenerationContext::ChatGptOAuth {
                service,
                connection_id,
                workspace_id,
                turn_id,
                ..
            } => {
                service
                    .generate_image_with_account_quota(
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
            ImageGenerationContext::ApiKey {
                service, api_key, ..
            } => {
                service
                    .generate_image_with_account_quota(
                        &request,
                        ImageGenerationAuth::ApiKey(api_key),
                        None,
                        cancellation,
                    )
                    .await
            }
            ImageGenerationContext::CompatibleApiKey { .. }
            | ImageGenerationContext::LocalEndpoint { .. } => Err(ServiceError::new(
                "image_generation_unavailable",
                "Image generation is unavailable for this provider route.",
                false,
            )),
        };
        let generated = match result {
            Ok(result) => result,
            Err(error) if is_image_cancellation(&error) => {
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "cancelled",
                    Some(error.code),
                )?;
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
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "failed",
                    Some(error.code),
                )?;
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

        let result = generated.result;

        if *cancellation.borrow() {
            crate::image_library::finish_generation(
                storage,
                &generation_id,
                "cancelled",
                Some("operation_cancelled"),
            )?;
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
            storage.database_key(),
            &snapshot.conversation_id,
            &snapshot.message_id,
            &images,
        ) {
            Ok(attachments) => attachments,
            Err(error) => {
                crate::image_library::finish_generation(
                    storage,
                    &generation_id,
                    "failed",
                    Some(error.code),
                )?;
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
        if let Err(error) =
            crate::image_library::complete_chat_generation(storage, &generation_id, &attachments)
        {
            let cleanup_result = crate::attachments::delete_generated_images(
                storage.root(),
                &snapshot.conversation_id,
                &snapshot.message_id,
                &attachments,
            );
            let status_result = crate::image_library::finish_generation(
                storage,
                &generation_id,
                "failed",
                Some(error.code),
            );
            let error = match (cleanup_result, status_result) {
                (Err(cleanup_error), _) => cleanup_error,
                (_, Err(status_error)) => status_error,
                (Ok(()), Ok(())) => error,
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
            let revert_result = crate::image_library::revert_chat_generation(
                storage,
                &generation_id,
                "image_storage_persistence_failed",
            );
            let error = if cleanup_result.is_err() || revert_result.is_err() {
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
            "generationId": generation_id,
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
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateImageArguments {
    prompt: String,
    model: String,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    moderation: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
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
    let moderation = match arguments.moderation.as_deref() {
        None => None,
        Some("auto") => Some(crate::chatgpt::ImageModeration::Auto),
        Some("low") => Some(crate::chatgpt::ImageModeration::Low),
        Some(_) => return Err(image_request_invalid()),
    };
    let output_format = match arguments.output_format.as_deref() {
        None => None,
        Some("png") => Some(ImageOutputFormat::Png),
        Some("jpeg") => Some(ImageOutputFormat::Jpeg),
        Some("webp") => Some(ImageOutputFormat::Webp),
        Some(_) => return Err(image_request_invalid()),
    };
    Ok(ImageGenerationRequest {
        prompt: arguments.prompt,
        model: arguments.model,
        size: arguments.size,
        quality,
        background,
        moderation,
        output_format,
        output_compression: arguments.output_compression,
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
