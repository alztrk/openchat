use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header::ACCEPT};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    instructions,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{
        ChatStreamEvent, ChatStreamSnapshot, ProviderChatRequest, ProviderMessage, ToolCall,
        ToolDefinition,
    },
    storage::AppStorage,
    tools::{self, ToolExecutor, ToolPermissionMode},
};

const MODELS_URL: &str = "https://opencode.ai/inference/v1/models";
const CHAT_URL: &str = "https://opencode.ai/inference/openai/v1/chat/completions";
const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MODEL_CATALOG_CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const SUPPORTED_FREE_CHAT_MODELS: &[&str] = &[
    "big-pickle",
    "deepseek-v4-flash-free",
    "ling-3.0-flash-fin-free",
    "longcat-2.5-preview-free",
    "mimo-v2.5-free",
    "mimo-v2.6-flash-free",
    "nemotron-3-ultra-free",
    "nemotron-3.5-lightning-free",
    "space-bunny-free",
];
const SUPPORTED_PAID_CHAT_MODELS: &[&str] = &[
    "deepseek-v4.1-flash",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "deepseek-v4-flash-vision-exp",
    "minimax-m3",
    "minimax-m2.7",
    "minimax-m2.5",
    "glm-5.3-flash",
    "glm-5.3",
    "glm-5.2",
    "glm-5.1",
    "glm-5",
    "kimi-k2.7-code",
    "kimi-k3",
    "kimi-k2.6",
    "kimi-k2.5",
    "qwen3.8-max",
];

#[derive(Default)]
struct StreamedToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| storage_error())
}

fn load_model_catalog(storage: &AppStorage) -> Result<Option<(Vec<Value>, i64)>, ServiceError> {
    let database = storage.connect().map_err(|_| storage_error())?;
    let cached = database
        .query_row(
            "SELECT models_json, fetched_at_unix_ms
             FROM opencode_model_catalog WHERE catalog_id = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|_| storage_error())?;
    cached
        .map(|(models_json, fetched_at)| {
            serde_json::from_str::<Vec<Value>>(&models_json)
                .map(|models| (models, fetched_at))
                .map_err(|_| invalid_response_error())
        })
        .transpose()
}

fn save_model_catalog(storage: &AppStorage, models: &[Value]) -> Result<(), ServiceError> {
    let models_json = serde_json::to_string(models).map_err(|_| invalid_response_error())?;
    storage
        .connect()
        .map_err(|_| storage_error())?
        .execute(
            "INSERT INTO opencode_model_catalog (catalog_id, fetched_at_unix_ms, models_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(catalog_id) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                models_json = excluded.models_json",
            rusqlite::params![current_time_millis()?, models_json],
        )
        .map_err(|_| storage_error())?;
    Ok(())
}

fn supported_models(value: &Value) -> Result<Vec<Value>, ServiceError> {
    value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response_error)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let id = model.get("id")?.as_str()?;
                    let is_free = is_supported_free_chat_model(id);
                    if !is_free && !is_supported_paid_chat_model(id) {
                        return None;
                    }
                    Some(json!({
                        "id": id,
                        "displayName": id,
                        "description": if is_free { "free" } else { "paid" },
                        "contextWindow": null,
                        "defaultReasoningLevel": null,
                        "reasoningLevels": [],
                        "isAvailable": true,
                    }))
                })
                .collect()
        })
}

fn visible_models(models: &[Value], api_key: Option<&str>) -> Vec<Value> {
    models
        .iter()
        .filter(|model| {
            model.get("description").and_then(Value::as_str) != Some("paid") || api_key.is_some()
        })
        .cloned()
        .collect()
}

fn models_response(models: &[Value], freshness: &str) -> Value {
    json!({"models": models, "freshness": freshness})
}

pub async fn models(
    storage: &AppStorage,
    api_key: Option<&str>,
    force_refresh: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let cached = load_model_catalog(storage)?;
    if !force_refresh {
        if let Some((models, fetched_at)) = &cached {
            if current_time_millis()?.saturating_sub(*fetched_at) < MODEL_CATALOG_CACHE_AGE_MS {
                return Ok(models_response(&visible_models(models, api_key), "current"));
            }
        }
    }

    let mut request = client()?.get(MODELS_URL);
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    let fetch = async {
        let response = request.send().await.map_err(|_| network_error())?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| invalid_response_error())?;
        let models = supported_models(&value)?;
        save_model_catalog(storage, &models)?;
        Ok::<_, ServiceError>(models)
    };
    let fetch_result = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Err(cancelled_error())
        }
        result = fetch => result,
    };

    match fetch_result {
        Ok(models) => Ok(models_response(
            &visible_models(&models, api_key),
            "current",
        )),
        Err(error) if error.code == "request_cancelled" => Err(error),
        Err(error) => match cached {
            Some((models, _)) => Ok(models_response(&visible_models(&models, api_key), "stale")),
            None => Err(error),
        },
    }
}

pub async fn send_message(
    storage: &AppStorage,
    request_id: Value,
    conversation_id: &str,
    excluded_assistant_message_id: Option<&str>,
    api_key: Option<&str>,
    custom_instructions: Option<&str>,
    project_root: Option<&Path>,
    data_root: &Path,
    permission_mode: ToolPermissionMode,
    permission_broker: &ToolPermissionBroker,
    cancellation: &mut watch::Receiver<bool>,
    events: EventSink,
) -> Result<Value, ServiceError> {
    let route = storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT provider_id, model_id FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()
        .map_err(|_| storage_error())?
        .ok_or_else(route_error)?;
    let (provider_id, model_id) = route;
    if provider_id.as_deref() != Some("opencode") {
        return Err(route_error());
    }
    let model_id = model_id
        .filter(|id| is_supported_free_chat_model(id) || is_supported_paid_chat_model(id))
        .ok_or_else(model_error)?;
    let is_free = is_supported_free_chat_model(&model_id);
    if !is_free && api_key.is_none() {
        return Err(authentication_required_error());
    }

    let stored_messages = crate::chatgpt_store::conversation_messages(storage, conversation_id)
        .map_err(|_| storage_error())?;
    if excluded_assistant_message_id.is_some_and(|id| {
        !crate::chatgpt_store::is_retryable_latest_assistant_message(&stored_messages, id)
    }) {
        return Err(invalid_retry_target_error());
    }
    let history = stored_messages
        .into_iter()
        .filter(|message| Some(message.id.as_str()) != excluded_assistant_message_id)
        .map(|message| ProviderMessage::from_history(&message.role, &message.content))
        .collect::<Result<Vec<_>, _>>()?;
    if history.is_empty() {
        return Err(ServiceError::new(
            "conversation_empty",
            "Write a message before starting the response.",
            false,
        ));
    }
    let provider_request = ProviderChatRequest {
        model: model_id,
        instructions: instructions::shared_instructions(
            custom_instructions,
            permission_mode,
            project_root.is_some(),
        ),
        messages: history,
        tools: tools::definitions(),
        reasoning_effort: None,
    };
    let mut messages = Vec::with_capacity(provider_request.messages.len() + 1);
    messages.push(json!({
        "role": "system",
        "content": provider_request.instructions,
    }));
    messages.extend(
        provider_request
            .messages
            .iter()
            .map(|message| json!({"role": message.role.as_str(), "content": message.content})),
    );

    let started = Instant::now();
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?
        .as_millis();
    let created_at = i64::try_from(created_at).map_err(|_| storage_error())?;
    let message_id = Uuid::new_v4().simple().to_string();
    save_message(
        storage,
        conversation_id,
        &message_id,
        "",
        "streaming",
        created_at,
        None,
        None,
    )?;
    if events
        .send(
            &ChatStreamEvent::Started(ChatStreamSnapshot::new(
                conversation_id,
                &message_id,
                "",
                created_at,
            ))
            .into_rpc(request_id.clone()),
        )
        .await
        .is_err()
    {
        save_message(
            storage,
            conversation_id,
            &message_id,
            "",
            "failed",
            created_at,
            None,
            Some(started.elapsed()),
        )?;
        return Err(protocol_error());
    }

    let mut tool_executor = ToolExecutor::new(project_root, data_root, permission_mode);
    let mut content = String::new();
    let mut output_tokens = None;
    let mut stopped = false;
    'model_turn: loop {
        let mut body = json!({
            "model": provider_request.model.as_str(),
            "messages": messages,
            "stream": true,
        });
        if !provider_request.tools.is_empty() {
            body["tools"] = json!(
                provider_request
                    .tools
                    .iter()
                    .map(chat_completion_tool)
                    .collect::<Vec<_>>()
            );
            body["tool_choice"] = json!("auto");
            body["parallel_tool_calls"] = json!(false);
        }
        let response = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                save_message(storage, conversation_id, &message_id, &content, "stopped", created_at, output_tokens, Some(started.elapsed()))?;
                return Ok(terminal_result(conversation_id, &message_id, "stopped", started.elapsed(), output_tokens));
            }
            response = {
                let mut request = client()?.post(CHAT_URL);
                if !is_free {
                    if let Some(api_key) = api_key {
                        request = request.bearer_auth(api_key);
                    }
                }
                request
                .header(ACCEPT, "text/event-stream")
                .json(&body)
                .send()
            } => match response {
                Ok(response) => response,
                Err(_) => {
                    save_message(
                        storage,
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        None,
                        Some(started.elapsed()),
                    )?;
                    return Err(network_error());
                }
            },
        };
        if !response.status().is_success() {
            save_message(
                storage,
                conversation_id,
                &message_id,
                &content,
                "failed",
                created_at,
                None,
                Some(started.elapsed()),
            )?;
            return Err(http_error(response.status()));
        }

        let mut stream = response.bytes_stream();
        let mut pending = Vec::new();
        let mut round_content = String::new();
        let mut tool_calls = BTreeMap::<usize, StreamedToolCall>::new();
        let mut saw_done = false;
        loop {
            let next = tokio::select! {
                changed = cancellation.changed() => {
                    let _ = changed;
                    stopped = true;
                    break;
                }
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = next else { break };
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(_) => {
                    save_message(
                        storage,
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        Some(started.elapsed()),
                    )?;
                    return Err(network_error());
                }
            };
            pending.extend_from_slice(&chunk);
            if pending.len() > MAX_EVENT_BYTES {
                save_message(
                    storage,
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    Some(started.elapsed()),
                )?;
                return Err(invalid_response_error());
            }
            while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                let mut line = pending.drain(..=newline).collect::<Vec<_>>();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let line = match String::from_utf8(line) {
                    Ok(line) => line,
                    Err(_) => {
                        save_message(
                            storage,
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            output_tokens,
                            Some(started.elapsed()),
                        )?;
                        return Err(invalid_response_error());
                    }
                };
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim_start();
                    if data.is_empty() {
                        continue;
                    }
                    if data == "[DONE]" {
                        saw_done = true;
                        continue;
                    }
                    let value: Value = match serde_json::from_str(data) {
                        Ok(value) => value,
                        Err(_) => {
                            save_message(
                                storage,
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                Some(started.elapsed()),
                            )?;
                            return Err(invalid_response_error());
                        }
                    };
                    if let Some(text) = value
                        .pointer("/choices/0/delta/content")
                        .and_then(Value::as_str)
                    {
                        content.push_str(text);
                        round_content.push_str(text);
                        if events
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
                            .is_err()
                        {
                            save_message(
                                storage,
                                conversation_id,
                                &message_id,
                                &content,
                                "failed",
                                created_at,
                                output_tokens,
                                Some(started.elapsed()),
                            )?;
                            return Err(protocol_error());
                        }
                    }
                    if let Some(tokens) = value
                        .pointer("/usage/completion_tokens")
                        .and_then(Value::as_i64)
                    {
                        output_tokens = Some(output_tokens.unwrap_or(0).saturating_add(tokens));
                    }
                    if let Some(deltas) = value
                        .pointer("/choices/0/delta/tool_calls")
                        .and_then(Value::as_array)
                    {
                        for delta in deltas {
                            let Some(index) = delta
                                .get("index")
                                .and_then(Value::as_u64)
                                .and_then(|value| usize::try_from(value).ok())
                            else {
                                save_message(
                                    storage,
                                    conversation_id,
                                    &message_id,
                                    &content,
                                    "failed",
                                    created_at,
                                    output_tokens,
                                    Some(started.elapsed()),
                                )?;
                                return Err(invalid_response_error());
                            };
                            if !tool_calls.contains_key(&index)
                                && tool_calls.len() >= tools::MAX_TOOL_CALLS_PER_TURN
                            {
                                save_message(
                                    storage,
                                    conversation_id,
                                    &message_id,
                                    &content,
                                    "failed",
                                    created_at,
                                    output_tokens,
                                    Some(started.elapsed()),
                                )?;
                                return Err(tools::tool_call_limit_error());
                            }
                            let call = tool_calls.entry(index).or_default();
                            if let Some(id) = delta.get("id").and_then(Value::as_str) {
                                call.id.push_str(id);
                            }
                            if let Some(function) = delta.get("function") {
                                if let Some(name) = function.get("name").and_then(Value::as_str) {
                                    call.name.push_str(name);
                                }
                                if let Some(arguments) =
                                    function.get("arguments").and_then(Value::as_str)
                                {
                                    if call.arguments.len().saturating_add(arguments.len())
                                        > tools::MAX_TOOL_ARGUMENT_BYTES
                                    {
                                        save_message(
                                            storage,
                                            conversation_id,
                                            &message_id,
                                            &content,
                                            "failed",
                                            created_at,
                                            output_tokens,
                                            Some(started.elapsed()),
                                        )?;
                                        return Err(invalid_response_error());
                                    }
                                    call.arguments.push_str(arguments);
                                }
                            }
                        }
                    }
                }
            }
        }
        if stopped {
            break 'model_turn;
        }
        let elapsed = started.elapsed();
        if !saw_done {
            save_message(
                storage,
                conversation_id,
                &message_id,
                &content,
                "failed",
                created_at,
                output_tokens,
                Some(elapsed),
            )?;
            return Err(invalid_response_error());
        }
        let tool_calls = match parse_streamed_tool_calls(tool_calls) {
            Ok(tool_calls) => tool_calls,
            Err(error) => {
                save_message(
                    storage,
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    Some(elapsed),
                )?;
                return Err(error);
            }
        };
        if !tool_calls.is_empty() {
            if let Err(error) = tool_executor.begin_round(&tool_calls) {
                save_message(
                    storage,
                    conversation_id,
                    &message_id,
                    &content,
                    "failed",
                    created_at,
                    output_tokens,
                    Some(elapsed),
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
                        save_message(
                            storage,
                            conversation_id,
                            &message_id,
                            &content,
                            "stopped",
                            created_at,
                            output_tokens,
                            Some(elapsed),
                        )?;
                        return Ok(terminal_result(
                            conversation_id,
                            &message_id,
                            "stopped",
                            elapsed,
                            output_tokens,
                        ));
                    }
                    Err(error) => {
                        save_message(
                            storage,
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            output_tokens,
                            Some(elapsed),
                        )?;
                        return Err(error);
                    }
                };
                results.push(result);
            }
            let assistant_calls = tool_calls
                .iter()
                .map(|call| {
                    let arguments = serde_json::to_string(&call.arguments)
                        .map_err(|_| invalid_response_error())?;
                    Ok(json!({
                        "id": call.id,
                        "type": "function",
                        "function": {"name": call.name, "arguments": arguments}
                    }))
                })
                .collect::<Result<Vec<_>, ServiceError>>();
            let assistant_calls = match assistant_calls {
                Ok(assistant_calls) => assistant_calls,
                Err(error) => {
                    save_message(
                        storage,
                        conversation_id,
                        &message_id,
                        &content,
                        "failed",
                        created_at,
                        output_tokens,
                        Some(elapsed),
                    )?;
                    return Err(error);
                }
            };
            messages.push(json!({
                "role": "assistant",
                "content": if round_content.is_empty() { Value::Null } else { json!(round_content) },
                "tool_calls": assistant_calls
            }));
            for result in results {
                let output = match serde_json::to_string(&result.output) {
                    Ok(output) => output,
                    Err(_) => {
                        save_message(
                            storage,
                            conversation_id,
                            &message_id,
                            &content,
                            "failed",
                            created_at,
                            output_tokens,
                            Some(elapsed),
                        )?;
                        return Err(invalid_response_error());
                    }
                };
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": result.call_id,
                    "content": output
                }));
            }
            continue 'model_turn;
        }
        break 'model_turn;
    }
    let elapsed = started.elapsed();
    let status = if stopped { "stopped" } else { "completed" };
    save_message(
        storage,
        conversation_id,
        &message_id,
        &content,
        status,
        created_at,
        output_tokens,
        Some(elapsed),
    )?;
    Ok(terminal_result(
        conversation_id,
        &message_id,
        status,
        elapsed,
        output_tokens,
    ))
}

fn parse_streamed_tool_calls(
    calls: BTreeMap<usize, StreamedToolCall>,
) -> Result<Vec<ToolCall>, ServiceError> {
    calls
        .into_values()
        .map(|call| {
            if call.id.is_empty() || call.name.is_empty() {
                return Err(invalid_response_error());
            }
            Ok(ToolCall {
                id: call.id,
                name: call.name,
                arguments: serde_json::from_str(&call.arguments)
                    .map_err(|_| invalid_response_error())?,
            })
        })
        .collect()
}

fn chat_completion_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters.clone(),
        }
    })
}

fn is_supported_free_chat_model(id: &str) -> bool {
    SUPPORTED_FREE_CHAT_MODELS.contains(&id)
}

fn is_supported_paid_chat_model(id: &str) -> bool {
    SUPPORTED_PAID_CHAT_MODELS.contains(&id)
}

fn client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent(concat!("Zihora/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| network_error())
}

fn save_message(
    storage: &AppStorage,
    conversation_id: &str,
    message_id: &str,
    content: &str,
    status: &str,
    created_at: i64,
    output_tokens: Option<i64>,
    elapsed: Option<Duration>,
) -> Result<(), ServiceError> {
    let tokens_per_second = output_tokens.and_then(|tokens| {
        elapsed.and_then(|duration| {
            (duration.as_secs_f64() > 0.0).then_some(tokens as f64 / duration.as_secs_f64())
        })
    });
    crate::chatgpt_store::save_assistant_message(
        storage,
        conversation_id,
        message_id,
        content,
        status,
        created_at,
        output_tokens,
        tokens_per_second,
        elapsed.and_then(|value| i64::try_from(value.as_micros()).ok()),
    )
    .map_err(|_| storage_error())
}

fn terminal_result(
    conversation_id: &str,
    message_id: &str,
    status: &str,
    elapsed: Duration,
    output_tokens: Option<i64>,
) -> Value {
    json!({
        "conversationId": conversation_id,
        "messageId": message_id,
        "status": status,
        "outputTokens": output_tokens,
        "tokensPerSecond": Value::Null,
        "elapsedMicroseconds": elapsed.as_micros(),
    })
}

fn http_error(status: StatusCode) -> ServiceError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ServiceError::new(
            "authentication_required",
            "OpenCode rejected the request. Check provider access.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            "OpenCode reported a usage limit. Try again later.",
            true,
        ),
        _ => ServiceError::new(
            "provider_request_failed",
            "OpenCode could not complete the request.",
            status.is_server_error(),
        ),
    }
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "OpenCode could not be reached. Check the network and try again.",
        true,
    )
}
fn invalid_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "OpenCode returned a response Zihora could not read.",
        true,
    )
}
fn storage_error() -> ServiceError {
    ServiceError::new(
        "storage_unavailable",
        "Chat history could not be updated.",
        false,
    )
}
fn route_error() -> ServiceError {
    ServiceError::new(
        "conversation_not_routed",
        "Choose an OpenCode model before sending a message.",
        false,
    )
}
fn model_error() -> ServiceError {
    ServiceError::new(
        "model_unavailable",
        "The OpenCode model is no longer available. Choose another model.",
        false,
    )
}
fn authentication_required_error() -> ServiceError {
    ServiceError::new(
        "authentication_required",
        "Add an OpenCode Console API key to use this paid model.",
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
fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The OpenCode request was cancelled.",
        true,
    )
}
fn protocol_error() -> ServiceError {
    ServiceError::new(
        "protocol_unavailable",
        "The local service could not deliver the response.",
        false,
    )
}
