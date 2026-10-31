//! Manages bounded interactive process sessions used by model-requested tools.
//!
//! Interactive session identifiers are random, and every session remains bound to the
//! conversation that created it so one conversation cannot control another's process.
use std::{
    collections::HashMap,
    fs, io,
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
use tokio::process::Command;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    time::{sleep, sleep_until},
};
use uuid::Uuid;

use crate::sandbox::SandboxBackend;

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
const MAX_WORKING_DIRECTORY_STATES: usize = 512;
const INTERNAL_TERMINAL_OWNER: &str = "openchat-internal";

#[derive(Debug)]
pub(crate) struct ToolRuntimeError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) details: Option<String>,
}

impl ToolRuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    fn process_start(error: io::Error, operation: &str) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => "process_not_found",
            io::ErrorKind::PermissionDenied => "process_permission_denied",
            _ => "process_start_failed",
        };
        let details = error.raw_os_error().map_or_else(
            || error.to_string(),
            |code| format!("OS error {code}: {error}"),
        );
        Self::new(
            code,
            format!("The operating system could not start the {operation} process."),
        )
        .with_details(details)
    }

    fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl From<String> for ToolRuntimeError {
    fn from(message: String) -> Self {
        Self::new("terminal_error", "The terminal operation failed.").with_details(message)
    }
}

#[cfg(windows)]
fn command_sandbox_options<'a>(
    workspace_roots: &'a [PathBuf],
) -> Result<SandboxOptions<'a>, String> {
    let job_limits = SandboxJobLimits::for_current_host().map_err(|_| {
        "Windows could not determine safe resource limits for this command.".to_owned()
    })?;

    Ok(SandboxOptions {
        additional_workspace_paths: workspace_roots,
        job_limits: Some(job_limits),
        ..SandboxOptions::default()
    })
}

fn terminal_session_unavailable() -> String {
    "Terminal session was not found or is not available to this conversation.".to_owned()
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .replace('\\', "/")
        .eq_ignore_ascii_case(&right.to_string_lossy().replace('\\', "/"))
}

fn shell_quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = path.to_string_lossy().replace('/', "\\");
    let root = root.to_string_lossy().replace('/', "\\");
    let root = root.trim_end_matches('\\');
    path.eq_ignore_ascii_case(root)
        || path
            .get(..root.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
            && path
                .as_bytes()
                .get(root.len())
                .is_some_and(|separator| *separator == b'\\')
}

#[cfg(not(windows))]
fn path_is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[derive(Clone)]
struct WorkingDirectoryState {
    workspace_root: PathBuf,
    current_directory: PathBuf,
    command_sequence: u64,
    last_used: Instant,
}

enum CapturedWorkingDirectory {
    Missing,
    Valid(PathBuf),
    OutsideAllowedRoots,
}

struct WorkingDirectoryCapture {
    directory: PathBuf,
    report_file: PathBuf,
    workspace_root: PathBuf,
    allowed_roots: Vec<PathBuf>,
    command_sequence: u64,
}

struct TerminalSessionRequest<'a> {
    command_str: &'a str,
    workdir: &'a Path,
    owner_conversation_id: &'a str,
    timeout_seconds: Option<u64>,
    workspace_roots: &'a [PathBuf],
    full_access: bool,
    idle_timeout: Option<Duration>,
    working_directory_capture: Option<WorkingDirectoryCapture>,
}

pub(crate) struct TerminalCommandRequest<'a> {
    pub(crate) command_str: &'a str,
    pub(crate) workdir: &'a Path,
    pub(crate) owner_conversation_id: &'a str,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) wait_ms: Option<u64>,
    pub(crate) workspace_roots: &'a [PathBuf],
    pub(crate) full_access: bool,
}

#[cfg(windows)]
pub(crate) struct ProjectToolRequest<'a> {
    pub(crate) program: &'a Path,
    pub(crate) workdir: &'a Path,
    pub(crate) input: &'a str,
    pub(crate) timeout_seconds: u64,
    pub(crate) output_limit_bytes: usize,
    pub(crate) cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    pub(crate) full_access: bool,
}

impl WorkingDirectoryCapture {
    async fn create(
        workspace_root: PathBuf,
        allowed_roots: Vec<PathBuf>,
        command_sequence: u64,
    ) -> io::Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("OpenChatTerminalState-{}", Uuid::new_v4().simple()));
        tokio::fs::create_dir(&directory).await?;
        let report_file = directory.join("working-directory");
        Ok(Self {
            directory,
            report_file,
            workspace_root,
            allowed_roots,
            command_sequence,
        })
    }

    fn read(&self) -> CapturedWorkingDirectory {
        let Ok(metadata) = fs::metadata(&self.report_file) else {
            return CapturedWorkingDirectory::Missing;
        };
        if metadata.len() > 32 * 1024 {
            return CapturedWorkingDirectory::Missing;
        }
        let Ok(contents) = fs::read_to_string(&self.report_file) else {
            return CapturedWorkingDirectory::Missing;
        };
        let directory = PathBuf::from(contents);
        let Ok(directory) = fs::canonicalize(directory) else {
            return CapturedWorkingDirectory::Missing;
        };
        if !directory.is_dir() {
            return CapturedWorkingDirectory::Missing;
        }
        if self
            .allowed_roots
            .iter()
            .any(|root| path_is_within(&directory, root))
        {
            CapturedWorkingDirectory::Valid(directory)
        } else {
            CapturedWorkingDirectory::OutsideAllowedRoots
        }
    }
}

impl Drop for WorkingDirectoryCapture {
    fn drop(&mut self) {
        if fs::remove_dir_all(&self.directory).is_err() {
            eprintln!("terminal_state_cleanup_failed");
        }
    }
}

struct WorkingDirectoryUpdate {
    current_directory: Option<PathBuf>,
    reset_to_workspace_root: bool,
}

fn store_working_directory(
    states: &mut HashMap<String, WorkingDirectoryState>,
    owner_conversation_id: &str,
    workspace_root: PathBuf,
    current_directory: PathBuf,
    command_sequence: u64,
) {
    if states
        .get(owner_conversation_id)
        .is_some_and(|state| state.command_sequence > command_sequence)
    {
        return;
    }
    if states.len() >= MAX_WORKING_DIRECTORY_STATES
        && !states.contains_key(owner_conversation_id)
        && let Some(oldest_owner) = states
            .iter()
            .min_by_key(|(_, state)| state.last_used)
            .map(|(owner, _)| owner.clone())
    {
        states.remove(&oldest_owner);
    }
    states.insert(
        owner_conversation_id.to_owned(),
        WorkingDirectoryState {
            workspace_root,
            current_directory,
            command_sequence,
            last_used: Instant::now(),
        },
    );
}

async fn finalize_session_working_directory(
    states: &Mutex<HashMap<String, WorkingDirectoryState>>,
    session: &TerminalSession,
) -> WorkingDirectoryUpdate {
    let capture = session.working_directory_capture.lock().await.take();
    let Some(capture) = capture else {
        let current_directory =
            current_working_directory(states, &session.owner_conversation_id).await;
        return WorkingDirectoryUpdate {
            current_directory,
            reset_to_workspace_root: false,
        };
    };

    let result = capture.read();
    let mut states = states.lock().await;
    if states
        .get(&session.owner_conversation_id)
        .is_some_and(|state| state.command_sequence > capture.command_sequence)
    {
        let current_directory = states.get_mut(&session.owner_conversation_id).map(|state| {
            state.last_used = Instant::now();
            state.current_directory.clone()
        });
        return WorkingDirectoryUpdate {
            current_directory,
            reset_to_workspace_root: false,
        };
    }
    match result {
        CapturedWorkingDirectory::Missing => {
            let current_directory = states.get_mut(&session.owner_conversation_id).map(|state| {
                state.last_used = Instant::now();
                state.current_directory.clone()
            });
            WorkingDirectoryUpdate {
                current_directory,
                reset_to_workspace_root: false,
            }
        }
        CapturedWorkingDirectory::Valid(current_directory) => {
            store_working_directory(
                &mut states,
                &session.owner_conversation_id,
                capture.workspace_root.clone(),
                current_directory.clone(),
                capture.command_sequence,
            );
            WorkingDirectoryUpdate {
                current_directory: Some(current_directory),
                reset_to_workspace_root: false,
            }
        }
        CapturedWorkingDirectory::OutsideAllowedRoots => {
            let workspace_root = capture.workspace_root.clone();
            store_working_directory(
                &mut states,
                &session.owner_conversation_id,
                workspace_root.clone(),
                workspace_root.clone(),
                capture.command_sequence,
            );
            WorkingDirectoryUpdate {
                current_directory: Some(workspace_root),
                reset_to_workspace_root: true,
            }
        }
    }
}

async fn current_working_directory(
    states: &Mutex<HashMap<String, WorkingDirectoryState>>,
    owner_conversation_id: &str,
) -> Option<PathBuf> {
    states
        .lock()
        .await
        .get_mut(owner_conversation_id)
        .map(|state| {
            state.last_used = Instant::now();
            state.current_directory.clone()
        })
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
    child: Arc<Mutex<TerminalChild>>,
    #[cfg(not(windows))]
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    stdin: Arc<Mutex<Option<TerminalStdin>>>,
    output_buffer: Arc<Mutex<Vec<u8>>>,
    last_activity: Arc<Mutex<Instant>>,
    working_directory_capture: Mutex<Option<WorkingDirectoryCapture>>,
    _session_slot: OwnedSemaphorePermit,
}

#[cfg(not(windows))]
fn build_shell_command(command_str: &str, state_file: Option<&Path>) -> Command {
    let mut cmd = Command::new("sh");
    let command = match state_file {
        Some(state_file) => {
            let state_file = shell_quote_posix(&state_file.to_string_lossy());
            let exit_trap = format!(
                "__openchat_exit_code=$?; trap - EXIT; printf '%s' \"$PWD\" > {state_file}; exit \"$__openchat_exit_code\""
            );
            format!("trap {} EXIT\n{command_str}", shell_quote_posix(&exit_trap))
        }
        None => command_str.to_owned(),
    };
    cmd.args(["-c", &command]);
    cmd
}

#[cfg(windows)]
enum TerminalChild {
    Managed(Box<SandboxedProcess>),
    Process(Box<tokio::process::Child>),
}

#[cfg(windows)]
type TerminalOutputReader = Box<dyn tokio::io::AsyncRead + Send + Unpin>;

enum TerminalStdin {
    #[cfg(windows)]
    Managed(tokio::fs::File),
    Child(tokio::process::ChildStdin),
}

impl TerminalSession {
    async fn process_status(&self) -> Result<Option<ExitStatus>, String> {
        let mut child = self.child.lock().await;
        #[cfg(windows)]
        {
            match &mut *child {
                TerminalChild::Managed(managed) => managed.try_wait().map_err(|error| {
                    format!("Could not check the command process status: {error}")
                }),
                TerminalChild::Process(proc) => proc.try_wait().map_err(|error| {
                    format!("Could not check the command process status: {error}")
                }),
            }
        }
        #[cfg(not(windows))]
        {
            child
                .try_wait()
                .map_err(|error| format!("Could not check the command process status: {error}"))
        }
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
        match stdin {
            #[cfg(windows)]
            TerminalStdin::Managed(f) => {
                f.write_all(input.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write to terminal stdin: {e}"))?;
                f.flush()
                    .await
                    .map_err(|e| format!("Failed to flush terminal stdin: {e}"))?;
            }
            TerminalStdin::Child(c) => {
                c.write_all(input.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write to terminal stdin: {e}"))?;
                c.flush()
                    .await
                    .map_err(|e| format!("Failed to flush terminal stdin: {e}"))?;
            }
        }
        *self.last_activity.lock().await = Instant::now();
        Ok(())
    }

    /// Stops the process and waits for its process tree to exit where supported.
    pub async fn kill(&self) -> Result<(), String> {
        let mut child = self.child.lock().await;
        #[cfg(windows)]
        {
            match &mut *child {
                TerminalChild::Managed(managed) => {
                    managed
                        .start_kill()
                        .map_err(|error| format!("Could not stop the command process: {error}"))?;
                    managed.wait_for_job_exit().map_err(|error| {
                        format!("Could not stop the command process tree: {error}")
                    })?;
                    return managed
                        .wait()
                        .await
                        .map(|_| ())
                        .map_err(|error| format!("Could not stop the command process: {error}"));
                }
                TerminalChild::Process(proc) => {
                    if let Err(error) = proc.start_kill() {
                        if matches!(proc.try_wait(), Ok(Some(_))) {
                            return Ok(());
                        }
                        return Err(format!("Could not stop the command process: {error}"));
                    }
                    return proc
                        .wait()
                        .await
                        .map(|_| ())
                        .map_err(|error| format!("Could not stop the command process: {error}"));
                }
            }
        }
        #[cfg(not(windows))]
        {
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
    working_directories: Arc<Mutex<HashMap<String, WorkingDirectoryState>>>,
    next_working_directory_sequence: AtomicU64,
    session_slots: Arc<Semaphore>,
    backend: Arc<Mutex<SandboxBackend>>,
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
            working_directories: Arc::new(Mutex::new(HashMap::new())),
            next_working_directory_sequence: AtomicU64::new(0),
            session_slots: Arc::new(Semaphore::new(session_limit)),
            backend: Arc::new(Mutex::new(SandboxBackend::Native)),
        };
        manager.start_reaper();
        manager
    }

    /// Sets the active sandbox backend for terminal sessions.
    #[allow(dead_code)]
    pub async fn set_backend(&self, backend: SandboxBackend) {
        let mut b = self.backend.lock().await;
        *b = backend;
    }

    /// Returns the active sandbox backend for terminal sessions.
    #[allow(dead_code)]
    pub async fn backend(&self) -> SandboxBackend {
        *self.backend.lock().await
    }

    #[cfg(test)]
    pub fn isolated() -> Self {
        Self::new()
    }

    fn start_reaper(&self) {
        let sessions = Arc::clone(&self.sessions);
        let working_directories = Arc::clone(&self.working_directories);
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
                        Ok(false) => {
                            finalize_session_working_directory(&working_directories, session).await;
                            to_remove.push(id.clone());
                        }
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

    async fn resolve_working_directory(
        &self,
        workspace_root: &Path,
        workspace_roots: &[PathBuf],
        owner_conversation_id: &str,
    ) -> Result<(PathBuf, PathBuf, Vec<PathBuf>, bool, u64), ToolRuntimeError> {
        let command_sequence = self
            .next_working_directory_sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |sequence| {
                sequence.checked_add(1)
            })
            .map(|previous| previous + 1)
            .map_err(|_| {
                ToolRuntimeError::new(
                    "terminal_state_sequence_exhausted",
                    "The terminal working-directory session could not be advanced.",
                )
            })?;
        let workspace_root = tokio::fs::canonicalize(workspace_root)
            .await
            .map_err(|error| {
                ToolRuntimeError::new(
                    "terminal_workdir_unavailable",
                    "The project working directory is unavailable.",
                )
                .with_details(error.to_string())
            })?;
        if !workspace_root.is_dir() {
            return Err(ToolRuntimeError::new(
                "terminal_workdir_unavailable",
                "The project working directory is unavailable.",
            ));
        }

        let mut allowed_roots = vec![workspace_root.clone()];
        for root in workspace_roots {
            let root = tokio::fs::canonicalize(root).await.map_err(|error| {
                ToolRuntimeError::new(
                    "terminal_workspace_unavailable",
                    "An approved terminal workspace directory is unavailable.",
                )
                .with_details(error.to_string())
            })?;
            if !allowed_roots
                .iter()
                .any(|existing| same_path(existing, &root))
            {
                allowed_roots.push(root);
            }
        }

        let previous = self
            .working_directories
            .lock()
            .await
            .get(owner_conversation_id)
            .cloned();
        let mut reset_to_workspace_root = false;
        let current_directory = if let Some(previous) = previous {
            if same_path(&previous.workspace_root, &workspace_root) {
                match tokio::fs::canonicalize(&previous.current_directory).await {
                    Ok(current_directory)
                        if current_directory.is_dir()
                            && allowed_roots
                                .iter()
                                .any(|root| path_is_within(&current_directory, root)) =>
                    {
                        current_directory
                    }
                    _ => {
                        reset_to_workspace_root = true;
                        workspace_root.clone()
                    }
                }
            } else {
                reset_to_workspace_root = true;
                workspace_root.clone()
            }
        } else {
            workspace_root.clone()
        };

        let mut directory_states = self.working_directories.lock().await;
        store_working_directory(
            &mut directory_states,
            owner_conversation_id,
            workspace_root.clone(),
            current_directory.clone(),
            command_sequence,
        );
        drop(directory_states);
        Ok((
            workspace_root,
            current_directory,
            allowed_roots,
            reset_to_workspace_root,
            command_sequence,
        ))
    }

    /// Starts a session owned by one conversation.
    ///
    /// The owner identifier is supplied by trusted execution context, not tool arguments.
    /// The returned session ID is random and required for later operations in that conversation.
    #[cfg(test)]
    pub async fn create_session(
        &self,
        command_str: &str,
        workdir: &Path,
        owner_conversation_id: &str,
        timeout_seconds: Option<u64>,
    ) -> Result<Arc<TerminalSession>, String> {
        self.create_session_with_workspace_roots(TerminalSessionRequest {
            command_str,
            workdir,
            owner_conversation_id,
            timeout_seconds,
            workspace_roots: &[],
            full_access: false,
            idle_timeout: Some(Duration::from_secs(120)),
            working_directory_capture: None,
        })
        .await
        .map_err(|error| error.message)
    }

    async fn create_session_with_workspace_roots(
        &self,
        request: TerminalSessionRequest<'_>,
    ) -> Result<Arc<TerminalSession>, ToolRuntimeError> {
        let TerminalSessionRequest {
            command_str,
            workdir,
            owner_conversation_id,
            timeout_seconds,
            workspace_roots,
            full_access,
            idle_timeout,
            working_directory_capture,
        } = request;
        let session_slot = Arc::clone(&self.session_slots)
            .try_acquire_owned()
            .map_err(|_| {
                ToolRuntimeError::new(
                    "terminal_session_limit",
                    "The maximum number of active terminal sessions has been reached. Stop an existing session before starting another.",
                )
            })?;
        let id = format!("term_{}", Uuid::new_v4().simple());

        let timeout_secs = timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            .clamp(5, MAX_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(timeout_secs);

        #[cfg(windows)]
        let (child, stdin, stdout, stderr): (
            TerminalChild,
            Option<TerminalStdin>,
            Option<TerminalOutputReader>,
            Option<TerminalOutputReader>,
        ) = {
            #[cfg(windows)]
            let fresh_path = crate::sandbox::fresh_path().map_err(|error| {
                ToolRuntimeError::new(
                    "terminal_environment_refresh_failed",
                    "Windows could not refresh PATH for the new terminal command.",
                )
                .with_details(error.to_string())
            })?;
            let active_backend = *self.backend.lock().await;
            if active_backend == SandboxBackend::Wsl2 {
                if !crate::sandbox::SandboxPolicy::is_wsl2_available() {
                    return Err(ToolRuntimeError::new(
                        "terminal_backend_unavailable",
                        "WSL2 is not available or not installed on this host.",
                    ));
                }
                let win_path = workdir.to_string_lossy().replace('\\', "/");
                let escaped_workdir = win_path.replace('\'', "'\\''");
                let mut cmd = tokio::process::Command::new("wsl.exe");
                #[cfg(windows)]
                cmd.env("PATH", &fresh_path);
                let mut process_workspace_roots = workspace_roots.to_vec();
                if let Some(capture) = &working_directory_capture {
                    process_workspace_roots.push(capture.directory.clone());
                }
                let additional_binds = process_workspace_roots
                    .iter()
                    .filter(|root| !same_path(root, workdir))
                    .map(|root| {
                        let path = root.to_string_lossy().replace('\\', "/");
                        let escaped_path = path.replace('\'', "'\\''");
                        format!(
                            " --bind \"$(wslpath -u '{escaped_path}')\" \"$(wslpath -u '{escaped_path}')\""
                        )
                    })
                    .collect::<String>();
                let shell_command = match &working_directory_capture {
                    Some(capture) => {
                        let report_path = capture.report_file.to_string_lossy().replace('\\', "/");
                        let report_path = shell_quote_posix(&report_path);
                        let exit_trap = format!(
                            "__openchat_exit_code=$?; trap - EXIT; __openchat_windows_pwd=\"$(wslpath -w \"$PWD\" 2>/dev/null)\" && printf '%s' \"$__openchat_windows_pwd\" > \"$(wslpath -u {report_path})\"; exit \"$__openchat_exit_code\""
                        );
                        format!("trap {} EXIT\n{command_str}", shell_quote_posix(&exit_trap))
                    }
                    None => command_str.to_owned(),
                };
                // Escape single quotes safely for the outer POSIX shell.
                let escaped_command = shell_command.replace('\'', "'\\''");
                let script = if full_access {
                    format!(
                        r#"WORKDIR="$(wslpath -u '{escaped_workdir}')" && cd "$WORKDIR" && sh -c '{escaped_command}'"#
                    )
                } else {
                    format!(
                        r#"WORKDIR="$(wslpath -u '{escaped_workdir}')" && bwrap --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind-try /lib64 /lib64 --ro-bind /bin /bin --ro-bind /sbin /sbin --proc /proc --dev /dev --bind "$WORKDIR" "$WORKDIR"{additional_binds} --chdir "$WORKDIR" --unshare-all -- sh -c '{escaped_command}'"#
                    )
                };
                cmd.args(["-e", "sh", "-c", &script])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                let mut proc = cmd
                    .spawn()
                    .map_err(|error| ToolRuntimeError::process_start(error, "WSL2 command"))?;
                let stdin = proc.stdin.take().map(TerminalStdin::Child);
                let stdout: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    proc.stdout.take().map(|s| Box::new(s) as _);
                let stderr: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    proc.stderr.take().map(|s| Box::new(s) as _);
                (
                    TerminalChild::Process(Box::new(proc)),
                    stdin,
                    stdout,
                    stderr,
                )
            } else if full_access {
                let options = command_sandbox_options(&[])?;
                let mut options = options;
                options.path_override = Some(&fresh_path);
                options.working_directory_state_file = working_directory_capture
                    .as_ref()
                    .map(|capture| capture.report_file.as_path());
                let mut child =
                    SandboxedProcess::spawn_without_sandbox(command_str, workdir, options)
                        .map_err(|error| ToolRuntimeError::process_start(error, "command"))?;
                let stdin = child.take_stdin().map(TerminalStdin::Managed);
                let stdout: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    child.take_stdout().map(|stream| Box::new(stream) as _);
                let stderr: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    child.take_stderr().map(|stream| Box::new(stream) as _);
                (
                    TerminalChild::Managed(Box::new(child)),
                    stdin,
                    stdout,
                    stderr,
                )
            } else {
                let mut additional_roots = workspace_roots
                    .iter()
                    .filter(|root| !same_path(root, workdir))
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(capture) = &working_directory_capture {
                    additional_roots.push(capture.directory.clone());
                }
                let mut options = command_sandbox_options(&additional_roots)?;
                options.path_override = Some(&fresh_path);
                options.working_directory_state_file = working_directory_capture
                    .as_ref()
                    .map(|capture| capture.report_file.as_path());
                let mut child = SandboxedProcess::spawn_with_options(command_str, workdir, options)
                    .map_err(|error| ToolRuntimeError::process_start(error, "sandboxed command"))?;
                let stdin = child.take_stdin().map(TerminalStdin::Managed);
                let stdout: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    child.take_stdout().map(|s| Box::new(s) as _);
                let stderr: Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>> =
                    child.take_stderr().map(|s| Box::new(s) as _);
                (
                    TerminalChild::Managed(Box::new(child)),
                    stdin,
                    stdout,
                    stderr,
                )
            }
        };

        #[cfg(not(windows))]
        let (child, stdin, stdout, stderr) = {
            let mut cmd = build_shell_command(
                command_str,
                working_directory_capture
                    .as_ref()
                    .map(|capture| capture.report_file.as_path()),
            );
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
            working_directory_capture: Mutex::new(working_directory_capture),
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
    #[cfg(test)]
    pub async fn execute(
        &self,
        command_str: &str,
        workdir: &Path,
        owner_conversation_id: &str,
        timeout_seconds: Option<u64>,
        wait_ms: Option<u64>,
    ) -> Result<Value, String> {
        self.execute_with_workspace_roots(TerminalCommandRequest {
            command_str,
            workdir,
            owner_conversation_id,
            timeout_seconds,
            wait_ms,
            workspace_roots: &[],
            full_access: false,
        })
        .await
        .map_err(|error| error.message)
    }

    /// Runs a command with write access to the explicitly configured project roots.
    pub async fn execute_with_workspace_roots(
        &self,
        request: TerminalCommandRequest<'_>,
    ) -> Result<Value, ToolRuntimeError> {
        let TerminalCommandRequest {
            command_str,
            workdir,
            owner_conversation_id,
            timeout_seconds,
            wait_ms,
            workspace_roots,
            full_access,
        } = request;
        let wait_duration =
            Duration::from_millis(wait_ms.unwrap_or(DEFAULT_WAIT_MS).clamp(100, MAX_WAIT_MS));

        let (workspace_root, workdir, allowed_roots, reset_to_workspace_root, command_sequence) =
            self.resolve_working_directory(workdir, workspace_roots, owner_conversation_id)
                .await?;
        let capture =
            WorkingDirectoryCapture::create(workspace_root, allowed_roots, command_sequence)
                .await
                .map_err(|error| {
                    ToolRuntimeError::new(
                        "terminal_state_initialization_failed",
                        "The terminal working-directory state could not be initialized.",
                    )
                    .with_details(error.to_string())
                })?;
        let mut process_workspace_roots = workspace_roots.to_vec();
        process_workspace_roots.push(capture.directory.clone());

        let session = self
            .create_session_with_workspace_roots(TerminalSessionRequest {
                command_str,
                workdir: &workdir,
                owner_conversation_id,
                timeout_seconds,
                workspace_roots: &process_workspace_roots,
                full_access,
                idle_timeout: Some(Duration::from_secs(120)),
                working_directory_capture: Some(capture),
            })
            .await?;
        let (output, _offset, truncated) = session.wait_for_output(0, wait_duration).await?;
        let status = session.process_status().await?;

        if let Some(status) = status {
            let directory_update =
                finalize_session_working_directory(&self.working_directories, &session).await;
            let final_working_directory = directory_update
                .current_directory
                .unwrap_or_else(|| workdir.clone())
                .to_string_lossy()
                .into_owned();
            // Process completed within wait window; clean up session
            self.remove_session(&session.id).await;
            Ok(json!({
                "command": command_str,
                "is_running": false,
                "exit_code": status.code(),
                "output": output,
                "truncated": truncated,
                "working_directory": final_working_directory,
                "working_directory_reset": reset_to_workspace_root || directory_update.reset_to_workspace_root
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
                "working_directory": workdir.to_string_lossy(),
                "working_directory_reset": reset_to_workspace_root,
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
        full_access: bool,
        cancellation: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<Value, ToolRuntimeError> {
        if !(5..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
            return Err(ToolRuntimeError::new(
                "project_task_invalid_timeout",
                "The named task timeout is outside the supported range.",
            ));
        }
        if *cancellation.borrow() {
            return Ok(
                json!({"status": "cancelled", "exitCode": Value::Null, "output": "", "truncated": false}),
            );
        }

        let session = self
            .create_session_with_workspace_roots(TerminalSessionRequest {
                command_str,
                workdir,
                owner_conversation_id: INTERNAL_TERMINAL_OWNER,
                timeout_seconds: Some(timeout_seconds),
                workspace_roots: &[],
                full_access,
                idle_timeout: None,
                working_directory_capture: None,
            })
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
                            return Err(ToolRuntimeError::from(error));
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
        request: ProjectToolRequest<'_>,
    ) -> Result<Value, ToolRuntimeError> {
        let ProjectToolRequest {
            program,
            workdir,
            input,
            timeout_seconds,
            output_limit_bytes,
            cancellation,
            full_access,
        } = request;
        if !(5..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds)
            || !(1..=MAX_BUFFER_BYTES).contains(&output_limit_bytes)
            || input.len() > 16 * 1024
        {
            return Err(ToolRuntimeError::new(
                "project_tool_failed",
                "The configured project tool is outside its execution limits.",
            ));
        }
        if *cancellation.borrow() {
            return Ok(
                json!({"status":"cancelled","exit_code":Value::Null,"output":"","truncated":false}),
            );
        }

        let options = command_sandbox_options(&[])?;
        let mut process = if full_access {
            SandboxedProcess::spawn_program_without_sandbox(program, &[], workdir, &[], options)
        } else {
            SandboxedProcess::spawn_program_with_options(program, &[], workdir, &[], options)
        }
        .map_err(|error| ToolRuntimeError::process_start(error, "configured project tool"))?;
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
            let directory_update =
                finalize_session_working_directory(&self.working_directories, &session).await;
            let working_directory = directory_update
                .current_directory
                .map(|directory| directory.to_string_lossy().into_owned());
            self.remove_session(terminal_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "is_running": false,
                "exit_code": status.code(),
                "output": output,
                "truncated": truncated,
                "working_directory": working_directory,
                "working_directory_reset": directory_update.reset_to_workspace_root
            }))
        } else {
            let working_directory =
                current_working_directory(&self.working_directories, owner_conversation_id).await;
            Ok(json!({
                "terminal_id": terminal_id,
                "is_running": true,
                "waiting_for_input": true,
                "exit_code": Value::Null,
                "output": output,
                "truncated": truncated,
                "working_directory": working_directory
                    .map(|directory| directory.to_string_lossy().into_owned())
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

        let directory_update = if status.is_some() {
            let update =
                finalize_session_working_directory(&self.working_directories, &session).await;
            self.remove_session(terminal_id).await;
            update
        } else {
            WorkingDirectoryUpdate {
                current_directory: current_working_directory(
                    &self.working_directories,
                    owner_conversation_id,
                )
                .await,
                reset_to_workspace_root: false,
            }
        };
        let working_directory = directory_update
            .current_directory
            .map(|directory| directory.to_string_lossy().into_owned());

        Ok(json!({
            "terminal_id": terminal_id,
            "is_running": status.is_none(),
            "exit_code": status.and_then(|status| status.code()),
            "output": output,
            "truncated": truncated,
            "working_directory": working_directory,
            "working_directory_reset": directory_update.reset_to_workspace_root
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
        self.working_directories.lock().await.clear();
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
    use super::{TerminalSessionManager, TerminalSessionRequest, shell_quote_posix};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn terminal_working_directory_keeps_all_configured_workspace_roots() {
        let base = std::env::temp_dir().join(format!(
            "openchat-terminal-workspace-roots-{}",
            Uuid::new_v4().simple()
        ));
        let primary = base.join("primary");
        let secondary = base.join("secondary");
        fs::create_dir_all(&primary).expect("create primary workspace root");
        fs::create_dir_all(&secondary).expect("create secondary workspace root");
        let primary = fs::canonicalize(primary).expect("canonicalize primary workspace root");
        let secondary = fs::canonicalize(secondary).expect("canonicalize secondary workspace root");
        let manager = TerminalSessionManager::isolated();

        let (workspace_root, current_directory, allowed_roots, reset, _) = manager
            .resolve_working_directory(
                &primary,
                std::slice::from_ref(&secondary),
                "workspace-roots-test",
            )
            .await
            .expect("resolve configured terminal workspace roots");

        assert_eq!(workspace_root, primary);
        assert_eq!(current_directory, primary);
        assert_eq!(allowed_roots, vec![primary, secondary]);
        assert!(!reset);
        drop(manager);
        fs::remove_dir_all(base).expect("remove terminal workspace test folder");
    }

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

    #[tokio::test]
    async fn manager_supports_configuring_sandbox_backend() {
        let manager = TerminalSessionManager::isolated();
        assert_eq!(
            manager.backend().await,
            crate::sandbox::SandboxBackend::Native
        );
        manager
            .set_backend(crate::sandbox::SandboxBackend::Wsl2)
            .await;
        assert_eq!(
            manager.backend().await,
            crate::sandbox::SandboxBackend::Wsl2
        );
    }

    #[tokio::test]
    async fn wsl2_terminal_runs_a_command_inside_the_bubblewrap_workspace() {
        if !crate::sandbox::SandboxPolicy::is_wsl2_available() {
            return;
        }

        let base = std::env::temp_dir().join(format!(
            "openchat-terminal-wsl2-{}",
            Uuid::new_v4().simple()
        ));
        let primary = base.join("primary");
        let secondary = base.join("secondary");
        let ungranted = base.join("ungranted");
        fs::create_dir_all(&primary).expect("create the primary WSL2 workspace");
        fs::create_dir_all(&secondary).expect("create the secondary WSL2 workspace");
        fs::create_dir_all(&ungranted).expect("create an ungranted WSL2 sibling");
        let primary = fs::canonicalize(primary).expect("canonicalize primary WSL2 workspace");
        let secondary = fs::canonicalize(secondary).expect("canonicalize secondary WSL2 workspace");
        let ungranted = fs::canonicalize(ungranted).expect("canonicalize ungranted WSL2 sibling");
        let secondary_path = wsl_path(&secondary);
        let ungranted_path = wsl_path(&ungranted);
        let command = format!(
            "test -d {} && test ! -e {} && printf 'wsl2-service-test-ok'",
            shell_quote_posix(&secondary_path),
            shell_quote_posix(&ungranted_path),
        );
        let manager = TerminalSessionManager::isolated();
        manager
            .set_backend(crate::sandbox::SandboxBackend::Wsl2)
            .await;
        let session = manager
            .create_session_with_workspace_roots(TerminalSessionRequest {
                command_str: &command,
                workdir: &primary,
                owner_conversation_id: "wsl2-terminal-test",
                timeout_seconds: Some(10),
                workspace_roots: std::slice::from_ref(&secondary),
                full_access: false,
                idle_timeout: Some(Duration::from_secs(120)),
                working_directory_capture: None,
            })
            .await
            .expect("start a scoped non-mutating command in the WSL2 sandbox");

        let result = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let state = manager
                    .read_output_of(&session.id, "wsl2-terminal-test", Some(250))
                    .await?;
                let is_running = state["is_running"]
                    .as_bool()
                    .ok_or_else(|| "terminal response omitted its running state".to_owned())?;
                if !is_running {
                    return Ok::<_, String>(state);
                }
            }
        })
        .await
        .expect("WSL2 terminal command completes within the test deadline")
        .expect("read WSL2 terminal command output");

        assert_eq!(result["exit_code"], 0, "WSL2 scope check failed: {result}");
        assert!(
            result["output"]
                .as_str()
                .is_some_and(|output| output.contains("wsl2-service-test-ok")),
            "WSL2 terminal output did not contain the command marker: {result}"
        );
        manager
            .stop_all()
            .await
            .expect("stop any remaining WSL2 terminal sessions");
        drop(session);
        drop(manager);
        fs::remove_dir_all(base).expect("remove isolated WSL2 test directory");
    }

    fn wsl_path(path: &Path) -> String {
        let output = Command::new("wsl.exe")
            .args(["--exec", "wslpath", "-u"])
            .arg(path)
            .output()
            .expect("convert the temporary Windows path to its WSL path");
        assert!(
            output.status.success(),
            "wslpath failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("wslpath output should be UTF-8")
            .trim()
            .to_owned()
    }
}
