//! Frontend-invocable Tauri native commands for local service lifecycle and permissions.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

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
