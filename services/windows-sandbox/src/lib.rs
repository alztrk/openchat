//! Windows AppContainer process isolation shared by OpenChat desktop components.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{BTreeMap, HashSet},
    env,
    ffi::{OsStr, c_void},
    fs, io,
    mem::size_of,
    os::windows::fs::MetadataExt,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    ptr::{NonNull, null, null_mut},
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::{fs::File, time::sleep};
use windows_sys::Win32::{
    Foundation::{
        GetLastError, HANDLE, HANDLE_FLAG_INHERIT, STILL_ACTIVE, WAIT_FAILED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile},
    Security::{
        ACL,
        Authorization::{
            ConvertSidToStringSidW, EXPLICIT_ACCESS_W, GetSecurityInfo, REVOKE_ACCESS,
            SE_FILE_OBJECT, SET_ACCESS, SetEntriesInAclW, SetSecurityInfo, TRUSTEE_IS_SID,
            TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
        },
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, DeriveCapabilitySidsFromName, FreeSid,
        OBJECT_INHERIT_ACE, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FILE_TRAVERSE, GetFileInformationByHandle, GetFinalPathNameByHandleW, READ_CONTROL,
        VOLUME_NAME_DOS, WRITE_DAC,
    },
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_LIMIT_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject,
        },
        Pipes::CreatePipe,
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        SystemServices::{MAXIMUM_ALLOWED, SE_GROUP_ENABLED},
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
            InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
            STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, UpdateProcThreadAttribute,
            WaitForSingleObject,
        },
    },
};
use zeroize::Zeroizing;

/// Caps the number of file-system paths scheduled for one AppContainer ACL traversal.
const MAX_ACL_ENTRIES: usize = 20_000;
const ACL_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const ACL_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const WAIT_INTERVAL: Duration = Duration::from_millis(25);
static APP_CONTAINER_ACL_LOCK: Mutex<()> = Mutex::new(());

/// Returns the stable lock-file path shared by OpenChat processes for one local profile.
fn app_container_acl_lock_path(local_app_data: &Path) -> PathBuf {
    local_app_data
        .join("OpenChat")
        .join("appcontainer-acl.lock")
}

const ACTIVE_PROCESS_LIMIT: u32 = 128;
const MAX_JOB_MEMORY_LIMIT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_PROCESS_MEMORY_LIMIT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MIN_JOB_MEMORY_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

/// Configures the minimum extra permissions required by a Windows AppContainer process.
#[derive(Default)]
pub struct SandboxOptions<'a> {
    /// Existing runtime directories that the child may read and execute.
    pub additional_read_only_paths: &'a [PathBuf],
    /// Explicit base variables for launchers that must not inherit the host profile.
    pub base_environment: Option<&'a [(String, Zeroizing<String>)]>,
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

    fn for_physical_memory(total_memory_bytes: u64) -> io::Result<Self> {
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

/// Owns an AppContainer process, its pipes, and its kill-on-close job object.
pub struct SandboxedProcess {
    process: OwnedHandle,
    job: OwnedHandle,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
    _sandbox: AppContainerSandbox,
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
        let sandbox = AppContainerSandbox::create(workdir, options.additional_read_only_paths)?;
        let workdir = sandbox.working_directory.clone();
        let (application, arguments) = shell_invocation(command, &workdir);
        Self::spawn_in_sandbox(sandbox, application, arguments, workdir, &[], options)
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
        let sandbox = AppContainerSandbox::create(workdir, options.additional_read_only_paths)?;
        let working_directory = sandbox.working_directory.clone();
        Self::spawn_in_sandbox(
            sandbox,
            program,
            arguments.to_vec(),
            working_directory,
            additional_environment,
            options,
        )
    }

    fn spawn_in_sandbox(
        sandbox: AppContainerSandbox,
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
        let capability_sids = options
            .internet_client
            .then(DerivedCapabilitySids::internet_client)
            .transpose()?;
        let mut internet_client = capability_sids.as_ref().map(|sids| SID_AND_ATTRIBUTES {
            Sid: sids.capability_sid(),
            Attributes: SE_GROUP_ENABLED as u32,
        });
        let mut security_capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: sandbox.sid as windows_sys::Win32::Security::PSID,
            Capabilities: internet_client.as_mut().map_or(null_mut(), |capability| {
                capability as *mut SID_AND_ATTRIBUTES
            }),
            CapabilityCount: u32::from(internet_client.is_some()),
            Reserved: 0,
        };

        let mut attribute_size = 0usize;
        // SAFETY: The null probe asks Windows for the required size; the output integer is writable.
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut attribute_size);
        }
        if attribute_size == 0 {
            return Err(last_os_error());
        }
        let mut attribute_storage = vec![0usize; attribute_size.div_ceil(size_of::<usize>())];
        let attribute_list = attribute_storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: `attribute_storage` is sized from the Windows probe and aligned as usize storage.
        unsafe {
            if InitializeProcThreadAttributeList(attribute_list, 2, 0, &mut attribute_size) == 0 {
                return Err(last_os_error());
            }
        }
        let _attribute_list = AttributeList(attribute_list);
        // SAFETY: The attribute list and all referenced values remain initialized and live through both updates.
        unsafe {
            if UpdateProcThreadAttribute(
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
        let mut environment = Zeroizing::new(sandbox.environment_block(
            &workdir,
            additional_environment,
            options.base_environment,
        )?);
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
                environment.as_mut_ptr().cast(),
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
                    windows_sys::Win32::System::Threading::TerminateProcess(
                        process.as_raw_handle() as HANDLE,
                        1,
                    );
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
                    windows_sys::Win32::System::Threading::TerminateProcess(
                        process.as_raw_handle() as HANDLE,
                        1,
                    );
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
                windows_sys::Win32::System::Threading::TerminateProcess(
                    process.as_raw_handle() as HANDLE,
                    1,
                );
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

    /// Terminates every process assigned to the sandbox job.
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

    /// Waits up to five seconds for the process tree to leave its job object.
    pub fn wait_for_job_exit(&self) -> io::Result<()> {
        // SAFETY: `self.job` is an owned, live job handle.
        match unsafe { WaitForSingleObject(self.job.as_raw_handle() as HANDLE, 5_000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The AppContainer process tree did not stop in time.",
            )),
            WAIT_FAILED => Err(last_os_error()),
            _ => Err(io::Error::other(
                "The AppContainer process tree returned an unknown wait result.",
            )),
        }
    }
}

impl Drop for SandboxedProcess {
    fn drop(&mut self) {
        // SAFETY: `self.job` remains owned and live throughout this destructor.
        if unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) } == 0 {
            eprintln!("app_container_process_tree_termination_failed");
        }
        if self.wait_for_job_exit().is_err() {
            eprintln!("app_container_process_tree_cleanup_failed");
        }
    }
}

/// Owns a temporary AppContainer profile and the ACL grants made for its process.
struct AppContainerSandbox {
    /// NUL-terminated unique profile name used to remove the profile on drop.
    name: Vec<u16>,
    /// Windows-allocated AppContainer SID retained until profile cleanup.
    sid: usize,
    /// Windows-allocated textual SID used to resolve the profile directory.
    sid_string: usize,
    /// Canonical directory used as the child process working directory.
    working_directory: PathBuf,
    /// Isolated temporary directory exposed to the child through `TEMP` and `TMP`.
    temp_directory: PathBuf,
    /// Paths whose AppContainer ACEs must be revoked when this sandbox is dropped.
    granted_paths: Vec<PathBuf>,
}

impl AppContainerSandbox {
    /// Creates a temporary profile and grants only workspace and selected runtime access.
    ///
    /// # Errors
    /// Returns an error if Windows profile setup, path validation, ACL updates, or directory
    /// traversal fails. Any ACLs already granted are revoked during cleanup.
    fn create(
        working_directory: &Path,
        additional_read_only_paths: &[PathBuf],
    ) -> io::Result<Self> {
        let acl_working_directory = fs::canonicalize(working_directory)?;
        let working_directory = shell_path(&acl_working_directory);
        if !working_directory.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox working directory is unavailable.",
            ));
        }
        let name = wide_null(&format!("OpenChatProcess{}", uuid::Uuid::new_v4().simple()));
        let display_name = wide_null("OpenChat restricted process");
        let description = wide_null("Temporary AppContainer for a restricted OpenChat process.");
        let mut sid = null_mut();
        // SAFETY: The input strings are NUL-terminated and `sid` is a writable output pointer.
        let status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                display_name.as_ptr(),
                description.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if status < 0 || sid.is_null() {
            // SAFETY: Any non-null SID was returned by the failed profile call; the profile name remains live and terminated.
            unsafe {
                if !sid.is_null() {
                    FreeSid(sid);
                }
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other(
                "Windows could not create the AppContainer sandbox.",
            ));
        }

        let mut sid_string = null_mut();
        // SAFETY: `sid` was returned by Windows and `sid_string` is a writable output pointer.
        if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 || sid_string.is_null() {
            let error = last_os_error();
            // SAFETY: Any non-null string was allocated by the conversion call; `sid` and profile name are still valid.
            unsafe {
                if !sid_string.is_null() {
                    windows_sys::Win32::Foundation::LocalFree(sid_string.cast());
                }
                FreeSid(sid);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(error);
        }
        let sid_string = sid_string as usize;
        let mut app_container_data = null_mut();
        // SAFETY: `sid_string` points to the converted NUL-terminated SID and output is writable.
        let folder_result = unsafe {
            windows_sys::Win32::Security::Isolation::GetAppContainerFolderPath(
                sid_string as windows_sys::core::PCWSTR,
                &mut app_container_data,
            )
        };
        if folder_result < 0 || app_container_data.is_null() {
            // SAFETY: These are the still-owned SID/string/profile resources allocated above.
            unsafe {
                FreeSid(sid);
                windows_sys::Win32::Foundation::LocalFree(sid_string as _);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other(
                "Windows could not locate the AppContainer profile folder.",
            ));
        }
        // SAFETY: Windows returned a readable NUL-terminated AppContainer folder path.
        let package_root = PathBuf::from(unsafe { wide_os_string(app_container_data) });
        // SAFETY: `app_container_data` is the CoTaskMem allocation returned by the folder API.
        unsafe { CoTaskMemFree(app_container_data.cast()) };
        let temp_directory = package_root.join("OpenChatProcessTemp");
        if let Err(error) = fs::create_dir_all(&temp_directory) {
            // SAFETY: These are the still-owned SID/string/profile resources allocated above.
            unsafe {
                FreeSid(sid);
                windows_sys::Win32::Foundation::LocalFree(sid_string as _);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(error);
        }

        let mut sandbox = Self {
            name,
            sid: sid as usize,
            sid_string,
            working_directory: working_directory.clone(),
            temp_directory,
            granted_paths: Vec::new(),
        };
        if let Err(error) = grant_user_objects_access(sid) {
            sandbox.cleanup();
            return Err(error);
        }
        if let Err(error) = sandbox.grant_working_directory(acl_working_directory) {
            sandbox.cleanup();
            return Err(error);
        }
        for path in additional_read_only_paths {
            if let Err(error) = sandbox.grant_read_only_path(path) {
                sandbox.cleanup();
                return Err(error);
            }
        }
        Ok(sandbox)
    }

    /// Builds a sorted, NUL-terminated environment containing only the allowed base variables.
    ///
    /// # Errors
    /// Returns an error for invalid variable names, embedded NUL characters, temporary-directory
    /// failures, or an environment block larger than the Windows process limit.
    fn environment_block(
        &self,
        working_directory: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
        base_environment: Option<&[(String, Zeroizing<String>)]>,
    ) -> io::Result<Vec<u16>> {
        let mut variables = BTreeMap::new();
        if let Some(base_environment) = base_environment {
            for (key, value) in base_environment {
                if !is_valid_base_environment_name(key) || value.contains('\0') {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "The sandbox base environment is invalid.",
                    ));
                }
                variables.insert(
                    key.to_ascii_uppercase(),
                    Zeroizing::new(value.encode_utf16().collect::<Vec<_>>()),
                );
            }
        } else {
            for (key, value) in std::env::vars_os() {
                let Some(key_text) = key.to_str() else {
                    continue;
                };
                if matches!(
                    key_text.to_ascii_uppercase().as_str(),
                    "PATH"
                        | "SYSTEMROOT"
                        | "WINDIR"
                        | "PATHEXT"
                        | "COMSPEC"
                        | "USERPROFILE"
                        | "APPDATA"
                        | "LOCALAPPDATA"
                ) {
                    variables.insert(
                        key_text.to_ascii_uppercase(),
                        Zeroizing::new(value.encode_wide().collect::<Vec<_>>()),
                    );
                }
            }
        }
        let working_directory = shell_path(working_directory);
        let working_directory_text = working_directory.to_string_lossy();
        let mut drive_characters = working_directory_text.chars();
        if let (Some(drive), Some(':')) = (drive_characters.next(), drive_characters.next())
            && drive.is_ascii_alphabetic()
        {
            let drive = drive.to_ascii_uppercase();
            variables.insert(
                format!("={drive}:"),
                Zeroizing::new(
                    working_directory
                        .as_os_str()
                        .encode_wide()
                        .collect::<Vec<_>>(),
                ),
            );
        }
        fs::create_dir_all(&self.temp_directory)?;
        variables.insert(
            "TEMP".to_owned(),
            Zeroizing::new(
                self.temp_directory
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        variables.insert(
            "TMP".to_owned(),
            Zeroizing::new(
                self.temp_directory
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        for (key, value) in additional_environment {
            let normalized_key = key.to_ascii_uppercase();
            if !is_valid_additional_environment_name(key)
                || value.contains('\0')
                || matches!(
                    normalized_key.as_str(),
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
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The sandbox environment variable is invalid.",
                ));
            }
            variables.insert(
                normalized_key,
                Zeroizing::new(value.encode_utf16().collect::<Vec<_>>()),
            );
        }

        let mut block = Vec::new();
        for (key, value) in variables {
            block.extend(std::ffi::OsStr::new(&key).encode_wide());
            block.push(b'=' as u16);
            block.extend(value.iter().copied());
            block.push(0);
        }
        block.push(0);
        if block.len() > 32_767 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox environment exceeds the Windows process limit.",
            ));
        }
        Ok(block)
    }

    /// Grants read and execute access to a runtime file tree without following reparse points.
    ///
    /// Hard-linked files are rejected because they could alias data outside the granted tree.
    ///
    /// # Errors
    /// Returns an error when the root is invalid, contains an unsafe file, exceeds the traversal
    /// bound, or Windows cannot inspect or update one of its paths.
    fn grant_read_only_path(&mut self, root: &Path) -> io::Result<()> {
        let root = fs::canonicalize(root)?;
        let root_metadata = fs::symlink_metadata(&root)?;
        if root_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox runtime path cannot be a reparse point.",
            ));
        }
        if !root_metadata.is_dir() && !root_metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox runtime path must be a file or directory.",
            ));
        }

        let access_root = if root_metadata.is_dir() {
            root.clone()
        } else {
            root.parent()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "A sandbox runtime file must have a parent directory.",
                    )
                })?
                .to_path_buf()
        };
        let mut ancestor = access_root.parent();
        while let Some(path) = ancestor {
            if path.parent().is_none() {
                break;
            }
            set_app_container_traverse_access(
                path,
                self.sid as windows_sys::Win32::Security::PSID,
                true,
            )?;
            self.granted_paths.push(path.to_path_buf());
            ancestor = path.parent();
        }

        let mut pending = vec![root];
        let mut scheduled_paths = 1;
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_ACL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The runtime path contains too many entries for a bounded AppContainer.",
                ));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }
            let is_directory = metadata.is_dir();
            set_app_container_read_access(
                &path,
                self.sid as windows_sys::Win32::Security::PSID,
                true,
                is_directory,
            )?;
            self.granted_paths.push(path.clone());
            if is_directory {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let child_path = entry.path();
                    let child_metadata = fs::symlink_metadata(&child_path)?;
                    if child_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
                        && !schedule_acl_path(&mut pending, &mut scheduled_paths, child_path)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "The runtime path contains too many entries for a bounded AppContainer.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Grants read, write, and execute access to the selected workspace tree.
    ///
    /// Reparse points are not traversed, and hard-linked files are rejected to keep access within
    /// the workspace boundary.
    ///
    /// # Errors
    /// Returns an error when traversal exceeds its bound, a hard-linked file is found, or a file
    /// system or Windows ACL operation fails.
    fn grant_working_directory(&mut self, root: PathBuf) -> io::Result<()> {
        let mut ancestor = root.parent();
        while let Some(path) = ancestor {
            if path.parent().is_none() {
                break;
            }
            set_app_container_traverse_access(
                path,
                self.sid as windows_sys::Win32::Security::PSID,
                true,
            )?;
            self.granted_paths.push(path.to_path_buf());
            ancestor = path.parent();
        }

        let mut pending = vec![root.clone()];
        let mut scheduled_paths = 1;
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_ACL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The working directory contains too many entries for a bounded AppContainer sandbox.",
                ));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }
            let is_directory = metadata.is_dir();
            set_app_container_access(
                &path,
                self.sid as windows_sys::Win32::Security::PSID,
                true,
                is_directory,
            )?;
            self.granted_paths.push(path.clone());
            if is_directory {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let child_path = entry.path();
                    let child_metadata = fs::symlink_metadata(&child_path)?;
                    if child_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        continue;
                    }
                    if !schedule_acl_path(&mut pending, &mut scheduled_paths, child_path) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "The working directory contains too many entries for a bounded AppContainer sandbox.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Revokes this profile's ACE from every path recorded during sandbox setup.
    fn cleanup(&mut self) {
        revoke_user_objects_access(self.sid as windows_sys::Win32::Security::PSID);
        for path in self.granted_paths.drain(..).rev() {
            if set_app_container_access(
                &path,
                self.sid as windows_sys::Win32::Security::PSID,
                false,
                false,
            )
            .is_err()
            {
                eprintln!("app_container_acl_cleanup_failed");
            }
        }
    }
}

/// Grants minimal window station and desktop access to the AppContainer SID.
fn grant_user_objects_access(sid: windows_sys::Win32::Security::PSID) -> io::Result<()> {
    // SAFETY: GetProcessWindowStation returns the current process window station handle.
    let winsta =
        unsafe { windows_sys::Win32::System::StationsAndDesktops::GetProcessWindowStation() };
    if !winsta.is_null() {
        let winsta_mask = windows_sys::Win32::Storage::FileSystem::STANDARD_RIGHTS_READ
            | 0x0001 /* WINSTA_ENUMDESKTOPS */
            | 0x0002 /* WINSTA_READATTRIBUTES */
            | 0x0004 /* WINSTA_ACCESSCLIPBOARD */
            | 0x0020 /* WINSTA_ACCESSGLOBALATOMS */
            | 0x0100 /* WINSTA_ENUMERATE */;
        update_user_object_acl(winsta as _, sid, true, winsta_mask)?;
    }
    // SAFETY: GetCurrentThreadId retrieves the active thread ID for the desktop query.
    let thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
    // SAFETY: GetThreadDesktop returns the desktop assigned to the calling thread.
    let desk =
        unsafe { windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop(thread_id) };
    if !desk.is_null() {
        let desk_mask = windows_sys::Win32::Storage::FileSystem::STANDARD_RIGHTS_READ
            | windows_sys::Win32::Storage::FileSystem::STANDARD_RIGHTS_WRITE
            | 0x0001 /* DESKTOP_READOBJECTS */
            | 0x0002 /* DESKTOP_CREATEWINDOW */
            | 0x0004 /* DESKTOP_CREATEMENU */
            | 0x0020 /* DESKTOP_HOOKCONTROL */
            | 0x0040 /* DESKTOP_ENUMERATE */
            | 0x0080 /* DESKTOP_WRITEOBJECTS */;
        update_user_object_acl(desk as _, sid, true, desk_mask)?;
    }
    Ok(())
}

/// Revokes window station and desktop access from the AppContainer SID.
fn revoke_user_objects_access(sid: windows_sys::Win32::Security::PSID) {
    // SAFETY: Querying the current window station and desktop to revoke the AppContainer SID ACE.
    let winsta =
        unsafe { windows_sys::Win32::System::StationsAndDesktops::GetProcessWindowStation() };
    if !winsta.is_null() {
        let _ = update_user_object_acl(winsta as _, sid, false, 0);
    }
    // SAFETY: GetCurrentThreadId retrieves the active thread ID for the desktop query.
    let thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
    // SAFETY: GetThreadDesktop returns the desktop assigned to the calling thread.
    let desk =
        unsafe { windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop(thread_id) };
    if !desk.is_null() {
        let _ = update_user_object_acl(desk as _, sid, false, 0);
    }
}

/// Updates the discretionary ACL on a window station or desktop object for the given SID.
fn update_user_object_acl(
    handle: windows_sys::Win32::Foundation::HANDLE,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    access_mask: u32,
) -> io::Result<()> {
    if handle.is_null() {
        return Ok(());
    }
    let _guard = APP_CONTAINER_ACL_LOCK
        .lock()
        .map_err(|_| io::Error::other("The AppContainer ACL lock is unavailable."))?;
    let mut p_dacl: *mut windows_sys::Win32::Security::ACL = null_mut();
    let mut p_sd: windows_sys::Win32::Security::PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: GetSecurityInfo queries the security descriptor of the specified window object.
    let status = unsafe {
        windows_sys::Win32::Security::Authorization::GetSecurityInfo(
            handle,
            windows_sys::Win32::Security::Authorization::SE_WINDOW_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut p_dacl,
            null_mut(),
            &mut p_sd,
        )
    };
    if status != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    struct LocalFreeGuard(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);
    impl Drop for LocalFreeGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: Pointer was allocated by a Windows security API.
                unsafe { windows_sys::Win32::Foundation::LocalFree(self.0.cast()) };
            }
        }
    }
    let _sd_guard = LocalFreeGuard(p_sd);

    let mode = if allow {
        windows_sys::Win32::Security::Authorization::GRANT_ACCESS
    } else {
        windows_sys::Win32::Security::Authorization::REVOKE_ACCESS
    };

    let explicit_access = windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W {
        grfAccessPermissions: if allow { access_mask } else { 0 },
        grfAccessMode: mode,
        grfInheritance: windows_sys::Win32::Security::NO_INHERITANCE,
        Trustee: windows_sys::Win32::Security::Authorization::TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID,
            TrusteeType: windows_sys::Win32::Security::Authorization::TRUSTEE_IS_USER,
            ptstrName: sid as *mut u16,
        },
    };
    let mut new_dacl: *mut windows_sys::Win32::Security::ACL = null_mut();
    // SAFETY: SetEntriesInAclW modifies the DACL by adding or revoking the trustee entry.
    let status = unsafe {
        windows_sys::Win32::Security::Authorization::SetEntriesInAclW(
            1,
            &explicit_access,
            p_dacl,
            &mut new_dacl,
        )
    };
    if status != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _dacl_guard = LocalFreeGuard(new_dacl as _);

    // SAFETY: SetSecurityInfo applies the modified DACL to the window object handle.
    let status = unsafe {
        windows_sys::Win32::Security::Authorization::SetSecurityInfo(
            handle,
            windows_sys::Win32::Security::Authorization::SE_WINDOW_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_dacl,
            null_mut(),
        )
    };
    if status != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

/// Adds a discovered path only while the AppContainer tree remains within its entry limit.
///
/// `scheduled_paths` includes the root and paths already processed, so the queue never needs to
/// retain more than `MAX_ACL_ENTRIES` paths for one traversal.
fn schedule_acl_path(
    pending: &mut Vec<PathBuf>,
    scheduled_paths: &mut usize,
    path: PathBuf,
) -> bool {
    if *scheduled_paths >= MAX_ACL_ENTRIES {
        return false;
    }

    pending.push(path);
    *scheduled_paths += 1;
    true
}

impl Drop for AppContainerSandbox {
    /// Revokes granted ACLs, then releases the profile SID and deletes the profile.
    fn drop(&mut self) {
        self.cleanup();
        // SAFETY: These pointers were allocated by the paired Windows APIs and are still owned by this sandbox.
        unsafe {
            FreeSid(self.sid as windows_sys::Win32::Security::PSID);
            windows_sys::Win32::Foundation::LocalFree(self.sid_string as _);
            if DeleteAppContainerProfile(self.name.as_ptr()) < 0 {
                eprintln!("app_container_profile_cleanup_failed");
            }
        }
    }
}

/// Adds or removes the full workspace access mask for one AppContainer SID.
fn set_app_container_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    is_directory: bool,
) -> io::Result<()> {
    let permissions = FILE_GENERIC_READ
        | FILE_GENERIC_WRITE
        | FILE_GENERIC_EXECUTE
        | FILE_TRAVERSE
        | FILE_ADD_FILE
        | FILE_ADD_SUBDIRECTORY
        | FILE_DELETE_CHILD
        | windows_sys::Win32::Storage::FileSystem::DELETE;
    set_app_container_permissions(path, sid, allow, permissions, is_directory)
}

/// Adds or removes the read-only runtime access mask for one AppContainer SID.
fn set_app_container_read_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    is_directory: bool,
) -> io::Result<()> {
    let permissions =
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE | FILE_TRAVERSE | FILE_READ_ATTRIBUTES;
    set_app_container_permissions(path, sid, allow, permissions, is_directory)
}

/// Applies a supplied access mask and enables inheritance only for granted directories.
fn set_app_container_permissions(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    permissions: u32,
    is_directory: bool,
) -> io::Result<()> {
    let inheritance = if allow && is_directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    update_app_container_acl(path, sid, allow, permissions, inheritance)
}

/// Adds or removes traversal and attribute-read access without inheriting it to children.
fn set_app_container_traverse_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
) -> io::Result<()> {
    update_app_container_acl(path, sid, allow, FILE_TRAVERSE | FILE_READ_ATTRIBUTES, 0)
}

/// Performs a serialized read-modify-write update of one canonical target's discretionary ACL.
///
/// # Errors
/// Returns an error when the target changes, is a reparse point, already has multiple hard links
/// during a grant, the ACL is absent, the cross-process lock cannot be acquired, or any Windows
/// security API rejects the update.
fn update_app_container_acl(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    permissions: u32,
    inheritance: u32,
) -> io::Result<()> {
    // Keep the ACL read-modify-write sequence atomic across threads and OpenChat processes.
    let _guard = APP_CONTAINER_ACL_LOCK
        .lock()
        .map_err(|_| io::Error::other("The AppContainer ACL lock is unavailable."))?;
    let _system_guard = AppContainerAclLock::acquire()?;
    let use_maximum_allowed_for_target = allow && inheritance != 0;
    let (path_handles, information) = lock_acl_path(path, use_maximum_allowed_for_target)?;
    let file = path_handles
        .last()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "The ACL path is empty."))?;
    let is_directory = information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if allow && !is_directory && information.nNumberOfLinks > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A sandbox path contains a hard-linked file.",
        ));
    }
    // File and directory ACLs are updated through their inspected handles to avoid resolving a
    // potentially redirected path again. MAXIMUM_ALLOWED is used when an inheritable grant must
    // avoid changing existing children.
    let mut current_dacl = null_mut();
    let mut source_descriptor = null_mut();
    // SAFETY: `file` owns the live target handle and every requested output pointer is writable.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle() as HANDLE,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut current_dacl,
            null_mut(),
            &mut source_descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if current_dacl.is_null() {
        // SAFETY: The security descriptor pointer was returned by GetSecurityInfo; LocalFree accepts null.
        unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "A null or absent DACL cannot be safely scoped to an AppContainer.",
        ));
    }
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: permissions,
        grfAccessMode: if allow { SET_ACCESS } else { REVOKE_ACCESS },
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    };
    let mut updated_dacl = null_mut();
    // SAFETY: `current_dacl` came from the successful query, `access` is initialized, and the output pointer is writable.
    let acl_status = unsafe { SetEntriesInAclW(1, &access, current_dacl, &mut updated_dacl) };
    if acl_status != 0 {
        // SAFETY: The descriptor was allocated by GetSecurityInfo and has not yet been freed.
        unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
        return Err(io::Error::from_raw_os_error(acl_status as i32));
    }
    // SAFETY: The descriptor was allocated by GetSecurityInfo and has not yet been freed.
    unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
    let updated_dacl = require_non_null_acl(updated_dacl)?;
    let mut descriptor_storage =
        std::mem::MaybeUninit::<windows_sys::Win32::Security::SECURITY_DESCRIPTOR>::uninit();
    let descriptor = descriptor_storage.as_mut_ptr();
    // SAFETY: `descriptor` points to writable storage large enough for a SECURITY_DESCRIPTOR.
    let initialized =
        unsafe { windows_sys::Win32::Security::InitializeSecurityDescriptor(descriptor.cast(), 1) };
    let dacl_attached = if initialized != 0 {
        // SAFETY: Initialization succeeded and `updated_dacl` is a valid API-produced ACL.
        (unsafe {
            windows_sys::Win32::Security::SetSecurityDescriptorDacl(
                descriptor.cast(),
                1,
                updated_dacl.as_ptr(),
                0,
            )
        }) != 0
    } else {
        false
    };
    if initialized == 0 || !dacl_attached {
        let error = last_os_error();
        // SAFETY: `updated_dacl` was allocated by SetEntriesInAclW and is still owned here.
        unsafe { windows_sys::Win32::Foundation::LocalFree(updated_dacl.as_ptr().cast()) };
        return Err(error);
    }
    let set_result = if !is_directory || use_maximum_allowed_for_target {
        // SAFETY: `file` owns the live target handle and `updated_dacl` is a valid API-produced ACL.
        // MAXIMUM_ALLOWED suppresses propagation to existing children.
        let status = unsafe {
            SetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                updated_dacl.as_ptr(),
                null_mut(),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    } else {
        let path_wide = wide(path.as_os_str());
        // Open path handles disallow delete sharing, which pins name resolution through the write.
        // SetFileSecurityW preserves the existing nonrecursive ACL update behavior.
        // SAFETY: `path_wide` is NUL-terminated and `descriptor` references the live ACL.
        let success = unsafe {
            windows_sys::Win32::Security::SetFileSecurityW(
                path_wide.as_ptr(),
                DACL_SECURITY_INFORMATION,
                descriptor.cast(),
            )
        };
        if success != 0 {
            Ok(())
        } else {
            Err(last_os_error())
        }
    };
    // SAFETY: `updated_dacl` was allocated by SetEntriesInAclW and has no remaining users.
    unsafe { windows_sys::Win32::Foundation::LocalFree(updated_dacl.as_ptr().cast()) };
    set_result
}

/// Converts a successful ACL API result into a non-null pointer before applying it.
///
/// A null pointer supplied to `SetSecurityDescriptorDacl` represents a null DACL and grants
/// unrestricted access, so the sandbox must reject it instead of treating it as an empty ACL.
fn require_non_null_acl(acl: *mut ACL) -> io::Result<NonNull<ACL>> {
    NonNull::new(acl).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned a null ACL after a successful ACL update.",
        )
    })
}

/// Holds the stable per-profile file lock while an ACL update is in progress.
#[must_use = "the file lock must remain held until the ACL update completes"]
struct AppContainerAclLock {
    _lock_file: fs::File,
}

impl AppContainerAclLock {
    /// Acquires the per-profile file lock within the configured timeout.
    ///
    /// # Errors
    /// Returns an error if the profile path is unavailable, the lock file cannot be opened, or
    /// another process keeps the lock for the full timeout.
    fn acquire() -> io::Result<Self> {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "The current Windows user's local application-data path is unavailable.",
                )
            })?;
        if !local_app_data.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The current Windows user's local application-data path is not absolute.",
            ));
        }

        Self::acquire_for_path(&app_container_acl_lock_path(&local_app_data))
    }

    /// Acquires a file lock at the supplied path; tests pass a temporary workspace path.
    fn acquire_for_path(lock_path: &Path) -> io::Result<Self> {
        use std::fs::TryLockError;

        let parent = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The AppContainer ACL lock path has no parent directory.",
                )
            })?;
        fs::create_dir_all(parent)?;
        let lock_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        let deadline = Instant::now() + ACL_LOCK_TIMEOUT;

        loop {
            match lock_file.try_lock() {
                Ok(()) => {
                    return Ok(Self {
                        _lock_file: lock_file,
                    });
                }
                Err(TryLockError::WouldBlock) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Timed out while acquiring the AppContainer ACL lock.",
                        ));
                    }
                    std::thread::sleep(remaining.min(ACL_LOCK_RETRY_INTERVAL));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
    }
}

/// Opens and pins each component of one canonical ACL path without following reparse points.
///
/// Keeping the chain open without delete sharing stabilizes name resolution until the ACL write
/// completes. Each opened handle is checked against its canonical component to reject redirection.
fn lock_acl_path(
    path: &Path,
    use_maximum_allowed_for_target: bool,
) -> io::Result<(Vec<fs::File>, BY_HANDLE_FILE_INFORMATION)> {
    let mut components: Vec<PathBuf> = path
        .ancestors()
        .filter(|component| !component.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    components.reverse();
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The ACL path is empty.",
        ));
    }

    let mut handles = Vec::with_capacity(components.len());
    let mut target_information = BY_HANDLE_FILE_INFORMATION::default();
    for (index, component) in components.iter().enumerate() {
        let is_target = index + 1 == components.len();
        let access_mode = if is_target && use_maximum_allowed_for_target {
            MAXIMUM_ALLOWED
        } else {
            FILE_READ_ATTRIBUTES
                | if is_target {
                    READ_CONTROL | WRITE_DAC
                } else {
                    0
                }
        };
        let mut options = fs::OpenOptions::new();
        options
            .access_mode(access_mode)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let file = options.open(component)?;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `file` owns a live file handle and `information` is a writable output structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) }
            == 0
        {
            return Err(last_os_error());
        }
        if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox path cannot contain a reparse point.",
            ));
        }
        if !is_target && information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox path ancestor is not a directory.",
            ));
        }

        let mut final_path = vec![0u16; 32_768];
        // SAFETY: `file` is live and `final_path` is writable for its declared capacity.
        let final_path_length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as HANDLE,
                final_path.as_mut_ptr(),
                final_path.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if final_path_length == 0 {
            return Err(last_os_error());
        }
        if final_path_length as usize >= final_path.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox path exceeds the Windows path limit.",
            ));
        }
        final_path.truncate(final_path_length as usize);
        if !component
            .as_os_str()
            .encode_wide()
            .eq(final_path.iter().copied())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox path changed while its ACL was being prepared.",
            ));
        }

        target_information = information;
        handles.push(file);
    }

    Ok((handles, target_information))
}

/// Creates an owned anonymous pipe pair with the supplied inheritance attributes.
fn create_pipe(attributes: &SECURITY_ATTRIBUTES) -> io::Result<(OwnedHandle, OwnedHandle)> {
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

/// Creates a job object that terminates its assigned process tree when its last handle closes.
///
/// Optional positive process and memory limits are applied before the job is returned.
fn create_kill_on_close_job(limits: Option<SandboxJobLimits>) -> io::Result<OwnedHandle> {
    use windows_sys::Win32::System::JobObjects::{
        JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    };

    // SAFETY: Null attributes and name request a new unnamed job object; Windows returns the handle.
    let handle = unsafe { CreateJobObjectW(null(), null()) };
    let job = owned_handle(handle)?;
    let Some(limits) = limits.filter(|limits| {
        limits.active_process_limit > 0
            && limits.process_memory_limit_bytes > 0
            && limits.job_memory_limit_bytes > 0
    }) else {
        if limits.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox job limits must be positive.",
            ));
        }
        return configure_kill_on_close_job(job);
    };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_JOB_MEMORY,
            ActiveProcessLimit: limits.active_process_limit,
            ..Default::default()
        },
        ProcessMemoryLimit: limits.process_memory_limit_bytes,
        JobMemoryLimit: limits.job_memory_limit_bytes,
        ..Default::default()
    };
    // SAFETY: `job` is live and `limits` is initialized and sized for the selected information class.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(job)
}

/// Applies only the kill-on-close policy to an existing job object.
fn configure_kill_on_close_job(job: OwnedHandle) -> io::Result<OwnedHandle> {
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `job` is live and `limits` is initialized and sized for the selected information class.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(job)
}

/// Returns the number of processes still associated with a Windows job object.
fn job_active_process_count(job: HANDLE) -> io::Result<u32> {
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: `job` is a live job handle and `accounting` is writable for the requested class.
    if unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            null_mut(),
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(accounting.ActiveProcesses)
}

/// Owns SID arrays allocated while resolving the optional `internetClient` capability.
struct DerivedCapabilitySids {
    group_sids: *mut PSID,
    group_sid_count: u32,
    capability_sids: *mut PSID,
    capability_sid_count: u32,
}

impl DerivedCapabilitySids {
    /// Resolves the single SID required to grant the AppContainer internet capability.
    ///
    /// # Errors
    /// Returns an error if Windows cannot derive the SIDs or returns an unexpected capability set.
    fn internet_client() -> io::Result<Self> {
        let name = wide_null("internetClient");
        let mut sids = Self {
            group_sids: null_mut(),
            group_sid_count: 0,
            capability_sids: null_mut(),
            capability_sid_count: 0,
        };
        // SAFETY: `name` is NUL-terminated and the SID arrays/counts are writable output locations.
        let derived = unsafe {
            DeriveCapabilitySidsFromName(
                name.as_ptr(),
                &mut sids.group_sids,
                &mut sids.group_sid_count,
                &mut sids.capability_sids,
                &mut sids.capability_sid_count,
            )
        };
        if derived == 0 {
            return Err(last_os_error());
        }
        if sids.capability_sid_count != 1 || sids.capability_sids.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned an invalid AppContainer capability SID set.",
            ));
        }
        Ok(sids)
    }

    /// Returns the validated sole capability SID for `SECURITY_CAPABILITIES`.
    fn capability_sid(&self) -> PSID {
        // SAFETY: `internet_client` validates exactly one non-null capability SID before this accessor is used.
        unsafe { *self.capability_sids }
    }
}

impl Drop for DerivedCapabilitySids {
    /// Releases the API-allocated SID entries and their containing arrays.
    fn drop(&mut self) {
        // SAFETY: Both arrays and counts were returned by DeriveCapabilitySidsFromName and are freed once here.
        unsafe {
            free_sid_array(self.group_sids, self.group_sid_count);
            free_sid_array(self.capability_sids, self.capability_sid_count);
        }
    }
}

/// Releases the SID pointers returned by `DeriveCapabilitySidsFromName`.
///
/// # Safety
/// `sids` and `count` must be the matching pointer and count returned by that API, and the array must not have been freed.
unsafe fn free_sid_array(sids: *mut PSID, count: u32) {
    if sids.is_null() {
        return;
    }
    for index in 0..count as usize {
        // SAFETY: The caller guarantees this API-owned array has `count` initialized SID pointers.
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(*sids.add(index) as _);
        }
    }
    // SAFETY: The array itself was allocated by the Windows API and has not been freed yet.
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(sids as _);
    }
}

/// Accepts bounded uppercase Windows environment names used by the sandbox base profile.
fn is_valid_base_environment_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && value.len() <= 128
}

/// Rejects base and temporary variables from caller-supplied additional environment values.
fn is_valid_additional_environment_name(value: &str) -> bool {
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
fn owned_handle(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle as isize == -1 {
        return Err(last_os_error());
    }
    // SAFETY: The successful Win32 call transferred ownership of this non-null, non-invalid handle here.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
}

/// Encodes an operating-system string as NUL-terminated UTF-16 for Win32 calls.
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// Encodes a Rust string as NUL-terminated UTF-16 for Win32 calls.
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Rejects embedded NULs before a value is passed to a NUL-terminated Win32 API.
fn validate_win32_string(value: &OsStr) -> io::Result<()> {
    if value.encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Process command-line values must not contain NUL characters.",
        ));
    }
    Ok(())
}

/// Builds a mutable Win32 command line with each executable and argument quoted separately.
fn wide_command_line(application: &Path, arguments: &[std::ffi::OsString]) -> Vec<u16> {
    let mut command_line = quote_windows_argument(application.as_os_str());
    for argument in arguments {
        command_line.push(' ');
        command_line.push_str(&quote_windows_argument(argument));
    }
    wide_null(&command_line)
}

/// Escapes quotes and backslashes using the Windows command-line parsing convention.
fn quote_windows_argument(argument: &std::ffi::OsStr) -> String {
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

/// Removes extended-length prefixes that PowerShell cannot use for its working directory.
fn shell_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(unc_path) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc_path}"));
    }
    if let Some(local_path) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(local_path);
    }
    path.to_path_buf()
}

/// Selects PowerShell and constructs arguments for a sandboxed shell command.
fn shell_invocation(command: &str, working_directory: &Path) -> (PathBuf, Vec<std::ffi::OsString>) {
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
fn powershell_command(command: &str, working_directory: &Path) -> String {
    let working_directory = working_directory.to_string_lossy().replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'Stop'; [System.Environment]::CurrentDirectory = '{working_directory}'; Set-Location -LiteralPath '{working_directory}'; {command}"
    )
}

/// Captures the calling thread's last Win32 error immediately after a failing API call.
fn last_os_error() -> io::Error {
    // SAFETY: GetLastError is thread-local and may be read immediately after a failing Win32 call.
    io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
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

/// Converts a Windows-owned NUL-terminated UTF-16 path to an `OsString`.
///
/// # Safety
/// `value` must be non-null and point to a readable NUL-terminated UTF-16 string.
unsafe fn wide_os_string(value: windows_sys::core::PWSTR) -> std::ffi::OsString {
    let mut length = 0usize;
    // SAFETY: The caller guarantees that `value` points to a readable NUL-terminated string.
    while unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: The scan found the terminator, so `length` code units are readable before it.
    std::ffi::OsString::from_wide(unsafe { std::slice::from_raw_parts(value, length) })
}

#[cfg(test)]
mod tests {
    use super::{
        AppContainerAclLock, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        MAX_ACL_ENTRIES, OBJECT_INHERIT_ACE, READ_CONTROL, SE_FILE_OBJECT, SandboxJobLimits,
        SandboxOptions, SandboxedProcess, WAIT_INTERVAL, Zeroizing, app_container_acl_lock_path,
        create_kill_on_close_job, job_active_process_count, owned_handle, require_non_null_acl,
        schedule_acl_path, shell_invocation, update_app_container_acl,
    };
    use std::{
        ffi::OsString,
        fs,
        io::ErrorKind,
        mem::size_of,
        os::windows::ffi::OsStringExt,
        os::windows::fs::OpenOptionsExt,
        os::windows::io::AsRawHandle,
        path::{Path, PathBuf},
        ptr::null_mut,
        time::Duration,
    };
    use tokio::io::AsyncReadExt;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Security::ACL;
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;

    /// Ensures a successful ACL update cannot be applied as a null, unrestricted DACL.
    #[test]
    fn null_acl_update_results_are_rejected() {
        assert!(matches!(
            require_non_null_acl(null_mut()),
            Err(error) if error.kind() == ErrorKind::InvalidData
        ));
    }

    /// Rejects an embedded NUL before sandbox resources are created for a shell command.
    #[test]
    fn shell_command_rejects_embedded_nul() {
        let error = SandboxedProcess::spawn_with_options(
            "Write-Output safe\0; Write-Output truncated",
            Path::new("missing-workspace"),
            SandboxOptions::default(),
        )
        .err()
        .expect("an embedded NUL must be rejected before process creation");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    /// Rejects an embedded NUL before resolving a configured executable path.
    #[test]
    fn configured_program_rejects_embedded_nul_argument() {
        let argument_wide: Vec<u16> = "safe\0truncated".encode_utf16().collect();
        let arguments = [OsString::from_wide(&argument_wide)];
        let error = SandboxedProcess::spawn_program_with_options(
            Path::new("missing-program"),
            &arguments,
            Path::new("missing-workspace"),
            &[],
            SandboxOptions::default(),
        )
        .err()
        .expect("an embedded NUL must be rejected before process creation");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    /// Verifies that an over-limit path is rejected before it is retained in the queue.
    #[test]
    fn acl_tree_limit_rejects_paths_before_enqueueing_them() {
        let mut pending = Vec::new();
        let mut scheduled_paths = MAX_ACL_ENTRIES - 1;

        assert!(schedule_acl_path(
            &mut pending,
            &mut scheduled_paths,
            PathBuf::from("last-allowed"),
        ));
        assert_eq!(scheduled_paths, MAX_ACL_ENTRIES);
        assert!(!schedule_acl_path(
            &mut pending,
            &mut scheduled_paths,
            PathBuf::from("over-limit"),
        ));
        assert_eq!(pending, [PathBuf::from("last-allowed")]);
    }

    #[test]
    fn host_memory_limits_are_bounded_and_reject_unavailable_memory() {
        let limits = SandboxJobLimits::for_physical_memory(4 * 1024 * 1024 * 1024)
            .expect("derive limits for a 4 GiB host");
        assert_eq!(limits.active_process_limit, 128);
        assert_eq!(limits.job_memory_limit_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(
            limits.process_memory_limit_bytes,
            limits.job_memory_limit_bytes
        );

        let limits = SandboxJobLimits::for_physical_memory(64 * 1024 * 1024 * 1024)
            .expect("derive limits for a high-memory host");
        assert_eq!(limits.job_memory_limit_bytes, 8 * 1024 * 1024 * 1024);
        assert_eq!(limits.process_memory_limit_bytes, 4 * 1024 * 1024 * 1024);

        assert!(SandboxJobLimits::for_physical_memory(0).is_err());
    }

    #[test]
    fn job_object_stores_requested_process_and_memory_limits() {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectExtendedLimitInformation, QueryInformationJobObject,
        };

        let expected = SandboxJobLimits {
            active_process_limit: 3,
            process_memory_limit_bytes: 4 * 1024 * 1024,
            job_memory_limit_bytes: 16 * 1024 * 1024,
        };
        let job = create_kill_on_close_job(Some(expected)).expect("create limited job object");
        let mut actual = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: The job and output structure are live, and the structure size matches the class.
        let query_succeeded = unsafe {
            QueryInformationJobObject(
                job.as_raw_handle() as HANDLE,
                JobObjectExtendedLimitInformation,
                (&mut actual as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                null_mut(),
            )
        };

        assert_ne!(query_succeeded, 0);
        assert_eq!(actual.BasicLimitInformation.ActiveProcessLimit, 3);
        assert_eq!(
            actual.ProcessMemoryLimit,
            expected.process_memory_limit_bytes
        );
        assert_eq!(actual.JobMemoryLimit, expected.job_memory_limit_bytes);
    }

    #[tokio::test]
    async fn failed_job_termination_is_not_success_while_a_child_remains() {
        use windows_sys::Win32::{
            Foundation::DuplicateHandle,
            System::{SystemServices::JOB_OBJECT_QUERY, Threading::GetCurrentProcess},
        };

        let workspace = TestWorkspace::new();
        let command = r#"$child = Start-Process -FilePath "$env:SystemRoot\System32\cmd.exe" -ArgumentList '/c timeout /t 15 /nobreak >nul' -PassThru -ErrorAction Stop; exit 0"#;
        let mut process = SandboxedProcess::spawn(command, workspace.path())
            .expect("start a command that leaves a child in the job");
        tokio::time::timeout(Duration::from_secs(5), async {
            while process
                .try_wait()
                .expect("query primary process status")
                .is_none()
            {
                tokio::time::sleep(WAIT_INTERVAL).await;
            }
        })
        .await
        .expect("the primary process should exit while its child is still running");
        assert!(
            job_active_process_count(process.job.as_raw_handle() as HANDLE)
                .expect("query active job processes")
                > 0
        );

        // SAFETY: GetCurrentProcess returns a valid pseudo handle for this test process.
        let current_process = unsafe { GetCurrentProcess() };
        let mut restricted_job = null_mut();
        // SAFETY: Both process handles are valid, the source job is owned, and the output is writable.
        let duplicated = unsafe {
            DuplicateHandle(
                current_process,
                process.job.as_raw_handle() as HANDLE,
                current_process,
                &mut restricted_job,
                JOB_OBJECT_QUERY,
                0,
                0,
            )
        };
        assert_ne!(duplicated, 0, "duplicate a query-only job handle");
        let restricted_job = owned_handle(restricted_job).expect("own the duplicated job handle");
        let full_access_job = std::mem::replace(&mut process.job, restricted_job);
        let termination = process.start_kill();
        process.job = full_access_job;
        let error = termination.expect_err("a live child must not be reported as terminated");
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);

        process
            .start_kill()
            .expect("terminate the remaining child with the full job handle");
        process
            .wait_for_job_exit()
            .expect("wait for the child process tree to exit");
    }

    #[tokio::test]
    async fn process_limit_blocks_child_processes_inside_the_job() {
        let workspace = TestWorkspace::new();
        let command = r#"$child = Start-Process -FilePath "$env:SystemRoot\System32\cmd.exe" -ArgumentList '/c exit 0' -PassThru -ErrorAction Stop; $child.WaitForExit(); exit 0"#;
        let (program, arguments) = shell_invocation(command, workspace.path());
        let baseline = SandboxedProcess::spawn_program(&program, &arguments, workspace.path())
            .expect("start child-process baseline in AppContainer");
        let (exit_code, stdout, stderr) = capture(baseline).await;
        assert_eq!(exit_code, 0, "stdout: {stdout}; stderr: {stderr}");

        let command = format!("try {{ {command} }} catch {{ exit 17 }}");
        let (program, arguments) = shell_invocation(&command, workspace.path());
        let limits = SandboxJobLimits {
            active_process_limit: 1,
            process_memory_limit_bytes: 1024 * 1024 * 1024,
            job_memory_limit_bytes: 2 * 1024 * 1024 * 1024,
        };
        let process = SandboxedProcess::spawn_program_with_options(
            &program,
            &arguments,
            workspace.path(),
            &[],
            SandboxOptions {
                job_limits: Some(limits),
                ..SandboxOptions::default()
            },
        )
        .expect("start process with a one-process job limit");
        let (exit_code, stdout, stderr) = capture(process).await;

        assert_eq!(exit_code, 17, "stdout: {stdout}; stderr: {stderr}");
    }

    struct TestWorkspace(PathBuf);

    impl TestWorkspace {
        fn new() -> Self {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!(
                    "openchat-appcontainer-test-{}",
                    uuid::Uuid::new_v4().simple()
                ));
            fs::create_dir_all(&path).expect("create an isolated AppContainer test folder");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove the isolated AppContainer test folder");
        }
    }

    async fn capture(mut process: SandboxedProcess) -> (i32, String, String) {
        let mut stdout = process.take_stdout().expect("sandbox stdout handle");
        let mut stderr = process.take_stderr().expect("sandbox stderr handle");
        let status = tokio::time::timeout(Duration::from_secs(15), process.wait())
            .await
            .expect("sandbox test process exceeded its exit timeout")
            .expect("wait for sandbox process");
        let mut stdout_text = String::new();
        stdout
            .read_to_string(&mut stdout_text)
            .await
            .expect("read sandbox stdout");
        let mut stderr_text = String::new();
        stderr
            .read_to_string(&mut stderr_text)
            .await
            .expect("read sandbox stderr");
        drop(process);
        (status.code().unwrap_or(-1), stdout_text, stderr_text)
    }

    #[tokio::test]
    async fn command_can_write_inside_its_workspace() {
        let workspace = TestWorkspace::new();
        let output = SandboxedProcess::spawn(
            "Set-Content -Path 'result.txt' -Value 'workspace_write_ok'; Get-Content -Raw 'result.txt'",
            workspace.path(),
        )
        .expect("start command in AppContainer");
        let (exit_code, stdout, stderr) = capture(output).await;

        assert_eq!(exit_code, 0, "PowerShell error output: {stderr}");
        assert!(stdout.contains("workspace_write_ok"));
        assert_eq!(
            fs::read_to_string(workspace.path().join("result.txt")).expect("read workspace result"),
            "workspace_write_ok\r\n"
        );
    }

    #[tokio::test]
    async fn configured_program_receives_arguments_inside_the_appcontainer() {
        let workspace = TestWorkspace::new();
        let (program, arguments) =
            shell_invocation("Write-Output configured_program_ok", workspace.path());
        let process = SandboxedProcess::spawn_program(&program, &arguments, workspace.path())
            .expect("start configured program in AppContainer");
        let (exit_code, stdout, stderr) = capture(process).await;

        assert_eq!(exit_code, 0, "stdout: {stdout}; stderr: {stderr}");
        assert!(stdout.contains("configured_program_ok"), "{stdout}");
    }

    #[tokio::test]
    async fn configured_program_receives_only_explicit_additional_environment_values() {
        let workspace = TestWorkspace::new();
        let (program, arguments) =
            shell_invocation("Write-Output $env:OPENCHAT_TEST_SECRET", workspace.path());
        let environment = vec![(
            "OPENCHAT_TEST_SECRET".to_owned(),
            Zeroizing::new("temporary_test_value".to_owned()),
        )];
        let process = SandboxedProcess::spawn_program_with_environment(
            &program,
            &arguments,
            workspace.path(),
            &environment,
        )
        .expect("start configured program with an explicit environment value");
        let (exit_code, stdout, stderr) = capture(process).await;

        assert_eq!(exit_code, 0, "stdout: {stdout}; stderr: {stderr}");
        assert_eq!(stdout.trim(), "temporary_test_value");
    }

    /// Rejects NUL values that could introduce additional Win32 environment entries.
    #[tokio::test]
    async fn configured_program_rejects_nul_in_additional_environment_values() {
        let workspace = TestWorkspace::new();
        let (program, arguments) = shell_invocation("Write-Output unreachable", workspace.path());
        let environment = vec![(
            "OPENCHAT_TEST_VALUE".to_owned(),
            Zeroizing::new("safe\0PATH=C:\\attacker".to_owned()),
        )];

        let error = SandboxedProcess::spawn_program_with_environment(
            &program,
            &arguments,
            workspace.path(),
            &environment,
        )
        .err()
        .expect("embedded NUL must fail before the child starts");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn configured_runtime_paths_are_readable_but_not_writable() {
        let workspace = TestWorkspace::new();
        let work_directory = workspace.path().join("work");
        let runtime_directory = workspace.path().join("runtime");
        fs::create_dir_all(&work_directory).expect("create sandbox work directory");
        fs::create_dir_all(&runtime_directory).expect("create sandbox runtime directory");
        fs::write(runtime_directory.join("runtime.txt"), "runtime_read_ok")
            .expect("write test runtime file");
        let read_command = "try { Get-Content -Raw -LiteralPath '..\\runtime\\runtime.txt' -ErrorAction Stop } catch { exit 17 }";
        let (program, arguments) = shell_invocation(read_command, &work_directory);
        let read_only_paths = [runtime_directory.clone()];
        let options = SandboxOptions {
            additional_read_only_paths: &read_only_paths,
            ..SandboxOptions::default()
        };
        let process = SandboxedProcess::spawn_program_with_options(
            &program,
            &arguments,
            &work_directory,
            &[],
            options,
        )
        .expect("start process with a read-only runtime directory");
        let (exit_code, stdout, stderr) = capture(process).await;
        assert_eq!(exit_code, 0, "stdout: {stdout}; stderr: {stderr}");
        assert!(stdout.contains("runtime_read_ok"));

        let write_command = "try { Set-Content -LiteralPath '..\\runtime\\runtime.txt' -Value 'runtime_write_bad' -ErrorAction Stop; exit 0 } catch { exit 31 }";
        let (program, arguments) = shell_invocation(write_command, &work_directory);
        let options = SandboxOptions {
            additional_read_only_paths: &read_only_paths,
            ..SandboxOptions::default()
        };
        let process = SandboxedProcess::spawn_program_with_options(
            &program,
            &arguments,
            &work_directory,
            &[],
            options,
        )
        .expect("start process for the runtime write-denial check");
        let (exit_code, _, _) = capture(process).await;
        assert_eq!(exit_code, 31);
        assert_eq!(
            fs::read_to_string(runtime_directory.join("runtime.txt"))
                .expect("read unchanged test runtime file"),
            "runtime_read_ok"
        );
    }

    #[tokio::test]
    async fn command_cannot_read_outside_its_workspace() {
        let workspace = TestWorkspace::new();
        let allowed = workspace.path().join("allowed");
        fs::create_dir_all(&allowed).expect("create the allowed folder");
        fs::write(
            workspace.path().join("outside.txt"),
            "outside_secret_sentinel",
        )
        .expect("write the outside test file");
        let process = SandboxedProcess::spawn(
            "try { Get-Content -Raw -LiteralPath '..\\outside.txt' -ErrorAction Stop; exit 0 } catch { exit 17 }",
            &allowed,
        )
        .expect("start command in AppContainer");
        let (exit_code, stdout, _) = capture(process).await;

        assert_eq!(exit_code, 17);
        assert!(!stdout.contains("outside_secret_sentinel"));
    }

    #[tokio::test]
    async fn command_cannot_connect_to_local_network_listener() {
        let workspace = TestWorkspace::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind local listener for network boundary test");
        let port = listener
            .local_addr()
            .expect("read local listener address")
            .port();
        let command = format!(
            "$client = [System.Net.Sockets.TcpClient]::new(); try {{ $pending = $client.BeginConnect('127.0.0.1', {port}, $null, $null); if (-not $pending.AsyncWaitHandle.WaitOne(1000)) {{ exit 19 }}; $client.EndConnect($pending); exit 0 }} catch {{ exit 19 }} finally {{ $client.Dispose() }}"
        );
        let process = SandboxedProcess::spawn(&command, workspace.path())
            .expect("start command in AppContainer");
        let (exit_code, _, _) = capture(process).await;

        assert_eq!(exit_code, 19);
        drop(listener);
    }

    #[tokio::test]
    async fn internet_client_capability_cannot_connect_to_a_local_listener() {
        let workspace = TestWorkspace::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind local listener for network capability test");
        let port = listener
            .local_addr()
            .expect("read local listener address")
            .port();
        let command = format!(
            "$client = [System.Net.Sockets.TcpClient]::new(); try {{ $pending = $client.BeginConnect('127.0.0.1', {port}, $null, $null); if (-not $pending.AsyncWaitHandle.WaitOne(1000)) {{ exit 19 }}; $client.EndConnect($pending); exit 0 }} catch {{ exit 19 }} finally {{ $client.Dispose() }}"
        );
        let (program, arguments) = shell_invocation(&command, workspace.path());
        let options = SandboxOptions {
            internet_client: true,
            ..SandboxOptions::default()
        };
        let process = SandboxedProcess::spawn_program_with_options(
            &program,
            &arguments,
            workspace.path(),
            &[],
            options,
        )
        .expect("start process with outbound Internet capability");
        let (exit_code, _, _) = capture(process).await;

        assert_eq!(exit_code, 19);
        drop(listener);
    }

    #[test]
    fn acl_updates_share_a_file_lock_between_threads() {
        use std::{sync::mpsc, thread, time::Duration};

        let workspace = TestWorkspace::new();
        let lock_path = workspace
            .path()
            .join("OpenChat")
            .join("appcontainer-acl.lock");
        let lock =
            AppContainerAclLock::acquire_for_path(&lock_path).expect("acquire ACL file lock");
        let worker_lock_path = lock_path.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = AppContainerAclLock::acquire_for_path(&worker_lock_path);
            sender
                .send(result.is_ok())
                .expect("send ACL file-lock result");
            drop(result);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(lock);
        assert!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("worker should acquire ACL file lock after release")
        );
        worker.join().expect("join ACL file-lock worker");
    }

    /// Verifies that ACL lock files are scoped to the current Windows user profile.
    #[test]
    fn acl_lock_path_is_scoped_to_local_app_data() {
        let local_app_data = Path::new(r"C:\Users\ProfileOne\AppData\Local");

        assert_eq!(
            app_container_acl_lock_path(local_app_data),
            PathBuf::from(r"C:\Users\ProfileOne\AppData\Local\OpenChat\appcontainer-acl.lock")
        );
    }

    /// Verifies that updating a directory does not apply its inheritable ACE to existing children.
    #[test]
    fn acl_updates_do_not_propagate_to_existing_children() {
        let workspace = TestWorkspace::new();
        let directory = workspace.path().join("directory");
        let child = directory.join("existing.txt");
        fs::create_dir_all(&directory).expect("create ACL test directory");
        fs::write(&child, "existing child").expect("create existing ACL test child");
        let directory = fs::canonicalize(directory).expect("canonicalize ACL test directory");
        let child = fs::canonicalize(child).expect("canonicalize ACL test child");
        let original_ace_count = dacl_ace_count(&child).expect("read child DACL");

        let sid_text = super::wide_null("S-1-15-2-1");
        let mut sid = null_mut();
        // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
        let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
        assert_ne!(converted, 0);
        assert!(!sid.is_null());

        let update = update_app_container_acl(
            &directory,
            sid,
            true,
            FILE_GENERIC_READ,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
        );
        let updated_child_ace_count = dacl_ace_count(&child);
        let revoke = update_app_container_acl(&directory, sid, false, FILE_GENERIC_READ, 0);
        // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
        unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

        update.expect("update directory DACL");
        revoke.expect("revoke directory DACL");
        assert_eq!(
            updated_child_ace_count.expect("read updated child DACL"),
            original_ace_count,
            "existing descendants must be updated individually"
        );
    }

    /// Verifies that a non-inheriting directory grant and revoke stay on the target DACL.
    #[test]
    fn noninheriting_directory_acl_updates_are_nonrecursive() {
        let workspace = TestWorkspace::new();
        let directory = workspace.path().join("directory");
        let child = directory.join("existing.txt");
        fs::create_dir_all(&directory).expect("create ACL test directory");
        fs::write(&child, "existing child").expect("create existing ACL test child");
        let directory = fs::canonicalize(directory).expect("canonicalize ACL test directory");
        let child = fs::canonicalize(child).expect("canonicalize ACL test child");
        let original_directory_ace_count = dacl_ace_count(&directory).expect("read directory DACL");
        let original_child_ace_count = dacl_ace_count(&child).expect("read child DACL");

        let sid_text = super::wide_null("S-1-15-2-1");
        let mut sid = null_mut();
        // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
        let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
        assert_ne!(converted, 0);
        assert!(!sid.is_null());

        let update = update_app_container_acl(
            &directory,
            sid,
            true,
            FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
            0,
        );
        let updated_directory_ace_count = dacl_ace_count(&directory);
        let updated_child_ace_count = dacl_ace_count(&child);
        let revoke = update_app_container_acl(
            &directory,
            sid,
            false,
            FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
            0,
        );
        let revoked_directory_ace_count = dacl_ace_count(&directory);
        let revoked_child_ace_count = dacl_ace_count(&child);
        // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
        unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

        update.expect("update non-inheriting directory DACL");
        revoke.expect("revoke non-inheriting directory DACL");
        assert_eq!(
            updated_directory_ace_count.expect("read updated directory DACL"),
            original_directory_ace_count + 1,
            "the grant must update the directory DACL"
        );
        assert_eq!(
            updated_child_ace_count.expect("read updated child DACL"),
            original_child_ace_count,
            "a non-inheriting directory grant must not update existing children"
        );
        assert_eq!(
            revoked_directory_ace_count.expect("read revoked directory DACL"),
            original_directory_ace_count,
            "revoking the grant must restore the directory DACL"
        );
        assert_eq!(
            revoked_child_ace_count.expect("read revoked child DACL"),
            original_child_ace_count,
            "revoking a non-inheriting directory grant must not change existing children"
        );
    }

    #[test]
    fn inheritable_acl_update_works_while_directory_is_open_for_attributes() {
        let workspace = TestWorkspace::new();
        let directory = workspace.path().join("directory");
        fs::create_dir_all(&directory).expect("create ACL test directory");
        let directory = fs::canonicalize(directory).expect("canonicalize ACL test directory");
        let mut options = fs::OpenOptions::new();
        options
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        let _open_directory = options
            .open(&directory)
            .expect("hold a read-only directory handle");

        let sid_text = super::wide_null("S-1-15-2-1");
        let mut sid = null_mut();
        // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
        let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
        assert_ne!(converted, 0);
        assert!(!sid.is_null());

        let update = update_app_container_acl(
            &directory,
            sid,
            true,
            FILE_GENERIC_READ,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
        );
        // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
        unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

        update.expect("update a directory held open by another read handle");
    }

    #[test]
    fn file_acl_update_changes_only_the_target_file() {
        let workspace = TestWorkspace::new();
        let directory = workspace.path().join("directory");
        let target = directory.join("target.txt");
        let sibling = directory.join("sibling.txt");
        fs::create_dir_all(&directory).expect("create ACL test directory");
        fs::write(&target, "target").expect("create target file");
        fs::write(&sibling, "sibling").expect("create sibling file");
        let directory = fs::canonicalize(directory).expect("canonicalize ACL test directory");
        let target = fs::canonicalize(target).expect("canonicalize ACL target");
        let sibling = fs::canonicalize(sibling).expect("canonicalize ACL sibling");
        let original_directory_ace_count = dacl_ace_count(&directory).expect("read directory DACL");
        let original_target_ace_count = dacl_ace_count(&target).expect("read target DACL");
        let original_sibling_ace_count = dacl_ace_count(&sibling).expect("read sibling DACL");

        let sid_text = super::wide_null("S-1-15-2-1");
        let mut sid = null_mut();
        // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
        let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
        assert_ne!(converted, 0);
        assert!(!sid.is_null());

        let update = update_app_container_acl(&target, sid, true, FILE_GENERIC_READ, 0);
        let updated_target_ace_count = dacl_ace_count(&target);
        let updated_directory_ace_count = dacl_ace_count(&directory);
        let updated_sibling_ace_count = dacl_ace_count(&sibling);
        let revoke = update_app_container_acl(&target, sid, false, FILE_GENERIC_READ, 0);
        // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
        unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

        update.expect("grant target file access");
        revoke.expect("revoke target file access");
        assert_eq!(
            updated_target_ace_count.expect("read updated target DACL"),
            original_target_ace_count + 1,
            "the grant must update the target file DACL"
        );
        assert_eq!(
            updated_directory_ace_count.expect("read updated directory DACL"),
            original_directory_ace_count,
            "a file grant must not update its parent directory"
        );
        assert_eq!(
            updated_sibling_ace_count.expect("read updated sibling DACL"),
            original_sibling_ace_count,
            "a file grant must not update sibling files"
        );
        assert_eq!(
            dacl_ace_count(&target).expect("read revoked target DACL"),
            original_target_ace_count,
            "revoking the grant must restore the target DACL"
        );
    }

    #[tokio::test]
    async fn a_hard_link_in_the_workspace_is_rejected_before_access_is_granted() {
        let workspace = TestWorkspace::new();
        let allowed = workspace.path().join("allowed");
        let original = workspace.path().join("outside.txt");
        fs::create_dir_all(&allowed).expect("create the allowed folder");
        fs::write(&original, "shared_file").expect("write the outside test file");
        fs::hard_link(&original, allowed.join("linked.txt")).expect("create hard link");

        let error = match SandboxedProcess::spawn("echo blocked", &allowed) {
            Ok(process) => {
                drop(process);
                panic!("hard link must block sandbox startup");
            }
            Err(error) => error,
        };

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            fs::read_to_string(original).expect("read outside file"),
            "shared_file"
        );
    }

    /// Reads the number of ACEs in a file or directory DACL for inheritance regression checks.
    fn dacl_ace_count(path: &Path) -> std::io::Result<u16> {
        let mut options = fs::OpenOptions::new();
        options
            .access_mode(READ_CONTROL)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let file = options.open(path)?;
        let mut dacl = null_mut();
        let mut descriptor = null_mut();
        // SAFETY: `file` is live and each requested output pointer is writable.
        let status = unsafe {
            super::GetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status as i32));
        }
        let result = if dacl.is_null() {
            Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "The test child has no DACL.",
            ))
        } else {
            // SAFETY: A successful GetSecurityInfo call returned a readable DACL pointer.
            Ok(unsafe { (*dacl.cast::<ACL>()).AceCount })
        };
        // SAFETY: `descriptor` was allocated by GetSecurityInfo and has no remaining users.
        unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor) };
        result
    }
}
