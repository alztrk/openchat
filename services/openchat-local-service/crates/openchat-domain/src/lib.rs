//! OpenChat domain layer — pure business logic with no I/O dependencies.
//!
//! Contains all domain types, value objects, and business rules.
//! Zero framework dependencies — only serde, uuid, thiserror.

pub mod conversation;
pub mod provider;
pub mod model;
pub mod project;
pub mod tool;
pub mod settings;

/// Domain error types
pub mod error {
    use thiserror::Error;

    /// Errors that can occur in domain operations.
    #[derive(Error, Debug, Clone, PartialEq, Eq)]
    pub enum DomainError {
        /// Invalid input was provided.
        #[error("invalid input: {0}")]
        InvalidInput(String),
        /// The requested entity was not found.
        #[error("not found: {0}")]
        NotFound(String),
        /// A conflict occurred (e.g., duplicate entry).
        #[error("conflict: {0}")]
        Conflict(String),
        /// Validation failed.
        #[error("validation failed: {0}")]
        ValidationError(String),
    }

    /// Result type for domain operations.
    pub type DomainResult<T> = Result<T, DomainError>;
}
