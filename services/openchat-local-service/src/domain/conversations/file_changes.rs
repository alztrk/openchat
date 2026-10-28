//! Captures and reads file-change snapshots produced during conversation tools.
//!
//! Snapshot blobs are content-addressed under application storage, while
//! manifest paths are validated relative paths and writes use atomic replacement.

#[path = "file_changes/blobs.rs"]
pub(crate) mod blobs;
#[path = "file_changes/checkpoints.rs"]
pub(crate) mod checkpoints;
#[path = "file_changes/diff.rs"]
pub(crate) mod diff;
#[path = "file_changes/revert.rs"]
pub(crate) mod revert;
#[path = "file_changes/store.rs"]
pub(crate) mod store;
#[path = "file_changes/types.rs"]
pub(crate) mod types;
#[path = "file_changes/workspace.rs"]
pub(crate) mod workspace;

#[cfg(test)]
#[path = "file_changes/tests.rs"]
mod tests;

pub(crate) use blobs::*;
pub(crate) use diff::*;
pub(crate) use revert::*;
pub(crate) use store::*;
pub(crate) use types::*;
pub(crate) use workspace::*;
