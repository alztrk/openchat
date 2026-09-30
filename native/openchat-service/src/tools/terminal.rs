use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    sync::Mutex,
    time::sleep,
};

const DEFAULT_TIMEOUT_SECONDS: u64 = 180;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_WAIT_MS: u64 = 2000;
const MAX_WAIT_MS: u64 = 30000;
const QUIET_PERIOD_MS: u64 = 50;
const MAX_BUFFER_BYTES: usize = 128 * 1024;

pub struct TerminalSession {
    pub id: String,
    #[allow(dead_code)]
    pub command_str: String,
    #[allow(dead_code)]
    pub workdir: PathBuf,
    pub created_at: Instant,
    pub timeout: Duration,
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    output_buffer: Arc<Mutex<Vec<u8>>>,
    last_activity: Arc<Mutex<Instant>>,
}

#[cfg(windows)]
fn build_shell_command(command_str: &str) -> Command {
    if Path::new(r"C:\Program Files\PowerShell\7\pwsh.exe").exists() {
        let mut cmd = Command::new(r"C:\Program Files\PowerShell\7\pwsh.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", command_str]);
        return cmd;
    }
    if Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe").exists() {
        let mut cmd = Command::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", command_str]);
        return cmd;
    }
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/C", command_str]);
    cmd
}

#[cfg(not(windows))]
fn build_shell_command(command_str: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command_str]);
    cmd
}

impl TerminalSession {
    pub async fn is_running(&self) -> bool {
        let mut child = self.child.lock().await;
        matches!(child.try_wait(), Ok(None))
    }

    pub async fn exit_code(&self) -> Option<i32> {
        let mut child = self.child.lock().await;
        match child.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        }
    }

    pub async fn send_input(&self, input: &str) -> Result<(), String> {
        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| "Terminal stdin is closed or unavailable.".to_owned())?;
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to terminal stdin: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush terminal stdin: {e}"))?;
        *self.last_activity.lock().await = Instant::now();
        Ok(())
    }

    pub async fn kill(&self) {
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
    }

    pub async fn read_output_from(&self, start_offset: usize) -> (String, usize, bool) {
        let buffer = self.output_buffer.lock().await;
        let total_len = buffer.len();
        let slice = if start_offset < total_len {
            &buffer[start_offset..]
        } else {
            &[]
        };
        let text = String::from_utf8_lossy(slice).into_owned();
        let truncated = total_len >= MAX_BUFFER_BYTES;
        (text, total_len, truncated)
    }

    pub async fn wait_for_output(
        &self,
        start_offset: usize,
        wait_duration: Duration,
    ) -> (String, usize, bool) {
        let deadline = Instant::now() + wait_duration;
        let mut last_len = { self.output_buffer.lock().await.len() };
        let mut quiet_start: Option<Instant> = None;

        while Instant::now() < deadline {
            sleep(Duration::from_millis(25)).await;

            let current_len = { self.output_buffer.lock().await.len() };
            let running = self.is_running().await;

            if !running {
                // Process finished, give it one quick moment to collect remaining output
                sleep(Duration::from_millis(30)).await;
                break;
            }

            if current_len > last_len {
                last_len = current_len;
                quiet_start = Some(Instant::now());
            } else if let Some(q_start) = quiet_start
                && q_start.elapsed() >= Duration::from_millis(QUIET_PERIOD_MS)
            {
                // Output has stabilized for QUIET_PERIOD_MS
                break;
            }
        }

        self.read_output_from(start_offset).await
    }
}

pub struct TerminalSessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalSession>>>>,
    counter: AtomicU64,
}

static MANAGER: OnceLock<TerminalSessionManager> = OnceLock::new();

impl TerminalSessionManager {
    pub fn global() -> &'static Self {
        MANAGER.get_or_init(|| {
            let mgr = Self {
                sessions: Arc::new(Mutex::new(HashMap::new())),
                counter: AtomicU64::new(1),
            };
            mgr.start_reaper();
            mgr
        })
    }

    fn start_reaper(&self) {
        let sessions = Arc::clone(&self.sessions);
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await;
                let mut guard = sessions.lock().await;
                let mut to_remove = Vec::new();

                for (id, session) in guard.iter() {
                    let running = session.is_running().await;
                    let expired = session.created_at.elapsed() > session.timeout;
                    let idle = {
                        let last = *session.last_activity.lock().await;
                        last.elapsed() > Duration::from_secs(120)
                    };

                    if !running || expired || idle {
                        if running && (expired || idle) {
                            session.kill().await;
                        }
                        to_remove.push(id.clone());
                    }
                }

                for id in to_remove {
                    guard.remove(&id);
                }
            }
        });
    }

    pub async fn create_session(
        &self,
        command_str: &str,
        workdir: &Path,
        timeout_seconds: Option<u64>,
    ) -> Result<Arc<TerminalSession>, String> {
        let id_num = self.counter.fetch_add(1, Ordering::Relaxed);
        let id = format!("term_{id_num}");

        let timeout_secs = timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            .clamp(5, MAX_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(timeout_secs);

        let mut cmd = build_shell_command(command_str);

        cmd.current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn command process: {e}"))?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let last_activity = Arc::new(Mutex::new(Instant::now()));

        // Spawn async reader for stdout
        if let Some(mut stdout) = stdout {
            let buf_clone = Arc::clone(&output_buffer);
            let act_clone = Arc::clone(&last_activity);
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                loop {
                    match stdout.read(&mut chunk).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut b = buf_clone.lock().await;
                            if b.len() + n <= MAX_BUFFER_BYTES {
                                b.extend_from_slice(&chunk[..n]);
                            } else {
                                let remaining = MAX_BUFFER_BYTES.saturating_sub(b.len());
                                if remaining > 0 {
                                    b.extend_from_slice(&chunk[..remaining]);
                                }
                            }
                            *act_clone.lock().await = Instant::now();
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Spawn async reader for stderr
        if let Some(mut stderr) = stderr {
            let buf_clone = Arc::clone(&output_buffer);
            let act_clone = Arc::clone(&last_activity);
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                loop {
                    match stderr.read(&mut chunk).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut b = buf_clone.lock().await;
                            if b.len() + n <= MAX_BUFFER_BYTES {
                                b.extend_from_slice(&chunk[..n]);
                            } else {
                                let remaining = MAX_BUFFER_BYTES.saturating_sub(b.len());
                                if remaining > 0 {
                                    b.extend_from_slice(&chunk[..remaining]);
                                }
                            }
                            *act_clone.lock().await = Instant::now();
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let session = Arc::new(TerminalSession {
            id: id.clone(),
            command_str: command_str.to_owned(),
            workdir: workdir.to_path_buf(),
            created_at: Instant::now(),
            timeout,
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            output_buffer,
            last_activity,
        });

        self.sessions.lock().await.insert(id, Arc::clone(&session));
        Ok(session)
    }

    pub async fn get_session(&self, id: &str) -> Option<Arc<TerminalSession>> {
        self.sessions.lock().await.get(id).cloned()
    }

    pub async fn remove_session(&self, id: &str) -> Option<Arc<TerminalSession>> {
        self.sessions.lock().await.remove(id)
    }

    pub async fn execute(
        &self,
        command_str: &str,
        workdir: &Path,
        timeout_seconds: Option<u64>,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let wait_duration =
            Duration::from_millis(wait_ms.unwrap_or(DEFAULT_WAIT_MS).clamp(100, MAX_WAIT_MS));

        let session = self
            .create_session(command_str, workdir, timeout_seconds)
            .await?;
        let (output, _offset, truncated) = session.wait_for_output(0, wait_duration).await;
        let running = session.is_running().await;
        let exit_code = session.exit_code().await;

        if !running {
            // Process completed within wait window; clean up session
            self.remove_session(&session.id).await;
            Ok(json!({
                "command": command_str,
                "is_running": false,
                "exit_code": exit_code.unwrap_or(0),
                "output": output,
                "truncated": truncated
            }))
        } else {
            // Process is still running (e.g. interactive prompt, server, long build)
            Ok(json!({
                "command": command_str,
                "terminal_id": session.id,
                "is_running": true,
                "waiting_for_input": true,
                "exit_code": Value::Null,
                "output": output,
                "truncated": truncated,
                "message": "The command is still running and may be waiting for input. Use send_terminal_input with this terminal_id to send input or read more output."
            }))
        }
    }

    pub async fn send_input_to(
        &self,
        terminal_id: &str,
        input: &str,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let session = self.get_session(terminal_id).await.ok_or_else(|| {
            format!("Terminal session '{terminal_id}' not found or already terminated.")
        })?;

        let start_offset = { session.output_buffer.lock().await.len() };
        session.send_input(input).await?;

        let wait_duration =
            Duration::from_millis(wait_ms.unwrap_or(DEFAULT_WAIT_MS).clamp(100, MAX_WAIT_MS));
        let (output, _offset, truncated) =
            session.wait_for_output(start_offset, wait_duration).await;
        let running = session.is_running().await;
        let exit_code = session.exit_code().await;

        if !running {
            self.remove_session(terminal_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "is_running": false,
                "exit_code": exit_code.unwrap_or(0),
                "output": output,
                "truncated": truncated
            }))
        } else {
            Ok(json!({
                "terminal_id": terminal_id,
                "is_running": true,
                "waiting_for_input": true,
                "exit_code": Value::Null,
                "output": output,
                "truncated": truncated
            }))
        }
    }

    pub async fn read_output_of(
        &self,
        terminal_id: &str,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let session = self.get_session(terminal_id).await.ok_or_else(|| {
            format!("Terminal session '{terminal_id}' not found or already terminated.")
        })?;

        let wait_duration = Duration::from_millis(wait_ms.unwrap_or(500).clamp(50, MAX_WAIT_MS));
        let (output, _offset, truncated) = session.wait_for_output(0, wait_duration).await;
        let running = session.is_running().await;
        let exit_code = session.exit_code().await;

        if !running {
            self.remove_session(terminal_id).await;
        }

        Ok(json!({
            "terminal_id": terminal_id,
            "is_running": running,
            "exit_code": exit_code,
            "output": output,
            "truncated": truncated
        }))
    }

    pub async fn kill_session(&self, terminal_id: &str) -> Result<Value, String> {
        if let Some(session) = self.remove_session(terminal_id).await {
            session.kill().await;
            Ok(json!({
                "terminal_id": terminal_id,
                "status": "terminated",
                "message": "Terminal session was killed successfully."
            }))
        } else {
            Err(format!("Terminal session '{terminal_id}' was not found."))
        }
    }
}
