//! Unit tests for notification activation arguments and payload parsing.

use super::{
    activation::{
        activation_arguments, notification_reply_input, parse_activation_arguments,
        NotificationActivation, MAXIMUM_REPLY_CHARACTERS,
    },
    commands::{AssistantResponseNotification, MAXIMUM_NOTIFICATION_CONTENT_BYTES},
};

fn reply_activation_arguments() -> String {
    activation_arguments(
        &NotificationActivation::ReplyToAssistant {
            conversation_id: "conv_1".to_owned(),
            assistant_message_id: "msg_1".to_owned(),
            reply_text: None,
            reply_input_too_long: false,
        },
        true,
    )
    .expect("reply activation arguments should encode")
}

#[test]
fn oversized_reply_text_is_not_retained_in_activation_state() {
    let arguments = reply_activation_arguments();
    let reply = "x".repeat(MAXIMUM_REPLY_CHARACTERS + 1);
    let (reply_text, reply_input_too_long) = notification_reply_input(Some(&reply));
    assert!(reply_text.is_none());
    assert!(reply_input_too_long);
    let activation =
        parse_activation_arguments(&arguments, reply_text.as_deref(), reply_input_too_long)
            .expect("the activation target should remain valid");

    assert!(matches!(
        activation,
        NotificationActivation::ReplyToAssistant {
            reply_text: None,
            reply_input_too_long: true,
            ..
        }
    ));
}

#[test]
fn over_limit_reply_is_reported_without_copying_text() {
    let activation = parse_activation_arguments(&reply_activation_arguments(), None, true)
        .expect("the activation target should remain valid");

    assert!(matches!(
        activation,
        NotificationActivation::ReplyToAssistant {
            reply_text: None,
            reply_input_too_long: true,
            ..
        }
    ));
}

#[test]
fn assistant_notification_deserializes_renderer_fields_with_content_bound() {
    let request = serde_json::json!({
        "conversationId": "conversation-1",
        "assistantMessageId": "message-1",
        "title": "Conversation",
        "content": "Response text",
        "genericBody": "Open the response",
        "sendLabel": "Send",
        "replyLabel": "Reply",
        "showPreview": true,
        "allowQuickReply": true
    });
    let parsed = serde_json::from_value::<AssistantResponseNotification>(request.clone())
        .expect("the renderer notification shape should remain accepted");
    assert_eq!(parsed.conversation_id, "conversation-1");
    assert_eq!(parsed.content, "Response text");

    let mut oversized = request;
    oversized["content"] =
        serde_json::Value::String("x".repeat(MAXIMUM_NOTIFICATION_CONTENT_BYTES + 1));
    assert!(serde_json::from_value::<AssistantResponseNotification>(oversized).is_err());
}
