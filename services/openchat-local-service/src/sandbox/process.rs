//! Sandboxed process lifecycle, pipe I/O, and shell invocation helpers.

use std::{
    ffi::{OsStr, c_void},
    fs, io,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    time::Duration,
};

use tokio::{fs::File, time::sleep};
use windows_sys::Win32::{
    Foundation::{
        HANDLE, HANDLE_FLAG_INHERIT, STILL_ACTIVE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::{PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES},
    System::{
        JobObjects::{AssignProcessToJobObject, TerminateJobObject},
        Pipes::CreatePipe,
        SystemServices::SE_GROUP_ENABLED,
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
            InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
            STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};
use zeroize::Zeroizing;

use super::{
    appcontainer::{AppContainerSandbox, DerivedCapabilitySids},
    job_objects::{create_kill_on_close_job, job_active_process_count},
    types::{
        SandboxOptions, last_os_error, owned_handle, quote_windows_argument, validate_win32_string,
        wide, wide_command_line,
    },
};

pub(crate) const WAIT_INTERVAL: Duration = Duration::from_millis(25);

/// Owns a managed process, its pipes, and its kill-on-close job object.
pub struct SandboxedProcess {
    process: OwnedHandle,
    pub(crate) job: OwnedHandle,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
    _sandbox: Option<AppContainerSandbox>,
}

impl SandboxedProcess {
    /// Starts a PowerShell command with workspace-only write access and no network capability.
    pub fn spawn(command: &str, workdir: &Path) -> io::Result<Self> {
        Self::spawn_with_options(command, workdir, SandboxOptions::default())
    }

    /// Starts a PowerShell command with explicit sandbox permissions and job limits.
    pub fn spawn_with_options(
        command: &str,
        workdir: &Path,
        options: SandboxOptions<'_>,
    ) -> io::Result<Self> {
        validate_win32_string(OsStr::new(command))?;
        let sandbox = AppContainerSandbox::create(
            workdir,
            options.additional_workspace_paths,
            options.additional_read_only_paths,
        )?;
        let workdir = sandbox.working_directory.clone();
        let (application, arguments) = shell_invocation(command, &workdir);
        Self::spawn_process(Some(sandbox), application, arguments, workdir, &[], options)
    }

    /// Starts a PowerShell command with the current user's normal filesystem and network access.
    pub fn spawn_without_sandbox(
        command: &str,
        workdir: &Path,
        options: SandboxOptions<'_>,
    ) -> io::Result<Self> {
        validate_win32_string(OsStr::new(command))?;
        let (application, arguments) = shell_invocation(command, workdir);
        Self::spawn_process(
            None,
            application,
            arguments,
            workdir.to_path_buf(),
            &[],
            options,
        )
    }

    /// Starts a program with workspace-only write access and no network capability.
    pub fn spawn_program(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
    ) -> io::Result<Self> {
        Self::spawn_program_with_environment(program, arguments, workdir, &[])
    }

    /// Starts a program with only the listed additional environment variables.
    pub fn spawn_program_with_environment(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
    ) -> io::Result<Self> {
        Self::spawn_program_with_options(
            program,
            arguments,
            workdir,
            additional_environment,
            SandboxOptions::default(),
        )
    }

    /// Starts a program with explicit runtime access, environment, capabilities, and job limits.
    pub fn spawn_program_with_options(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
        options: SandboxOptions<'_>,
    ) -> io::Result<Self> {
        validate_win32_string(program.as_os_str())?;
        for argument in arguments {
            validate_win32_string(argument.as_os_str())?;
        }
        let program = fs::canonicalize(program)?;
        if !program.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandboxed program is unavailable.",
            ));
        }
        let sandbox = AppContainerSandbox::create(
            workdir,
            options.additional_workspace_paths,
            options.additional_read_only_paths,
        )?;
        let working_directory = sandbox.working_directory.clone();
        Self::spawn_process(
            Some(sandbox),
            program,
            arguments.to_vec(),
            working_directory,
            additional_environment,
            options,
        )
    }

    /// Starts the same executable as the long-lived, networkless storage AppContainer.
    pub(crate) fn spawn_storage_broker(program: &Path) -> io::Result<Self> {
        validate_win32_string(program.as_os_str())?;
        let program = fs::canonicalize(program)?;
        let (sandbox, _) = AppContainerSandbox::create_storage_broker(&program)?;
        let working_directory = sandbox.working_directory.clone();
        Self::spawn_process(
            Some(sandbox),
            program,
            vec![std::ffi::OsString::from("--storage-broker-stdio")],
            working_directory,
            &[],
            SandboxOptions::default(),
        )
    }

    /// Starts a program with the current user's normal filesystem and network access.
    pub fn spawn_program_without_sandbox(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
        options: SandboxOptions<'_>,
    ) -> io::Result<Self> {
        validate_win32_string(program.as_os_str())?;
        for argument in arguments {
            validate_win32_string(argument.as_os_str())?;
        }
        let program = fs::canonicalize(program)?;
        if !program.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The configured program is not a file.",
            ));
        }
        Self::spawn_process(
            None,
            program,
            arguments.to_vec(),
            workdir.to_path_buf(),
            additional_environment,
            options,
        )
    }

    fn spawn_process(
        sandbox: Option<AppContainerSandbox>,
        application: PathBuf,
        arguments: Vec<std::ffi::OsString>,
        workdir: PathBuf,
        additional_environment: &[(String, Zeroizing<String>)],
        options: SandboxOptions<'_>,
    ) -> io::Result<Self> {
        let security_attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: 1,
            lpSecurityDescriptor: null_mut(),
        };

        let (child_stdin_read, parent_stdin_write) = create_pipe(&security_attributes)?;
        let (parent_stdout_read, child_stdout_write) = create_pipe(&security_attributes)?;
        let (parent_stderr_read, child_stderr_write) = create_pipe(&security_attributes)?;
        // SAFETY: These handles are owned by the parent process and remain open while inheritance is disabled.
        unsafe {
            for handle in [
                parent_stdin_write.as_raw_handle() as HANDLE,
                parent_stdout_read.as_raw_handle() as HANDLE,
                parent_stderr_read.as_raw_handle() as HANDLE,
            ] {
                if windows_sys::Win32::Foundation::SetHandleInformation(
                    handle,
                    HANDLE_FLAG_INHERIT,
                    0,
                ) == 0
                {
                    return Err(last_os_error());
                }
            }
        }
        let child_handles = [
            child_stdin_read.as_raw_handle() as HANDLE,
            child_stdout_write.as_raw_handle() as HANDLE,
            child_stderr_write.as_raw_handle() as HANDLE,
        ];
        let capability_sids = if sandbox.is_some() && options.internet_client {
            Some(DerivedCapabilitySids::internet_client()?)
        } else {
            None
        };
        let mut internet_client = capability_sids.as_ref().map(|sids| SID_AND_ATTRIBUTES {
            Sid: sids.capability_sid(),
            Attributes: SE_GROUP_ENABLED as u32,
        });
        let mut security_capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: sandbox
                .as_ref()
                .map_or(null_mut(), |sandbox| sandbox.sid as PSID),
            Capabilities: internet_client.as_mut().map_or(null_mut(), |capability| {
                capability as *mut SID_AND_ATTRIBUTES
            }),
            CapabilityCount: u32::from(internet_client.is_some()),
            Reserved: 0,
        };

        let mut attribute_size = 0usize;
        // SAFETY: The null probe asks Windows for the required size; the output integer is writable.
        let attribute_count = if sandbox.is_some() { 2 } else { 1 };
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), attribute_count, 0, &mut attribute_size);
        }
        if attribute_size == 0 {
            return Err(last_os_error());
        }
        let mut attribute_storage = vec![0usize; attribute_size.div_ceil(size_of::<usize>())];
        let attribute_list = attribute_storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: `attribute_storage` is sized from the Windows probe and aligned as usize storage.
        unsafe {
            if InitializeProcThreadAttributeList(
                attribute_list,
                attribute_count,
                0,
                &mut attribute_size,
            ) == 0
            {
                return Err(last_os_error());
            }
        }
        let _attribute_list = AttributeList(attribute_list);
        // SAFETY: The attribute list and all referenced values remain initialized and live through both updates.
        unsafe {
            if sandbox.is_some()
                && UpdateProcThreadAttribute(
                    attribute_list,
                    0,
                    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                    (&mut security_capabilities as *mut SECURITY_CAPABILITIES).cast(),
                    size_of::<SECURITY_CAPABILITIES>(),
                    null_mut(),
                    null_mut(),
                ) == 0
            {
                return Err(last_os_error());
            }
            if UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                child_handles.as_ptr().cast(),
                size_of::<[HANDLE; 3]>(),
                null_mut(),
                null_mut(),
            ) == 0
            {
                return Err(last_os_error());
            }
        }

        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = child_handles[0];
        startup.StartupInfo.hStdOutput = child_handles[1];
        startup.StartupInfo.hStdError = child_handles[2];
        startup.lpAttributeList = attribute_list;
        let mut process_information = PROCESS_INFORMATION::default();
        let application_wide = wide(application.as_os_str());
        let mut command_line = wide_command_line(&application, &arguments);
        let workdir_wide = wide(workdir.as_os_str());
        let mut environment = match sandbox.as_ref() {
            Some(sandbox) => Some(Zeroizing::new(sandbox.environment_block(
                &workdir,
                additional_environment,
                options.base_environment,
            )?)),
            None => Some(inherited_environment_block(additional_environment)?),
        };
        // SAFETY: All Win32 strings are NUL-terminated; startup, environment, and output structures stay live and writable for the call.
        let created = unsafe {
            CreateProcessW(
                application_wide.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_UNICODE_ENVIRONMENT
                    | CREATE_NO_WINDOW,
                environment
                    .as_mut()
                    .map_or(null_mut(), |environment| environment.as_mut_ptr().cast()),
                workdir_wide.as_ptr(),
                (&startup as *const STARTUPINFOEXW).cast::<STARTUPINFOW>(),
                &mut process_information,
            )
        };
        if created == 0 {
            return Err(io::Error::other(format!(
                "CreateProcessW: {}",
                last_os_error()
            )));
        }
        let process = owned_handle(process_information.hProcess)?;
        let thread = match owned_handle(process_information.hThread) {
            Ok(thread) => thread,
            Err(error) => {
                // SAFETY: The process handle is owned and valid; termination completes before the handle is dropped.
                unsafe {
                    TerminateProcess(process.as_raw_handle() as HANDLE, 1);
                    WaitForSingleObject(process.as_raw_handle() as HANDLE, 5_000);
                }
                return Err(error);
            }
        };
        let job = match create_kill_on_close_job(options.job_limits) {
            Ok(job) => job,
            Err(error) => {
                // SAFETY: The process handle is owned and valid; termination completes before the handle is dropped.
                unsafe {
                    TerminateProcess(process.as_raw_handle() as HANDLE, 1);
                    WaitForSingleObject(process.as_raw_handle() as HANDLE, 5_000);
                }
                return Err(error);
            }
        };
        // SAFETY: Both process handles are owned and valid; the child is still suspended until it is assigned to the job.
        unsafe {
            if AssignProcessToJobObject(
                job.as_raw_handle() as HANDLE,
                process.as_raw_handle() as HANDLE,
            ) == 0
            {
                let error = last_os_error();
                TerminateProcess(process.as_raw_handle() as HANDLE, 1);
                WaitForSingleObject(process.as_raw_handle() as HANDLE, 5_000);
                return Err(error);
            }
            if ResumeThread(thread.as_raw_handle() as HANDLE) == u32::MAX {
                let error = last_os_error();
                TerminateJobObject(job.as_raw_handle() as HANDLE, 1);
                WaitForSingleObject(process.as_raw_handle() as HANDLE, 5_000);
                return Err(error);
            }
        }
        drop(thread);
        drop(child_stdin_read);
        drop(child_stdout_write);
        drop(child_stderr_write);

        Ok(Self {
            process,
            job,
            // SAFETY: `into_raw_handle` transfers each uniquely owned pipe handle to `from_raw_handle` exactly once.
            stdin: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stdin_write.into_raw_handle())
            })),
            // SAFETY: `into_raw_handle` transfers each uniquely owned pipe handle to `from_raw_handle` exactly once.
            stdout: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stdout_read.into_raw_handle())
            })),
            // SAFETY: `into_raw_handle` transfers each uniquely owned pipe handle to `from_raw_handle` exactly once.
            stderr: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stderr_read.into_raw_handle())
            })),
            _sandbox: sandbox,
        })
    }

    /// Takes the parent side of the process's standard input pipe.
    pub fn take_stdin(&mut self) -> Option<File> {
        self.stdin.take()
    }

    /// Takes the parent side of the process's standard output pipe.
    pub fn take_stdout(&mut self) -> Option<File> {
        self.stdout.take()
    }

    /// Takes the parent side of the process's standard error pipe.
    pub fn take_stderr(&mut self) -> Option<File> {
        self.stderr.take()
    }

    /// Takes the process's standard input pipe as a blocking standard-library file.
    pub(crate) fn take_stdin_blocking(&mut self) -> io::Result<std::fs::File> {
        take_blocking_file(
            &mut self.stdin,
            "The storage broker input pipe is unavailable.",
        )
    }

    /// Takes the process's standard output pipe as a blocking standard-library file.
    pub(crate) fn take_stdout_blocking(&mut self) -> io::Result<std::fs::File> {
        take_blocking_file(
            &mut self.stdout,
            "The storage broker output pipe is unavailable.",
        )
    }

    /// Takes the process's standard error pipe as a blocking standard-library file.
    pub(crate) fn take_stderr_blocking(&mut self) -> io::Result<std::fs::File> {
        take_blocking_file(
            &mut self.stderr,
            "The storage broker error pipe is unavailable.",
        )
    }

    /// Checks whether the primary process has exited without blocking.
    pub fn try_wait(&self) -> io::Result<Option<std::process::ExitStatus>> {
        // SAFETY: `self.process` is an owned, live process handle.
        let wait_result = unsafe { WaitForSingleObject(self.process.as_raw_handle() as HANDLE, 0) };
        match wait_result {
            WAIT_OBJECT_0 => {
                let mut exit_code = STILL_ACTIVE as u32;
                // SAFETY: The process handle is valid and `exit_code` is a writable output value.
                if unsafe {
                    GetExitCodeProcess(self.process.as_raw_handle() as HANDLE, &mut exit_code)
                } == 0
                {
                    return Err(last_os_error());
                }
                Ok(Some(std::process::ExitStatus::from_raw(exit_code)))
            }
            WAIT_TIMEOUT => Ok(None),
            WAIT_FAILED => Err(last_os_error()),
            _ => Err(io::Error::other(
                "The command process returned an unknown wait result.",
            )),
        }
    }

    /// Terminates every process assigned to the managed job.
    pub fn start_kill(&self) -> io::Result<()> {
        // SAFETY: `self.job` is an owned, live job handle.
        if unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) } == 0 {
            let termination_error = last_os_error();
            if job_active_process_count(self.job.as_raw_handle() as HANDLE)? == 0 {
                return Ok(());
            }
            return Err(termination_error);
        }
        Ok(())
    }

    /// Waits asynchronously for the primary process to exit.
    pub async fn wait(&self) -> io::Result<std::process::ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            sleep(WAIT_INTERVAL).await;
        }
    }

    /// Waits up to five seconds for the managed process tree to leave its job object.
    pub fn wait_for_job_exit(&self) -> io::Result<()> {
        // SAFETY: `self.job` is an owned, live job handle.
        match unsafe { WaitForSingleObject(self.job.as_raw_handle() as HANDLE, 5_000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The managed process tree did not stop in time.",
            )),
            WAIT_FAILED => Err(last_os_error()),
            _ => Err(io::Error::other(
                "The managed process tree returned an unknown wait result.",
            )),
        }
    }
}

fn take_blocking_file(
    slot: &mut Option<File>,
    unavailable_message: &'static str,
) -> io::Result<std::fs::File> {
    let file = slot
        .take()
        .ok_or_else(|| io::Error::other(unavailable_message))?;
    match file.try_into_std() {
        Ok(file) => Ok(file),
        Err(file) => {
            *slot = Some(file);
            Err(io::Error::other(unavailable_message))
        }
    }
}

impl Drop for SandboxedProcess {
    fn drop(&mut self) {
        // SAFETY: `self.job` remains owned and live throughout this destructor.
        if unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) } == 0 {
            eprintln!("managed_process_tree_termination_failed");
        }
        if self.wait_for_job_exit().is_err() {
            eprintln!("managed_process_tree_cleanup_failed");
        }
    }
}

/// Frees an initialized `STARTUPINFOEXW` process-attribute list on every return path.
struct AttributeList(*mut c_void);

impl Drop for AttributeList {
    /// Releases the Windows process-attribute list before its backing storage is dropped.
    fn drop(&mut self) {
        // SAFETY: The wrapper is created only after successful attribute-list initialization.
        unsafe { DeleteProcThreadAttributeList(self.0) };
    }
}

/// Creates an owned anonymous pipe pair with the supplied inheritance attributes.
pub(crate) fn create_pipe(
    attributes: &SECURITY_ATTRIBUTES,
) -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = null_mut();
    let mut write = null_mut();
    // SAFETY: The security attributes are live and both output handle pointers are writable.
    if unsafe { CreatePipe(&mut read, &mut write, attributes, 0) } == 0 {
        return Err(last_os_error());
    }
    let read = owned_handle(read)?;
    let write = owned_handle(write)?;
    Ok((read, write))
}

/// Selects PowerShell and constructs arguments for a sandboxed shell command.
pub(crate) fn shell_invocation(
    command: &str,
    working_directory: &Path,
) -> (PathBuf, Vec<std::ffi::OsString>) {
    let pwsh = Path::new(r"C:\Program Files\PowerShell\7\pwsh.exe");
    if pwsh.exists() {
        return (
            pwsh.to_path_buf(),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                powershell_command(command, working_directory).into(),
            ],
        );
    }
    let powershell = Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
    if powershell.exists() {
        return (
            powershell.to_path_buf(),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                powershell_command(command, working_directory).into(),
            ],
        );
    }
    (
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        vec![
            "/D".into(),
            "/S".into(),
            "/C".into(),
            format!(
                "cd /D {} && {command}",
                quote_windows_argument(working_directory.as_os_str())
            )
            .into(),
        ],
    )
}

/// Wraps a command so PowerShell starts in the requested directory and preserves its exit code.
pub(crate) fn powershell_command(command: &str, working_directory: &Path) -> String {
    let working_directory = working_directory.to_string_lossy().replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'Stop'; [System.Environment]::CurrentDirectory = '{working_directory}'; Set-Location -LiteralPath '{working_directory}'; {command}"
    )
}

fn inherited_environment_block(
    additional_environment: &[(String, Zeroizing<String>)],
) -> io::Result<Zeroizing<Vec<u16>>> {
    let mut variables = std::env::vars_os().collect::<Vec<_>>();
    for (name, value) in additional_environment {
        if name.is_empty() || name.contains('=') || name.contains('\0') || value.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Process environment variables contain an invalid name or value.",
            ));
        }
        if let Some((_, existing_value)) = variables
            .iter_mut()
            .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case(name))
        {
            *existing_value = value.as_str().into();
        } else {
            variables.push((name.into(), value.as_str().into()));
        }
    }
    variables.sort_unstable_by(|(left, _), (right, _)| {
        left.to_string_lossy()
            .to_lowercase()
            .cmp(&right.to_string_lossy().to_lowercase())
    });

    let mut block = Zeroizing::new(Vec::new());
    for (name, value) in variables {
        block.extend(name.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    if block.len() == 1 {
        block.push(0);
    }
    Ok(block)
}
