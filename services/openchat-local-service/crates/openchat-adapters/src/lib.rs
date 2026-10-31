//! OpenChat adapters layer — database, filesystem, credentials, sandbox.
//!
//! Implements external-facing interfaces (ports) for domain operations.
//! Currently delegates to openchat-domain's storage and related modules.

#![warn(missing_docs)]

// Re-export domain adapters for compatibility
pub use openchat_domain::storage;
pub use openchat_domain::credentials;
pub use openchat_domain::filesystem;
