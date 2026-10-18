//! Windows AppContainer process isolation shared by OpenChat desktop components.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]

mod appcontainer;
mod job_objects;
mod process;
mod security_acl;
mod types;

#[cfg(test)]
mod tests;

pub use process::SandboxedProcess;
pub use types::{SandboxJobLimits, SandboxOptions};
