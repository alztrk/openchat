use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::{Client, Method, Response, StatusCode, header::ACCEPT};
use rusqlite::Error as DatabaseError;
use serde_json::{Value, json};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt_store::{self, ChatGptModel},
    credentials::{OAuthCredentialReference, OAuthTokenPair},
    instructions,
    oauth::OAuthClient,
    protocol::ServiceError,
    provider_schema::ProviderChatRequest,
    storage::AppStorage,
    tools,
};

mod account;
mod compaction;
mod images;
mod response_events;
mod response_parser;
mod streaming;
mod title_generation;
pub(super) use self::images::{
    ImageBackground, ImageGenerationAuth, ImageGenerationRequest, ImageQuality,
};
use self::response_parser::{
    ensure_success, http_error, parse_models, parse_reset_credits, parse_usage, responses_tool,
};

const CHATGPT_CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";
const CHATGPT_WHAM_BASE: &str = "https://chatgpt.com/backend-api/wham";
const CHATGPT_FAST_SERVICE_TIER: &str = "priority";
// The private catalog filters entries by its Codex client compatibility version, not OpenChat's product version.
const CHATGPT_CLIENT_VERSION: &str = "0.157.0";
const MODEL_CATALOG_CACHE_AGE: Duration = Duration::from_secs(6 * 60 * 60);
const METADATA_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const USAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const RESET_CREDITS_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const RESET_CREDIT_CONSUME_TIMEOUT: Duration = Duration::from_secs(10);
const ACCESS_TOKEN_REFRESH_WINDOW: Duration = Duration::from_secs(5 * 60);
const MAX_JSON_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_STREAM_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct ChatGptService {
    storage: Arc<AppStorage>,
    oauth: Arc<OAuthClient>,
    http: Client,
    refresh_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

pub(super) struct StreamRequest<'a> {
    method: Method,
    url: String,
    connection_id: &'a str,
    external_workspace_id: &'a str,
    body: Option<Value>,
    response_context_id: &'a str,
}

impl ChatGptService {
    pub fn new(storage: Arc<AppStorage>) -> Result<Self, ServiceError> {
        let http = Client::builder()
            .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
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

    pub async fn send_message(
        self: &Arc<Self>,
        context: ChatSendContext<'_>,
        reasoning_effort: Option<&str>,
        fast_mode: bool,
    ) -> Result<Value, ServiceError> {
        let conversation_id = context.conversation_id;
        let excluded_assistant_message_id = context.excluded_assistant_message_id;
        let custom_instructions = context.custom_instructions;
        let project_root = context.project_root;
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
        if fast_mode && !model.supports_fast_mode {
            return Err(ServiceError::new(
                "fast_mode_unsupported",
                "Fast mode is not available for the selected ChatGPT model.",
                false,
            ));
        }
        let context_state =
            chatgpt_store::load_conversation_context_state(&self.storage, conversation_id)
                .map_err(database_error)?;
        let active_compaction_route_matches = context_state.as_ref().is_some_and(|state| {
            crate::context_compaction::compaction_payload_matches_route(
                state,
                "chatgpt",
                Some(&route.connection_id),
                Some(&route.workspace_id),
                &model.id,
            )
        });
        let messages = if active_compaction_route_matches
            && let Some(boundary_id) = context_state
                .as_ref()
                .and_then(|state| state.compacted_through_message_id.as_deref())
        {
            match chatgpt_store::conversation_messages_from_boundary(
                &self.storage,
                conversation_id,
                boundary_id,
            )
            .map_err(database_error)?
            {
                Some(messages) => messages,
                None => chatgpt_store::conversation_messages(&self.storage, conversation_id)
                    .map_err(database_error)?,
            }
        } else {
            chatgpt_store::conversation_messages(&self.storage, conversation_id)
                .map_err(database_error)?
        };
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
        let included_messages = messages
            .iter()
            .filter(|message| Some(message.id.as_str()) != excluded_assistant_message_id)
            .collect::<Vec<_>>();
        crate::history::validate_model_attachments(&messages, model.supports_images)?;
        let last_message_id = included_messages.last().map(|message| message.id.clone());
        let tools = tools::definitions_for_chatgpt_model();
        let provider_request = ProviderChatRequest {
            model: model.id.clone(),
            instructions: instructions::shared_instructions(
                custom_instructions,
                context.permission_mode,
                project_root.is_some(),
                !tools.is_empty(),
                "chatgpt",
            ),
            messages: Vec::new(),
            last_message_id,
            tools,
            reasoning_effort: reasoning_effort
                .filter(|effort| model.reasoning_levels.iter().any(|level| level == *effort))
                .map(str::to_owned),
        };
        let compaction_messages = included_messages
            .iter()
            .map(|message| (*message).clone())
            .collect::<Vec<_>>();
        let input = compaction::prepare_input(
            compaction::PrepareInputRequest {
                service: self.as_ref(),
                storage: &self.storage,
                conversation_id,
                route: &route,
                external_workspace_id: &workspace.external_id,
                model: &model,
                instructions: &provider_request.instructions,
                tools: &provider_request.tools,
                reasoning_effort: provider_request.reasoning_effort.as_deref(),
                messages: &compaction_messages,
            },
            &mut *context.cancellation,
        )
        .await?;
        let mut payload = json!({
            "model": provider_request.model.as_str(),
            "input": input,
            "instructions": provider_request.instructions,
            "stream": true,
            "store": false,
        });
        if fast_mode {
            payload["service_tier"] = json!(CHATGPT_FAST_SERVICE_TIER);
        }
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
            context,
            &route,
            &workspace.external_id,
            &model,
            provider_request.last_message_id.as_deref(),
            payload,
        )
        .await
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

    async fn authorized_request(
        &self,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
        response_context_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        self.authorized_request_with_image_turn_id(
            method,
            url,
            connection_id,
            external_workspace_id,
            body,
            response_context_id,
            None,
        )
        .await
    }

    async fn authorized_image_request(
        &self,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
        image_turn_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        self.authorized_request_with_image_turn_id(
            method,
            url,
            connection_id,
            external_workspace_id,
            body,
            None,
            image_turn_id,
        )
        .await
    }

    async fn authorized_request_with_image_turn_id(
        &self,
        method: Method,
        url: String,
        connection_id: &str,
        external_workspace_id: &str,
        body: Option<Value>,
        response_context_id: Option<&str>,
        image_turn_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        let tokens = self.load_request_tokens(connection_id).await?;
        let observed_access_token = Zeroizing::new(tokens.access_token().to_owned());
        let response = self
            .request_once(
                method.clone(),
                &url,
                external_workspace_id,
                &observed_access_token,
                body.as_ref(),
                response_context_id,
                image_turn_id,
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
            image_turn_id,
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
        request: StreamRequest<'_>,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Option<Response>, ServiceError> {
        if *cancellation.borrow() {
            return Ok(None);
        }
        let tokens = self.load_request_tokens(request.connection_id).await?;
        if *cancellation.borrow() {
            return Ok(None);
        }
        let observed_access_token = Zeroizing::new(tokens.access_token().to_owned());
        let first_result = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Ok(None);
            }
            result = self.request_once(
                request.method.clone(),
                &request.url,
                request.external_workspace_id,
                &observed_access_token,
                request.body.as_ref(),
                Some(request.response_context_id),
                None,
            ) => result?,
        };
        if first_result.status() != StatusCode::UNAUTHORIZED {
            return Ok(Some(first_result));
        }
        drop(first_result);
        if *cancellation.borrow() {
            return Ok(None);
        }

        self.refresh_if_unchanged(request.connection_id, &observed_access_token)
            .await?;
        if *cancellation.borrow() {
            return Ok(None);
        }
        let refreshed = self
            .oauth
            .load_tokens(&self.storage, request.connection_id)?;
        let response = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Ok(None);
            }
            result = self.request_once(
                request.method,
                &request.url,
                request.external_workspace_id,
                refreshed.access_token(),
                request.body.as_ref(),
                Some(request.response_context_id),
                None,
            ) => result?,
        };
        if response.status() == StatusCode::UNAUTHORIZED {
            chatgpt_store::set_connection_auth_status(
                &self.storage,
                request.connection_id,
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
        image_turn_id: Option<&str>,
    ) -> Result<Response, ServiceError> {
        let mut request = self
            .http
            .request(method, url)
            .bearer_auth(access_token)
            .header("chatgpt-account-id", external_workspace_id)
            .header("originator", "Codex");
        if let Some(turn_metadata) = body
            .and_then(|body| body.pointer("/client_metadata/x-codex-turn-metadata"))
            .and_then(Value::as_str)
        {
            request = request.header("x-codex-turn-metadata", turn_metadata);
        }
        if let Some(context_id) = response_context_id {
            request = request
                .header(ACCEPT, "text/event-stream")
                .header("session-id", context_id)
                .header("thread-id", context_id)
                .header("x-client-request-id", context_id);
        }
        request = apply_image_turn_id_header(request, image_turn_id);
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().await.map_err(|error| {
            if error.is_timeout() {
                provider_timeout()
            } else {
                network_error()
            }
        })
    }

    async fn load_request_tokens(
        &self,
        connection_id: &str,
    ) -> Result<OAuthTokenPair, ServiceError> {
        let tokens = self.oauth.load_tokens(&self.storage, connection_id)?;
        if !tokens.expires_within(SystemTime::now(), ACCESS_TOKEN_REFRESH_WINDOW) {
            return Ok(tokens);
        }

        let observed_access_token = Zeroizing::new(tokens.access_token().to_owned());
        self.refresh_if_unchanged(connection_id, &observed_access_token)
            .await?;
        self.oauth.load_tokens(&self.storage, connection_id)
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
        message: chatgpt_store::AssistantMessageWrite<'_>,
    ) -> Result<(), ServiceError> {
        chatgpt_store::save_assistant_message(&self.storage, message).map_err(database_error)
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
            .inspect_err(|error| {
                self.record_chatgpt_event(
                    "request_failed",
                    operation,
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
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
            .inspect_err(|error| {
                self.record_chatgpt_event(
                    "response_failed",
                    operation,
                    Some(status),
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
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

pub(super) fn database_error(_: DatabaseError) -> ServiceError {
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

fn apply_image_turn_id_header(
    request: reqwest::RequestBuilder,
    image_turn_id: Option<&str>,
) -> reqwest::RequestBuilder {
    match image_turn_id {
        Some(image_turn_id) => request.header("x-codex-image-turn-id", image_turn_id),
        None => request,
    }
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
        "ChatGPT returned data that OpenChat could not read. The provider response format may have changed.",
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
        "OpenChat could not update the local app with the ChatGPT response.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::apply_image_turn_id_header;
    use reqwest::Client;

    #[test]
    fn image_turn_header_is_added_only_for_codex_correlation() {
        let request = apply_image_turn_id_header(
            Client::new().post("https://chatgpt.com/backend-api/codex/images/generations"),
            Some("turn-123"),
        )
        .build()
        .expect("request should build");
        assert_eq!(
            request
                .headers()
                .get("x-codex-image-turn-id")
                .and_then(|value| value.to_str().ok()),
            Some("turn-123")
        );

        let request = apply_image_turn_id_header(
            Client::new().post("https://api.openai.com/v1/images/generations"),
            None,
        )
        .build()
        .expect("request should build");
        assert!(request.headers().get("x-codex-image-turn-id").is_none());
    }
}
