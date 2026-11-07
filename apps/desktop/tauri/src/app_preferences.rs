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

#[cfg_attr(
    any(
        feature = "startup-benchmark",
        feature = "desktop-profile-test-support"
    ),
    allow(dead_code)
)]
pub fn should_minimize_on_close(state: &AppPreferencesState) -> bool {
    state.minimize_on_close.load(Ordering::Acquire)
}

#[tauri::command]
pub fn set_windows_startup_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        set_windows_startup_entry(enabled)
    }

    #[cfg(target_os = "linux")]
    {
        set_linux_startup_entry(enabled)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = enabled;
        Err(
            "Starting OpenChat with the operating system is unavailable on this platform."
                .to_owned(),
        )
    }
}

#[cfg(target_os = "linux")]
fn set_linux_startup_entry(enabled: bool) -> Result<(), String> {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        path::PathBuf,
    };

    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|path| std::path::Path::new(path).is_absolute())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config"))
                .filter(|path| path.is_absolute())
        })
        .ok_or_else(|| "The Linux user configuration directory is unavailable.".to_owned())?;
    let autostart_directory = config_home.join("autostart");
    let entry_path = autostart_directory.join("openchat.desktop");

    match fs::symlink_metadata(&entry_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("The OpenChat startup entry is not a regular file.".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("The OpenChat startup entry could not be inspected.".to_owned()),
    }

    if !enabled {
        match fs::remove_file(entry_path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("The OpenChat startup entry could not be removed.".to_owned()),
        }
    }

    let executable = std::env::current_exe()
        .map_err(|_| "Linux could not locate the OpenChat application.".to_owned())?;
    let executable = executable
        .to_str()
        .ok_or_else(|| "The OpenChat application path is not valid UTF-8.".to_owned())?;
    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=OpenChat\nExec={}\nX-GNOME-Autostart-enabled=true\n",
        escape_desktop_exec_argument(executable)
    );

    fs::create_dir_all(&autostart_directory)
        .map_err(|_| "The Linux autostart directory could not be created.".to_owned())?;
    let temporary_path =
        autostart_directory.join(format!(".openchat-{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary_path)
            .map_err(|_| "The OpenChat startup entry could not be written.".to_owned())?;
        file.write_all(desktop_entry.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|_| "The OpenChat startup entry could not be written.".to_owned())?;
        fs::rename(&temporary_path, &entry_path)
            .map_err(|_| "The OpenChat startup entry could not be installed.".to_owned())
    })();
    if let Err(error) = write_result {
        match fs::remove_file(temporary_path) {
            Ok(()) => return Err(error),
            Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => {
                return Err(error);
            }
            Err(_) => {
                return Err(format!(
                    "{error} The temporary startup file could not be removed."
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn escape_desktop_exec_argument(argument: &str) -> String {
    let mut escaped = String::with_capacity(argument.len() + 2);
    escaped.push('"');
    for character in argument.chars() {
        if matches!(character, '\\' | '"' | '`' | '$') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('"');
    escaped
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

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::escape_desktop_exec_argument;

    #[test]
    fn desktop_exec_argument_quotes_spaces_and_escapes_reserved_characters() {
        assert_eq!(
            escape_desktop_exec_argument("/opt/Open Chat/$app\\name`\""),
            "\"/opt/Open Chat/\\$app\\\\name\\`\\\"\""
        );
    }
}
