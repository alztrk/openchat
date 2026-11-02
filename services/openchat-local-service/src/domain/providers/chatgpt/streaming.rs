//! Runs ChatGPT streaming turns, including permission-gated tool round trips.
//!
//! Provider events are translated to local conversation events, cancellation is
//! propagated, and tool execution remains constrained by the request allowlist.

use futures_util::StreamExt;
use hyper::Error as HyperError;
use reqwest::{
    Method, Response as HttpResponse, StatusCode,
    header::{CONTENT_ENCODING, CONTENT_LENGTH, TRANSFER_ENCODING},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    error::Error as StdError,
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::response_events::{
    AssistantMessageCheckpoint, ResponseEventContext, reasoning_summary_groups_value,
};
use super::response_parser::{
    attach_provider_citations, empty_response_shape, parse_responses_tool_calls,
    response_output_text,
};
use super::{
    CHATGPT_CODEX_BASE, ChatGptService, MAX_STREAM_EVENT_BYTES, StreamRequest,
    TemporaryResponseContext, http_error, invalid_response_error, network_error_with_details,
    now_unix_millis, protocol_error,
};
use crate::{
    chat_operation::ChatSendContext,
    chat_operation::TitleGenerationRequest,
    chatgpt_store::{self, AssistantMessageWrite, ChatGptModel},
    goals::{self, GoalDecision},
    protocol::{Response, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot},
    response_budget::ProviderResponseBudget,
    storage::ChatGptStreamDiagnostic,
    tools::{ImageGenerationContext, ToolExecutionContext, ToolExecutor},
    usage_statistics::{UsageData, UsageRequestDetails},
};

const MAX_TRANSIENT_RESPONSE_RETRIES: usize = 2;
const MAX_STREAM_CONTINUATIONS: usize = 2;
const TRANSIENT_RESPONSE_RETRY_DELAYS: [Duration; MAX_TRANSIENT_RESPONSE_RETRIES] =
    [Duration::from_millis(500), Duration::from_secs(1)];

fn is_retryable_response_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn is_retryable_stream_error(error: &ServiceError) -> bool {
    error.retryable
        && matches!(
            error.code,
            "network_unavailable"
                | "provider_timeout"
                | "response_incomplete"
                | "provider_unavailable"
        )
}

fn append_response_continuation(payload: &mut Value, content: &str) -> Result<(), ServiceError> {
    let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
        return Err(invalid_response_error());
    };
    input.push(json!({"role": "assistant", "content": content}));
    input.push(json!({
        "role": "user",
        "content": "The previous assistant response was interrupted. Continue the same response from its exact stopping point. Do not repeat or summarize any text already present; return only the continuation."
    }));
    Ok(())
}

fn response_content_encoding(response: &HttpResponse) -> &'static str {
    let Some(value) = response.headers().get(CONTENT_ENCODING) else {
        return "missing_or_auto_decoded";
    };
    let Ok(value) = value.to_str() else {
        return "invalid";
    };
    let value = value.trim();
    if value.contains(',') {
        return "multiple";
    }
    if value.eq_ignore_ascii_case("gzip") {
        "gzip"
    } else if value.eq_ignore_ascii_case("br") {
        "br"
    } else if value.eq_ignore_ascii_case("deflate") {
        "deflate"
    } else if value.eq_ignore_ascii_case("zstd") {
        "zstd"
    } else if value.eq_ignore_ascii_case("identity") {
        "identity"
    } else {
        "other"
    }
}

fn response_transfer_encoding(response: &HttpResponse) -> &'static str {
    let Some(value) = response.headers().get(TRANSFER_ENCODING) else {
        return "missing";
    };
    let Ok(value) = value.to_str() else {
        return "invalid";
    };
    let value = value.trim();
    if value.contains(',') {
        return "multiple";
    }
    if value.eq_ignore_ascii_case("chunked") {
        "chunked"
    } else {
        "other"
    }
}

fn response_content_length(response: &HttpResponse) -> Option<u64> {
    response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

fn response_http_version(response: &HttpResponse) -> &'static str {
    let version = response.version();
    if version == http::Version::HTTP_09 {
        "http/0.9"
    } else if version == http::Version::HTTP_10 {
        "http/1.0"
    } else if version == http::Version::HTTP_11 {
        "http/1.1"
    } else if version == http::Version::HTTP_2 {
        "http/2"
    } else if version == http::Version::HTTP_3 {
        "http/3"
    } else {
        "other"
    }
}

fn reqwest_stream_error_class(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_decode() {
        "decode"
    } else if error.is_body() {
        "body"
    } else if error.is_request() {
        "request"
    } else if error.is_status() {
        "status"
    } else {
        "other"
    }
}

fn stream_error_source_details(
    error: &reqwest::Error,
) -> (Option<&'static str>, Option<&'static str>) {
    let mut source = StdError::source(error);
    while let Some(source_error) = source {
        if let Some(hyper_error) = source_error.downcast_ref::<HyperError>() {
            if hyper_error.is_incomplete_message() {
                return (Some("incomplete_http_message"), None);
            }
            if hyper_error.is_closed() {
                return (Some("http_body_channel_closed"), None);
            }
            if hyper_error.is_canceled() {
                return (Some("http_body_cancelled"), None);
            }
            if hyper_error.is_timeout() {
                return (Some("http_body_timeout"), None);
            }
            if hyper_error.is_parse() {
                return (Some("http_protocol_parse"), None);
            }
        }
        if let Some(io_error) = source_error.downcast_ref::<io::Error>() {
            return (Some("io_error"), Some(io_error_kind_label(io_error.kind())));
        }
        source = StdError::source(source_error);
    }
    (None, None)
}

fn io_error_kind_label(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::ConnectionReset => "connection_reset",
        io::ErrorKind::ConnectionAborted => "connection_aborted",
        io::ErrorKind::UnexpectedEof => "unexpected_eof",
        io::ErrorKind::TimedOut => "timed_out",
        io::ErrorKind::NotConnected => "not_connected",
        io::ErrorKind::BrokenPipe => "broken_pipe",
        io::ErrorKind::InvalidData => "invalid_data",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::WouldBlock => "would_block",
        io::ErrorKind::Other => "other",
        _ => "other",
    }
}

fn response_tool_names(payload: &Value) -> Vec<String> {
    payload
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

pub(super) struct StreamResponseRequest<'a> {
    pub(super) context: ChatSendContext<'a>,
    pub(super) route: &'a chatgpt_store::ConversationRoute,
    pub(super) external_workspace_id: &'a str,
    pub(super) model: &'a ChatGptModel,
    pub(super) last_prompt_message_id: Option<&'a str>,
    pub(super) tool_definitions: Vec<crate::provider_schema::ToolDefinition>,
    pub(super) payload: Value,
}

impl ChatGptService {
    pub(super) async fn stream_response(
        self: &Arc<Self>,
        request: StreamResponseRequest<'_>,
    ) -> Result<Value, ServiceError> {
        let StreamResponseRequest {
            context,
            route,
            external_workspace_id,
            model,
            last_prompt_message_id,
            tool_definitions,
            mut payload,
        } = request;
        let ChatSendContext {
            request_id,
            conversation_id,
            excluded_assistant_message_id,
            custom_instructions,
            project_instructions,
            project_memory_context,
            project_skills,
            mut project_history_context,
            project_root,
            workspace_roots,
            data_root,
            storage,
            permission_mode,
            is_security_scan,
            security_scan_public_web_research,
            tool_permission_rules,
            available_subagent_models,
            #[cfg(windows)]
                mcp_configs: _,
            #[cfg(windows)]
            mcp_registry,
            permission_broker,
            user_question_broker,
            run_id,
            mut goal,
            cancellation,
            events,
            title_generation,
            ..
        } = context;
        let message_id = Uuid::new_v4().simple().to_string();
        let stream_id = Uuid::new_v4().simple().to_string();
        let created_at = now_unix_millis()?;
        let started = Instant::now();
        let mut content = String::new();
        let mut message_checkpoint = AssistantMessageCheckpoint::default();
        let mut reasoning_summaries = Vec::new();
        self.record_chatgpt_event("request_started", "responses", None, None, None, None);
        self.retry_storage_operation("send_initial_assistant_message_save", || {
            chatgpt_store::save_assistant_message(
                &self.storage,
                AssistantMessageWrite {
                    conversation_id,
                    message_id: &message_id,
                    content: &content,
                    status: "streaming",
                    created_at_unix_ms: created_at,
                    output_tokens: None,
                    elapsed: None,
                },
            )
        })
        .await?;

        events
            .send(
                &ChatStreamEvent::Started(ChatStreamSnapshot {
                    conversation_id: conversation_id.to_owned(),
                    message_id: message_id.clone(),
                    content: content.clone(),
                    created_at_unix_ms: created_at,
                })
                .into_rpc(request_id.clone()),
            )
            .await
            .map_err(|_| protocol_error())?;

        if *cancellation.borrow() {
            self.record_chatgpt_event(
                "request_cancelled",
                "responses",
                None,
                Some("request_cancelled"),
                Some(started.elapsed().as_millis()),
                None,
            );
            self.persist_terminal_message(AssistantMessageWrite {
                conversation_id,
                message_id: &message_id,
                content: &content,
                status: "stopped",
                created_at_unix_ms: created_at,
                output_tokens: None,
                elapsed: Some(started.elapsed()),
            })
            .await?;
            return Ok(json!({
                "conversationId": conversation_id,
                "messageId": message_id,
                "status": "stopped",
                "outputTokens": Value::Null,
                "tokensPerSecond": Value::Null,
                "elapsedMicroseconds": started.elapsed().as_micros(),
            }));
        }

        let mut output_tokens = None;
        let mut input_tokens;
        let mut tool_executor = ToolExecutor::with_permission_rules(
            project_root,
            data_root,
            permission_mode,
            response_tool_names(&payload),
            tool_permission_rules,
        )
        .with_workspace_roots(workspace_roots)
        .with_security_scan_scope(is_security_scan, security_scan_public_web_research)
        .with_project_memory_context(project_memory_context.map(str::to_owned))
        .with_chat_service(self.as_ref().clone())
        .with_subagent_models(available_subagent_models)
        .with_subagent_tool_definitions(tool_definitions);
        #[cfg(windows)]
        {
            tool_executor = tool_executor.with_mcp_registry(mcp_registry);
        }
        let citation_sources;
        let image_generation = Some(ImageGenerationContext::ChatGptOAuth {
            service: self,
            connection_id: &route.connection_id,
            workspace_id: &route.workspace_id,
            external_workspace_id,
            model_id: &model.id,
            reasoning_effort: payload
                .pointer("/reasoning/effort")
                .and_then(Value::as_str)
                .map(str::to_owned),
            fast_mode: payload.get("service_tier").and_then(Value::as_str)
                == Some(super::CHATGPT_FAST_SERVICE_TIER),
            turn_id: Some(&message_id),
        });
        let initial_input_len = payload
            .get("input")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let mut operation = "chat";
        let mut response_budget = ProviderResponseBudget::default();
        let mut initial_provider_request = true;
        let mut stream_continuations = 0;
        let mut provider_request_attempts: usize = 0;
        let mut stream_started_diagnostic =
            ChatGptStreamDiagnostic::new("stream_started", &stream_id);
        stream_started_diagnostic.partial_text_available = Some(false);
        self.record_chatgpt_stream_diagnostic(stream_started_diagnostic);
        'model_turn: loop {
            let is_stream_continuation = operation == "stream_continuation";
            let is_follow_up_turn = !initial_provider_request;
            if is_follow_up_turn
                && !is_stream_continuation
                && let Some(history_context) = project_history_context.take()
            {
                let removed = match payload.get_mut("instructions") {
                    Some(Value::String(instructions)) => {
                        crate::instructions::remove_project_history_context(
                            instructions,
                            project_memory_context,
                            &history_context,
                            model.context_window,
                        )
                    }
                    _ => false,
                };
                if !removed {
                    let elapsed = started.elapsed();
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(elapsed),
                    })
                    .await?;
                    return Err(ServiceError::new(
                        "project_history_context_cleanup_failed",
                        "The project history context could not be removed before a follow-up request.",
                        false,
                    ));
                }
            }
            initial_provider_request = false;
            if is_follow_up_turn
                && crate::context_compaction::validate_request_context(
                    &payload,
                    model.context_window,
                    &model.id,
                )
                .is_err()
            {
                compact_in_loop_responses_payload(
                    &mut payload,
                    goal.as_ref(),
                    initial_input_len,
                    model.context_window,
                    &model.id,
                );
            }
            if let Err(error) = crate::context_compaction::validate_request_context(
                &payload,
                model.context_window,
                &model.id,
            ) {
                let elapsed = started.elapsed();
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(elapsed.as_millis()),
                    None,
                );
                self.persist_terminal_message(AssistantMessageWrite {
                    conversation_id,
                    message_id: &message_id,
                    content: &content,
                    status: "failed",
                    created_at_unix_ms: created_at,
                    output_tokens,
                    elapsed: Some(elapsed),
                })
                .await?;
                return Err(error);
            }
            let mut round_output_tokens = None;
            let mut round_input_tokens = None;
            let mut round_usage = UsageData::default();
            let mut response_output_items = Vec::new();
            let mut usage_request = self
                .start_usage_request(
                    "send_usage_request_start",
                    UsageRequestDetails {
                        conversation_id,
                        assistant_message_id: Some(&message_id),
                        provider_id: "chatgpt",
                        model_id: &model.id,
                        reasoning_effort: payload
                            .pointer("/reasoning/effort")
                            .and_then(Value::as_str),
                        operation,
                        fast_requested: payload
                            .get("service_tier")
                            .and_then(Value::as_str)
                            .is_some_and(|tier| tier == "priority"),
                        run_id: Some(&run_id),
                        connection_id: Some(&route.connection_id),
                        workspace_id: Some(&route.workspace_id),
                        project_id: None,
                        parent_run_id: None,
                    },
                )
                .await?;
            let request_sources = crate::usage_statistics::RequestDataSources {
                instruction_sources: crate::usage_statistics::instruction_source_categories(
                    custom_instructions,
                    project_instructions,
                    &project_skills,
                    project_memory_context,
                    project_history_context.as_deref(),
                    goal.is_some(),
                ),
                ..Default::default()
            };
            self.retry_storage_operation("send_usage_request_manifest", || {
                usage_request.record_request_manifest(&payload, Some(&request_sources))
            })
            .await?;
            let provider_request_started = Instant::now();
            let mut retries = 0;
            let response_result = loop {
                provider_request_attempts = provider_request_attempts.saturating_add(1);
                let attempt = provider_request_attempts;
                let attempt_started = Instant::now();
                let response_result = self
                    .authorized_stream_request(
                        StreamRequest {
                            method: Method::POST,
                            url: format!("{CHATGPT_CODEX_BASE}/responses"),
                            connection_id: &route.connection_id,
                            external_workspace_id,
                            body: Some(payload.clone()),
                            response_context_id: conversation_id,
                        },
                        cancellation,
                    )
                    .await;
                let attempt_elapsed_ms = attempt_started.elapsed().as_millis();
                match response_result {
                    Ok(Some(response)) => {
                        let status = response.status();
                        let mut diagnostic =
                            ChatGptStreamDiagnostic::new("response_headers_received", &stream_id);
                        diagnostic.attempt = Some(attempt);
                        diagnostic.status = Some(status.as_u16());
                        diagnostic.http_version = Some(response_http_version(&response));
                        diagnostic.content_encoding = Some(response_content_encoding(&response));
                        diagnostic.transfer_encoding = Some(response_transfer_encoding(&response));
                        diagnostic.content_length_header_bytes = response_content_length(&response);
                        diagnostic.elapsed_ms = Some(attempt_elapsed_ms);
                        self.record_chatgpt_stream_diagnostic(diagnostic);

                        if retries < MAX_TRANSIENT_RESPONSE_RETRIES
                            && is_retryable_response_status(status)
                        {
                            retries += 1;
                            let delay = TRANSIENT_RESPONSE_RETRY_DELAYS[retries - 1];
                            let mut diagnostic =
                                ChatGptStreamDiagnostic::new("request_retry_scheduled", &stream_id);
                            diagnostic.attempt = Some(attempt);
                            diagnostic.retry_count = Some(retries);
                            diagnostic.retry_delay_ms = Some(delay.as_millis());
                            diagnostic.status = Some(status.as_u16());
                            diagnostic.elapsed_ms =
                                Some(provider_request_started.elapsed().as_millis());
                            diagnostic.error_code = Some("provider_unavailable");
                            diagnostic.retryable = Some(true);
                            self.record_chatgpt_stream_diagnostic(diagnostic);
                            self.record_chatgpt_event(
                                "retry_scheduled",
                                "responses",
                                Some(status.as_u16()),
                                Some("provider_unavailable"),
                                Some(provider_request_started.elapsed().as_millis()),
                                Some(retries),
                            );
                            drop(response);
                            if *cancellation.borrow() {
                                break Ok(None);
                            }
                            tokio::select! {
                                changed = cancellation.changed() => {
                                    let _ = changed;
                                    break Ok(None);
                                }
                                () = tokio::time::sleep(delay) => {}
                            }
                            continue;
                        }
                        break Ok(Some(response));
                    }
                    Err(error) => {
                        let retryable = is_retryable_stream_error(&error);
                        let mut diagnostic = ChatGptStreamDiagnostic::new(
                            "request_failed_before_headers",
                            &stream_id,
                        );
                        diagnostic.attempt = Some(attempt);
                        diagnostic.elapsed_ms = Some(attempt_elapsed_ms);
                        diagnostic.error_code = Some(error.code);
                        diagnostic.retryable = Some(retryable);
                        self.record_chatgpt_stream_diagnostic(diagnostic);

                        if retries < MAX_TRANSIENT_RESPONSE_RETRIES && retryable {
                            retries += 1;
                            let delay = TRANSIENT_RESPONSE_RETRY_DELAYS[retries - 1];
                            let mut diagnostic =
                                ChatGptStreamDiagnostic::new("request_retry_scheduled", &stream_id);
                            diagnostic.attempt = Some(attempt);
                            diagnostic.retry_count = Some(retries);
                            diagnostic.retry_delay_ms = Some(delay.as_millis());
                            diagnostic.elapsed_ms =
                                Some(provider_request_started.elapsed().as_millis());
                            diagnostic.error_code = Some(error.code);
                            diagnostic.retryable = Some(true);
                            self.record_chatgpt_stream_diagnostic(diagnostic);
                            self.record_chatgpt_event(
                                "retry_scheduled",
                                "responses",
                                None,
                                Some(error.code),
                                Some(provider_request_started.elapsed().as_millis()),
                                Some(retries),
                            );
                            tokio::select! {
                                changed = cancellation.changed() => {
                                    let _ = changed;
                                    break Ok(None);
                                }
                                () = tokio::time::sleep(delay) => {}
                            }
                            continue;
                        }
                        break Err(error);
                    }
                    Ok(None) => break Ok(None),
                }
            };
            let response = match response_result {
                Ok(Some(response)) => {
                    let status = response.status();
                    self.record_chatgpt_event(
                        "http_response",
                        "responses",
                        Some(status.as_u16()),
                        None,
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    if status.is_success() {
                        response
                    } else {
                        let error = http_error(status);
                        self.record_chatgpt_event(
                            "request_failed",
                            "responses",
                            Some(status.as_u16()),
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        self.persist_terminal_message(AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "failed",
                            created_at_unix_ms: created_at,
                            output_tokens: None,
                            elapsed: Some(started.elapsed()),
                        })
                        .await?;
                        return Err(error);
                    }
                }
                Ok(None) => {
                    self.retry_storage_operation("send_usage_request_cancel", || {
                        usage_request.cancel()
                    })
                    .await?;
                    let elapsed = started.elapsed();
                    self.record_chatgpt_event(
                        "request_cancelled",
                        "responses",
                        None,
                        Some("request_cancelled"),
                        Some(elapsed.as_millis()),
                        None,
                    );
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "stopped",
                        created_at_unix_ms: created_at,
                        output_tokens: None,
                        elapsed: Some(elapsed),
                    })
                    .await?;
                    return Ok(json!({
                        "conversationId": conversation_id,
                        "messageId": message_id,
                        "status": "stopped",
                        "outputTokens": Value::Null,
                        "tokensPerSecond": Value::Null,
                        "elapsedMicroseconds": elapsed.as_micros(),
                    }));
                }
                Err(error) => {
                    self.record_chatgpt_event(
                        "request_failed",
                        "responses",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens: None,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(error);
                }
            };

            let response_status = response.status().as_u16();
            let body_started = Instant::now();
            let mut last_chunk_at = body_started;
            let mut first_chunk_ms = None;
            let mut body_chunks_received = 0_usize;
            let mut decoded_body_bytes_received = 0_usize;
            let mut body_stream = response.bytes_stream();
            let mut pending_bytes = Vec::new();
            let mut event_data = Vec::<String>::new();
            let mut completed = false;
            let mut failure = None;

            loop {
                let next = tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            self.retry_storage_operation("send_usage_request_cancel", || {
                                usage_request.cancel()
                            })
                            .await?;
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(started.elapsed().as_millis()),
                                None,
                            );
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "stopped",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Ok(json!({
                                "conversationId": conversation_id,
                                "messageId": message_id,
                                "status": "stopped",
                                "outputTokens": output_tokens,
                                "tokensPerSecond": output_tokens.and_then(|tokens| {
                                    let seconds = started.elapsed().as_secs_f64();
                                    (seconds > 0.0).then_some(tokens as f64 / seconds)
                                }),
                                "elapsedMicroseconds": started.elapsed().as_micros(),
                            }));
                        }
                        continue;
                    }
                    chunk = body_stream.next() => chunk,
                };

                let Some(chunk) = next else { break };
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        let (source_error_class, source_io_kind) =
                            stream_error_source_details(&error);
                        let mut diagnostic =
                            ChatGptStreamDiagnostic::new("response_body_read_failed", &stream_id);
                        diagnostic.attempt = Some(provider_request_attempts);
                        diagnostic.status = Some(response_status);
                        diagnostic.elapsed_ms = Some(body_started.elapsed().as_millis());
                        diagnostic.body_chunks_received = Some(body_chunks_received);
                        diagnostic.decoded_body_bytes_received = Some(decoded_body_bytes_received);
                        diagnostic.first_chunk_ms = first_chunk_ms;
                        diagnostic.last_chunk_idle_ms = Some(last_chunk_at.elapsed().as_millis());
                        diagnostic.error_code = Some("network_unavailable");
                        diagnostic.reqwest_error_class = Some(reqwest_stream_error_class(&error));
                        diagnostic.source_error_class = source_error_class;
                        diagnostic.source_io_kind = source_io_kind;
                        diagnostic.retryable = Some(true);
                        diagnostic.completion_event_seen = Some(false);
                        diagnostic.partial_text_available = Some(!content.is_empty());
                        self.record_chatgpt_stream_diagnostic(diagnostic);
                        failure = Some(network_error_with_details(error));
                        break;
                    }
                };
                body_chunks_received = body_chunks_received.saturating_add(1);
                decoded_body_bytes_received =
                    decoded_body_bytes_received.saturating_add(chunk.len());
                if first_chunk_ms.is_none() {
                    first_chunk_ms = Some(body_started.elapsed().as_millis());
                }
                last_chunk_at = Instant::now();
                if let Err(error) = response_budget.consume(chunk.len()) {
                    self.record_chatgpt_event(
                        "request_failed",
                        "responses",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(error);
                }
                pending_bytes.extend_from_slice(&chunk);
                if pending_bytes.len() > MAX_STREAM_EVENT_BYTES {
                    self.record_chatgpt_event(
                        "request_failed",
                        "responses",
                        None,
                        Some("response_event_too_large"),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(ServiceError::new(
                        "response_event_too_large",
                        "ChatGPT returned an oversized stream event. The partial answer was saved.",
                        false,
                    ));
                }
                while let Some(line_end) = pending_bytes.iter().position(|byte| *byte == b'\n') {
                    let mut line = pending_bytes.drain(..=line_end).collect::<Vec<_>>();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    if line.is_empty() {
                        if event_data.is_empty() {
                            continue;
                        }
                        let event = event_data.join("\n");
                        event_data.clear();
                        if response_event_has_output(&event) {
                            usage_request.record_first_output(provider_request_started.elapsed());
                        }
                        match self
                            .process_response_event(
                                &event,
                                ResponseEventContext {
                                    content: &mut content,
                                    output_tokens: &mut round_output_tokens,
                                    input_tokens: &mut round_input_tokens,
                                    usage: &mut round_usage,
                                    reasoning_summaries: &mut reasoning_summaries,
                                    response_output_items: &mut response_output_items,
                                    conversation_id,
                                    message_id: &message_id,
                                    created_at,
                                    request_id: &request_id,
                                    events: &events,
                                    persist_message: true,
                                    checkpoint: Some(&mut message_checkpoint),
                                },
                            )
                            .await
                        {
                            Ok(true) => completed = true,
                            Ok(false) => {}
                            Err(error) => {
                                failure = Some(error);
                                break;
                            }
                        }
                        if completed || failure.is_some() {
                            break;
                        }
                    } else if let Some(data) = line.strip_prefix(b"data:") {
                        let data = data.strip_prefix(b" ").unwrap_or(data);
                        match String::from_utf8(data.to_vec()) {
                            Ok(data) => event_data.push(data),
                            Err(_) => {
                                let error = ServiceError::new(
                                    "invalid_provider_response",
                                    "ChatGPT returned a malformed stream event.",
                                    false,
                                );
                                self.record_chatgpt_event(
                                    "request_failed",
                                    "responses",
                                    None,
                                    Some(error.code),
                                    Some(started.elapsed().as_millis()),
                                    None,
                                );
                                self.persist_terminal_message(AssistantMessageWrite {
                                    conversation_id,
                                    message_id: &message_id,
                                    content: &content,
                                    status: "failed",
                                    created_at_unix_ms: created_at,
                                    output_tokens,
                                    elapsed: Some(started.elapsed()),
                                })
                                .await?;
                                return Err(error);
                            }
                        }
                    }
                }
                if completed || failure.is_some() {
                    break;
                }
            }

            if failure.is_none() {
                let mut diagnostic = ChatGptStreamDiagnostic::new(
                    if completed {
                        "response_body_completed"
                    } else {
                        "response_body_ended_without_completion"
                    },
                    &stream_id,
                );
                diagnostic.attempt = Some(provider_request_attempts);
                diagnostic.status = Some(response_status);
                diagnostic.elapsed_ms = Some(body_started.elapsed().as_millis());
                diagnostic.body_chunks_received = Some(body_chunks_received);
                diagnostic.decoded_body_bytes_received = Some(decoded_body_bytes_received);
                diagnostic.first_chunk_ms = first_chunk_ms;
                diagnostic.last_chunk_idle_ms = Some(last_chunk_at.elapsed().as_millis());
                diagnostic.error_code = (!completed).then_some("response_incomplete");
                diagnostic.retryable = Some(!completed);
                diagnostic.completion_event_seen = Some(completed);
                diagnostic.partial_text_available = Some(!content.is_empty());
                self.record_chatgpt_stream_diagnostic(diagnostic);
            }

            let stream_error = failure.or_else(|| {
                (!completed).then(|| {
                    ServiceError::new(
                        "response_incomplete",
                        "The ChatGPT response ended before completion. The partial answer was saved.",
                        true,
                    )
                })
            });
            if let Some(error) = stream_error {
                self.retry_storage_operation("send_usage_request_fail", || {
                    usage_request.fail(&round_usage)
                })
                .await?;
                if stream_continuations < MAX_STREAM_CONTINUATIONS
                    && is_retryable_stream_error(&error)
                {
                    stream_continuations += 1;
                    let mut diagnostic =
                        ChatGptStreamDiagnostic::new("stream_continuation_scheduled", &stream_id);
                    diagnostic.attempt = Some(provider_request_attempts);
                    diagnostic.continuation_count = Some(stream_continuations);
                    diagnostic.retry_delay_ms =
                        Some(TRANSIENT_RESPONSE_RETRY_DELAYS[stream_continuations - 1].as_millis());
                    diagnostic.elapsed_ms = Some(started.elapsed().as_millis());
                    diagnostic.error_code = Some(error.code);
                    diagnostic.retryable = Some(true);
                    diagnostic.partial_text_available = Some(!content.is_empty());
                    self.record_chatgpt_stream_diagnostic(diagnostic);
                    self.record_chatgpt_event(
                        "continuation_scheduled",
                        "responses",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        Some(stream_continuations),
                    );
                    if !content.is_empty() {
                        if let Err(continuation_error) =
                            append_response_continuation(&mut payload, &content)
                        {
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "failed",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Err(continuation_error);
                        }
                        self.retry_storage_operation("send_continuation_message_save", || {
                            chatgpt_store::save_assistant_message(
                                &self.storage,
                                AssistantMessageWrite {
                                    conversation_id,
                                    message_id: &message_id,
                                    content: &content,
                                    status: "streaming",
                                    created_at_unix_ms: created_at,
                                    output_tokens: None,
                                    elapsed: None,
                                },
                            )
                        })
                        .await?;
                        events
                            .send(
                                &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                                    conversation_id,
                                    &message_id,
                                    &content,
                                    created_at,
                                ))
                                .into_rpc(request_id.clone()),
                            )
                            .await
                            .map_err(|_| protocol_error())?;
                    }
                    operation = "stream_continuation";
                    let delay = TRANSIENT_RESPONSE_RETRY_DELAYS[stream_continuations - 1];
                    let cancelled = tokio::select! {
                        changed = cancellation.changed() => {
                            let _ = changed;
                            true
                        }
                        () = tokio::time::sleep(delay) => false,
                    };
                    if cancelled || *cancellation.borrow() {
                        self.record_chatgpt_event(
                            "request_cancelled",
                            "responses",
                            None,
                            Some("request_cancelled"),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        self.persist_terminal_message(AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "stopped",
                            created_at_unix_ms: created_at,
                            output_tokens,
                            elapsed: Some(started.elapsed()),
                        })
                        .await?;
                        return Ok(json!({
                            "conversationId": conversation_id,
                            "messageId": message_id,
                            "status": "stopped",
                            "outputTokens": output_tokens,
                            "elapsedMicroseconds": started.elapsed().as_micros(),
                        }));
                    }
                    continue 'model_turn;
                }
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                let mut diagnostic = ChatGptStreamDiagnostic::new("stream_failed", &stream_id);
                diagnostic.attempt = Some(provider_request_attempts);
                diagnostic.continuation_count = Some(stream_continuations);
                diagnostic.elapsed_ms = Some(started.elapsed().as_millis());
                diagnostic.error_code = Some(error.code);
                diagnostic.retryable = Some(error.retryable);
                diagnostic.partial_text_available = Some(!content.is_empty());
                self.record_chatgpt_stream_diagnostic(diagnostic);
                self.persist_terminal_message(AssistantMessageWrite {
                    conversation_id,
                    message_id: &message_id,
                    content: &content,
                    status: "failed",
                    created_at_unix_ms: created_at,
                    output_tokens,
                    elapsed: Some(started.elapsed()),
                })
                .await?;
                return Err(error);
            }

            self.retry_storage_operation("send_usage_request_complete", || {
                usage_request.complete(&round_usage)
            })
            .await?;

            if let Some(round_tokens) = round_output_tokens {
                output_tokens = Some(output_tokens.unwrap_or(0i64).saturating_add(round_tokens));
            }
            input_tokens = round_input_tokens;
            let final_text = response_output_text(&response_output_items);
            if !final_text.is_empty() && final_text != content.as_str() {
                content = final_text;
                self.retry_storage_operation("send_final_text_snapshot_save", || {
                    chatgpt_store::save_assistant_message(
                        &self.storage,
                        AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "streaming",
                            created_at_unix_ms: created_at,
                            output_tokens: None,
                            elapsed: None,
                        },
                    )
                })
                .await?;
                events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            conversation_id,
                            &message_id,
                            &content,
                            created_at,
                        ))
                        .into_rpc(request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
            }
            let tool_calls = match parse_responses_tool_calls(&response_output_items) {
                Ok(tool_calls) => tool_calls,
                Err(error) => {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(error);
                }
            };
            if tool_calls.is_empty()
                && goal
                    .as_ref()
                    .is_some_and(|goal| matches!(&goal.decision, GoalDecision::Continue))
            {
                let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(invalid_response_error());
                };
                input.push(goals::responses_continuation_input());
                tool_executor.reset_turn_state();
                operation = "chat";
                continue 'model_turn;
            }
            if !tool_calls.is_empty()
                && goal.is_none()
                && goals::split_control_calls(&tool_calls).1.is_empty()
            {
                tool_executor.begin_round(&tool_calls);
                let mut results = Vec::with_capacity(tool_calls.len());
                for call in &tool_calls {
                    let snapshot =
                        ChatStreamSnapshot::new(conversation_id, &message_id, &content, created_at);
                    let result = match tool_executor
                        .execute_call_with_image_context(
                            call,
                            ToolExecutionContext {
                                permissions: permission_broker,
                                request_id: &request_id,
                                snapshot: &snapshot,
                                events: &events,
                                cancellation,
                                storage,
                                run_id: &run_id,
                                provider_id: "chatgpt",
                                user_questions: user_question_broker,
                                image_generation: image_generation.as_ref(),
                            },
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) if error.code == "operation_cancelled" => {
                            let elapsed = started.elapsed();
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(elapsed.as_millis()),
                                None,
                            );
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "stopped",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(elapsed),
                            })
                            .await?;
                            return Ok(json!({
                                "conversationId": conversation_id,
                                "messageId": message_id,
                                "status": "stopped",
                                "outputTokens": output_tokens,
                                "tokensPerSecond": output_tokens.and_then(|tokens| {
                                    let seconds = elapsed.as_secs_f64();
                                    (seconds > 0.0).then_some(tokens as f64 / seconds)
                                }),
                                "elapsedMicroseconds": elapsed.as_micros(),
                                "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
                            }));
                        }
                        Err(error) => {
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "failed",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Err(error);
                        }
                    };
                    results.push(result);
                }
                let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(invalid_response_error());
                };
                input.extend(response_output_items);
                for result in results {
                    let output = match crate::attachments::function_call_output_value(
                        storage.root(),
                        conversation_id,
                        &message_id,
                        &result.output,
                    ) {
                        Ok(Some(output)) => output,
                        Ok(None) => match serde_json::to_string(&result.output) {
                            Ok(output) => Value::String(output),
                            Err(_) => {
                                self.persist_terminal_message(AssistantMessageWrite {
                                    conversation_id,
                                    message_id: &message_id,
                                    content: &content,
                                    status: "failed",
                                    created_at_unix_ms: created_at,
                                    output_tokens,
                                    elapsed: Some(started.elapsed()),
                                })
                                .await?;
                                return Err(invalid_response_error());
                            }
                        },
                        Err(error) => {
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "failed",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Err(error);
                        }
                    };
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": result.call_id,
                        "output": output
                    }));
                }
                operation = "tool_follow_up";
                continue 'model_turn;
            }
            if !tool_calls.is_empty() {
                if goal
                    .as_ref()
                    .is_some_and(|goal| !matches!(&goal.decision, GoalDecision::Continue))
                {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(ServiceError::new(
                        "invalid_provider_response",
                        "The provider continued acting after closing the goal.",
                        false,
                    ));
                }
                let (regular_calls, control_calls) = goals::split_control_calls(&tool_calls);
                if let Err(error) =
                    goals::validate_control_call_mix(&control_calls, !regular_calls.is_empty())
                {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(error);
                }
                tool_executor.begin_round(&regular_calls);
                let starting_goal = goal.is_none()
                    && control_calls
                        .iter()
                        .any(|call| call.name == goals::START_GOAL_TOOL_NAME);
                let control_results = match goals::process_control_calls(
                    &control_calls,
                    &mut goal,
                    goals::ControlCallContext {
                        storage,
                        run_id: &run_id,
                        conversation_id,
                        request_id: &request_id,
                        events: &events,
                        is_security_scan,
                    },
                )
                .await
                {
                    Ok(results) => results,
                    Err(error) => {
                        self.persist_terminal_message(AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "failed",
                            created_at_unix_ms: created_at,
                            output_tokens,
                            elapsed: Some(started.elapsed()),
                        })
                        .await?;
                        return Err(error);
                    }
                };
                if starting_goal
                    && let Some(started_goal) = goal.as_ref()
                    && let Err(error) = goals::append_goal_instructions_to_responses_payload(
                        &mut payload,
                        &started_goal.objective,
                    )
                {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(error);
                }
                let mut results_by_id = control_results
                    .into_iter()
                    .map(|result| (result.call_id, result.output))
                    .collect::<HashMap<_, _>>();
                for call in &regular_calls {
                    let snapshot =
                        ChatStreamSnapshot::new(conversation_id, &message_id, &content, created_at);
                    let result = match tool_executor
                        .execute_call_with_image_context(
                            call,
                            ToolExecutionContext {
                                permissions: permission_broker,
                                request_id: &request_id,
                                snapshot: &snapshot,
                                events: &events,
                                cancellation,
                                storage,
                                run_id: &run_id,
                                provider_id: "chatgpt",
                                user_questions: user_question_broker,
                                image_generation: image_generation.as_ref(),
                            },
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) if error.code == "operation_cancelled" => {
                            let elapsed = started.elapsed();
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(elapsed.as_millis()),
                                None,
                            );
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "stopped",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(elapsed),
                            })
                            .await?;
                            return Ok(json!({
                                "conversationId": conversation_id,
                                "messageId": message_id,
                                "status": "stopped",
                                "outputTokens": output_tokens,
                                "tokensPerSecond": output_tokens.and_then(|tokens| {
                                    let seconds = elapsed.as_secs_f64();
                                    (seconds > 0.0).then_some(tokens as f64 / seconds)
                                }),
                                "elapsedMicroseconds": elapsed.as_micros(),
                                "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
                            }));
                        }
                        Err(error) => {
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "failed",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Err(error);
                        }
                    };
                    results_by_id.insert(result.call_id, result.output);
                }
                let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
                    self.persist_terminal_message(AssistantMessageWrite {
                        conversation_id,
                        message_id: &message_id,
                        content: &content,
                        status: "failed",
                        created_at_unix_ms: created_at,
                        output_tokens,
                        elapsed: Some(started.elapsed()),
                    })
                    .await?;
                    return Err(invalid_response_error());
                };
                input.extend(response_output_items);
                for call in &tool_calls {
                    let Some(result) = results_by_id.remove(&call.id) else {
                        self.persist_terminal_message(AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "failed",
                            created_at_unix_ms: created_at,
                            output_tokens,
                            elapsed: Some(started.elapsed()),
                        })
                        .await?;
                        return Err(invalid_response_error());
                    };
                    let output = match crate::attachments::function_call_output_value(
                        storage.root(),
                        conversation_id,
                        &message_id,
                        &result,
                    ) {
                        Ok(Some(output)) => output,
                        Ok(None) => match serde_json::to_string(&result) {
                            Ok(output) => Value::String(output),
                            Err(_) => {
                                self.persist_terminal_message(AssistantMessageWrite {
                                    conversation_id,
                                    message_id: &message_id,
                                    content: &content,
                                    status: "failed",
                                    created_at_unix_ms: created_at,
                                    output_tokens,
                                    elapsed: Some(started.elapsed()),
                                })
                                .await?;
                                return Err(invalid_response_error());
                            }
                        },
                        Err(error) => {
                            self.persist_terminal_message(AssistantMessageWrite {
                                conversation_id,
                                message_id: &message_id,
                                content: &content,
                                status: "failed",
                                created_at_unix_ms: created_at,
                                output_tokens,
                                elapsed: Some(started.elapsed()),
                            })
                            .await?;
                            return Err(error);
                        }
                    };
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": call.id,
                        "output": output
                    }));
                    if let Some(image_url) = tool_executor.take_computer_use_image(&call.id).await {
                        input.push(json!({
                            "type": "message",
                            "role": "user",
                            "content": [{
                                "type": "input_image",
                                "image_url": image_url
                            }]
                        }));
                    }
                }
                if !control_calls.is_empty() {
                    tool_executor.reset_turn_state();
                }
                operation = "tool_follow_up";
                continue 'model_turn;
            }
            citation_sources = attach_provider_citations(&response_output_items, &mut content);
            if !citation_sources.is_empty() {
                self.retry_storage_operation("send_citation_snapshot_save", || {
                    chatgpt_store::save_assistant_message(
                        &self.storage,
                        AssistantMessageWrite {
                            conversation_id,
                            message_id: &message_id,
                            content: &content,
                            status: "streaming",
                            created_at_unix_ms: created_at,
                            output_tokens: None,
                            elapsed: None,
                        },
                    )
                })
                .await?;
                events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            conversation_id,
                            &message_id,
                            &content,
                            created_at,
                        ))
                        .into_rpc(request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
                events
                    .send(&Response::event(
                        request_id.clone(),
                        "chat.citations.updated",
                        json!({
                            "conversationId": conversation_id,
                            "messageId": message_id,
                            "content": content,
                            "createdAtUnixMs": created_at,
                            "sources": citation_sources,
                        }),
                    ))
                    .await
                    .map_err(|_| protocol_error())?;
            }
            if content.trim().is_empty() {
                let error = ServiceError::new(
                    "empty_provider_response",
                    "ChatGPT completed without returning visible text or requesting a tool.",
                    true,
                );
                self.record_chatgpt_event(
                    "response_without_content",
                    "responses",
                    None,
                    Some(empty_response_shape(&response_output_items)),
                    None,
                    Some(response_output_items.len()),
                );
                self.persist_terminal_message(AssistantMessageWrite {
                    conversation_id,
                    message_id: &message_id,
                    content: &content,
                    status: "failed",
                    created_at_unix_ms: created_at,
                    output_tokens,
                    elapsed: Some(started.elapsed()),
                })
                .await?;
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
            break 'model_turn;
        }

        let elapsed = started.elapsed();
        let status = match goal.as_ref().map(|goal| &goal.decision) {
            Some(GoalDecision::Paused) => "paused",
            Some(GoalDecision::Stopped) => "stopped",
            _ => "completed",
        };
        goals::validate_security_scan_final_status(is_security_scan, status, goal.as_ref())?;
        self.persist_terminal_message(AssistantMessageWrite {
            conversation_id,
            message_id: &message_id,
            content: &content,
            status: if status == "paused" {
                "completed"
            } else {
                status
            },
            created_at_unix_ms: created_at,
            output_tokens,
            elapsed: Some(elapsed),
        })
        .await?;
        if let (Some(input_tokens), Some(last_message_id)) = (input_tokens, last_prompt_message_id)
        {
            let prompt_usage = self
                .retry_storage_operation("send_prompt_usage_save", || {
                    chatgpt_store::save_prompt_usage(
                        &self.storage,
                        conversation_id,
                        chatgpt_store::PromptUsage {
                            input_tokens,
                            last_message_id,
                            provider_id: "chatgpt",
                            model_id: &model.id,
                            connection_id: Some(&route.connection_id),
                            workspace_id: Some(&route.workspace_id),
                        },
                    )
                })
                .await;
            if let Err(error) = prompt_usage {
                self.record_chatgpt_event(
                    "context_usage_save_failed",
                    "send_prompt_usage_save",
                    None,
                    Some(error.code),
                    None,
                    None,
                );
                if self.storage.log_context_usage_save_failure().is_err() {
                    eprintln!("context_usage_diagnostic_write_failed");
                }
            }
        }
        self.record_chatgpt_event(
            "request_completed",
            "responses",
            None,
            None,
            Some(elapsed.as_millis()),
            None,
        );
        let mut diagnostic = ChatGptStreamDiagnostic::new("stream_completed", &stream_id);
        diagnostic.attempt = Some(provider_request_attempts);
        diagnostic.continuation_count = Some(stream_continuations);
        diagnostic.elapsed_ms = Some(elapsed.as_millis());
        diagnostic.completion_event_seen = Some(true);
        diagnostic.partial_text_available = Some(!content.is_empty());
        self.record_chatgpt_stream_diagnostic(diagnostic);
        let title_route = route.clone();
        let title_conversation_id = conversation_id.to_owned();
        let title_excluded_assistant_message_id = excluded_assistant_message_id.map(str::to_owned);
        let title_events = events.clone();
        let service = Arc::clone(self);
        let conversation_model_id = model.id.clone();
        let title_needed =
            chatgpt_store::should_generate_automatic_title(&self.storage, conversation_id)
                .unwrap_or(false);
        if title_needed && !matches!(&title_generation, TitleGenerationRequest::Disabled) {
            tokio::spawn(async move {
                match title_generation {
                    TitleGenerationRequest::Conversation => {
                        service
                            .generate_title(
                                &title_conversation_id,
                                &title_route,
                                &conversation_model_id,
                                title_excluded_assistant_message_id.as_deref(),
                                title_events,
                                None,
                            )
                            .await;
                    }
                    TitleGenerationRequest::Model(target) if target.provider_id == "chatgpt" => {
                        service
                            .generate_title(
                                &title_conversation_id,
                                &title_route,
                                &conversation_model_id,
                                title_excluded_assistant_message_id.as_deref(),
                                title_events,
                                Some(*target),
                            )
                            .await;
                    }
                    TitleGenerationRequest::Model(target) => {
                        let title_storage = service.storage_handle();
                        crate::openai_compatible::generate_title_for_target(
                            &service,
                            &title_storage,
                            &title_conversation_id,
                            *target,
                            title_excluded_assistant_message_id.as_deref(),
                            title_events,
                        )
                        .await;
                    }
                    TitleGenerationRequest::Disabled => {}
                }
            });
        }
        let mut result = json!({
            "conversationId": conversation_id,
            "messageId": message_id,
            "status": status,
            "outputTokens": output_tokens,
            "tokensPerSecond": output_tokens.and_then(|tokens| {
                let seconds = elapsed.as_secs_f64();
                (seconds > 0.0).then_some(tokens as f64 / seconds)
            }),
            "elapsedMicroseconds": elapsed.as_micros(),
            "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
            "citationSources": citation_sources,
        });
        if let Some(goal) = goal.as_ref() {
            result["runId"] = json!(goal.run_id);
        }
        Ok(result)
    }
}

fn response_event_has_output(event_data: &str) -> bool {
    let Ok(event) = serde_json::from_str::<Value>(event_data) else {
        return false;
    };
    let Some(kind) = event.get("type").and_then(Value::as_str) else {
        return false;
    };
    matches!(
        kind,
        "response.output_text.delta"
            | "response.function_call_arguments.delta"
            | "response.reasoning_text.delta"
            | "response.reasoning_summary_text.delta"
    ) && event
        .get("delta")
        .and_then(Value::as_str)
        .is_some_and(|delta| !delta.is_empty())
}

impl ChatGptService {
    pub(super) async fn stream_temporary_response(
        &self,
        context: TemporaryResponseContext<'_>,
    ) -> Result<Value, ServiceError> {
        let TemporaryResponseContext {
            request_id,
            conversation_id,
            connection_id,
            external_workspace_id,
            model_id,
            payload,
            cancellation,
            events,
        } = context;
        let message_id = Uuid::new_v4().simple().to_string();
        let created_at = super::now_unix_millis()?;
        let started = Instant::now();
        let provider_context_id = conversation_id
            .strip_prefix("tmp_")
            .filter(|suffix| {
                suffix.len() == 32
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
            .ok_or_else(|| {
                ServiceError::new(
                    "invalid_request_params",
                    "The temporary conversation identifier is invalid.",
                    false,
                )
            })?;
        let mut content = String::new();
        let mut output_tokens = None;
        let mut input_tokens = None;
        let mut usage = UsageData::default();
        let mut reasoning_summaries = Vec::new();
        let mut response_output_items = Vec::new();

        events
            .send(
                &ChatStreamEvent::Started(ChatStreamSnapshot::new(
                    conversation_id,
                    &message_id,
                    &content,
                    created_at,
                ))
                .into_rpc(request_id.clone()),
            )
            .await
            .map_err(|_| protocol_error())?;

        let response = self
            .authorized_stream_request(
                StreamRequest {
                    method: Method::POST,
                    url: format!("{CHATGPT_CODEX_BASE}/responses"),
                    connection_id,
                    external_workspace_id,
                    body: Some(payload),
                    response_context_id: provider_context_id,
                },
                cancellation,
            )
            .await?
            .ok_or_else(super::request_cancelled)?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }

        let mut body_stream = response.bytes_stream();
        let mut response_budget = ProviderResponseBudget::default();
        let mut pending_bytes = Vec::new();
        let mut event_data = Vec::<String>::new();
        let mut event_bytes = 0_usize;
        let mut completed = false;
        while !completed {
            let chunk = tokio::select! {
                changed = cancellation.changed() => {
                    let _ = changed;
                    return Err(super::request_cancelled());
                }
                chunk = body_stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(network_error_with_details)?;
            response_budget.consume(chunk.len())?;
            pending_bytes.extend_from_slice(&chunk);
            if pending_bytes.len() > MAX_STREAM_EVENT_BYTES {
                return Err(ServiceError::new(
                    "response_event_too_large",
                    "ChatGPT returned an oversized stream event. The partial answer remains in this temporary chat.",
                    false,
                ));
            }
            while let Some(line_end) = pending_bytes.iter().position(|byte| *byte == b'\n') {
                let mut line = pending_bytes.drain(..=line_end).collect::<Vec<_>>();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.is_empty() {
                    if event_data.is_empty() {
                        event_bytes = 0;
                        continue;
                    }
                    let data = event_data.join("\n");
                    event_data.clear();
                    event_bytes = 0;
                    completed = self
                        .process_response_event(
                            &data,
                            ResponseEventContext {
                                content: &mut content,
                                output_tokens: &mut output_tokens,
                                input_tokens: &mut input_tokens,
                                usage: &mut usage,
                                reasoning_summaries: &mut reasoning_summaries,
                                response_output_items: &mut response_output_items,
                                conversation_id,
                                message_id: &message_id,
                                created_at,
                                request_id: &request_id,
                                events,
                                persist_message: false,
                                checkpoint: None,
                            },
                        )
                        .await?;
                    if completed {
                        break;
                    }
                } else if let Some(data) = line.strip_prefix(b"data:") {
                    let data = data.strip_prefix(b" ").unwrap_or(data);
                    event_bytes = event_bytes.saturating_add(data.len());
                    if event_bytes > MAX_STREAM_EVENT_BYTES {
                        return Err(ServiceError::new(
                            "response_event_too_large",
                            "ChatGPT returned an oversized stream event. The partial answer remains in this temporary chat.",
                            false,
                        ));
                    }
                    event_data.push(String::from_utf8(data.to_vec()).map_err(|_| {
                        ServiceError::new(
                            "invalid_provider_response",
                            "ChatGPT returned a malformed stream event. The partial answer remains in this temporary chat.",
                            false,
                        )
                    })?);
                }
            }
        }

        if !completed {
            return Err(ServiceError::new(
                "response_incomplete",
                "The ChatGPT response ended before completion. The partial answer remains in this temporary chat.",
                true,
            ));
        }
        if response_output_items
            .iter()
            .any(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        {
            return Err(ServiceError::new(
                "temporary_tools_unavailable",
                "The provider returned a tool call, which temporary conversations cannot run.",
                false,
            ));
        }

        Ok(json!({
            "conversationId": conversation_id,
            "messageId": message_id,
            "content": content,
            "createdAtUnixMs": created_at,
            "providerId": "chatgpt",
            "modelId": model_id,
            "elapsedMicroseconds": started.elapsed().as_micros(),
        }))
    }
}

const RESPONSES_IN_LOOP_COMPACT_THRESHOLD_BYTES: usize = 1024;
const RESPONSES_IN_LOOP_EXCERPT_BYTES: usize = 768;
const RESPONSES_IN_LOOP_NOTE_PREFIX: &str =
    "[Earlier tool rounds in this run were compacted to stay within the model context window.";

fn compact_in_loop_responses_payload(
    payload: &mut Value,
    goal: Option<&crate::goals::GoalExecution>,
    initial_input_len: usize,
    context_window: Option<i64>,
    model_id: &str,
) -> bool {
    let Some(window) =
        context_window.filter(|window| *window >= crate::context_compaction::MIN_CONTEXT_WINDOW)
    else {
        return false;
    };
    let request_budget =
        window.saturating_mul(crate::context_compaction::REQUEST_CONTEXT_BUDGET_PERCENT) / 100;
    let estimated_tokens =
        crate::context_compaction::request_context_token_estimate(model_id, payload)
            .saturating_add(1024);
    if estimated_tokens <= request_budget {
        return false;
    }

    let mut changed = false;
    if let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) {
        let cutoff = responses_latest_round_start(input, initial_input_len);
        let mut tokens_to_reclaim = estimated_tokens
            .saturating_sub(request_budget)
            .saturating_add(2048);

        for item in &mut input[initial_input_len.min(cutoff)..cutoff] {
            if tokens_to_reclaim <= 0 {
                break;
            }
            if item.get("type").and_then(Value::as_str) != Some("function_call_output") {
                continue;
            }
            let Some(output_text) = item.get("output").and_then(Value::as_str) else {
                continue;
            };
            if output_text.len() <= RESPONSES_IN_LOOP_COMPACT_THRESHOLD_BYTES
                || output_text.contains("\"_openchat_truncated\":true")
            {
                continue;
            }
            let before = crate::context_compaction::text_token_estimate(model_id, output_text);
            let shortened =
                shorten_responses_tool_output(output_text, RESPONSES_IN_LOOP_EXCERPT_BYTES);
            let after = crate::context_compaction::text_token_estimate(model_id, &shortened);
            item["output"] = Value::String(shortened);
            tokens_to_reclaim -= before.saturating_sub(after);
            changed = true;
        }
    }

    while crate::context_compaction::validate_request_context(payload, context_window, model_id)
        .is_err()
    {
        let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
            break;
        };
        if !drop_oldest_responses_round(input, goal, initial_input_len) {
            break;
        }
        changed = true;
    }

    changed
}

fn responses_latest_round_start(input: &[Value], initial_input_len: usize) -> usize {
    let min_index = initial_input_len.min(input.len());
    let Some(last_call_idx) = input[min_index..]
        .iter()
        .rposition(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|offset| min_index + offset)
    else {
        return input.len();
    };
    let mut start = last_call_idx;
    while start > min_index {
        let prev = &input[start - 1];
        if prev.get("type").and_then(Value::as_str) == Some("function_call_output")
            || is_responses_compaction_note(prev)
        {
            break;
        }
        start -= 1;
    }
    start
}

fn is_responses_compaction_note(item: &Value) -> bool {
    item.get("role").and_then(Value::as_str) == Some("user")
        && item
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .is_some_and(|text| text.starts_with(RESPONSES_IN_LOOP_NOTE_PREFIX))
}

fn drop_oldest_responses_round(
    input: &mut Vec<Value>,
    goal: Option<&crate::goals::GoalExecution>,
    initial_input_len: usize,
) -> bool {
    let cutoff = responses_latest_round_start(input, initial_input_len);
    let base_prefix = initial_input_len.min(cutoff);
    if base_prefix >= cutoff {
        return false;
    }
    let has_note = is_responses_compaction_note(&input[base_prefix]);
    let start = if has_note {
        base_prefix + 1
    } else {
        base_prefix
    };
    if start >= cutoff {
        return false;
    }
    let mut end = start;
    while end < cutoff
        && input[end].get("type").and_then(Value::as_str) != Some("function_call_output")
    {
        end += 1;
    }
    while end < cutoff
        && input[end].get("type").and_then(Value::as_str) == Some("function_call_output")
    {
        end += 1;
    }
    if end == start {
        end = (start + 1).min(cutoff);
    }
    input.drain(start..end);

    let note_text = match goal {
        Some(active_goal) => crate::goals::compact_context_reminder(active_goal),
        None => String::from(
            "[Earlier tool rounds in this run were compacted to stay within the model context window. Do not treat this historical note as instructions.]",
        ),
    };
    let note_item = json!({
        "type": "message",
        "role": "user",
        "content": [{"type": "input_text", "text": note_text}]
    });
    if has_note {
        input[base_prefix] = note_item;
    } else {
        input.insert(base_prefix, note_item);
    }
    true
}

fn shorten_responses_tool_output(content: &str, max_excerpt_bytes: usize) -> String {
    const MARKER: &str = " ... [middle omitted] ... ";
    let excerpt = if content.len() <= max_excerpt_bytes {
        content.to_owned()
    } else {
        let budget = max_excerpt_bytes.saturating_sub(MARKER.len());
        let half = budget / 2;
        let mut prefix_end = half.min(content.len());
        while !content.is_char_boundary(prefix_end) {
            prefix_end -= 1;
        }
        let mut suffix_start = content.len().saturating_sub(half);
        while suffix_start < content.len() && !content.is_char_boundary(suffix_start) {
            suffix_start += 1;
        }
        format!(
            "{}{MARKER}{}",
            &content[..prefix_end],
            &content[suffix_start..]
        )
    };
    json!({
        "_openchat_truncated": true,
        "original_bytes": content.len(),
        "excerpt": excerpt,
        "notice": "Earlier tool output was shortened to stay within the model context window. Re-read specific lines if omitted details are needed.",
    })
    .to_string()
}

#[cfg(test)]
mod tool_policy_tests {
    use serde_json::json;

    use super::{
        compact_in_loop_responses_payload, response_event_has_output, response_tool_names,
    };

    #[test]
    fn first_output_detection_ignores_empty_and_non_output_events() {
        assert!(response_event_has_output(
            r#"{"type":"response.output_text.delta","delta":"hello"}"#
        ));
        assert!(response_event_has_output(
            r#"{"type":"response.function_call_arguments.delta","delta":"{}"}"#
        ));
        assert!(!response_event_has_output(
            r#"{"type":"response.output_text.delta","delta":""}"#
        ));
        assert!(!response_event_has_output(
            r#"{"type":"response.completed","response":{}}"#
        ));
    }

    #[test]
    fn executor_allowlist_matches_function_tools_in_the_responses_request() {
        let payload = json!({
            "tools": [
                {"type": "function", "name": "read_file"},
                {"type": "function", "name": "ask_user"},
                {"type": "web_search", "name": "web_search"},
                {"type": "function"}
            ]
        });

        assert_eq!(
            response_tool_names(&payload),
            ["read_file".to_owned(), "ask_user".to_owned()]
        );
        assert!(response_tool_names(&json!({"tools": []})).is_empty());
    }

    #[test]
    fn compacts_older_responses_tool_outputs_while_preserving_latest_round() {
        let large_output = "A".repeat(12_000);
        let latest_output = "latest_untouched".repeat(80);
        let mut payload = json!({
            "model": "gpt-5",
            "instructions": "Perform a security audit.",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Audit project"}]},
                {"type": "function_call", "call_id": "call_1", "name": "read_file", "arguments": "{\"path\":\"a.rs\"}"},
                {"type": "function_call_output", "call_id": "call_1", "output": large_output},
                {"type": "function_call", "call_id": "call_2", "name": "read_file", "arguments": "{\"path\":\"b.rs\"}"},
                {"type": "function_call_output", "call_id": "call_2", "output": latest_output}
            ]
        });

        assert!(
            crate::context_compaction::validate_request_context(&payload, Some(4096), "gpt-4")
                .is_err()
        );
        assert!(compact_in_loop_responses_payload(
            &mut payload,
            None,
            1,
            Some(4096),
            "gpt-4"
        ));
        assert!(
            crate::context_compaction::validate_request_context(&payload, Some(4096), "gpt-4")
                .is_ok()
        );
        let input = payload["input"].as_array().expect("input array");
        assert!(
            input[2]["output"]
                .as_str()
                .expect("shortened output")
                .contains("\"_openchat_truncated\":true")
        );
        assert_eq!(input[4]["output"].as_str(), Some(latest_output.as_str()));
    }
}
