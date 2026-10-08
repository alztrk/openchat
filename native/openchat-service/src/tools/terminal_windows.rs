use std::{
    collections::{BTreeMap, HashSet},
    ffi::c_void,
    fs, io,
    mem::size_of,
    os::windows::fs::MetadataExt,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::Mutex,
    time::Duration,
};

use tokio::{fs::File, time::sleep};
use windows_sys::Win32::{
    Foundation::{
        GetLastError, HANDLE, HANDLE_FLAG_INHERIT, STILL_ACTIVE, WAIT_FAILED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, REVOKE_ACCESS,
            SE_FILE_OBJECT, SET_ACCESS, SetEntriesInAclW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN,
            TRUSTEE_W,
        },
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, FreeSid, OBJECT_INHERIT_ACE,
        SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ,
        FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_TRAVERSE, GetFileInformationByHandle,
    },
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
        },
        Pipes::CreatePipe,
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

const MAX_ACL_ENTRIES: usize = 20_000;
const WAIT_INTERVAL: Duration = Duration::from_millis(25);
static APP_CONTAINER_ACL_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct SandboxedProcess {
    process: OwnedHandle,
    job: OwnedHandle,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
    _sandbox: AppContainerSandbox,
}

impl SandboxedProcess {
    pub(crate) fn spawn(command: &str, workdir: &Path) -> io::Result<Self> {
        let sandbox = AppContainerSandbox::create(workdir)?;
        let workdir = sandbox.working_directory.clone();
        let (application, arguments) = shell_invocation(command, &workdir);
        Self::spawn_in_sandbox(sandbox, application, arguments, workdir, &[])
    }

    #[cfg(test)]
    pub(crate) fn spawn_program(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
    ) -> io::Result<Self> {
        Self::spawn_program_with_environment(program, arguments, workdir, &[])
    }

    pub(crate) fn spawn_program_with_environment(
        program: &Path,
        arguments: &[std::ffi::OsString],
        workdir: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
    ) -> io::Result<Self> {
        let program = fs::canonicalize(program)?;
        if !program.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandboxed program is unavailable.",
            ));
        }
        let sandbox = AppContainerSandbox::create(workdir)?;
        let working_directory = sandbox.working_directory.clone();
        Self::spawn_in_sandbox(
            sandbox,
            program,
            arguments.to_vec(),
            working_directory,
            additional_environment,
        )
    }

    fn spawn_in_sandbox(
        sandbox: AppContainerSandbox,
        application: PathBuf,
        arguments: Vec<std::ffi::OsString>,
        workdir: PathBuf,
        additional_environment: &[(String, Zeroizing<String>)],
    ) -> io::Result<Self> {
        let security_attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: 1,
            lpSecurityDescriptor: null_mut(),
        };

        let (child_stdin_read, parent_stdin_write) = create_pipe(&security_attributes)?;
        let (parent_stdout_read, child_stdout_write) = create_pipe(&security_attributes)?;
        let (parent_stderr_read, child_stderr_write) = create_pipe(&security_attributes)?;
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
        let mut security_capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: sandbox.sid as windows_sys::Win32::Security::PSID,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };

        let mut attribute_size = 0usize;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut attribute_size);
        }
        if attribute_size == 0 {
            return Err(last_os_error());
        }
        let mut attribute_storage = vec![0usize; attribute_size.div_ceil(size_of::<usize>())];
        let attribute_list = attribute_storage.as_mut_ptr().cast::<c_void>();
        unsafe {
            if InitializeProcThreadAttributeList(attribute_list, 2, 0, &mut attribute_size) == 0 {
                return Err(last_os_error());
            }
        }
        let _attribute_list = AttributeList(attribute_list);
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
        let mut environment =
            Zeroizing::new(sandbox.environment_block(&workdir, additional_environment)?);
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
        let thread = owned_handle(process_information.hThread)?;
        let job = create_kill_on_close_job()?;
        unsafe {
            if AssignProcessToJobObject(
                job.as_raw_handle() as HANDLE,
                process.as_raw_handle() as HANDLE,
            ) == 0
            {
                windows_sys::Win32::System::Threading::TerminateProcess(
                    process.as_raw_handle() as HANDLE,
                    1,
                );
                return Err(last_os_error());
            }
            if ResumeThread(thread.as_raw_handle() as HANDLE) == u32::MAX {
                TerminateJobObject(job.as_raw_handle() as HANDLE, 1);
                return Err(last_os_error());
            }
        }
        drop(thread);
        drop(child_stdin_read);
        drop(child_stdout_write);
        drop(child_stderr_write);

        Ok(Self {
            process,
            job,
            stdin: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stdin_write.into_raw_handle())
            })),
            stdout: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stdout_read.into_raw_handle())
            })),
            stderr: Some(File::from_std(unsafe {
                std::fs::File::from_raw_handle(parent_stderr_read.into_raw_handle())
            })),
            _sandbox: sandbox,
        })
    }

    pub(crate) fn take_stdin(&mut self) -> Option<File> {
        self.stdin.take()
    }

    pub(crate) fn take_stdout(&mut self) -> Option<File> {
        self.stdout.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<File> {
        self.stderr.take()
    }

    pub(crate) fn try_wait(&self) -> io::Result<Option<std::process::ExitStatus>> {
        let wait_result = unsafe { WaitForSingleObject(self.process.as_raw_handle() as HANDLE, 0) };
        match wait_result {
            WAIT_OBJECT_0 => {
                let mut exit_code = STILL_ACTIVE as u32;
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

    pub(crate) fn start_kill(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) } == 0 {
            if self.try_wait()?.is_some() {
                return Ok(());
            }
            return Err(last_os_error());
        }
        Ok(())
    }

    pub(crate) async fn wait(&self) -> io::Result<std::process::ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            sleep(WAIT_INTERVAL).await;
        }
    }

    pub(crate) fn wait_for_job_exit(&self) -> io::Result<()> {
        match unsafe { WaitForSingleObject(self.job.as_raw_handle() as HANDLE, 5_000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The terminal process tree did not stop in time.",
            )),
            WAIT_FAILED => Err(last_os_error()),
            _ => Err(io::Error::other(
                "The terminal process tree returned an unknown wait result.",
            )),
        }
    }
}

impl Drop for SandboxedProcess {
    fn drop(&mut self) {
        let _ = unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) };
        let _ = self.wait_for_job_exit();
    }
}

struct AppContainerSandbox {
    name: Vec<u16>,
    sid: usize,
    sid_string: usize,
    working_directory: PathBuf,
    temp_directory: PathBuf,
    granted_paths: Vec<PathBuf>,
}

impl AppContainerSandbox {
    fn create(working_directory: &Path) -> io::Result<Self> {
        let working_directory = shell_path(&fs::canonicalize(working_directory)?);
        if !working_directory.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The command working directory is unavailable.",
            ));
        }
        let name = wide_null(&format!(
            "OpenChatTerminal{}",
            uuid::Uuid::new_v4().simple()
        ));
        let display_name = wide_null("OpenChat terminal sandbox");
        let description = wide_null("Temporary AppContainer for an OpenChat terminal session.");
        let mut sid = null_mut();
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
            unsafe { DeleteAppContainerProfile(name.as_ptr()) };
            return Err(io::Error::other(
                "Windows could not create the terminal AppContainer.",
            ));
        }

        let mut sid_string = null_mut();
        if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 || sid_string.is_null() {
            unsafe {
                FreeSid(sid);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(last_os_error());
        }
        let sid_string = sid_string as usize;
        let mut app_container_data = null_mut();
        let folder_result = unsafe {
            windows_sys::Win32::Security::Isolation::GetAppContainerFolderPath(
                sid_string as windows_sys::core::PCWSTR,
                &mut app_container_data,
            )
        };
        if folder_result < 0 || app_container_data.is_null() {
            unsafe {
                FreeSid(sid);
                windows_sys::Win32::Foundation::LocalFree(sid_string as _);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other(
                "Windows could not locate the terminal AppContainer folder.",
            ));
        }
        let package_root = PathBuf::from(unsafe { wide_os_string(app_container_data) });
        unsafe { CoTaskMemFree(app_container_data.cast()) };
        let temp_directory = package_root.join("OpenChatTerminalTemp");
        if let Err(error) = fs::create_dir_all(&temp_directory) {
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
        if let Err(error) = sandbox.grant_working_directory(working_directory) {
            sandbox.cleanup();
            return Err(error);
        }
        Ok(sandbox)
    }

    fn environment_block(
        &self,
        working_directory: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
    ) -> io::Result<Vec<u16>> {
        let mut variables = BTreeMap::new();
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
        let working_directory = shell_path(working_directory);
        let working_directory_text = working_directory.to_string_lossy();
        let mut drive_characters = working_directory_text.chars();
        if let (Some(drive), Some(':')) = (drive_characters.next(), drive_characters.next()) {
            if drive.is_ascii_alphabetic() {
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
            if !crate::credentials::is_valid_environment_name(key)
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
                    "The sandbox environment variable name is invalid.",
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
        Ok(block)
    }

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
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_ACL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The project contains too many entries for a bounded terminal sandbox.",
                ));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }
            if metadata.is_file() && has_multiple_links(&path)? {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The project contains a hard-linked file that cannot be safely sandboxed.",
                ));
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
                    pending.push(child_path);
                }
            }
        }
        Ok(())
    }

    fn cleanup(&mut self) {
        for path in self.granted_paths.drain(..).rev() {
            if set_app_container_access(
                &path,
                self.sid as windows_sys::Win32::Security::PSID,
                false,
                false,
            )
            .is_err()
            {
                eprintln!("terminal_sandbox_acl_cleanup_failed");
            }
        }
    }
}

impl Drop for AppContainerSandbox {
    fn drop(&mut self) {
        self.cleanup();
        unsafe {
            FreeSid(self.sid as windows_sys::Win32::Security::PSID);
            windows_sys::Win32::Foundation::LocalFree(self.sid_string as _);
            if DeleteAppContainerProfile(self.name.as_ptr()) < 0 {
                eprintln!("terminal_sandbox_profile_cleanup_failed");
            }
        }
    }
}

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
    let inheritance = if allow && is_directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    update_app_container_acl(path, sid, allow, permissions, inheritance)
}

fn set_app_container_traverse_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
) -> io::Result<()> {
    update_app_container_acl(path, sid, allow, FILE_TRAVERSE | FILE_READ_ATTRIBUTES, 0)
}

fn update_app_container_acl(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    allow: bool,
    permissions: u32,
    inheritance: u32,
) -> io::Result<()> {
    // Keep the DACL read-modify-write sequence atomic across concurrent terminal sessions.
    let _guard = APP_CONTAINER_ACL_LOCK
        .lock()
        .map_err(|_| io::Error::other("The AppContainer ACL lock is unavailable."))?;
    let path_wide = wide(path.as_os_str());
    let mut current_dacl = null_mut();
    let mut source_descriptor = null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
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
        unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
        return Ok(());
    }
    let mut access = EXPLICIT_ACCESS_W {
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
    let acl_status = unsafe { SetEntriesInAclW(1, &mut access, current_dacl, &mut updated_dacl) };
    if acl_status != 0 {
        unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
        return Err(io::Error::from_raw_os_error(acl_status as i32));
    }
    unsafe { windows_sys::Win32::Foundation::LocalFree(source_descriptor) };
    let mut descriptor_storage =
        std::mem::MaybeUninit::<windows_sys::Win32::Security::SECURITY_DESCRIPTOR>::uninit();
    let descriptor = descriptor_storage.as_mut_ptr();
    let initialized =
        unsafe { windows_sys::Win32::Security::InitializeSecurityDescriptor(descriptor.cast(), 1) };
    if initialized == 0
        || unsafe {
            windows_sys::Win32::Security::SetSecurityDescriptorDacl(
                descriptor.cast(),
                1,
                updated_dacl,
                0,
            )
        } == 0
    {
        let error = last_os_error();
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(updated_dacl.cast());
        }
        return Err(error);
    }
    // SetFileSecurity updates this DACL without recursively propagating inherited ACEs.
    // Existing descendants are checked and updated individually by grant_working_directory.
    let set_status = unsafe {
        windows_sys::Win32::Security::SetFileSecurityW(
            path_wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            descriptor.cast(),
        )
    };
    let set_error = (set_status == 0).then(last_os_error);
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(updated_dacl.cast());
    }
    if let Some(error) = set_error {
        return Err(error);
    }
    Ok(())
}

fn has_multiple_links(path: &Path) -> io::Result<bool> {
    let file = fs::File::open(path)?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) } == 0
    {
        return Err(last_os_error());
    }
    Ok(information.nNumberOfLinks > 1)
}

fn create_pipe(attributes: &SECURITY_ATTRIBUTES) -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = null_mut();
    let mut write = null_mut();
    if unsafe { CreatePipe(&mut read, &mut write, attributes, 0) } == 0 {
        return Err(last_os_error());
    }
    let read = owned_handle(read)?;
    let write = owned_handle(write)?;
    Ok((read, write))
}

fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    let handle = unsafe { CreateJobObjectW(null(), null()) };
    let job = owned_handle(handle)?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..Default::default()
        },
        ..Default::default()
    };
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

fn owned_handle(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle as isize == -1 {
        return Err(last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
}

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_command_line(application: &Path, arguments: &[std::ffi::OsString]) -> Vec<u16> {
    let mut command_line = quote_windows_argument(application.as_os_str());
    for argument in arguments {
        command_line.push(' ');
        command_line.push_str(&quote_windows_argument(&argument));
    }
    wide_null(&command_line)
}

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

fn powershell_command(command: &str, working_directory: &Path) -> String {
    let working_directory = working_directory.to_string_lossy().replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'Stop'; [System.Environment]::CurrentDirectory = '{working_directory}'; Set-Location -LiteralPath '{working_directory}'; {command}"
    )
}

fn last_os_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
}

struct AttributeList(*mut c_void);

impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.0) };
    }
}

unsafe fn wide_os_string(value: windows_sys::core::PWSTR) -> std::ffi::OsString {
    let mut length = 0usize;
    while unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    std::ffi::OsString::from_wide(unsafe { std::slice::from_raw_parts(value, length) })
}

#[cfg(test)]
mod tests {
    use super::{SandboxedProcess, Zeroizing, shell_invocation};
    use std::{
        fs,
        io::ErrorKind,
        path::{Path, PathBuf},
        time::Duration,
    };
    use tokio::io::AsyncReadExt;

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
}
