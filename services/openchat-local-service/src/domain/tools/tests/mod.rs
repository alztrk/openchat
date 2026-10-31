//! Integration-style tests for tool permissions, scoped I/O, and execution flows.
//!
//! Tests use isolated temporary workspaces and explicit tool approval to verify
//! the externally observable service behavior.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::executor::ToolExecutionContext;
use super::search::{ascii_shift_table, contains_ascii_case_insensitive};
use super::{get_file_info, list_files, read_file, search_files};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const TOOL_TEST_CONVERSATION_ID: &str = "tools-test-conversation";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("openchat-tools-test-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }

    fn root(&self) -> &str {
        self.0.to_str().expect("temporary path is valid UTF-8")
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.0.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create test subdirectory");
        }
        fs::write(path, content).expect("write test file");
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

mod benchmarks;
mod executor;
mod file_ops;
mod listing;
mod project;
mod search;
mod terminal;
mod web_search;
mod wire;
