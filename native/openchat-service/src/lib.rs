mod attachments;
mod chat_operation;
mod chatgpt;
mod chatgpt_store;
mod context_compaction;
mod conversation_archive;
mod file_changes;
mod git_inspection;
mod git_worktrees;
mod goals;
mod history;
mod hugging_face;
mod instructions;
mod local_engines;
mod oauth;
mod openai_api;
mod openai_compatible;
mod permissions;
mod profile_archive;
mod project_instructions;
mod protocol;
mod provider_schema;
mod rpc;
mod service;
mod storage;
mod tools;
mod usage_statistics;
mod user_question_broker;

#[cfg(windows)]
pub mod credentials;

pub use service::{RunError, run};
