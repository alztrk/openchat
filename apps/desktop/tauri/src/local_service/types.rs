//! Types, limits, and shared state for local-service process and permission management.

use std::{
    borrow::Cow,
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc, Mutex,
    },
};

use serde::{
    de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::Value;
use tauri_plugin_shell::process::CommandChild;

pub(crate) const LOCAL_SERVICE_EVENT: &str = "openchat:local-service";
pub(crate) const LOCAL_SERVICE_ERROR_PREFIX: &str = "openchat-local-service-error: ";
pub(crate) const MAX_LOCAL_SERVICE_ERROR_DETAIL_BYTES: usize = 2048;
pub(crate) const MAX_RPC_FRAME_BYTES: usize = 24 * 1024 * 1024;
pub(crate) const MAX_PROJECT_PERMISSION_RPC_FRAME_BYTES: usize = 136 * 1024;
pub(crate) const MAX_PROJECT_PERMISSION_RULE_COUNT: usize = 1_080;
pub(crate) const MAX_PROJECT_PERMISSION_RULE_NAME_BYTES: usize = 96;
pub(crate) const MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES: usize = 8;
pub(crate) const MAX_PENDING_PERMISSIONS: usize = 32;
pub(crate) const MAX_NATIVE_SERVICE_REQUESTS: usize = 64;
pub(crate) const MAX_PERMISSION_TOOL_NAME_BYTES: usize = 128;
pub(crate) const MAX_PERMISSION_TARGET_BYTES: usize = 2 * 1024;
pub(crate) const MAX_PERMISSION_ARGUMENTS_BYTES: usize = 8 * 1024;

/// Keeps one packaged service process alive for the desktop application lifetime.
#[derive(Default)]
pub(crate) struct LocalServiceState {
    pub(crate) process: Arc<Mutex<ProcessState>>,
    pub(crate) permissions: Arc<Mutex<PendingPermissionState>>,
    pub(crate) native_responses: Arc<Mutex<HashMap<String, NativeResponseWaiter>>>,
    pub(crate) permission_policy_confirmation_active: AtomicBool,
    pub(crate) storage_root_override: Option<PathBuf>,
}

/// Routes a private native-host request response without forwarding it to the renderer.
pub(crate) struct NativeResponseWaiter {
    pub(crate) generation: u64,
    pub(crate) sender: SyncSender<Value>,
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
    #[serde(
        rename = "id",
        default,
        deserialize_with = "deserialize_native_request_id"
    )]
    pub(crate) has_native_request_id: bool,
    #[serde(borrow)]
    pub(crate) method: Option<Cow<'a, str>>,
}

fn deserialize_native_request_id<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct NativeRequestIdVisitor;

    impl<'de> Visitor<'de> for NativeRequestIdVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an RPC request ID")
        }

        fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.starts_with("native-"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.starts_with("native-"))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.starts_with("native-"))
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(false)
        }

        fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(false)
        }

        fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(false)
        }

        fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(false)
        }

        fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(false)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            while sequence.next_element::<IgnoredAny>()?.is_some() {}
            Ok(false)
        }

        fn visit_map<A>(self, mut fields: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            while fields.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(false)
        }
    }

    deserializer.deserialize_any(NativeRequestIdVisitor)
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
    #[serde(rename = "nativeConfirmation")]
    pub(crate) native_confirmation: Option<bool>,
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
    #[serde(rename = "nativeConfirmation")]
    pub(crate) native_confirmation: Option<bool>,
}

/// Identifies whether forwarding a renderer RPC needs a native confirmation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RendererRpcDisposition {
    Forward,
    Confirm(PermissionDetails),
    CheckLegacyGlobal {
        imported_full_access: bool,
    },
    CheckLegacyProject {
        project_id: String,
        import_confirmation: Option<PermissionDetails>,
    },
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

/// Carries one stdout frame, safe startup failure, or process-stop notification to the renderer.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum LocalServiceEvent {
    Output { generation: u64, line: String },
    Failure { generation: u64, details: String },
    Stopped { generation: u64 },
}

/// Manages the runtime process and generation state of the local service.
impl LocalServiceState {
    /// Creates a service state whose database lives under an isolated local profile.
    pub(crate) fn with_storage_root(storage_root: PathBuf) -> Self {
        Self {
            storage_root_override: Some(storage_root),
            ..Self::default()
        }
    }

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
