//! Tauri notification command handlers and request payloads.

use crate::request_bounds::{deserialize_bounded_text, BoundedText};
use tauri::AppHandle;

use super::activation::validate_identifier;

pub(crate) const MAXIMUM_NOTIFICATION_IDENTIFIER_BYTES: usize = 128;
pub(crate) const MAXIMUM_TEST_NOTIFICATION_TITLE_BYTES: usize = 120 * 4;
pub(crate) const MAXIMUM_NOTIFICATION_TITLE_BYTES: usize = 160 * 4;
pub(crate) const MAXIMUM_NOTIFICATION_BODY_BYTES: usize = 500 * 4;
pub(crate) const MAXIMUM_NOTIFICATION_ACTION_LABEL_BYTES: usize = 80 * 4;
pub(crate) const MAXIMUM_NOTIFICATION_REPLY_LABEL_BYTES: usize = 120 * 4;
pub(crate) const MAXIMUM_NOTIFICATION_CONTENT_CHARS: usize = 200_000;
pub(crate) const MAXIMUM_NOTIFICATION_CONTENT_BYTES: usize = MAXIMUM_NOTIFICATION_CONTENT_CHARS * 4;

/// Deserializes notification identifiers within their existing 128-byte validation limit.
pub(crate) fn deserialize_notification_identifier<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes notification titles within the UTF-8 maximum of their 160-character limit.
pub(crate) fn deserialize_notification_title<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_TITLE_BYTES>(deserializer)
}

/// Deserializes notification bodies within the UTF-8 maximum of their 500-character limit.
pub(crate) fn deserialize_notification_body<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_BODY_BYTES>(deserializer)
}

/// Deserializes notification action labels within the UTF-8 maximum of their 80-character limit.
pub(crate) fn deserialize_notification_action_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_ACTION_LABEL_BYTES>(deserializer)
}

/// Deserializes notification reply labels within the UTF-8 maximum of their 120-character limit.
pub(crate) fn deserialize_notification_reply_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_REPLY_LABEL_BYTES>(deserializer)
}

/// Deserializes assistant notification content within the UTF-8 maximum of its 200,000-character limit.
pub(crate) fn deserialize_notification_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_CONTENT_BYTES>(deserializer)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated display content and actions for an assistant-response notification.
pub struct AssistantResponseNotification {
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    pub(crate) conversation_id: String,
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    pub(crate) assistant_message_id: String,
    #[serde(deserialize_with = "deserialize_notification_title")]
    pub(crate) title: String,
    #[serde(deserialize_with = "deserialize_notification_content")]
    pub(crate) content: String,
    #[serde(deserialize_with = "deserialize_notification_body")]
    pub(crate) generic_body: String,
    #[serde(deserialize_with = "deserialize_notification_action_label")]
    pub(crate) send_label: String,
    #[serde(deserialize_with = "deserialize_notification_reply_label")]
    pub(crate) reply_label: String,
    pub(crate) show_preview: bool,
    pub(crate) allow_quick_reply: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated display content for a notification that opens a user-question group.
pub struct UserQuestionNotification {
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    pub(crate) group_id: String,
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    pub(crate) conversation_id: String,
    #[serde(deserialize_with = "deserialize_notification_title")]
    pub(crate) title: String,
    #[serde(deserialize_with = "deserialize_notification_body")]
    pub(crate) body: String,
}

#[tauri::command]
/// Reports whether native notifications are enabled on the current platform.
pub fn read_windows_notification_status() -> String {
    #[cfg(target_os = "windows")]
    {
        super::windows_backend::read_status()
    }
    #[cfg(not(target_os = "windows"))]
    {
        "notSupported".to_owned()
    }
}

#[tauri::command]
/// Opens the operating system's notification settings page where supported.
///
/// # Errors
/// Returns a platform or operating-system error when the settings page cannot be opened.
pub fn open_windows_notification_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        super::windows_backend::open_settings()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows notification settings are not available on this platform.".to_owned())
    }
}

#[tauri::command]
/// Sends a test notification after validating its title and body lengths.
///
/// Returns `Ok(false)` when native notifications are unsupported or unavailable.
///
/// # Errors
/// Returns a validation or platform notification error.
pub fn send_test_notification(
    title: BoundedText<MAXIMUM_TEST_NOTIFICATION_TITLE_BYTES>,
    body: BoundedText<MAXIMUM_NOTIFICATION_BODY_BYTES>,
) -> Result<bool, String> {
    let title = title.into_inner();
    let body = body.into_inner();
    validate_text(&title, 120, "The notification title is invalid.")?;
    validate_text(&body, 500, "The notification body is invalid.")?;
    #[cfg(target_os = "windows")]
    {
        super::windows_backend::send_test(&title, &body)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

#[tauri::command]
/// Sends a validated assistant-response notification with optional preview and quick reply.
///
/// # Errors
/// Returns a validation error or a platform notification error.
pub fn send_assistant_response_notification(
    app: AppHandle,
    request: AssistantResponseNotification,
) -> Result<bool, String> {
    validate_identifier(&request.conversation_id)?;
    validate_identifier(&request.assistant_message_id)?;
    validate_text(&request.title, 160, "The conversation title is invalid.")?;
    validate_text(
        &request.generic_body,
        500,
        "The notification body is invalid.",
    )?;
    validate_text(
        &request.send_label,
        80,
        "The notification action label is invalid.",
    )?;
    validate_text(
        &request.reply_label,
        120,
        "The notification input label is invalid.",
    )?;
    if request.content.chars().count() > MAXIMUM_NOTIFICATION_CONTENT_CHARS {
        return Err("The assistant response is too large for a notification.".to_owned());
    }

    #[cfg(target_os = "windows")]
    {
        super::windows_backend::send_assistant_response(&app, request)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, request);
        Ok(false)
    }
}

#[tauri::command]
/// Sends a validated notification that opens a pending user-question group.
///
/// # Errors
/// Returns a validation error or a platform notification error.
pub fn send_user_question_notification(
    app: AppHandle,
    request: UserQuestionNotification,
) -> Result<bool, String> {
    validate_identifier(&request.group_id)?;
    validate_identifier(&request.conversation_id)?;
    validate_text(&request.title, 160, "The notification title is invalid.")?;
    validate_text(&request.body, 500, "The notification body is invalid.")?;
    #[cfg(target_os = "windows")]
    {
        super::windows_backend::send_user_question(&app, request)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, request);
        Ok(false)
    }
}

pub(crate) fn validate_text(value: &str, maximum_characters: usize, message: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.chars().count() > maximum_characters
        || value.chars().any(char::is_control)
    {
        return Err(message.to_owned());
    }
    Ok(())
}
