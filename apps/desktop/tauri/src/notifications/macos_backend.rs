//! macOS notifications using the system notification center through Tauri's native plugin.

use std::process::Command;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use super::activation::{dispatch_activation, validate_identifier, NotificationActivation};
use super::commands::{AssistantResponseNotification, UserQuestionNotification};

const OPEN_ACTIVATION: &str = "openchatActivation";

pub(crate) fn initialize(app: &AppHandle) -> Result<(), String> {
    let action_app = app.clone();
    app.notification()
        .on_action(move |action| handle_action(&action_app, action))
        .map_err(|_| "macOS notification actions could not be initialized.".to_owned())
}

pub(crate) fn read_status() -> String {
    "unavailable".to_owned()
}

pub(crate) fn open_settings() -> Result<(), String> {
    Command::new("open")
        .args(["-b", "com.apple.systempreferences"])
        .spawn()
        .map(|_| ())
        .map_err(|_| "macOS notification settings could not be opened.".to_owned())
}

pub(crate) fn send_test(app: &AppHandle, title: &str, body: &str) -> Result<bool, String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|_| "The macOS test notification could not be sent.".to_owned())?;
    Ok(true)
}

pub(crate) fn send_assistant_response(
    app: &AppHandle,
    request: AssistantResponseNotification,
) -> Result<bool, String> {
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
    app.notification()
        .builder()
        .title(request.title)
        .body(body)
        .extra(
            OPEN_ACTIVATION,
            serde_json::to_value(activation).map_err(|_| {
                "The macOS notification activation could not be prepared.".to_owned()
            })?,
        )
        .show()
        .map_err(|_| "The macOS assistant notification could not be sent.".to_owned())?;
    Ok(true)
}

pub(crate) fn send_user_question(
    app: &AppHandle,
    request: UserQuestionNotification,
) -> Result<bool, String> {
    let activation = NotificationActivation::OpenQuestion {
        conversation_id: request.conversation_id,
        group_id: request.group_id,
    };
    app.notification()
        .builder()
        .title(request.title)
        .body(request.body)
        .extra(
            OPEN_ACTIVATION,
            serde_json::to_value(activation)
                .map_err(|_| "The macOS question activation could not be prepared.".to_owned())?,
        )
        .show()
        .map_err(|_| "The macOS question notification could not be sent.".to_owned())?;
    Ok(true)
}

fn handle_action(app: &AppHandle, action: &tauri_plugin_notification::ActionPerformed) {
    let Some(notification) = action.notification() else {
        return;
    };
    let extra = notification.extra();
    if action.action_id() != "tap" {
        return;
    }
    let Some(activation) = extra
        .get(OPEN_ACTIVATION)
        .and_then(|value| serde_json::from_value::<NotificationActivation>(value.clone()).ok())
    else {
        return;
    };

    if validate_activation(&activation).is_err() {
        return;
    }
    if dispatch_activation(activation).is_err() {
        eprintln!("macos_notification_activation_dispatch_failed");
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        if window.show().is_err() || window.unminimize().is_err() || window.set_focus().is_err() {
            eprintln!("macos_notification_window_activation_failed");
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
    fn preview_is_bounded_without_splitting_unicode() {
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
