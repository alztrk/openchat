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
    } = options;
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
        reasoning_effort: None,
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
