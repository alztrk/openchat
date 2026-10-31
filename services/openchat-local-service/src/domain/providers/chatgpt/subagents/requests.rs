//! Child request payload construction, response parsing, and transport dispatch.

use super::*;

pub(super) async fn finish_child_run(
    user_questions: &UserQuestionBroker,
    storage: &AppStorage,
    snapshot: &ChatStreamSnapshot,
    child_run_id: &str,
    outcome: RunOutcome,
) -> Result<(), ServiceError> {
    user_questions
        .finish_run(storage, &snapshot.conversation_id, child_run_id, outcome)
        .await
}

pub(super) fn chat_completion_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

pub(super) fn messages_tool(tool: &ToolDefinition) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

pub(super) fn chat_completion_child_payload(
    provider_id: &str,
    model_id: &str,
    reasoning_effort: Option<&str>,
    definitions: &[ToolDefinition],
    instructions: &str,
    user_input: &str,
) -> Value {
    let max_tokens_field = if matches!(provider_id, "gemini" | "mistral") {
        "max_tokens"
    } else {
        "max_completion_tokens"
    };
    let mut payload = json!({
        "model": model_id,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": user_input}
        ],
        "stream": true,
        "tools": definitions.iter().map(chat_completion_tool).collect::<Vec<_>>(),
        "tool_choice": "auto",
        "parallel_tool_calls": false,
    });
    payload[max_tokens_field] = json!(CHILD_MAX_OUTPUT_TOKENS);
    if provider_id == "mistral"
        && let Some(reasoning_effort) = reasoning_effort
    {
        payload["reasoning_effort"] = json!(reasoning_effort);
    }
    payload
}

pub(super) fn parse_chat_completion_child_response(
    value: &Value,
) -> Result<(Vec<ToolCall>, String, Value), ServiceError> {
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(subagent_response_error)?;
    let summary = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let raw_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if raw_calls.len() > CHILD_MAX_TOOL_CALLS {
        return Err(subagent_limit_error());
    }
    let mut calls = Vec::with_capacity(raw_calls.len());
    for raw_call in raw_calls {
        let id = raw_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .ok_or_else(subagent_response_error)?;
        let function = raw_call
            .get("function")
            .ok_or_else(subagent_response_error)?;
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty() && name.len() <= 128)
            .ok_or_else(subagent_response_error)?;
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .filter(|arguments| arguments.len() <= tools::MAX_TOOL_ARGUMENT_BYTES)
            .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
            .filter(Value::is_object)
            .ok_or_else(subagent_response_error)?;
        calls.push(ToolCall {
            id: id.to_owned(),
            name: name.to_owned(),
            arguments,
        });
    }
    let assistant_message = json!({
        "role": "assistant",
        "content": if summary.is_empty() { Value::Null } else { json!(summary) },
        "tool_calls": raw_tool_calls_for_history(message),
    });
    Ok((calls, summary, assistant_message))
}

pub(super) fn parse_messages_child_response(
    value: &Value,
) -> Result<(Vec<ToolCall>, String, Value), ServiceError> {
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(subagent_response_error)?;
    if blocks.len() > CHILD_MAX_TOOL_CALLS + 32 {
        return Err(subagent_limit_error());
    }
    let mut calls = Vec::new();
    let mut summary = String::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    summary.push_str(text);
                }
            }
            Some("tool_use") => {
                if calls.len() >= CHILD_MAX_TOOL_CALLS {
                    return Err(subagent_limit_error());
                }
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty() && id.len() <= 128)
                    .ok_or_else(subagent_response_error)?;
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty() && name.len() <= 128)
                    .ok_or_else(subagent_response_error)?;
                let arguments = block
                    .get("input")
                    .filter(|input| input.is_object())
                    .cloned()
                    .ok_or_else(subagent_response_error)?;
                calls.push(ToolCall {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    arguments,
                });
            }
            _ => {}
        }
    }
    Ok((calls, summary, json!({"role":"assistant","content":blocks})))
}

pub(super) fn raw_tool_calls_for_history(message: &Value) -> Vec<Value> {
    message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|call| {
            json!({
                "id": call.get("id"),
                "type": "function",
                "function": {
                    "name": call.pointer("/function/name"),
                    "arguments": call.pointer("/function/arguments"),
                }
            })
        })
        .collect()
}

#[cfg(test)]
pub(super) async fn send_api_key_child_request_to(
    http: &Client,
    endpoint: &str,
    api_key: Option<&str>,
    payload: &Value,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<Option<HttpResponse>, ServiceError> {
    tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Ok(None)
        }
        response = async {
            let request = http.post(endpoint);
            let request = match api_key {
                Some(api_key) => request.bearer_auth(api_key),
                None => request,
            };
            request.json(payload).send().await
        } => response
            .map(Some)
            .map_err(|_| ServiceError::new(
                "network_unavailable",
                "The selected provider endpoint could not be reached.",
                true,
            )),
    }
}

pub(super) async fn send_compatible_child_request_to(
    http: &Client,
    endpoint: &str,
    api_key: Option<&str>,
    protocol: ChildRequestProtocol,
    payload: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Option<HttpResponse>, ServiceError> {
    let mut request = http.post(endpoint).header(ACCEPT, "text/event-stream");
    if protocol == ChildRequestProtocol::Messages {
        if let Some(api_key) = api_key {
            request = request
                .header(
                    "x-api-key",
                    sensitive_api_key_header(api_key).map_err(|_| {
                        ServiceError::new(
                            "invalid_provider_credential",
                            "The configured provider API key is invalid.",
                            false,
                        )
                    })?,
                )
                .header("anthropic-version", "2023-06-01");
        }
    } else if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Ok(None)
        }
        response = request.json(payload).send() => response
            .map(Some)
            .map_err(|_| ServiceError::new(
                "network_unavailable",
                "The selected provider endpoint could not be reached.",
                true,
            )),
    }
}

pub(super) async fn send_copilot_child_request(
    http: &Client,
    endpoint: &str,
    access_token: &str,
    payload: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Option<HttpResponse>, ServiceError> {
    let request = crate::openai_compatible::authorize_copilot_child_inference(
        http.post(endpoint).header(ACCEPT, "text/event-stream"),
        access_token,
    );
    tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Ok(None)
        }
        response = request.json(payload).send() => response
            .map(Some)
            .map_err(|_| ServiceError::new(
                "network_unavailable",
                "The selected provider endpoint could not be reached.",
                true,
            )),
    }
}
