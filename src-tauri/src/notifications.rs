use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const NOTIFICATION_ACTIVATED_EVENT: &str = "openchat-notification-activated";

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NotificationActivation {
    OpenConversation {
        conversation_id: String,
    },
    ReplyToAssistant {
        conversation_id: String,
        assistant_message_id: String,
        reply_text: Option<String>,
        reply_input_too_long: bool,
    },
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
pub fn take_pending_notification_activations() -> Result<Vec<NotificationActivation>, String> {
    let runtime = ACTIVATION_RUNTIME.get_or_init(|| Mutex::new(ActivationRuntime::default()));
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Notification activation state is unavailable.".to_owned())?;
    runtime.frontend_ready = true;
    Ok(runtime.pending.drain(..).collect())
}

#[tauri::command]
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

fn parse_activation_arguments(
    arguments: &str,
    reply_text: Option<&str>,
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
            let reply_input_too_long = reply_text.is_some_and(|text| {
                text.encode_utf16().count() > 8192 || text.chars().count() > 4096
            });
            Ok(NotificationActivation::ReplyToAssistant {
                conversation_id: payload.conversation_id,
                assistant_message_id,
                reply_text: reply_text.map(str::trim).map(str::to_owned),
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponseNotification {
    conversation_id: String,
    assistant_message_id: String,
    title: String,
    content: String,
    generic_body: String,
    send_label: String,
    reply_label: String,
    show_preview: bool,
    allow_quick_reply: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuestionNotification {
    group_id: String,
    conversation_id: String,
    title: String,
    body: String,
}

#[tauri::command]
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
pub fn open_windows_notification_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return windows_backend::open_settings();
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows notification settings are not available on this platform.".to_owned())
    }
}

#[tauri::command]
pub fn send_test_notification(title: String, body: String) -> Result<bool, String> {
    validate_text(&title, 120, "The notification title is invalid.")?;
    validate_text(&body, 500, "The notification body is invalid.")?;
    #[cfg(target_os = "windows")]
    {
        return windows_backend::send_test(&title, &body);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

#[tauri::command]
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
    if request.content.chars().count() > 200_000 {
        return Err("The assistant response is too large for a notification.".to_owned());
    }

    #[cfg(target_os = "windows")]
    {
        return windows_backend::send_assistant_response(&app, request);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, request);
        Ok(false)
    }
}

#[tauri::command]
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
        return windows_backend::send_user_question(&app, request);
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
        parse_activation_arguments, AssistantResponseNotification, NotificationActivation,
        UserQuestionNotification,
    };

    const APP_USER_MODEL_ID: &str = "OpenChat.Desktop";
    const REPLY_INPUT_ID: &str = "assistant_reply";
    const REPLY_ACTION_ID: &str = "assistant_reply_send";
    const MAXIMUM_REPLY_CHARACTERS: usize = 4096;
    const MAXIMUM_RETAINED_NOTIFICATIONS: usize = 64;
    const ACTIVATOR_CLASS_ID: GUID = GUID::from_u128(0x7db420de_9ea4_4e36_91f5_8d2224017381);
    const ACTIVATOR_CLASS_ID_TEXT: &str = "7db420de-9ea4-4e36-91f5-8d2224017381";

    static APP_IDENTITY: OnceLock<Result<(), String>> = OnceLock::new();
    static NOTIFIER: OnceLock<Result<ToastsNotifier, String>> = OnceLock::new();
    static ACTIVE_NOTIFICATIONS: OnceLock<Mutex<Vec<Notification<'static>>>> = OnceLock::new();

    pub(super) fn read_status() -> String {
        let Ok(notifier) = notifier() else {
            return "unavailable".to_owned();
        };
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
                Some(REPLY_ACTION_ID) => NotificationActivation::ReplyToAssistant {
                    conversation_id: conversation_id.clone(),
                    assistant_message_id: assistant_message_id.clone(),
                    reply_text: args
                        .as_ref()
                        .and_then(|args| args.user_input.get(REPLY_INPUT_ID))
                        .map(|text| text.trim().to_owned()),
                    reply_input_too_long: args
                        .as_ref()
                        .and_then(|args| args.user_input.get(REPLY_INPUT_ID))
                        .is_some_and(|text| {
                            text.encode_utf16().count() > MAXIMUM_REPLY_CHARACTERS * 2
                                || text.chars().count() > MAXIMUM_REPLY_CHARACTERS
                        }),
                },
                Some(arguments) => parse_activation_arguments(
                    arguments,
                    args.as_ref()
                        .and_then(|args| args.user_input.get(REPLY_INPUT_ID))
                        .map(String::as_str),
                )
                .map_err(|error| WindowsError::new(HRESULT(0x8007_0057u32 as i32), error))?,
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
        let bytes = unsafe {
            std::slice::from_raw_parts(
                value.as_ptr().cast::<u8>(),
                value.len() * std::mem::size_of::<u16>(),
            )
        };
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

            let arguments = unsafe { invoked_arguments.to_string() }.map_err(|_| {
                WindowsError::new(
                    E_INVALIDARG,
                    "The Windows notification activation data is invalid.",
                )
            })?;
            let mut reply_text = None;
            let inputs: &[NOTIFICATION_USER_INPUT_DATA] = if count == 0 {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(data, count as usize) }
            };
            for item in inputs {
                if item.Key.is_null() || item.Value.is_null() {
                    return Err(WindowsError::new(
                        E_INVALIDARG,
                        "The Windows notification reply data is invalid.",
                    ));
                }
                let key = unsafe { item.Key.to_string() }.map_err(|_| {
                    WindowsError::new(
                        E_INVALIDARG,
                        "The Windows notification reply data is invalid.",
                    )
                })?;
                if key == REPLY_INPUT_ID {
                    if reply_text.is_some() {
                        return Err(WindowsError::new(
                            E_INVALIDARG,
                            "The Windows notification reply data is invalid.",
                        ));
                    }
                    reply_text = Some(unsafe { item.Value.to_string() }.map_err(|_| {
                        WindowsError::new(
                            E_INVALIDARG,
                            "The Windows notification reply data is invalid.",
                        )
                    })?);
                }
            }

            let activation = parse_activation_arguments(&arguments, reply_text.as_deref())
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
            unsafe { *object = ptr::null_mut() };
            let callback: INotificationActivationCallback = ToastActivationCallback.into();
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
                let registration = (|| {
                    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                        .ok()
                        .map_err(|_| {
                            "Windows could not initialize notification activation.".to_owned()
                        })?;
                    let factory: IClassFactory = ActivationClassFactory.into();
                    let factory: IUnknown = factory.cast().map_err(|_| {
                        "Windows could not create the notification activation factory.".to_owned()
                    })?;
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
            unsafe { CoUninitialize() };
        }
        result
    }

    fn create_toast_shortcut() -> Result<(), String> {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        const CLSID_SHELL_LINK: windows::core::GUID =
            windows::core::GUID::from_u128(0x00021401_0000_0000_c000_000000000046);

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
        let app_user_model_id_buffer = unsafe {
            CoTaskMemAlloc(std::mem::size_of_val(app_user_model_id.as_slice())).cast::<u16>()
        };
        if app_user_model_id_buffer.is_null() {
            return Err("Windows could not allocate notification identity data.".to_owned());
        }
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
        let property_result = unsafe {
            property_store
                .SetValue(&PKEY_AppUserModel_ID, &app_id_value)
                .and_then(|()| property_store.Commit())
        };
        let clear_result = unsafe { PropVariantClear(&mut app_id_value) };
        property_result
            .map_err(|_| "Windows could not save the OpenChat notification identity.".to_owned())?;
        clear_result
            .map_err(|_| "Windows could not release notification registration data.".to_owned())?;

        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|_| "Windows could not save the OpenChat notification shortcut.".to_owned())?;
        let programs_path =
            unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_CREATE, None) }.map_err(
                |_| "Windows could not locate the Start Menu programs folder.".to_owned(),
            )?;
        let path = unsafe {
            let mut length = 0;
            while *programs_path.0.add(length) != 0 {
                length += 1;
            }
            std::ffi::OsString::from_wide(std::slice::from_raw_parts(programs_path.0, length))
        };
        unsafe { CoTaskMemFree(Some(programs_path.0.cast())) };
        let shortcut_path = std::path::PathBuf::from(path).join("OpenChat.lnk");
        let shortcut_wide = shortcut_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
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
}
