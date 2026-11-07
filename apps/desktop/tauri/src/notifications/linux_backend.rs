//! Linux desktop notifications using the active session's freedesktop notification service.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{Action, ActionPerformed, ActionType, NotificationExt};

use super::activation::{
    dispatch_activation, require_reply_confirmation, validate_identifier, NotificationActivation,
};
use super::commands::{AssistantResponseNotification, UserQuestionNotification};

const ASSISTANT_ACTION_TYPE: &str = "openchat-assistant-response";
const OPENCHAT_ACTIVATION: &str = "openchatActivation";

pub(crate) fn initialize(app: &AppHandle) -> Result<(), String> {
    let action_app = app.clone();
    app.notification()
        .on_action(move |action| handle_action(&action_app, action))
        .map_err(|_| "Linux notification actions could not be initialized.".to_owned())
}

pub(crate) fn read_status() -> String {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
        || std::env::var_os("XDG_RUNTIME_DIR").is_some()
    {
        "enabled".to_owned()
    } else {
        "unavailable".to_owned()
    }
}

pub(crate) fn open_settings() -> Result<(), String> {
    Err("Open notification settings from your desktop environment's system settings.".to_owned())
}

pub(crate) fn send_test(app: &AppHandle, title: &str, body: &str) -> Result<bool, String> {
    if read_status() != "enabled" {
        return Ok(false);
    }
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|_| "The Linux test notification could not be sent.".to_owned())?;
    Ok(true)
}

pub(crate) fn send_assistant_response(
    app: &AppHandle,
    request: AssistantResponseNotification,
) -> Result<bool, String> {
    if read_status() != "enabled" {
        return Ok(false);
    }

    let body = if request.show_preview {
        notification_preview(&request.content)
    } else {
        request.generic_body.clone()
    };
    if body.is_empty() {
        return Ok(false);
    }

    let activation = NotificationActivation::OpenConversation {
        conversation_id: request.conversation_id.clone(),
    };
    let mut builder = app
        .notification()
        .builder()
        .title(request.title)
        .body(body)
        .extra(
            OPENCHAT_ACTIVATION,
            serde_json::to_value(&activation).map_err(|_| {
                "The Linux notification activation could not be prepared.".to_owned()
            })?,
        );

    if request.allow_quick_reply {
        let actions = ActionType::builder(ASSISTANT_ACTION_TYPE)
            .actions(vec![Action::builder("reply", request.reply_label).build()])
            .build();
        app.notification()
            .register_action_types(vec![actions])
            .map_err(|_| "Linux notification actions could not be registered.".to_owned())?;
        let reply_activation = NotificationActivation::ReplyToAssistant {
            conversation_id: request.conversation_id,
            assistant_message_id: request.assistant_message_id,
            confirmation_required: true,
            reply_text: None,
            reply_input_too_long: false,
        };
        builder = builder.action_type_id(ASSISTANT_ACTION_TYPE).extra(
            "openchatReplyActivation",
            serde_json::to_value(reply_activation)
                .map_err(|_| "The Linux reply activation could not be prepared.".to_owned())?,
        );
    }

    builder
        .show()
        .map_err(|_| "The Linux assistant notification could not be sent.".to_owned())?;
    Ok(true)
}

pub(crate) fn send_user_question(
    app: &AppHandle,
    request: UserQuestionNotification,
) -> Result<bool, String> {
    if read_status() != "enabled" {
        return Ok(false);
    }
    let activation = NotificationActivation::OpenQuestion {
        conversation_id: request.conversation_id,
        group_id: request.group_id,
    };
    app.notification()
        .builder()
        .title(request.title)
        .body(request.body)
        .extra(
            OPENCHAT_ACTIVATION,
            serde_json::to_value(activation)
                .map_err(|_| "The Linux question activation could not be prepared.".to_owned())?,
        )
        .show()
        .map_err(|_| "The Linux question notification could not be sent.".to_owned())?;
    Ok(true)
}

fn handle_action(app: &AppHandle, action: &ActionPerformed) {
    let Some(notification) = action.notification() else {
        return;
    };
    let extra = notification.extra();
    let activation_key = if action.action_id() == "reply" {
        "openchatReplyActivation"
    } else if action.action_id() == "tap" || action.action_id() == "open" {
        OPENCHAT_ACTIVATION
    } else {
        return;
    };
    let Some(mut activation) = extra
        .get(activation_key)
        .and_then(|value| serde_json::from_value::<NotificationActivation>(value.clone()).ok())
    else {
        return;
    };

    if action.action_id() == "reply" {
        require_reply_confirmation(&mut activation);
    }
    if validate_activation(&activation).is_err() {
        return;
    }
    if dispatch_activation(activation).is_err() {
        eprintln!("linux_notification_activation_dispatch_failed");
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        if window.show().is_err() || window.unminimize().is_err() || window.set_focus().is_err() {
            eprintln!("linux_notification_window_activation_failed");
        }
    }
}

fn validate_activation(activation: &NotificationActivation) -> Result<(), String> {
    match activation {
        NotificationActivation::OpenConversation { conversation_id } => {
            validate_identifier(conversation_id)
        }
        NotificationActivation::ReplyToAssistant {
            conversation_id,
            assistant_message_id,
            ..
        } => {
            validate_identifier(conversation_id)?;
            validate_identifier(assistant_message_id)
        }
        NotificationActivation::OpenQuestion {
            conversation_id,
            group_id,
        } => {
            validate_identifier(conversation_id)?;
            validate_identifier(group_id)
        }
    }
}

fn notification_preview(content: &str) -> String {
    content.chars().take(500).collect()
}

#[cfg(test)]
mod tests {
    use super::{notification_preview, validate_activation};
    use crate::notifications::activation::NotificationActivation;

    #[test]
    fn preview_is_bounded_by_characters_without_splitting_unicode() {
        let preview = notification_preview(&"ş🙂".repeat(300));
        assert_eq!(preview.chars().count(), 500);
        assert!(preview.is_char_boundary(preview.len()));
    }

    #[test]
    fn notification_activation_rejects_invalid_identifiers() {
        let activation = NotificationActivation::OpenConversation {
            conversation_id: "../outside".to_owned(),
        };
        assert!(validate_activation(&activation).is_err());
    }
}
