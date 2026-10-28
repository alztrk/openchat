use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::ErrorKind,
    mem::size_of,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr::null_mut,
    time::Duration,
};

use tokio::io::AsyncReadExt;
use windows_sys::Win32::{
    Foundation::HANDLE,
    Security::{
        ACE_HEADER, ACL,
        Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo},
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, GetAce, GetLengthSid,
        GetSecurityDescriptorControl, INHERIT_ONLY_ACE, OBJECT_INHERIT_ACE,
        PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_AUTO_INHERIT_REQ, SE_DACL_AUTO_INHERITED,
        SE_DACL_DEFAULTED, SE_DACL_PRESENT, SE_DACL_PROTECTED,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FILE_TRAVERSE, GetFileInformationByHandle, READ_CONTROL, WRITE_DAC,
    },
    System::{
        JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION, SystemServices::ACCESS_ALLOWED_ACE_TYPE,
    },
};
use zeroize::Zeroizing;

use super::{
    job_objects::{create_kill_on_close_job, job_active_process_count},
    process::{SandboxedProcess, WAIT_INTERVAL, shell_invocation},
    security_acl::{
        AppContainerAclLock, MAX_ACL_ENTRIES, app_container_acl_lock_path, lock_acl_path,
        noninheriting_directory_control_value, require_non_null_acl, schedule_acl_path,
        update_acl_handle, update_app_container_acl,
    },
    types::{SandboxJobLimits, SandboxOptions, owned_handle, wide_null},
};

/// Ensures a successful ACL update cannot be applied as a null, unrestricted DACL.
#[test]
fn null_acl_update_results_are_rejected() {
    assert!(matches!(
        require_non_null_acl(null_mut()),
        Err(error) if error.kind() == ErrorKind::InvalidData
    ));
}

#[test]
fn noninheriting_directory_control_clears_auto_propagation_requests() {
    let original = SE_DACL_AUTO_INHERIT_REQ | SE_DACL_AUTO_INHERITED | SE_DACL_PROTECTED;
    assert_eq!(
        noninheriting_directory_control_value(original),
        SE_DACL_AUTO_INHERITED | SE_DACL_PROTECTED
    );
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
async fn additional_workspace_root_is_writable_without_granting_its_sibling() {
    let workspace = TestWorkspace::new();
    let primary = workspace.path().join("primary");
    let secondary = workspace.path().join("secondary");
    let outside = workspace.path().join("outside");
    fs::create_dir_all(&primary).expect("create primary workspace root");
    fs::create_dir_all(&secondary).expect("create secondary workspace root");
    fs::create_dir_all(&outside).expect("create ungranted sibling directory");
    let (program, arguments) = shell_invocation(
        "Set-Content -LiteralPath '..\\secondary\\allowed.txt' -Value 'secondary_root_write_ok'; try { Set-Content -LiteralPath '..\\outside\\denied.txt' -Value 'outside_write_bad' -ErrorAction Stop; exit 31 } catch { Write-Output 'outside_root_write_denied'; exit 0 }",
        &primary,
    );
    let additional_workspace_paths = [secondary.clone()];
    let process = SandboxedProcess::spawn_program_with_options(
        &program,
        &arguments,
        &primary,
        &[],
        SandboxOptions {
            additional_workspace_paths: &additional_workspace_paths,
            ..SandboxOptions::default()
        },
    )
    .expect("start process with one additional workspace root");
    let (exit_code, stdout, stderr) = capture(process).await;

    assert_eq!(exit_code, 0, "stdout: {stdout}; stderr: {stderr}");
    assert!(stdout.contains("outside_root_write_denied"), "{stdout}");
    assert_eq!(
        fs::read_to_string(secondary.join("allowed.txt"))
            .expect("read the file created in the additional root"),
        "secondary_root_write_ok\r\n"
    );
    assert!(
        !outside.join("denied.txt").exists(),
        "an ungranted sibling root must remain unwritable"
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
    let process =
        SandboxedProcess::spawn(&command, workspace.path()).expect("start command in AppContainer");
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
    let lock = AppContainerAclLock::acquire_for_path(&lock_path).expect("acquire ACL file lock");
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

    let sid_text = wide_null("S-1-15-2-1");
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
    set_directory_dacl_protected(&directory).expect("protect ACL test directory DACL");
    let original_directory_dacl = dacl_snapshot(&directory).expect("read original directory DACL");
    let original_child_dacl = dacl_snapshot(&child).expect("read original child DACL");

    let sid_text = wide_null("S-1-15-2-1");
    let mut sid = null_mut();
    // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
    let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
    assert_ne!(converted, 0);
    assert!(!sid.is_null());
    let expected_allow_ace = expected_allow_ace(sid, FILE_TRAVERSE | FILE_READ_ATTRIBUTES, 0);
    let target_sid_bytes = expected_allow_ace[8..].to_vec();
    assert!(!dacl_has_sid(&original_directory_dacl, &target_sid_bytes));

    let update = update_app_container_acl(
        &directory,
        sid,
        true,
        FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        0,
    );
    let updated_directory_dacl = dacl_snapshot(&directory);
    let updated_child_dacl = dacl_snapshot(&child);
    let revoke = update_app_container_acl(
        &directory,
        sid,
        false,
        FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        0,
    );
    let revoked_directory_dacl = dacl_snapshot(&directory);
    let revoked_child_dacl = dacl_snapshot(&child);
    // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
    unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

    update.expect("update non-inheriting directory DACL");
    revoke.expect("revoke non-inheriting directory DACL");
    let updated_directory_dacl = updated_directory_dacl.expect("read updated directory DACL");
    let updated_child_dacl = updated_child_dacl.expect("read updated child DACL");
    let expected_access_mask = FILE_TRAVERSE | FILE_READ_ATTRIBUTES;
    let original_access = dacl_effective_allow_rights(&original_directory_dacl);
    let mut expected_updated_access = original_access.clone();
    *expected_updated_access
        .entry(target_sid_bytes.clone())
        .or_default() |= expected_access_mask;
    assert_eq!(
        updated_directory_dacl
            .aces
            .iter()
            .filter(|ace| {
                ace.get(8..)
                    .is_some_and(|sid| sid == target_sid_bytes.as_slice())
            })
            .count(),
        1,
        "the grant must add only one ACE for the requested SID"
    );
    assert_eq!(
        updated_directory_dacl
            .aces
            .iter()
            .filter(|ace| **ace == expected_allow_ace)
            .count(),
        1,
        "the grant must use the requested allow mask and non-inheriting flags"
    );
    assert_eq!(
        dacl_effective_allow_rights(&updated_directory_dacl),
        expected_updated_access,
        "the grant must add only the requested effective access while preserving all existing allow rights"
    );
    assert_eq!(
        stable_dacl_control(updated_directory_dacl.control),
        stable_dacl_control(original_directory_dacl.control),
        "the grant must preserve stable DACL control flags"
    );
    assert_eq!(
        updated_directory_dacl.acl_revision, original_directory_dacl.acl_revision,
        "the grant must preserve the ACL revision"
    );
    assert_eq!(
        updated_child_dacl, original_child_dacl,
        "a non-inheriting directory grant must not change existing child ACLs"
    );
    let revoked_directory_dacl = revoked_directory_dacl.expect("read revoked directory DACL");
    assert_eq!(
        dacl_effective_allow_rights(&revoked_directory_dacl),
        original_access,
        "revoking the grant must restore the original effective allow rights"
    );
    assert_eq!(
        stable_dacl_control(revoked_directory_dacl.control),
        stable_dacl_control(original_directory_dacl.control),
        "revoking the grant must restore stable DACL control flags"
    );
    assert_eq!(
        revoked_directory_dacl.acl_revision, original_directory_dacl.acl_revision,
        "revoking the grant must restore the ACL revision"
    );
    assert!(
        !dacl_has_sid(&revoked_directory_dacl, &target_sid_bytes),
        "revoking the grant must remove all ACEs for the AppContainer SID"
    );
    assert_eq!(
        revoked_child_dacl.expect("read revoked child DACL"),
        original_child_dacl,
        "revoking a non-inheriting directory grant must leave existing child ACLs unchanged"
    );
}

#[test]
fn noninheriting_directory_acl_update_stays_on_the_inspected_handle() {
    let workspace = TestWorkspace::new();
    let directory = workspace.path().join("directory");
    let moved_directory = workspace.path().join("moved-directory");
    let child = directory.join("existing.txt");
    fs::create_dir(&directory).expect("create ACL test directory");
    fs::write(&child, "existing child").expect("create existing ACL test child");
    let directory = fs::canonicalize(directory).expect("canonicalize ACL test directory");
    let child = fs::canonicalize(child).expect("canonicalize ACL test child");
    let original_child_dacl = dacl_snapshot(&child).expect("read original child DACL");

    let sid_text = wide_null("S-1-15-2-1");
    let mut sid = null_mut();
    // SAFETY: `sid_text` is NUL-terminated and `sid` is a writable output pointer.
    let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) };
    assert_ne!(converted, 0);
    assert!(!sid.is_null());
    let target_sid_bytes =
        expected_allow_ace(sid, FILE_TRAVERSE | FILE_READ_ATTRIBUTES, 0)[8..].to_vec();

    let (mut handles, inspected) = lock_acl_path(&directory, false).expect("inspect ACL path");
    assert_ne!(inspected.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY, 0);
    let target = handles
        .pop()
        .expect("retain the inspected directory handle");
    fs::rename(&directory, &moved_directory).expect("move the inspected directory");
    fs::create_dir(&directory).expect("create a replacement at the inspected path");
    let replacement_dacl_before = dacl_snapshot(&directory).expect("read replacement DACL");

    let update = update_acl_handle(
        &target,
        sid,
        true,
        FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        0,
        true,
        false,
    );
    let moved_directory_dacl = dacl_snapshot(&moved_directory);
    let replacement_dacl_after = dacl_snapshot(&directory);
    let moved_child_dacl = dacl_snapshot(&moved_directory.join("existing.txt"));
    // SAFETY: ConvertStringSidToSidW allocated `sid` with LocalAlloc.
    unsafe { windows_sys::Win32::Foundation::LocalFree(sid.cast()) };

    update.expect("update the inspected directory object");
    let moved_directory_dacl = moved_directory_dacl.expect("read updated inspected directory DACL");
    assert!(dacl_has_sid(&moved_directory_dacl, &target_sid_bytes));
    assert_eq!(
        replacement_dacl_after.expect("read replacement DACL after update"),
        replacement_dacl_before,
        "the path replacement must not receive the ACL update"
    );
    assert_eq!(
        moved_child_dacl.expect("read moved child DACL"),
        original_child_dacl,
        "the non-inheriting update must preserve existing child DACLs"
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

    let sid_text = wide_null("S-1-15-2-1");
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

    let sid_text = wide_null("S-1-15-2-1");
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

#[test]
fn acl_path_handle_stays_bound_after_hard_link_and_path_replacement() {
    let workspace = TestWorkspace::new();
    let allowed = workspace.path().join("allowed");
    fs::create_dir_all(&allowed).expect("create the allowed folder");
    let target = allowed.join("target.txt");
    let alias = workspace.path().join("alias.txt");
    let renamed = allowed.join("renamed.txt");
    fs::write(&target, "selected file").expect("create the target file");
    let target = fs::canonicalize(target).expect("canonicalize the target file");

    let (handles, inspected) = lock_acl_path(&target, false).expect("pin the ACL path");
    assert_eq!(inspected.nNumberOfLinks, 1);

    fs::hard_link(&target, &alias).expect("add an alias to the inspected file");
    let alias_file = fs::File::open(&alias).expect("open the new alias");
    let mut alias_information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `alias_file` owns a live file handle and `alias_information` is writable.
    let alias_information_read = unsafe {
        GetFileInformationByHandle(alias_file.as_raw_handle() as HANDLE, &mut alias_information)
    };
    assert_ne!(
        alias_information_read, 0,
        "read the new alias file identity"
    );
    assert_eq!(
        (
            alias_information.dwVolumeSerialNumber,
            alias_information.nFileIndexHigh,
            alias_information.nFileIndexLow,
        ),
        (
            inspected.dwVolumeSerialNumber,
            inspected.nFileIndexHigh,
            inspected.nFileIndexLow,
        ),
        "a late hard link must name the same object selected for the ACL update"
    );
    assert_eq!(alias_information.nNumberOfLinks, 2);

    fs::rename(&target, &renamed)
        .expect("a same-user writer with parent delete-child access can rename the target");
    fs::write(&target, "replacement file").expect("replace the target path");

    let replacement_file = fs::File::open(&target).expect("open the replacement file");
    let mut replacement_information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `replacement_file` owns a live file handle and the information structure is writable.
    let replacement_information_read = unsafe {
        GetFileInformationByHandle(
            replacement_file.as_raw_handle() as HANDLE,
            &mut replacement_information,
        )
    };
    assert_ne!(
        replacement_information_read, 0,
        "read the replacement file identity"
    );
    assert_ne!(
        (
            replacement_information.dwVolumeSerialNumber,
            replacement_information.nFileIndexHigh,
            replacement_information.nFileIndexLow,
        ),
        (
            inspected.dwVolumeSerialNumber,
            inspected.nFileIndexHigh,
            inspected.nFileIndexLow,
        ),
        "the target path now names a different file object"
    );

    let inspected_file = handles.last().expect("retain the inspected target handle");
    let mut pinned_information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `inspected_file` owns a live handle and the information structure is writable.
    let pinned_information_read = unsafe {
        GetFileInformationByHandle(
            inspected_file.as_raw_handle() as HANDLE,
            &mut pinned_information,
        )
    };
    assert_ne!(
        pinned_information_read, 0,
        "read the retained target identity"
    );
    assert_eq!(
        (
            pinned_information.dwVolumeSerialNumber,
            pinned_information.nFileIndexHigh,
            pinned_information.nFileIndexLow,
        ),
        (
            inspected.dwVolumeSerialNumber,
            inspected.nFileIndexHigh,
            inspected.nFileIndexLow,
        ),
        "the ACL handle remains bound to the original file object"
    );

    drop(alias_file);
    drop(replacement_file);
    drop(handles);
}

#[derive(Debug, Eq, PartialEq)]
struct DaclSnapshot {
    control: u16,
    acl_revision: u8,
    aces: Vec<Vec<u8>>,
}

/// Returns the number of ACEs from a validated DACL snapshot.
fn dacl_ace_count(path: &Path) -> std::io::Result<u16> {
    let count = dacl_snapshot(path)?.aces.len();
    u16::try_from(count).map_err(|_| {
        std::io::Error::new(ErrorKind::InvalidData, "The test DACL has too many ACEs.")
    })
}

/// Returns effective allow masks by SID for a DACL containing only standard allow ACEs.
fn dacl_effective_allow_rights(snapshot: &DaclSnapshot) -> BTreeMap<Vec<u8>, u32> {
    assert!(
        snapshot
            .aces
            .iter()
            .all(|ace| ace.first() == Some(&(ACCESS_ALLOWED_ACE_TYPE as u8))),
        "effective allow rights require standard allow ACEs without deny entries"
    );
    let mut rights = BTreeMap::new();
    for ace in &snapshot.aces {
        assert!(
            ace.len() >= 8,
            "a standard allow ACE must contain its header and mask"
        );
        if u32::from(ace[1]) & INHERIT_ONLY_ACE != 0 {
            continue;
        }
        let access_mask = u32::from_le_bytes([ace[4], ace[5], ace[6], ace[7]]);
        *rights.entry(ace[8..].to_vec()).or_default() |= access_mask;
    }
    rights
}

fn stable_dacl_control(control: u16) -> u16 {
    // The filesystem resource manager recalculates SE_DACL_AUTO_INHERITED on DACL updates.
    control & (SE_DACL_PRESENT | SE_DACL_DEFAULTED | SE_DACL_PROTECTED)
}

fn dacl_has_sid(snapshot: &DaclSnapshot, sid: &[u8]) -> bool {
    snapshot
        .aces
        .iter()
        .any(|ace| ace.len() >= 8 && ace[0] == ACCESS_ALLOWED_ACE_TYPE as u8 && &ace[8..] == sid)
}

/// Reads a DACL snapshot while preserving ACE order because deny/allow order affects access checks.
fn dacl_snapshot(path: &Path) -> std::io::Result<DaclSnapshot> {
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
        GetSecurityInfo(
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
    let result = (|| {
        if descriptor.is_null() || dacl.is_null() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "The test object has no DACL security descriptor.",
            ));
        }
        let mut control = 0;
        let mut descriptor_revision = 0;
        // SAFETY: `descriptor` is a live descriptor returned by GetSecurityInfo.
        if unsafe {
            GetSecurityDescriptorControl(descriptor, &mut control, &mut descriptor_revision)
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `dacl` is a live ACL returned by GetSecurityInfo.
        let (ace_count, acl_revision, acl_start, acl_end) = unsafe {
            let acl_start = dacl.cast::<u8>() as usize;
            let acl_size = usize::from((*dacl).AclSize);
            let acl_end = acl_start.checked_add(acl_size);
            ((*dacl).AceCount, (*dacl).AclRevision, acl_start, acl_end)
        };
        let acl_end = acl_end.ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidData, "The test DACL size overflows.")
        })?;
        let acl_header_end = acl_start.checked_add(size_of::<ACL>()).ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidData, "The test ACL overflows.")
        })?;
        if acl_header_end > acl_end {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "The test DACL is smaller than its header.",
            ));
        }
        let mut aces = Vec::with_capacity(usize::from(ace_count));
        for index in 0..ace_count {
            let mut ace = null_mut();
            // SAFETY: `dacl` is a live ACL and `ace` is a writable output pointer.
            if unsafe { GetAce(dacl, u32::from(index), &mut ace) } == 0 || ace.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let ace_start = ace as usize;
            let ace_header_end =
                ace_start
                    .checked_add(size_of::<ACE_HEADER>())
                    .ok_or_else(|| {
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "The test ACE header overflows.",
                        )
                    })?;
            if ace_start < acl_header_end
                || ace_header_end > acl_end
                || !ace_start.is_multiple_of(std::mem::align_of::<ACE_HEADER>())
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "The test DACL contains an invalid ACE header range.",
                ));
            }
            // SAFETY: The complete ACE header was checked to lie within the live DACL buffer.
            let ace_size = usize::from(unsafe { (*ace.cast::<ACE_HEADER>()).AceSize });
            if ace_size < size_of::<ACE_HEADER>()
                || !ace_size.is_multiple_of(std::mem::align_of::<ACE_HEADER>())
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "The test DACL contains an invalid ACE size.",
                ));
            }
            let ace_end = ace_start.checked_add(ace_size).ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidData, "The test ACE size overflows.")
            })?;
            if ace_start < acl_header_end || ace_end > acl_end {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "The test DACL contains an ACE outside its buffer.",
                ));
            }
            // SAFETY: The ACE size comes from the validated ACE header in the live DACL.
            aces.push(unsafe { std::slice::from_raw_parts(ace.cast::<u8>(), ace_size) }.to_vec());
        }
        let dacl_control_mask = SE_DACL_PRESENT
            | SE_DACL_DEFAULTED
            | SE_DACL_AUTO_INHERIT_REQ
            | SE_DACL_AUTO_INHERITED
            | SE_DACL_PROTECTED;
        Ok(DaclSnapshot {
            control: control & dacl_control_mask,
            acl_revision,
            aces,
        })
    })();
    // SAFETY: `descriptor` was allocated by GetSecurityInfo and has no remaining users.
    unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor) };
    result
}

fn set_directory_dacl_protected(path: &Path) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options
        .access_mode(READ_CONTROL | WRITE_DAC)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: `file` owns the directory handle and both output pointers are writable.
    let status = unsafe {
        GetSecurityInfo(
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
    if dacl.is_null() {
        // SAFETY: The descriptor was allocated by GetSecurityInfo and LocalFree accepts null.
        unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor) };
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The ACL test directory has no DACL.",
        ));
    }
    // SAFETY: `file` owns the target handle and `dacl` is a valid ACL returned by GetSecurityInfo.
    let status = unsafe {
        SetSecurityInfo(
            file.as_raw_handle() as HANDLE,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null_mut(),
        )
    };
    // SAFETY: The descriptor was allocated by GetSecurityInfo and has no remaining users.
    unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor) };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status as i32))
    }
}

/// Encodes the exact non-object allow ACE expected from the tested directory grant.
fn expected_allow_ace(
    sid: windows_sys::Win32::Security::PSID,
    access_mask: u32,
    flags: u8,
) -> Vec<u8> {
    // SAFETY: `sid` is a valid SID allocated by ConvertStringSidToSidW and remains live here.
    let sid_length = unsafe { GetLengthSid(sid) } as usize;
    let ace_size = u16::try_from(8 + sid_length).expect("the test SID fits in an ACE");
    let mut ace = Vec::with_capacity(usize::from(ace_size));
    ace.push(u8::try_from(ACCESS_ALLOWED_ACE_TYPE).expect("ACE type fits in one byte"));
    ace.push(flags);
    ace.extend_from_slice(&ace_size.to_le_bytes());
    ace.extend_from_slice(&access_mask.to_le_bytes());
    // SAFETY: `sid` is a valid SID allocated by ConvertStringSidToSidW and GetLengthSid
    // returned its readable byte length.
    ace.extend_from_slice(unsafe { std::slice::from_raw_parts(sid.cast(), sid_length) });
    ace
}
