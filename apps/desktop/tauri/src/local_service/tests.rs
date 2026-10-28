//! Tests for RPC frame bounds, approval tracking, and permission policies.

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::process::*;
use super::rpc_frame::*;
use super::types::*;

#[test]
fn full_access_mode_changes_require_native_confirmation() {
    let set_frame =
        "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"full_access\"}}\n";
    let import_frame =
        "{\"id\":1,\"method\":\"tools.permissions.mode.get\",\"params\":{\"initialMode\":\"full_access\"}}\n";

    assert_eq!(
        renderer_rpc_disposition(set_frame, 256),
        Some(RendererRpcDisposition::Confirm(
            PermissionDetails::EnableFullAccess
        ))
    );
    assert_eq!(
        renderer_rpc_disposition(import_frame, 256),
        Some(RendererRpcDisposition::CheckLegacyGlobal {
            imported_full_access: true
        })
    );
}

#[test]
fn project_allow_rules_require_native_confirmation_for_save_and_import() {
    let set_frame = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"allow\"}}}\n";
    let import_frame = "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\",\"initialRules\":{\"read_file\":\"allow\"}}}\n";
    let expected = PermissionDetails::AllowProjectRules {
        project_id: "project-1".to_owned(),
        allow_rule_count: 1,
        rule_names: vec!["read_file".to_owned()],
        additional_rule_count: 0,
    };

    assert_eq!(
        renderer_rpc_disposition(set_frame, 256),
        Some(RendererRpcDisposition::Confirm(expected.clone()))
    );
    assert_eq!(
        renderer_rpc_disposition(import_frame, 256),
        Some(RendererRpcDisposition::CheckLegacyProject {
            project_id: "project-1".to_owned(),
            import_confirmation: Some(expected),
        })
    );
}

#[test]
fn project_confirmation_lists_all_rules_without_git_tools() {
    let frame = "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"edit_file\":\"allow\",\"execute_command\":\"allow\",\"get_file_info\":\"allow\",\"list_files\":\"allow\",\"read_file\":\"allow\",\"search_files\":\"allow\"}}}\n";

    assert_eq!(
        renderer_rpc_disposition(frame, 1_024),
        Some(RendererRpcDisposition::Confirm(
            PermissionDetails::AllowProjectRules {
                project_id: "project-1".to_owned(),
                allow_rule_count: 6,
                rule_names: vec![
                    "edit_file",
                    "execute_command",
                    "get_file_info",
                    "list_files",
                    "read_file",
                    "search_files",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
                additional_rule_count: 0,
            }
        ))
    );
}

#[test]
fn native_confirmation_marker_is_added_only_after_host_confirmation() {
    for elevated in [
        "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"full_access\"}}\n",
        "{\"id\":1,\"method\":\"tools.permissions.mode.get\",\"params\":{\"initialMode\":\"full_access\"}}\n",
        "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\",\"initialRules\":{\"read_file\":\"allow\"}}}\n",
    ] {
        let confirmed = add_native_confirmation(elevated).expect("mark host-confirmed policy");
        assert!(confirmed.contains("\"nativeConfirmation\":true"));
        assert_eq!(
            renderer_rpc_disposition(&confirmed, MAX_RPC_FRAME_BYTES),
            None
        );
    }

    let forged = "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"full_access\",\"nativeConfirmation\":true}}\n";
    assert_eq!(renderer_rpc_disposition(forged, 256), None);
    assert!(add_native_confirmation("{\"id\":1,\"method\":\"chat.send\"}\n").is_err());
}

#[test]
fn restricted_permission_modes_and_unrelated_calls_do_not_need_confirmation() {
    for frame in [
        "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"require_approval\"}}\n",
        "{\"id\":1,\"method\":\"tools.permissions.mode.set\",\"params\":{\"mode\":\"approve_safe_operations\"}}\n",
        "{\"id\":1,\"method\":\"project.tool_permissions.set\",\"params\":{\"projectId\":\"project-1\",\"rules\":{\"read_file\":\"ask\",\"write_file\":\"deny\"}}}\n",
        "{\"id\":1,\"method\":\"chat.send\",\"params\":{}}\n",
    ] {
        assert_eq!(
            renderer_rpc_disposition(frame, 256),
            Some(RendererRpcDisposition::Forward)
        );
    }

    assert_eq!(
        renderer_rpc_disposition(
            "{\"id\":1,\"method\":\"tools.permissions.mode.get\",\"params\":{}}\n",
            256
        ),
        Some(RendererRpcDisposition::CheckLegacyGlobal {
            imported_full_access: false
        })
    );

    for frame in [
        "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\",\"initialRules\":{\"read_file\":\"deny\"}}}\n",
        "{\"id\":1,\"method\":\"project.tool_permissions.get\",\"params\":{\"projectId\":\"project-1\"}}\n",
    ] {
        assert!(matches!(
            renderer_rpc_disposition(frame, 256),
            Some(RendererRpcDisposition::CheckLegacyProject {
                import_confirmation: None,
                ..
            })
        ));
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
    let first = PermissionPolicyConfirmationGuard::acquire(&active).expect("acquire first guard");
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

    let request = begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef")
        .expect("captured active request can be answered");
    assert!(matches!(
        request.details,
        PermissionDetails::Reviewable { .. }
    ));
    assert!(
        begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",).is_err()
    );

    let result = json!({"id": 7, "result": {"status": "cancelled"}}).to_string();
    observe_service_output(&permissions, 3, &result);
    assert!(
        begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",).is_err()
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
        begin_permission_response(&permissions, 4, "0123456789abcdef0123456789abcdef",).is_err()
    );
    assert!(
        begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",).is_err()
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
        begin_permission_response(&permissions, 3, "0123456789abcdef0123456789abcdef",).is_err()
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
    let (_, details) = parse_pending_permission(&data).expect("valid approval ID");
    assert!(matches!(details, PermissionDetails::TooLargeToReview));
}

#[test]
fn permission_details_show_bidi_controls_as_visible_codes() {
    let data = json!({
        "approvalRequestId": "0123456789abcdef0123456789abcdef",
        "toolName": "read_file\u{202e}",
        "targetPath": "D:/safe\u{2066}target",
        "arguments": {"path": "report\u{202e}.txt"}
    });

    let (_, details) = parse_pending_permission(&data).expect("valid approval ID");
    let PermissionDetails::Reviewable {
        tool_name,
        target_path,
        arguments,
    } = details
    else {
        panic!("bounded permission details should remain reviewable");
    };

    assert_eq!(tool_name, "read_file<U+202E>");
    assert_eq!(target_path, "D:/safe<U+2066>target");
    assert!(arguments.contains("report<U+202E>.txt"));
}
