//! OpenChat adapters layer — database, filesystem, credentials, sandbox.
//!
//! Implements external-facing interfaces (ports) for domain operations.

#![deny(missing_docs)]

pub mod storage;
pub mod filesystem;
pub mod credentials;
pub mod sandbox;
