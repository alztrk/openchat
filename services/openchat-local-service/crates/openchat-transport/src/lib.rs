//! OpenChat transport layer — NDJSON RPC protocol implementation.
//!
//! Handles request parsing, response serialization, and the RPC handler dispatch.

#![deny(missing_docs)]

pub mod protocol;
pub mod rpc;
pub mod bootstrap;
