//! Discovers tools from installed global MCP servers under bounded AppContainer isolation.

use serde_json::{json, Value};
use tauri::State;
use zeroize::Zeroizing;

#[cfg(windows)]
use openchat_local_service::sandbox::{SandboxJobLimits, SandboxOptions, SandboxedProcess};
#[cfg(windows)]
use std::{
    ffi::OsString,
    fs,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};
#[cfg(windows)]
use tokio::io::AsyncReadExt;
#[cfg(windows)]
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::local_service::LocalServiceState;
use crate::mcp_storage::{
    all_environment_names, mcp_directory, read_secret_map, require_server,
    validate_environment_name, write_server_config, GlobalMcpServer, McpStorageError,
    MAXIMUM_SERVER_ID_BYTES,
};
use crate::request_bounds::BoundedText;

#[cfg(windows)]
const PROBE_TIMEOUT: Duration = Duration::from_secs(12);
#[cfg(windows)]
const PROCESS_STOP_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(windows)]
const MAXIMUM_RPC_LINE_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const MAXIMUM_RPC_TOTAL_BYTES: usize = 8 * 1024 * 1024;
#[cfg(windows)]
const MAXIMUM_RPC_LINES: usize = 1000;
#[cfg(windows)]
const MAXIMUM_ACTIVE_PROBE_PROCESSES: u32 = 32;
#[cfg(windows)]
const MAXIMUM_PROBE_PROCESS_MEMORY_BYTES: usize = 1024 * 1024 * 1024;
#[cfg(windows)]
const MAXIMUM_PROBE_JOB_MEMORY_BYTES: usize = 2 * 1024 * 1024 * 1024;
#[cfg(windows)]
static GLOBAL_MCP_PROBE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(windows)]
#[derive(Default)]
struct RpcOutputBudget {
    bytes_read: usize,
}

#[cfg(windows)]
impl RpcOutputBudget {
    fn record_line(&mut self, line_bytes: usize) -> std::io::Result<()> {
        let bytes_read = self
            .bytes_read
            .checked_add(line_bytes)
            .filter(|bytes_read| *bytes_read <= MAXIMUM_RPC_TOTAL_BYTES)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "The MCP server exceeded the total protocol output limit.",
                )
            })?;
        self.bytes_read = bytes_read;
        Ok(())
    }
}

/// Starts an installed global MCP server and saves its discovered tool summary.
///
/// The configured command runs inside an AppContainer with only outbound Internet
/// capability, bounded process resources, and access to its install and runtime paths.
/// Failures leave the stored tool catalog unchanged.
#[tauri::command]
pub async fn probe_global_mcp_server(
    server_id: BoundedText<MAXIMUM_SERVER_ID_BYTES>,
    state: State<'_, LocalServiceState>,
) -> Result<GlobalMcpServer, McpStorageError> {
    let server_id = server_id.into_inner();
    #[cfg(windows)]
    {
        probe_global_mcp_server_windows(&state, &server_id).await
    }
    #[cfg(not(windows))]
    {
        let _ = server_id;
        Err(McpStorageError::probe_failed())
    }
}

#[cfg(windows)]
async fn probe_global_mcp_server_windows(
    state: &LocalServiceState,
    server_id: &str,
) -> Result<GlobalMcpServer, McpStorageError> {
    let _probe_guard = GLOBAL_MCP_PROBE_LOCK
        .try_lock()
        .map_err(|_| McpStorageError::probe_failed())?;
    let mut server = require_server(state, server_id).await?;
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(root).map_err(|_| McpStorageError::probe_failed())?;
    let install_directory = canonical_install_directory(&canonical_root, &server.id)?;
    server.install_dir = install_directory.to_string_lossy().into_owned();

    let secrets = read_secret_map(state).await?;
    let allowed_names = all_environment_names(&server);
    let mut process_environment = Vec::new();

    for name in &server.required_env_keys {
        let key = secret_key(&server.id, name);
        let Some(value) = secrets.get(&key).filter(|value| !value.trim().is_empty()) else {
            return Err(McpStorageError::missing_credentials());
        };
        process_environment.push((name.clone(), Zeroizing::new(value.to_owned())));
    }
    for name in &server.optional_env_keys {
        if !allowed_names.contains(name.as_str()) {
            return Err(McpStorageError::probe_failed());
        }
        if let Some(value) = secrets.get(&secret_key(&server.id, name)) {
            if !value.trim().is_empty() {
                process_environment.push((name.clone(), Zeroizing::new(value.to_owned())));
            }
        }
    }
    for (name, value) in &process_environment {
        validate_environment_name(name)?;
        if is_reserved_probe_environment_name(name) || value.len() > 2500 || value.contains('\0') {
            return Err(McpStorageError::probe_failed());
        }
    }

    server.discovered_tools = probe_tools(&server, process_environment).await?;
    write_server_config(state, &canonical_root, &server).await?;
    Ok(server)
}

#[cfg(windows)]
fn canonical_install_directory(root: &Path, server_id: &str) -> Result<PathBuf, McpStorageError> {
    let candidate = root.join(server_id);
    let metadata = fs::symlink_metadata(&candidate).map_err(|_| McpStorageError::probe_failed())?;
    if !metadata.is_dir()
        || metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
    {
        return Err(McpStorageError::probe_failed());
    }
    let canonical = fs::canonicalize(candidate).map_err(|_| McpStorageError::probe_failed())?;
    if canonical.parent() != Some(root) {
        return Err(McpStorageError::probe_failed());
    }
    Ok(canonical)
}

#[cfg(windows)]
fn secret_key(server_id: &str, environment_name: &str) -> String {
    format!("openchat_global_mcp_{server_id}_{environment_name}")
}

#[cfg(windows)]
fn is_reserved_probe_environment_name(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
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
            | "HOME"
            | "HOMEDRIVE"
            | "HOMEPATH"
            | "NPM_CONFIG_CACHE"
            | "NPM_CONFIG_USERCONFIG"
            | "NPM_CONFIG_GLOBALCONFIG"
            | "UV_CACHE_DIR"
            | "UV_CONFIG_FILE"
    )
}

#[cfg(windows)]
struct ProbeInvocation {
    program: PathBuf,
    arguments: Vec<OsString>,
    read_only_paths: Vec<PathBuf>,
    search_paths: Vec<PathBuf>,
    system_directory: PathBuf,
}

#[cfg(windows)]
fn resolve_probe_invocation(server: &GlobalMcpServer) -> Result<ProbeInvocation, McpStorageError> {
    let system_directory = system_directory()?;
    let mut read_only_paths = Vec::new();
    let mut search_paths = Vec::new();

    let (program, arguments) = match server.command.as_str() {
        "npx" => {
            let npx = resolve_executable_in_path("npx.cmd")?;
            let node = resolve_sibling_executable_or_path(&npx, "node.exe")?;
            let npm_root = npx
                .parent()
                .ok_or_else(McpStorageError::probe_failed)?
                .join("node_modules")
                .join("npm");
            let npm_root =
                fs::canonicalize(npm_root).map_err(|_| McpStorageError::probe_failed())?;
            let npx_cli = npm_root.join("bin").join("npx-cli.js");
            if !npx_cli.is_file() {
                return Err(McpStorageError::probe_failed());
            }
            read_only_paths.extend([node.clone(), npm_root]);
            add_unique_parent(&mut search_paths, &node)?;
            add_unique_parent(&mut search_paths, &npx)?;
            node_script_invocation(&node, &npx_cli, &server.args)
        }
        "uvx" => {
            let uvx = resolve_executable_in_path("uvx.exe")?;
            let uv = resolve_sibling_executable(&uvx, "uv.exe")?;
            add_unique_parent(&mut search_paths, &uvx)?;
            read_only_paths.extend([uvx.clone(), uv]);
            let arguments = server.args.iter().map(OsString::from).collect();
            (uvx, arguments)
        }
        _ => return Err(McpStorageError::probe_failed()),
    };
    add_unique_path(&mut search_paths, &system_directory);

    Ok(ProbeInvocation {
        program,
        arguments,
        read_only_paths,
        search_paths,
        system_directory,
    })
}

#[cfg(windows)]
fn resolve_sibling_executable(
    executable: &Path,
    sibling_name: &str,
) -> Result<PathBuf, McpStorageError> {
    let sibling = executable
        .parent()
        .ok_or_else(McpStorageError::probe_failed)?
        .join(sibling_name);
    if !sibling.is_file() {
        return Err(McpStorageError::probe_failed());
    }
    fs::canonicalize(sibling).map_err(|_| McpStorageError::probe_failed())
}

#[cfg(windows)]
fn resolve_sibling_executable_or_path(
    executable: &Path,
    sibling_name: &str,
) -> Result<PathBuf, McpStorageError> {
    let sibling = executable
        .parent()
        .ok_or_else(McpStorageError::probe_failed)?
        .join(sibling_name);
    if sibling.is_file() {
        return fs::canonicalize(sibling).map_err(|_| McpStorageError::probe_failed());
    }
    resolve_executable_in_path(sibling_name)
}

#[cfg(windows)]
fn add_unique_parent(paths: &mut Vec<PathBuf>, file: &Path) -> Result<(), McpStorageError> {
    let parent = file.parent().ok_or_else(McpStorageError::probe_failed)?;
    add_unique_path(paths, parent);
    Ok(())
}

#[cfg(windows)]
fn add_unique_path(paths: &mut Vec<PathBuf>, path: &Path) {
    if !paths.iter().any(|existing| existing == path) {
        paths.push(path.to_path_buf());
    }
}

#[cfg(windows)]
fn resolve_executable_in_path(name: &str) -> Result<PathBuf, McpStorageError> {
    let path = std::env::var_os("PATH").ok_or_else(McpStorageError::probe_failed)?;
    for directory in std::env::split_paths(&path).filter(|directory| directory.is_absolute()) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            let resolved =
                fs::canonicalize(candidate).map_err(|_| McpStorageError::probe_failed())?;
            if resolved.is_file() {
                return Ok(resolved);
            }
        }
    }
    Err(McpStorageError::probe_failed())
}

#[cfg(windows)]
fn probe_base_environment(
    install_directory: &Path,
    invocation: &ProbeInvocation,
) -> Result<Vec<(String, Zeroizing<String>)>, McpStorageError> {
    let path = std::env::join_paths(&invocation.search_paths)
        .map_err(|_| McpStorageError::probe_failed())?;
    let path = path.to_str().ok_or_else(McpStorageError::probe_failed)?;
    let install_directory = install_directory
        .to_str()
        .ok_or_else(McpStorageError::probe_failed)?;
    let system_root = invocation
        .system_directory
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(McpStorageError::probe_failed)?;
    let user_profile = Path::new(install_directory).join(".profile");
    let app_data = user_profile.join("Roaming");
    let local_app_data = user_profile.join("Local");
    let npm_cache = Path::new(install_directory).join(".cache").join("npm");
    let uv_cache = Path::new(install_directory).join(".cache").join("uv");
    let npm_user_config = user_profile.join(".openchat-user.npmrc");
    let npm_global_config = user_profile.join(".openchat-global.npmrc");
    let mut environment = Vec::new();
    for (name, value) in [
        ("PATH", path),
        ("SYSTEMROOT", system_root),
        ("WINDIR", system_root),
        ("PATHEXT", ".COM;.EXE;.BAT;.CMD"),
        (
            "USERPROFILE",
            user_profile
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "APPDATA",
            app_data
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "LOCALAPPDATA",
            local_app_data
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "NPM_CONFIG_CACHE",
            npm_cache
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "NPM_CONFIG_USERCONFIG",
            npm_user_config
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "NPM_CONFIG_GLOBALCONFIG",
            npm_global_config
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
        (
            "UV_CACHE_DIR",
            uv_cache
                .to_str()
                .ok_or_else(McpStorageError::probe_failed)?,
        ),
    ] {
        environment.push((name.to_owned(), Zeroizing::new(value.to_owned())));
    }
    Ok(environment)
}

#[cfg(windows)]
async fn probe_tools(
    server: &GlobalMcpServer,
    environment: Vec<(String, Zeroizing<String>)>,
) -> Result<Vec<String>, McpStorageError> {
    let invocation = resolve_probe_invocation(server)?;
    let install_directory = PathBuf::from(&server.install_dir);
    let base_environment = probe_base_environment(&install_directory, &invocation)?;
    let mut process = tokio::task::spawn_blocking(move || {
        let options = SandboxOptions {
            additional_read_only_paths: &invocation.read_only_paths,
            additional_workspace_paths: &[],
            base_environment: Some(&base_environment),
            internet_client: true,
            job_limits: Some(SandboxJobLimits {
                active_process_limit: MAXIMUM_ACTIVE_PROBE_PROCESSES,
                process_memory_limit_bytes: MAXIMUM_PROBE_PROCESS_MEMORY_BYTES,
                job_memory_limit_bytes: MAXIMUM_PROBE_JOB_MEMORY_BYTES,
            }),
        };
        SandboxedProcess::spawn_program_with_options(
            &invocation.program,
            &invocation.arguments,
            &install_directory,
            &environment,
            options,
        )
    })
    .await
    .map_err(|_| McpStorageError::probe_failed())?
    .map_err(|_| McpStorageError::probe_failed())?;

    let Some(stderr) = process.take_stderr() else {
        drop(process);
        return Err(McpStorageError::probe_failed());
    };
    let stderr_task = tokio::spawn(drain_stderr(stderr));
    let protocol_result = tokio::time::timeout(PROBE_TIMEOUT, async {
        let mut stdin = process
            .take_stdin()
            .ok_or_else(McpStorageError::probe_failed)?;
        let stdout = process
            .take_stdout()
            .ok_or_else(McpStorageError::probe_failed)?;
        let mut stdout = BufReader::new(stdout);
        write_request(
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
        .await
        .map_err(|_| McpStorageError::probe_failed())?;
        receive_tool_list(&mut stdin, &mut stdout).await
    })
    .await
    .unwrap_or_else(|_| Err(McpStorageError::probe_failed()));

    let stop_result = process.start_kill();
    let wait_result = tokio::time::timeout(PROCESS_STOP_TIMEOUT, process.wait()).await;
    drop(process);
    let stderr_result = tokio::time::timeout(PROCESS_STOP_TIMEOUT, stderr_task).await;
    if stop_result.is_err()
        || !matches!(wait_result, Ok(Ok(_)))
        || !matches!(stderr_result, Ok(Ok(Ok(()))))
    {
        return Err(McpStorageError::probe_failed());
    }
    protocol_result
}

#[cfg(windows)]
async fn drain_stderr(mut stderr: tokio::fs::File) -> Result<(), std::io::Error> {
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stderr.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
    }
}

#[cfg(windows)]
fn system_directory() -> Result<PathBuf, McpStorageError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buffer = vec![0_u16; 32_768];
    let buffer_length = u32::try_from(buffer.len()).map_err(|_| McpStorageError::probe_failed())?;
    // SAFETY: The writable buffer has `buffer_length` code units and remains alive for the call.
    let path_length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer_length) } as usize;
    if path_length == 0 || path_length >= buffer.len() {
        return Err(McpStorageError::probe_failed());
    }

    let directory = PathBuf::from(OsString::from_wide(&buffer[..path_length]));
    if !directory.is_absolute() {
        return Err(McpStorageError::probe_failed());
    }
    Ok(directory)
}

#[cfg(windows)]
fn node_script_invocation(
    node: &Path,
    script: &Path,
    script_arguments: &[String],
) -> (PathBuf, Vec<OsString>) {
    let mut arguments = Vec::with_capacity(script_arguments.len() + 1);
    arguments.push(script.as_os_str().to_os_string());
    arguments.extend(script_arguments.iter().map(OsString::from));
    (node.to_path_buf(), arguments)
}

#[cfg(windows)]
async fn receive_tool_list<W, R>(
    stdin: &mut W,
    stdout: &mut R,
) -> Result<Vec<String>, McpStorageError>
where
    W: AsyncWrite + Unpin,
    R: AsyncBufRead + Unpin,
{
    let mut line_count = 0;
    let mut requested_tools = false;
    let mut output_budget = RpcOutputBudget::default();

    loop {
        let line = read_bounded_line(stdout, MAXIMUM_RPC_LINE_BYTES)
            .await
            .map_err(|_| McpStorageError::probe_failed())?
            .ok_or_else(McpStorageError::probe_failed)?;
        output_budget
            .record_line(line.len())
            .map_err(|_| McpStorageError::probe_failed())?;
        line_count += 1;
        if line_count > MAXIMUM_RPC_LINES {
            return Err(McpStorageError::probe_failed());
        }
        let line = String::from_utf8(line).map_err(|_| McpStorageError::probe_failed())?;
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
                .await
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
                .await
                .map_err(|_| McpStorageError::probe_failed())?;
                requested_tools = true;
            }
            Some(2) if requested_tools => return parse_tools(&message),
            _ => {}
        }
    }
}

#[cfg(windows)]
async fn write_request<W: AsyncWrite + Unpin>(
    stdin: &mut W,
    value: Value,
) -> Result<(), std::io::Error> {
    let payload = serde_json::to_vec(&value)?;
    stdin.write_all(&payload).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await
}

#[cfg(windows)]
async fn read_bounded_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    maximum_bytes: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf().await?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "The MCP server ended an incomplete protocol line.",
                ))
            };
        }

        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |index| index + 1);
        if line.len().saturating_add(consumed) > maximum_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "The MCP server exceeded the protocol line limit.",
            ));
        }
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(Some(line));
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

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::{ffi::OsString, io::Cursor, path::PathBuf};

    #[cfg(windows)]
    use super::{
        node_script_invocation, probe_base_environment, read_bounded_line, system_directory,
        ProbeInvocation, RpcOutputBudget, GLOBAL_MCP_PROBE_LOCK, MAXIMUM_RPC_LINE_BYTES,
        MAXIMUM_RPC_TOTAL_BYTES,
    };

    #[cfg(windows)]
    #[tokio::test]
    async fn bounded_reader_accepts_limit_and_preserves_following_line() {
        let mut input = vec![b'x'; MAXIMUM_RPC_LINE_BYTES - 1];
        input.extend_from_slice(b"\nnext\n");
        let mut reader = tokio::io::BufReader::new(Cursor::new(input));

        let first = read_bounded_line(&mut reader, MAXIMUM_RPC_LINE_BYTES)
            .await
            .expect("maximum-sized line should be accepted")
            .expect("first line should exist");
        assert_eq!(first.len(), MAXIMUM_RPC_LINE_BYTES);
        assert_eq!(
            read_bounded_line(&mut reader, MAXIMUM_RPC_LINE_BYTES)
                .await
                .expect("following line should remain readable"),
            Some(b"next\n".to_vec())
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn bounded_reader_rejects_oversized_and_incomplete_lines() {
        let mut oversized =
            tokio::io::BufReader::new(Cursor::new(vec![b'x'; MAXIMUM_RPC_LINE_BYTES + 1]));
        assert_eq!(
            read_bounded_line(&mut oversized, MAXIMUM_RPC_LINE_BYTES)
                .await
                .expect_err("oversized line must fail")
                .kind(),
            std::io::ErrorKind::InvalidData
        );

        let mut incomplete = tokio::io::BufReader::new(Cursor::new(b"partial".to_vec()));
        assert_eq!(
            read_bounded_line(&mut incomplete, 64)
                .await
                .expect_err("unterminated line must fail")
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[cfg(windows)]
    #[test]
    fn protocol_output_budget_rejects_bytes_beyond_total_limit() {
        let mut budget = RpcOutputBudget::default();

        budget
            .record_line(MAXIMUM_RPC_TOTAL_BYTES - 1)
            .expect("output below the total limit should be accepted");
        budget
            .record_line(1)
            .expect("output exactly at the total limit should be accepted");
        assert_eq!(
            budget
                .record_line(1)
                .expect_err("output beyond the total limit must fail")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[cfg(windows)]
    #[test]
    fn protocol_output_budget_rejects_counter_overflow() {
        let mut budget = RpcOutputBudget::default();

        assert_eq!(
            budget
                .record_line(usize::MAX)
                .expect_err("an unrepresentable or over-limit output count must fail")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[cfg(windows)]
    #[test]
    fn probe_uses_windows_system_directory() {
        let path = system_directory().expect("Windows should provide its system directory");

        assert!(path.is_absolute());
        assert!(path.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn npx_invocation_uses_node_and_keeps_arguments_separate() {
        let node = PathBuf::from(r"C:\Program Files\nodejs\node.exe");
        let script = PathBuf::from(r"C:\Program Files\nodejs\node_modules\npm\bin\npx-cli.js");
        let supplied = vec!["-y".to_owned(), "@scope/mcp-server".to_owned()];

        let (program, arguments) = node_script_invocation(&node, &script, &supplied);

        assert_eq!(program, node);
        assert_eq!(
            arguments,
            [
                OsString::from(script.as_os_str()),
                OsString::from("-y"),
                OsString::from("@scope/mcp-server"),
            ]
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn only_one_global_probe_can_run_at_a_time() {
        let _guard = GLOBAL_MCP_PROBE_LOCK
            .try_lock()
            .expect("first global probe should acquire the slot");

        assert!(GLOBAL_MCP_PROBE_LOCK.try_lock().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn probe_base_environment_uses_only_sandbox_paths() {
        let install_directory = PathBuf::from(r"C:\OpenChat\mcp\server");
        let invocation = ProbeInvocation {
            program: PathBuf::from(r"C:\Program Files\nodejs\node.exe"),
            arguments: vec![OsString::from(
                r"C:\Program Files\nodejs\node_modules\npm\bin\npx-cli.js",
            )],
            read_only_paths: Vec::new(),
            search_paths: vec![PathBuf::from(r"C:\Program Files\nodejs")],
            system_directory: PathBuf::from(r"C:\Windows\System32"),
        };

        let environment = probe_base_environment(&install_directory, &invocation)
            .expect("sandbox environment should be valid");
        let names: Vec<_> = environment.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "PATH",
                "SYSTEMROOT",
                "WINDIR",
                "PATHEXT",
                "USERPROFILE",
                "APPDATA",
                "LOCALAPPDATA",
                "NPM_CONFIG_CACHE",
                "NPM_CONFIG_USERCONFIG",
                "NPM_CONFIG_GLOBALCONFIG",
                "UV_CACHE_DIR",
            ]
        );
        assert!(environment
            .iter()
            .all(|(_, value)| !value.contains("OPENAI_API_KEY")));
    }

    #[cfg(windows)]
    #[test]
    fn probe_environment_variables_cannot_replace_sandbox_controls() {
        for name in ["PATH", "npm_config_cache", "UV_CONFIG_FILE", "HOME"] {
            assert!(super::is_reserved_probe_environment_name(name));
        }
        assert!(!super::is_reserved_probe_environment_name("OPENCHAT_TOKEN"));
    }
}
