//! Native application preferences that need operating-system integration.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

#[derive(Default)]
pub struct AppPreferencesState {
    minimize_on_close: AtomicBool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseWindowBehavior {
    Exit,
    Minimize,
}

#[tauri::command]
pub fn set_close_window_behavior(
    state: State<'_, AppPreferencesState>,
    behavior: CloseWindowBehavior,
) {
    state.minimize_on_close.store(
        matches!(behavior, CloseWindowBehavior::Minimize),
        Ordering::Release,
    );
}

pub fn should_minimize_on_close(state: &AppPreferencesState) -> bool {
    state.minimize_on_close.load(Ordering::Acquire)
}

#[tauri::command]
pub fn set_windows_startup_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        set_windows_startup_entry(enabled)
    }

    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err(
            "Starting OpenChat with the operating system is unavailable on this platform."
                .to_owned(),
        )
    }
}

#[cfg(windows)]
fn set_windows_startup_entry(enabled: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "OpenChat";

    let value = if enabled {
        let executable = std::env::current_exe()
            .map_err(|_| "Windows could not locate the OpenChat application.".to_owned())?;
        let mut command = std::ffi::OsString::from("\"");
        command.push(executable.as_os_str());
        command.push("\"");
        Some(
            command
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let key_path = encode_wide(RUN_KEY);
    let value_name = encode_wide(VALUE_NAME);
    let mut key: HKEY = ptr::null_mut();

    let open_result = if enabled {
        // SAFETY: `key_path` is NUL-terminated and all output pointers are writable.
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        }
    } else {
        // SAFETY: `key_path` is NUL-terminated and `key` is a writable output pointer.
        unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            )
        }
    };

    if !enabled && open_result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if open_result != ERROR_SUCCESS {
        return Err("Windows could not update the OpenChat startup preference.".to_owned());
    }

    let update_result = if enabled {
        let Some(value) = value.as_ref() else {
            // SAFETY: `key` was opened successfully above and must be closed before returning.
            unsafe { RegCloseKey(key) };
            return Err("The startup command is unavailable.".to_owned());
        };
        // SAFETY: `value` is a live UTF-16 vector, and its byte slice covers initialized elements.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                value.as_ptr().cast::<u8>(),
                value.len() * std::mem::size_of::<u16>(),
            )
        };
        // SAFETY: `key` is open and the NUL-terminated name and value buffers remain alive for the call.
        unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        }
    } else {
        // SAFETY: `key` is open and `value_name` is NUL-terminated.
        let result = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
        if result == ERROR_FILE_NOT_FOUND {
            ERROR_SUCCESS
        } else {
            result
        }
    };

    // SAFETY: `key` was opened successfully above and is closed exactly once.
    let close_result = unsafe { RegCloseKey(key) };
    if update_result != ERROR_SUCCESS || close_result != ERROR_SUCCESS {
        return Err("Windows could not update the OpenChat startup preference.".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn encode_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
