//! Frontend-invocable Tauri native commands for local service lifecycle and permissions.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use openchat_local_service::StartupFailure;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use zeroize::{Zeroize, Zeroizing};

use super::dialogs::{NativeDialogLocale, PermissionDialogCopy};
use super::process::*;
use super::process_interrupt::ProcessInterrupt;
use super::rpc_frame::*;
use super::types::*;
use super::writer::ServiceWriter;

const MAX_STARTUP_DIAGNOSTIC_LOG_BYTES: u64 = 1024 * 1024;

/// Starts the sidecar during native startup so it can initialize while the WebView loads.
pub(crate) fn start_local_service_early(app: &AppHandle) -> Result<u64, String> {
    let state = app.state::<LocalServiceState>();
    start_local_service_process(app.clone(), &state, false)
}

#[tauri::command]
pub(crate) fn start_local_service(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
) -> Result<u64, String> {
    start_local_service_process(app, &state, false)
}

/// Restarts the owned sidecar with the user's explicit empty-profile choice.
#[tauri::command]
pub(crate) fn start_local_service_with_empty_storage(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
) -> Result<u64, String> {
    state.stop().map_err(|_| {
        startup_failure_response(
            state.storage_root_override.as_deref(),
            "service_stop_failed",
            "process",
            "The local service could not be stopped cleanly.",
            "service_stop_failed",
            true,
        )
    })?;
    start_local_service_process(app, &state, true)
}

fn start_local_service_process(
    app: AppHandle,
    state: &LocalServiceState,
    start_with_empty_storage: bool,
) -> Result<u64, String> {
    let diagnostic_storage_root = state.storage_root_override.clone();
    let mut process = match state.process.lock() {
        Ok(process) => process,
        Err(_) => {
            return Err(startup_failure_response(
                diagnostic_storage_root.as_deref(),
                "service_state_unavailable",
                "unavailable",
                "The local service state is unavailable.",
                "service_state_lock_failed",
                true,
            ));
        }
    };
    if let Some(current) = process.current.as_ref() {
        return Ok(current.generation);
    }

    let Some(generation) = process.next_generation.checked_add(1) else {
        return Err(startup_failure_response(
            diagnostic_storage_root.as_deref(),
            "service_generation_limit_reached",
            "resource_limit",
            "The local service could not start another process.",
            "service_generation_limit_reached",
            false,
        ));
    };
    let parent_pid = std::process::id().to_string();
    let storage_root = state
        .storage_root_override
        .as_ref()
        .map(|root| root.to_string_lossy().into_owned());
    let spawn = |sidecar_name: &str| {
        app.shell().sidecar(sidecar_name).and_then(|mut command| {
            if let Some(root) = storage_root.as_deref() {
                command = command.args(["--data-root", root]);
            }
            let command = if start_with_empty_storage {
                command.args(["--start-with-empty-storage"])
            } else {
                command
            };
            command
                .args(["--parent-pid", parent_pid.as_str()])
                .set_process_group(true)
                .spawn()
        })
    };
    let spawn_result =
        spawn("binaries/openchat-local-service").or_else(|_| spawn("openchat-local-service"));
    let (mut events, child) = match spawn_result {
        Ok(process) => process,
        Err(_) => {
            return Err(startup_failure_response(
                diagnostic_storage_root.as_deref(),
                "service_start_failed",
                "process",
                "The local service could not be started.",
                "sidecar_launch_failed",
                true,
            ));
        }
    };
    #[cfg(feature = "startup-benchmark")]
    crate::record_startup_benchmark_sidecar_spawned(&app);
    let process_id = child.pid();
    let child = Arc::new(Mutex::new(Some(child)));
    let interrupt = match ProcessInterrupt::new(process_id) {
        Ok(interrupt) => Arc::new(interrupt),
        Err(_) => {
            if let Ok(mut child) = child.lock() {
                if let Some(child) = child.take() {
                    let _ = child.kill();
                }
            }
            return Err(startup_failure_response(
                diagnostic_storage_root.as_deref(),
                "service_process_control_failed",
                "process",
                "The local service could not be managed safely.",
                "service_process_control_failed",
                true,
            ));
        }
    };
    let (writer, writer_task) =
        match ServiceWriter::start(Arc::clone(&child), Arc::clone(&interrupt)) {
            Ok(writer) => writer,
            Err(_) => {
                let _ = interrupt.interrupt();
                if let Ok(mut child) = child.lock() {
                    if let Some(child) = child.take() {
                        let _ = child.kill();
                    }
                }
                return Err(startup_failure_response(
                    diagnostic_storage_root.as_deref(),
                    "service_writer_start_failed",
                    "process",
                    "The local service could not be managed safely.",
                    "service_writer_start_failed",
                    true,
                ));
            }
        };
    process.next_generation = generation;
    process.current = Some(RunningProcess {
        generation,
        child,
        interrupt,
        writer,
        writer_task,
    });

    let shared_process = Arc::clone(&state.process);
    let shared_permissions = Arc::clone(&state.permissions);
    let shared_native_responses = Arc::clone(&state.native_responses);
    tauri::async_runtime::spawn(async move {
        let mut stderr_line = Vec::new();
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => match String::from_utf8(bytes) {
                    Ok(mut line) => {
                        if deliver_native_response(&shared_native_responses, generation, &line) {
                            line.zeroize();
                            continue;
                        }
                        observe_service_output(&shared_permissions, generation, &line);
                        let output = LocalServiceEvent::Output { generation, line };
                        if app.emit_to("main", LOCAL_SERVICE_EVENT, output).is_err() {
                            report_local_service_host_error(
                                "protocol",
                                "renderer_output_delivery_failed",
                            );
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                },
                CommandEvent::Error(_) => break,
                CommandEvent::Terminated(_) => break,
                CommandEvent::Stderr(bytes) => {
                    for byte in bytes {
                        if byte == b'\n' {
                            if let Some(failure) = local_service_startup_failure(&stderr_line) {
                                if record_local_service_startup_failure(
                                    diagnostic_storage_root.as_deref(),
                                    &failure,
                                )
                                .is_err()
                                {
                                    eprintln!("local_service_startup_diagnostic_write_failed");
                                }
                                let failure = LocalServiceEvent::Failure {
                                    generation,
                                    failure,
                                };
                                if app.emit_to("main", LOCAL_SERVICE_EVENT, failure).is_err() {
                                    report_local_service_host_error(
                                        "protocol",
                                        "renderer_failure_delivery_failed",
                                    );
                                    break;
                                }
                            }
                            stderr_line.clear();
                        } else if stderr_line.len() < MAX_LOCAL_SERVICE_STARTUP_FAILURE_BYTES {
                            stderr_line.push(byte);
                        }
                    }
                }
                _ => {}
            }
        }

        let running = match shared_process.lock() {
            Ok(mut process)
                if process
                    .current
                    .as_ref()
                    .is_some_and(|item| item.generation == generation) =>
            {
                process.current.take()
            }
            Ok(_) => None,
            Err(_) => {
                report_local_service_host_error("process", "service_state_lock_failed_after_exit");
                None
            }
        };
        if let Some(running) = running {
            let cleanup = tauri::async_runtime::spawn_blocking(move || running.terminate()).await;
            if !matches!(cleanup, Ok(Ok(()))) {
                report_local_service_host_error("process", "service_termination_failed");
            }
        }
        clear_generation_permissions(&shared_permissions, generation);
        clear_native_responses(&shared_native_responses, generation);
        let stopped = LocalServiceEvent::Stopped { generation };
        if app.emit_to("main", LOCAL_SERVICE_EVENT, stopped).is_err() {
            report_local_service_host_error("protocol", "renderer_stop_delivery_failed");
        }
    });

    Ok(generation)
}

/// Sends one host-owned request to the service and waits for its correlated response.
pub(crate) async fn request_local_service(
    state: &LocalServiceState,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    if !matches!(
        method,
        "storage.settings.get"
            | "storage.settings.get_all"
            | "storage.settings.set"
            | "storage.settings.delete"
            | "internal.secrets.map.read"
            | "internal.secrets.map.write"
            | "internal.tool_permissions.global_status"
            | "internal.tool_permissions.global_confirm"
            | "internal.tool_permissions.project_status"
            | "internal.tool_permissions.project_confirm"
    ) {
        return Err("The native host is not allowed to call this local-service method.".to_owned());
    }

    let request_id = format!("native-{}", uuid::Uuid::new_v4().simple());
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut request = json!({
        "id": request_id.clone(),
        "method": method,
        "params": params,
    });
    let encoded = serde_json::to_vec(&request)
        .map_err(|_| "The local-service request could not be encoded.".to_owned());
    zeroize_json_value(&mut request);
    let mut frame = Zeroizing::new(encoded?);
    frame.push(b'\n');
    if frame.len() > MAX_RPC_FRAME_BYTES {
        return Err("The native local-service request is too large.".to_owned());
    }

    let (generation, writer) = {
        let process = state
            .process
            .lock()
            .map_err(|_| "Local service state is unavailable.".to_owned())?;
        let Some(current) = process.current.as_ref() else {
            return Err("The local service is not running.".to_owned());
        };
        (current.generation, current.writer.clone())
    };
    {
        let mut pending = state
            .native_responses
            .lock()
            .map_err(|_| "Local service response state is unavailable.".to_owned())?;
        if pending.len() >= MAX_NATIVE_SERVICE_REQUESTS {
            return Err("The local service has too many pending native requests.".to_owned());
        }
        pending.insert(
            request_id.clone(),
            NativeResponseWaiter { generation, sender },
        );
    }
    let _response_guard = NativeResponseGuard {
        pending: Arc::clone(&state.native_responses),
        request_id: request_id.clone(),
    };
    write_service_frame(&state.process, generation, &writer, frame).await?;

    let mut response = match tauri::async_runtime::spawn_blocking(move || {
        receiver.recv_timeout(Duration::from_secs(15))
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(_)) => {
            if let Ok(mut pending) = state.native_responses.lock() {
                pending.remove(&request_id);
            }
            return Err("The local service did not answer the native request in time.".to_owned());
        }
        Err(_) => {
            if let Ok(mut pending) = state.native_responses.lock() {
                pending.remove(&request_id);
            }
            return Err("The local service request could not be completed.".to_owned());
        }
    };

    if response
        .get("error")
        .and_then(Value::as_object)
        .is_some_and(|error| error.contains_key("code"))
    {
        let code = response["error"]["code"]
            .as_str()
            .unwrap_or("service_error")
            .to_owned();
        let message = response["error"]["message"]
            .as_str()
            .unwrap_or("The local service request failed.")
            .to_owned();
        zeroize_json_value(&mut response);
        return Err(format!("{code}: {message}"));
    }

    response
        .as_object_mut()
        .and_then(|response| response.remove("result"))
        .ok_or_else(|| {
            zeroize_json_value(&mut response);
            "The local service returned an invalid native response.".to_owned()
        })
}

fn deliver_native_response(
    pending: &Arc<std::sync::Mutex<std::collections::HashMap<String, NativeResponseWaiter>>>,
    generation: u64,
    line: &str,
) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(request_id) = value.get("id").and_then(Value::as_str) else {
        return false;
    };
    if !request_id.starts_with("native-") {
        return false;
    }
    let waiter = match pending.lock() {
        Ok(mut pending) => match pending.get(request_id) {
            Some(waiter) if waiter.generation == generation => pending.remove(request_id),
            _ => None,
        },
        Err(_) => None,
    };
    let Some(waiter) = waiter else {
        let mut value = value;
        zeroize_json_value(&mut value);
        return true;
    };
    if let Err(error) = waiter.sender.send(value) {
        let mut response = error.0;
        zeroize_json_value(&mut response);
    }
    true
}

struct NativeResponseGuard {
    pending: Arc<std::sync::Mutex<std::collections::HashMap<String, NativeResponseWaiter>>>,
    request_id: String,
}

impl Drop for NativeResponseGuard {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&self.request_id);
        }
    }
}

fn clear_native_responses(
    pending: &Arc<std::sync::Mutex<std::collections::HashMap<String, NativeResponseWaiter>>>,
    generation: u64,
) {
    if let Ok(mut pending) = pending.lock() {
        pending.retain(|_, waiter| {
            if waiter.generation == generation {
                let _ = waiter.sender.send(json!({
                    "error": {
                        "code": "service_unavailable",
                        "message": "The local service stopped before completing the native request."
                    }
                }));
                false
            } else {
                true
            }
        });
    }
}

fn zeroize_json_value(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(values) => {
            for value in values {
                zeroize_json_value(value);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                zeroize_json_value(value);
            }
        }
        _ => {}
    }
}

fn local_service_startup_failure(line: &[u8]) -> Option<StartupFailure> {
    let line = String::from_utf8_lossy(line);
    let payload = line
        .trim_end_matches('\r')
        .strip_prefix(LOCAL_SERVICE_ERROR_PREFIX)?
        .trim();
    if payload.is_empty() {
        return None;
    }
    let mut failure = serde_json::from_str::<StartupFailure>(payload).ok()?;
    let valid_code = |value: &str| {
        !value.is_empty()
            && value.len() <= 96
            && value.as_bytes()[0].is_ascii_lowercase()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    let valid_category = matches!(
        failure.category.as_str(),
        "validation"
            | "authentication"
            | "authorization"
            | "not_found"
            | "conflict"
            | "rate_limit"
            | "network"
            | "provider"
            | "storage"
            | "filesystem"
            | "process"
            | "protocol"
            | "timeout"
            | "resource_limit"
            | "cancelled"
            | "unavailable"
            | "internal"
    );
    let valid_reference = failure.correlation_id.len() == 32
        && failure
            .correlation_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let valid_message = !failure.message.is_empty()
        && failure.message.len() <= 512
        && !failure.message.chars().any(char::is_control);
    if !valid_code(&failure.code)
        || !valid_category
        || !valid_reference
        || !valid_message
        || failure
            .diagnostic_code
            .as_deref()
            .is_some_and(|code| !valid_code(code))
    {
        return None;
    }
    failure.message = "The local service could not be started.".to_owned();
    Some(failure)
}

fn record_local_service_startup_failure(
    storage_root: Option<&Path>,
    failure: &StartupFailure,
) -> std::io::Result<()> {
    let root = match storage_root {
        Some(root) => root.to_path_buf(),
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "the local application data directory is unavailable",
            )
        })?)
        .join("OpenChat"),
    };
    let log_directory = root.join("logs");
    fs::create_dir_all(&log_directory)?;
    let log_path = log_directory.join("openchat-service.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_millis();
    let diagnostic_code = failure.diagnostic_code.as_deref().unwrap_or("none");
    let entry = format!(
        "timestamp_unix_ms={timestamp} pid={} component=local_service event=startup_failed category={} code={} diagnostic_code={} correlation_id={} retryable={} message={}\n",
        std::process::id(),
        failure.category,
        failure.code,
        diagnostic_code,
        failure.correlation_id,
        failure.retryable,
        failure.message,
    );
    if entry.len() > 4096 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "startup diagnostic entry exceeded its size limit",
        ));
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let entry_size = u64::try_from(entry.len()).map_err(std::io::Error::other)?;
    if file.metadata()?.len().saturating_add(entry_size) > MAX_STARTUP_DIAGNOSTIC_LOG_BYTES {
        file.set_len(0)?;
    }
    file.write_all(entry.as_bytes())?;
    file.flush()
}

fn startup_failure_response(
    storage_root: Option<&Path>,
    code: &str,
    category: &str,
    message: &str,
    diagnostic_code: &str,
    retryable: bool,
) -> String {
    let failure = StartupFailure {
        code: code.to_owned(),
        category: category.to_owned(),
        message: message.to_owned(),
        diagnostic_code: Some(diagnostic_code.to_owned()),
        correlation_id: uuid::Uuid::new_v4().simple().to_string(),
        retryable,
    };
    if record_local_service_startup_failure(storage_root, &failure).is_err() {
        eprintln!("local_service_startup_diagnostic_write_failed");
    }
    match serialize_startup_failure(&failure) {
        Ok(payload) => payload,
        Err(_) => "startup_failure_serialization_failed".to_owned(),
    }
}

fn serialize_startup_failure(failure: &StartupFailure) -> Result<String, serde_json::Error> {
    serde_json::to_string(failure)
}

pub(crate) fn report_local_service_host_error(category: &str, code: &str) {
    let correlation_id = uuid::Uuid::new_v4().simple();
    eprintln!(
        "component=local_service category={category} code={code} correlation_id={correlation_id}"
    );
}

/// Writes one bounded, newline-terminated JSON-RPC frame to the owned service process.
#[tauri::command]
pub(crate) async fn write_local_service(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
    generation: u64,
    line: String,
    permission_locale: NativeDialogLocale,
) -> Result<(), String> {
    let disposition = renderer_rpc_disposition(&line, MAX_RPC_FRAME_BYTES)
        .ok_or_else(|| "The local service request frame is invalid or too large.".to_owned())?;
    let mut forwarded_line = line;
    match disposition {
        RendererRpcDisposition::Forward => {}
        RendererRpcDisposition::Confirm(details) => {
            let _confirmation_guard = PermissionPolicyConfirmationGuard::acquire(
                &state.permission_policy_confirmation_active,
            )?;
            let permission_copy =
                PermissionDialogCopy::for_locale(permission_locale).map_err(str::to_owned)?;
            permission_copy.validate().map_err(str::to_owned)?;
            let confirmed = confirm_permission_with_os(app, &details, &permission_copy).await?;
            if !confirmed {
                return Err("The permission policy change requires native confirmation.".to_owned());
            }
            forwarded_line = add_native_confirmation(&forwarded_line)?;
        }
        RendererRpcDisposition::CheckLegacyGlobal {
            imported_full_access,
        } => {
            let _confirmation_guard = PermissionPolicyConfirmationGuard::acquire(
                &state.permission_policy_confirmation_active,
            )?;
            let confirmation =
                confirm_legacy_global_policy(&app, &state, permission_locale, imported_full_access)
                    .await?;
            match confirmation {
                LegacyGlobalPolicyConfirmation::ExistingPolicy => {}
                LegacyGlobalPolicyConfirmation::ConfirmedImport => {
                    forwarded_line = add_native_confirmation(&forwarded_line)?;
                }
                LegacyGlobalPolicyConfirmation::DeclinedImport => {
                    return Err(
                        "The permission policy change requires native confirmation.".to_owned()
                    );
                }
            }
        }
        RendererRpcDisposition::CheckLegacyProject {
            project_id,
            import_confirmation,
        } => {
            let _confirmation_guard = PermissionPolicyConfirmationGuard::acquire(
                &state.permission_policy_confirmation_active,
            )?;
            let has_saved_policy =
                confirm_legacy_project_policy(&app, &state, permission_locale, &project_id).await?;
            if !has_saved_policy {
                if let Some(details) = import_confirmation {
                    let permission_copy = PermissionDialogCopy::for_locale(permission_locale)
                        .map_err(str::to_owned)?;
                    permission_copy.validate().map_err(str::to_owned)?;
                    let confirmed =
                        confirm_permission_with_os(app, &details, &permission_copy).await?;
                    if !confirmed {
                        return Err(
                            "The permission policy change requires native confirmation.".to_owned()
                        );
                    }
                    forwarded_line = add_native_confirmation(&forwarded_line)?;
                }
            }
        }
    }

    let writer = {
        let process = state
            .process
            .lock()
            .map_err(|_| "Local service state is unavailable.")?;
        let Some(current) = process.current.as_ref() else {
            return Err("The local service is not running.".to_owned());
        };
        if current.generation != generation {
            return Err("The local service connection is no longer current.".to_owned());
        }
        current.writer.clone()
    };
    write_service_frame(
        &state.process,
        generation,
        &writer,
        Zeroizing::new(forwarded_line.into_bytes()),
    )
    .await
}

enum LegacyGlobalPolicyConfirmation {
    ExistingPolicy,
    ConfirmedImport,
    DeclinedImport,
}

async fn confirm_legacy_global_policy(
    app: &AppHandle,
    state: &LocalServiceState,
    locale: NativeDialogLocale,
    imported_full_access: bool,
) -> Result<LegacyGlobalPolicyConfirmation, String> {
    let status =
        request_local_service(state, "internal.tool_permissions.global_status", json!({})).await?;
    let needs_confirmation = status
        .get("needsConfirmation")
        .and_then(Value::as_bool)
        .ok_or("The local service returned invalid permission status.")?;
    let has_saved_policy = status
        .get("hasSavedPolicy")
        .and_then(Value::as_bool)
        .ok_or("The local service returned invalid permission status.")?;
    if needs_confirmation && !has_saved_policy {
        return Err("The local service returned invalid permission status.".to_owned());
    }

    if needs_confirmation {
        let copy = PermissionDialogCopy::for_locale(locale).map_err(str::to_owned)?;
        copy.validate().map_err(str::to_owned)?;
        let confirmed =
            confirm_permission_with_os(app.clone(), &PermissionDetails::EnableFullAccess, &copy)
                .await?;
        if confirmed {
            let result =
                request_local_service(state, "internal.tool_permissions.global_confirm", json!({}))
                    .await?;
            if result.get("confirmed").and_then(Value::as_bool).is_none() {
                return Err("The local service returned invalid permission status.".to_owned());
            }
        }
    }

    if has_saved_policy || !imported_full_access {
        return Ok(LegacyGlobalPolicyConfirmation::ExistingPolicy);
    }
    let copy = PermissionDialogCopy::for_locale(locale).map_err(str::to_owned)?;
    copy.validate().map_err(str::to_owned)?;
    if confirm_permission_with_os(app.clone(), &PermissionDetails::EnableFullAccess, &copy).await? {
        Ok(LegacyGlobalPolicyConfirmation::ConfirmedImport)
    } else {
        Ok(LegacyGlobalPolicyConfirmation::DeclinedImport)
    }
}

async fn confirm_legacy_project_policy(
    app: &AppHandle,
    state: &LocalServiceState,
    locale: NativeDialogLocale,
    project_id: &str,
) -> Result<bool, String> {
    let status = request_local_service(
        state,
        "internal.tool_permissions.project_status",
        json!({"projectId": project_id}),
    )
    .await?;
    let needs_confirmation = status
        .get("needsConfirmation")
        .and_then(Value::as_bool)
        .ok_or("The local service returned invalid permission status.")?;
    let has_saved_policy = status
        .get("hasSavedPolicy")
        .and_then(Value::as_bool)
        .ok_or("The local service returned invalid permission status.")?;
    if needs_confirmation && !has_saved_policy {
        return Err("The local service returned invalid permission status.".to_owned());
    }
    if !needs_confirmation {
        return Ok(has_saved_policy);
    }
    let rules_json = status
        .get("rulesJson")
        .and_then(Value::as_str)
        .filter(|rules| rules.len() <= 131_072)
        .ok_or("The local service returned invalid project permission rules.")?;
    let rules: Value = serde_json::from_str(rules_json)
        .map_err(|_| "The local service returned invalid project permission rules.")?;
    let rules = rules
        .as_object()
        .ok_or("The local service returned invalid project permission rules.")?;
    let display_rules = rules
        .iter()
        .map(|(name, rule)| Some((name.as_str(), rule.as_str()?)))
        .collect::<Option<Vec<_>>>()
        .ok_or("The local service returned invalid project permission rules.")?;
    let details = project_allow_confirmation(project_id, display_rules.into_iter())
        .ok_or("The local service returned invalid project permission rules.")?;
    let copy = PermissionDialogCopy::for_locale(locale).map_err(str::to_owned)?;
    copy.validate().map_err(str::to_owned)?;
    if !confirm_permission_with_os(app.clone(), &details, &copy).await? {
        return Ok(true);
    }
    let result = request_local_service(
        state,
        "internal.tool_permissions.project_confirm",
        json!({"projectId": project_id, "rulesJson": rules_json}),
    )
    .await?;
    if result.get("confirmed").and_then(Value::as_bool).is_none() {
        return Err("The local service returned invalid permission status.".to_owned());
    }
    Ok(true)
}

/// Sends a permission decision only for a live service request and requires native consent to allow it.
#[tauri::command]
pub(crate) async fn respond_local_service_permission(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
    generation: u64,
    approval_request_id: String,
    approved: bool,
    permission_locale: NativeDialogLocale,
) -> Result<bool, String> {
    let process = Arc::clone(&state.process);
    let permissions = Arc::clone(&state.permissions);
    let request = begin_permission_response(&permissions, generation, &approval_request_id)?;
    let approved = if approved {
        let permission_copy =
            PermissionDialogCopy::for_locale(permission_locale).map_err(str::to_owned)?;
        permission_copy.validate().map_err(str::to_owned)?;
        match confirm_permission_with_os(app, &request.details, &permission_copy).await {
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
    )
    .await
    {
        release_permission_response(&permissions, generation, &approval_request_id);
        return Err(error);
    }
    Ok(approved)
}
