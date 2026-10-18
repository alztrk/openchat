//! Notification activation state management, queueing, and payload parsing.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub(crate) const NOTIFICATION_ACTIVATED_EVENT: &str = "openchat-notification-activated";
pub(crate) const MAXIMUM_REPLY_CHARACTERS: usize = 4096;
pub(crate) const MAXIMUM_REPLY_UTF16_UNITS: usize = MAXIMUM_REPLY_CHARACTERS * 2;
pub(crate) const MAXIMUM_ACTIVATION_ARGUMENT_UTF16_UNITS: usize = 1024;
pub(crate) const MAXIMUM_NOTIFICATION_INPUT_KEY_UTF16_UNITS: usize = 128;
pub(crate) const MAX_PENDING_ACTIVATIONS: usize = 64;

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
/// Notification action delivered to the frontend after the activation payload is validated.
pub enum NotificationActivation {
    /// Opens an existing conversation from an assistant-response notification.
    OpenConversation {
        /// Unique conversation identifier.
        conversation_id: String,
    },
    /// Opens a conversation and optionally submits a bounded quick reply.
    ReplyToAssistant {
        /// Target conversation identifier.
        conversation_id: String,
        /// Target assistant message identifier.
        assistant_message_id: String,
        /// Trimmed reply text if within limits.
        reply_text: Option<String>,
        /// Indicates if user input exceeded the maximum allowable reply length.
        reply_input_too_long: bool,
    },
    /// Opens the conversation and user-question group targeted by a notification.
    OpenQuestion {
        /// Target conversation identifier.
        conversation_id: String,
        /// Target question group identifier.
        group_id: String,
    },
}

#[derive(Default)]
struct ActivationRuntime {
    app: Option<AppHandle>,
    frontend_ready: bool,
    pending: VecDeque<NotificationActivation>,
}

static ACTIVATION_RUNTIME: OnceLock<Mutex<ActivationRuntime>> = OnceLock::new();
static ACTIVATION_SETUP_ERROR: OnceLock<String> = OnceLock::new();

/// Registers the app handle used for activation dispatch and starts platform setup.
pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let runtime = ACTIVATION_RUNTIME.get_or_init(|| Mutex::new(ActivationRuntime::default()));
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?;
    runtime.app = Some(app.clone());
    drop(runtime);
    #[cfg(target_os = "windows")]
    {
        let started = std::thread::Builder::new()
            .name("openchat-notification-setup".to_owned())
            .spawn(|| {
                if let Err(error) = super::windows_backend::register_activation_server() {
                    ACTIVATION_SETUP_ERROR.get_or_init(|| error);
                }
            });
        if started.is_err() {
            ACTIVATION_SETUP_ERROR
                .get_or_init(|| "Windows could not start notification setup.".to_owned());
        }
    }
    Ok(())
}

#[tauri::command]
/// Marks the frontend ready and drains queued notification actions exactly once.
///
/// # Errors
/// Returns an error if activation state is unavailable because its mutex was poisoned.
pub fn take_pending_notification_activations() -> Result<Vec<NotificationActivation>, String> {
    let runtime = ACTIVATION_RUNTIME.get_or_init(|| Mutex::new(ActivationRuntime::default()));
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?;
    runtime.frontend_ready = true;
    Ok(runtime.pending.drain(..).collect())
}

#[tauri::command]
/// Marks the frontend unavailable so later activations are queued until it reconnects.
///
/// # Errors
/// Returns an error if activation state is unavailable because its mutex was poisoned.
pub fn set_notification_frontend_unready() -> Result<(), String> {
    let runtime = ACTIVATION_RUNTIME.get_or_init(|| Mutex::new(ActivationRuntime::default()));
    runtime
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?
        .frontend_ready = false;
    Ok(())
}

pub(crate) fn dispatch_activation(activation: NotificationActivation) -> Result<(), String> {
    let runtime = ACTIVATION_RUNTIME.get_or_init(|| Mutex::new(ActivationRuntime::default()));
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?;
    if !runtime.frontend_ready {
        if runtime.pending.len() >= MAX_PENDING_ACTIVATIONS {
            return Err("Too many notification activations are waiting for OpenChat.".to_owned());
        }
        runtime.pending.push_back(activation);
        return Ok(());
    }

    let Some(app) = runtime.app.clone() else {
        runtime.frontend_ready = false;
        if runtime.pending.len() >= MAX_PENDING_ACTIVATIONS {
            return Err("Too many notification activations are waiting for OpenChat.".to_owned());
        }
        runtime.pending.push_back(activation);
        return Ok(());
    };
    drop(runtime);

    if app
        .emit(NOTIFICATION_ACTIVATED_EVENT, activation.clone())
        .is_ok()
    {
        return Ok(());
    }

    let mut runtime = ACTIVATION_RUNTIME
        .get_or_init(|| Mutex::new(ActivationRuntime::default()))
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?;
    runtime.frontend_ready = false;
    if runtime.pending.len() >= MAX_PENDING_ACTIVATIONS {
        return Err("Too many notification activations are waiting for OpenChat.".to_owned());
    }
    runtime.pending.push_back(activation);
    Err("The notification activation could not reach OpenChat.".to_owned())
}

pub(crate) fn activation_setup_result() -> Result<(), String> {
    ACTIVATION_SETUP_ERROR.get().cloned().map_or(Ok(()), Err)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToastActivationPayload {
    #[serde(rename = "type")]
    kind: Option<String>,
    conversation_id: String,
    group_id: Option<String>,
    assistant_message_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToastActivationArguments<'a> {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
    conversation_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message_id: Option<&'a str>,
}

pub(crate) fn activation_arguments(
    activation: &NotificationActivation,
    reply_action: bool,
) -> Result<String, String> {
    let (kind, conversation_id, group_id, assistant_message_id) = match activation {
        NotificationActivation::OpenConversation { conversation_id } => {
            (Some("assistant_response"), conversation_id, None, None)
        }
        NotificationActivation::ReplyToAssistant {
            conversation_id,
            assistant_message_id,
            ..
        } => (
            Some(if reply_action {
                "assistant_response_reply"
            } else {
                "assistant_response"
            }),
            conversation_id,
            None,
            Some(assistant_message_id.as_str()),
        ),
        NotificationActivation::OpenQuestion {
            conversation_id,
            group_id,
        } => (None, conversation_id, Some(group_id.as_str()), None),
    };
    serde_json::to_string(&ToastActivationArguments {
        kind,
        conversation_id,
        group_id,
        assistant_message_id,
    })
    .map_err(|_| "The notification activation could not be encoded.".to_owned())
}

/// Validates activation identifiers and removes over-limit reply text before dispatch.
pub(crate) fn parse_activation_arguments(
    arguments: &str,
    reply_text: Option<&str>,
    reply_input_too_long: bool,
) -> Result<NotificationActivation, String> {
    if arguments.len() > 1024 {
        return Err("The notification activation payload is too large.".to_owned());
    }
    let payload: ToastActivationPayload = serde_json::from_str(arguments)
        .map_err(|_| "The notification activation payload is invalid.".to_owned())?;
    validate_identifier(&payload.conversation_id)?;
    match payload.kind.as_deref() {
        Some("assistant_response") if payload.group_id.is_none() => {
            Ok(NotificationActivation::OpenConversation {
                conversation_id: payload.conversation_id,
            })
        }
        Some("assistant_response_reply") if payload.group_id.is_none() => {
            let assistant_message_id = payload
                .assistant_message_id
                .ok_or_else(|| "The notification reply target is invalid.".to_owned())?;
            validate_identifier(&assistant_message_id)?;
            let reply_input_too_long =
                reply_input_too_long || reply_text.is_some_and(reply_text_is_too_long);
            Ok(NotificationActivation::ReplyToAssistant {
                conversation_id: payload.conversation_id,
                assistant_message_id,
                reply_text: if reply_input_too_long {
                    None
                } else {
                    reply_text.map(str::trim).map(str::to_owned)
                },
                reply_input_too_long,
            })
        }
        None if payload.assistant_message_id.is_none() => {
            let group_id = payload
                .group_id
                .ok_or_else(|| "The notification question target is invalid.".to_owned())?;
            validate_identifier(&group_id)?;
            Ok(NotificationActivation::OpenQuestion {
                conversation_id: payload.conversation_id,
                group_id,
            })
        }
        _ => Err("The notification activation payload is not supported.".to_owned()),
    }
}

/// Checks at most one character beyond the notification reply limit.
pub(crate) fn reply_text_is_too_long(text: &str) -> bool {
    text.chars().take(MAXIMUM_REPLY_CHARACTERS + 1).count() > MAXIMUM_REPLY_CHARACTERS
}

/// Trims accepted reply text and drops content that exceeds the notification limit.
pub(crate) fn notification_reply_input(value: Option<&str>) -> (Option<String>, bool) {
    let input_too_long = value.is_some_and(reply_text_is_too_long);
    let text = if input_too_long {
        None
    } else {
        value.map(|text| text.trim().to_owned())
    };
    (text, input_too_long)
}

pub(crate) fn validate_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("The notification target is invalid.".to_owned());
    }
    Ok(())
}
