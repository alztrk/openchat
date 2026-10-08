use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};
use uuid::Uuid;

use crate::{git_inspection, storage::AppStorage};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_WORKTREES: usize = 20;

#[derive(Debug)]
pub(crate) enum GitWorktreeError {
    InvalidInput,
    ProjectUnavailable,
    NotRepository,
    UnsafePath,
    GitFailed,
    TimedOut,
    Unavailable,
}

struct ProjectLocation {
    repository_root: PathBuf,
    project_subdirectory: PathBuf,
    worktree_base: PathBuf,
}

pub(crate) async fn list(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root).await?;
    let base_exists = ensure_worktree_base(&project, false)?;
    let mut worktrees = Vec::new();
    let mut truncated = false;
    if !base_exists {
        return Ok(json!({"worktrees": [], "truncated": false}));
    }
    let entries = match fs::read_dir(&project.worktree_base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"worktrees": [], "truncated": false}));
        }
        Err(_) => return Err(GitWorktreeError::Unavailable),
    };

    for entry in entries {
        let entry = entry.map_err(|_| GitWorktreeError::Unavailable)?;
        let file_type = entry
            .file_type()
            .map_err(|_| GitWorktreeError::Unavailable)?;
        if !file_type.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if canonical_worktree_id(&id).is_none() {
            continue;
        }
        let worktree = checked_worktree(&project, &id).await?;
        let branch = git_text(&worktree, &["branch", "--show-current"]).await?;
        let status = git_inspection::status(&project_subdirectory(&worktree, &project)?)
            .await
            .map_err(|_| GitWorktreeError::Unavailable)?;
        if worktrees.len() == MAX_WORKTREES {
            truncated = true;
            break;
        }
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
    Ok(json!({"worktrees": worktrees, "truncated": truncated}))
}

pub(crate) async fn create(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root).await?;
    ensure_worktree_base(&project, true)?;
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

pub(crate) async fn review(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root).await?;
    let worktree = checked_worktree(&project, worktree_id).await?;
    let project_path = project_subdirectory(&worktree, &project)?;
    let status = git_inspection::status(&project_path)
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?;
    let diff = git_inspection::diff(&project_path)
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?;
    Ok(json!({"status": status, "diff": diff}))
}

pub(crate) async fn project_root_for_worktree(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<PathBuf, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root).await?;
    let worktree = checked_worktree(&project, worktree_id).await?;
    project_subdirectory(&worktree, &project)
}

pub(crate) async fn remove(
    storage: &AppStorage,
    project_id: &str,
    project_root: &str,
    worktree_id: &str,
) -> Result<Value, GitWorktreeError> {
    let project = project_location(storage, project_id, project_root).await?;
    let worktree = checked_worktree(&project, worktree_id).await?;
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
) -> Result<ProjectLocation, GitWorktreeError> {
    if !valid_identifier(project_id) || project_path.len() > 4096 {
        return Err(GitWorktreeError::InvalidInput);
    }
    let project_root =
        fs::canonicalize(project_path).map_err(|_| GitWorktreeError::ProjectUnavailable)?;
    if !project_root.is_dir() {
        return Err(GitWorktreeError::ProjectUnavailable);
    }
    let output = git_text(&project_root, &["rev-parse", "--show-toplevel"]).await?;
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
    let common_dir = git_text(&canonical_path, &["rev-parse", "--git-common-dir"]).await?;
    let worktree_common_dir = canonical_path.join(common_dir);
    let worktree_common_dir =
        fs::canonicalize(worktree_common_dir).map_err(|_| GitWorktreeError::UnsafePath)?;
    let project_common_dir =
        git_text(&project.repository_root, &["rev-parse", "--git-common-dir"]).await?;
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
    if create && !worktrees_root.exists() {
        fs::create_dir(worktrees_root).map_err(|_| GitWorktreeError::Unavailable)?;
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
    if create && !project.worktree_base.exists() {
        fs::create_dir(&project.worktree_base).map_err(|_| GitWorktreeError::Unavailable)?;
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

async fn git_text(root: &Path, arguments: &[&str]) -> Result<String, GitWorktreeError> {
    let output = run_git(root, root, arguments).await?;
    String::from_utf8(output)
        .map(|value| value.trim().to_owned())
        .map_err(|_| GitWorktreeError::Unavailable)
}

async fn run_git(
    root: &Path,
    hooks_directory: &Path,
    arguments: &[&str],
) -> Result<Vec<u8>, GitWorktreeError> {
    let mut child = Command::new("git")
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
    let stdout_task = tokio::spawn(read_bounded(stdout));
    let stderr_task = tokio::spawn(read_bounded(stderr));
    let status = match timeout(COMMAND_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => return Err(GitWorktreeError::Unavailable),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(GitWorktreeError::TimedOut);
        }
    };
    let (stdout, stdout_truncated) = stdout_task
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?
        .map_err(|_| GitWorktreeError::Unavailable)?;
    let (_stderr, stderr_truncated) = stderr_task
        .await
        .map_err(|_| GitWorktreeError::Unavailable)?
        .map_err(|_| GitWorktreeError::Unavailable)?;
    if stdout_truncated || stderr_truncated {
        return Err(GitWorktreeError::Unavailable);
    }
    if !status.success() {
        return Err(GitWorktreeError::GitFailed);
    }
    Ok(stdout)
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
    use std::{fs, path::PathBuf, process::Command};

    use super::{create, list, project_root_for_worktree, remove, review};
    use crate::storage::AppStorage;

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
            let status = Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(&repository)
                .status()
                .expect("initialize test repository");
            assert!(status.success());
            let status = Command::new("git")
                .args(["add", "README.md", ".openchat/tasks.json"])
                .current_dir(&repository)
                .status()
                .expect("stage initial file");
            assert!(status.success());
            let status = Command::new("git")
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

    #[tokio::test]
    async fn creates_reviews_lists_and_removes_an_isolated_worktree() {
        let fixture = Fixture::new();
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

        let listed = list(&fixture.storage, "project-1", &project_path)
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
        let listed = list(&fixture.storage, "project-1", &project_path)
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
        let listed = list(&fixture.storage, "project-1", &project_path)
            .await
            .expect("list after removal");
        assert_eq!(listed["worktrees"].as_array().map(Vec::len), Some(0));
    }
}
