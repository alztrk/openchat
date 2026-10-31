//! Split from tests.rs (9 items).
use super::*;

#[test]
fn named_project_task_is_loaded_as_a_bounded_terminal_command() {
    let directory = TestDirectory::new();
    directory.write(
        ".openchat/tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );

    let task =
        super::super::project_tasks::load_project_task(Path::new(directory.root()), "verify")
            .expect("load declared project task");
    assert_eq!(task.id, "verify");
    assert_eq!(task.command, "cargo test");
    assert_eq!(task.timeout_seconds, 90);

    let tasks = super::super::project_tasks::load_project_tasks(Path::new(directory.root()))
        .expect("list declared project tasks");
    assert_eq!(tasks, vec![task]);

    let missing =
        super::super::project_tasks::load_project_task(Path::new(directory.root()), "build")
            .expect_err("undeclared tasks must fail explicitly");
    assert_eq!(missing.code, "project_task_not_found");
}

#[tokio::test]
async fn terminal_manager_runs_command_and_handles_input_and_kill() {
    use super::super::terminal::TerminalSessionManager;
    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();

    // 1. Fast command execution test
    let exec_res = manager
        .execute(
            "echo openchat_terminal_ok",
            Path::new(directory.root()),
            TOOL_TEST_CONVERSATION_ID,
            Some(10),
            Some(10000),
        )
        .await
        .expect("execute command");
    let exec_res = if exec_res["is_running"].as_bool() == Some(true) {
        let terminal_id = exec_res["terminal_id"]
            .as_str()
            .expect("running command should return its terminal ID")
            .to_owned();
        manager
            .read_output_of(&terminal_id, TOOL_TEST_CONVERSATION_ID, Some(10000))
            .await
            .expect("wait for the short command to finish")
    } else {
        exec_res
    };
    assert_eq!(exec_res["is_running"], false);
    assert_eq!(exec_res["exit_code"], 0);
    assert!(
        exec_res["output"]
            .as_str()
            .unwrap_or("")
            .contains("openchat_terminal_ok")
    );

    let failing_command = "exit 17";
    let failed = manager
        .execute(
            failing_command,
            Path::new(directory.root()),
            TOOL_TEST_CONVERSATION_ID,
            Some(10),
            Some(10000),
        )
        .await
        .expect("execute nonzero command");
    assert_eq!(failed["is_running"], false);
    assert_eq!(failed["exit_code"], 17);

    // 2. Interactive session creation test
    #[cfg(windows)]
    let interactive_cmd = "$line = [Console]::ReadLine(); Write-Host ('INPUT_RECV:' + $line)";
    #[cfg(not(windows))]
    let interactive_cmd = "read line; echo \"INPUT_RECV:$line\"";

    let session = manager
        .create_session(
            interactive_cmd,
            Path::new(directory.root()),
            TOOL_TEST_CONVERSATION_ID,
            Some(30),
        )
        .await
        .expect("create interactive session");

    let term_id = session.id.clone();
    assert!(session.is_running().await.expect("read process status"));
    assert!(
        manager
            .get_session(&term_id, "another-conversation")
            .await
            .is_none(),
        "another conversation cannot inspect this terminal session"
    );
    assert!(
        manager
            .send_input_to(
                &term_id,
                "another-conversation",
                "unauthorized\n",
                Some(100)
            )
            .await
            .is_err(),
        "another conversation cannot send terminal input"
    );
    assert!(
        manager
            .read_output_of(&term_id, "another-conversation", Some(100))
            .await
            .is_err(),
        "another conversation cannot read terminal output"
    );
    assert!(
        manager
            .kill_session(&term_id, "another-conversation")
            .await
            .is_err(),
        "another conversation cannot terminate the session"
    );

    // Send input to the waiting process
    let input_res = manager
        .send_input_to(
            &term_id,
            TOOL_TEST_CONVERSATION_ID,
            "interactive_message\n",
            Some(3000),
        )
        .await
        .expect("send input to terminal");

    assert!(
        input_res["output"]
            .as_str()
            .unwrap_or("")
            .contains("INPUT_RECV:interactive_message")
    );

    // 3. Kill session test
    let kill_res = manager
        .kill_session(&term_id, TOOL_TEST_CONVERSATION_ID)
        .await;
    // Session may have already finished after readLine or is killed
    let _ = kill_res;
    assert!(
        manager
            .get_session(&term_id, TOOL_TEST_CONVERSATION_ID)
            .await
            .is_none()
    );
    manager.stop_all().await.expect("stop remaining sessions");
}

#[tokio::test]
async fn terminal_manager_shutdown_stops_running_command_sessions() {
    use super::super::terminal::TerminalSessionManager;

    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();
    #[cfg(windows)]
    let command = "Start-Sleep -Seconds 30";
    #[cfg(not(windows))]
    let command = "sleep 30";

    let result = manager
        .execute(
            command,
            Path::new(directory.root()),
            TOOL_TEST_CONVERSATION_ID,
            Some(60),
            Some(100),
        )
        .await
        .expect("start a long-running command");
    let terminal_id = result["terminal_id"]
        .as_str()
        .expect("running command has a terminal id")
        .to_owned();
    let session = manager
        .get_session(&terminal_id, TOOL_TEST_CONVERSATION_ID)
        .await
        .expect("running session stays registered");
    assert!(session.is_running().await.expect("check running session"));

    manager
        .stop_all()
        .await
        .expect("stop sessions during shutdown");

    assert!(!session.is_running().await.expect("check stopped session"));
    assert!(
        manager
            .get_session(&terminal_id, TOOL_TEST_CONVERSATION_ID)
            .await
            .is_none()
    );
}

#[cfg(windows)]
/// Ensures terminal shutdown exits its descendant, observed through a stable process handle.
#[tokio::test]
async fn terminal_manager_shutdown_stops_descendant_processes() {
    use super::super::terminal::TerminalSessionManager;
    use std::{fs, time::Duration};
    use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};

    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();
    let command = "$child = Start-Process -FilePath 'powershell.exe' -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep -Seconds 30' -PassThru; Set-Content -Path 'child-pid.txt' -Value $child.Id; Start-Sleep -Seconds 30";
    manager
        .execute(
            command,
            Path::new(directory.root()),
            TOOL_TEST_CONVERSATION_ID,
            Some(60),
            Some(100),
        )
        .await
        .expect("start a shell with a long-running descendant");

    let pid_file = directory.0.join("child-pid.txt");
    let child_pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(contents) = fs::read_to_string(&pid_file)
                && let Ok(pid) = contents.trim().parse::<u32>()
            {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    let child_pid = match child_pid {
        Ok(pid) => pid,
        Err(_) => {
            manager
                .stop_all()
                .await
                .expect("clean up shell after descendant startup timeout");
            panic!("descendant process starts");
        }
    };

    let child_process = match open_windows_process_handle(child_pid) {
        Ok(handle) => handle,
        Err(error) => {
            manager
                .stop_all()
                .await
                .expect("clean up shell after opening descendant handle fails");
            panic!("open descendant process handle: {error}");
        }
    };
    let was_running = windows_process_wait_result(&child_process, 0) == WAIT_TIMEOUT;
    let stop_result = manager.stop_all().await;

    assert!(was_running, "descendant process is running before shutdown");
    stop_result.expect("stop shell and descendant processes");
    assert_eq!(
        windows_process_wait_result(&child_process, 5_000),
        WAIT_OBJECT_0,
        "descendant process exits after its job is stopped"
    );
}

#[cfg(windows)]
/// Opens a process with only the synchronization right needed by shutdown assertions.
fn open_windows_process_handle(pid: u32) -> std::io::Result<std::os::windows::io::OwnedHandle> {
    use std::os::windows::io::{FromRawHandle, RawHandle};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};

    // SAFETY: PROCESS_SYNCHRONIZE grants only the wait right needed by this test.
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error());
    }

    // SAFETY: OpenProcess returned a valid handle owned by this test process.
    Ok(unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle as RawHandle) })
}

#[cfg(windows)]
/// Waits on an owned process handle and returns the Windows wait status unchanged.
fn windows_process_wait_result(handle: &std::os::windows::io::OwnedHandle, timeout_ms: u32) -> u32 {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Foundation::WAIT_EVENT, System::Threading::WaitForSingleObject};

    // SAFETY: The process handle remains owned and valid for this synchronous wait.
    unsafe { WaitForSingleObject(handle.as_raw_handle(), timeout_ms) as WAIT_EVENT }
}

#[tokio::test]
async fn simulate_real_world_agent_terminal_and_file_flow() {
    use super::super::executor::{ToolExecutor, ToolPermissionMode, execute_model_tool};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    // Step 1: Write a workflow script file
    #[cfg(windows)]
    let script_name = "workflow.ps1";
    #[cfg(windows)]
    let script_content = "Write-Output STEP1_INIT_DONE\r\nWrite-Output ENTER_CODE:\r\n$code = [Console]::In.ReadLine()\r\nWrite-Output \"STEP2_CODE_IS:$code\"\r\n";

    #[cfg(not(windows))]
    let script_name = "workflow.sh";
    #[cfg(not(windows))]
    let script_content = "#!/bin/sh\necho \"STEP1_INIT_DONE\"\nprintf \"ENTER_CODE:\"\nread CODE\necho \"STEP2_CODE_IS:$CODE\"\n";

    let script_path = Path::new(directory.root())
        .join(script_name)
        .display()
        .to_string();

    let write_call = ToolCall {
        id: "call_write_1".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({
            "path": script_path,
            "content": script_content,
        }),
    };
    let prepared_write = executor
        .prepare_call(&write_call)
        .expect("prepare write_file");
    let write_result = execute_model_tool(&prepared_write, TOOL_TEST_CONVERSATION_ID).await;
    assert!(
        write_result.get("error").is_none(),
        "write_file failed: {write_result:?}"
    );

    // Step 2: Read file back to verify integrity
    let read_call = ToolCall {
        id: "call_read_1".to_owned(),
        name: "read_file".to_owned(),
        arguments: json!({
            "path": script_path,
        }),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read_file");
    let read_result = execute_model_tool(&prepared_read, TOOL_TEST_CONVERSATION_ID).await;
    assert!(read_result.get("error").is_none());
    assert!(read_result.to_string().contains("STEP1_INIT_DONE"));

    // Step 3: Execute the script interactively via execute_command / bash
    #[cfg(windows)]
    let run_cmd = format!(
        "& '{}'",
        Path::new(directory.root()).join(script_name).display()
    );
    #[cfg(not(windows))]
    let run_cmd = format!(
        "sh \"{}\"",
        Path::new(directory.root()).join(script_name).display()
    );

    let exec_call = ToolCall {
        id: "call_exec_1".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": run_cmd,
            "wait_ms": 1500,
        }),
    };
    let prepared_exec = executor
        .prepare_call(&exec_call)
        .expect("prepare execute_command");
    let exec_result = execute_model_tool(&prepared_exec, TOOL_TEST_CONVERSATION_ID).await;
    assert!(
        exec_result.get("error").is_none(),
        "exec failed: {exec_result:?}"
    );

    let term_id = exec_result["terminal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("terminal_id should be returned: {exec_result}"))
        .to_owned();
    let initial_output = exec_result["output"].as_str().unwrap_or("");
    assert!(
        initial_output.contains("STEP1_INIT_DONE"),
        "initial output was: {initial_output}"
    );
    assert_eq!(
        exec_result["is_running"], true,
        "process should still be running waiting for input"
    );

    // Step 4: Send the expected input via send_terminal_input
    let input_call = ToolCall {
        id: "call_input_1".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": term_id,
            "input": "AGENT_VERIFIED_777\n",
            "wait_ms": 2500,
        }),
    };
    let prepared_input = executor
        .prepare_call(&input_call)
        .expect("prepare send_terminal_input");
    let mut input_result = execute_model_tool(&prepared_input, TOOL_TEST_CONVERSATION_ID).await;
    assert!(
        input_result.get("error").is_none(),
        "send_terminal_input failed: {input_result:?}"
    );

    for _ in 0..60 {
        if input_result["is_running"] != true {
            break;
        }
        input_result = super::super::terminal::TerminalSessionManager::global()
            .read_output_of(&term_id, TOOL_TEST_CONVERSATION_ID, Some(500))
            .await
            .expect("interactive workflow output should remain readable");
    }
    if input_result["is_running"] == true {
        super::super::terminal::TerminalSessionManager::global()
            .kill_session(&term_id, TOOL_TEST_CONVERSATION_ID)
            .await
            .expect("timed-out interactive workflow should be terminated");
    }

    let after_input_output = input_result["output"].as_str().unwrap_or("");
    assert!(
        after_input_output.contains("STEP2_CODE_IS:AGENT_VERIFIED_777"),
        "expected code in output, got: {after_input_output}"
    );
    assert_eq!(
        input_result["is_running"], false,
        "process should have completed after receiving input"
    );
    assert_eq!(input_result["exit_code"], 0);

    // Step 5: Test edit_file on the script
    let edit_call = ToolCall {
        id: "call_edit_1".to_owned(),
        name: "edit_file".to_owned(),
        arguments: json!({
            "path": script_path,
            "old_string": "STEP1_INIT_DONE",
            "new_string": "STEP1_INITIALIZED_REVISED",
        }),
    };
    let prepared_edit = executor
        .prepare_call(&edit_call)
        .expect("prepare edit_file");
    let edit_result = execute_model_tool(&prepared_edit, TOOL_TEST_CONVERSATION_ID).await;
    assert!(
        edit_result.get("error").is_none(),
        "edit_file failed: {edit_result:?}"
    );

    // Verify revision by reading
    let read_revised = execute_model_tool(&prepared_read, TOOL_TEST_CONVERSATION_ID).await;
    assert!(
        read_revised
            .to_string()
            .contains("STEP1_INITIALIZED_REVISED")
    );

    // Step 6: Test long-running process and kill action via send_terminal_input
    #[cfg(windows)]
    let long_running_cmd = "Start-Sleep -Seconds 120";
    #[cfg(not(windows))]
    let long_running_cmd = "sleep 120";

    let long_exec_call = ToolCall {
        id: "call_long_exec".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": long_running_cmd,
            "wait_ms": 500,
        }),
    };
    let prepared_long_exec = executor
        .prepare_call(&long_exec_call)
        .expect("prepare long bash call");
    let long_result = execute_model_tool(&prepared_long_exec, TOOL_TEST_CONVERSATION_ID).await;
    assert_eq!(long_result["is_running"], true);
    let long_term_id = long_result["terminal_id"]
        .as_str()
        .expect("long term id")
        .to_owned();

    // Kill the long-running session
    let kill_call = ToolCall {
        id: "call_kill".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": long_term_id,
            "action": "kill",
        }),
    };
    let prepared_kill = executor
        .prepare_call(&kill_call)
        .expect("prepare kill call");
    let kill_result = execute_model_tool(&prepared_kill, TOOL_TEST_CONVERSATION_ID).await;
    assert_eq!(kill_result["status"], "terminated");
}

#[tokio::test]
async fn simulate_terminal_edge_cases_and_exit_codes() {
    use super::super::executor::{ToolExecutor, ToolPermissionMode, execute_model_tool};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    // 1. Non-zero exit code simulation
    let non_zero_cmd = "exit 77";

    let exit_call = ToolCall {
        id: "call_exit_code".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": non_zero_cmd,
        }),
    };
    let prepared_exit = executor
        .prepare_call(&exit_call)
        .expect("prepare exit call");
    let exit_result = execute_model_tool(&prepared_exit, TOOL_TEST_CONVERSATION_ID).await;
    assert_eq!(exit_result["exit_code"], 77);
    assert_eq!(exit_result["is_running"], false);

    // 2. Reading output from an existing active terminal session
    #[cfg(windows)]
    let pause_cmd = "Write-Output READY_FOR_READ; [void][Console]::In.ReadLine()";
    #[cfg(not(windows))]
    let pause_cmd = "sh -c \"echo READY_FOR_READ; read -r dummy\"";

    let start_call = ToolCall {
        id: "call_start_read".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": pause_cmd,
            "wait_ms": 1000,
        }),
    };
    let prepared_start = executor
        .prepare_call(&start_call)
        .expect("prepare pause call");
    let start_result = execute_model_tool(&prepared_start, TOOL_TEST_CONVERSATION_ID).await;
    let tid = start_result["terminal_id"]
        .as_str()
        .expect("tid")
        .to_owned();
    assert_eq!(start_result["is_running"], true);
    assert!(
        start_result["output"]
            .as_str()
            .unwrap_or("")
            .contains("READY_FOR_READ")
    );

    // Call execute_command with terminal_id and action: "read"
    let read_call = ToolCall {
        id: "call_read_term".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "terminal_id": tid,
            "action": "read",
            "wait_ms": 200,
        }),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read call");
    let read_result = execute_model_tool(&prepared_read, TOOL_TEST_CONVERSATION_ID).await;
    assert_eq!(read_result["terminal_id"], tid);
    assert_eq!(read_result["is_running"], true);

    // 3. Send enter key to finish pause via execute_command with terminal_id and input
    let send_finish = ToolCall {
        id: "call_finish".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "terminal_id": tid,
            "input": "\n",
            "wait_ms": 2000,
        }),
    };
    let prepared_finish = executor.prepare_call(&send_finish).expect("prepare finish");
    let finish_result = execute_model_tool(&prepared_finish, TOOL_TEST_CONVERSATION_ID).await;
    assert_eq!(finish_result["is_running"], false);
    assert_eq!(
        finish_result["exit_code"], 0,
        "finish result: {finish_result}"
    );

    // 4. Invalid terminal session id error handling
    let stale_call = ToolCall {
        id: "call_stale".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": "non_existent_terminal_9999",
            "input": "test\n",
        }),
    };
    let prepared_stale = executor
        .prepare_call(&stale_call)
        .expect("prepare stale call");
    let stale_result = execute_model_tool(&prepared_stale, TOOL_TEST_CONVERSATION_ID).await;
    assert!(stale_result.get("error").is_some());
    assert_eq!(stale_result["error"]["code"], "terminal_error");
}

#[tokio::test]
async fn simulate_named_project_task_and_terminal_permission_approval_flow() {
    use super::super::executor::{ToolExecutor, ToolPermissionMode};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use std::sync::Arc;
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    let storage = Arc::new(
        AppStorage::open_at(PathBuf::from(directory.root())).expect("open isolated test storage"),
    );
    storage
        .connect()
        .expect("connect isolated test storage")
        .execute_batch(
            "CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                UNIQUE(conversation_id, id)
            );",
        )
        .expect("create assistant message checkpoint schema");
    fs::create_dir_all(Path::new(directory.root()).join(".openchat"))
        .expect("create named project task directory");
    fs::write(
        Path::new(directory.root()).join(".openchat/tasks.json"),
        r#"{"version":1,"tasks":[{"id":"verify","command":"echo USER_APPROVED_EXECUTION","timeoutSeconds":20}]}"#,
    )
    .expect("write named project task catalog");
    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_req_1");
    let snapshot = ChatStreamSnapshot::new("conv_1", "msg_1", "", 0);

    // 1. Approval Granted Case
    let call_allow = ToolCall {
        id: "call_allow_1".to_owned(),
        name: "run_project_task".to_owned(),
        arguments: json!({"task": "verify"}),
    };

    let (_cancellation_tx, mut cancellation_rx) = watch::channel(false);
    let broker_clone = broker.clone();

    let exec_task = tokio::spawn({
        let mut allowed_tools = super::super::definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        allowed_tools
            .push(super::super::project_tasks::tool_definition(&["verify".to_owned()]).name);
        let executor = ToolExecutor::with_allowed_tool_names(
            Some(Path::new(directory.root())),
            Path::new(directory.root()),
            ToolPermissionMode::RequireApproval,
            allowed_tools,
        );
        let request_id = request_id.clone();
        let snapshot = snapshot.clone();
        let events = events.clone();
        let storage = Arc::clone(&storage);
        let user_questions = UserQuestionBroker::default();
        async move {
            executor
                .execute_call(
                    &call_allow,
                    ToolExecutionContext {
                        permissions: &broker_clone,
                        request_id: &request_id,
                        snapshot: &snapshot,
                        events: &events,
                        cancellation: &mut cancellation_rx,
                        storage: &storage,
                        run_id: "test_run_1",
                        provider_id: "opencode",
                        user_questions: &user_questions,
                        image_generation: None,
                    },
                )
                .await
        }
    });

    // Wait until permission is requested
    let mut pending_id = None;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let pending = broker.pending.lock().expect("approval registry lock");
        if let Some(first_id) = pending.keys().next() {
            pending_id = Some(first_id.clone());
            break;
        }
    }
    let approval_id = pending_id.expect("approval should be requested");

    // Simulate user approving the command execution in UI
    let respond_res = broker.respond(&approval_id, true).await;
    assert!(respond_res.is_ok());

    let mut result = exec_task.await.expect("join task").expect("execute_call");
    for _ in 0..60 {
        if result.output["is_running"] != true {
            break;
        }
        let terminal_id = result.output["terminal_id"]
            .as_str()
            .expect("running command should expose its terminal session")
            .to_owned();
        result.output = super::super::terminal::TerminalSessionManager::global()
            .read_output_of(&terminal_id, "conv_1", Some(500))
            .await
            .expect("approved command output should remain readable");
    }
    if result.output["is_running"] == true
        && let Some(terminal_id) = result.output["terminal_id"].as_str()
    {
        super::super::terminal::TerminalSessionManager::global()
            .kill_session(terminal_id, "conv_1")
            .await
            .expect("timed-out approved command should be terminated");
    }
    assert!(
        result.output["output"]
            .as_str()
            .unwrap_or("")
            .contains("USER_APPROVED_EXECUTION")
    );
    assert_eq!(result.output["exit_code"], 0);

    // 2. Approval Denied Case
    let call_deny = ToolCall {
        id: "call_deny_1".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": "echo USER_DENIED_EXECUTION",
        }),
    };

    let (_cancellation_tx2, mut cancellation_rx2) = watch::channel(false);
    let broker_clone2 = broker.clone();

    let deny_task = tokio::spawn({
        let executor = ToolExecutor::new(
            Some(Path::new(directory.root())),
            Path::new(directory.root()),
            ToolPermissionMode::RequireApproval,
        );
        let request_id = request_id.clone();
        let snapshot = snapshot.clone();
        let events = events.clone();
        let storage = Arc::clone(&storage);
        let user_questions = UserQuestionBroker::default();
        async move {
            executor
                .execute_call(
                    &call_deny,
                    ToolExecutionContext {
                        permissions: &broker_clone2,
                        request_id: &request_id,
                        snapshot: &snapshot,
                        events: &events,
                        cancellation: &mut cancellation_rx2,
                        storage: &storage,
                        run_id: "test_run_2",
                        provider_id: "opencode",
                        user_questions: &user_questions,
                        image_generation: None,
                    },
                )
                .await
        }
    });

    let mut deny_pending_id = None;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let pending = broker.pending.lock().expect("approval registry lock");
        if let Some(first_id) = pending.keys().next() {
            deny_pending_id = Some(first_id.clone());
            break;
        }
    }
    let deny_approval_id = deny_pending_id.expect("deny approval should be requested");

    // Simulate user clicking "Deny" in UI
    let deny_res = broker.respond(&deny_approval_id, false).await;
    assert!(deny_res.is_ok());

    let deny_result = deny_task.await.expect("join task").expect("execute_call");
    assert_eq!(deny_result.output["error"]["code"], "permission_denied");
}
