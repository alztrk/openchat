//! Manages bounded interactive process sessions used by model-requested tools.
//!
//! Interactive session identifiers are random, and every session remains bound to the
//! conversation that created it so one conversation cannot control another's process.
use std::{
    collections::HashMap,
    path::Path,
    process::ExitStatus,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
#[cfg(not(windows))]
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde_json::{Value, json};
#[cfg(not(windows))]
use tokio::process::{ChildStdin, Command};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    time::{sleep, sleep_until},
};
use uuid::Uuid;

#[cfg(windows)]
#[path = "terminal_windows.rs"]
pub(crate) mod windows_sandbox;
#[cfg(windows)]
use windows_sandbox::{SandboxJobLimits, SandboxOptions, SandboxedProcess};

const DEFAULT_TIMEOUT_SECONDS: u64 = 180;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_WAIT_MS: u64 = 2000;
const MAX_WAIT_MS: u64 = 30000;
const QUIET_PERIOD_MS: u64 = 50;
const MAX_BUFFER_BYTES: usize = 128 * 1024;
const MAX_ACTIVE_TERMINAL_SESSIONS: usize = 16;
const INTERNAL_TERMINAL_OWNER: &str = "openchat-internal";

#[cfg(windows)]
fn command_sandbox_options() -> Result<SandboxOptions<'static>, String> {
    let job_limits = SandboxJobLimits::for_current_host().map_err(|_| {
        "Windows could not determine safe resource limits for this command.".to_owned()
    })?;

    Ok(SandboxOptions {
        job_limits: Some(job_limits),
        ..SandboxOptions::default()
    })
}

fn terminal_session_unavailable() -> String {
    "Terminal session was not found or is not available to this conversation.".to_owned()
}

/// A running command process with bounded, asynchronously captured output.
pub struct TerminalSession {
    /// Random identifier returned to the owning conversation while the process is running.
    pub id: String,
    owner_conversation_id: String,
    /// Monotonic creation time used by the session reaper.
    pub created_at: Instant,
    /// Maximum allowed process lifetime.
    pub timeout: Duration,
    idle_timeout: Option<Duration>,
    #[cfg(windows)]
    child: Arc<Mutex<SandboxedProcess>>,
    #[cfg(not(windows))]
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    stdin: Arc<Mutex<Option<TerminalStdin>>>,
    output_buffer: Arc<Mutex<Vec<u8>>>,
    last_activity: Arc<Mutex<Instant>>,
    _session_slot: OwnedSemaphorePermit,
}

#[cfg(not(windows))]
fn build_shell_command(command_str: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command_str]);
    cmd
}

enum TerminalStdin {
    #[cfg(windows)]
    Sandboxed(tokio::fs::File),
    #[cfg(not(windows))]
    Child(ChildStdin),
}

impl TerminalSession {
    async fn process_status(&self) -> Result<Option<ExitStatus>, String> {
        let child = self.child.lock().await;
        #[cfg(windows)]
        {
            child.try_wait().map_err(|error| {
                format!("Could not check the sandboxed command process status: {error}")
            })
        }
        #[cfg(not(windows))]
        let mut child = child;
        #[cfg(not(windows))]
        child
            .try_wait()
            .map_err(|error| format!("Could not check the command process status: {error}"))
    }

    /// Reports whether the process remains active.
    pub async fn is_running(&self) -> Result<bool, String> {
        self.process_status().await.map(|status| status.is_none())
    }

    /// Writes input to the process stdin and updates its idle timer.
    pub async fn send_input(&self, input: &str) -> Result<(), String> {
        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| "Terminal stdin is closed or unavailable.".to_owned())?;
        #[cfg(windows)]
        let TerminalStdin::Sandboxed(stdin) = stdin;
        #[cfg(not(windows))]
        let TerminalStdin::Child(stdin) = stdin;
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

    /// Stops the process and waits for its process tree to exit where supported.
    pub async fn kill(&self) -> Result<(), String> {
        let child = self.child.lock().await;
        #[cfg(windows)]
        {
            child.start_kill().map_err(|error| {
                format!("Could not stop the sandboxed command process: {error}")
            })?;
            child.wait_for_job_exit().map_err(|error| {
                format!("Could not stop the sandboxed command process tree: {error}")
            })?;
            return child
                .wait()
                .await
                .map(|_| ())
                .map_err(|error| format!("Could not stop the sandboxed command process: {error}"));
        }
        #[cfg(not(windows))]
        {
            let mut child = child;
            if let Err(error) = child.start_kill() {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return Ok(());
                }
                return Err(format!("Could not stop the command process: {error}"));
            }
            child
                .wait()
                .await
                .map(|_| ())
                .map_err(|error| format!("Could not stop the command process: {error}"))
        }
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
    ) -> Result<(String, usize, bool), String> {
        let deadline = Instant::now() + wait_duration;
        let mut last_len = { self.output_buffer.lock().await.len() };
        let mut quiet_start: Option<Instant> = None;

        while Instant::now() < deadline {
            sleep(Duration::from_millis(25)).await;

            let current_len = { self.output_buffer.lock().await.len() };
            let running = self.is_running().await?;

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

        Ok(self.read_output_from(start_offset).await)
    }
}

/// Creates and controls process sessions while enforcing conversation ownership.
pub struct TerminalSessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalSession>>>>,
    session_slots: Arc<Semaphore>,
}

static MANAGER: OnceLock<TerminalSessionManager> = OnceLock::new();

impl TerminalSessionManager {
    /// Returns the service-wide manager and starts its session reaper once.
    pub fn global() -> &'static Self {
        MANAGER.get_or_init(Self::new)
    }

    fn new() -> Self {
        Self::with_session_limit(MAX_ACTIVE_TERMINAL_SESSIONS)
    }

    fn with_session_limit(session_limit: usize) -> Self {
        let manager = Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            session_slots: Arc::new(Semaphore::new(session_limit)),
        };
        manager.start_reaper();
        manager
    }

    #[cfg(test)]
    pub fn isolated() -> Self {
        Self::new()
    }

    fn start_reaper(&self) {
        let sessions = Arc::clone(&self.sessions);
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await;
                let mut guard = sessions.lock().await;
                let mut to_remove = Vec::new();

                for (id, session) in guard.iter() {
                    let expired = session.created_at.elapsed() > session.timeout;
                    let idle = session
                        .idle_timeout
                        .is_some_and(|idle_timeout| session.created_at.elapsed() > idle_timeout)
                        && {
                            let last = *session.last_activity.lock().await;
                            last.elapsed() > Duration::from_secs(120)
                        };

                    match session.is_running().await {
                        Ok(false) => to_remove.push(id.clone()),
                        Ok(true) if expired || idle => {
                            if session.kill().await.is_err() {
                                eprintln!("terminal_session_cleanup_failed");
                            }
                            to_remove.push(id.clone());
                        }
                        Ok(true) => {}
                        Err(_) => {
                            eprintln!("terminal_process_state_unavailable");
                            if session.kill().await.is_err() {
                                eprintln!("terminal_session_cleanup_failed");
                            }
                            to_remove.push(id.clone());
                        }
                    }
                }

                for id in to_remove {
                    guard.remove(&id);
                }
            }
        });
    }

    /// Starts a session owned by one conversation.
    ///
    /// The owner identifier is supplied by trusted execution context, not tool arguments.
    /// The returned session ID is random and required for later operations in that conversation.
    pub async fn create_session(
        &self,
        command_str: &str,
        workdir: &Path,
        owner_conversation_id: &str,
        timeout_seconds: Option<u64>,
    ) -> Result<Arc<TerminalSession>, String> {
        self.create_session_with_idle_timeout(
            command_str,
            workdir,
            owner_conversation_id,
            timeout_seconds,
            Some(Duration::from_secs(120)),
        )
        .await
    }

    async fn create_session_with_idle_timeout(
        &self,
        command_str: &str,
        workdir: &Path,
        owner_conversation_id: &str,
        timeout_seconds: Option<u64>,
        idle_timeout: Option<Duration>,
    ) -> Result<Arc<TerminalSession>, String> {
        let session_slot = Arc::clone(&self.session_slots)
            .try_acquire_owned()
            .map_err(|_| {
                "The maximum number of active terminal sessions has been reached. Stop an existing session before starting another."
                    .to_owned()
            })?;
        let id = format!("term_{}", Uuid::new_v4().simple());

        let timeout_secs = timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            .clamp(5, MAX_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(timeout_secs);

        #[cfg(windows)]
        let (child, stdin, stdout, stderr) = {
            let options = command_sandbox_options()?;
            let mut child = SandboxedProcess::spawn_with_options(command_str, workdir, options).map_err(|_| {
                "The command could not start inside the Windows AppContainer sandbox. Check the project folder and Windows security settings.".to_owned()
            })?;
            let stdin = child.take_stdin().map(TerminalStdin::Sandboxed);
            let stdout = child.take_stdout();
            let stderr = child.take_stderr();
            (child, stdin, stdout, stderr)
        };

        #[cfg(not(windows))]
        let (child, stdin, stdout, stderr) = {
            let mut cmd = build_shell_command(command_str);
            cmd.current_dir(workdir)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            let mut wrapped_command = CommandWrap::from(cmd);
            wrapped_command.wrap(KillOnDrop);
            #[cfg(unix)]
            wrapped_command.wrap(ProcessGroup::leader());

            let mut child = wrapped_command
                .spawn()
                .map_err(|error| format!("Failed to spawn command process: {error}"))?;
            let stdin = child.stdin().take().map(TerminalStdin::Child);
            let stdout = child.stdout().take();
            let stderr = child.stderr().take();
            (child, stdin, stdout, stderr)
        };

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
            owner_conversation_id: owner_conversation_id.to_owned(),
            created_at: Instant::now(),
            timeout,
            idle_timeout,
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            output_buffer,
            last_activity,
            _session_slot: session_slot,
        });

        self.sessions.lock().await.insert(id, Arc::clone(&session));
        Ok(session)
    }

    /// Returns a session only when its owner matches the supplied conversation identifier.
    pub async fn get_session(
        &self,
        id: &str,
        owner_conversation_id: &str,
    ) -> Option<Arc<TerminalSession>> {
        self.sessions
            .lock()
            .await
            .get(id)
            .filter(|session| session.owner_conversation_id == owner_conversation_id)
            .cloned()
    }

    async fn remove_session(&self, id: &str) -> Option<Arc<TerminalSession>> {
        self.sessions.lock().await.remove(id)
    }

    /// Runs a command and returns its completed output or an owner-bound terminal ID.
    pub async fn execute(
        &self,
        command_str: &str,
        workdir: &Path,
        owner_conversation_id: &str,
        timeout_seconds: Option<u64>,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let wait_duration =
            Duration::from_millis(wait_ms.unwrap_or(DEFAULT_WAIT_MS).clamp(100, MAX_WAIT_MS));

        let session = self
            .create_session(command_str, workdir, owner_conversation_id, timeout_seconds)
            .await?;
        let (output, _offset, truncated) = session.wait_for_output(0, wait_duration).await?;
        let status = session.process_status().await?;

        if let Some(status) = status {
            // Process completed within wait window; clean up session
            self.remove_session(&session.id).await;
            Ok(json!({
                "command": command_str,
                "is_running": false,
                "exit_code": status.code(),
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

    /// Runs a bounded non-interactive command until completion, cancellation, or timeout.
    pub async fn execute_to_completion(
        &self,
        command_str: &str,
        workdir: &Path,
        timeout_seconds: u64,
        cancellation: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<Value, String> {
        if !(5..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
            return Err("The named task timeout is outside the supported range.".to_owned());
        }
        if *cancellation.borrow() {
            return Ok(
                json!({"status": "cancelled", "exitCode": Value::Null, "output": "", "truncated": false}),
            );
        }

        let session = self
            .create_session_with_idle_timeout(
                command_str,
                workdir,
                INTERNAL_TERMINAL_OWNER,
                Some(timeout_seconds),
                None,
            )
            .await?;
        session.stdin.lock().await.take();
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);

        loop {
            tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_err() || *cancellation.borrow() {
                        session.kill().await?;
                        self.remove_session(&session.id).await;
                        let (output, _, truncated) = session.read_output_from(0).await;
                        return Ok(json!({
                            "status": "cancelled",
                            "exitCode": Value::Null,
                            "output": output,
                            "truncated": truncated,
                        }));
                    }
                }
                _ = sleep(Duration::from_millis(50)) => {
                    match session.process_status().await {
                        Ok(Some(status)) => {
                            self.remove_session(&session.id).await;
                            let (output, _, truncated) = session.read_output_from(0).await;
                            return Ok(json!({
                                "status": "completed",
                                "exitCode": status.code(),
                                "output": output,
                                "truncated": truncated,
                            }));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            session.kill().await?;
                            self.remove_session(&session.id).await;
                            return Err(error);
                        }
                    }

                    if Instant::now() >= deadline {
                        session.kill().await?;
                        self.remove_session(&session.id).await;
                        let (output, _, truncated) = session.read_output_from(0).await;
                        return Ok(json!({
                            "status": "timed_out",
                            "exitCode": Value::Null,
                            "output": output,
                            "truncated": truncated,
                        }));
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    pub async fn execute_project_tool(
        &self,
        program: &Path,
        workdir: &Path,
        input: &str,
        timeout_seconds: u64,
        output_limit_bytes: usize,
        cancellation: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<Value, String> {
        if !(5..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds)
            || !(1..=MAX_BUFFER_BYTES).contains(&output_limit_bytes)
            || input.len() > 16 * 1024
        {
            return Err("The configured project tool is outside its execution limits.".to_owned());
        }
        if *cancellation.borrow() {
            return Ok(
                json!({"status":"cancelled","exit_code":Value::Null,"output":"","truncated":false}),
            );
        }

        let options = command_sandbox_options()?;
        let mut process =
            SandboxedProcess::spawn_program_with_options(program, &[], workdir, &[], options)
                .map_err(|_| {
                    "The configured tool could not start inside the Windows AppContainer."
                        .to_owned()
                })?;
        let capture = Arc::new(Mutex::new(ProjectToolOutput::new(output_limit_bytes)));
        let stdout = process
            .take_stdout()
            .ok_or_else(|| "The configured tool output could not be captured.".to_owned())?;
        let stderr = process
            .take_stderr()
            .ok_or_else(|| "The configured tool errors could not be captured.".to_owned())?;
        let stdout_task =
            tokio::spawn(drain_project_tool_pipe(stdout, Arc::clone(&capture), false));
        let stderr_task = tokio::spawn(drain_project_tool_pipe(stderr, Arc::clone(&capture), true));
        let mut stdin = process
            .take_stdin()
            .ok_or_else(|| "The configured tool input could not be sent.".to_owned())?;
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|_| "The configured tool input could not be sent.".to_owned())?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|_| "The configured tool input could not be sent.".to_owned())?;
        stdin
            .flush()
            .await
            .map_err(|_| "The configured tool input could not be sent.".to_owned())?;
        drop(stdin);

        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        let (status, result_status) = loop {
            tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_err() || *cancellation.borrow() {
                        process.start_kill().map_err(|_| "The cancelled project tool could not be stopped.".to_owned())?;
                        process.wait_for_job_exit().map_err(|_| "The cancelled project tool process tree did not stop in time.".to_owned())?;
                        let _ = process.wait().await;
                        break (None, "cancelled");
                    }
                }
                result = process.wait() => {
                    let status = result.map_err(|_| "The configured project tool process status could not be read.".to_owned())?;
                    // A one-shot project tool must not leave descendants holding its output pipes open.
                    process.start_kill().map_err(|_| "The completed project tool process tree could not be stopped.".to_owned())?;
                    process.wait_for_job_exit().map_err(|_| "The completed project tool process tree did not stop in time.".to_owned())?;
                    break (Some(status.code()), "completed");
                }
                _ = sleep_until(deadline.into()) => {
                    process.start_kill().map_err(|_| "The timed-out project tool could not be stopped.".to_owned())?;
                    process.wait_for_job_exit().map_err(|_| "The timed-out project tool process tree did not stop in time.".to_owned())?;
                    let _ = process.wait().await;
                    break (None, "timed_out");
                }
            }
        };
        stdout_task
            .await
            .map_err(|_| "The configured tool output reader stopped unexpectedly.".to_owned())?
            .map_err(|_| "The configured tool output could not be read.".to_owned())?;
        stderr_task
            .await
            .map_err(|_| "The configured tool error reader stopped unexpectedly.".to_owned())?
            .map_err(|_| "The configured tool errors could not be read.".to_owned())?;
        let capture = capture.lock().await;
        let mut output = String::from_utf8_lossy(&capture.stdout).into_owned();
        if !capture.stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("[stderr]\n");
            output.push_str(&String::from_utf8_lossy(&capture.stderr));
        }
        Ok(json!({
            "status": result_status,
            "exit_code": status,
            "output": output,
            "truncated": capture.truncated,
        }))
    }

    /// Sends stdin only to a running session owned by the supplied conversation.
    ///
    /// # Errors
    ///
    /// Returns an unavailable-session error for missing or foreign session IDs, and an I/O
    /// error when the owning process no longer accepts input.
    pub async fn send_input_to(
        &self,
        terminal_id: &str,
        owner_conversation_id: &str,
        input: &str,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let session = self
            .get_session(terminal_id, owner_conversation_id)
            .await
            .ok_or_else(terminal_session_unavailable)?;

        let start_offset = { session.output_buffer.lock().await.len() };
        session.send_input(input).await?;

        let wait_duration =
            Duration::from_millis(wait_ms.unwrap_or(DEFAULT_WAIT_MS).clamp(100, MAX_WAIT_MS));
        let (output, _offset, truncated) =
            session.wait_for_output(start_offset, wait_duration).await?;
        let status = session.process_status().await?;

        if let Some(status) = status {
            self.remove_session(terminal_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "is_running": false,
                "exit_code": status.code(),
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

    /// Reads buffered output only from a session owned by the supplied conversation.
    pub async fn read_output_of(
        &self,
        terminal_id: &str,
        owner_conversation_id: &str,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let session = self
            .get_session(terminal_id, owner_conversation_id)
            .await
            .ok_or_else(terminal_session_unavailable)?;

        let wait_duration = Duration::from_millis(wait_ms.unwrap_or(500).clamp(50, MAX_WAIT_MS));
        let (output, _offset, truncated) = session.wait_for_output(0, wait_duration).await?;
        let status = session.process_status().await?;

        if status.is_some() {
            self.remove_session(terminal_id).await;
        }

        Ok(json!({
            "terminal_id": terminal_id,
            "is_running": status.is_none(),
            "exit_code": status.and_then(|status| status.code()),
            "output": output,
            "truncated": truncated
        }))
    }

    /// Terminates a session only when it belongs to the supplied conversation.
    pub async fn kill_session(
        &self,
        terminal_id: &str,
        owner_conversation_id: &str,
    ) -> Result<Value, String> {
        if let Some(session) = self.get_session(terminal_id, owner_conversation_id).await {
            session.kill().await?;
            self.remove_session(terminal_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "status": "terminated",
                "message": "Terminal session was killed successfully."
            }))
        } else {
            Err(terminal_session_unavailable())
        }
    }

    pub async fn stop_all(&self) -> Result<(), String> {
        let sessions = std::mem::take(&mut *self.sessions.lock().await);
        let mut first_error = None;

        for session in sessions.into_values() {
            match session.is_running().await {
                Ok(true) => {
                    if let Err(error) = session.kill().await
                        && first_error.is_none()
                    {
                        first_error = Some(error);
                    }
                }
                Ok(false) => {}
                Err(_) => {
                    if let Err(error) = session.kill().await {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                        eprintln!("terminal_session_cleanup_failed");
                    }
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub async fn stop_all_global() -> Result<(), String> {
        match MANAGER.get() {
            Some(manager) => manager.stop_all().await,
            None => Ok(()),
        }
    }
}

#[cfg(windows)]
struct ProjectToolOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    used: usize,
    limit: usize,
    truncated: bool,
}

#[cfg(windows)]
impl ProjectToolOutput {
    fn new(limit: usize) -> Self {
        Self {
            stdout: Vec::new(),
            stderr: Vec::new(),
            used: 0,
            limit,
            truncated: false,
        }
    }
}

#[cfg(windows)]
async fn drain_project_tool_pipe(
    mut reader: tokio::fs::File,
    capture: Arc<Mutex<ProjectToolOutput>>,
    is_stderr: bool,
) -> std::io::Result<()> {
    let mut chunk = [0_u8; 4096];
    loop {
        let bytes_read = reader.read(&mut chunk).await?;
        if bytes_read == 0 {
            return Ok(());
        }
        let mut capture = capture.lock().await;
        let keep = bytes_read.min(capture.limit.saturating_sub(capture.used));
        if is_stderr {
            capture.stderr.extend_from_slice(&chunk[..keep]);
        } else {
            capture.stdout.extend_from_slice(&chunk[..keep]);
        }
        capture.used = capture.used.saturating_add(keep);
        capture.truncated |= keep < bytes_read;
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalSessionManager;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[tokio::test]
    async fn active_session_limit_rejects_new_processes_and_releases_capacity() {
        let workdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "openchat-terminal-session-limit-{}",
                Uuid::new_v4().simple()
            ));
        std::fs::create_dir_all(&workdir).expect("create an isolated terminal test folder");

        let manager = TerminalSessionManager::with_session_limit(1);
        let invalid_workdir = workdir.join("missing");
        assert!(
            manager
                .create_session(
                    "echo failed",
                    &invalid_workdir,
                    "terminal-limit-test",
                    Some(10),
                )
                .await
                .is_err(),
            "a process start failure should return an error"
        );

        let first = manager
            .create_session("echo first", &workdir, "terminal-limit-test", Some(10))
            .await
            .expect("first terminal session should acquire the only slot");

        let rejected = manager
            .create_session("echo rejected", &workdir, "terminal-limit-test", Some(10))
            .await;
        assert_eq!(
            rejected.err().as_deref(),
            Some(
                "The maximum number of active terminal sessions has been reached. Stop an existing session before starting another."
            )
        );
        assert_eq!(manager.sessions.lock().await.len(), 1);

        manager
            .kill_session(&first.id, "terminal-limit-test")
            .await
            .expect("the active session should stop and release its manager entry");
        drop(first);

        let next = manager
            .create_session("echo next", &workdir, "terminal-limit-test", Some(10))
            .await
            .expect("stopping and releasing the first session should free capacity");
        manager
            .kill_session(&next.id, "terminal-limit-test")
            .await
            .expect("the next session should stop cleanly");
        drop(next);

        std::fs::remove_dir_all(&workdir).expect("remove the isolated terminal test folder");
    }
}
