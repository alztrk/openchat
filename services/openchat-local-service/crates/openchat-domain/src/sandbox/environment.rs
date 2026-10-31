//! Reads the current Windows user and machine environment for newly spawned tools.

use std::{ffi::OsString, io, mem::size_of, os::windows::ffi::OsStringExt, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_EXPAND_SZ, REG_SZ, RRF_RT_REG_EXPAND_SZ,
        RRF_RT_REG_SZ, RegGetValueW,
    },
};

use super::types::wide_null;

const MACHINE_ENVIRONMENT_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const USER_ENVIRONMENT_KEY: &str = "Environment";
const MAX_REGISTRY_PATH_BYTES: u32 = 1024 * 1024;
const MAX_REGISTRY_READ_ATTEMPTS: usize = 3;

/// Builds the current effective Windows `PATH` from machine and user environment values.
///
/// This avoids inheriting the local-service process's startup-time environment snapshot.
pub(crate) fn fresh_path() -> io::Result<OsString> {
    let machine_path = read_registry_path(HKEY_LOCAL_MACHINE, MACHINE_ENVIRONMENT_KEY)?
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "The machine PATH is unavailable.")
        })?;
    let user_path = read_registry_path(HKEY_CURRENT_USER, USER_ENVIRONMENT_KEY)?;

    let mut path = machine_path;
    if let Some(user_path) = user_path.filter(|value| !value.is_empty()) {
        if !path.is_empty() {
            path.push(";");
        }
        path.push(user_path);
    }

    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The current Windows PATH is empty.",
        ));
    }

    Ok(path)
}

fn read_registry_path(root: HKEY, key: &str) -> io::Result<Option<OsString>> {
    let key = wide_null(key);
    let value_name = wide_null("Path");
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
    let mut required_bytes = 0u32;

    // SAFETY: The registry paths and value name are NUL-terminated, and `required_bytes` is writable.
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value_name.as_ptr(),
            flags,
            null_mut(),
            null_mut(),
            &mut required_bytes,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    if required_bytes > MAX_REGISTRY_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The Windows PATH registry value exceeds the supported size.",
        ));
    }

    let mut buffer = vec![0u16; (required_bytes as usize).div_ceil(size_of::<u16>()) + 1];
    for _ in 0..MAX_REGISTRY_READ_ATTEMPTS {
        let mut bytes =
            u32::try_from(buffer.len().saturating_mul(size_of::<u16>())).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "The Windows PATH is too large.")
            })?;
        let mut value_type = 0u32;

        // SAFETY: The buffer is writable for `bytes` and all pointers stay valid for the call.
        let status = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                value_name.as_ptr(),
                flags,
                &mut value_type,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status == ERROR_MORE_DATA {
            if bytes > MAX_REGISTRY_PATH_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The Windows PATH registry value exceeds the supported size.",
                ));
            }
            buffer.resize((bytes as usize).div_ceil(size_of::<u16>()) + 1, 0);
            continue;
        }
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        if value_type != REG_SZ && value_type != REG_EXPAND_SZ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The Windows PATH registry value has an unsupported type.",
            ));
        }

        let units = (bytes as usize / size_of::<u16>()).min(buffer.len());
        let end = buffer[..units]
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units);
        return Ok(Some(OsString::from_wide(&buffer[..end])));
    }

    Err(io::Error::new(
        io::ErrorKind::Interrupted,
        "The Windows PATH changed repeatedly while it was being read.",
    ))
}
