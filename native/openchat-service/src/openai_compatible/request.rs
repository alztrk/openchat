use serde_json::{Value, json};

use crate::{
    chatgpt_store, instructions,
    protocol::ServiceError,
    provider_schema::{ProviderChatRequest, ProviderMessage},
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
};

use super::{chat_completion_tool, invalid_retry_target_error, storage_error};

pub(super) struct ProviderRequestOptions<'a> {
    pub(super) model_id: String,
    pub(super) provider_id: &'a str,
    pub(super) excluded_assistant_message_id: Option<&'a str>,
    pub(super) custom_instructions: Option<&'a str>,
    pub(super) permission_mode: ToolPermissionMode,
    pub(super) has_project: bool,
    pub(super) reasoning_effort: Option<&'a str>,
}

pub(super) fn build_provider_request(
    storage: &AppStorage,
    conversation_id: &str,
    options: ProviderRequestOptions<'_>,
) -> Result<ProviderChatRequest, ServiceError> {
    let ProviderRequestOptions {
        model_id,
        provider_id,
        excluded_assistant_message_id,
        custom_instructions,
        permission_mode,
        has_project,
        reasoning_effort,
    } = options;
    let reasoning_effort = match (provider_id, reasoning_effort) {
        ("opencode", Some(level)) => {
            if !super::models::supports_reasoning_level(storage, &model_id, level)? {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The selected reasoning level is not supported by this OpenCode model.",
                    false,
                ));
            }
            Some(level.to_owned())
        }
        _ => None,
    };
    let stored_messages = chatgpt_store::conversation_messages(storage, conversation_id)
        .map_err(|_| storage_error())?;
    if excluded_assistant_message_id.is_some_and(|id| {
        !chatgpt_store::is_retryable_latest_assistant_message(&stored_messages, id)
    }) {
        return Err(invalid_retry_target_error());
    }
    let messages = stored_messages
        .into_iter()
        .filter(|message| Some(message.id.as_str()) != excluded_assistant_message_id)
        .map(|message| ProviderMessage::from_history(&message.role, &message.content))
        .collect::<Result<Vec<_>, _>>()?;
    if messages.is_empty() {
        return Err(ServiceError::new(
            "conversation_empty",
            "Write a message before starting the response.",
            false,
        ));
    }

    Ok(ProviderChatRequest {
        model: model_id,
        instructions: instructions::shared_instructions(
            custom_instructions,
            permission_mode,
            has_project,
        ),
        messages,
        tools: tools::definitions_for_provider(provider_id),
        reasoning_effort,
    })
}

pub(super) fn completion_messages(request: &ProviderChatRequest) -> Vec<Value> {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(json!({
        "role": "system",
        "content": request.instructions,
    }));
    messages.extend(
        request
            .messages
            .iter()
            .map(|message| json!({"role": message.role.as_str(), "content": message.content})),
    );
    messages
}

pub(super) fn chat_completion_body(
    request: &ProviderChatRequest,
    messages: &[Value],
    is_opencode: bool,
) -> Value {
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
    });
    if is_opencode {
        body["tools"] = json!(tools::opencode_wire_tools(&request.tools));
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(false);
        if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
            body["reasoning_effort"] = json!(reasoning_effort);
        }
    } else if !request.tools.is_empty() {
        body["tools"] = json!(
            request
                .tools
                .iter()
                .map(chat_completion_tool)
                .collect::<Vec<_>>()
        );
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(false);
    }
    body
}

pub(super) fn responses_input_items(messages: &[Value]) -> Vec<Value> {
    let mut items = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        match role {
            "system" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str) {
                    items.push(json!({
                        "role": "developer",
                        "content": [{
                            "type": "input_text",
                            "text": content,
                        }]
                    }));
                }
            }
            "user" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str) {
                    items.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": content,
                        }]
                    }));
                }
            }
            "assistant" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        items.push(json!({
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": content,
                            }]
                        }));
                    }
                }
                if let Some(tool_calls) = msg.get("tool_calls").and_then(Value::as_array) {
                    for call in tool_calls {
                        let id = call.get("id").and_then(Value::as_str).unwrap_or("");
                        let name = call
                            .pointer("/function/name")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let arguments = call
                            .pointer("/function/arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        items.push(json!({
                            "type": "function_call",
                            "call_id": id,
                            "name": name,
                            "arguments": arguments,
                        }));
                    }
                }
            }
            "tool" => {
                let call_id = msg.get("tool_call_id").and_then(Value::as_str).unwrap_or("");
                let content = msg.get("content").and_then(Value::as_str).unwrap_or("");
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": content,
                }));
            }
            _ => {}
        }
    }
    items
}

pub(super) fn responses_api_body(
    request: &ProviderChatRequest,
    messages: &[Value],
) -> Value {
    let mut body = json!({
        "model": request.model,
        "input": responses_input_items(messages),
        "stream": true,
        "tools": tools::opencode_responses_wire_tools(&request.tools),
        "tool_choice": "auto",
        "parallel_tool_calls": true,
    });
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        body["reasoning"] = json!({
            "effort": reasoning_effort,
        });
    }
    body
}
