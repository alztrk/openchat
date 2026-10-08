use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};
use uuid::Uuid;

use super::{
    CHATGPT_CODEX_BASE, ChatGptService, MAX_STREAM_EVENT_BYTES, connection_unavailable,
    database_error, http_error, invalid_response_error, network_error, truncate_chars,
};
use crate::{
    chatgpt_store::{self, ChatGptModel},
    protocol::{EventSink, Response as RpcResponse, ServiceError},
    usage_statistics::{UsageData, UsageRequestTracker},
};

impl ChatGptService {
    pub(super) async fn generate_title(
        self: &Arc<Self>,
        conversation_id: &str,
        route: &chatgpt_store::ConversationRoute,
        conversation_model_id: &str,
        excluded_assistant_message_id: Option<&str>,
        events: EventSink,
    ) {
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "title", None, None, None, None);
        let conversation_models = match self
            .fetch_models(&route.connection_id, &route.workspace_id)
            .await
        {
            Ok(models) => models,
            Err(_) => {
                self.record_title_skip(conversation_id, route, "model_catalog_unavailable");
                return;
            }
        };
        let same_account_model = conversation_models
            .into_iter()
            .find(|model| model.is_available && model.id != conversation_model_id);
        let (title_route, model) = if let Some(model) = same_account_model {
            (route.clone(), model)
        } else {
            let preference = match chatgpt_store::title_preference(&self.storage) {
                Ok(preference) => preference,
                Err(_) => {
                    self.record_title_skip(conversation_id, route, "title_preference_unavailable");
                    return;
                }
            };
            let (Some(connection_id), Some(workspace_id)) = (
                preference.connection_id.as_deref(),
                preference.workspace_id.as_deref(),
            ) else {
                self.record_title_skip(conversation_id, route, "no_alternate_model");
                return;
            };
            if connection_id == route.connection_id && workspace_id == route.workspace_id {
                self.record_title_skip(conversation_id, route, "no_alternate_model");
                return;
            }
            if self.ensure_active_connection(connection_id).is_err() {
                let mut title_route = route.clone();
                title_route.connection_id = connection_id.to_owned();
                title_route.workspace_id = workspace_id.to_owned();
                self.record_title_skip(
                    conversation_id,
                    &title_route,
                    "title_connection_unavailable",
                );
                return;
            }
            let title_models = match self.fetch_models(connection_id, workspace_id).await {
                Ok(models) => models,
                Err(_) => {
                    let mut title_route = route.clone();
                    title_route.connection_id = connection_id.to_owned();
                    title_route.workspace_id = workspace_id.to_owned();
                    self.record_title_skip(
                        conversation_id,
                        &title_route,
                        "title_model_catalog_unavailable",
                    );
                    return;
                }
            };
            let Some(model) = title_models
                .into_iter()
                .find(|model| model.is_available && model.id != conversation_model_id)
            else {
                let mut title_route = route.clone();
                title_route.connection_id = connection_id.to_owned();
                title_route.workspace_id = workspace_id.to_owned();
                self.record_title_skip(conversation_id, &title_route, "no_alternate_model");
                return;
            };
            let mut title_route = route.clone();
            title_route.connection_id = connection_id.to_owned();
            title_route.workspace_id = workspace_id.to_owned();
            (title_route, model)
        };
        let usage = match self
            .refresh_usage(&title_route.connection_id, &title_route.workspace_id)
            .await
        {
            Ok(usage) => usage,
            Err(_) => {
                self.record_title_skip(conversation_id, &title_route, "quota_unavailable");
                return;
            }
        };
        if usage.freshness != "current" || usage.ordinary_usage_allowed != Some(true) {
            self.record_title_skip(
                conversation_id,
                &title_route,
                "ordinary_quota_not_available",
            );
            return;
        }
        let job_id = Uuid::new_v4().simple().to_string();
        if chatgpt_store::create_title_job(
            &self.storage,
            chatgpt_store::NewTitleJob {
                id: &job_id,
                conversation_id,
                connection_id: &title_route.connection_id,
                workspace_id: &title_route.workspace_id,
                model_id: Some(&model.id),
                status: "queued",
                reason_code: None,
            },
        )
        .is_err()
        {
            self.record_chatgpt_event(
                "request_failed",
                "title",
                None,
                Some("local_storage_failed"),
                Some(started.elapsed().as_millis()),
                None,
            );
            return;
        }
        let _ = chatgpt_store::finish_title_job(&self.storage, &job_id, "running", None);
        let title = match self
            .request_title(
                conversation_id,
                &title_route,
                &model,
                conversation_model_id,
                excluded_assistant_message_id,
            )
            .await
        {
            Ok(title) => title,
            Err(error) => {
                self.record_chatgpt_event(
                    "request_failed",
                    "title",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                let _ = chatgpt_store::finish_title_job(
                    &self.storage,
                    &job_id,
                    "failed",
                    Some(error.code),
                );
                return;
            }
        };
        match chatgpt_store::apply_generated_title(&self.storage, conversation_id, &title) {
            Ok(true) => {
                self.record_chatgpt_event(
                    "request_completed",
                    "title",
                    None,
                    None,
                    Some(started.elapsed().as_millis()),
                    None,
                );
                let _ = chatgpt_store::finish_title_job(&self.storage, &job_id, "completed", None);
                let _ = events
                    .send(&RpcResponse::event(
                        json!(0),
                        "chat.title_updated",
                        json!({"conversationId": conversation_id, "title": title}),
                    ))
                    .await;
            }
            Ok(false) => {
                self.record_chatgpt_event(
                    "request_cancelled",
                    "title",
                    None,
                    Some("manual_title_preserved"),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                let _ = chatgpt_store::finish_title_job(
                    &self.storage,
                    &job_id,
                    "cancelled",
                    Some("manual_title_preserved"),
                );
            }
            Err(_) => {
                self.record_chatgpt_event(
                    "request_failed",
                    "title",
                    None,
                    Some("local_storage_failed"),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                let _ = chatgpt_store::finish_title_job(
                    &self.storage,
                    &job_id,
                    "failed",
                    Some("local_storage_failed"),
                );
            }
        }
    }

    async fn request_title(
        &self,
        conversation_id: &str,
        route: &chatgpt_store::ConversationRoute,
        title_model: &ChatGptModel,
        conversation_model_id: &str,
        excluded_assistant_message_id: Option<&str>,
    ) -> Result<String, ServiceError> {
        if title_model.id == conversation_model_id {
            return Err(ServiceError::new(
                "title_model_not_distinct",
                "A different model is required for conversation titles.",
                false,
            ));
        }
        let workspace = chatgpt_store::connection_workspace(
            &self.storage,
            &route.connection_id,
            &route.workspace_id,
        )
        .map_err(database_error)?
        .ok_or_else(connection_unavailable)?;
        let mut messages = chatgpt_store::conversation_messages(&self.storage, conversation_id)
            .map_err(database_error)?;
        if let Some(excluded_id) = excluded_assistant_message_id {
            messages.retain(|message| message.id != excluded_id);
        }
        let first_user = messages.iter().find(|message| message.role == "user");
        let first_assistant = messages.iter().find(|message| message.role == "assistant");
        let user_text = first_user.map(|message| truncate_chars(&message.content, 1200));
        let assistant_text = first_assistant.map(|message| truncate_chars(&message.content, 1200));
        let mut context = String::from(
            "Create a concise title for this conversation. Return only the title, without quotation marks.\n\n",
        );
        if let Some(user_text) = user_text {
            context.push_str("User: ");
            context.push_str(&user_text);
            context.push('\n');
        }
        if let Some(assistant_text) = assistant_text {
            context.push_str("Assistant: ");
            context.push_str(&assistant_text);
        }
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "title_response", None, None, None, None);
        let mut usage_request = UsageRequestTracker::start(
            &self.storage,
            conversation_id,
            None,
            "chatgpt",
            &title_model.id,
            None,
            "title_generation",
            false,
        )
        .map_err(database_error)?;
        let body = json!({
            "model": title_model.id,
            "input": [{"role": "user", "content": context}],
            "stream": true,
            "store": false,
        });
        usage_request
            .record_request_manifest(&body, None)
            .map_err(database_error)?;
        let response = match self
            .authorized_request(
                Method::POST,
                format!("{CHATGPT_CODEX_BASE}/responses"),
                &route.connection_id,
                &workspace.external_id,
                Some(body),
                Some(conversation_id),
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.record_chatgpt_event(
                    "request_failed",
                    "title_response",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
        };
        let status = response.status();
        self.record_chatgpt_event(
            "http_response",
            "title_response",
            Some(status.as_u16()),
            None,
            Some(started.elapsed().as_millis()),
            None,
        );
        if !status.is_success() {
            let error = http_error(status);
            self.record_chatgpt_event(
                "request_failed",
                "title_response",
                Some(status.as_u16()),
                Some(error.code),
                Some(started.elapsed().as_millis()),
                None,
            );
            return Err(error);
        }
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        let mut data = Vec::<String>::new();
        let mut title = String::new();
        let mut usage = UsageData::default();
        let mut completed = false;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| network_error())?;
            buffer.extend_from_slice(&chunk);
            if buffer.len() > MAX_STREAM_EVENT_BYTES {
                return Err(invalid_response_error());
            }
            while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
                let mut line = buffer.drain(..=end).collect::<Vec<_>>();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.is_empty() && !data.is_empty() {
                    let payload = data.join("\n");
                    data.clear();
                    let event: Value =
                        serde_json::from_str(&payload).map_err(|_| invalid_response_error())?;
                    if event.get("response").is_some() {
                        usage = UsageData::from_responses_event(&event, "chatgpt");
                    }
                    match event.get("type").and_then(Value::as_str) {
                        Some("response.output_text.delta") => {
                            let delta = event
                                .get("delta")
                                .and_then(Value::as_str)
                                .ok_or_else(invalid_response_error)?;
                            title.push_str(delta);
                        }
                        Some("response.completed") => {
                            completed = true;
                            break;
                        }
                        Some("response.failed" | "response.incomplete" | "error") => {
                            usage_request.fail(&usage).map_err(database_error)?;
                            return Err(ServiceError::new(
                                "title_generation_failed",
                                "ChatGPT could not generate a conversation title.",
                                false,
                            ));
                        }
                        _ => {}
                    }
                } else if let Some(value) = line.strip_prefix(b"data:") {
                    let value = value.strip_prefix(b" ").unwrap_or(value);
                    data.push(
                        String::from_utf8(value.to_vec()).map_err(|_| invalid_response_error())?,
                    );
                }
            }
            if completed {
                break;
            }
        }
        if !completed {
            return Err(ServiceError::new(
                "title_generation_incomplete",
                "ChatGPT ended the title response before completion.",
                false,
            ));
        }
        usage_request.complete(&usage).map_err(database_error)?;
        let title = title.trim().trim_matches(['"', '\'']).trim();
        if title.is_empty() {
            return Err(invalid_response_error());
        }
        self.record_chatgpt_event(
            "request_completed",
            "title_response",
            Some(status.as_u16()),
            None,
            Some(started.elapsed().as_millis()),
            None,
        );
        Ok(truncate_chars(title, 100))
    }

    fn record_title_skip(
        &self,
        conversation_id: &str,
        route: &chatgpt_store::ConversationRoute,
        reason: &'static str,
    ) {
        self.record_chatgpt_event("request_skipped", "title", None, Some(reason), None, None);
        let job_id = Uuid::new_v4().simple().to_string();
        let _ = chatgpt_store::create_title_job(
            &self.storage,
            chatgpt_store::NewTitleJob {
                id: &job_id,
                conversation_id,
                connection_id: &route.connection_id,
                workspace_id: &route.workspace_id,
                model_id: None,
                status: "skipped",
                reason_code: Some(reason),
            },
        );
    }
}
