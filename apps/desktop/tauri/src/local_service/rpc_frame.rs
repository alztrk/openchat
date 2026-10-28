//! RPC frame validation and disposition parsing.

use super::types::*;

pub(crate) fn is_valid_rpc_frame(frame: &str, maximum_bytes: usize) -> bool {
    if frame.len() > maximum_bytes || !frame.ends_with('\n') {
        return false;
    }
    let body = &frame.as_bytes()[..frame.len() - 1];
    !body.is_empty() && !body.iter().any(|&b| b == b'\n' || b == b'\r')
}

/// Blocks renderer-forged decisions and identifies policies that need host review.
pub(crate) fn renderer_rpc_disposition(
    frame: &str,
    maximum_bytes: usize,
) -> Option<RendererRpcDisposition> {
    if !is_valid_rpc_frame(frame, maximum_bytes) {
        return None;
    }
    let body = &frame[..frame.len() - 1];
    let Ok(request) = serde_json::from_str::<RendererRpcMethod<'_>>(body) else {
        return Some(RendererRpcDisposition::Forward);
    };
    if request.has_native_request_id {
        return None;
    }
    let Some(method) = request.method.as_deref() else {
        return Some(RendererRpcDisposition::Forward);
    };
    if method == "chat.tool.permission.respond" || method.starts_with("internal.") {
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
        if params.native_confirmation.is_some() {
            return None;
        }
        return Some(match method {
            "tools.permissions.mode.set" if params.mode.as_deref() == Some("full_access") => {
                RendererRpcDisposition::Confirm(PermissionDetails::EnableFullAccess)
            }
            "tools.permissions.mode.get" => RendererRpcDisposition::CheckLegacyGlobal {
                imported_full_access: params.initial_mode.as_deref() == Some("full_access"),
            },
            _ => RendererRpcDisposition::Forward,
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
pub(crate) fn project_permission_rpc_disposition(
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
    if params.native_confirmation.is_some() || !is_valid_project_id(params.project_id?) {
        return None;
    }
    let project_id = params.project_id?;
    match method {
        "project.tool_permissions.set" => {
            let rules = params.rules.as_ref()?;
            if rules.len() > MAX_PROJECT_PERMISSION_RULE_COUNT {
                return None;
            }
            Some(
                project_allow_confirmation(
                    project_id,
                    rules.iter().map(|(name, rule)| (*name, *rule)),
                )
                .map(RendererRpcDisposition::Confirm)
                .unwrap_or(RendererRpcDisposition::Forward),
            )
        }
        "project.tool_permissions.get" => {
            if params
                .initial_rules
                .as_ref()
                .is_some_and(|rules| rules.len() > MAX_PROJECT_PERMISSION_RULE_COUNT)
            {
                return None;
            }
            let import_confirmation = params.initial_rules.as_ref().and_then(|rules| {
                project_allow_confirmation(
                    project_id,
                    rules.iter().map(|(name, rule)| (*name, *rule)),
                )
            });
            Some(RendererRpcDisposition::CheckLegacyProject {
                project_id: project_id.to_owned(),
                import_confirmation,
            })
        }
        _ => None,
    }
}

/// Adds the host-only marker after a native dialog approved an elevated RPC.
pub(crate) fn add_native_confirmation(frame: &str) -> Result<String, String> {
    if !is_valid_rpc_frame(frame, MAX_RPC_FRAME_BYTES) {
        return Err("The permission request frame is invalid.".to_owned());
    }
    if !matches!(
        renderer_rpc_disposition(frame, MAX_RPC_FRAME_BYTES),
        Some(
            RendererRpcDisposition::Confirm(_)
                | RendererRpcDisposition::CheckLegacyGlobal {
                    imported_full_access: true
                }
                | RendererRpcDisposition::CheckLegacyProject {
                    import_confirmation: Some(_),
                    ..
                }
        )
    ) {
        return Err("The request does not contain a confirmed elevated permission.".to_owned());
    }
    let mut request = serde_json::from_str::<serde_json::Value>(&frame[..frame.len() - 1])
        .map_err(|_| "The permission request frame is invalid.")?;
    let params = request
        .as_object_mut()
        .and_then(|request| request.get_mut("params"))
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("The permission request parameters are invalid.")?;
    if params.contains_key("nativeConfirmation") {
        return Err("The permission request contains a reserved field.".to_owned());
    }
    params.insert(
        "nativeConfirmation".to_owned(),
        serde_json::Value::Bool(true),
    );
    let mut encoded = serde_json::to_string(&request)
        .map_err(|_| "The permission request could not be encoded.")?;
    encoded.push('\n');
    if encoded.len() > MAX_RPC_FRAME_BYTES {
        return Err("The confirmed permission request is too large.".to_owned());
    }
    Ok(encoded)
}

/// Builds bounded native-dialog details from a validated project policy map.
pub(crate) fn project_allow_confirmation<'a>(
    project_id: &str,
    rules: impl Iterator<Item = (&'a str, &'a str)>,
) -> Option<PermissionDetails> {
    if !is_valid_project_id(project_id) {
        return None;
    }
    let mut allowed_names = Vec::new();
    for (name, rule) in rules {
        if rule != "allow" {
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
        allowed_names.push(name);
        if allowed_names.len() > MAX_PROJECT_PERMISSION_RULE_COUNT {
            return None;
        }
    }
    if allowed_names.is_empty() {
        return None;
    }
    allowed_names.sort_unstable();
    let allow_rule_count = allowed_names.len();
    let additional_rule_count =
        allow_rule_count.saturating_sub(MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES);
    allowed_names.truncate(MAX_PROJECT_PERMISSION_DIALOG_RULE_NAMES);
    Some(PermissionDetails::AllowProjectRules {
        project_id: project_id.to_owned(),
        allow_rule_count,
        rule_names: allowed_names.into_iter().map(str::to_owned).collect(),
        additional_rule_count,
    })
}

fn is_valid_project_id(project_id: &str) -> bool {
    !project_id.is_empty()
        && project_id.len() <= 128
        && project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
