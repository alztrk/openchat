use std::collections::HashMap;

use serde_json::{Value, json};

use crate::{
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamSnapshot, ToolCall},
    storage::AppStorage,
    tools::{ImageGenerationContext, ToolExecutor},
    user_question_broker::UserQuestionBroker,
};

use super::super::invalid_response_error;

pub(super) struct ToolRoundContext<'a> {
    pub(super) request_id: &'a Value,
    pub(super) conversation_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) created_at: i64,
    pub(super) content: &'a str,
    pub(super) storage: &'a AppStorage,
    pub(super) run_id: &'a str,
    pub(super) provider_id: &'a str,
    pub(super) image_generation: Option<ImageGenerationContext<'a>>,
    pub(super) permission_broker: &'a ToolPermissionBroker,
    pub(super) user_question_broker: &'a UserQuestionBroker,
    pub(super) cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    pub(super) events: &'a EventSink,
}

pub(super) async fn execute(
    calls: &[ToolCall],
    executable_calls: &[ToolCall],
    precomputed_results: Vec<crate::provider_schema::ToolResult>,
    round_content: &str,
    is_opencode: bool,
    messages: &mut Vec<Value>,
    tool_executor: &mut ToolExecutor,
    context: ToolRoundContext<'_>,
) -> Result<(), ServiceError> {
    tool_executor.begin_round(executable_calls)?;
    let mut results_by_id = HashMap::with_capacity(calls.len());
    for result in precomputed_results {
        if results_by_id
            .insert(result.call_id, result.output)
            .is_some()
        {
            return Err(invalid_response_error());
        }
    }
    for call in executable_calls {
        let snapshot = ChatStreamSnapshot::new(
            context.conversation_id,
            context.message_id,
            context.content,
            context.created_at,
        );
        let result = tool_executor
            .execute_call_with_image_context(
                call,
                context.permission_broker,
                context.request_id,
                &snapshot,
                context.events,
                context.cancellation,
                context.storage,
                context.run_id,
                context.provider_id,
                context.user_question_broker,
                context.image_generation.as_ref(),
            )
            .await?;
        if results_by_id
            .insert(result.call_id, result.output)
            .is_some()
        {
            return Err(invalid_response_error());
        }
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
    for call in calls {
        let output = results_by_id
            .remove(&call.id)
            .ok_or_else(invalid_response_error)?;
        let output = serde_json::to_string(&output).map_err(|_| invalid_response_error())?;
        messages.push(json!({
            "role": "tool",
            "tool_call_id": call.id,
            "content": output
        }));
    }
    Ok(())
}
