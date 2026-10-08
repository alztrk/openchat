use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
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
    sync::Mutex,
    time::sleep,
};

#[cfg(windows)]
#[path = "terminal_windows.rs"]
pub(crate) mod windows_sandbox;
#[cfg(windows)]
use windows_sandbox::SandboxedProcess;

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
    idle_timeout: Option<Duration>,
    #[cfg(windows)]
    child: Arc<Mutex<SandboxedProcess>>,
    #[cfg(not(windows))]
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    stdin: Arc<Mutex<Option<TerminalStdin>>>,
    output_buffer: Arc<Mutex<Vec<u8>>>,
    last_activity: Arc<Mutex<Instant>>,
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
            return child.try_wait().map_err(|error| {
                format!("Could not check the sandboxed command process status: {error}")
            });
        }
        #[cfg(not(windows))]
        let mut child = child;
        #[cfg(not(windows))]
        child
            .try_wait()
            .map_err(|error| format!("Could not check the command process status: {error}"))
    }

    pub async fn is_running(&self) -> Result<bool, String> {
        self.process_status().await.map(|status| status.is_none())
    }

    pub async fn send_input(&self, input: &str) -> Result<(), String> {
        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| "Terminal stdin is closed or unavailable.".to_owned())?;
        let stdin = match stdin {
            #[cfg(windows)]
            TerminalStdin::Sandboxed(stdin) => stdin,
            #[cfg(not(windows))]
            TerminalStdin::Child(stdin) => stdin,
        };
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

pub struct TerminalSessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalSession>>>>,
    counter: AtomicU64,
}

static MANAGER: OnceLock<TerminalSessionManager> = OnceLock::new();

impl TerminalSessionManager {
    pub fn global() -> &'static Self {
        MANAGER.get_or_init(Self::new)
    }

    fn new() -> Self {
        let manager = Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            counter: AtomicU64::new(1),
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

    pub async fn create_session(
        &self,
        command_str: &str,
        workdir: &Path,
        timeout_seconds: Option<u64>,
    ) -> Result<Arc<TerminalSession>, String> {
        self.create_session_with_idle_timeout(
            command_str,
            workdir,
            timeout_seconds,
            Some(Duration::from_secs(120)),
        )
        .await
    }

    async fn create_session_with_idle_timeout(
        &self,
        command_str: &str,
        workdir: &Path,
        timeout_seconds: Option<u64>,
        idle_timeout: Option<Duration>,
    ) -> Result<Arc<TerminalSession>, String> {
        let id_num = self.counter.fetch_add(1, Ordering::Relaxed);
        let id = format!("term_{id_num}");

        let timeout_secs = timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            .clamp(5, MAX_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(timeout_secs);

        #[cfg(windows)]
        let (child, stdin, stdout, stderr) = {
            let mut child = SandboxedProcess::spawn(command_str, workdir).map_err(|_| {
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
            command_str: command_str.to_owned(),
            workdir: workdir.to_path_buf(),
            created_at: Instant::now(),
            timeout,
            idle_timeout,
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
            .create_session_with_idle_timeout(command_str, workdir, Some(timeout_seconds), None)
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

    pub async fn read_output_of(
        &self,
        terminal_id: &str,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        let session = self.get_session(terminal_id).await.ok_or_else(|| {
            format!("Terminal session '{terminal_id}' not found or already terminated.")
        })?;

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

    pub async fn kill_session(&self, terminal_id: &str) -> Result<Value, String> {
        if let Some(session) = self.get_session(terminal_id).await {
            session.kill().await?;
            self.remove_session(terminal_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "status": "terminated",
                "message": "Terminal session was killed successfully."
            }))
        } else {
            Err(format!("Terminal session '{terminal_id}' was not found."))
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
