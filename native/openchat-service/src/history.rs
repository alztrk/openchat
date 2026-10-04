use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use crate::{
    chatgpt_store::StoredMessage,
    protocol::ServiceError,
    provider_schema::{MessageRole, ProviderMessage, ToolActivity, ToolCall},
};

pub(crate) fn provider_messages(
    message: &StoredMessage,
) -> Result<Vec<ProviderMessage>, ServiceError> {
    let tool_rounds = completed_tool_rounds(message);
    let mut messages = Vec::with_capacity(1 + tool_rounds.len().saturating_mul(2));

    if message.role == "assistant" && !tool_rounds.is_empty() {
        let mut emitted_text_bytes = 0;
        for tool_round in tool_rounds {
            append_assistant_text(
                &mut messages,
                message,
                &mut emitted_text_bytes,
                tool_round
                    .first()
                    .and_then(|activity| activity.assistant_text_before_byte_offset),
            );
            messages.push(ProviderMessage {
                role: MessageRole::Assistant,
                content: String::new(),
                images: Vec::new(),
                tool_calls: tool_round
                    .iter()
                    .map(|activity| ToolCall {
                        id: activity.call_id.clone(),
                        name: activity.name.clone(),
                        arguments: activity.arguments.clone(),
                    })
                    .collect(),
                tool_call_id: None,
            });
            messages.extend(tool_round.iter().map(|activity| {
                ProviderMessage {
                    role: MessageRole::Tool,
                    content: activity
                        .output
                        .as_ref()
                        .map_or_else(String::new, Value::to_string),
                    images: Vec::new(),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(activity.call_id.clone()),
                }
            }));
        }
        append_assistant_text(
            &mut messages,
            message,
            &mut emitted_text_bytes,
            Some(message.content.len()),
        );
    } else {
        messages.push(ProviderMessage {
            role: MessageRole::from_history(&message.role)?,
            content: message.content.clone(),
            images: if message.role == "user" {
                image_data_urls(message)?
            } else {
                Vec::new()
            },
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
    }

    Ok(messages)
}

pub(crate) fn responses_input_items(message: &StoredMessage) -> Result<Vec<Value>, ServiceError> {
    if message.role == "user" {
        let mut content = Vec::with_capacity(1 + message.attachments.len());
        if !message.content.is_empty() {
            content.push(json!({
                "type": "input_text",
                "text": message.content,
            }));
        }
        for image_url in image_data_urls(message)? {
            content.push(json!({
                "type": "input_image",
                "image_url": image_url,
                "detail": "auto",
            }));
        }
        return Ok(vec![json!({
            "role": "user",
            "content": content,
        })]);
    }

    let tool_rounds = completed_tool_rounds(message);
    let mut items = Vec::with_capacity(1 + tool_rounds.len().saturating_mul(2));
    if tool_rounds.is_empty() {
        items.push(json!({
            "role": "assistant",
            "content": message.content,
        }));
        return Ok(items);
    }

    let mut emitted_text_bytes = 0;
    for tool_round in tool_rounds {
        append_responses_assistant_text(
            &mut items,
            message,
            &mut emitted_text_bytes,
            tool_round
                .first()
                .and_then(|activity| activity.assistant_text_before_byte_offset),
        );
        for activity in &tool_round {
            items.push(json!({
                "type": "function_call",
                "call_id": activity.call_id,
                "name": activity.name,
                "arguments": activity.arguments.to_string(),
            }));
        }
        for activity in &tool_round {
            if let Some(output) = activity.output.as_ref() {
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": activity.call_id,
                    "output": output.to_string(),
                }));
            }
        }
    }
    append_responses_assistant_text(
        &mut items,
        message,
        &mut emitted_text_bytes,
        Some(message.content.len()),
    );
    Ok(items)
}

pub(crate) fn validate_attachments(messages: &[StoredMessage]) -> Result<(), ServiceError> {
    if messages
        .iter()
        .flat_map(|message| &message.attachments)
        .any(|attachment| {
            attachment.content.as_deref().is_none_or(|content| {
                attachment.kind == "text" && std::str::from_utf8(content).is_err()
            })
        })
    {
        return Err(ServiceError::new(
            "attachment_unavailable",
            "An attached file is unavailable. Reattach it and try again.",
            false,
        ));
    }
    Ok(())
}

pub(crate) fn validate_model_attachments(
    messages: &[StoredMessage],
    supports_images: bool,
) -> Result<(), ServiceError> {
    validate_attachments(messages)?;
    if !supports_images
        && messages
            .iter()
            .flat_map(|message| &message.attachments)
            .any(|attachment| attachment.kind == "image")
    {
        return Err(ServiceError::new(
            "model_does_not_support_images",
            "The selected model does not support image attachments.",
            false,
        ));
    }
    Ok(())
}

fn image_data_urls(message: &StoredMessage) -> Result<Vec<String>, ServiceError> {
    message
        .attachments
        .iter()
        .filter(|attachment| attachment.kind == "image")
        .map(|attachment| {
            let content = attachment.content.as_deref().ok_or_else(|| {
                ServiceError::new(
                    "attachment_unavailable",
                    "An attached image is unavailable. Reattach it and try again.",
                    false,
                )
            })?;
            Ok(format!(
                "data:{};base64,{}",
                attachment.mime_type,
                STANDARD.encode(content)
            ))
        })
        .collect()
}

fn append_assistant_text(
    messages: &mut Vec<ProviderMessage>,
    message: &StoredMessage,
    emitted_text_bytes: &mut usize,
    requested_end: Option<usize>,
) {
    let Some((text, end)) = assistant_text_segment(message, *emitted_text_bytes, requested_end)
    else {
        return;
    };
    *emitted_text_bytes = end;
    if !text.is_empty() {
        messages.push(ProviderMessage {
            role: MessageRole::Assistant,
            content: text.to_owned(),
            images: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
    }
}

fn append_responses_assistant_text(
    items: &mut Vec<Value>,
    message: &StoredMessage,
    emitted_text_bytes: &mut usize,
    requested_end: Option<usize>,
) {
    let Some((text, end)) = assistant_text_segment(message, *emitted_text_bytes, requested_end)
    else {
        return;
    };
    *emitted_text_bytes = end;
    if !text.is_empty() {
        items.push(json!({
            "role": "assistant",
            "content": text,
        }));
    }
}

fn assistant_text_segment(
    message: &StoredMessage,
    start: usize,
    requested_end: Option<usize>,
) -> Option<(&str, usize)> {
    let end = requested_end?;
    if start > end || end > message.content.len() || !message.content.is_char_boundary(end) {
        return None;
    }
    Some((&message.content[start..end], end))
}

pub(crate) fn tool_activity_summary_segments(message: &StoredMessage) -> Vec<(String, String)> {
    completed_tool_activities(message)
        .into_iter()
        .filter_map(|activity| {
            let arguments = activity.arguments.to_string();
            let output = activity.output.as_ref()?.to_string();
            Some(vec![
                (
                    format!("[historical tool call: {} arguments]\n", activity.name),
                    arguments,
                ),
                (
                    format!("[historical tool result: {}]\n", activity.name),
                    output,
                ),
            ])
        })
        .flatten()
        .collect()
}

pub(crate) fn summary_text(message: &StoredMessage) -> String {
    let tool_rounds = completed_tool_rounds(message);
    if tool_rounds.is_empty() {
        return message.content.clone();
    }

    let mut text = String::new();
    let mut emitted_text_bytes = 0;
    for tool_round in tool_rounds {
        if let Some((segment, end)) = assistant_text_segment(
            message,
            emitted_text_bytes,
            tool_round
                .first()
                .and_then(|activity| activity.assistant_text_before_byte_offset),
        ) {
            text.push_str(segment);
            emitted_text_bytes = end;
        }
        for activity in tool_round {
            append_summary_tool_activity(&mut text, activity);
        }
    }
    if let Some((segment, end)) =
        assistant_text_segment(message, emitted_text_bytes, Some(message.content.len()))
    {
        text.push_str(segment);
        emitted_text_bytes = end;
    }
    if emitted_text_bytes == 0 && text.is_empty() {
        return message.content.clone();
    }
    text
}

fn append_summary_tool_activity(text: &mut String, activity: &ToolActivity) {
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!(
        "[historical tool call: {} arguments]\n",
        activity.name
    ));
    text.push_str(&activity.arguments.to_string());
    text.push_str(&format!("\n[historical tool result: {}]\n", activity.name));
    if let Some(output) = activity.output.as_ref() {
        text.push_str(&output.to_string());
    }
    text.push('\n');
}

fn completed_tool_activities(message: &StoredMessage) -> Vec<&ToolActivity> {
    if message.role != "assistant" {
        return Vec::new();
    }
    message
        .tool_activities
        .iter()
        .filter(|activity| activity.has_completed_result())
        .collect()
}

fn completed_tool_rounds(message: &StoredMessage) -> Vec<Vec<&ToolActivity>> {
    let mut rounds = Vec::<Vec<&ToolActivity>>::new();
    for activity in completed_tool_activities(message) {
        if rounds
            .last()
            .and_then(|round| round.first())
            .is_some_and(|first| first.round_id == activity.round_id)
        {
            if let Some(round) = rounds.last_mut() {
                round.push(activity);
            }
        } else {
            rounds.push(vec![activity]);
        }
    }
    rounds
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        chatgpt_store::StoredMessage,
        provider_schema::{MessageRole, ToolActivity, ToolActivityStatus},
    };

    use super::{provider_messages, responses_input_items, tool_activity_summary_segments};

    fn assistant_message() -> StoredMessage {
        StoredMessage {
            id: "assistant".to_owned(),
            role: "assistant".to_owned(),
            content: "I found the setting.".to_owned(),
            status: "completed".to_owned(),
            output_tokens: None,
            tool_activities: vec![crate::provider_schema::ToolActivity {
                call_id: "call-1".to_owned(),
                name: "read".to_owned(),
                arguments: json!({"path": "settings.json"}),
                round_id: Some("round-1".to_owned()),
                assistant_text_before_byte_offset: Some(0),
                output: Some(json!({"content": "theme = warm"})),
                file_changes: Vec::new(),
                file_changes_error: None,
                status: ToolActivityStatus::Completed,
                target_path: Some("private/local/path".to_owned()),
            }],
            attachments: Vec::new(),
        }
    }

    #[test]
    fn provider_history_restores_assistant_tool_calls_and_results() {
        let messages = provider_messages(&assistant_message()).expect("valid stored history");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert!(messages[0].content.is_empty());
        assert_eq!(messages[0].tool_calls[0].id, "call-1");
        assert_eq!(messages[0].tool_calls[0].arguments["path"], "settings.json");
        assert_eq!(messages[1].role, MessageRole::Tool);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert!(messages[1].content.contains("theme = warm"));
        assert_eq!(messages[2].role, MessageRole::Assistant);
        assert_eq!(messages[2].content, "I found the setting.");
    }

    #[test]
    fn responses_history_restores_calls_before_their_outputs() {
        let items = responses_input_items(&assistant_message()).expect("valid response history");

        assert_eq!(items[0]["type"], "function_call");
        assert_eq!(items[0]["call_id"], "call-1");
        assert_eq!(items[1]["type"], "function_call_output");
        assert_eq!(items[1]["call_id"], "call-1");
        assert!(
            items[1]["output"]
                .as_str()
                .unwrap()
                .contains("theme = warm")
        );
        assert_eq!(items[2]["role"], "assistant");
        assert_eq!(items[2]["content"], "I found the setting.");
    }

    #[test]
    fn history_preserves_assistant_text_and_multiple_tool_round_order() {
        let mut message = assistant_message();
        message.content = "Inspecting. Reading. Done.".to_owned();
        message.tool_activities[0].round_id = Some("round-1".to_owned());
        message.tool_activities[0].assistant_text_before_byte_offset = Some("Inspecting.".len());
        let second_activity = ToolActivity {
            call_id: "call-2".to_owned(),
            name: "read".to_owned(),
            arguments: json!({"path": "other.json"}),
            round_id: Some("round-2".to_owned()),
            assistant_text_before_byte_offset: Some("Inspecting. Reading.".len()),
            target_path: None,
            output: Some(json!({"content": "second result"})),
            file_changes: Vec::new(),
            file_changes_error: None,
            status: ToolActivityStatus::Completed,
        };
        message.tool_activities.push(second_activity);

        let chat_messages = provider_messages(&message).expect("valid stored history");
        assert_eq!(chat_messages[0].content, "Inspecting.");
        assert_eq!(chat_messages[1].tool_calls[0].id, "call-1");
        assert_eq!(chat_messages[2].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(chat_messages[3].content, " Reading.");
        assert_eq!(chat_messages[4].tool_calls[0].id, "call-2");
        assert_eq!(chat_messages[5].tool_call_id.as_deref(), Some("call-2"));
        assert_eq!(chat_messages[6].content, " Done.");

        let responses_items = responses_input_items(&message).expect("valid response history");
        assert_eq!(responses_items[0]["content"], "Inspecting.");
        assert_eq!(responses_items[1]["call_id"], "call-1");
        assert_eq!(responses_items[2]["type"], "function_call_output");
        assert_eq!(responses_items[3]["content"], " Reading.");
        assert_eq!(responses_items[4]["call_id"], "call-2");
        assert_eq!(responses_items[5]["type"], "function_call_output");
        assert_eq!(responses_items[6]["content"], " Done.");

        let summary = super::summary_text(&message);
        assert!(
            summary.find("Inspecting.").unwrap()
                < summary.find("historical tool call: read").unwrap()
        );
        assert!(summary.find("theme = warm").unwrap() < summary.find(" Reading.").unwrap());
        assert!(summary.find("second result").unwrap() < summary.find(" Done.").unwrap());
    }

    #[test]
    fn calls_from_the_same_round_share_one_assistant_call_message() {
        let mut message = assistant_message();
        message.content.clear();
        message.tool_activities[0].round_id = Some("round-1".to_owned());
        let mut second = message.tool_activities[0].clone();
        second.call_id = "call-2".to_owned();
        second.arguments = json!({"path": "other.json"});
        message.tool_activities.push(second);

        let messages = provider_messages(&message).expect("valid stored history");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].tool_calls.len(), 2);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-2"));
    }

    #[test]
    fn unfinished_activities_are_not_added_to_model_history() {
        let mut message = assistant_message();
        message.tool_activities[0].output = None;
        message.tool_activities[0].status = ToolActivityStatus::Running;

        let provider_messages = provider_messages(&message).expect("valid stored history");
        let responses_items = responses_input_items(&message).expect("valid response history");
        let summary_segments = tool_activity_summary_segments(&message);

        assert_eq!(provider_messages.len(), 1);
        assert!(provider_messages[0].tool_calls.is_empty());
        assert_eq!(responses_items.len(), 1);
        assert!(summary_segments.is_empty());
    }

    #[test]
    fn old_tool_activity_records_without_order_metadata_still_deserialize() {
        let activity = serde_json::from_value::<ToolActivity>(json!({
            "callId": "legacy-call",
            "name": "read",
            "arguments": {"path": "settings.json"},
            "output": {"content": "stored"},
            "status": "completed"
        }))
        .expect("deserialize legacy activity");

        assert_eq!(activity.round_id, None);
        assert_eq!(activity.assistant_text_before_byte_offset, None);
    }
}
