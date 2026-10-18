//! Background process observation, permission resolution, and stdio handling.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use super::dialogs::permission_dialog_copy;
use super::types::*;

pub(crate) fn observe_service_output(state: &Arc<Mutex<PendingPermissionState>>, generation: u64, line: &str) {
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
pub(crate) fn parse_response_id(line: &str) -> Option<u64> {
    let body = line.strip_prefix("{\"id\":")?;
    let end = body.find(',')?;
    body[..end].parse().ok()
}

/// Parses an approval ID and retains only bounded, displayable operation details.
pub(crate) fn parse_pending_permission(data: &Value) -> Option<(String, PermissionDetails)> {
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
pub(crate) fn begin_permission_response(
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
pub(crate) fn release_permission_response(
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
pub(crate) fn deliver_permission_response(
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
pub(crate) async fn confirm_permission_with_os(
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

pub(crate) fn clear_generation_permissions(state: &Arc<Mutex<PendingPermissionState>>, generation: u64) {
    if let Ok(mut state) = state.lock() {
        state
            .requests
            .retain(|_, request| request.generation != generation);
    }
}
