use std::{
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
        ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE,
        Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT},
    },
    Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, READ_CONTROL,
    },
    System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
};
use zeroize::Zeroizing;

use crate::{
    job_objects::{create_kill_on_close_job, job_active_process_count},
    process::{shell_invocation, SandboxedProcess, WAIT_INTERVAL},
    security_acl::{
        app_container_acl_lock_path, require_non_null_acl, schedule_acl_path,
        update_app_container_acl, AppContainerAclLock, MAX_ACL_ENTRIES,
    },
    types::{owned_handle, wide_null, SandboxJobLimits, SandboxOptions},
};

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
    let original_directory_ace_count = dacl_ace_count(&directory).expect("read directory DACL");
    let original_child_ace_count = dacl_ace_count(&child).expect("read child DACL");

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
