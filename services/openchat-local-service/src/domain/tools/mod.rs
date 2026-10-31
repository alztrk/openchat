//! Defines tool schemas and coordinates scoped file, project, web, and terminal operations.
//!
//! Public tool functions enforce bounded inputs and path scope; provider schemas
//! are derived from the active request's model, permissions, and project context.

pub(super) use std::{
    collections::{BinaryHeap, HashSet},
    fs,
    io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

pub(super) use cap_std::fs::{Dir, OpenOptions};
#[cfg(test)]
pub(super) use ignore::WalkBuilder;
pub(super) use ignore::gitignore::{Gitignore, GitignoreBuilder};
pub(super) use serde_json::{Value, json};

pub(super) use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

mod search;
pub use search::search_files;

#[cfg(windows)]
pub(crate) mod computer_use;
#[cfg(windows)]
pub(crate) mod mcp;
pub(crate) mod project_tasks;
pub(crate) mod project_tools;
pub mod terminal;
pub mod web_search;

const MAX_LIST_RESULTS: usize = 100;
const MAX_LIST_OFFSET: usize = 100_000;
const MAX_LIST_SCAN: usize = 100_000;
const MAX_GITIGNORE_ANCESTORS: usize = 512;
const MAX_GITIGNORE_FILES: usize = 512;
const MAX_GITIGNORE_FILE_BYTES: usize = 1024 * 1024;
const MAX_GITIGNORE_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_GITIGNORE_LINE_BYTES: usize = 4 * 1024;
const MAX_GITIGNORE_LINES: usize = 10_000;
const MAX_READ_LINES: usize = 500;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DIRECT_READ_BYTES: u64 = MAX_OUTPUT_BYTES as u64 * 1024;
const MAX_EDIT_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const MAX_TOOL_ARGUMENT_BYTES: usize = 16 * 1024;
pub(crate) const MAX_TOOL_CALLS_PER_RESPONSE: usize = 16;
const MAX_PROFILE_TOOL_ALLOWLIST: usize = 32;

mod allowlist;
mod definitions;
mod file_ops;
mod listing;
mod paths;
mod wire;

pub(crate) use allowlist::*;
pub use definitions::*;
pub use file_ops::*;
pub use listing::*;
pub(crate) use paths::*;
pub use wire::*;

mod executor;
pub(crate) use executor::ImageGenerationContext;
pub(crate) use executor::ToolExecutionContext;
pub(crate) use executor::tool_response_limit_error;
pub use executor::{ToolExecutor, ToolPermissionMode};
pub(crate) use executor::{
    ToolPermissionRule, ToolPermissionRules, parse_tool_permission_rules,
    project_task_permission_rule,
};

#[cfg(test)]
mod image_tool_tests;

#[cfg(test)]
mod tests;
