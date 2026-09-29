use serde_json::{Value, json};

use crate::{
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamSnapshot, ToolCall},
    tools::ToolExecutor,
};

use super::super::invalid_response_error;

pub(super) struct ToolRoundContext<'a> {
    pub(super) request_id: &'a Value,
    pub(super) conversation_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) created_at: i64,
    pub(super) content: &'a str,
    pub(super) permission_broker: &'a ToolPermissionBroker,
    pub(super) cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    pub(super) events: &'a EventSink,
}

pub(super) async fn execute(
    calls: &[ToolCall],
    round_content: &str,
    is_opencode: bool,
    messages: &mut Vec<Value>,
    tool_executor: &mut ToolExecutor,
    context: ToolRoundContext<'_>,
) -> Result<(), ServiceError> {
    tool_executor.begin_round(calls)?;
    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let snapshot = ChatStreamSnapshot::new(
            context.conversation_id,
            context.message_id,
            context.content,
            context.created_at,
        );
        results.push(
            tool_executor
                .execute_call(
                    call,
                    context.permission_broker,
                    context.request_id,
                    &snapshot,
                    context.events,
                    context.cancellation,
                )
                .await?,
        );
    }

    let assistant_calls = calls
        .iter()
        .map(|call| {
            let wire_name = if is_opencode {
                crate::tools::opencode_wire_name(&call.name)
            } else {
                call.name.as_str()
            };
            let arguments =
                serde_json::to_string(&call.arguments).map_err(|_| invalid_response_error())?;
            Ok(json!({
                "id": call.id,
                "type": "function",
                "function": {"name": wire_name, "arguments": arguments}
            }))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;

    messages.push(json!({
        "role": "assistant",
        "content": if round_content.is_empty() { Value::Null } else { json!(round_content) },
        "tool_calls": assistant_calls
    }));
    for result in results {
        let output = serde_json::to_string(&result.output).map_err(|_| invalid_response_error())?;
        messages.push(json!({
            "role": "tool",
            "tool_call_id": result.call_id,
            "content": output
        }));
    }
    Ok(())
}
