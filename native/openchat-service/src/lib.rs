mod attachments;
mod chat_operation;
mod chatgpt;
mod chatgpt_store;
mod context_compaction;
mod history;
mod instructions;
mod oauth;
mod openai_api;
mod openai_compatible;
mod permissions;
mod protocol;
mod provider_schema;
mod rpc;
mod service;
mod storage;
mod tools;

#[cfg(windows)]
pub mod credentials;

pub use service::{RunError, run};
