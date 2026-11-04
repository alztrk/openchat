//! Manages project-scoped Git worktrees and exposes bounded review metadata.
//!
//! Worktree paths are derived from validated project records and canonical IDs.
//! Git commands use fixed argument lists, time limits, output limits, and a
//! temporary hooks directory so repository hooks do not run during management.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex as StdMutex, OnceLock, Weak},
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{Mutex, watch},
    task::JoinHandle,
    time::{Instant, sleep_until, timeout},
};
use uuid::Uuid;

use crate::{git_inspection, storage::AppStorage};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
const LIST_TIMEOUT: Duration = Duration::from_secs(25);
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_WORKTREES: usize = 20;

#[derive(Debug)]
/// Failure categories returned by project worktree operations.
pub(crate) enum GitWorktreeError {
    InvalidInput,
    ProjectUnavailable,
    NotRepository,
    UnsafePath,
    GitFailed,
    Cancelled,
    TimedOut,
    LimitReached,
    Unavailable,
}

struct ListControl<'a> {
    cancellation: &'a mut watch::Receiver<bool>,
    deadline: Instant,
}

impl ListControl<'_> {
    fn check(&self) -> Result<(), GitWorktreeError> {
        if *self.cancellation.borrow() {
            return Err(GitWorktreeError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(GitWorktreeError::TimedOut);
        }
        Ok(())
    }
}

struct ProjectLocation {
    repository_root: PathBuf,
    project_subdirectory: PathBuf,
    worktree_base: PathBuf,
}

/// Lists managed worktrees for a registered project, up to the configured cap.
pub(crate) async fn list(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, GitWorktreeError> {
    let mut control = ListControl {
        cancellation,
        deadline: Instant::now() + LIST_TIMEOUT,
    };
    control.check()?;
    let project = project_location(storage, project_id, project_root, Some(&mut control)).await?;
    control.check()?;
    let base_exists = ensure_worktree_base(&project, false)?;
    if !base_exists {
        control.check()?;
        return Ok(json!({"worktrees": [], "truncated": false}));
    }
    let entries = match fs::read_dir(&project.worktree_base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            control.check()?;
            return Ok(json!({"worktrees": [], "truncated": false}));
        }
        Err(_) => {
            control.check()?;
            return Err(GitWorktreeError::Unavailable);
        }
    };

    let mut worktree_ids = BTreeSet::new();
    let mut truncated = false;
    let mut scanned_entries = 0usize;
    for entry in entries {
        scanned_entries = scanned_entries.saturating_add(1);
        if scanned_entries % 64 == 0 {
            tokio::task::yield_now().await;
        }
        control.check()?;
        let entry = entry.map_err(|_| GitWorktreeError::Unavailable)?;
        let file_type = entry
            .file_type()
            .map_err(|_| GitWorktreeError::Unavailable)?;
        if !file_type.is_dir() {
            continue;
        }
        let entry_id = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = canonical_worktree_id(&entry_id) else {
            continue;
        };
        retain_worktree_id(&mut worktree_ids, &mut truncated, id);
    }

    let mut worktrees = Vec::with_capacity(worktree_ids.len());
    for id in worktree_ids {
        control.check()?;
        let worktree = checked_worktree(&project, &id, Some(&mut control)).await?;
        let branch = git_text(&worktree, &["branch", "--show-current"], Some(&mut control)).await?;
        let status = git_inspection::status_with_cancellation(
            &project_subdirectory(&worktree, &project)?,
            control.cancellation,
            control.deadline,
        )
        .await
        .map_err(map_git_inspection_error)?;
        worktrees.push(json!({
            "id": id,
            "branch": branch,
            "path": project_subdirectory(&worktree, &project)?.to_string_lossy(),
            "status": status,
        }));
    }

    worktrees.sort_by(|left, right| {
        left.get("branch")
            .and_then(Value::as_str)
            .cmp(&right.get("branch").and_then(Value::as_str))
    });
    control.check()?;
    Ok(json!({"worktrees": worktrees, "truncated": truncated}))
}

/// Creates a new managed worktree on a generated branch for a registered project.
pub(crate) async fn create(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root, None).await?;
    let creation_lock = worktree_creation_lock(&project.worktree_base)?;
    let _creation_guard = creation_lock.lock().await;
    ensure_worktree_base(&project, true)?;
    let _creation_file_guard = acquire_worktree_creation_file_lock(&project.worktree_base).await?;
    if existing_worktree_count(&project.worktree_base)? >= MAX_WORKTREES {
        return Err(GitWorktreeError::LimitReached);
    }
    let id = Uuid::new_v4().simple().to_string();
    let branch = format!("openchat/{id}");
    let checkout = project.worktree_base.join(&id);
    let checkout_text = git_compatible_path(&checkout);
    let hooks_directory = storage.root().join(format!("git-hooks-disabled-{id}"));
    fs::create_dir(&hooks_directory).map_err(|_| GitWorktreeError::Unavailable)?;

    let result = run_git(
        &project.repository_root,
        &hooks_directory,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            &branch,
            &checkout_text,
            "HEAD",
        ],
        None,
    )
    .await;
    let _ = fs::remove_dir(&hooks_directory);
    if result.is_err() {
        let _ = fs::remove_dir_all(&checkout);
        return Err(result.err().unwrap_or(GitWorktreeError::Unavailable));
    }

    let path = project_subdirectory(&checkout, &project)?;
    Ok(json!({
        "id": id,
        "branch": branch,
        "path": path.to_string_lossy(),
    }))
}

fn retain_worktree_id(ids: &mut BTreeSet<String>, truncated: &mut bool, id: String) {
    ids.insert(id);
    if ids.len() > MAX_WORKTREES {
        *truncated = true;
        ids.pop_last();
    }
}

fn existing_worktree_count(base: &Path) -> Result<usize, GitWorktreeError> {
    let entries = fs::read_dir(base).map_err(|_| GitWorktreeError::Unavailable)?;
    let mut ids = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|_| GitWorktreeError::Unavailable)?;
        if !entry
            .file_type()
            .map_err(|_| GitWorktreeError::Unavailable)?
            .is_dir()
        {
            continue;
        }
        let entry_id = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = canonical_worktree_id(&entry_id) {
            ids.insert(id);
            if ids.len() >= MAX_WORKTREES {
                return Ok(ids.len());
            }
        }
    }
    Ok(ids.len())
}

fn worktree_creation_lock(path: &Path) -> Result<Arc<Mutex<()>>, GitWorktreeError> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut locks = locks.lock().map_err(|_| GitWorktreeError::Unavailable)?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
    Ok(lock)
}

async fn acquire_worktree_creation_file_lock(base: &Path) -> Result<fs::File, GitWorktreeError> {
    let lock_path = base.join(".create.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|_| GitWorktreeError::Unavailable)?;
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                sleep_until((Instant::now() + Duration::from_millis(25)).min(deadline)).await;
            }
            Err(std::fs::TryLockError::WouldBlock) => return Err(GitWorktreeError::TimedOut),
            Err(std::fs::TryLockError::Error(_)) => return Err(GitWorktreeError::Unavailable),
        }
    }
}

/// Returns bounded Git status and staged/unstaged diff data for one worktree.
pub(crate) async fn review(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root, None).await?;
    let worktree = checked_worktree(&project, worktree_id, None).await?;
    let project_path = project_subdirectory(&worktree, &project)?;
    let status = git_inspection::status(&project_path)
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?;
    let diff = git_inspection::diff(&project_path)
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?;
    Ok(json!({"status": status, "diff": diff}))
}

/// Resolves a validated managed worktree ID to its project subdirectory.
pub(crate) async fn project_root_for_worktree(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<PathBuf, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root, None).await?;
    let worktree = checked_worktree(&project, worktree_id, None).await?;
    project_subdirectory(&worktree, &project)
}

/// Removes a validated managed worktree and its associated checkout directory.
pub(crate) async fn remove(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root, None).await?;
    let worktree = checked_worktree(&project, worktree_id, None).await?;
    let hooks_directory = storage.root().join(format!(
        "git-hooks-disabled-{}",
        canonical_worktree_id(worktree_id).ok_or(GitWorktreeError::InvalidInput)?
    ));
    fs::create_dir(&hooks_directory).map_err(|_| GitWorktreeError::Unavailable)?;
    let worktree_text = git_compatible_path(&worktree);
    let result = run_git(
        &project.repository_root,
        &hooks_directory,
        &["worktree", "remove", "--force", &worktree_text],
        None,
    )
    .await;
    let _ = fs::remove_dir(&hooks_directory);
    result?;
    let branch = format!(
        "openchat/{}",
        canonical_worktree_id(worktree_id).ok_or(GitWorktreeError::InvalidInput)?
    );
    Ok(json!({"id": worktree_id, "branch": branch}))
}

async fn project_location(
    storage: &AppStorage,
    project_id: &str,
    project_path: &str,
    mut control: Option<&mut ListControl<'_>>,
) -> Result<ProjectLocation, GitWorktreeError> {
    if !valid_identifier(project_id) || project_path.len() > 4096 {
        return Err(GitWorktreeError::InvalidInput);
    }
    let project_root =
        fs::canonicalize(project_path).map_err(|_| GitWorktreeError::ProjectUnavailable)?;
    if !project_root.is_dir() {
        return Err(GitWorktreeError::ProjectUnavailable);
    }
    let output = git_text(
        &project_root,
        &["rev-parse", "--show-toplevel"],
        control.as_deref_mut(),
    )
    .await?;
    let repository_root = fs::canonicalize(output).map_err(|_| GitWorktreeError::NotRepository)?;
    let project_subdirectory = project_root
        .strip_prefix(&repository_root)
        .map_err(|_| GitWorktreeError::NotRepository)?
        .to_path_buf();
    let storage_root =
        fs::canonicalize(storage.root()).map_err(|_| GitWorktreeError::Unavailable)?;
    if repository_root.starts_with(&storage_root) || storage_root.starts_with(&repository_root) {
        return Err(GitWorktreeError::UnsafePath);
    }
    let worktree_base = storage_root.join("worktrees").join(project_id);
    Ok(ProjectLocation {
        repository_root,
        project_subdirectory,
        worktree_base,
    })
}

fn project_subdirectory(
    checkout: &Path,
    project: &ProjectLocation,
) -> Result<PathBuf, GitWorktreeError> {
    let path = checkout.join(&project.project_subdirectory);
    let canonical_base = fs::canonicalize(checkout).map_err(|_| GitWorktreeError::UnsafePath)?;
    let canonical_path = fs::canonicalize(path).map_err(|_| GitWorktreeError::UnsafePath)?;
    if !canonical_path.starts_with(canonical_base) {
        return Err(GitWorktreeError::UnsafePath);
    }
    Ok(canonical_path)
}

async fn checked_worktree(
    project: &ProjectLocation,
    id: &str,
    mut control: Option<&mut ListControl<'_>>,
) -> Result<PathBuf, GitWorktreeError> {
    let id = canonical_worktree_id(id).ok_or(GitWorktreeError::InvalidInput)?;
    if !ensure_worktree_base(project, false)? {
        return Err(GitWorktreeError::ProjectUnavailable);
    }
    let path = project.worktree_base.join(&id);
    let metadata = fs::symlink_metadata(&path).map_err(|_| GitWorktreeError::ProjectUnavailable)?;
    if !metadata.file_type().is_dir() {
        return Err(GitWorktreeError::UnsafePath);
    }
    let canonical_path = fs::canonicalize(&path).map_err(|_| GitWorktreeError::UnsafePath)?;
    let canonical_base =
        fs::canonicalize(&project.worktree_base).map_err(|_| GitWorktreeError::UnsafePath)?;
    if canonical_path.parent() != Some(canonical_base.as_path()) {
        return Err(GitWorktreeError::UnsafePath);
    }
    let common_dir = git_text(
        &canonical_path,
        &["rev-parse", "--git-common-dir"],
        control.as_deref_mut(),
    )
    .await?;
    let worktree_common_dir = canonical_path.join(common_dir);
    let worktree_common_dir =
        fs::canonicalize(worktree_common_dir).map_err(|_| GitWorktreeError::UnsafePath)?;
    let project_common_dir = git_text(
        &project.repository_root,
        &["rev-parse", "--git-common-dir"],
        control.as_deref_mut(),
    )
    .await?;
    let project_common_dir = fs::canonicalize(project.repository_root.join(project_common_dir))
        .map_err(|_| GitWorktreeError::UnsafePath)?;
    if worktree_common_dir != project_common_dir {
        return Err(GitWorktreeError::UnsafePath);
    }
    Ok(canonical_path)
}

fn ensure_worktree_base(project: &ProjectLocation, create: bool) -> Result<bool, GitWorktreeError> {
    let storage_root = project
        .worktree_base
        .parent()
        .and_then(Path::parent)
        .ok_or(GitWorktreeError::UnsafePath)?;
    let worktrees_root = project
        .worktree_base
        .parent()
        .ok_or(GitWorktreeError::UnsafePath)?;
    if create {
        match fs::create_dir(worktrees_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(GitWorktreeError::Unavailable),
        }
    }
    let worktrees_metadata = match fs::symlink_metadata(worktrees_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => return Ok(false),
        Err(_) => return Err(GitWorktreeError::Unavailable),
    };
    if !worktrees_metadata.file_type().is_dir() {
        return Err(GitWorktreeError::UnsafePath);
    }
    let canonical_storage =
        fs::canonicalize(storage_root).map_err(|_| GitWorktreeError::UnsafePath)?;
    let canonical_worktrees =
        fs::canonicalize(worktrees_root).map_err(|_| GitWorktreeError::UnsafePath)?;
    if canonical_worktrees.parent() != Some(canonical_storage.as_path()) {
        return Err(GitWorktreeError::UnsafePath);
    }
    if create {
        match fs::create_dir(&project.worktree_base) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(GitWorktreeError::Unavailable),
        }
    }
    let base_metadata = match fs::symlink_metadata(&project.worktree_base) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => return Ok(false),
        Err(_) => return Err(GitWorktreeError::Unavailable),
    };
    if !base_metadata.file_type().is_dir() {
        return Err(GitWorktreeError::UnsafePath);
    }
    let canonical_base =
        fs::canonicalize(&project.worktree_base).map_err(|_| GitWorktreeError::UnsafePath)?;
    if canonical_base.parent() != Some(canonical_worktrees.as_path()) {
        return Err(GitWorktreeError::UnsafePath);
    }
    Ok(true)
}

fn canonical_worktree_id(value: &str) -> Option<String> {
    Uuid::parse_str(value)
        .ok()
        .map(|id| id.simple().to_string())
}

fn git_compatible_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc_path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        if let Some(path) = path.strip_prefix(r"\\?\") {
            return path.to_owned();
        }
    }
    path.into_owned()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

async fn git_text(
    root: &Path,
    arguments: &[&str],
    control: Option<&mut ListControl<'_>>,
) -> Result<String, GitWorktreeError> {
    let output = run_git(root, root, arguments, control).await?;
    String::from_utf8(output)
        .map(|value| value.trim().to_owned())
        .map_err(|_| GitWorktreeError::Unavailable)
}

async fn run_git(
    root: &Path,
    hooks_directory: &Path,
    arguments: &[&str],
    mut control: Option<&mut ListControl<'_>>,
) -> Result<Vec<u8>, GitWorktreeError> {
    if let Some(control) = control.as_deref_mut() {
        control.check()?;
    }
    let mut child = Command::new(git_inspection::git_program())
        .args(["-c", "core.fsmonitor=false", "-c"])
        .arg(format!(
            "core.hooksPath={}",
            git_compatible_path(hooks_directory)
        ))
        .args(["--no-pager"])
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| GitWorktreeError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(GitWorktreeError::Unavailable)?;
    let stderr = child.stderr.take().ok_or(GitWorktreeError::Unavailable)?;
    let mut stdout_task = tokio::spawn(read_bounded(stdout));
    let mut stderr_task = tokio::spawn(read_bounded(stderr));
    let status_result = match control.as_deref_mut() {
        Some(control) => git_inspection::wait_for_git_process_with_cancellation(
            &mut child,
            control.cancellation,
            control.deadline.min(Instant::now() + COMMAND_TIMEOUT),
        )
        .await
        .map_err(map_git_inspection_error),
        None => match timeout(COMMAND_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) => Ok(status),
            Ok(Err(_)) => Err(GitWorktreeError::Unavailable),
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                Err(GitWorktreeError::TimedOut)
            }
        },
    };
    let status = match status_result {
        Ok(status) => status,
        Err(error) => {
            stdout_task.abort();
            stderr_task.abort();
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(error);
        }
    };
    let stdout_result = match control.as_deref_mut() {
        Some(control) => wait_for_list_reader(&mut stdout_task, control).await,
        None => (&mut stdout_task)
            .await
            .map_err(|_| GitWorktreeError::Unavailable)?
            .map_err(|_| GitWorktreeError::Unavailable),
    };
    let (stdout, stdout_truncated) = match stdout_result {
        Ok(output) => output,
        Err(error) => {
            stdout_task.abort();
            stderr_task.abort();
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(error);
        }
    };
    let stderr_result = match control.as_deref_mut() {
        Some(control) => wait_for_list_reader(&mut stderr_task, control).await,
        None => (&mut stderr_task)
            .await
            .map_err(|_| GitWorktreeError::Unavailable)?
            .map_err(|_| GitWorktreeError::Unavailable),
    };
    let (_stderr, stderr_truncated) = match stderr_result {
        Ok(output) => output,
        Err(error) => {
            stderr_task.abort();
            let _ = stderr_task.await;
            return Err(error);
        }
    };
    if stdout_truncated || stderr_truncated {
        return Err(GitWorktreeError::Unavailable);
    }
    if !status.success() {
        return Err(GitWorktreeError::GitFailed);
    }
    Ok(stdout)
}

async fn wait_for_list_reader<T>(
    task: &mut JoinHandle<std::io::Result<T>>,
    control: &mut ListControl<'_>,
) -> Result<T, GitWorktreeError> {
    loop {
        control.check()?;
        tokio::select! {
            biased;
            changed = control.cancellation.changed() => {
                if changed.is_err() || *control.cancellation.borrow() {
                    return Err(GitWorktreeError::Cancelled);
                }
            }
            _ = sleep_until(control.deadline) => return Err(GitWorktreeError::TimedOut),
            result = &mut *task => {
                return result
                    .map_err(|_| GitWorktreeError::Unavailable)?
                    .map_err(|_| GitWorktreeError::Unavailable);
            }
        }
    }
}

fn map_git_inspection_error(error: git_inspection::GitInspectionError) -> GitWorktreeError {
    match error {
        git_inspection::GitInspectionError::Cancelled => GitWorktreeError::Cancelled,
        git_inspection::GitInspectionError::TimedOut => GitWorktreeError::TimedOut,
        git_inspection::GitInspectionError::NotRepository
        | git_inspection::GitInspectionError::Unavailable => GitWorktreeError::Unavailable,
    }
}

async fn read_bounded<R: AsyncRead + Unpin>(mut reader: R) -> std::io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::with_capacity(1024);
    let mut buffer = [0u8; 2048];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = MAX_OUTPUT_BYTES.saturating_sub(output.len());
        let retained = read.min(remaining);
        output.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    Ok((output, truncated))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };

    use super::{
        MAX_WORKTREES, create, list, project_root_for_worktree, remove, retain_worktree_id, review,
    };
    use crate::{git_inspection, storage::AppStorage};
    use tokio::sync::watch;

    struct Fixture {
        root: std::path::PathBuf,
        storage: AppStorage,
        repository: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "openchat-worktrees-{}",
                uuid::Uuid::new_v4().simple()
            ));
            let repository = root.join("repo");
            fs::create_dir_all(&repository).expect("create test repository");
            fs::write(repository.join("README.md"), "before\n").expect("write tracked file");
            fs::create_dir_all(repository.join(".openchat"))
                .expect("create project task directory");
            fs::write(
                repository.join(".openchat/tasks.json"),
                r#"{"version":1,"tasks":[{"id":"verify","command":"echo verify","timeoutSeconds":30}]}"#,
            )
            .expect("write project task catalog");
            let status = Command::new(git_inspection::git_program())
                .args(["init", "--quiet"])
                .current_dir(&repository)
                .status()
                .expect("initialize test repository");
            assert!(status.success());
            let status = Command::new(git_inspection::git_program())
                .args(["config", "core.autocrlf", "false"])
                .current_dir(&repository)
                .status()
                .expect("configure test repository line endings");
            assert!(status.success());
            let status = Command::new(git_inspection::git_program())
                .args(["add", "README.md", ".openchat/tasks.json"])
                .current_dir(&repository)
                .status()
                .expect("stage initial file");
            assert!(status.success());
            let status = Command::new(git_inspection::git_program())
                .args([
                    "-c",
                    "user.name=OpenChat Test",
                    "-c",
                    "user.email=test@openchat.invalid",
                    "commit",
                    "--allow-empty",
                    "--quiet",
                    "-m",
                    "initial",
                ])
                .current_dir(&repository)
                .status()
                .expect("create initial commit");
            assert!(status.success());
            let storage = AppStorage::open_at(root.join("storage")).expect("open test storage");
            Self {
                root,
                storage,
                repository,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn direct_worktree_directory_count(base: &Path) -> usize {
        fs::read_dir(base)
            .expect("read worktree base")
            .map(|entry| {
                entry
                    .expect("read worktree directory entry")
                    .file_type()
                    .expect("read worktree entry type")
            })
            .filter(|file_type| file_type.is_dir())
            .count()
    }

    #[tokio::test]
    async fn creates_reviews_lists_and_removes_an_isolated_worktree() {
        let fixture = Fixture::new();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let project_path = fixture.repository.to_string_lossy();
        let created = create(&fixture.storage, "project-1", &project_path)
            .await
            .expect("create a worktree");
        let id = created["id"].as_str().expect("worktree id");
        assert!(
            created["branch"]
                .as_str()
                .is_some_and(|branch| branch.starts_with("openchat/"))
        );

        let listed = list(
            &fixture.storage,
            "project-1",
            &project_path,
            &mut cancellation,
        )
        .await
        .expect("list worktrees");
        assert_eq!(listed["worktrees"].as_array().map(Vec::len), Some(1));
        assert_eq!(listed["worktrees"][0]["id"], id);

        let worktree_path = created["path"].as_str().expect("worktree path");
        let task_root = project_root_for_worktree(&fixture.storage, "project-1", &project_path, id)
            .await
            .expect("resolve task root for the selected worktree");
        let tasks = crate::tools::project_tasks::load_project_tasks(&task_root)
            .expect("load task catalog from the selected worktree");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].command, "echo verify");
        fs::write(PathBuf::from(worktree_path).join("README.md"), "after\n")
            .expect("edit worktree file");
        let listed = list(
            &fixture.storage,
            "project-1",
            &project_path,
            &mut cancellation,
        )
        .await
        .expect("list worktree changes");
        assert_eq!(
            listed["worktrees"][0]["status"]["files"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );

        let reviewed = review(&fixture.storage, "project-1", &project_path, id)
            .await
            .expect("review worktree");
        assert_eq!(
            reviewed["status"]["files"].as_array().map(Vec::len),
            Some(1)
        );
        assert!(
            reviewed["diff"]["unstaged"]
                .as_str()
                .is_some_and(|diff| diff.contains("after"))
        );

        remove(&fixture.storage, "project-1", &project_path, id)
            .await
            .expect("remove worktree");
        let listed = list(
            &fixture.storage,
            "project-1",
            &project_path,
            &mut cancellation,
        )
        .await
        .expect("list after removal");
        assert_eq!(listed["worktrees"].as_array().map(Vec::len), Some(0));
    }

    #[tokio::test]
    async fn list_stops_before_starting_when_the_request_is_already_cancelled() {
        let fixture = Fixture::new();
        let (_cancellation_sender, mut cancellation) = watch::channel(true);
        let project_path = fixture.repository.to_string_lossy();

        let error = list(
            &fixture.storage,
            "project-1",
            &project_path,
            &mut cancellation,
        )
        .await
        .expect_err("an already cancelled request must not start list work");

        assert!(matches!(error, super::GitWorktreeError::Cancelled));
    }

    #[test]
    fn worktree_selection_is_stable_and_bounded() {
        let candidates = (1..=MAX_WORKTREES + 5)
            .map(|value| uuid::Uuid::from_u128(value as u128).simple().to_string())
            .collect::<Vec<_>>();
        let mut forward = BTreeSet::new();
        let mut forward_truncated = false;
        for id in candidates.iter().cloned() {
            retain_worktree_id(&mut forward, &mut forward_truncated, id);
        }
        let mut reverse = BTreeSet::new();
        let mut reverse_truncated = false;
        for id in candidates.iter().rev().cloned() {
            retain_worktree_id(&mut reverse, &mut reverse_truncated, id);
        }

        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), MAX_WORKTREES);
        assert!(forward_truncated);
        assert!(reverse_truncated);
        assert_eq!(
            forward.into_iter().collect::<Vec<_>>(),
            candidates[..MAX_WORKTREES]
        );
    }

    #[tokio::test]
    async fn create_rejects_when_the_managed_worktree_limit_is_reached() {
        let fixture = Fixture::new();
        let base = fixture.storage.root().join("worktrees").join("project-1");
        fs::create_dir_all(&base).expect("create worktree base");
        for value in 1..=MAX_WORKTREES {
            let id = uuid::Uuid::from_u128(value as u128).simple().to_string();
            fs::create_dir(base.join(id)).expect("create existing worktree directory");
        }
        let project_path = fixture.repository.to_string_lossy();

        let error = create(&fixture.storage, "project-1", &project_path)
            .await
            .expect_err("creation must stop at the managed worktree limit");

        assert!(matches!(error, super::GitWorktreeError::LimitReached));
        assert_eq!(direct_worktree_directory_count(&base), MAX_WORKTREES);
    }

    #[tokio::test]
    async fn concurrent_creates_cannot_exceed_the_managed_worktree_limit() {
        let fixture = Fixture::new();
        let base = fixture.storage.root().join("worktrees").join("project-1");
        fs::create_dir_all(&base).expect("create worktree base");
        for value in 1..MAX_WORKTREES {
            let id = uuid::Uuid::from_u128(value as u128).simple().to_string();
            fs::create_dir(base.join(id)).expect("create existing worktree directory");
        }
        let project_path = fixture.repository.to_string_lossy().into_owned();

        let (first, second) = tokio::join!(
            create(&fixture.storage, "project-1", &project_path),
            create(&fixture.storage, "project-1", &project_path),
        );

        assert!(first.is_ok() ^ second.is_ok());
        let failed = if first.is_err() {
            first.err()
        } else {
            second.err()
        };
        assert!(matches!(
            failed,
            Some(super::GitWorktreeError::LimitReached)
        ));
        assert_eq!(direct_worktree_directory_count(&base), MAX_WORKTREES);
    }

    #[tokio::test]
    async fn worktree_creation_file_lock_waits_for_another_handle() {
        let fixture = Fixture::new();
        let base = fixture.storage.root().join("worktrees").join("project-1");
        fs::create_dir_all(&base).expect("create worktree base");
        let first = super::acquire_worktree_creation_file_lock(&base)
            .await
            .expect("acquire first worktree creation lock");

        let second_attempt = tokio::time::timeout(
            Duration::from_millis(150),
            super::acquire_worktree_creation_file_lock(&base),
        )
        .await;
        assert!(second_attempt.is_err());

        drop(first);
        let second = tokio::time::timeout(
            Duration::from_secs(1),
            super::acquire_worktree_creation_file_lock(&base),
        )
        .await
        .expect("second worktree creation lock should not time out")
        .expect("acquire worktree creation lock after release");
        drop(second);
    }
}
