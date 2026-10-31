//! Local OpenChat sidecar services and their process-level RPC entry point.
//!
//! The public surface is intentionally limited to starting the service and,
//! on Windows, accessing credentials stored by the operating system.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]

#[cfg(all(feature = "desktop-profile-test-support", not(debug_assertions)))]
compile_error!("desktop profile test support is unavailable in optimized builds");

//! Domain module re-exports from openchat-domain crate.
pub(crate) use openchat_domain::attachments;
pub(crate) use openchat_domain::chat_operation;
pub(crate) use openchat_domain::chatgpt;
pub(crate) use openchat_domain::chatgpt_store;
pub(crate) use openchat_domain::connector_packages;
pub(crate) use openchat_domain::connectors;
pub(crate) use openchat_domain::context_compaction;
pub(crate) use openchat_domain::conversation_archive;
pub(crate) use openchat_domain::copilot_oauth;
pub(crate) use openchat_domain::document_parser;
pub(crate) use openchat_domain::file_changes;
pub(crate) use openchat_domain::git_inspection;
pub(crate) use openchat_domain::git_worktrees;
pub(crate) use openchat_domain::goals;
pub(crate) use openchat_domain::history;
pub(crate) use openchat_domain::hugging_face;
pub(crate) use openchat_domain::image_library;
pub(crate) use openchat_domain::instructions;
pub(crate) use openchat_domain::local_engines;
pub(crate) use openchat_domain::oauth;
pub(crate) use openchat_domain::openai_api;
pub(crate) use openchat_domain::openai_compatible;
pub(crate) use openchat_domain::pending_archive_export;
pub(crate) use openchat_domain::permissions;
pub(crate) use openchat_domain::profile_archive;
pub(crate) use openchat_domain::project_instructions;
pub(crate) use openchat_domain::project_memory;
pub(crate) use openchat_domain::project_skills;
pub(crate) use openchat_domain::provider_credentials;
pub(crate) use openchat_domain::provider_schema;
pub(crate) use openchat_domain::response_budget;
pub(crate) use openchat_domain::security_scan;
pub(crate) use openchat_domain::temporary_conversation;
pub(crate) use openchat_domain::tools;
pub(crate) use openchat_domain::usage_statistics;
pub(crate) use openchat_domain::user_question_broker;
pub(crate) use openchat_domain::workspace_settings;
pub(crate) use openchat_domain::protocol;

#[path = "transport/rpc.rs"]
mod rpc;
#[path = "bootstrap/service.rs"]
mod service;
#[path = "adapters/storage/mod.rs"]
mod storage;
#[cfg(windows)]
#[path = "storage_broker.rs"]
mod storage_broker;