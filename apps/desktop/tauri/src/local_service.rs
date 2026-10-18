//! Owns the packaged local-service process and mediates renderer access to it.

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

const LOCAL_SERVICE_EVENT: &str = "openchat:local-service";
const MAX_RPC_FRAME_BYTES: usize = 24 * 1024 * 1024;
const MAX_PROJECT_PERMISSION_RPC_FRAME_BYTES: usize = 136 * 1024;
const MAX_PROJECT_PERMISSION_RULE_COUNT: usize = 1_080;
const MAX_PROJECT_PERMISSION_RULE_NAME_BYTES: usize = 96;
const MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES: usize = 8;
const MAX_PENDING_PERMISSIONS: usize = 32;
const MAX_PERMISSION_TOOL_NAME_BYTES: usize = 128;
const MAX_PERMISSION_TARGET_BYTES: usize = 2 * 1024;
const MAX_PERMISSION_ARGUMENTS_BYTES: usize = 8 * 1024;

/// Keeps one packaged service process alive for the desktop application lifetime.
#[derive(Default)]
pub(crate) struct LocalServiceState {
    process: Arc<Mutex<ProcessState>>,
    permissions: Arc<Mutex<PendingPermissionState>>,
    permission_policy_confirmation_active: AtomicBool,
}

/// Serializes process creation, writes, replacement, and shutdown.
#[derive(Default)]
struct ProcessState {
    /// Monotonic identifier used to discard events from an earlier process.
    next_generation: u64,
    /// The only local-service process allowed to exist for this application.
    current: Option<RunningProcess>,
}

/// Associates the owned child with the generation assigned at startup.
struct RunningProcess {
    generation: u64,
    child: CommandChild,
}

/// Holds only bounded permission details captured from the owned service's output.
#[derive(Default)]
struct PendingPermissionState {
    requests: HashMap<String, PendingPermission>,
}

/// Associates a service approval ID with its originating operation and review details.
#[derive(Clone)]
struct PendingPermission {
    generation: u64,
    operation_id: u64,
    details: PermissionDetails,
    responding: bool,
}

/// Describes bounded details that the native host requires before approving a tool action or policy change.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PermissionDetails {
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
struct RendererRpcMethod<'a> {
    #[serde(borrow)]
    method: Option<Cow<'a, str>>,
}

/// Reads only the permission fields needed to recognize privileged mode changes.
#[derive(Deserialize)]
struct RendererRpcPermissionMode<'a> {
    #[serde(borrow)]
    method: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    params: Option<RendererRpcPermissionModeParams<'a>>,
}

/// Reads permission-mode values while skipping unrelated request fields.
#[derive(Default, Deserialize)]
struct RendererRpcPermissionModeParams<'a> {
    #[serde(borrow)]
    mode: Option<Cow<'a, str>>,
    #[serde(borrow, rename = "initialMode")]
    initial_mode: Option<Cow<'a, str>>,
}

/// Reads project permission fields while borrowing their bounded JSON strings.
#[derive(Deserialize)]
struct RendererRpcProjectPermission<'a> {
    #[serde(borrow)]
    method: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    params: Option<RendererRpcProjectPermissionParams<'a>>,
}

/// Reads only the project ID and mode map for a project permission RPC.
#[derive(Default, Deserialize)]
struct RendererRpcProjectPermissionParams<'a> {
    #[serde(borrow, rename = "projectId")]
    project_id: Option<&'a str>,
    #[serde(borrow)]
    rules: Option<HashMap<&'a str, &'a str>>,
    #[serde(borrow, rename = "initialRules")]
    initial_rules: Option<HashMap<&'a str, &'a str>>,
}

/// Identifies whether forwarding a renderer RPC needs a native confirmation.
#[derive(Debug, PartialEq, Eq)]
enum RendererRpcDisposition {
    Forward,
    Confirm(PermissionDetails),
}

/// Releases the single-flight guard even if an OS dialog future is cancelled.
struct PermissionPolicyConfirmationGuard<'a>(&'a AtomicBool);

impl<'a> PermissionPolicyConfirmationGuard<'a> {
    /// Acquires the guard or rejects a concurrent attempt to open another dialog.
    fn acquire(active: &'a AtomicBool) -> Result<Self, String> {
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
enum LocalServiceEvent {
    Output { generation: u64, line: String },
    Stopped { generation: u64 },
}

/// Starts the packaged service once and returns its generation identifier.
#[tauri::command]
pub(crate) fn start_local_service(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
) -> Result<u64, String> {
    let mut process = state
        .process
        .lock()
        .map_err(|_| "Local service state is unavailable.")?;
    if let Some(current) = process.current.as_ref() {
        return Ok(current.generation);
    }

    let generation = process
        .next_generation
        .checked_add(1)
        .ok_or("The local service generation limit was reached.")?;
    let command = app
        .shell()
        .sidecar("binaries/openchat_service")
        .map_err(|_| "The packaged local service could not be located.")?;
    let (mut events, child) = command
        .spawn()
        .map_err(|_| "The local service could not be started.")?;
    process.next_generation = generation;
    process.current = Some(RunningProcess { generation, child });

    let shared_process = Arc::clone(&state.process);
    let shared_permissions = Arc::clone(&state.permissions);
    tauri::async_runtime::spawn(async move {
        let mut terminate_child = true;
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => match String::from_utf8(bytes) {
                    Ok(line) => {
                        observe_service_output(&shared_permissions, generation, &line);
                        let output = LocalServiceEvent::Output { generation, line };
                        if let Err(error) = app.emit_to("main", LOCAL_SERVICE_EVENT, output) {
                            eprintln!("OpenChat local service output could not reach the renderer: {error}");
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                },
                CommandEvent::Error(_) => break,
                CommandEvent::Terminated(_) => {
                    terminate_child = false;
                    break;
                }
                CommandEvent::Stderr(_) => {}
                _ => {}
            }
        }

        let child = match shared_process.lock() {
            Ok(mut process)
                if process
                    .current
                    .as_ref()
                    .is_some_and(|item| item.generation == generation) =>
            {
                process.current.take().map(|item| item.child)
            }
            Ok(_) => None,
            Err(error) => {
                eprintln!(
                    "OpenChat local service state could not be locked after process exit: {error}"
                );
                None
            }
        };
        if terminate_child {
            if let Some(child) = child {
                if let Err(error) = child.kill() {
                    eprintln!("OpenChat local service could not be stopped after a bridge failure: {error}");
                }
            }
        }
        clear_generation_permissions(&shared_permissions, generation);
        let stopped = LocalServiceEvent::Stopped { generation };
        if let Err(error) = app.emit_to("main", LOCAL_SERVICE_EVENT, stopped) {
            eprintln!("OpenChat local service stop event could not reach the renderer: {error}");
        }
    });

    Ok(generation)
}

/// Writes one bounded, newline-terminated JSON-RPC frame to the owned service process.
#[tauri::command]
pub(crate) async fn write_local_service(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
    generation: u64,
    line: String,
    locale: String,
) -> Result<(), String> {
    let disposition = renderer_rpc_disposition(&line, MAX_RPC_FRAME_BYTES)
        .ok_or_else(|| "The local service request frame is invalid or too large.".to_owned())?;
    if let RendererRpcDisposition::Confirm(details) = disposition {
        let _confirmation_guard = PermissionPolicyConfirmationGuard::acquire(
            &state.permission_policy_confirmation_active,
        )?;
        let confirmed = confirm_permission_with_os(app, &details, &locale).await?;
        if !confirmed {
            return Err("The permission policy change requires native confirmation.".to_owned());
        }
    }

    let mut process = state
        .process
        .lock()
        .map_err(|_| "Local service state is unavailable.")?;
    let Some(current) = process.current.as_mut() else {
        return Err("The local service is not running.".to_owned());
    };
    if current.generation != generation {
        return Err("The local service connection is no longer current.".to_owned());
    }
    current
        .child
        .write(line.as_bytes())
        .map_err(|_| "The local service connection was interrupted.".to_owned())
}

/// Sends a permission decision only for a live service request and requires native consent to allow it.
#[tauri::command]
pub(crate) async fn respond_local_service_permission(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
    generation: u64,
    approval_request_id: String,
    approved: bool,
    locale: String,
) -> Result<bool, String> {
    let process = Arc::clone(&state.process);
    let permissions = Arc::clone(&state.permissions);
    let request = begin_permission_response(&permissions, generation, &approval_request_id)?;
    let approved = if approved {
        match confirm_permission_with_os(app, &request.details, &locale).await {
            Ok(approved) => approved,
            Err(error) => {
                release_permission_response(&permissions, generation, &approval_request_id);
                return Err(error);
            }
        }
    } else {
        false
    };

    if let Err(error) = deliver_permission_response(
        &process,
        &permissions,
        generation,
        &approval_request_id,
        approved,
    ) {
        release_permission_response(&permissions, generation, &approval_request_id);
        return Err(error);
    }
    Ok(approved)
}

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

/// Accepts one non-empty, newline-terminated frame within the renderer bridge's byte budget.
fn is_valid_rpc_frame(frame: &str, maximum_bytes: usize) -> bool {
    if frame.len() > maximum_bytes || !frame.ends_with('\n') {
        return false;
    }
    let body = &frame.as_bytes()[..frame.len() - 1];
    !body.is_empty() && !body.contains(&b'\n') && !body.contains(&b'\r')
}

/// Blocks renderer-forged decisions and marks full-access mode changes for OS confirmation.
fn renderer_rpc_disposition(frame: &str, maximum_bytes: usize) -> Option<RendererRpcDisposition> {
    if !is_valid_rpc_frame(frame, maximum_bytes) {
        return None;
    }
    let body = &frame[..frame.len() - 1];
    let Ok(request) = serde_json::from_str::<RendererRpcMethod<'_>>(body) else {
        return Some(RendererRpcDisposition::Forward);
    };
    let Some(method) = request.method.as_deref() else {
        return Some(RendererRpcDisposition::Forward);
    };
    if method == "chat.tool.permission.respond" {
        return None;
    }
    if matches!(
        method,
        "tools.permissions.mode.set" | "tools.permissions.mode.get"
    ) {
        let request = serde_json::from_str::<RendererRpcPermissionMode<'_>>(body).ok()?;
        if request.method.as_deref() != Some(method) {
            return None;
        }
        let Some(params) = request.params else {
            return Some(RendererRpcDisposition::Forward);
        };
        let requests_full_access = match method {
            "tools.permissions.mode.set" => params.mode.as_deref() == Some("full_access"),
            "tools.permissions.mode.get" => params.initial_mode.as_deref() == Some("full_access"),
            _ => false,
        };
        return Some(if requests_full_access {
            RendererRpcDisposition::Confirm(PermissionDetails::EnableFullAccess)
        } else {
            RendererRpcDisposition::Forward
        });
    }
    if matches!(
        method,
        "project.tool_permissions.set" | "project.tool_permissions.get"
    ) {
        return project_permission_rpc_disposition(body, method, frame.len());
    }
    Some(RendererRpcDisposition::Forward)
}

/// Requires native confirmation before a project policy adds or imports any `allow` rule.
fn project_permission_rpc_disposition(
    body: &str,
    method: &str,
    frame_bytes: usize,
) -> Option<RendererRpcDisposition> {
    if !matches!(
        method,
        "project.tool_permissions.set" | "project.tool_permissions.get"
    ) {
        return None;
    }
    if frame_bytes > MAX_PROJECT_PERMISSION_RPC_FRAME_BYTES {
        return None;
    }

    let request = serde_json::from_str::<RendererRpcProjectPermission<'_>>(body).ok()?;
    if request.method.as_deref() != Some(method) {
        return None;
    }
    let params = request.params?;
    let rules = match method {
        "project.tool_permissions.set" => params.rules.as_ref()?,
        "project.tool_permissions.get" => {
            let Some(rules) = params.initial_rules.as_ref() else {
                return Some(RendererRpcDisposition::Forward);
            };
            rules
        }
        _ => return None,
    };
    if rules.len() > MAX_PROJECT_PERMISSION_RULE_COUNT {
        return None;
    }
    let mut allowed_names = Vec::new();
    for (name, rule) in rules {
        if *rule != "allow" {
            continue;
        }
        if name.is_empty()
            || name.len() > MAX_PROJECT_PERMISSION_RULE_NAME_BYTES
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return None;
        }
        allowed_names.push(*name);
    }
    let allow_rule_count = allowed_names.len();
    if allow_rule_count == 0 {
        return Some(RendererRpcDisposition::Forward);
    }

    let project_id = params.project_id?;
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return None;
    }
    allowed_names.sort_unstable();
    let additional_rule_count =
        allow_rule_count.saturating_sub(MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES);
    allowed_names.truncate(MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES);

    Some(RendererRpcDisposition::Confirm(
        PermissionDetails::AllowProjectRules {
            project_id: project_id.to_owned(),
            allow_rule_count,
            rule_names: allowed_names.into_iter().map(str::to_owned).collect(),
            additional_rule_count,
        },
    ))
}

/// Captures service approval details and removes them when their originating operation completes.
fn observe_service_output(state: &Arc<Mutex<PendingPermissionState>>, generation: u64, line: &str) {
    if line.contains("\"event\":\"chat.tool.permission.requested\"") {
        let Ok(response) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let Some(operation_id) = response.get("id").and_then(Value::as_u64) else {
            return;
        };
        let Some(request) = response.get("data").and_then(parse_pending_permission) else {
            return;
        };
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state.requests.len() >= MAX_PENDING_PERMISSIONS
            || state.requests.contains_key(&request.0)
        {
            return;
        }
        state.requests.insert(
            request.0,
            PendingPermission {
                generation,
                operation_id,
                details: request.1,
                responding: false,
            },
        );
        return;
    }

    let Some(operation_id) = parse_response_id(line) else {
        return;
    };
    if let Ok(mut state) = state.lock() {
        state.requests.retain(|_, request| {
            request.generation != generation || request.operation_id != operation_id
        });
    }
}

/// Reads the compact service response ID without parsing or allocating the full response body.
fn parse_response_id(line: &str) -> Option<u64> {
    let body = line.strip_prefix("{\"id\":")?;
    let end = body.find(',')?;
    body[..end].parse().ok()
}

/// Parses an approval ID and retains only bounded, displayable operation details.
fn parse_pending_permission(data: &Value) -> Option<(String, PermissionDetails)> {
    let approval_id = data.get("approvalRequestId")?.as_str()?;
    if !is_valid_approval_id(approval_id) {
        return None;
    }

    let tool_name = bounded_display_text(data.get("toolName")?, MAX_PERMISSION_TOOL_NAME_BYTES);
    let target_path = bounded_display_text(data.get("targetPath")?, MAX_PERMISSION_TARGET_BYTES);
    let arguments = data
        .get("arguments")
        .filter(|value| value.is_object())
        .and_then(|value| serde_json::to_string(value).ok())
        .filter(|value| value.len() <= MAX_PERMISSION_ARGUMENTS_BYTES);
    let details = match (tool_name, target_path, arguments) {
        (Some(tool_name), Some(target_path), Some(arguments)) => PermissionDetails::Reviewable {
            tool_name,
            target_path,
            arguments,
        },
        _ => PermissionDetails::TooLargeToReview,
    };
    Some((approval_id.to_owned(), details))
}

/// Accepts a short, non-empty service field and removes control characters before native display.
fn bounded_display_text(value: &Value, maximum_bytes: usize) -> Option<String> {
    let value = value.as_str()?;
    if value.trim().is_empty() || value.len() > maximum_bytes {
        return None;
    }
    Some(
        value
            .chars()
            .map(|character| {
                if character.is_control() {
                    '\u{fffd}'
                } else {
                    character
                }
            })
            .collect(),
    )
}

/// Accepts only the service's 32-character, hexadecimal single-use approval identifiers.
fn is_valid_approval_id(approval_id: &str) -> bool {
    approval_id.len() == 32 && approval_id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Marks one active request as being answered and returns its host-captured details.
fn begin_permission_response(
    state: &Arc<Mutex<PendingPermissionState>>,
    generation: u64,
    approval_id: &str,
) -> Result<PendingPermission, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Local permission state is unavailable.")?;
    let Some(request) = state.requests.get_mut(approval_id) else {
        return Err("The tool permission request is no longer active.".to_owned());
    };
    if request.generation != generation {
        state.requests.remove(approval_id);
        return Err("The tool permission request is no longer active.".to_owned());
    }
    if request.responding {
        return Err("The tool permission request is already being answered.".to_owned());
    }
    request.responding = true;
    Ok(request.clone())
}

/// Restores an active request after the host could not deliver its response.
fn release_permission_response(
    state: &Arc<Mutex<PendingPermissionState>>,
    generation: u64,
    approval_id: &str,
) {
    if let Ok(mut state) = state.lock() {
        if let Some(request) = state.requests.get_mut(approval_id) {
            if request.generation == generation {
                request.responding = false;
            }
        }
    }
}

/// Delivers one host-authorized response and consumes its single-use approval ID.
fn deliver_permission_response(
    process: &Arc<Mutex<ProcessState>>,
    permissions: &Arc<Mutex<PendingPermissionState>>,
    generation: u64,
    approval_id: &str,
    approved: bool,
) -> Result<(), String> {
    let mut permissions = permissions
        .lock()
        .map_err(|_| "Local permission state is unavailable.")?;
    let Some(request) = permissions.requests.get(approval_id) else {
        return Err("The tool permission request is no longer active.".to_owned());
    };
    if request.generation != generation || !request.responding {
        permissions.requests.remove(approval_id);
        return Err("The tool permission request is no longer active.".to_owned());
    }

    let mut process = match process.lock() {
        Ok(process) => process,
        Err(_) => {
            if let Some(request) = permissions.requests.get_mut(approval_id) {
                request.responding = false;
            }
            return Err("Local service state is unavailable.".to_owned());
        }
    };
    let Some(current) = process.current.as_mut() else {
        permissions.requests.remove(approval_id);
        return Err("The local service is not running.".to_owned());
    };
    if current.generation != generation {
        permissions.requests.remove(approval_id);
        return Err("The local service connection is no longer current.".to_owned());
    }

    let mut line = serde_json::to_string(&json!({
        "id": -1,
        "method": "chat.tool.permission.respond",
        "params": {
            "approvalRequestId": approval_id,
            "approved": approved,
        },
    }))
    .map_err(|_| "The tool permission response could not be encoded.")?;
    line.push('\n');
    if let Err(error) = current.child.write(line.as_bytes()) {
        if let Some(request) = permissions.requests.get_mut(approval_id) {
            request.responding = false;
        }
        return Err(format!(
            "The local service connection was interrupted: {error}"
        ));
    }
    permissions.requests.remove(approval_id);
    Ok(())
}

/// Shows an OS-owned confirmation for a tool call or persistent permission-policy change.
async fn confirm_permission_with_os(
    app: AppHandle,
    details: &PermissionDetails,
    locale: &str,
) -> Result<bool, String> {
    let copy = permission_dialog_copy(locale);
    let (message, buttons, allowed_if_confirmed) = match details {
        PermissionDetails::EnableFullAccess => (
            format!("{}\n\n{}", copy.full_access_intro, copy.full_access_warning),
            MessageDialogButtons::OkCancelCustom(
                copy.enable_full_access.to_owned(),
                copy.deny.to_owned(),
            ),
            true,
        ),
        PermissionDetails::AllowProjectRules {
            project_id,
            allow_rule_count,
            rule_names,
            additional_rule_count,
        } => {
            let additional_rules = if *additional_rule_count == 0 {
                String::new()
            } else {
                copy.project_allow_more
                    .replace("{count}", &additional_rule_count.to_string())
            };
            (
                format!(
                    "{}\n\n{}",
                    copy.project_allow_intro
                        .replace("{project}", project_id)
                        .replace("{count}", &allow_rule_count.to_string())
                        .replace("{rules}", &rule_names.join(", "))
                        .replace("{more}", &additional_rules),
                    copy.project_allow_warning,
                ),
                MessageDialogButtons::OkCancelCustom(
                    copy.save_project_allow.to_owned(),
                    copy.deny.to_owned(),
                ),
                true,
            )
        }
        PermissionDetails::Reviewable {
            tool_name,
            target_path,
            arguments,
        } => (
            format!(
                "{}\n\n{}: {}\n{}: {}\n{}: {}\n\n{}",
                copy.intro,
                copy.tool_label,
                tool_name,
                copy.target_label,
                target_path,
                copy.arguments_label,
                arguments,
                copy.warning,
            ),
            MessageDialogButtons::OkCancelCustom(copy.allow_once.to_owned(), copy.deny.to_owned()),
            true,
        ),
        PermissionDetails::TooLargeToReview => (
            copy.too_large.to_owned(),
            MessageDialogButtons::OkCustom(copy.close.to_owned()),
            false,
        ),
    };
    let title = copy.title.to_owned();
    let app_for_dialog = app.clone();
    let confirmed = tauri::async_runtime::spawn_blocking(move || {
        app_for_dialog
            .dialog()
            .message(message)
            .title(title)
            .buttons(buttons)
            .blocking_show()
    })
    .await
    .map_err(|_| "The operating-system permission dialog could not be shown.")?;
    Ok(allowed_if_confirmed && confirmed)
}

/// Returns fixed operating-system dialog wording for a supported application locale.
fn permission_dialog_copy(locale: &str) -> PermissionDialogCopy {
    match locale.split(['-', '_']).next() {
        Some("tr") => PermissionDialogCopy {
            title: "OpenChat izin isteği",
            intro: "OpenChat tek bir araç çağrısı için izin istiyor.",
            full_access_intro: "Araç izin modu tam erişim olarak değiştirilecek.",
            full_access_warning: "Bu mod dosya araçlarına tüm konumlarda erişim verir ve web ile terminal işlemlerini ek onay olmadan çalıştırır.",
            project_allow_intro: "{project} projesinde şu araçlar için ek onay istemeyen kurallar etkinleştirilecek: {rules}{more} (toplam {count}).",
            project_allow_warning: "Eşleşen araç çağrıları ayrı bir onay istemeden çalışabilir. Bu projeyi tanıyor ve bu izinleri istiyorsanız devam edin.",
            project_allow_more: ", ayrıca {count} kural daha",
            tool_label: "Araç",
            target_label: "Hedef",
            arguments_label: "İşlem ayrıntıları",
            warning: "Yalnızca beklediğiniz işlemle eşleşiyorsa izin verin.",
            allow_once: "Bir kez izin ver",
            enable_full_access: "Tam erişimi etkinleştir",
            save_project_allow: "İzin kurallarını kaydet",
            deny: "Reddet",
            too_large: "İstek güvenli biçimde incelenemeyecek kadar büyük. İstek reddedilecek.",
            close: "Kapat",
        },
        Some("de") => PermissionDialogCopy {
            title: "OpenChat-Berechtigungsanfrage",
            intro: "OpenChat möchte einen Werkzeugaufruf ausführen.",
            full_access_intro: "Der Werkzeugberechtigungsmodus wird auf Vollzugriff gesetzt.",
            full_access_warning: "In diesem Modus können Dateifunktionen auf alle Speicherorte zugreifen und Web- sowie Terminalaktionen ohne weitere Bestätigung ausführen.",
            project_allow_intro: "Für das Projekt {project} werden Regeln aktiviert, die für diese Werkzeuge keine zusätzliche Bestätigung verlangen: {rules}{more} (insgesamt {count}).",
            project_allow_warning: "Passende Werkzeugaufrufe können ohne separate Bestätigung ausgeführt werden. Fahren Sie nur fort, wenn Sie das Projekt kennen und diese Berechtigungen beabsichtigen.",
            project_allow_more: ", außerdem {count} weitere",
            tool_label: "Werkzeug",
            target_label: "Ziel",
            arguments_label: "Details",
            warning: "Erlaube den Aufruf nur, wenn diese Angaben der erwarteten Aktion entsprechen.",
            allow_once: "Einmal erlauben",
            enable_full_access: "Vollzugriff aktivieren",
            save_project_allow: "Erlaubnisregeln speichern",
            deny: "Ablehnen",
            too_large: "Die Anfrage ist zu groß, um sie sicher zu prüfen. Sie wird abgelehnt.",
            close: "Schließen",
        },
        Some("es") => PermissionDialogCopy {
            title: "Solicitud de permiso de OpenChat",
            intro: "OpenChat solicita permiso para ejecutar una herramienta.",
            full_access_intro: "El modo de permisos de herramientas cambiará a acceso completo.",
            full_access_warning: "Este modo permite que las herramientas de archivos accedan a cualquier ubicación y que la Web y el terminal funcionen sin más confirmaciones.",
            project_allow_intro: "El proyecto {project} activará reglas que no piden confirmación adicional para estas herramientas: {rules}{more} (total: {count}).",
            project_allow_warning: "Las llamadas coincidentes pueden ejecutarse sin otra confirmación. Continúa solo si reconoces este proyecto y deseas estos permisos.",
            project_allow_more: " y {count} más",
            tool_label: "Herramienta",
            target_label: "Destino",
            arguments_label: "Detalles",
            warning: "Permite la llamada solo si estos datos coinciden con la acción esperada.",
            allow_once: "Permitir una vez",
            enable_full_access: "Activar acceso completo",
            save_project_allow: "Guardar reglas permitidas",
            deny: "Denegar",
            too_large: "La solicitud es demasiado grande para revisarla con seguridad. Se denegará.",
            close: "Cerrar",
        },
        Some("fr") => PermissionDialogCopy {
            title: "Demande d’autorisation OpenChat",
            intro: "OpenChat demande l’autorisation d’exécuter un outil.",
            full_access_intro: "Le mode d’autorisation des outils passera en accès complet.",
            full_access_warning: "Ce mode permet aux outils de fichiers d’accéder à tous les emplacements et au Web et au terminal de fonctionner sans confirmation supplémentaire.",
            project_allow_intro: "Le projet {project} activera des règles qui ne demandent pas de confirmation supplémentaire pour ces outils : {rules}{more} (total : {count}).",
            project_allow_warning: "Les appels correspondants pourront s’exécuter sans autre confirmation. Continuez uniquement si vous reconnaissez ce projet et souhaitez ces autorisations.",
            project_allow_more: " et {count} de plus",
            tool_label: "Outil",
            target_label: "Cible",
            arguments_label: "Détails",
            warning: "N’autorisez l’appel que si ces informations correspondent à l’action attendue.",
            allow_once: "Autoriser une fois",
            enable_full_access: "Activer l’accès complet",
            save_project_allow: "Enregistrer les règles d’autorisation",
            deny: "Refuser",
            too_large: "La demande est trop volumineuse pour être vérifiée en toute sécurité. Elle sera refusée.",
            close: "Fermer",
        },
        _ => PermissionDialogCopy {
            title: "OpenChat permission request",
            intro: "OpenChat requests permission to run one tool call.",
            full_access_intro: "The tool permission mode will change to full access.",
            full_access_warning: "This mode lets file tools access any location and lets web and terminal operations run without further confirmation.",
            project_allow_intro: "Project {project} will enable rules that skip per-call approval for these tools: {rules}{more} (total: {count}).",
            project_allow_warning: "Matching tool calls may run without separate approval. Continue only if you recognize this project and intend to grant these permissions.",
            project_allow_more: ", plus {count} more",
            tool_label: "Tool",
            target_label: "Target",
            arguments_label: "Arguments",
            warning: "Allow this call only if these details match the action you expect.",
            allow_once: "Allow once",
            enable_full_access: "Enable full access",
            save_project_allow: "Save allow rules",
            deny: "Deny",
            too_large: "The request is too large to review safely. It will be denied.",
            close: "Close",
        },
    }
}

/// Static text used by the native permission confirmation.
struct PermissionDialogCopy {
    title: &'static str,
    intro: &'static str,
    full_access_intro: &'static str,
    full_access_warning: &'static str,
    project_allow_intro: &'static str,
    project_allow_warning: &'static str,
    project_allow_more: &'static str,
    tool_label: &'static str,
    target_label: &'static str,
    arguments_label: &'static str,
    warning: &'static str,
    allow_once: &'static str,
    enable_full_access: &'static str,
    save_project_allow: &'static str,
    deny: &'static str,
    too_large: &'static str,
    close: &'static str,
}

/// Drops every approval captured from a service generation that has stopped.
fn clear_generation_permissions(state: &Arc<Mutex<PendingPermissionState>>, generation: u64) {
    if let Ok(mut state) = state.lock() {
        state
            .requests
            .retain(|_, request| request.generation != generation);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::{
        begin_permission_response, clear_generation_permissions, observe_service_output,
        parse_response_id, renderer_rpc_disposition, PendingPermissionState, PermissionDetails,
        PermissionPolicyConfirmationGuard, RendererRpcDisposition,
    };

    #[test]
    fn full_access_mode_changes_require_native_confirmation() {
        for frame in [
            "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"full_access\"}}\n",
            "{\"id\":1,\"method\":\"tools.permissions.mode.get\",\"params\":{\"initialMode\":\"full_access\"}}\n",
        ] {
            assert_eq!(
                renderer_rpc_disposition(frame, 256),
                Some(RendererRpcDisposition::Confirm(
                    PermissionDetails::EnableFullAccess
                ))
            );
        }
    }

    #[test]
    fn project_allow_rules_require_native_confirmation_for_save_and_import() {
        for frame in [
            "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"allow\"}}}\n",
            "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\",\"initialRules\":{\"read_file\":\"allow\"}}}\n",
        ] {
            assert_eq!(
                renderer_rpc_disposition(frame, 256),
                Some(RendererRpcDisposition::Confirm(
                    PermissionDetails::AllowProjectRules {
                        project_id: "project-1".to_owned(),
                        allow_rule_count: 1,
                        rule_names: vec!["read_file".to_owned()],
                        additional_rule_count: 0,
                    }
                ))
            );
        }
    }

    #[test]
    fn project_confirmation_lists_eight_rules_and_counts_the_rest() {
        let frame = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"edit_file\":\"allow\",\"execute_command\":\"allow\",\"get_file_info\":\"allow\",\"git_diff\":\"allow\",\"git_history\":\"allow\",\"git_status\":\"allow\",\"list_files\":\"allow\",\"read_file\":\"allow\",\"search_files\":\"allow\"}}}\n";

        assert_eq!(
            renderer_rpc_disposition(frame, 1_024),
            Some(RendererRpcDisposition::Confirm(
                PermissionDetails::AllowProjectRules {
                    project_id: "project-1".to_owned(),
                    allow_rule_count: 9,
                    rule_names: vec![
                        "edit_file",
                        "execute_command",
                        "get_file_info",
                        "git_diff",
                        "git_history",
                        "git_status",
                        "list_files",
                        "read_file",
                    ]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                    additional_rule_count: 1,
                }
            ))
        );
    }

    #[test]
    fn restricted_permission_modes_and_unrelated_calls_do_not_need_confirmation() {
        for frame in [
            "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"require_approval\"}}\n",
            "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"approve_safe_operations\"}}\n",
            "{\"id\":1,\"method\":\"tools.permissions.mode.get\",\"params\":{}}\n",
            "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"ask\",\"write_file\":\"deny\"}}}\n",
            "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\",\"initialRules\":{\"read_file\":\"deny\"}}}\n",
            "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\"}}\n",
            "{\"id\":1,\"method\":\"chat.send\",\"params\":{}}\n",
        ] {
            assert_eq!(
                renderer_rpc_disposition(frame, 256),
                Some(RendererRpcDisposition::Forward)
            );
        }
    }

    #[test]
    fn ambiguous_permission_mode_frames_are_rejected() {
        let duplicate_mode = "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"require_approval\",\"mode\":\"full_access\"}}\n";
        let malformed_mode =
            "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":true}}\n";

        assert_eq!(renderer_rpc_disposition(duplicate_mode, 256), None);
        assert_eq!(renderer_rpc_disposition(malformed_mode, 256), None);
    }

    #[test]
    fn malformed_or_out_of_scope_project_permission_frames_are_rejected() {
        let malformed_rule = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":true}}}\n";
        let invalid_project = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"../project\",\"rules\":{\"read_file\":\"allow\"}}}\n";

        assert_eq!(renderer_rpc_disposition(malformed_rule, 512), None);
        assert_eq!(renderer_rpc_disposition(invalid_project, 512), None);
    }

    #[test]
    fn duplicate_project_rule_keys_match_service_last_value_parsing() {
        let last_value_allows = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"ask\",\"read_file\":\"allow\"}}}\n";
        let last_value_denies = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"allow\",\"read_file\":\"deny\"}}}\n";

        assert!(matches!(
            renderer_rpc_disposition(last_value_allows, 512),
            Some(RendererRpcDisposition::Confirm(_))
        ));
        assert_eq!(
            renderer_rpc_disposition(last_value_denies, 512),
            Some(RendererRpcDisposition::Forward)
        );
    }

    #[test]
    fn oversized_project_permission_frames_are_rejected_before_rule_parsing() {
        let padding = "x".repeat(super::MAX_PROJECT_PERMISSION_RPC_FRAME_BYTES);
        let frame = format!(
            "{{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{{\"projectId\":\"project-1\",\"rules\":{{}},\"extra\":\"{padding}\"}}}}\n"
        );

        assert_eq!(
            renderer_rpc_disposition(&frame, super::MAX_RPC_FRAME_BYTES),
            None
        );
    }

    #[test]
    fn permission_policy_confirmation_guard_allows_only_one_live_dialog() {
        let active = std::sync::atomic::AtomicBool::new(false);
        let first =
            PermissionPolicyConfirmationGuard::acquire(&active).expect("acquire first guard");
        assert!(PermissionPolicyConfirmationGuard::acquire(&active).is_err());

        drop(first);
        assert!(PermissionPolicyConfirmationGuard::acquire(&active).is_ok());
    }

    #[test]
    fn rpc_frame_requires_one_newline_terminated_line_within_the_limit() {
        assert_eq!(
            renderer_rpc_disposition("{\"id\":1,\"method\":\"chat.send\"}\n", 40),
            Some(RendererRpcDisposition::Forward)
        );
        assert_eq!(renderer_rpc_disposition("{\"id\":1}", 9), None);
        assert_eq!(renderer_rpc_disposition("{\"id\":1}\n\n", 10), None);
        assert_eq!(renderer_rpc_disposition("{\"id\":1}\r\n", 10), None);
        assert_eq!(renderer_rpc_disposition("{\"id\":1}\n", 8), None);
    }

    #[test]
    fn renderer_cannot_submit_permission_decisions() {
        for approved in [true, false] {
            let frame = format!(
                "{{\"id\":1,\"method\":\"chat.tool.permission.respond\",\"params\":{{\"approved\":{approved}}}}}\n"
            );
            assert_eq!(renderer_rpc_disposition(&frame, 256), None);
        }
        let escaped_method = "{\"id\":1,\"method\":\"chat.tool.permission.\\u0072espond\"}\n";
        assert_eq!(renderer_rpc_disposition(escaped_method, 256), None);
    }

    #[test]
    fn response_id_is_read_without_parsing_the_result_body() {
        assert_eq!(parse_response_id("{\"id\":42,\"result\":{}}\n"), Some(42));
        assert_eq!(parse_response_id("{\"result\":{},\"id\":42}\n"), None);
        assert_eq!(parse_response_id("not a response"), None);
    }

    #[test]
    fn permission_requests_are_tied_to_live_generation_and_operation() {
        let permissions = Arc::new(Mutex::new(PendingPermissionState::default()));
        let event = json!({
            "id": 7,
            "event": "chat.tool.permission.requested",
            "data": {
                "approvalRequestId": "0123456789abcdef0123456789abcdef",
                "toolName": "execute_command",
                "targetPath": "D:/projects/OpenChat",
                "arguments": {"command": "cargo fmt"}
            }
        })
        .to_string();
        observe_service_output(&permissions, 3, &event);

        let request =
            begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef")
                .expect("captured active request can be answered");
        assert!(matches!(
            request.details,
            PermissionDetails::Reviewable { .. }
        ));
        assert!(
            begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",)
                .is_err()
        );

        let result = json!({"id": 7, "result": {"status": "cancelled"}}).to_string();
        observe_service_output(&permissions, 3, &result);
        assert!(
            begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",)
                .is_err()
        );
    }

    #[test]
    fn approval_ids_cannot_cross_service_generations() {
        let permissions = Arc::new(Mutex::new(PendingPermissionState::default()));
        let event = json!({
            "id": 7,
            "event": "chat.tool.permission.requested",
            "data": {
                "approvalRequestId": "0123456789abcdef0123456789abcdef",
                "toolName": "execute_command",
                "targetPath": "D:/projects/OpenChat",
                "arguments": {"command": "cargo fmt"}
            }
        })
        .to_string();
        observe_service_output(&permissions, 3, &event);

        assert!(
            begin_permission_response(&permissions, 4, "0123456789abcdef0123456789abcdef",)
                .is_err()
        );
        assert!(
            begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",)
                .is_err()
        );
    }

    #[test]
    fn stopped_service_generation_cannot_leave_approvable_requests() {
        let permissions = Arc::new(Mutex::new(PendingPermissionState::default()));
        let event = json!({
            "id": 7,
            "event": "chat.tool.permission.requested",
            "data": {
                "approvalRequestId": "0123456789abcdef0123456789abcdef",
                "toolName": "execute_command",
                "targetPath": "D:/projects/OpenChat",
                "arguments": {"command": "cargo fmt"}
            }
        })
        .to_string();
        observe_service_output(&permissions, 3, &event);
        clear_generation_permissions(&permissions, 3);

        assert!(
            begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",)
                .is_err()
        );
    }

    #[test]
    fn oversized_permission_details_cannot_be_approved() {
        let data = json!({
            "approvalRequestId": "0123456789abcdef0123456789abcdef",
            "toolName": "execute_command",
            "targetPath": "D:/projects/OpenChat",
            "arguments": {"command": "x".repeat(9 * 1024)}
        });
        let (_, details) = super::parse_pending_permission(&data).expect("valid approval ID");
        assert!(matches!(details, PermissionDetails::TooLargeToReview));
    }
}
