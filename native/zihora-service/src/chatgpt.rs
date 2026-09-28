use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::{Client, Method, Response, StatusCode, header::ACCEPT};
use rusqlite::Error as DatabaseError;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    chatgpt_store::{self, ChatGptModel, NewUsageSnapshot, ResetCredit, UsageBucket},
    credentials::OAuthCredentialReference,
    instructions,
    oauth::{OAuthClient, parse_reference},
    permissions::ToolPermissionBroker,
    protocol::{EventSink, Response as RpcResponse, ServiceError},
    provider_schema::{
        ChatStreamEvent, ChatStreamSnapshot, ProviderChatRequest, ProviderMessage,
        ReasoningSummary, ToolCall, ToolDefinition,
    },
    storage::AppStorage,
    tools::{self, ToolExecutor, ToolPermissionMode},
};

struct ReasoningSummaryGroup {
    item_id: String,
    summary_index: u64,
    content: String,
    started_at: Instant,
    elapsed: Option<Duration>,
    is_complete: bool,
}

fn reasoning_summary_group_mut<'a>(
    groups: &'a mut Vec<ReasoningSummaryGroup>,
    item_id: &str,
    summary_index: u64,
) -> &'a mut ReasoningSummaryGroup {
    let index = match groups
        .iter()
        .position(|group| group.item_id == item_id && group.summary_index == summary_index)
    {
        Some(index) => index,
        None => {
            groups.push(ReasoningSummaryGroup {
                item_id: item_id.to_owned(),
                summary_index,
                content: String::new(),
                started_at: Instant::now(),
                elapsed: None,
                is_complete: false,
            });
            groups.len() - 1
        }
    };
    &mut groups[index]
}

fn reasoning_summary_groups_value(groups: &[ReasoningSummaryGroup]) -> Vec<ReasoningSummary> {
    let now = Instant::now();
    groups
        .iter()
        .map(|group| {
            let elapsed = group
                .elapsed
                .unwrap_or_else(|| now.saturating_duration_since(group.started_at));
            ReasoningSummary {
                id: format!("{}:{}", group.item_id, group.summary_index),
                content: group.content.clone(),
                elapsed_microseconds: i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX),
                is_complete: group.is_complete,
            }
        })
        .collect()
}

async fn send_reasoning_snapshot(
    events: &EventSink,
    request_id: &Value,
    conversation_id: &str,
    message_id: &str,
    content: &str,
    created_at_unix_ms: i64,
    groups: &[ReasoningSummaryGroup],
) -> Result<(), ServiceError> {
    events
        .send(
            &ChatStreamEvent::ReasoningSummariesUpdated {
                snapshot: ChatStreamSnapshot::new(
                    conversation_id,
                    message_id,
                    content,
                    created_at_unix_ms,
                ),
                summaries: reasoning_summary_groups_value(groups),
            }
            .into_rpc(request_id.clone()),
        )
        .await
        .map_err(|_| protocol_error())
}

fn finish_reasoning_summary_group(
    groups: &mut Vec<ReasoningSummaryGroup>,
    item_id: &str,
    summary_index: u64,
    content: Option<&str>,
) {
    let group = reasoning_summary_group_mut(groups, item_id, summary_index);
    if let Some(content) = content {
        group.content = content.to_owned();
    }
    group.elapsed = Some(Instant::now().saturating_duration_since(group.started_at));
    group.is_complete = true;
}

fn finish_reasoning_summary_groups(groups: &mut [ReasoningSummaryGroup]) {
    let now = Instant::now();
    for group in groups.iter_mut().filter(|group| !group.is_complete) {
        group.elapsed = Some(now.saturating_duration_since(group.started_at));
        group.is_complete = true;
    }
}

const CHATGPT_CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";
const CHATGPT_WHAM_BASE: &str = "https://chatgpt.com/backend-api/wham";
// The private catalog filters entries by its Codex client compatibility version, not Zihora's product version.
const CHATGPT_CLIENT_VERSION: &str = "0.157.0";
const MODEL_CATALOG_CACHE_AGE: Duration = Duration::from_secs(6 * 60 * 60);
const METADATA_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const USAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const RESET_CREDITS_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_JSON_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_STREAM_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct ChatGptService {
    storage: Arc<AppStorage>,
    oauth: Arc<OAuthClient>,
    http: Client,
    refresh_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl ChatGptService {
    pub fn new(storage: Arc<AppStorage>) -> Result<Self, ServiceError> {
        let http = Client::builder()
            .user_agent(concat!("Zihora/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|_| network_error())?;
        Ok(Self {
            storage,
            oauth: Arc::new(OAuthClient::new()?),
            http,
            refresh_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn oauth_sign_in(
        &self,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        self.oauth.authorize(&self.storage, cancellation).await
    }

    pub fn list_connections(&self) -> Result<Value, ServiceError> {
        let connections = chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
        serde_json::to_value(json!({"connections": connections})).map_err(|_| serialization_error())
    }

    pub async fn delete_connection(&self, connection_id: &str) -> Result<Value, ServiceError> {
        let lock = self.connection_refresh_lock(connection_id).await;
        let _guard = lock.lock().await;
        let credential_reference =
            chatgpt_store::credential_reference(&self.storage, connection_id)
                .map_err(database_error)?
                .ok_or_else(|| {
                    ServiceError::new(
                        "connection_not_found",
                        "The ChatGPT connection could not be found.",
                        false,
                    )
                })?;
        let reference = parse_reference(connection_id, &credential_reference)?;
        self.oauth.delete_token_pair(&reference)?;

        if !chatgpt_store::delete_connection(&self.storage, connection_id)
            .map_err(database_error)?
        {
            return Err(ServiceError::new(
                "connection_not_found",
                "The ChatGPT connection could not be found.",
                false,
            ));
        }

        self.list_connections()
    }

    pub fn select_connection(&self, connection_id: &str) -> Result<Value, ServiceError> {
        let selected = chatgpt_store::set_selected_connection(&self.storage, connection_id)
            .map_err(database_error)?;
        if !selected {
            return Err(connection_unavailable());
        }
        self.list_connections()
    }

    pub fn select_workspace(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<Value, ServiceError> {
        let selected =
            chatgpt_store::set_selected_workspace(&self.storage, connection_id, workspace_id)
                .map_err(database_error)?;
        if !selected {
            return Err(ServiceError::new(
                "workspace_not_found",
                "The selected ChatGPT workspace is no longer available. Refresh the connection list.",
                false,
            ));
        }
        self.list_connections()
    }

    pub async fn models(
        &self,
        connection_id: &str,
        workspace_id: &str,
        force_refresh: bool,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "models", None, None, None, None);
        if !force_refresh {
            let cached = chatgpt_store::list_fresh_models(
                &self.storage,
                connection_id,
                workspace_id,
                CHATGPT_CLIENT_VERSION,
                i64::try_from(MODEL_CATALOG_CACHE_AGE.as_millis()).unwrap_or(i64::MAX),
            )
            .map_err(database_error)?;
            if let Some(models) = cached {
                self.record_chatgpt_event(
                    "request_cache_hit",
                    "models",
                    None,
                    None,
                    Some(started.elapsed().as_millis()),
                    Some(models.len()),
                );
                return Ok(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": models,
                    "freshness": "current",
                    "errorCode": Value::Null,
                }));
            }
        }
        let fetch_result = if *cancellation.borrow() {
            Err(request_cancelled())
        } else {
            tokio::select! {
                _ = cancellation.changed() => Err(request_cancelled()),
                result = tokio::time::timeout(
                    METADATA_REQUEST_TIMEOUT,
                    self.fetch_models(connection_id, workspace_id),
                ) => result.unwrap_or_else(|_| Err(provider_timeout())),
            }
        };

        match fetch_result {
            Ok(models) => {
                let model_count = models.len();
                let save_result = chatgpt_store::save_models(
                    &self.storage,
                    connection_id,
                    workspace_id,
                    CHATGPT_CLIENT_VERSION,
                    &models,
                )
                .map_err(database_error);
                if let Err(error) = save_result {
                    self.record_chatgpt_event(
                        "request_failed",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    return Err(error);
                }
                let result = serde_json::to_value(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": models,
                    "freshness": "current",
                    "errorCode": Value::Null,
                }))
                .map_err(|_| serialization_error());
                match result {
                    Ok(value) => {
                        self.record_chatgpt_event(
                            "request_completed",
                            "models",
                            None,
                            None,
                            Some(started.elapsed().as_millis()),
                            Some(model_count),
                        );
                        Ok(value)
                    }
                    Err(error) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "models",
                            None,
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        Err(error)
                    }
                }
            }

            Err(error) => {
                if error.code == "request_cancelled" {
                    self.record_chatgpt_event(
                        "request_cancelled",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    return Err(error);
                }
                let cached = chatgpt_store::list_models(&self.storage, connection_id, workspace_id)
                    .map_err(database_error)?;
                if cached.is_empty() {
                    self.record_chatgpt_event(
                        "request_failed",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        Some(0),
                    );
                    return Err(error);
                }
                let cached_count = cached.len();
                let result = serde_json::to_value(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": cached,
                    "freshness": "stale",
                    "errorCode": error.code,
                }))
                .map_err(|_| serialization_error());
                match result {
                    Ok(value) => {
                        self.record_chatgpt_event(
                            "request_stale",
                            "models",
                            None,
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            Some(cached_count),
                        );
                        Ok(value)
                    }
                    Err(serialization_error) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "models",
                            None,
                            Some(serialization_error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        Err(serialization_error)
                    }
                }
            }
        }
    }

    pub fn title_preference(&self) -> Result<Value, ServiceError> {
        serde_json::to_value(
            chatgpt_store::title_preference(&self.storage).map_err(database_error)?,
        )
        .map_err(|_| serialization_error())
    }

    pub fn select_title_target(
        &self,
        connection_id: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Result<Value, ServiceError> {
        match (connection_id, workspace_id) {
            (None, None) => {}
            (Some(connection_id), None) => {
                let connections =
                    chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
                if !connections
                    .iter()
                    .any(|connection| connection.id == connection_id)
                {
                    return Err(connection_unavailable());
                }
            }
            (Some(connection_id), Some(workspace_id)) => {
                let workspace =
                    chatgpt_store::connection_workspace(&self.storage, connection_id, workspace_id)
                        .map_err(database_error)?;
                if workspace.is_none() {
                    return Err(ServiceError::new(
                        "workspace_not_found",
                        "The selected title workspace is no longer available.",
                        false,
                    ));
                }
            }
            (None, Some(_)) => {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "Choose a ChatGPT account before choosing a title workspace.",
                    false,
                ));
            }
        }
        chatgpt_store::set_title_preference(&self.storage, connection_id, workspace_id)
            .map_err(database_error)?;
        self.title_preference()
    }

    pub async fn usage(
        &self,
        connection_id: &str,
        workspace_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "usage", None, None, None, None);
        let refresh_result = if *cancellation.borrow() {
            Err(request_cancelled())
        } else {
            tokio::select! {
                _ = cancellation.changed() => Err(request_cancelled()),
                result = tokio::time::timeout(
                    METADATA_REQUEST_TIMEOUT,
                    self.refresh_usage(connection_id, workspace_id),
                ) => result.unwrap_or_else(|_| Err(provider_timeout())),
            }
        };
        let snapshot = match refresh_result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let event = if error.code == "request_cancelled" {
                    "request_cancelled"
                } else {
                    "request_failed"
                };
                self.record_chatgpt_event(
                    event,
                    "usage",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
        };
        let result = serde_json::to_value(&snapshot).map_err(|_| serialization_error());
        match result {
            Ok(value) => {
                self.record_chatgpt_event(
                    "request_completed",
                    "usage",
                    None,
                    None,
                    Some(started.elapsed().as_millis()),
                    Some(snapshot.buckets.len()),
                );
                Ok(value)
            }
            Err(error) => {
                self.record_chatgpt_event(
                    "request_failed",
                    "usage",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                Err(error)
            }
        }
    }

    pub async fn send_message(
        self: &Arc<Self>,
        request_id: Value,
        conversation_id: &str,
        reasoning_effort: Option<&str>,
        excluded_assistant_message_id: Option<&str>,
        custom_instructions: Option<&str>,
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
        permission_broker: &ToolPermissionBroker,
        cancellation: &mut watch::Receiver<bool>,
        events: EventSink,
    ) -> Result<Value, ServiceError> {
        let route = chatgpt_store::conversation_route(&self.storage, conversation_id)
            .map_err(database_error)?
            .ok_or_else(|| {
                ServiceError::new(
                    "conversation_not_routed",
                    "Choose a ChatGPT account and model before sending a message.",
                    false,
                )
            })?;
        self.ensure_active_connection(&route.connection_id)?;
        let workspace = chatgpt_store::connection_workspace(
            &self.storage,
            &route.connection_id,
            &route.workspace_id,
        )
        .map_err(database_error)?
        .ok_or_else(connection_unavailable)?;
        let model = chatgpt_store::selected_model(
            &self.storage,
            &route.connection_id,
            &route.workspace_id,
            &route.model_id,
        )
        .map_err(database_error)?
        .filter(|model| model.is_available)
        .ok_or_else(|| {
            ServiceError::new(
                "model_unavailable",
                "The model selected for this conversation is no longer available. Choose another model.",
                false,
            )
        })?;
        let messages = chatgpt_store::conversation_messages(&self.storage, conversation_id)
            .map_err(database_error)?;
        if excluded_assistant_message_id
            .is_some_and(|id| !chatgpt_store::is_retryable_latest_assistant_message(&messages, id))
        {
            return Err(invalid_retry_target_error());
        }
        if messages.is_empty() {
            return Err(ServiceError::new(
                "conversation_empty",
                "Write a message before starting the ChatGPT response.",
                false,
            ));
        }
        let history = messages
            .iter()
            .filter(|message| Some(message.id.as_str()) != excluded_assistant_message_id)
            .map(|message| ProviderMessage::from_history(&message.role, &message.content))
            .collect::<Result<Vec<_>, _>>()?;
        let provider_request = ProviderChatRequest {
            model: model.id.clone(),
            instructions: instructions::shared_instructions(
                custom_instructions,
                permission_mode,
                project_root.is_some(),
            ),
            messages: history,
            tools: tools::definitions(),
            reasoning_effort: reasoning_effort
                .filter(|effort| model.reasoning_levels.iter().any(|level| level == *effort))
                .map(str::to_owned),
        };
        let input = provider_request
            .messages
            .iter()
            .map(|message| {
                json!({"role": message.role.as_str(), "content": message.content.as_str()})
            })
            .collect::<Vec<_>>();
        let mut payload = json!({
            "model": provider_request.model.as_str(),
            "input": input,
            "instructions": provider_request.instructions,
            "stream": true,
            "store": false,
        });
        if !provider_request.tools.is_empty() {
            payload["tools"] = json!(
                provider_request
                    .tools
                    .iter()
                    .map(responses_tool)
                    .collect::<Vec<_>>()
            );
            payload["tool_choice"] = json!("auto");
            payload["parallel_tool_calls"] = json!(false);
        }
        let effort = provider_request.reasoning_effort.as_deref();
        if model.supports_reasoning_summary_parameter {
            let mut reasoning = json!({"summary": "auto"});
            if let Some(effort) = effort {
                reasoning["effort"] = json!(effort);
            }
            payload["reasoning"] = reasoning;
        } else if let Some(effort) = effort {
            payload["reasoning"] = json!({"effort": effort});
        }

        self.stream_response(
            request_id,
            conversation_id,
            &route,
            &workspace.external_id,
            &model,
            payload,
            excluded_assistant_message_id,
            project_root,
            data_root,
            permission_mode,
            permission_broker,
            cancellation,
            events,
        )
        .await
    }

    async fn stream_response(
        self: &Arc<Self>,
        request_id: Value,
        conversation_id: &str,
        route: &chatgpt_store::ConversationRoute,
        external_workspace_id: &str,
        model: &ChatGptModel,
        mut payload: Value,
        excluded_assistant_message_id: Option<&str>,
        project_root: Option<&Path>,
        data_root: &Path,
        permission_mode: ToolPermissionMode,
        permission_broker: &ToolPermissionBroker,
        cancellation: &mut watch::Receiver<bool>,
        events: EventSink,
    ) -> Result<Value, ServiceError> {
        let message_id = Uuid::new_v4().simple().to_string();
        let created_at = now_unix_millis()?;
        let started = Instant::now();
        let mut content = String::new();
        let mut reasoning_summaries = Vec::new();
        self.record_chatgpt_event("request_started", "responses", None, None, None, None);
        chatgpt_store::save_assistant_message(
            &self.storage,
            conversation_id,
            &message_id,
            &content,
            "streaming",
            created_at,
            None,
            None,
            None,
        )
        .map_err(database_error)?;

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
            self.persist_terminal_message(
                conversation_id,
                &message_id,
                &content,
                "stopped",
                created_at,
                None,
                started.elapsed(),
            )?;
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
        let mut tool_executor = ToolExecutor::new(project_root, data_root, permission_mode);
        'model_turn: loop {
            let mut round_output_tokens = None;
            let mut response_output_items = Vec::new();
            let response = match self
                .authorized_stream_request(
                    Method::POST,
                    format!("{CHATGPT_CODEX_BASE}/responses"),
                    &route.connection_id,
                    external_workspace_id,
                    Some(payload.clone()),
                    conversation_id,
                    cancellation,
                )
                .await
            {
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
                        self.persist_terminal_message(
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            None,
                            started.elapsed(),
                        )?;
                        return Err(error);
                    }
                }
                Ok(None) => {
                    let elapsed = started.elapsed();
                    self.record_chatgpt_event(
                        "request_cancelled",
                        "responses",
                        None,
                        Some("request_cancelled"),
                        Some(elapsed.as_millis()),
                        None,
                    );
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "stopped",
                        created_at,
                        None,
                        elapsed,
                    )?;
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
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        None,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
            };

            let mut body_stream = response.bytes_stream();
            let mut pending_bytes = Vec::new();
            let mut event_data = Vec::<String>::new();
            let mut completed = false;
            let mut failure = None;

            loop {
                let next = tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            self.record_chatgpt_event(
                                "request_cancelled",
                                "responses",
                                None,
                                Some("request_cancelled"),
                                Some(started.elapsed().as_millis()),
                                None,
                            );
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "stopped",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
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
                    Err(_) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "responses",
                            None,
                            Some("network_unavailable"),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        self.persist_terminal_message(
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            output_tokens,
                            started.elapsed(),
                        )?;
                        return Err(network_error());
                    }
                };
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
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
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
                        match self
                            .process_response_event(
                                &event,
                                &mut content,
                                &mut round_output_tokens,
                                &mut reasoning_summaries,
                                &mut response_output_items,
                                conversation_id,
                                &message_id,
                                created_at,
                                started,
                                &request_id,
                                &events,
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
                                self.persist_terminal_message(
                                    conversation_id,
                                    &message_id,
                                    &content,
                                    "failed",
                                    created_at,
                                    output_tokens,
                                    started.elapsed(),
                                )?;
                                return Err(error);
                            }
                        }
                    }
                }
                if completed || failure.is_some() {
                    break;
                }
            }

            if let Some(error) = failure {
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
                return Err(error);
            }
            if !completed {
                let error = ServiceError::new(
                    "response_incomplete",
                    "The ChatGPT response ended before completion. The partial answer was saved.",
                    true,
                );
                self.record_chatgpt_event(
                    "request_failed",
                    "responses",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
                return Err(error);
            }

            if let Some(round_tokens) = round_output_tokens {
                output_tokens = Some(output_tokens.unwrap_or(0i64).saturating_add(round_tokens));
            }
            let final_text = response_output_text(&response_output_items);
            if !final_text.is_empty() && final_text != content.as_str() {
                content = final_text;
                chatgpt_store::save_assistant_message(
                    &self.storage,
                    conversation_id,
                    &message_id,
                    &content,
                    "streaming",
                    created_at,
                    None,
                    None,
                    None,
                )
                .map_err(database_error)?;
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
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
            };
            if !tool_calls.is_empty() {
                if let Err(error) = tool_executor.begin_round(&tool_calls) {
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(error);
                }
                let mut results = Vec::with_capacity(tool_calls.len());
                for call in &tool_calls {
                    let snapshot =
                        ChatStreamSnapshot::new(conversation_id, &message_id, &content, created_at);
                    let result = match tool_executor
                        .execute_call(
                            call,
                            permission_broker,
                            &request_id,
                            &snapshot,
                            &events,
                            cancellation,
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
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "stopped",
                                created_at,
                                output_tokens,
                                elapsed,
                            )?;
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
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
                            return Err(error);
                        }
                    };
                    results.push(result);
                }
                let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
                    self.persist_terminal_message(
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        started.elapsed(),
                    )?;
                    return Err(invalid_response_error());
                };
                input.extend(response_output_items);
                for result in results {
                    let output = match serde_json::to_string(&result.output) {
                        Ok(output) => output,
                        Err(_) => {
                            self.persist_terminal_message(
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                started.elapsed(),
                            )?;
                            return Err(invalid_response_error());
                        }
                    };
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": result.call_id,
                        "output": output
                    }));
                }
                continue 'model_turn;
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
                self.persist_terminal_message(
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    started.elapsed(),
                )?;
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
        self.persist_terminal_message(
            conversation_id,
            &message_id,
            &content,
            "completed",
            created_at,
            output_tokens,
            elapsed,
        )?;
        self.record_chatgpt_event(
            "request_completed",
            "responses",
            None,
            None,
            Some(elapsed.as_millis()),
            None,
        );
        let title_route = route.clone();
        let title_conversation_id = conversation_id.to_owned();
        let title_excluded_assistant_message_id = excluded_assistant_message_id.map(str::to_owned);
        let title_events = events.clone();
        let service = Arc::clone(self);
        let conversation_model_id = model.id.clone();
        let title_is_automatic = route.title_is_automatic;
        if title_is_automatic {
            tokio::spawn(async move {
                service
                    .generate_title(
                        &title_conversation_id,
                        &title_route,
                        &conversation_model_id,
                        title_excluded_assistant_message_id.as_deref(),
                        title_events,
                    )
                    .await;
            });
        }
        Ok(json!({
            "conversationId": conversation_id,
            "messageId": message_id,
            "status": "completed",
            "outputTokens": output_tokens,
            "tokensPerSecond": output_tokens.and_then(|tokens| {
                let seconds = elapsed.as_secs_f64();
                (seconds > 0.0).then_some(tokens as f64 / seconds)
            }),
            "elapsedMicroseconds": elapsed.as_micros(),
            "reasoningGroups": reasoning_summary_groups_value(&reasoning_summaries),
        }))
    }

    async fn process_response_event(
        &self,
        event_data: &str,
        content: &mut String,
        output_tokens: &mut Option<i64>,
        reasoning_summaries: &mut Vec<ReasoningSummaryGroup>,
        response_output_items: &mut Vec<Value>,
        conversation_id: &str,
        message_id: &str,
        created_at: i64,
        started: Instant,
        request_id: &Value,
        events: &EventSink,
    ) -> Result<bool, ServiceError> {
        if event_data == "[DONE]" {
            finish_reasoning_summary_groups(reasoning_summaries);
            return Ok(true);
        }
        let event: Value = serde_json::from_str(event_data).map_err(|_| {
            ServiceError::new(
                "invalid_provider_response",
                "ChatGPT returned a malformed stream event. The partial answer was saved.",
                false,
            )
        })?;
        let kind = event.get("type").and_then(Value::as_str).ok_or_else(|| {
            ServiceError::new(
                "invalid_provider_response",
                "ChatGPT returned an unsupported stream event. The partial answer was saved.",
                false,
            )
        })?;
        match kind {
            "response.output_item.done" => {
                let item = event
                    .get("item")
                    .filter(|item| item.is_object())
                    .ok_or_else(invalid_response_error)?;
                response_output_items.push(item.clone());
            }
            "response.output_text.delta" => {
                let delta = event.get("delta").and_then(Value::as_str).ok_or_else(|| {
                    ServiceError::new(
                        "invalid_provider_response",
                        "ChatGPT returned an invalid text update. The partial answer was saved.",
                        false,
                    )
                })?;
                content.push_str(delta);
                chatgpt_store::save_assistant_message(
                    &self.storage,
                    conversation_id,
                    message_id,
                    content,
                    "streaming",
                    created_at,
                    None,
                    None,
                    None,
                )
                .map_err(database_error)?;
                events
                    .send(
                        &ChatStreamEvent::TextUpdated(ChatStreamSnapshot::new(
                            conversation_id,
                            message_id,
                            content,
                            created_at,
                        ))
                        .into_rpc(request_id.clone()),
                    )
                    .await
                    .map_err(|_| protocol_error())?;
            }
            "response.reasoning_summary_part.added" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    let group =
                        reasoning_summary_group_mut(reasoning_summaries, item_id, summary_index);
                    if let Some(part_content) = event
                        .get("part")
                        .and_then(|part| part.get("text"))
                        .and_then(Value::as_str)
                    {
                        group.content = part_content.to_owned();
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let (Some(item_id), Some(summary_index), Some(delta)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    let group =
                        reasoning_summary_group_mut(reasoning_summaries, item_id, summary_index);
                    group.content.push_str(delta);
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.reasoning_summary_text.done" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    finish_reasoning_summary_group(
                        reasoning_summaries,
                        item_id,
                        summary_index,
                        event.get("text").and_then(Value::as_str),
                    );
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.reasoning_summary_part.done" => {
                if let (Some(item_id), Some(summary_index)) = (
                    event.get("item_id").and_then(Value::as_str),
                    event.get("summary_index").and_then(Value::as_u64),
                ) {
                    finish_reasoning_summary_group(
                        reasoning_summaries,
                        item_id,
                        summary_index,
                        event
                            .get("part")
                            .and_then(|part| part.get("text"))
                            .and_then(Value::as_str),
                    );
                    send_reasoning_snapshot(
                        events,
                        request_id,
                        conversation_id,
                        message_id,
                        content,
                        created_at,
                        reasoning_summaries,
                    )
                    .await?;
                }
            }
            "response.completed" => {
                finish_reasoning_summary_groups(reasoning_summaries);
                if let Some(response) = event.get("response") {
                    *output_tokens = response
                        .get("usage")
                        .and_then(|usage| usage.get("output_tokens"))
                        .and_then(Value::as_i64);
                    if let Some(items) = response
                        .get("output")
                        .and_then(Value::as_array)
                        .filter(|items| !items.is_empty())
                    {
                        response_output_items.clear();
                        response_output_items.extend(items.iter().cloned());
                    }
                }
                return Ok(true);
            }
            "response.failed" | "response.incomplete" | "error" => {
                return Err(ServiceError::new(
                    "provider_request_failed",
                    "ChatGPT could not complete the response. The partial answer was saved.",
                    false,
                ));
            }
            _ => {}
        }
        let _ = started;
        Ok(false)
    }

    async fn fetch_models(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<ChatGptModel>, ServiceError> {
        let external_workspace_id = self.external_workspace_id(connection_id, workspace_id)?;
        let mut url = url::Url::parse(&format!("{CHATGPT_CODEX_BASE}/models"))
            .map_err(|_| network_error())?;
        url.query_pairs_mut()
            .append_pair("client_version", CHATGPT_CLIENT_VERSION);
        let (_, value) = self
            .authorized_json_request(
                "models",
                Method::GET,
                url.into(),
                connection_id,
                &external_workspace_id,
                None,
            )
            .await?;
        parse_models(&value)
    }

    async fn refresh_usage(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<chatgpt_store::UsageSnapshot, ServiceError> {
        let external_workspace_id = self.external_workspace_id(connection_id, workspace_id)?;
        let usage_started = Instant::now();
        let usage_result = tokio::time::timeout(
            USAGE_REQUEST_TIMEOUT,
            self.authorized_json_request(
                "usage",
                Method::GET,
                format!("{CHATGPT_WHAM_BASE}/usage"),
                connection_id,
                &external_workspace_id,
                None,
            ),
        )
        .await;
        let usage = match usage_result {
            Ok(Ok((_, usage))) => usage,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                let error = provider_timeout();
                self.record_chatgpt_event(
                    "request_failed",
                    "usage",
                    None,
                    Some(error.code),
                    Some(usage_started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
        };

        let credits_started = Instant::now();
        let credit_result = tokio::time::timeout(
            RESET_CREDITS_REQUEST_TIMEOUT,
            self.authorized_json_request(
                "reset_credits",
                Method::GET,
                format!("{CHATGPT_WHAM_BASE}/rate-limit-reset-credits"),
                connection_id,
                &external_workspace_id,
                None,
            ),
        )
        .await;
        let usage_credit_count = usage
            .get("rate_limit_reset_credits")
            .and_then(|credits| credits.get("available_count"))
            .and_then(Value::as_i64);
        let (credits, credits_count, details_state) = match credit_result {
            Ok(Ok((_, value))) => match parse_reset_credits(&value) {
                Ok(result) => result,
                Err(error) => {
                    self.record_chatgpt_event(
                        "details_unavailable",
                        "reset_credits",
                        None,
                        Some(error.code),
                        Some(credits_started.elapsed().as_millis()),
                        None,
                    );
                    (Vec::new(), usage_credit_count, "unavailable")
                }
            },
            Ok(Err(error))
                if error.code == "authentication_required" || error.code == "refresh_rejected" =>
            {
                return Err(error);
            }
            Ok(Err(error)) => {
                self.record_chatgpt_event(
                    "details_unavailable",
                    "reset_credits",
                    None,
                    Some(error.code),
                    Some(credits_started.elapsed().as_millis()),
                    None,
                );
                (Vec::new(), usage_credit_count, "unavailable")
            }
            Err(_) => {
                let error = provider_timeout();
                self.record_chatgpt_event(
                    "details_unavailable",
                    "reset_credits",
                    None,
                    Some(error.code),
                    Some(credits_started.elapsed().as_millis()),
                    None,
                );
                (Vec::new(), usage_credit_count, "unavailable")
            }
        };
        let snapshot = parse_usage(
            &usage,
            connection_id,
            workspace_id,
            credits,
            credits_count,
            details_state,
        )?;
        if let Some(plan_type) = usage.get("plan_type").and_then(Value::as_str) {
            chatgpt_store::update_profile(
                &self.storage,
                connection_id,
                None,
                None,
                Some(plan_type),
            )
            .map_err(database_error)?;
            chatgpt_store::update_workspace_plan(
                &self.storage,
                connection_id,
                workspace_id,
                Some(plan_type),
            )
            .map_err(database_error)?;
        }
        chatgpt_store::save_usage_snapshot(&self.storage, snapshot).map_err(database_error)?;
        self.read_latest_usage(connection_id, workspace_id)
    }

    async fn generate_title(
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
            &job_id,
            conversation_id,
            &title_route.connection_id,
            &title_route.workspace_id,
            Some(&model.id),
            "queued",
            None,
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
        let response = match self
            .authorized_request(
                Method::POST,
                format!("{CHATGPT_CODEX_BASE}/responses"),
                &route.connection_id,
                &workspace.external_id,
                Some(json!({
                    "model": title_model.id,
                    "input": [{"role": "user", "content": context}],
                    "stream": true,
                    "store": false,
                })),
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
            &job_id,
            conversation_id,
            &route.connection_id,
            &route.workspace_id,
            None,
            "skipped",
            Some(reason),
        );
    }

    async fn authorized_request(
        &self,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
        response_context_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        let tokens = self.oauth.load_tokens(&self.storage, connection_id)?;
        let observed_access_token = Zeroizing::new(tokens.access_token().to_owned());
        let response = self
            .request_once(
                method.clone(),
                &url,
                external_workspace_id,
                &observed_access_token,
                body.as_ref(),
                response_context_id,
            )
            .await?;
        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }
        drop(response);
        self.refresh_if_unchanged(connection_id, &observed_access_token)
            .await?;
        let refreshed = self.oauth.load_tokens(&self.storage, connection_id)?;
        self.request_once(
            method,
            &url,
            external_workspace_id,
            refreshed.access_token(),
            body.as_ref(),
            response_context_id,
        )
        .await
        .and_then(|response| {
            if response.status() == StatusCode::UNAUTHORIZED {
                chatgpt_store::set_connection_auth_status(
                    &self.storage,
                    connection_id,
                    "reauth_required",
                )
                .map_err(database_error)?;
            }
            Ok(response)
        })
    }

    async fn authorized_stream_request(
        &self,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
        response_context_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Option<Response>, ServiceError> {
        if *cancellation.borrow() {
            return Ok(None);
        }
        let tokens = self.oauth.load_tokens(&self.storage, connection_id)?;
        let observed_access_token = Zeroizing::new(tokens.access_token().to_owned());
        let first_result = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Ok(None);
            }
            result = self.request_once(
                method.clone(),
                &url,
                external_workspace_id,
                &observed_access_token,
                body.as_ref(),
                Some(response_context_id),
            ) => result?,
        };
        if first_result.status() != StatusCode::UNAUTHORIZED {
            return Ok(Some(first_result));
        }
        drop(first_result);
        if *cancellation.borrow() {
            return Ok(None);
        }

        self.refresh_if_unchanged(connection_id, &observed_access_token)
            .await?;
        if *cancellation.borrow() {
            return Ok(None);
        }
        let refreshed = self.oauth.load_tokens(&self.storage, connection_id)?;
        let response = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Ok(None);
            }
            result = self.request_once(
                method,
                &url,
                external_workspace_id,
                refreshed.access_token(),
                body.as_ref(),
                Some(response_context_id),
            ) => result?,
        };
        if response.status() == StatusCode::UNAUTHORIZED {
            chatgpt_store::set_connection_auth_status(
                &self.storage,
                connection_id,
                "reauth_required",
            )
            .map_err(database_error)?;
        }
        Ok(Some(response))
    }

    async fn request_once(
        &self,
        method: Method,
        url: &str,
        external_workspace_id: &str,
        access_token: &str,
        body: Option<&Value>,
        response_context_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        let mut request = self
            .http
            .request(method, url)
            .bearer_auth(access_token)
            .header("chatgpt-account-id", external_workspace_id)
            .header("originator", "Codex");
        if let Some(context_id) = response_context_id {
            request = request
                .header(ACCEPT, "text/event-stream")
                .header("session-id", context_id)
                .header("thread-id", context_id)
                .header("x-client-request-id", context_id);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().await.map_err(|_| network_error())
    }

    async fn refresh_if_unchanged(
        &self,
        connection_id: &str,
        observed_access_token: &str,
    ) -> Result<(), ServiceError> {
        let lock = self.connection_refresh_lock(connection_id).await;
        let _guard = lock.lock().await;
        let current_tokens = self.oauth.load_tokens(&self.storage, connection_id)?;
        if current_tokens.access_token() != observed_access_token {
            return Ok(());
        }
        let refreshed = match self.oauth.refresh_tokens(&current_tokens).await {
            Ok(tokens) => tokens,
            Err(error) => {
                if error.code == "refresh_rejected" {
                    chatgpt_store::set_connection_auth_status(
                        &self.storage,
                        connection_id,
                        "reauth_required",
                    )
                    .map_err(database_error)?;
                }
                return Err(error);
            }
        };
        let reference = OAuthCredentialReference::new(
            connection_id.to_owned(),
            Uuid::new_v4().simple().to_string(),
        )
        .map_err(|_| {
            ServiceError::new(
                "credential_reference_invalid",
                "ChatGPT credentials could not be refreshed.",
                false,
            )
        })?;
        self.oauth.store_token_pair(&reference, &refreshed)?;
        let old_reference =
            match self
                .oauth
                .replace_stored_reference(&self.storage, connection_id, &reference)
            {
                Ok(Some(reference)) => reference,
                Ok(None) => {
                    let _ = self.oauth.delete_token_pair(&reference);
                    return Err(connection_unavailable());
                }
                Err(error) => {
                    let _ = self.oauth.delete_token_pair(&reference);
                    return Err(error);
                }
            };
        self.oauth.delete_token_pair(&old_reference)
    }

    async fn connection_refresh_lock(&self, connection_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.lock().await;
        Arc::clone(
            locks
                .entry(connection_id.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    fn ensure_active_connection(&self, connection_id: &str) -> Result<(), ServiceError> {
        let connections = chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
        if connections
            .iter()
            .any(|connection| connection.id == connection_id && connection.auth_status == "active")
        {
            Ok(())
        } else {
            Err(connection_unavailable())
        }
    }

    fn external_workspace_id(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<String, ServiceError> {
        chatgpt_store::workspace_external_id(&self.storage, connection_id, workspace_id)
            .map_err(database_error)?
            .ok_or_else(|| {
                ServiceError::new(
                    "workspace_not_found",
                    "The ChatGPT workspace for this conversation could not be found.",
                    false,
                )
            })
    }

    fn persist_terminal_message(
        &self,
        conversation_id: &str,
        message_id: &str,
        content: &str,
        status: &str,
        created_at: i64,
        output_tokens: Option<i64>,
        elapsed: Duration,
    ) -> Result<(), ServiceError> {
        let elapsed_microseconds = i64::try_from(elapsed.as_micros()).ok();
        let tokens_per_second = output_tokens.and_then(|tokens| {
            let seconds = elapsed.as_secs_f64();
            (seconds > 0.0).then_some(tokens as f64 / seconds)
        });
        chatgpt_store::save_assistant_message(
            &self.storage,
            conversation_id,
            message_id,
            content,
            status,
            created_at,
            output_tokens,
            tokens_per_second,
            elapsed_microseconds,
        )
        .map_err(database_error)
    }

    fn record_chatgpt_event(
        &self,
        event: &'static str,
        operation: &'static str,
        status: Option<u16>,
        code: Option<&'static str>,
        duration_ms: Option<u128>,
        item_count: Option<usize>,
    ) {
        let _ =
            self.storage
                .log_chatgpt_event(event, operation, status, code, duration_ms, item_count);
    }

    async fn authorized_json_request(
        &self,
        operation: &'static str,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
    ) -> Result<(u16, Value), ServiceError> {
        let started = Instant::now();
        let response = self
            .authorized_request(
                method,
                url,
                connection_id,
                external_workspace_id,
                body,
                None,
            )
            .await
            .map_err(|error| {
                self.record_chatgpt_event(
                    "request_failed",
                    operation,
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                error
            })?;
        let status = response.status().as_u16();
        self.record_chatgpt_event(
            "http_response",
            operation,
            Some(status),
            None,
            Some(started.elapsed().as_millis()),
            None,
        );
        response_json(response, MAX_JSON_BODY_BYTES)
            .await
            .map(|value| (status, value))
            .map_err(|error| {
                self.record_chatgpt_event(
                    "response_failed",
                    operation,
                    Some(status),
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                error
            })
    }

    fn read_latest_usage(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<chatgpt_store::UsageSnapshot, ServiceError> {
        chatgpt_store::latest_usage_snapshot(&self.storage, connection_id, workspace_id)
            .map_err(database_error)?
            .ok_or_else(|| {
                ServiceError::new(
                    "quota_unavailable",
                    "ChatGPT usage information is not available yet.",
                    true,
                )
            })
    }
}

async fn response_json(response: Response, max_bytes: usize) -> Result<Value, ServiceError> {
    ensure_success(response.status())?;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(invalid_response_error());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| network_error())?;
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(invalid_response_error());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| invalid_response_error())
}

fn parse_models(value: &Value) -> Result<Vec<ChatGptModel>, ServiceError> {
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response_error)?;
    let mut parsed = Vec::with_capacity(models.len());
    for model in models {
        let id = model
            .get("slug")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid_response_error)?
            .to_owned();
        let display_name = model
            .get("display_name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(&id)
            .to_owned();
        let reasoning_levels: Vec<String> = model
            .get("supported_reasoning_levels")
            .and_then(Value::as_array)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(|level| level.get("effort").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let context_window = model.get("context_window").and_then(Value::as_i64);
        let default_reasoning_level = model
            .get("default_reasoning_level")
            .and_then(Value::as_str)
            .filter(|level| reasoning_levels.iter().any(|supported| supported == *level))
            .map(str::to_owned);
        let supports_reasoning_summary_parameter = model
            .get("supports_reasoning_summary_parameter")
            .map(|supported| supported.as_bool().ok_or_else(invalid_response_error))
            .transpose()?
            .unwrap_or(true);
        parsed.push(ChatGptModel {
            id,
            display_name,
            description: model
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            context_window,
            default_reasoning_level,
            reasoning_levels,
            supports_reasoning_summary_parameter,
            is_available: model.get("visibility").and_then(Value::as_str) == Some("list")
                && model.get("supported_in_api").and_then(Value::as_bool) == Some(true),
        });
    }
    Ok(parsed)
}

fn parse_usage(
    value: &Value,
    connection_id: &str,
    workspace_id: &str,
    reset_credits: Vec<ResetCredit>,
    credit_count: Option<i64>,
    reset_credit_details_state: &str,
) -> Result<NewUsageSnapshot, ServiceError> {
    let rate_limit = value.get("rate_limit");
    let ordinary_usage_allowed = rate_limit
        .and_then(|limit| limit.get("allowed"))
        .and_then(Value::as_bool);
    let mut buckets = Vec::new();
    if let Some(limit) = rate_limit {
        add_rate_limit_buckets(&mut buckets, "codex", limit)?;
    }
    if let Some(additional_limits) = value
        .get("additional_rate_limits")
        .and_then(Value::as_array)
    {
        for limit in additional_limits {
            let id = limit
                .get("limit_name")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(invalid_response_error)?;
            if let Some(rate_limit) = limit.get("rate_limit") {
                add_rate_limit_buckets(&mut buckets, id, rate_limit)?;
            }
        }
    }
    let usage_credit_count = value
        .get("rate_limit_reset_credits")
        .and_then(|credits| credits.get("available_count"))
        .and_then(Value::as_i64);
    Ok(NewUsageSnapshot {
        id: Uuid::new_v4().simple().to_string(),
        connection_id: connection_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        fetched_at_unix_ms: now_unix_millis()?,
        freshness: "current".to_owned(),
        ordinary_usage_allowed,
        reset_credit_count: usage_credit_count.or(credit_count),
        reset_credit_details_state: reset_credit_details_state.to_owned(),
        buckets,
        reset_credits,
    })
}

fn add_rate_limit_buckets(
    buckets: &mut Vec<UsageBucket>,
    limit_id: &str,
    rate_limit: &Value,
) -> Result<(), ServiceError> {
    for (window_name, suffix) in [
        ("primary_window", "primary"),
        ("secondary_window", "secondary"),
    ] {
        let Some(window) = rate_limit.get(window_name).filter(|value| !value.is_null()) else {
            continue;
        };
        let used_percent = window.get("used_percent").and_then(Value::as_f64);
        if used_percent.is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value)) {
            return Err(invalid_response_error());
        }
        let reset_at_unix_ms = window
            .get("reset_at")
            .and_then(Value::as_i64)
            .and_then(|seconds| seconds.checked_mul(1000));
        buckets.push(UsageBucket {
            limit_id: format!("{limit_id}:{suffix}"),
            used_percent,
            window_seconds: window.get("limit_window_seconds").and_then(Value::as_i64),
            reset_at_unix_ms,
        });
    }
    Ok(())
}

fn parse_reset_credits(
    value: &Value,
) -> Result<(Vec<ResetCredit>, Option<i64>, &'static str), ServiceError> {
    let available_count = value.get("available_count").and_then(Value::as_i64);
    let Some(credits_value) = value.get("credits") else {
        return Ok((Vec::new(), available_count, "omitted"));
    };
    if credits_value.is_null() {
        return Ok((Vec::new(), available_count, "omitted"));
    }
    let credits = credits_value
        .as_array()
        .ok_or_else(invalid_response_error)?;
    let mut parsed = Vec::with_capacity(credits.len());
    for credit in credits {
        let id = credit
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid_response_error)?
            .to_owned();
        parsed.push(ResetCredit {
            id,
            reset_type: credit
                .get("reset_type")
                .and_then(Value::as_str)
                .map(str::to_owned),
            status: credit
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_owned),
            granted_at_unix_ms: parse_timestamp(credit.get("granted_at")),
            expires_at_unix_ms: parse_timestamp(credit.get("expires_at")),
            title: credit
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_owned),
            description: credit
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    Ok((parsed, available_count, "available"))
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(seconds) = value.as_i64() {
        return seconds.checked_mul(1000);
    }
    let timestamp = value.as_str()?;
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    i64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).ok()
}

fn ensure_success(status: StatusCode) -> Result<(), ServiceError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(http_error(status))
    }
}

fn http_error(status: StatusCode) -> ServiceError {
    match status {
        StatusCode::UNAUTHORIZED => ServiceError::new(
            "authentication_required",
            "ChatGPT needs you to sign in to this account again.",
            false,
        ),
        StatusCode::FORBIDDEN => ServiceError::new(
            "permission_denied",
            "This ChatGPT account or workspace does not allow the requested operation.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            "ChatGPT has reached a usage limit for this account. Wait for its reset or choose another account.",
            false,
        ),
        StatusCode::NOT_FOUND => ServiceError::new(
            "provider_endpoint_unavailable",
            "The ChatGPT endpoint is unavailable or has changed.",
            false,
        ),
        status if status.is_server_error() => ServiceError::new(
            "provider_unavailable",
            "ChatGPT is temporarily unavailable. Try again later.",
            true,
        ),
        _ => ServiceError::new(
            "provider_request_failed",
            "ChatGPT rejected the request. Check the account and model selection.",
            false,
        ),
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn now_unix_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        ServiceError::new(
            "system_clock_invalid",
            "The system clock is invalid.",
            false,
        )
    })?;
    i64::try_from(elapsed.as_millis()).map_err(|_| {
        ServiceError::new(
            "system_clock_invalid",
            "The system clock is outside the supported range.",
            false,
        )
    })
}

fn parse_responses_tool_calls(output_items: &[Value]) -> Result<Vec<ToolCall>, ServiceError> {
    output_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|item| {
            let id = item
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(invalid_response_error)?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(invalid_response_error)?;
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .ok_or_else(invalid_response_error)?;
            if arguments.len() > tools::MAX_TOOL_ARGUMENT_BYTES {
                return Err(invalid_response_error());
            }
            let arguments =
                serde_json::from_str(arguments).map_err(|_| invalid_response_error())?;
            Ok(ToolCall {
                id: id.to_owned(),
                name: name.to_owned(),
                arguments,
            })
        })
        .collect()
}

fn response_output_text(output_items: &[Value]) -> String {
    let mut text = String::new();
    for item in output_items {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in content {
            match part.get("type").and_then(Value::as_str) {
                Some("output_text") => {
                    if let Some(value) = part.get("text").and_then(Value::as_str) {
                        text.push_str(value);
                    }
                }
                Some("refusal") => {
                    if let Some(value) = part.get("refusal").and_then(Value::as_str) {
                        text.push_str(value);
                    }
                }
                _ => {}
            }
        }
    }
    text
}

fn empty_response_shape(output_items: &[Value]) -> &'static str {
    if output_items.is_empty() {
        return "no_output_items";
    }
    if output_items
        .iter()
        .all(|item| item.get("type").and_then(Value::as_str) == Some("reasoning"))
    {
        return "reasoning_only";
    }
    if output_items
        .iter()
        .any(|item| item.get("type").and_then(Value::as_str) == Some("message"))
    {
        return "message_without_visible_text";
    }
    "other_output_items"
}

fn responses_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "strict": false,
        "parameters": tool.parameters.clone(),
    })
}

fn database_error(_: DatabaseError) -> ServiceError {
    ServiceError::new(
        "local_storage_failed",
        "ChatGPT data could not be read from or written to the local database.",
        true,
    )
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "ChatGPT could not be reached. Check the internet connection and try again.",
        true,
    )
}

fn provider_timeout() -> ServiceError {
    ServiceError::new(
        "provider_timeout",
        "ChatGPT did not return this information in time. Try again.",
        true,
    )
}

fn request_cancelled() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The ChatGPT information request was cancelled.",
        true,
    )
}

fn connection_unavailable() -> ServiceError {
    ServiceError::new(
        "connection_unavailable",
        "The ChatGPT connection is unavailable. Sign in again or choose another connection.",
        false,
    )
}

fn invalid_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "ChatGPT returned data that Zihora could not read. The provider response format may have changed.",
        false,
    )
}

fn invalid_retry_target_error() -> ServiceError {
    ServiceError::new(
        "invalid_retry_target",
        "The selected response can no longer be retried. Refresh the conversation and try again.",
        false,
    )
}

fn serialization_error() -> ServiceError {
    ServiceError::new(
        "serialization_failed",
        "ChatGPT data could not be returned to the app.",
        false,
    )
}

fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_output_failed",
        "Zihora could not update the local app with the ChatGPT response.",
        false,
    )
}
