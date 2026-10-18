//! Types, limits, and shared state for local-service process and permission management.

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri_plugin_shell::process::CommandChild;

pub(crate) const LOCAL_SERVICE_EVENT: &str = "openchat:local-service";
pub(crate) const MAX_RPC_FRAME_BYTES: usize = 24 * 1024 * 1024;
pub(crate) const MAX_PROJECT_PERMISSION_RPC_FRAME_BYTES: usize = 136 * 1024;
pub(crate) const MAX_PROJECT_PERMISSION_RULE_COUNT: usize = 1_080;
pub(crate) const MAX_PROJECT_PERMISSION_RULE_NAME_BYTES: usize = 96;
pub(crate) const MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES: usize = 8;
pub(crate) const MAX_PENDING_PERMISSIONS: usize = 32;
pub(crate) const MAX_PERMISSION_TOOL_NAME_BYTES: usize = 128;
pub(crate) const MAX_PERMISSION_TARGET_BYTES: usize = 2 * 1024;
pub(crate) const MAX_PERMISSION_ARGUMENTS_BYTES: usize = 8 * 1024;

/// Keeps one packaged service process alive for the desktop application lifetime.
#[derive(Default)]
pub(crate) struct LocalServiceState {
    pub(crate) process: Arc<Mutex<ProcessState>>,
    pub(crate) permissions: Arc<Mutex<PendingPermissionState>>,
    pub(crate) permission_policy_confirmation_active: AtomicBool,
}

/// Serializes process creation, writes, replacement, and shutdown.
#[derive(Default)]
pub(crate) struct ProcessState {
    /// Monotonic identifier used to discard events from an earlier process.
    pub(crate) next_generation: u64,
    /// The only local-service process allowed to exist for this application.
    pub(crate) current: Option<RunningProcess>,
}

/// Associates the owned child with the generation assigned at startup.
pub(crate) struct RunningProcess {
    pub(crate) generation: u64,
    pub(crate) child: CommandChild,
}

/// Holds only bounded permission details captured from the owned service's output.
#[derive(Default)]
pub(crate) struct PendingPermissionState {
    pub(crate) requests: HashMap<String, PendingPermission>,
}

/// Associates a service approval ID with its originating operation and review details.
#[derive(Clone)]
pub(crate) struct PendingPermission {
    pub(crate) generation: u64,
    pub(crate) operation_id: u64,
    pub(crate) details: PermissionDetails,
    pub(crate) responding: bool,
}

/// Describes bounded details that the native host requires before approving a tool action or policy change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PermissionDetails {
    EnableFullAccess,
    AllowProjectRules {
        project_id: String,
        allow_rule_count: usize,
        rule_names: Vec<String>,
        additional_rule_count: usize,
    },
    Reviewable {
        tool_name: String,
        target_path: String,
        arguments: String,
    },
    TooLargeToReview,
}

/// Reads the method field while allowing serde to skip unneeded large renderer parameters.
#[derive(Deserialize)]
pub(crate) struct RendererRpcMethod<'a> {
    #[serde(borrow)]
    pub(crate) method: Option<Cow<'a, str>>,
}

/// Reads only the permission fields needed to recognize privileged mode changes.
#[derive(Deserialize)]
pub(crate) struct RendererRpcPermissionMode<'a> {
    #[serde(borrow)]
    pub(crate) method: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub(crate) params: Option<RendererRpcPermissionModeParams<'a>>,
}

/// Reads permission-mode values while skipping unrelated request fields.
#[derive(Default, Deserialize)]
pub(crate) struct RendererRpcPermissionModeParams<'a> {
    #[serde(borrow)]
    pub(crate) mode: Option<Cow<'a, str>>,
    #[serde(borrow, rename = "initialMode")]
    pub(crate) initial_mode: Option<Cow<'a, str>>,
}

/// Reads project permission fields while borrowing their bounded JSON strings.
#[derive(Deserialize)]
pub(crate) struct RendererRpcProjectPermission<'a> {
    #[serde(borrow)]
    pub(crate) method: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub(crate) params: Option<RendererRpcProjectPermissionParams<'a>>,
}

/// Reads only the project ID and mode map for a project permission RPC.
#[derive(Default, Deserialize)]
pub(crate) struct RendererRpcProjectPermissionParams<'a> {
    #[serde(borrow, rename = "projectId")]
    pub(crate) project_id: Option<&'a str>,
    #[serde(borrow)]
    pub(crate) rules: Option<HashMap<&'a str, &'a str>>,
    #[serde(borrow, rename = "initialRules")]
    pub(crate) initial_rules: Option<HashMap<&'a str, &'a str>>,
}

/// Identifies whether forwarding a renderer RPC needs a native confirmation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RendererRpcDisposition {
    Forward,
    Confirm(PermissionDetails),
}

/// Releases the single-flight guard even if an OS dialog future is cancelled.
pub(crate) struct PermissionPolicyConfirmationGuard<'a>(pub(crate) &'a AtomicBool);

impl<'a> PermissionPolicyConfirmationGuard<'a> {
    /// Acquires the guard or rejects a concurrent attempt to open another dialog.
    pub(crate) fn acquire(active: &'a AtomicBool) -> Result<Self, String> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(active))
            .map_err(|_| "A permission policy confirmation is already open.".to_owned())
    }
}

impl Drop for PermissionPolicyConfirmationGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Carries one stdout frame or one process-stop notification to the renderer.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum LocalServiceEvent {
    Output { generation: u64, line: String },
    Stopped { generation: u64 },
}

/// Starts the packaged service once and returns its generation identifier.

impl LocalServiceState {
    /// Stops the owned process during application shutdown.
    pub(crate) fn stop(&self) -> Result<(), String> {
        let child = self
            .process
            .lock()
            .map_err(|_| "Local service state is unavailable.")?
            .current
            .take()
            .map(|process| process.child);
        if let Some(child) = child {
            child
                .kill()
                .map_err(|_| "The local service did not stop cleanly.")?;
        }
        self.permissions
            .lock()
            .map_err(|_| "Local permission state is unavailable.")?
            .requests
            .clear();
        Ok(())
    }
}