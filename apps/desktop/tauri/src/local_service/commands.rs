//! Frontend-invocable Tauri native commands for local service lifecycle and permissions.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

use super::dialogs::PermissionDialogCopy;
use super::process::*;
use super::rpc_frame::*;
use super::types::*;

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
    let spawn_result = app
        .shell()
        .sidecar("binaries/openchat-local-service")
        .and_then(|command| command.spawn())
        .map_err(|primary_error| format!("Primary sidecar launch failed: {primary_error}"))
        .or_else(|primary_error| {
            let fallback_result = app
                .shell()
                .sidecar("openchat-local-service")
                .and_then(|command| command.spawn());
            fallback_result.map_err(|fallback_error| {
                format!("{primary_error}; fallback sidecar launch failed: {fallback_error}")
            })
        });
    let (mut events, child) =
        spawn_result.map_err(|error| format!("The local service could not be started: {error}"))?;
    process.next_generation = generation;
    process.current = Some(RunningProcess { generation, child });

    let shared_process = Arc::clone(&state.process);
    let shared_permissions = Arc::clone(&state.permissions);
    tauri::async_runtime::spawn(async move {
        let mut terminate_child = true;
        let mut stderr_line = Vec::new();
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
                CommandEvent::Stderr(bytes) => {
                    for byte in bytes {
                        if byte == b'\n' {
                            if let Some(details) = local_service_failure_details(&stderr_line) {
                                let failure = LocalServiceEvent::Failure {
                                    generation,
                                    details,
                                };
                                if let Err(error) =
                                    app.emit_to("main", LOCAL_SERVICE_EVENT, failure)
                                {
                                    eprintln!("OpenChat local service failure could not reach the renderer: {error}");
                                    break;
                                }
                            }
                            stderr_line.clear();
                        } else if stderr_line.len() < MAX_LOCAL_SERVICE_ERROR_DETAIL_BYTES {
                            stderr_line.push(byte);
                        }
                    }
                }
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

fn local_service_failure_details(line: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(line);
    let details = line
        .trim_end_matches('\r')
        .strip_prefix(LOCAL_SERVICE_ERROR_PREFIX)?
        .trim();
    (!details.is_empty()).then(|| details.to_owned())
}

/// Writes one bounded, newline-terminated JSON-RPC frame to the owned service process.
#[tauri::command]
pub(crate) async fn write_local_service(
    app: AppHandle,
    state: State<'_, LocalServiceState>,
    generation: u64,
    line: String,
    permission_copy: PermissionDialogCopy,
) -> Result<(), String> {
    permission_copy.validate().map_err(str::to_owned)?;
    let disposition = renderer_rpc_disposition(&line, MAX_RPC_FRAME_BYTES)
        .ok_or_else(|| "The local service request frame is invalid or too large.".to_owned())?;
    if let RendererRpcDisposition::Confirm(details) = disposition {
        let _confirmation_guard = PermissionPolicyConfirmationGuard::acquire(
            &state.permission_policy_confirmation_active,
        )?;
        let confirmed = confirm_permission_with_os(app, &details, &permission_copy).await?;
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
    permission_copy: PermissionDialogCopy,
) -> Result<bool, String> {
    permission_copy.validate().map_err(str::to_owned)?;
    let process = Arc::clone(&state.process);
    let permissions = Arc::clone(&state.permissions);
    let request = begin_permission_response(&permissions, generation, &approval_request_id)?;
    let approved = if approved {
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
    ) {
        release_permission_response(&permissions, generation, &approval_request_id);
        return Err(error);
    }
    Ok(approved)
}
