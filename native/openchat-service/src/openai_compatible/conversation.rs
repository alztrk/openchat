use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt_store::AssistantMessageWrite,
    protocol::ServiceError,
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot},
    storage::AppStorage,
    tools::ToolExecutor,
};

use super::{
    opencode_session_id_for_conversation, protocol_error, request, route, save_message,
    storage_error, terminal_result,
};

mod response_stream;
mod tool_round;

pub async fn send_message(
    storage: &AppStorage,
    context: ChatSendContext<'_>,
    api_key: Option<&str>,
    requested_api_key_connection_id: Option<&str>,
    stored_api_key_connection_id: Option<String>,
) -> Result<Value, ServiceError> {
    let ChatSendContext {
        request_id,
        conversation_id,
        excluded_assistant_message_id,
        custom_instructions,
        project_root,
        data_root,
        permission_mode,
        permission_broker,
        cancellation,
        events,
    } = context;
    let route = route::resolve_chat_route(
        storage,
        conversation_id,
        api_key,
        requested_api_key_connection_id,
        stored_api_key_connection_id.as_deref(),
    )?;
    let provider_request = request::build_provider_request(
        storage,
        conversation_id,
        request::ProviderRequestOptions {
            model_id: route.model_id.clone(),
            provider_id: route.provider_id.as_deref().unwrap_or(""),
            excluded_assistant_message_id,
            custom_instructions,
            permission_mode,
            has_project: project_root.is_some(),
        },
    )?;
    let started = Instant::now();
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?
        .as_millis();
    let created_at = i64::try_from(created_at).map_err(|_| storage_error())?;
    let message_id = Uuid::new_v4().simple().to_string();
    let mut content = String::new();
    let mut reasoning_content = String::new();
    let mut output_tokens = None;

    save_message(
        storage,
        AssistantMessageWrite {
            conversation_id,
            message_id: &message_id,
            content: &content,
            status: "streaming",
            created_at_unix_ms: created_at,
            output_tokens,
            elapsed: None,
        },
    )?;
    if events
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
        .is_err()
    {
        save_message(
            storage,
            AssistantMessageWrite {
                conversation_id,
                message_id: &message_id,
                content: &content,
                status: "failed",
                created_at_unix_ms: created_at,
                output_tokens,
                elapsed: Some(started.elapsed()),
            },
        )?;
        return Err(protocol_error());
    }

    let mut messages = request::completion_messages(&provider_request);
    let mut tool_executor = ToolExecutor::new(project_root, data_root, permission_mode);
    let result = stream_conversation(
        &route,
        &provider_request,
        &mut messages,
        &mut tool_executor,
        api_key,
        StreamConversationContext {
            request_id: &request_id,
            conversation_id,
            message_id: &message_id,
            created_at,
            content: &mut content,
            reasoning_content: &mut reasoning_content,
            output_tokens: &mut output_tokens,
            started,
            permission_broker,
            cancellation,
            events: &events,
        },
    )
    .await;

    let status = match result {
        Ok(()) => "completed",
        Err(error) if is_cancelled(&error) => "stopped",
        Err(error) => {
            save_message(
                storage,
                AssistantMessageWrite {
                    conversation_id,
                    message_id: &message_id,
                    content: &content,
                    status: "failed",
                    created_at_unix_ms: created_at,
                    output_tokens,
                    elapsed: Some(started.elapsed()),
                },
            )?;
            return Err(error);
        }
    };
    let elapsed = started.elapsed();
    save_message(
        storage,
        AssistantMessageWrite {
            conversation_id,
            message_id: &message_id,
            content: &content,
            status,
            created_at_unix_ms: created_at,
            output_tokens,
            elapsed: Some(elapsed),
        },
    )?;
    Ok(terminal_result(
        conversation_id,
        &message_id,
        status,
        elapsed,
        output_tokens,
    ))
}

struct StreamConversationContext<'a> {
    request_id: &'a Value,
    conversation_id: &'a str,
    message_id: &'a str,
    created_at: i64,
    content: &'a mut String,
    reasoning_content: &'a mut String,
    output_tokens: &'a mut Option<i64>,
    started: Instant,
    permission_broker: &'a crate::permissions::ToolPermissionBroker,
    cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    events: &'a crate::protocol::EventSink,
}

async fn stream_conversation(
    route: &route::ChatRoute,
    provider_request: &crate::provider_schema::ProviderChatRequest,
    messages: &mut Vec<Value>,
    tool_executor: &mut ToolExecutor,
    api_key: Option<&str>,
    context: StreamConversationContext<'_>,
) -> Result<(), ServiceError> {
    let session_id = route
        .is_opencode
        .then(|| opencode_session_id_for_conversation(context.conversation_id, context.created_at));

    loop {
        let body = request::chat_completion_body(provider_request, messages, route.is_opencode);
        let turn = response_stream::receive(ResponseStreamRequest {
            route,
            api_key,
            session_id: session_id.as_deref(),
            body: &body,
            request_id: context.request_id,
            conversation_id: context.conversation_id,
            message_id: context.message_id,
            created_at: context.created_at,
            content: context.content,
            reasoning_content: context.reasoning_content,
            output_tokens: context.output_tokens,
            started: context.started,
            cancellation: context.cancellation,
            events: context.events,
        })
        .await?;
        if turn.tool_calls.is_empty() {
            return Ok(());
        }

        tool_round::execute(
            &turn.tool_calls,
            &turn.round_content,
            route.is_opencode,
            messages,
            tool_executor,
            tool_round::ToolRoundContext {
                request_id: context.request_id,
                conversation_id: context.conversation_id,
                message_id: context.message_id,
                created_at: context.created_at,
                content: context.content,
                permission_broker: context.permission_broker,
                cancellation: context.cancellation,
                events: context.events,
            },
        )
        .await?;
    }
}

struct ResponseStreamRequest<'a> {
    route: &'a route::ChatRoute,
    api_key: Option<&'a str>,
    session_id: Option<&'a str>,
    body: &'a Value,
    request_id: &'a Value,
    conversation_id: &'a str,
    message_id: &'a str,
    created_at: i64,
    content: &'a mut String,
    reasoning_content: &'a mut String,
    output_tokens: &'a mut Option<i64>,
    started: Instant,
    cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    events: &'a crate::protocol::EventSink,
}

fn is_cancelled(error: &ServiceError) -> bool {
    matches!(error.code, "operation_cancelled" | "request_cancelled")
}
