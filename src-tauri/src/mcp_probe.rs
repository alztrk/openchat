use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::mcp_storage::{
    all_environment_names, mcp_directory, read_secret_map, require_server,
    validate_environment_name, write_server_config, GlobalMcpServer, McpStorageError,
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(12);
const MAXIMUM_RPC_LINE_BYTES: usize = 1024 * 1024;
const MAXIMUM_RPC_LINES: usize = 1000;

#[tauri::command]
pub fn probe_global_mcp_server(server_id: String) -> Result<GlobalMcpServer, McpStorageError> {
    #[cfg(windows)]
    {
        probe_global_mcp_server_windows(&server_id)
    }
    #[cfg(not(windows))]
    {
        let _ = server_id;
        Err(McpStorageError::probe_failed())
    }
}

#[cfg(windows)]
fn probe_global_mcp_server_windows(server_id: &str) -> Result<GlobalMcpServer, McpStorageError> {
    let mut server = require_server(server_id)?;
    let secrets = read_secret_map()?;
    let allowed_names = all_environment_names(&server);
    let mut process_environment = Vec::new();

    for name in &server.required_env_keys {
        let key = secret_key(&server.id, name);
        let Some(value) = secrets.0.get(&key).filter(|value| !value.trim().is_empty()) else {
            return Err(McpStorageError::missing_credentials());
        };
        process_environment.push((name.clone(), Zeroizing::new(value.clone())));
    }
    for name in &server.optional_env_keys {
        if !allowed_names.contains(name.as_str()) {
            return Err(McpStorageError::probe_failed());
        }
        if let Some(value) = secrets.0.get(&secret_key(&server.id, name)) {
            if !value.trim().is_empty() {
                process_environment.push((name.clone(), Zeroizing::new(value.clone())));
            }
        }
    }
    for (name, value) in &process_environment {
        validate_environment_name(name)?;
        if value.len() > 2500 || value.contains('\0') {
            return Err(McpStorageError::probe_failed());
        }
    }

    server.discovered_tools = probe_tools(&server, &process_environment)?;
    let root = mcp_directory()?;
    let canonical_root =
        std::fs::canonicalize(root).map_err(|_| McpStorageError::probe_failed())?;
    write_server_config(&canonical_root, &server)?;
    Ok(server)
}

#[cfg(windows)]
fn secret_key(server_id: &str, environment_name: &str) -> String {
    format!("openchat_global_mcp_{server_id}_{environment_name}")
}

#[cfg(windows)]
fn probe_tools(
    server: &GlobalMcpServer,
    environment: &[(String, Zeroizing<String>)],
) -> Result<Vec<String>, McpStorageError> {
    let mut command = if server.command == "npx" {
        let command_processor = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
        let mut command = Command::new(command_processor);
        command.args(["/D", "/C", "npx.cmd"]);
        command
    } else if server.command == "uvx" {
        Command::new("uvx")
    } else {
        return Err(McpStorageError::probe_failed());
    };
    command
        .args(&server.args)
        .current_dir(&server.install_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (name, value) in environment {
        command.env(name, value.as_str());
    }

    let mut child = command
        .spawn()
        .map_err(|_| McpStorageError::probe_failed())?;
    let process_job = match ProcessJob::assign(&child) {
        Ok(job) => job,
        Err(()) => {
            terminate_process(&mut child, None);
            return Err(McpStorageError::probe_failed());
        }
    };
    let Some(mut stdin) = child.stdin.take() else {
        terminate_process(&mut child, Some(process_job));
        return Err(McpStorageError::probe_failed());
    };
    let Some(stdout) = child.stdout.take() else {
        terminate_process(&mut child, Some(process_job));
        return Err(McpStorageError::probe_failed());
    };
    let (sender, receiver) = mpsc::sync_channel(16);
    thread::spawn(move || read_protocol_lines(stdout, sender));

    if write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "OpenChat", "version": env!("CARGO_PKG_VERSION") },
            },
        }),
    )
    .is_err()
    {
        terminate_process(&mut child, Some(process_job));
        return Err(McpStorageError::probe_failed());
    }

    let result = receive_tool_list(&mut stdin, &receiver);
    terminate_process(&mut child, Some(process_job));
    result
}

#[cfg(windows)]
fn receive_tool_list(
    stdin: &mut std::process::ChildStdin,
    receiver: &Receiver<Result<String, ()>>,
) -> Result<Vec<String>, McpStorageError> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut line_count = 0;
    let mut requested_tools = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(McpStorageError::probe_failed());
        }
        let line = match receiver.recv_timeout(remaining) {
            Ok(Ok(line)) => line,
            Ok(Err(())) | Err(RecvTimeoutError::Disconnected | RecvTimeoutError::Timeout) => {
                return Err(McpStorageError::probe_failed());
            }
        };
        line_count += 1;
        if line_count > MAXIMUM_RPC_LINES {
            return Err(McpStorageError::probe_failed());
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message.get("id").and_then(Value::as_u64) {
            Some(1) if !requested_tools => {
                if message.get("error").is_some() {
                    return Err(McpStorageError::probe_failed());
                }
                write_request(
                    stdin,
                    json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/initialized",
                    }),
                )
                .map_err(|_| McpStorageError::probe_failed())?;
                write_request(
                    stdin,
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/list",
                        "params": {},
                    }),
                )
                .map_err(|_| McpStorageError::probe_failed())?;
                requested_tools = true;
            }
            Some(2) if requested_tools => return parse_tools(&message),
            _ => {}
        }
    }
}

#[cfg(windows)]
fn parse_tools(message: &Value) -> Result<Vec<String>, McpStorageError> {
    if message.get("error").is_some() {
        return Err(McpStorageError::probe_failed());
    }
    let tools = message
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(McpStorageError::probe_failed)?;
    if tools.len() > 500 {
        return Err(McpStorageError::probe_failed());
    }
    let mut names = Vec::with_capacity(tools.len());
    for tool in tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        let name = sanitize_tool_text(name, 160);
        if name.is_empty() {
            continue;
        }
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .map(|description| sanitize_tool_text(description, 320))
            .filter(|description| !description.is_empty());
        names.push(match description {
            Some(description) => format!("{name}: {description}"),
            None => name,
        });
    }
    Ok(names)
}

#[cfg(windows)]
fn sanitize_tool_text(value: &str, maximum_characters: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum_characters)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(windows)]
fn write_request(stdin: &mut std::process::ChildStdin, value: Value) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *stdin, &value)?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

#[cfg(windows)]
fn read_protocol_lines(stdout: ChildStdout, sender: mpsc::SyncSender<Result<String, ()>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut bytes = Vec::new();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) => {
                let _ = sender.send(Err(()));
                return;
            }
            Ok(length) if length <= MAXIMUM_RPC_LINE_BYTES => {
                if sender
                    .send(String::from_utf8(bytes).map_err(|_| ()))
                    .is_err()
                {
                    return;
                }
            }
            Ok(_) | Err(_) => {
                let _ = sender.send(Err(()));
                return;
            }
        }
    }
}

#[cfg(windows)]
fn terminate_process(child: &mut Child, job: Option<ProcessJob>) {
    let _ = child.kill();
    let _ = child.wait();
    drop(job);
}

#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl ProcessJob {
    fn assign(child: &Child) -> Result<Self, ()> {
        use std::{mem::size_of, ptr::null};
        use windows_sys::Win32::{
            Foundation::{CloseHandle, HANDLE},
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
        };

        let job = unsafe { CreateJobObjectW(null(), null()) };
        if job.is_null() {
            return Err(());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        let assigned = configured != 0
            && unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) != 0 };
        if !assigned {
            unsafe { CloseHandle(job) };
            return Err(());
        }
        Ok(Self(job))
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
