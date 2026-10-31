//! OpenChat transport layer — NDJSON RPC protocol implementation.
//!
//! Handles request parsing, response serialization, and the RPC handler dispatch.
//! Currently delegates to openchat-domain's transport module.

#![warn(missing_docs)]

// Re-export domain transport types for compatibility
pub use openchat_domain::protocol;
pub use openchat_domain::rpc;
