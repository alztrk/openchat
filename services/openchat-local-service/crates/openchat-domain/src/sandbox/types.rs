//! Common domain types, limits, and Win32 string/handle utilities for the Windows sandbox.

use std::{
    ffi::OsStr,
    io,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
};

use windows_sys::Win32::{
    Foundation::{GetLastError, HANDLE},
    System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
};
use zeroize::Zeroizing;

pub(crate) const ACTIVE_PROCESS_LIMIT: u32 = 128;
pub(crate) const MAX_JOB_MEMORY_LIMIT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub(crate) const MAX_PROCESS_MEMORY_LIMIT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const MIN_JOB_MEMORY_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

/// Configures the minimum extra permissions required by a Windows AppContainer process.
#[derive(Default)]
pub struct SandboxOptions<'a> {
    /// Existing runtime directories that the child may read and execute.
    pub additional_read_only_paths: &'a [PathBuf],
    /// Additional project directories that the child may read and modify.
    pub additional_workspace_paths: &'a [PathBuf],
    /// Explicit base variables for launchers that must not inherit the host profile.
    pub base_environment: Option<&'a [(String, Zeroizing<String>)]>,
    /// Current host PATH to use for a newly spawned process, when available.
    pub path_override: Option<&'a OsStr>,
    /// Optional private file where shell wrappers report their final filesystem directory.
    pub working_directory_state_file: Option<&'a Path>,
    /// Grants only the Windows `internetClient` capability.
    pub internet_client: bool,
    /// Optional limits applied to the entire child process tree.
    pub job_limits: Option<SandboxJobLimits>,
}

/// Process-count and memory ceilings applied to the sandbox job object.
#[derive(Clone, Copy, Debug)]
pub struct SandboxJobLimits {
    /// Maximum number of processes allowed in the job.
    pub active_process_limit: u32,
    /// Maximum committed memory allowed for each process.
    pub process_memory_limit_bytes: usize,
    /// Maximum committed memory allowed for the complete process tree.
    pub job_memory_limit_bytes: usize,
}

impl SandboxJobLimits {
    /// Derives conservative process and memory ceilings from the host's physical memory.
    ///
    /// The process-tree limit is half of physical memory, clamped between 512 MiB and
    /// 8 GiB. A single process is capped at 4 GiB or the full job limit on smaller hosts.
    /// If Windows cannot report physical memory, the caller must fail closed.
    pub fn for_current_host() -> io::Result<Self> {
        let mut memory_status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `memory_status` is initialized with its required size and remains writable.
        if unsafe { GlobalMemoryStatusEx(&mut memory_status) } == 0 {
            return Err(last_os_error());
        }

        Self::for_physical_memory(memory_status.ullTotalPhys)
    }

    pub(crate) fn for_physical_memory(total_memory_bytes: u64) -> io::Result<Self> {
        if total_memory_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows reported no physical memory.",
            ));
        }

        let job_memory_limit_bytes = (total_memory_bytes / 2)
            .clamp(MIN_JOB_MEMORY_LIMIT_BYTES, MAX_JOB_MEMORY_LIMIT_BYTES)
            .min(usize::MAX as u64) as usize;
        let process_memory_limit_bytes = job_memory_limit_bytes
            .min(MAX_PROCESS_MEMORY_LIMIT_BYTES.min(usize::MAX as u64) as usize);

        Ok(Self {
            active_process_limit: ACTIVE_PROCESS_LIMIT,
            process_memory_limit_bytes,
            job_memory_limit_bytes,
        })
    }
}

/// Accepts bounded uppercase Windows environment names used by the sandbox base profile.
pub(crate) fn is_valid_base_environment_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && value.len() <= 128
}

/// Rejects base and temporary variables from caller-supplied additional environment values.
pub(crate) fn is_valid_additional_environment_name(value: &str) -> bool {
    is_valid_base_environment_name(value)
        && !matches!(
            value,
            "PATH"
                | "SYSTEMROOT"
                | "WINDIR"
                | "PATHEXT"
                | "COMSPEC"
                | "USERPROFILE"
                | "APPDATA"
                | "LOCALAPPDATA"
                | "TEMP"
                | "TMP"
        )
}

/// Converts a successful Win32 handle result into a Rust owner.
pub(crate) fn owned_handle(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle as isize == -1 {
        return Err(last_os_error());
    }
    // SAFETY: The successful Win32 call transferred ownership of this non-null, non-invalid handle here.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
}

/// Encodes an operating-system string as NUL-terminated UTF-16 for Win32 calls.
pub(crate) fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// Encodes a Rust string as NUL-terminated UTF-16 for Win32 calls.
pub(crate) fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Rejects embedded NULs before a value is passed to a NUL-terminated Win32 API.
pub(crate) fn validate_win32_string(value: &OsStr) -> io::Result<()> {
    if value.encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Process command-line values must not contain NUL characters.",
        ));
    }
    Ok(())
}

/// Builds a mutable Win32 command line with each executable and argument quoted separately.
pub(crate) fn wide_command_line(application: &Path, arguments: &[std::ffi::OsString]) -> Vec<u16> {
    let mut command_line = quote_windows_argument(application.as_os_str());
    for argument in arguments {
        command_line.push(' ');
        command_line.push_str(&quote_windows_argument(argument));
    }
    wide_null(&command_line)
}

/// Escapes quotes and backslashes using the Windows command-line parsing convention.
pub(crate) fn quote_windows_argument(argument: &std::ffi::OsStr) -> String {
    let argument = argument.to_string_lossy();
    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2).saturating_add(1)));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2)));
    quoted.push('"');
    quoted
}

/// Captures the calling thread's last Win32 error immediately after a failing API call.
pub(crate) fn last_os_error() -> io::Error {
    // SAFETY: GetLastError is thread-local and may be read immediately after a failing Win32 call.
    io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
}

/// Removes extended-length prefixes that PowerShell cannot use for its working directory.
pub(crate) fn shell_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(unc_path) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc_path}"));
    }
    if let Some(local_path) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(local_path);
    }
    path.to_path_buf()
}
