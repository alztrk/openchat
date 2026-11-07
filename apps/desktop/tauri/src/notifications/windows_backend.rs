//! Windows-specific COM registration, Shell link shortcuts, and WinRT toast notifications.

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
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::Notifications::{
    INotificationActivationCallback, INotificationActivationCallback_Impl,
    NOTIFICATION_USER_INPUT_DATA,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath, SetCurrentProcessExplicitAppUserModelID,
    KF_FLAG_CREATE,
};
use windows::UI::Notifications::NotificationSetting;

use super::{
    activation::{
        activation_arguments, activation_setup_result, dispatch_activation,
        notification_reply_input, parse_activation_arguments, require_reply_confirmation,
        NotificationActivation, MAXIMUM_ACTIVATION_ARGUMENT_UTF16_UNITS,
        MAXIMUM_NOTIFICATION_INPUT_KEY_UTF16_UNITS, MAXIMUM_REPLY_UTF16_UNITS,
    },
    commands::{AssistantResponseNotification, UserQuestionNotification},
};

#[derive(Clone, Copy)]
struct ToastIdentity {
    app_user_model_id: &'static str,
    activator_class_id: GUID,
    activator_class_id_text: &'static str,
    shortcut_name: &'static str,
}

// Debug toast registration must not replace the installed app's Search shortcut or COM activator.
const RELEASE_TOAST_IDENTITY: ToastIdentity = ToastIdentity {
    app_user_model_id: "OpenChat.Desktop",
    activator_class_id: GUID::from_u128(0x7db420de_9ea4_4e36_91f5_8d2224017381),
    activator_class_id_text: "7db420de-9ea4-4e36-91f5-8d2224017381",
    shortcut_name: "OpenChat.lnk",
};
const DEVELOPMENT_TOAST_IDENTITY: ToastIdentity = ToastIdentity {
    app_user_model_id: "OpenChat.Desktop.Dev",
    activator_class_id: GUID::from_u128(0x2f4a8c7e_74f3_4e9e_b80d_d84b3d39e1e1),
    activator_class_id_text: "2f4a8c7e-74f3-4e9e-b80d-d84b3d39e1e1",
    shortcut_name: "OpenChat Dev.lnk",
};
const REPLY_INPUT_ID: &str = "assistant_reply";
const REPLY_ACTION_ID: &str = "assistant_reply_send";
const MAXIMUM_RETAINED_NOTIFICATIONS: usize = 64;

static APP_IDENTITY: OnceLock<Result<(), String>> = OnceLock::new();
static TOAST_IDENTITY: OnceLock<Result<(), String>> = OnceLock::new();
static PROCESS_APP_USER_MODEL_ID: OnceLock<Result<(), String>> = OnceLock::new();
type NotificationJob = Box<dyn FnOnce() + Send + 'static>;
static NOTIFICATION_WORKER: OnceLock<Result<mpsc::Sender<NotificationJob>, String>> =
    OnceLock::new();
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

struct WinRtApartmentGuard;

impl Drop for WinRtApartmentGuard {
    fn drop(&mut self) {
        // SAFETY: This guard is created only after WinRT initialization succeeds on this thread.
        unsafe { RoUninitialize() };
    }
}

pub(crate) fn read_status() -> String {
    if TOAST_IDENTITY.get_or_init(register_toast_identity).is_err() {
        return "unavailable".to_owned();
    }

    run_on_notification_thread(|| Ok(read_status_on_current_thread()))
        .unwrap_or_else(|_| "unavailable".to_owned())
}

fn read_status_on_current_thread() -> String {
    let Ok(notifier) = notifier_instance() else {
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

fn run_on_notification_thread<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let sender = NOTIFICATION_WORKER
        .get_or_init(start_notification_worker)
        .as_ref()
        .map_err(Clone::clone)?;
    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    sender
        .send(Box::new(move || {
            let _ = response_sender.send(operation());
        }))
        .map_err(|_| "Windows notification delivery is unavailable.".to_owned())?;
    response_receiver
        .recv()
        .map_err(|_| "Windows notification delivery is unavailable.".to_owned())?
}

fn start_notification_worker() -> Result<mpsc::Sender<NotificationJob>, String> {
    let (job_sender, job_receiver) = mpsc::channel::<NotificationJob>();
    let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("openchat-windows-notifications".to_owned())
        .spawn(move || {
            // SAFETY: This dedicated thread owns one WinRT MTA for its full lifetime.
            if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
                let _ = startup_sender.send(Err(
                    "Windows could not initialize notification delivery.".to_owned(),
                ));
                return;
            }
            let _apartment = WinRtApartmentGuard;
            if startup_sender.send(Ok(())).is_err() {
                return;
            }
            while let Ok(job) = job_receiver.recv() {
                job();
            }
        })
        .map_err(|_| "Windows could not start notification delivery.".to_owned())?;
    startup_receiver
        .recv()
        .map_err(|_| "Windows could not start notification delivery.".to_owned())??;
    Ok(job_sender)
}

fn is_delivery_blocked(status: &str) -> bool {
    matches!(
        status,
        "disabledForApplication"
            | "disabledForUser"
            | "disabledByGroupPolicy"
            | "disabledByManifest"
    )
}

pub(crate) fn open_settings() -> Result<(), String> {
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

pub(crate) fn register_activation_server() -> Result<(), String> {
    APP_IDENTITY.get_or_init(register_app_identity).clone()
}

pub(crate) fn send_test(title: &str, body: &str) -> Result<bool, String> {
    if is_delivery_blocked(&read_status()) {
        return Ok(false);
    }
    let title = title.to_owned();
    let body = body.to_owned();
    run_on_notification_thread(move || {
        let notifier = delivery_notifier()?;
        let notification = NotificationBuilder::new()
            .visual(Text::create(0, &title).with_style(HintStyle::Title))
            .visual(Text::create(1, &body).with_style(HintStyle::Body))
            .build(1, notifier, "openchat-test", "openchat-test")
            .map_err(|_| "The Windows test notification could not be created.".to_owned())?;
        notification
            .show()
            .map_err(|_| "The Windows test notification could not be sent.".to_owned())?;
        Ok(true)
    })
}

pub(crate) fn send_assistant_response(
    _app: &AppHandle,
    request: AssistantResponseNotification,
) -> Result<bool, String> {
    if is_delivery_blocked(&read_status()) {
        return Ok(false);
    }
    run_on_notification_thread(move || send_assistant_response_on_notification_thread(request))
}

fn send_assistant_response_on_notification_thread(
    request: AssistantResponseNotification,
) -> Result<bool, String> {
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
                    confirmation_required: false,
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
                parse_activation_arguments(arguments, reply_text.as_deref(), reply_input_too_long)
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

    let notifier = activation_notifier()?;
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
                confirmation_required: false,
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

pub(crate) fn send_user_question(
    _app: &AppHandle,
    request: UserQuestionNotification,
) -> Result<bool, String> {
    if is_delivery_blocked(&read_status()) {
        return Ok(false);
    }
    run_on_notification_thread(move || send_user_question_on_notification_thread(request))
}

fn send_user_question_on_notification_thread(
    request: UserQuestionNotification,
) -> Result<bool, String> {
    let conversation_id = request.conversation_id.clone();
    let group_id = request.group_id.clone();
    let handler = NotificationActivatedEventHandler::new(move |_notification, _args| {
        dispatch_activation(NotificationActivation::OpenQuestion {
            conversation_id: conversation_id.clone(),
            group_id: group_id.clone(),
        })
        .map_err(|error| WindowsError::new(windows::core::HRESULT(0x8000_4005u32 as i32), error))
    });

    let notifier = activation_notifier()?;
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

fn activation_notifier() -> Result<&'static ToastsNotifier, String> {
    activation_setup_result()?;
    APP_IDENTITY.get_or_init(register_app_identity).clone()?;
    delivery_notifier()
}

// Test toasts only verify delivery, so they must not depend on COM activation setup.
fn delivery_notifier() -> Result<&'static ToastsNotifier, String> {
    TOAST_IDENTITY
        .get_or_init(register_toast_identity)
        .clone()?;
    notifier_instance()
}

fn notifier_instance() -> Result<&'static ToastsNotifier, String> {
    match NOTIFIER.get_or_init(|| {
        ToastsNotifier::new(Some(current_toast_identity().app_user_model_id))
            .map_err(|_| "Windows notification delivery is unavailable.".to_owned())
    }) {
        Ok(notifier) => Ok(notifier),
        Err(error) => Err(error.clone()),
    }
}

fn register_app_identity() -> Result<(), String> {
    TOAST_IDENTITY
        .get_or_init(register_toast_identity)
        .clone()?;
    register_custom_activator()?;
    start_activation_class_server()
}

pub(crate) fn set_process_app_user_model_id() -> Result<(), String> {
    PROCESS_APP_USER_MODEL_ID
        .get_or_init(|| {
            let app_user_model_id = encode_wide(current_toast_identity().app_user_model_id);
            // SAFETY: The identity buffer is NUL-terminated and remains live through the API call.
            unsafe {
                SetCurrentProcessExplicitAppUserModelID(PCWSTR::from_raw(
                    app_user_model_id.as_ptr(),
                ))
            }
            .map_err(|_| {
                "Windows could not register the OpenChat notification identity.".to_owned()
            })
        })
        .clone()
}

fn register_toast_identity() -> Result<(), String> {
    set_process_app_user_model_id()?;
    ensure_toast_shortcut()
}

fn register_custom_activator() -> Result<(), String> {
    let identity = current_toast_identity();
    let class_id = format!("{{{}}}", identity.activator_class_id_text);
    set_registry_string(
        &format!(
            "Software\\Classes\\AppUserModelId\\{}",
            identity.app_user_model_id
        ),
        "CustomActivator",
        &OsString::from(class_id),
    )?;

    let executable = std::env::current_exe()
        .map_err(|_| "Windows could not locate the OpenChat application.".to_owned())?;
    let mut local_server = OsString::from("\"");
    local_server.push(executable.as_os_str());
    local_server.push("\" -Embedding");
    let subkey = format!(
        "Software\\Classes\\CLSID\\{{{}}}\\LocalServer32",
        identity.activator_class_id_text
    );
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
        return Err("Windows could not register the OpenChat notification activator.".to_owned());
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
        return Err("Windows could not register the OpenChat notification activator.".to_owned());
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

        let mut activation =
            parse_activation_arguments(&arguments, reply_text.as_deref(), reply_input_too_long)
                .map_err(|_| {
                    WindowsError::new(
                        E_INVALIDARG,
                        "The Windows notification activation data is invalid.",
                    )
                })?;
        require_reply_confirmation(&mut activation);
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
                    let _ = started.send(Err(
                        "Windows could not initialize notification activation.".to_owned(),
                    ));
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
                        &current_toast_identity().activator_class_id,
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
    use std::os::windows::ffi::OsStringExt;

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
    .map_err(|_| "Windows could not configure the OpenChat notification shortcut.".to_owned())?;

    let property_store: IPropertyStore = shell_link.cast().map_err(|_| {
        "Windows could not configure the OpenChat notification identity.".to_owned()
    })?;
    let identity = current_toast_identity();
    let app_user_model_id = encode_wide(identity.app_user_model_id);
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
        unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_CREATE, None) }
            .map_err(|_| "Windows could not locate the Start Menu programs folder.".to_owned())?;
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
    let shortcut_path = std::path::PathBuf::from(path).join(identity.shortcut_name);
    let shortcut_wide = shortcut_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: `shortcut_wide` is a live NUL-terminated path buffer and `persist_file` is a live COM interface.
    unsafe { persist_file.Save(PCWSTR::from_raw(shortcut_wide.as_ptr()), true) }
        .map_err(|_| "Windows could not save the OpenChat notification shortcut.".to_owned())
}

fn toast_identity(is_development_build: bool) -> &'static ToastIdentity {
    if is_development_build {
        &DEVELOPMENT_TOAST_IDENTITY
    } else {
        &RELEASE_TOAST_IDENTITY
    }
}

fn current_toast_identity() -> &'static ToastIdentity {
    toast_identity(cfg!(debug_assertions))
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
    use super::{bounded_wide_string, is_delivery_blocked, toast_identity, BoundedWideStringError};

    #[test]
    fn unknown_notification_status_allows_delivery_attempts() {
        assert!(!is_delivery_blocked("enabled"));
        assert!(!is_delivery_blocked("unavailable"));
        assert!(is_delivery_blocked("disabledForApplication"));
        assert!(is_delivery_blocked("disabledForUser"));
        assert!(is_delivery_blocked("disabledByGroupPolicy"));
        assert!(is_delivery_blocked("disabledByManifest"));
    }

    #[test]
    fn development_notifications_use_separate_search_and_activation_registration() {
        let development = toast_identity(true);
        let release = toast_identity(false);

        assert_eq!(development.app_user_model_id, "OpenChat.Desktop.Dev");
        assert_eq!(development.shortcut_name, "OpenChat Dev.lnk");
        assert_eq!(release.app_user_model_id, "OpenChat.Desktop");
        assert_eq!(release.shortcut_name, "OpenChat.lnk");
        assert_ne!(development.app_user_model_id, release.app_user_model_id);
        assert_ne!(development.activator_class_id, release.activator_class_id);
        assert_ne!(
            development.activator_class_id_text,
            release.activator_class_id_text
        );
        assert_ne!(development.shortcut_name, release.shortcut_name);
    }

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
