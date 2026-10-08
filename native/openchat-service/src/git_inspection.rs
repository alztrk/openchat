use std::{path::Path, process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_STATUS_BYTES: usize = 8 * 1024;
const MAX_DIFF_BYTES: usize = 8 * 1024;
const MAX_HISTORY_BYTES: usize = 8 * 1024;
const MAX_CHANGED_FILES: usize = 200;
const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug)]
pub(crate) enum GitInspectionError {
    NotRepository,
    Unavailable,
    TimedOut,
    InvalidInput,
}

pub(crate) async fn status(root: &Path) -> Result<Value, GitInspectionError> {
    let output = run_git(
        root,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
        ],
        MAX_STATUS_BYTES,
    )
    .await?;
    let complete_output = if output.truncated {
        output
            .stdout
            .iter()
            .rposition(|byte| *byte == 0)
            .map(|end| &output.stdout[..=end])
            .unwrap_or_default()
    } else {
        &output.stdout
    };
    let (branch, upstream, ahead, behind, files, truncated) = parse_status(complete_output);
    Ok(json!({
        "branch": branch,
        "upstream": upstream,
        "ahead": ahead,
        "behind": behind,
        "files": files,
        "truncated": truncated || output.truncated,
    }))
}

pub(crate) async fn diff(root: &Path) -> Result<Value, GitInspectionError> {
    let unstaged = run_git(
        root,
        &["diff", "--no-ext-diff", "--no-textconv", "--unified=3"],
        MAX_DIFF_BYTES,
    )
    .await?;
    let staged = run_git(
        root,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--unified=3",
        ],
        MAX_DIFF_BYTES,
    )
    .await?;

    Ok(json!({
        "unstaged": String::from_utf8_lossy(&unstaged.stdout),
        "unstagedTruncated": unstaged.truncated,
        "staged": String::from_utf8_lossy(&staged.stdout),
        "stagedTruncated": staged.truncated,
    }))
}

pub(crate) async fn history(
    root: &Path,
    requested_limit: usize,
) -> Result<Value, GitInspectionError> {
    if !(1..=MAX_HISTORY_ENTRIES).contains(&requested_limit) {
        return Err(GitInspectionError::InvalidInput);
    }
    let limit = requested_limit.to_string();
    let output = run_git(
        root,
        &[
            "log",
            "--no-color",
            "--format=%H%x1f%ct%x1f%s%x00",
            "-n",
            &limit,
        ],
        MAX_HISTORY_BYTES,
    )
    .await?;
    let entries = parse_history(&output.stdout);
    Ok(json!({
        "entries": entries,
        "truncated": output.truncated,
    }))
}

struct GitOutput {
    stdout: Vec<u8>,
    truncated: bool,
}

async fn run_git(
    root: &Path,
    arguments: &[&str],
    output_limit: usize,
) -> Result<GitOutput, GitInspectionError> {
    let mut child = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(["--no-pager"])
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| GitInspectionError::Unavailable)?;

    let stdout = child.stdout.take().ok_or(GitInspectionError::Unavailable)?;
    let stderr = child.stderr.take().ok_or(GitInspectionError::Unavailable)?;
    let stdout_task = tokio::spawn(read_bounded(stdout, output_limit));
    let stderr_task = tokio::spawn(read_bounded(stderr, 16 * 1024));
    let status = match timeout(COMMAND_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => return Err(GitInspectionError::Unavailable),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(GitInspectionError::TimedOut);
        }
    };
    let (stdout, truncated) = stdout_task
        .await
        .map_err(|_| GitInspectionError::Unavailable)?
        .map_err(|_| GitInspectionError::Unavailable)?;
    let _ = stderr_task
        .await
        .map_err(|_| GitInspectionError::Unavailable)?
        .map_err(|_| GitInspectionError::Unavailable)?;

    if !status.success() {
        let is_repository = Command::new("git")
            .args([
                "-c",
                "core.fsmonitor=false",
                "rev-parse",
                "--is-inside-work-tree",
            ])
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false);
        return Err(if is_repository {
            GitInspectionError::Unavailable
        } else {
            GitInspectionError::NotRepository
        });
    }

    Ok(GitOutput { stdout, truncated })
}

async fn read_bounded<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(output.len());
        let retained = read.min(remaining);
        output.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    Ok((output, truncated))
}

fn parse_status(
    bytes: &[u8],
) -> (
    Option<String>,
    Option<String>,
    Option<usize>,
    Option<usize>,
    Vec<Value>,
    bool,
) {
    let mut branch = None;
    let mut upstream = None;
    let mut ahead = None;
    let mut behind = None;
    let mut files = Vec::new();
    let mut truncated = false;
    let mut fields = bytes.split(|byte| *byte == 0).peekable();

    while let Some(record) = fields.next() {
        if record.is_empty() {
            continue;
        }
        if record.starts_with(b"# ") {
            let header = String::from_utf8_lossy(&record[2..]);
            if let Some(value) = header.strip_prefix("branch.head ") {
                branch = Some(value.to_owned());
            } else if let Some(value) = header.strip_prefix("branch.upstream ") {
                upstream = Some(value.to_owned());
            } else if let Some(value) = header.strip_prefix("branch.ab ") {
                let mut counts = value.split_whitespace();
                ahead = counts
                    .next()
                    .and_then(|value| value.strip_prefix('+'))
                    .and_then(|value| value.parse().ok());
                behind = counts
                    .next()
                    .and_then(|value| value.strip_prefix('-'))
                    .and_then(|value| value.parse().ok());
            }
            continue;
        }

        let record_type = record.first().copied();
        let (status, path, original_path) = match record_type {
            Some(b'?') => ("??".to_owned(), &record[2..], None),
            Some(b'1') => {
                let Some((status, path)) = split_status_record(record, 9) else {
                    truncated = true;
                    continue;
                };
                (status, path, None)
            }
            Some(b'2') => {
                let Some((status, path)) = split_status_record(record, 10) else {
                    truncated = true;
                    continue;
                };
                let original_path = fields.next().filter(|value| !value.is_empty());
                (status, path, original_path)
            }
            Some(b'u') => {
                let Some((status, path)) = split_status_record(record, 11) else {
                    truncated = true;
                    continue;
                };
                (status, path, None)
            }
            _ => continue,
        };

        if files.len() == MAX_CHANGED_FILES {
            truncated = true;
            break;
        }
        let staged = status != "??"
            && status
                .as_bytes()
                .first()
                .is_some_and(|value| *value != b'.');
        let unstaged = status.as_bytes().get(1).is_some_and(|value| *value != b'.');
        files.push(json!({
            "path": String::from_utf8_lossy(path),
            "status": status,
            "staged": staged,
            "unstaged": unstaged,
            "originalPath": original_path.map(String::from_utf8_lossy),
        }));
    }

    (branch, upstream, ahead, behind, files, truncated)
}

fn split_status_record(record: &[u8], field_count: usize) -> Option<(String, &[u8])> {
    let mut fields = record.splitn(field_count, |byte| *byte == b' ');
    let record_type = fields.next()?;
    let status = fields.next()?;
    let path = fields.last()?;
    if record_type.len() != 1 || status.len() != 2 || path.is_empty() {
        return None;
    }
    Some((String::from_utf8_lossy(status).into_owned(), path))
}

fn parse_history(bytes: &[u8]) -> Vec<Value> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            let mut fields = record.splitn(3, |byte| *byte == 0x1f);
            let id = String::from_utf8_lossy(fields.next()?).into_owned();
            let timestamp = String::from_utf8_lossy(fields.next()?)
                .parse::<i64>()
                .ok()?;
            let subject = String::from_utf8_lossy(fields.next()?).into_owned();
            Some(json!({
                "id": id,
                "timestampUnixSeconds": timestamp,
                "subject": subject,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{diff, history, parse_status, status};
    use std::{path::PathBuf, process::Command};

    struct TestRepository(PathBuf);

    impl TestRepository {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "openchat-git-inspection-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&root).expect("create isolated Git repository directory");
            run_git(&root, &["init", "--quiet"]);
            Self(root)
        }

        fn run(&self, arguments: &[&str]) {
            run_git(&self.0, arguments);
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("remove isolated Git repository directory");
        }
    }

    fn run_git(root: &std::path::Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .status()
            .expect("run the local Git test fixture");
        assert!(
            status.success(),
            "Git fixture command failed: {arguments:?}"
        );
    }

    #[test]
    fn parses_porcelain_v2_status_records_and_rename_paths() {
        let bytes = b"# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\01 M. N... 100644 100644 100644 aaaaaaa bbbbbbb src/file.rs\0? new file.txt\02 R. N... 100644 100644 100644 aaaaaaa bbbbbbb R100 src/new.rs\0src/old.rs\0";
        let (branch, upstream, ahead, behind, files, truncated) = parse_status(bytes);

        assert_eq!(branch.as_deref(), Some("main"));
        assert_eq!(upstream.as_deref(), Some("origin/main"));
        assert_eq!(ahead, Some(2));
        assert_eq!(behind, Some(1));
        assert!(!truncated);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0]["path"], "src/file.rs");
        assert!(files[0]["staged"].as_bool().unwrap_or(false));
        assert_eq!(files[1]["path"], "new file.txt");
        assert_eq!(files[2]["path"], "src/new.rs");
        assert_eq!(files[2]["originalPath"], "src/old.rs");
    }

    #[tokio::test]
    async fn reports_non_repository_without_exposing_git_diagnostics() {
        let root = std::env::temp_dir().join(format!(
            "openchat-git-inspection-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&root).expect("create isolated Git inspection directory");

        let error = status(&root)
            .await
            .expect_err("a non-repository must be rejected");
        assert!(matches!(error, super::GitInspectionError::NotRepository));

        std::fs::remove_dir_all(root).expect("remove isolated Git inspection directory");
    }

    #[tokio::test]
    async fn rejects_history_limits_outside_the_supported_range() {
        let error = history(std::path::Path::new("."), 0)
            .await
            .expect_err("zero is not a valid history limit");
        assert!(matches!(error, super::GitInspectionError::InvalidInput));
    }

    #[tokio::test]
    async fn inspects_real_repository_status_diff_and_history() {
        let repository = TestRepository::new();
        std::fs::write(repository.0.join("source.txt"), "initial\n")
            .expect("write initial test file");
        repository.run(&["add", "source.txt"]);
        repository.run(&[
            "-c",
            "user.name=OpenChat Test",
            "-c",
            "user.email=openchat-test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Add initial test file",
        ]);
        std::fs::write(repository.0.join("source.txt"), "changed\n").expect("modify test file");
        std::fs::write(repository.0.join("new file.txt"), "untracked\n")
            .expect("write untracked test file");

        let status = status(&repository.0).await.expect("read Git status");
        assert_eq!(status["files"].as_array().map(Vec::len), Some(2));
        assert!(
            status["files"]
                .as_array()
                .is_some_and(|files| files.iter().any(|file| file["path"] == "source.txt"))
        );

        let diff = diff(&repository.0).await.expect("read Git diff");
        assert!(diff["unstaged"].as_str().unwrap_or("").contains("changed"));

        let history = history(&repository.0, 5).await.expect("read Git history");
        assert_eq!(history["entries"][0]["subject"], "Add initial test file");
    }
}
