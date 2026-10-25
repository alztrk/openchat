//! RPC frame validation and disposition parsing.

use super::types::*;

pub(crate) fn is_valid_rpc_frame(frame: &str, maximum_bytes: usize) -> bool {
    if frame.len() > maximum_bytes || !frame.ends_with('\n') {
        return false;
    }
    let body = &frame.as_bytes()[..frame.len() - 1];
    !body.is_empty() && !body.iter().any(|&b| b == b'\n' || b == b'\r')
}

/// Blocks renderer-forged decisions and marks full-access mode changes for OS confirmation.
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
