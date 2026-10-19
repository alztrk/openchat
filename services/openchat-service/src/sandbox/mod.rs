//! Cross-platform process isolation and sandbox facilities.
//!
//! Windows supports dual sandbox backends:
//! 1. Native Windows AppContainer with Job Objects (zero-setup default).
//! 2. WSL2 Linux container backend (for enterprise Linux kernel boundary).

#[cfg(windows)]
mod appcontainer;
#[cfg(windows)]
mod job_objects;
#[cfg(windows)]
mod process;
#[cfg(windows)]
mod security_acl;
#[cfg(windows)]
mod types;

#[cfg(all(windows, test))]
mod tests;

#[cfg(windows)]
pub use process::SandboxedProcess;
#[cfg(windows)]
pub use types::{SandboxJobLimits, SandboxOptions};

use std::path::PathBuf;

/// Defines the security boundaries, accessible paths, and network policy for sandboxed execution.
#[derive(Clone, Debug, Default)]
pub struct SandboxPolicy {
    /// Workspace root directory where write operations are permitted.
    pub workspace_root: PathBuf,
    /// Additional directories granted read-only access (tools, runtimes, SDKs).
    pub read_only_paths: Vec<PathBuf>,
    /// Whether outbound network access to public registries is permitted.
    pub allow_network: bool,
    /// Optional limit on the number of concurrent child processes.
    pub max_processes: Option<u32>,
    /// Optional memory limit in megabytes.
    pub memory_limit_mb: Option<usize>,
    /// Selected sandbox isolation engine backend.
    pub backend: SandboxBackend,
}

/// The isolation backend engine to enforce for process execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackend {
    /// Native OS kernel boundary (Windows AppContainer, Linux Landlock, macOS Seatbelt).
    #[default]
    Native,
    /// WSL2 Hyper-V Linux VM isolation (available on Windows hosts with WSL2 installed).
    Wsl2,
}

/// Status of sandbox support on the current runtime platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxSupport {
    /// Native kernel-level sandbox is fully supported and enforced.
    Native,
    /// Platform is supported via process-isolation fallback.
    Fallback,
    /// Sandbox engine is scheduled on the platform roadmap.
    Roadmap,
}

impl SandboxPolicy {
    /// Returns the active sandbox implementation status for the current operating system.
    #[must_use]
    pub const fn current_platform_support() -> SandboxSupport {
        #[cfg(windows)]
        {
            SandboxSupport::Native
        }
        #[cfg(target_os = "linux")]
        {
            SandboxSupport::Roadmap
        }
        #[cfg(target_os = "macos")]
        {
            SandboxSupport::Roadmap
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            SandboxSupport::Roadmap
        }
    }

    /// Detects whether WSL2 is available on the current Windows host.
    #[must_use]
    pub fn is_wsl2_available() -> bool {
        #[cfg(windows)]
        {
            std::process::Command::new("wsl.exe")
                .arg("--status")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}
