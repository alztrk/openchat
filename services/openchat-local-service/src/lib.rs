//! Local OpenChat sidecar services and their process-level RPC entry point.
//!
//! The public surface is intentionally limited to starting the service and,
//! on Windows, accessing credentials stored by the operating system.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]

// This direct dependency enables ORT's copy-dylibs feature for fastembed on Windows.
#[cfg(windows)]
use ort as _;

mod domain;
pub(crate) use domain::{
    attachments, chat_operation, chatgpt, chatgpt_store, connector_packages, connectors,
    context_compaction, conversation_archive, copilot_oauth, document_parser, file_changes,
    git_inspection, git_worktrees, goals, history, hugging_face, image_library, instructions,
    local_engines, oauth, openai_api, openai_compatible, pending_archive_export, permissions,
    profile_archive, project_index, project_instructions, project_skills, provider_credentials,
    provider_schema, response_budget, temporary_conversation, tools, usage_statistics,
    user_question_broker, workspace_settings,
};

#[path = "adapters/filesystem.rs"]
mod filesystem;
#[path = "transport/protocol.rs"]
mod protocol;
#[path = "transport/rpc.rs"]
mod rpc;
#[path = "bootstrap/service.rs"]
mod service;
#[path = "adapters/storage/mod.rs"]
mod storage;
#[cfg(windows)]
#[path = "storage_broker.rs"]
mod storage_broker;

#[cfg(windows)]
#[path = "adapters/credentials.rs"]
/// Windows Credential Manager access for OAuth tokens and scoped MCP secrets.
pub mod credentials;

/// Protected provider secrets storage.
pub mod secrets;

/// Cross-platform process isolation and sandboxing.
pub mod sandbox;

pub use service::{RunError, run};
pub use storage::migrate_chat_schema;
#[cfg(windows)]
pub use storage_broker::run_broker_stdio;

/// Chat schema version supported by this service build.
pub const CHAT_SCHEMA_VERSION: i64 = 22;
