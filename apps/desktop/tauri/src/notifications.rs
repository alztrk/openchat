//! Bridges validated operating-system notification activations to the frontend.
//!
//! Activations arriving before frontend readiness are held in a bounded queue. Identifiers,
//! reply text, and notification copy are validated before dispatch or native delivery.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::request_bounds::{deserialize_bounded_text, BoundedText};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const NOTIFICATION_ACTIVATED_EVENT: &str = "openchat-notification-activated";
const MAXIMUM_REPLY_CHARACTERS: usize = 4096;
const MAXIMUM_REPLY_UTF16_UNITS: usize = MAXIMUM_REPLY_CHARACTERS * 2;
const MAXIMUM_ACTIVATION_ARGUMENT_UTF16_UNITS: usize = 1024;
const MAXIMUM_NOTIFICATION_INPUT_KEY_UTF16_UNITS: usize = 128;
const MAXIMUM_NOTIFICATION_IDENTIFIER_BYTES: usize = 128;
const MAXIMUM_TEST_NOTIFICATION_TITLE_BYTES: usize = 120 * 4;
const MAXIMUM_NOTIFICATION_TITLE_BYTES: usize = 160 * 4;
const MAXIMUM_NOTIFICATION_BODY_BYTES: usize = 500 * 4;
const MAXIMUM_NOTIFICATION_ACTION_LABEL_BYTES: usize = 80 * 4;
const MAXIMUM_NOTIFICATION_REPLY_LABEL_BYTES: usize = 120 * 4;
const MAXIMUM_NOTIFICATION_CONTENT_CHARS: usize = 200_000;
const MAXIMUM_NOTIFICATION_CONTENT_BYTES: usize = MAXIMUM_NOTIFICATION_CONTENT_CHARS * 4;

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
/// Notification action delivered to the frontend after the activation payload is validated.
pub enum NotificationActivation {
    /// Opens an existing conversation from an assistant-response notification.
    OpenConversation { conversation_id: String },
    /// Opens a conversation and optionally submits a bounded quick reply.
    ReplyToAssistant {
        conversation_id: String,
        assistant_message_id: String,
        reply_text: Option<String>,
        reply_input_too_long: bool,
    },
    /// Opens the conversation and user-question group targeted by a notification.
    OpenQuestion {
        conversation_id: String,
        group_id: String,
    },
}

const MAX_PENDING_ACTIVATIONS: usize = 64;

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
                if let Err(error) = windows_backend::register_activation_server() {
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

fn dispatch_activation(activation: NotificationActivation) -> Result<(), String> {
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

fn activation_setup_result() -> Result<(), String> {
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

/// Deserializes notification identifiers within their existing 128-byte validation limit.
fn deserialize_notification_identifier<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes notification titles within the UTF-8 maximum of their 160-character limit.
fn deserialize_notification_title<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_TITLE_BYTES>(deserializer)
}

/// Deserializes notification bodies within the UTF-8 maximum of their 500-character limit.
fn deserialize_notification_body<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_BODY_BYTES>(deserializer)
}

/// Deserializes notification action labels within the UTF-8 maximum of their 80-character limit.
fn deserialize_notification_action_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_ACTION_LABEL_BYTES>(deserializer)
}

/// Deserializes notification reply labels within the UTF-8 maximum of their 120-character limit.
fn deserialize_notification_reply_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_REPLY_LABEL_BYTES>(deserializer)
}

/// Deserializes assistant notification content within the UTF-8 maximum of its 200,000-character limit.
fn deserialize_notification_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_NOTIFICATION_CONTENT_BYTES>(deserializer)
}

fn activation_arguments(
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
fn parse_activation_arguments(
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
fn reply_text_is_too_long(text: &str) -> bool {
    text.chars().take(MAXIMUM_REPLY_CHARACTERS + 1).count() > MAXIMUM_REPLY_CHARACTERS
}

/// Trims accepted reply text and drops content that exceeds the notification limit.
fn notification_reply_input(value: Option<&str>) -> (Option<String>, bool) {
    let input_too_long = value.is_some_and(reply_text_is_too_long);
    let text = if input_too_long {
        None
    } else {
        value.map(|text| text.trim().to_owned())
    };
    (text, input_too_long)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated display content and actions for an assistant-response notification.
pub struct AssistantResponseNotification {
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    conversation_id: String,
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    assistant_message_id: String,
    #[serde(deserialize_with = "deserialize_notification_title")]
    title: String,
    #[serde(deserialize_with = "deserialize_notification_content")]
    content: String,
    #[serde(deserialize_with = "deserialize_notification_body")]
    generic_body: String,
    #[serde(deserialize_with = "deserialize_notification_action_label")]
    send_label: String,
    #[serde(deserialize_with = "deserialize_notification_reply_label")]
    reply_label: String,
    show_preview: bool,
    allow_quick_reply: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated display content for a notification that opens a user-question group.
pub struct UserQuestionNotification {
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    group_id: String,
    #[serde(deserialize_with = "deserialize_notification_identifier")]
    conversation_id: String,
    #[serde(deserialize_with = "deserialize_notification_title")]
    title: String,
    #[serde(deserialize_with = "deserialize_notification_body")]
    body: String,
}

#[tauri::command]
/// Reports whether native notifications are enabled on the current platform.
pub fn read_windows_notification_status() -> String {
    #[cfg(target_os = "windows")]
    {
        windows_backend::read_status()
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
        windows_backend::open_settings()
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
        windows_backend::send_test(&title, &body)
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
        windows_backend::send_assistant_response(&app, request)
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
        windows_backend::send_user_question(&app, request)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, request);
        Ok(false)
    }
}

fn validate_identifier(value: &str) -> Result<(), String> {
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

fn validate_text(value: &str, maximum_characters: usize, message: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.chars().count() > maximum_characters
        || value.chars().any(char::is_control)
    {
        return Err(message.to_owned());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
mod windows_backend {
    use std::{
        ffi::c_void,
        ffi::OsString,
        os::windows::ffi::OsStrExt,
        ptr,
        sync::{mpsc, Mutex, OnceLock},
        thread,
    };

    use tauri::AppHandle;
    use win32_notif::{
        handler::NotificationActivatedEventHandler,
        notification::{
            actions::{ActionButton, Input},
            visual::{text::HintStyle, Text},
            Scenario,
        },
        Notification, NotificationBuilder, ToastsNotifier,
    };
    use windows::core::{
        implement, w, Error as WindowsError, Interface, BOOL, HRESULT, HSTRING, PCWSTR, PWSTR,
    };
    use windows::core::{IUnknown, Ref, GUID};
    use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_INVALIDARG, E_POINTER};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::{
        PropVariantClear, PROPVARIANT, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoRegisterClassObject, CoTaskMemAlloc, CoTaskMemFree,
        CoUninitialize, IClassFactory, IClassFactory_Impl, IPersistFile, CLSCTX_INPROC_SERVER,
        CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, REGCLS_MULTIPLEUSE,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Notifications::{
        INotificationActivationCallback, INotificationActivationCallback_Impl,
        NOTIFICATION_USER_INPUT_DATA,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath,
        SetCurrentProcessExplicitAppUserModelID, KF_FLAG_CREATE,
    };
    use windows::UI::Notifications::NotificationSetting;

    use super::{
        activation_arguments, activation_setup_result, dispatch_activation,
        notification_reply_input, parse_activation_arguments, AssistantResponseNotification,
        NotificationActivation, UserQuestionNotification, MAXIMUM_ACTIVATION_ARGUMENT_UTF16_UNITS,
        MAXIMUM_NOTIFICATION_INPUT_KEY_UTF16_UNITS, MAXIMUM_REPLY_UTF16_UNITS,
    };

    const APP_USER_MODEL_ID: &str = "OpenChat.Desktop";
    const REPLY_INPUT_ID: &str = "assistant_reply";
    const REPLY_ACTION_ID: &str = "assistant_reply_send";
    const MAXIMUM_RETAINED_NOTIFICATIONS: usize = 64;
    const ACTIVATOR_CLASS_ID: GUID = GUID::from_u128(0x7db420de_9ea4_4e36_91f5_8d2224017381);
    const ACTIVATOR_CLASS_ID_TEXT: &str = "7db420de-9ea4-4e36-91f5-8d2224017381";

    static APP_IDENTITY: OnceLock<Result<(), String>> = OnceLock::new();
    static NOTIFIER: OnceLock<Result<ToastsNotifier, String>> = OnceLock::new();
    static ACTIVE_NOTIFICATIONS: OnceLock<Mutex<Vec<Notification<'static>>>> = OnceLock::new();

    /// Keeps a successfully initialized COM apartment alive for its owning worker thread.
    struct ComApartmentGuard;

    impl Drop for ComApartmentGuard {
        fn drop(&mut self) {
            // SAFETY: This guard is created only after COM initialization succeeds on this thread.
            unsafe { CoUninitialize() };
        }
    }

    pub(super) fn read_status() -> String {
        let Ok(notifier) = notifier() else {
            return "unavailable".to_owned();
        };
        // SAFETY: The notifier owns a live COM interface for the duration of this call.
        let setting = unsafe { notifier.as_raw().Setting() };
        match setting {
            Ok(NotificationSetting::Enabled) => "enabled",
            Ok(NotificationSetting::DisabledForApplication) => "disabledForApplication",
            Ok(NotificationSetting::DisabledForUser) => "disabledForUser",
            Ok(NotificationSetting::DisabledByGroupPolicy) => "disabledByGroupPolicy",
            Ok(NotificationSetting::DisabledByManifest) => "disabledByManifest",
            Ok(_) => "unavailable",
            Err(_) => "unavailable",
        }
        .to_owned()
    }

    pub(super) fn open_settings() -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let operation: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let uri: Vec<u16> = std::ffi::OsStr::new("ms-settings:notifications")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: Both UTF-16 strings are NUL-terminated and remain alive for the call.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                uri.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            return Err("Windows notification settings could not be opened.".to_owned());
        }
        Ok(())
    }

    pub(super) fn register_activation_server() -> Result<(), String> {
        APP_IDENTITY.get_or_init(register_app_identity).clone()
    }

    pub(super) fn send_test(title: &str, body: &str) -> Result<bool, String> {
        if read_status() != "enabled" {
            return Ok(false);
        }
        let notifier = notifier()?;
        let notification = NotificationBuilder::new()
            .visual(Text::create(0, title).with_style(HintStyle::Title))
            .visual(Text::create(1, body).with_style(HintStyle::Body))
            .build(1, notifier, "openchat-test", "openchat-test")
            .map_err(|_| "The Windows test notification could not be created.".to_owned())?;
        notification
            .show()
            .map_err(|_| "The Windows test notification could not be sent.".to_owned())?;
        Ok(true)
    }

    pub(super) fn send_assistant_response(
        _app: &AppHandle,
        request: AssistantResponseNotification,
    ) -> Result<bool, String> {
        if read_status() != "enabled" {
            return Ok(false);
        }
        let body = if request.show_preview {
            assistant_response_preview(&request.content)
        } else {
            request.generic_body.clone()
        };
        if body.is_empty() {
            return Ok(false);
        }

        let conversation_id = request.conversation_id.clone();
        let assistant_message_id = request.assistant_message_id.clone();
        let handler = NotificationActivatedEventHandler::new(move |_notification, args| {
            let activation = match args.as_ref().and_then(|args| args.button_id.as_deref()) {
                Some(REPLY_ACTION_ID) => {
                    let (reply_text, reply_input_too_long) = notification_reply_input(
                        args.as_ref()
                            .and_then(|args| args.user_input.get(REPLY_INPUT_ID))
                            .map(String::as_str),
                    );
                    NotificationActivation::ReplyToAssistant {
                        conversation_id: conversation_id.clone(),
                        assistant_message_id: assistant_message_id.clone(),
                        reply_text,
                        reply_input_too_long,
                    }
                }
                Some(arguments) => {
                    let (reply_text, reply_input_too_long) = notification_reply_input(
                        args.as_ref()
                            .and_then(|args| args.user_input.get(REPLY_INPUT_ID))
                            .map(String::as_str),
                    );
                    parse_activation_arguments(
                        arguments,
                        reply_text.as_deref(),
                        reply_input_too_long,
                    )
                    .map_err(|error| WindowsError::new(HRESULT(0x8007_0057u32 as i32), error))?
                }
                None => NotificationActivation::OpenConversation {
                    conversation_id: conversation_id.clone(),
                },
            };
            dispatch_activation(activation).map_err(|error| {
                WindowsError::new(windows::core::HRESULT(0x8000_4005u32 as i32), error)
            })
        });

        let notifier = notifier()?;
        let mut builder = NotificationBuilder::new()
            .visual(Text::create(0, &request.title).with_style(HintStyle::Title))
            .visual(Text::create(1, &body).with_style(HintStyle::Body))
            .with_scenario(Scenario::Default)
            .on_activated(handler);
        if request.allow_quick_reply {
            let reply_arguments = activation_arguments(
                &NotificationActivation::ReplyToAssistant {
                    conversation_id: request.conversation_id.clone(),
                    assistant_message_id: request.assistant_message_id.clone(),
                    reply_text: None,
                    reply_input_too_long: false,
                },
                true,
            )?;
            builder = builder
                .action(Input::create_text_input(
                    REPLY_INPUT_ID,
                    &request.reply_label,
                    &request.reply_label,
                ))
                .action(
                    ActionButton::create(&request.send_label)
                        .with_input_id(REPLY_INPUT_ID)
                        .with_id(&reply_arguments),
                );
        }

        let tag = notification_tag(&request.conversation_id);
        let notification = builder
            .build(1, notifier, &tag, "assistant-response")
            .map_err(|_| "The Windows response notification could not be created.".to_owned())?;
        set_launch_arguments(
            &notification,
            &activation_arguments(
                &NotificationActivation::OpenConversation {
                    conversation_id: request.conversation_id.clone(),
                },
                false,
            )?,
        )?;
        notification
            .show()
            .map_err(|_| "The Windows response notification could not be sent.".to_owned())?;
        retain_notification(notification)?;
        Ok(true)
    }

    pub(super) fn send_user_question(
        _app: &AppHandle,
        request: UserQuestionNotification,
    ) -> Result<bool, String> {
        if read_status() != "enabled" {
            return Ok(false);
        }
        let conversation_id = request.conversation_id.clone();
        let group_id = request.group_id.clone();
        let handler = NotificationActivatedEventHandler::new(move |_notification, _args| {
            dispatch_activation(NotificationActivation::OpenQuestion {
                conversation_id: conversation_id.clone(),
                group_id: group_id.clone(),
            })
            .map_err(|error| {
                WindowsError::new(windows::core::HRESULT(0x8000_4005u32 as i32), error)
            })
        });

        let notifier = notifier()?;
        let notification = NotificationBuilder::new()
            .visual(Text::create(0, &request.title).with_style(HintStyle::Title))
            .visual(Text::create(1, &request.body).with_style(HintStyle::Body))
            .with_scenario(Scenario::Default)
            .on_activated(handler)
            .build(
                1,
                notifier,
                &notification_tag(&request.group_id),
                "user-question",
            )
            .map_err(|_| "The Windows question notification could not be created.".to_owned())?;
        set_launch_arguments(
            &notification,
            &activation_arguments(
                &NotificationActivation::OpenQuestion {
                    conversation_id: request.conversation_id,
                    group_id: request.group_id,
                },
                false,
            )?,
        )?;
        notification
            .show()
            .map_err(|_| "The Windows question notification could not be sent.".to_owned())?;
        retain_notification(notification)?;
        Ok(true)
    }

    fn notifier() -> Result<&'static ToastsNotifier, String> {
        activation_setup_result()?;
        APP_IDENTITY.get_or_init(register_app_identity).clone()?;
        match NOTIFIER.get_or_init(|| {
            ToastsNotifier::new(Some(APP_USER_MODEL_ID))
                .map_err(|_| "Windows notification delivery is unavailable.".to_owned())
        }) {
            Ok(notifier) => Ok(notifier),
            Err(error) => Err(error.clone()),
        }
    }

    fn register_app_identity() -> Result<(), String> {
        // SAFETY: The macro supplies a static NUL-terminated application identifier.
        unsafe { SetCurrentProcessExplicitAppUserModelID(w!("OpenChat.Desktop")) }.map_err(
            |_| "Windows could not register the OpenChat notification identity.".to_owned(),
        )?;
        ensure_toast_shortcut()?;
        register_custom_activator()?;
        start_activation_class_server()
    }

    fn register_custom_activator() -> Result<(), String> {
        let class_id = format!("{{{ACTIVATOR_CLASS_ID_TEXT}}}");
        set_registry_string(
            "Software\\Classes\\AppUserModelId\\OpenChat.Desktop",
            "CustomActivator",
            &OsString::from(class_id),
        )?;

        let executable = std::env::current_exe()
            .map_err(|_| "Windows could not locate the OpenChat application.".to_owned())?;
        let mut local_server = OsString::from("\"");
        local_server.push(executable.as_os_str());
        local_server.push("\" -Embedding");
        let subkey =
            format!("Software\\Classes\\CLSID\\{{{ACTIVATOR_CLASS_ID_TEXT}}}\\LocalServer32");
        set_registry_string(&subkey, "", &local_server)?;
        Ok(())
    }

    fn set_registry_string(
        subkey: &str,
        value_name: &str,
        value: &std::ffi::OsStr,
    ) -> Result<(), String> {
        use windows_sys::Win32::Foundation::ERROR_SUCCESS;
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
            REG_OPTION_NON_VOLATILE, REG_SZ,
        };

        let subkey = encode_wide(subkey);
        let value_name = encode_wide(value_name);
        let value = value
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut key: HKEY = ptr::null_mut();
        // SAFETY: Both UTF-16 strings are NUL-terminated and live through the registry call; `key` is writable.
        let create_result = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        };
        if create_result != ERROR_SUCCESS {
            return Err(
                "Windows could not register the OpenChat notification activator.".to_owned(),
            );
        }
        // SAFETY: `value` is a live u16 vector; its byte length is the initialized element count times two.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                value.as_ptr().cast::<u8>(),
                value.len() * std::mem::size_of::<u16>(),
            )
        };
        // SAFETY: `key` was created successfully, and the name/data buffers remain valid for the call.
        let write_result = unsafe {
            RegSetValueExW(
                key,
                if value_name.len() == 1 {
                    ptr::null()
                } else {
                    value_name.as_ptr()
                },
                0,
                REG_SZ,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        // SAFETY: `key` is the valid handle returned by the successful create call above.
        let close_result = unsafe { RegCloseKey(key) };
        if write_result != ERROR_SUCCESS || close_result != ERROR_SUCCESS {
            return Err(
                "Windows could not register the OpenChat notification activator.".to_owned(),
            );
        }
        Ok(())
    }

    fn encode_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn set_launch_arguments(
        notification: &Notification<'static>,
        arguments: &str,
    ) -> Result<(), String> {
        // SAFETY: `notification` owns the live WinRT notification object for this call.
        let document = unsafe { notification.as_raw().Content() }
            .map_err(|_| "Windows could not configure notification activation.".to_owned())?;
        let root = document
            .DocumentElement()
            .map_err(|_| "Windows could not configure notification activation.".to_owned())?;
        root.SetAttribute(&HSTRING::from("launch"), &HSTRING::from(arguments))
            .map_err(|_| "Windows could not configure notification activation.".to_owned())
    }

    #[implement(INotificationActivationCallback)]
    struct ToastActivationCallback;

    impl INotificationActivationCallback_Impl for ToastActivationCallback_Impl {
        fn Activate(
            &self,
            _app_user_model_id: &PCWSTR,
            invoked_arguments: &PCWSTR,
            data: *const NOTIFICATION_USER_INPUT_DATA,
            count: u32,
        ) -> windows::core::Result<()> {
            if invoked_arguments.is_null() || count > 32 || (count > 0 && data.is_null()) {
                return Err(WindowsError::new(
                    E_INVALIDARG,
                    "The Windows notification activation data is invalid.",
                ));
            }

            // SAFETY: COM supplies a readable, null-terminated activation string for this callback.
            let arguments = unsafe {
                bounded_wide_string(invoked_arguments.0, MAXIMUM_ACTIVATION_ARGUMENT_UTF16_UNITS)
            }
            .map_err(|_| {
                WindowsError::new(
                    E_INVALIDARG,
                    "The Windows notification activation data is invalid.",
                )
            })?;
            let mut reply_text = None;
            let mut reply_input_too_long = false;
            let mut reply_input_seen = false;
            let inputs: &[NOTIFICATION_USER_INPUT_DATA] = if count == 0 {
                &[]
            } else {
                // SAFETY: The COM callback contract provides `count` initialized entries at `data`; null was rejected above and count is capped at 32.
                unsafe { std::slice::from_raw_parts(data, count as usize) }
            };
            for item in inputs {
                if item.Key.is_null() || item.Value.is_null() {
                    return Err(WindowsError::new(
                        E_INVALIDARG,
                        "The Windows notification reply data is invalid.",
                    ));
                }
                // SAFETY: COM supplies readable, null-terminated notification input strings.
                let key = unsafe {
                    bounded_wide_string(item.Key.0, MAXIMUM_NOTIFICATION_INPUT_KEY_UTF16_UNITS)
                }
                .map_err(|_| {
                    WindowsError::new(
                        E_INVALIDARG,
                        "The Windows notification reply data is invalid.",
                    )
                })?;
                if key == REPLY_INPUT_ID {
                    if reply_input_seen {
                        return Err(WindowsError::new(
                            E_INVALIDARG,
                            "The Windows notification reply data is invalid.",
                        ));
                    }
                    reply_input_seen = true;
                    // SAFETY: COM supplies readable, null-terminated notification input strings.
                    match unsafe { bounded_wide_string(item.Value.0, MAXIMUM_REPLY_UTF16_UNITS) } {
                        Ok(value) => reply_text = Some(value),
                        Err(BoundedWideStringError::TooLong) => reply_input_too_long = true,
                        Err(BoundedWideStringError::Invalid) => {
                            return Err(WindowsError::new(
                                E_INVALIDARG,
                                "The Windows notification reply data is invalid.",
                            ));
                        }
                    }
                }
            }

            let activation =
                parse_activation_arguments(&arguments, reply_text.as_deref(), reply_input_too_long)
                    .map_err(|_| {
                        WindowsError::new(
                            E_INVALIDARG,
                            "The Windows notification activation data is invalid.",
                        )
                    })?;
            dispatch_activation(activation).map_err(|_| {
                WindowsError::new(
                    windows::core::HRESULT(0x8000_4005u32 as i32),
                    "The notification activation could not reach OpenChat.",
                )
            })
        }
    }

    #[implement(IClassFactory)]
    struct ActivationClassFactory;

    impl IClassFactory_Impl for ActivationClassFactory_Impl {
        fn CreateInstance(
            &self,
            outer: Ref<IUnknown>,
            interface_id: *const GUID,
            object: *mut *mut c_void,
        ) -> windows::core::Result<()> {
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            if interface_id.is_null() || object.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: COM provided a writable output pointer, and null checks above reject absent pointers.
            unsafe { *object = ptr::null_mut() };
            let callback: INotificationActivationCallback = ToastActivationCallback.into();
            // SAFETY: COM provided a readable interface identifier and writable output pointer; both were checked above.
            unsafe { callback.query(interface_id, object).ok() }
        }

        fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
            Ok(())
        }
    }

    fn start_activation_class_server() -> Result<(), String> {
        let (started, ready) = mpsc::channel();
        thread::Builder::new()
            .name("openchat-toast-activator".to_owned())
            .spawn(move || {
                // SAFETY: This worker thread has not initialized COM and passes no raw pointers.
                let apartment = match unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
                    Ok(()) => ComApartmentGuard,
                    Err(_) => {
                        let _ =
                            started
                                .send(Err("Windows could not initialize notification activation."
                                    .to_owned()));
                        return;
                    }
                };
                let registration = (|| {
                    let factory: IClassFactory = ActivationClassFactory.into();
                    let factory: IUnknown = factory.cast().map_err(|_| {
                        "Windows could not create the notification activation factory.".to_owned()
                    })?;
                    // SAFETY: COM was initialized on this thread and both interface references remain alive for registration.
                    unsafe {
                        CoRegisterClassObject(
                            &ACTIVATOR_CLASS_ID,
                            &factory,
                            CLSCTX_LOCAL_SERVER,
                            REGCLS_MULTIPLEUSE,
                        )
                    }
                    .map_err(|_| "Windows could not register notification activation.".to_owned())
                })();
                match registration {
                    Ok(cookie) => {
                        let _ = started.send(Ok(()));
                        let _registration_cookie = cookie;
                        loop {
                            thread::park();
                        }
                    }
                    Err(error) => {
                        drop(apartment);
                        let _ = started.send(Err(error));
                    }
                }
            })
            .map_err(|_| "Windows could not start notification activation.".to_owned())?;
        ready
            .recv()
            .map_err(|_| "Windows could not start notification activation.".to_owned())?
    }

    fn ensure_toast_shortcut() -> Result<(), String> {
        use windows::Win32::Foundation::RPC_E_CHANGED_MODE;

        // SAFETY: This call initializes COM for the current thread without passing raw pointers.
        let initialization = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let uninitialize = if initialization.is_ok() {
            true
        } else if initialization == RPC_E_CHANGED_MODE {
            false
        } else {
            return Err("Windows could not prepare notification registration.".to_owned());
        };

        let result = create_toast_shortcut();
        if uninitialize {
            // SAFETY: This thread successfully initialized COM above and has not changed apartments.
            unsafe { CoUninitialize() };
        }
        result
    }

    fn create_toast_shortcut() -> Result<(), String> {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        const CLSID_SHELL_LINK: windows::core::GUID =
            windows::core::GUID::from_u128(0x00021401_0000_0000_c000_000000000046);

        // SAFETY: COM is initialized on this thread and the class/interface identifiers are valid constants.
        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER) }.map_err(
                |_| "Windows could not create the OpenChat notification shortcut.".to_owned(),
            )?;
        let executable = std::env::current_exe()
            .map_err(|_| "Windows could not locate the OpenChat application.".to_owned())?;
        let executable_wide = executable
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let executable_path = PCWSTR::from_raw(executable_wide.as_ptr());
        // SAFETY: `executable_path` points to a live NUL-terminated buffer for the duration of all calls.
        unsafe {
            shell_link
                .SetPath(executable_path)
                .and_then(|()| shell_link.SetDescription(w!("OpenChat")))
                .and_then(|()| shell_link.SetIconLocation(executable_path, 0))
        }
        .map_err(|_| {
            "Windows could not configure the OpenChat notification shortcut.".to_owned()
        })?;

        let property_store: IPropertyStore = shell_link.cast().map_err(|_| {
            "Windows could not configure the OpenChat notification identity.".to_owned()
        })?;
        let app_user_model_id = encode_wide("OpenChat.Desktop");
        // SAFETY: The requested allocation size matches the live UTF-16 source buffer copied below.
        let app_user_model_id_buffer = unsafe {
            CoTaskMemAlloc(std::mem::size_of_val(app_user_model_id.as_slice())).cast::<u16>()
        };
        if app_user_model_id_buffer.is_null() {
            return Err("Windows could not allocate notification identity data.".to_owned());
        }
        // SAFETY: The allocation is non-null and large enough for all source elements; the buffers do not overlap.
        unsafe {
            ptr::copy_nonoverlapping(
                app_user_model_id.as_ptr(),
                app_user_model_id_buffer,
                app_user_model_id.len(),
            );
        }
        let mut app_id_value = PROPVARIANT::default();
        app_id_value.Anonymous.Anonymous = std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
            vt: VT_LPWSTR,
            Anonymous: PROPVARIANT_0_0_0 {
                pwszVal: PWSTR::from_raw(app_user_model_id_buffer),
            },
            ..Default::default()
        });
        // SAFETY: `app_id_value` contains the valid allocated UTF-16 string and is cleared immediately after these calls.
        let property_result = unsafe {
            property_store
                .SetValue(&PKEY_AppUserModel_ID, &app_id_value)
                .and_then(|()| property_store.Commit())
        };
        // SAFETY: `app_id_value` is initialized and owns the string allocation exactly once.
        let clear_result = unsafe { PropVariantClear(&mut app_id_value) };
        property_result
            .map_err(|_| "Windows could not save the OpenChat notification identity.".to_owned())?;
        clear_result
            .map_err(|_| "Windows could not release notification registration data.".to_owned())?;

        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|_| "Windows could not save the OpenChat notification shortcut.".to_owned())?;
        // SAFETY: COM is initialized on this thread; the API returns an owned CoTaskMem path on success.
        let programs_path =
            unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_CREATE, None) }.map_err(
                |_| "Windows could not locate the Start Menu programs folder.".to_owned(),
            )?;
        // SAFETY: The API returned a readable NUL-terminated UTF-16 path owned by `programs_path`.
        let path = unsafe {
            let mut length = 0;
            while *programs_path.0.add(length) != 0 {
                length += 1;
            }
            std::ffi::OsString::from_wide(std::slice::from_raw_parts(programs_path.0, length))
        };
        // SAFETY: `programs_path` is the CoTaskMem allocation returned above and has not been freed yet.
        unsafe { CoTaskMemFree(Some(programs_path.0.cast())) };
        let shortcut_path = std::path::PathBuf::from(path).join("OpenChat.lnk");
        let shortcut_wide = shortcut_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: `shortcut_wide` is a live NUL-terminated path buffer and `persist_file` is a live COM interface.
        unsafe { persist_file.Save(PCWSTR::from_raw(shortcut_wide.as_ptr()), true) }
            .map_err(|_| "Windows could not save the OpenChat notification shortcut.".to_owned())
    }

    fn retain_notification(notification: Notification<'static>) -> Result<(), String> {
        let active = ACTIVE_NOTIFICATIONS.get_or_init(|| Mutex::new(Vec::new()));
        let mut notifications = active
            .lock()
            .map_err(|_| "Windows notification actions are unavailable.".to_owned())?;
        if notifications.len() == MAXIMUM_RETAINED_NOTIFICATIONS {
            notifications.remove(0);
        }
        notifications.push(notification);
        Ok(())
    }

    fn assistant_response_preview(content: &str) -> String {
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut characters = normalized.chars();
        let preview = characters.by_ref().take(160).collect::<String>();
        if characters.next().is_some() {
            let mut shortened = preview.chars().take(159).collect::<String>();
            shortened.push('…');
            shortened
        } else {
            preview
        }
    }

    fn notification_tag(conversation_id: &str) -> String {
        let id = conversation_id.chars().take(48).collect::<String>();
        format!("response-{id}")
    }

    #[derive(Debug, PartialEq, Eq)]
    enum BoundedWideStringError {
        Invalid,
        TooLong,
    }

    /// Converts a bounded, null-terminated UTF-16 string supplied by the Windows COM API.
    ///
    /// # Safety
    /// `value` must remain readable through its first null terminator, or for at least
    /// `maximum_units + 1` code units when the string exceeds the bound.
    unsafe fn bounded_wide_string(
        value: *const u16,
        maximum_units: usize,
    ) -> Result<String, BoundedWideStringError> {
        if value.is_null() {
            return Err(BoundedWideStringError::Invalid);
        }

        let mut units = Vec::with_capacity(maximum_units.min(256));
        for index in 0..=maximum_units {
            // SAFETY: The caller guarantees readability through the terminator or `maximum_units + 1` units.
            let unit = unsafe { *value.add(index) };
            if unit == 0 {
                return String::from_utf16(&units).map_err(|_| BoundedWideStringError::Invalid);
            }
            if index == maximum_units {
                return Err(BoundedWideStringError::TooLong);
            }
            units.push(unit);
        }
        unreachable!()
    }

    #[cfg(test)]
    mod tests {
        use super::{bounded_wide_string, BoundedWideStringError};

        #[test]
        fn bounded_wide_string_accepts_terminated_text_at_the_limit() {
            let value = [b'h' as u16, b'i' as u16, 0];
            // SAFETY: The array contains a null terminator after two readable code units.
            let result = unsafe { bounded_wide_string(value.as_ptr(), 2) };
            assert_eq!(result, Ok("hi".to_owned()));
        }

        #[test]
        fn bounded_wide_string_rejects_oversized_or_invalid_utf16() {
            let oversized = [b'a' as u16, b'b' as u16, b'c' as u16, 0];
            // SAFETY: The array contains more code units than the bound and a terminator.
            let oversized_result = unsafe { bounded_wide_string(oversized.as_ptr(), 2) };
            assert_eq!(oversized_result, Err(BoundedWideStringError::TooLong));

            let invalid_utf16 = [0xD800, 0];
            // SAFETY: The array contains an invalid code unit followed by a terminator.
            let invalid_result = unsafe { bounded_wide_string(invalid_utf16.as_ptr(), 1) };
            assert_eq!(invalid_result, Err(BoundedWideStringError::Invalid));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        activation_arguments, notification_reply_input, parse_activation_arguments,
        AssistantResponseNotification, NotificationActivation, MAXIMUM_NOTIFICATION_CONTENT_BYTES,
        MAXIMUM_REPLY_CHARACTERS,
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
}
