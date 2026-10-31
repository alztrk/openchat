use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::json;

use super::{
    PreparedToolCall, ToolCall, ToolExecutor, ToolPermissionMode, ToolPermissionRule,
    ToolPermissionRules, parse_tool_permission_rules, qualify_citation_source_ids,
};
use crate::storage::AppStorage;
use crate::tools::executor::{paths::ToolPathScope, validation::ToolOperation};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "openchat-project-rule-precheck-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create project rule fixture");
        Self(root)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove project rule fixture");
    }
}

struct TestStorage {
    storage: AppStorage,
    _directory: TestDirectory,
}

impl TestStorage {
    fn new() -> Self {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage
            .connect()
            .expect("connect test storage")
            .execute_batch(
                "CREATE TABLE projects (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL,
                        folder_path TEXT NOT NULL,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    );
                    CREATE TABLE conversations (
                        id TEXT PRIMARY KEY NOT NULL,
                        title TEXT NOT NULL,
                        title_source TEXT NOT NULL DEFAULT 'automatic',
                        connection_id TEXT,
                        workspace_id TEXT,
                        api_key_connection_id TEXT,
                        provider_id TEXT,
                        model_id TEXT,
                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                        is_pinned INTEGER NOT NULL DEFAULT 0,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    );
                    CREATE TABLE messages (
                        rowid INTEGER PRIMARY KEY,
                        id TEXT NOT NULL,
                        conversation_id TEXT NOT NULL
                            REFERENCES conversations(id) ON DELETE CASCADE,
                        role TEXT NOT NULL,
                        content TEXT NOT NULL,
                        created_at INTEGER,
                        output_tokens INTEGER,
                        tokens_per_second REAL,
                        elapsed_microseconds INTEGER,
                        reasoning_summaries TEXT NOT NULL DEFAULT '[]',
                        tool_activities TEXT NOT NULL DEFAULT '[]',
                        status TEXT NOT NULL,
                        failure_code TEXT,
                        UNIQUE (conversation_id, id)
                    );",
            )
            .expect("create test conversation tables");
        storage
            .initialize_backend_schema()
            .expect("initialize test backend schema");
        Self {
            storage,
            _directory: directory,
        }
    }

    fn add_conversation(&self, conversation_id: &str) {
        self.storage
            .connect()
            .expect("connect test storage")
            .execute(
                "INSERT INTO conversations (id, title, created_at, updated_at)
                     VALUES (?1, ?1, 1, 1)",
                [conversation_id],
            )
            .expect("insert test conversation");
    }
}

#[test]
fn rejects_calls_that_were_not_advertised_to_the_model() {
    let executor = ToolExecutor::with_allowed_tool_names(
        None,
        Path::new("."),
        ToolPermissionMode::FullAccess,
        ["read_file".to_owned()],
    );

    assert!(executor.validate_tool_name("read_file").is_ok());
    assert_eq!(
        executor
            .validate_tool_name("ask_user")
            .expect_err("unadvertised question tool must be rejected")
            .code,
        "invalid_provider_response"
    );
}

#[test]
fn security_scan_enforces_the_public_web_consent_allowlist() {
    let executor = ToolExecutor::with_allowed_tool_names(
        None,
        Path::new("."),
        ToolPermissionMode::FullAccess,
        ["web_search".to_owned(), "read_file".to_owned()],
    )
    .with_security_scan_scope(true, false);
    assert!(executor.validate_tool_name("read_file").is_ok());
    assert_eq!(
        executor
            .validate_tool_name("web_search")
            .expect_err("public web research must be denied without stored consent")
            .code,
        "invalid_provider_response"
    );

    let opted_in_executor = ToolExecutor::with_allowed_tool_names(
        None,
        Path::new("."),
        ToolPermissionMode::FullAccess,
        ["web_search".to_owned(), "read_file".to_owned()],
    )
    .with_security_scan_scope(true, true);
    assert!(opted_in_executor.validate_tool_name("web_search").is_ok());
}

#[test]
fn direct_file_tool_keeps_one_lock_through_capture_and_checkpoint_persistence() {
    let workspace = TestDirectory::new();
    let storage = TestStorage::new();
    let conversation_id = "direct-file-tool-lock";
    storage.add_conversation(conversation_id);
    fs::write(workspace.0.join("note.txt"), "before").expect("create workspace file");

    let write = PreparedToolCall {
        root: workspace.0.clone(),
        relative_path: "note.txt".to_owned(),
        requested_path: "note.txt".to_owned(),
        target_path: workspace.0.join("note.txt"),
        scope: ToolPathScope::Project,
        operation: ToolOperation::Write {
            content: "after".to_owned(),
        },
    };
    let (write_output, write_changes, write_error, write_checkpoint) =
        super::execute_direct_file_change_with_tracking(
            &storage.storage,
            conversation_id,
            "write_file",
            &write,
            None,
        );
    assert_eq!(write_output["success"].as_bool(), Some(true));
    assert_eq!(write_changes.len(), 1);
    assert_eq!(write_error, None);
    assert!(write_checkpoint.is_some());

    let edit = PreparedToolCall {
        operation: ToolOperation::Edit {
            old_string: "after".to_owned(),
            new_string: "edited".to_owned(),
        },
        ..write
    };
    let (edit_output, edit_changes, edit_error, edit_checkpoint) =
        super::execute_direct_file_change_with_tracking(
            &storage.storage,
            conversation_id,
            "edit_file",
            &edit,
            None,
        );
    assert_eq!(edit_output["success"].as_bool(), Some(true));
    assert_eq!(edit_changes.len(), 1);
    assert_eq!(edit_error, None);
    assert!(edit_checkpoint.is_some());
    assert_eq!(
        fs::read_to_string(workspace.0.join("note.txt")).expect("read edited file"),
        "edited"
    );
}

#[test]
fn direct_file_write_rejects_a_root_capability_from_another_approved_path() {
    let approved_workspace = TestDirectory::new();
    let active_workspace = TestDirectory::new();
    let storage = TestStorage::new();
    let conversation_id = "direct-file-root-approval";
    storage.add_conversation(conversation_id);
    fs::write(active_workspace.0.join("note.txt"), "before").expect("create active workspace file");
    let (approved_directory, _) =
        crate::file_changes::open_workspace_directory(&approved_workspace.0)
            .expect("open approved workspace");
    let prepared = PreparedToolCall {
        root: active_workspace.0.clone(),
        relative_path: "note.txt".to_owned(),
        requested_path: "note.txt".to_owned(),
        target_path: active_workspace.0.join("note.txt"),
        scope: ToolPathScope::Project,
        operation: ToolOperation::Write {
            content: "after".to_owned(),
        },
    };

    let (output, changes, tracking_error, checkpoint) =
        super::execute_direct_file_change_with_tracking(
            &storage.storage,
            conversation_id,
            "write_file",
            &prepared,
            Some(std::sync::Arc::new(approved_directory)),
        );

    assert_eq!(
        output["error"]["code"].as_str(),
        Some("permission_target_changed")
    );
    assert!(changes.is_empty());
    assert_eq!(tracking_error.as_deref(), Some("workspace_unavailable"));
    assert!(checkpoint.is_none());
    assert_eq!(
        fs::read_to_string(active_workspace.0.join("note.txt"))
            .expect("read unchanged active file"),
        "before"
    );
}

#[test]
fn approved_file_root_identity_rejects_replacement_at_the_same_path() {
    let workspace = TestDirectory::new();
    let (directory, _) = crate::file_changes::open_workspace_directory(&workspace.0)
        .expect("open approved workspace");
    let identity = crate::file_changes::workspace_directory_identity(&directory)
        .expect("read approved workspace identity");
    drop(directory);
    let moved_root = workspace.0.with_extension("approved-original");
    fs::rename(&workspace.0, &moved_root).expect("move approved workspace root");
    fs::create_dir_all(&workspace.0).expect("create replacement workspace root");

    let result = match super::open_approved_file_root(&workspace.0, identity) {
        Ok(directory) => {
            drop(directory);
            None
        }
        Err(code) => Some(code),
    };

    fs::remove_dir_all(&workspace.0).expect("remove replacement workspace root");
    fs::rename(&moved_root, &workspace.0).expect("restore approved workspace root");
    assert_eq!(result, Some("permission_target_changed"));
}

#[cfg(unix)]
#[test]
fn direct_file_write_rejects_a_same_path_root_replaced_after_approval() {
    let workspace = TestDirectory::new();
    let storage = TestStorage::new();
    let conversation_id = "direct-file-replaced-approval-root";
    storage.add_conversation(conversation_id);
    fs::write(workspace.0.join("note.txt"), "original").expect("create original workspace file");
    let (approved_directory, _) = crate::file_changes::open_workspace_directory(&workspace.0)
        .expect("open approved workspace");
    let moved_root = workspace.0.with_extension("approved-original");
    fs::rename(&workspace.0, &moved_root).expect("move approved workspace root");
    fs::create_dir_all(&workspace.0).expect("create replacement workspace root");
    fs::write(workspace.0.join("note.txt"), "replacement")
        .expect("create replacement workspace file");
    let prepared = PreparedToolCall {
        root: workspace.0.clone(),
        relative_path: "note.txt".to_owned(),
        requested_path: "note.txt".to_owned(),
        target_path: workspace.0.join("note.txt"),
        scope: ToolPathScope::Project,
        operation: ToolOperation::Write {
            content: "after".to_owned(),
        },
    };

    let (output, changes, tracking_error, checkpoint) =
        super::execute_direct_file_change_with_tracking(
            &storage.storage,
            conversation_id,
            "write_file",
            &prepared,
            Some(std::sync::Arc::new(approved_directory)),
        );

    let replacement_contents =
        fs::read_to_string(workspace.0.join("note.txt")).expect("read unchanged replacement file");
    fs::remove_dir_all(&workspace.0).expect("remove replacement workspace root");
    fs::rename(&moved_root, &workspace.0).expect("restore original workspace root");
    assert_eq!(
        output["error"]["code"].as_str(),
        Some("permission_target_changed")
    );
    assert!(changes.is_empty());
    assert_eq!(tracking_error.as_deref(), Some("workspace_unavailable"));
    assert!(checkpoint.is_none());
    assert_eq!(replacement_contents, "replacement");
    assert_eq!(
        fs::read_to_string(workspace.0.join("note.txt")).expect("read restored original file"),
        "original"
    );
}

#[test]
fn citation_source_ids_are_unique_to_the_tool_call() {
    let mut output = json!({
        "sourceId": "U1",
        "results": [{"sourceId": "S1"}, {"sourceId": "S2"}]
    });

    qualify_citation_source_ids(&mut output, "call_1234abcd");

    assert_eq!(output["sourceId"], "U1-call1234");
    assert_eq!(output["results"][0]["sourceId"], "S1-call1234");
    assert_eq!(output["results"][1]["sourceId"], "S2-call1234");
}

#[test]
fn project_tool_rules_override_the_global_mode_without_changing_its_default() {
    let operation = ToolOperation::Write {
        content: "content".to_owned(),
    };
    let mut rules = ToolPermissionRules::new();
    rules.insert("write_file".to_owned(), ToolPermissionRule::Allow);
    rules.insert("execute_command".to_owned(), ToolPermissionRule::Deny);
    let executor = ToolExecutor::with_permission_rules(
        None,
        Path::new("."),
        ToolPermissionMode::RequireApproval,
        ["write_file".to_owned(), "execute_command".to_owned()],
        rules,
    );

    assert_eq!(
        executor.permission_rule(
            &ToolCall {
                id: "call_write".to_owned(),
                name: "write_file".to_owned(),
                arguments: json!({}),
            },
            &operation,
            None,
        ),
        ToolPermissionRule::Allow
    );
    assert_eq!(
        executor.permission_rule(
            &ToolCall {
                id: "call_command".to_owned(),
                name: "execute_command".to_owned(),
                arguments: json!({}),
            },
            &operation,
            None,
        ),
        ToolPermissionRule::Deny
    );
    assert_eq!(
        executor.permission_rule(
            &ToolCall {
                id: "call_read".to_owned(),
                name: "read_file".to_owned(),
                arguments: json!({}),
            },
            &operation,
            None,
        ),
        ToolPermissionRule::Ask
    );
}

#[test]
fn plan_mode_denies_every_tool_even_when_project_rules_allow_them() {
    let mut rules = ToolPermissionRules::new();
    rules.insert("write_file".to_owned(), ToolPermissionRule::Allow);
    rules.insert("read_file".to_owned(), ToolPermissionRule::Deny);
    let executor = ToolExecutor::with_permission_rules(
        None,
        Path::new("."),
        ToolPermissionMode::Plan,
        ["write_file".to_owned(), "read_file".to_owned()],
        rules,
    );

    let write_call = ToolCall {
        id: "call_write".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({}),
    };
    assert_eq!(
        executor.permission_rule(
            &write_call,
            &ToolOperation::Write {
                content: "change".to_owned(),
            },
            None,
        ),
        ToolPermissionRule::Deny
    );

    let read_call = ToolCall {
        id: "call_read".to_owned(),
        name: "read_file".to_owned(),
        arguments: json!({}),
    };
    assert_eq!(
        executor.permission_rule(
            &read_call,
            &ToolOperation::Read {
                start_line: 1,
                line_count: 10,
            },
            None,
        ),
        ToolPermissionRule::Deny
    );

    let default_read_executor = ToolExecutor::with_allowed_tool_names(
        None,
        Path::new("."),
        ToolPermissionMode::Plan,
        ["read_file".to_owned()],
    );
    assert_eq!(
        default_read_executor.permission_rule(
            &read_call,
            &ToolOperation::Read {
                start_line: 1,
                line_count: 10,
            },
            None,
        ),
        ToolPermissionRule::Deny
    );

    let web_call = ToolCall {
        id: "call_search".to_owned(),
        name: "web_search".to_owned(),
        arguments: json!({}),
    };
    let web_operation = ToolOperation::WebSearch {
        query: "Rust documentation".to_owned(),
        limit: Some(3),
    };
    assert!(!web_operation.is_safe_for_auto_approval());
    assert_eq!(
        default_read_executor.permission_rule(&web_call, &web_operation, None),
        ToolPermissionRule::Deny
    );
}

#[test]
fn named_task_rules_override_the_project_wide_task_rule() {
    let operation = ToolOperation::Bash {
        command: Some("cargo test".to_owned()),
        terminal_id: None,
        input: None,
        action: None,
        timeout_seconds: Some(60),
        wait_ms: None,
    };
    let rules = parse_tool_permission_rules(Some(&json!({
        "run_project_task": "allow",
        "run_project_task__verify": "deny"
    })))
    .expect("valid named task rules");
    let executor = ToolExecutor::with_permission_rules(
        None,
        Path::new("."),
        ToolPermissionMode::FullAccess,
        ["run_project_task".to_owned()],
        rules,
    );
    let call = ToolCall {
        id: "call_verify".to_owned(),
        name: "run_project_task".to_owned(),
        arguments: json!({"task": "verify"}),
    };
    assert_eq!(
        executor.permission_rule(&call, &operation, None),
        ToolPermissionRule::Deny
    );
    let other_call = ToolCall {
        id: "call_lint".to_owned(),
        name: "run_project_task".to_owned(),
        arguments: json!({"task": "lint"}),
    };
    assert_eq!(
        executor.permission_rule(&other_call, &operation, None),
        ToolPermissionRule::Allow
    );
    assert_eq!(
        super::project_task_permission_rule(&executor.permission_rules, "verify"),
        Some(&ToolPermissionRule::Deny)
    );
}

#[test]
fn permission_rule_parser_rejects_unknown_tools_and_values() {
    let valid = json!({
        "execute_command": "ask",
        "read_file": "allow",
        "search_files": "ask",
        "git_diff": "allow",
        "project_tool__lint": "deny"
    });
    let rules = parse_tool_permission_rules(Some(&valid)).expect("valid rules");
    assert_eq!(rules["execute_command"], ToolPermissionRule::Ask);
    assert_eq!(rules["read_file"], ToolPermissionRule::Allow);
    assert_eq!(rules["search_files"], ToolPermissionRule::Ask);
    assert_eq!(rules["project_tool__lint"], ToolPermissionRule::Deny);

    #[cfg(windows)]
    {
        let mcp = json!({
            "mcp__local_docs__*": "allow",
            "mcp__local_docs__read-file": "deny"
        });
        let rules = parse_tool_permission_rules(Some(&mcp)).expect("valid MCP rule");
        assert_eq!(rules["mcp__local_docs__*"], ToolPermissionRule::Allow);
        assert_eq!(
            rules["mcp__local_docs__read-file"],
            ToolPermissionRule::Deny
        );
        assert!(
            parse_tool_permission_rules(Some(&json!({
                "mcp__Invalid__*": "allow"
            })))
            .is_err()
        );
        assert!(
            parse_tool_permission_rules(Some(&json!({
                "mcp__local_docs__read/file": "allow"
            })))
            .is_err()
        );
    }

    let invalid_tool = json!({"custom_command": "allow"});
    assert_eq!(
        parse_tool_permission_rules(Some(&invalid_tool))
            .expect_err("unknown tools must be rejected")
            .code,
        "invalid_tool_permission_rules"
    );
    let invalid_value = json!({"execute_command": "inherit"});
    assert_eq!(
        parse_tool_permission_rules(Some(&invalid_value))
            .expect_err("unknown rule values must be rejected")
            .code,
        "invalid_tool_permission_rules"
    );
    let invalid_task_rule = json!({"run_project_task__Verify": "allow"});
    assert_eq!(
        parse_tool_permission_rules(Some(&invalid_task_rule))
            .expect_err("task permission ids must use catalog id syntax")
            .code,
        "invalid_tool_permission_rules"
    );
}

#[cfg(windows)]
#[test]
fn permission_rule_parser_accepts_the_full_mcp_tool_limit_and_rejects_more() {
    let mut rules = serde_json::Map::new();
    for server in 0..8 {
        rules.insert(format!("mcp__server_{server}__*"), json!("ask"));
        for tool in 0..128 {
            rules.insert(format!("mcp__server_{server}__tool_{tool}"), json!("deny"));
        }
    }
    for tool in [
        "list_files",
        "search_files",
        "read_file",
        "write_file",
        "edit_file",
        "get_file_info",
        "execute_command",
        "send_terminal_input",
        "git_status",
        "git_diff",
        "git_history",
        "web_search",
        "read_url_content",
        "delegate_task",
        "run_project_task",
    ] {
        rules.insert(tool.to_owned(), json!("ask"));
    }
    for task in 0..32 {
        rules.insert(format!("run_project_task__task_{task}"), json!("deny"));
    }
    rules.insert("mcp__legacy_server__*".to_owned(), json!("allow"));
    assert_eq!(rules.len(), super::MAX_TOOL_PERMISSION_RULES);
    assert!(parse_tool_permission_rules(Some(&serde_json::Value::Object(rules.clone()))).is_ok());

    rules.insert("mcp__overflow_server__*".to_owned(), json!("allow"));
    assert!(parse_tool_permission_rules(Some(&serde_json::Value::Object(rules))).is_err());
}

#[cfg(windows)]
#[test]
fn mcp_permissions_are_bound_to_server_and_tool_definition_fingerprints() {
    const SERVER_FINGERPRINT: &str = "0123456789abcdef0123456789abcdef";
    const OTHER_SERVER_FINGERPRINT: &str = "abcdef0123456789abcdef0123456789";
    const TOOL_FINGERPRINT: &str = "11111111111111111111111111111111";
    const OTHER_TOOL_FINGERPRINT: &str = "22222222222222222222222222222222";
    let mut rules = ToolPermissionRules::new();
    assert_eq!(
        crate::tools::mcp::server_permission_rule(&rules, "local_docs", SERVER_FINGERPRINT),
        ToolPermissionRule::Ask
    );
    rules.insert("mcp__local_docs__*".to_owned(), ToolPermissionRule::Allow);
    rules.insert("mcp__remote_docs__*".to_owned(), ToolPermissionRule::Deny);
    assert_eq!(
        crate::tools::mcp::server_permission_rule(&rules, "local_docs", SERVER_FINGERPRINT),
        ToolPermissionRule::Ask
    );
    assert_eq!(
        crate::tools::mcp::server_permission_rule(&rules, "remote_docs", SERVER_FINGERPRINT),
        ToolPermissionRule::Deny
    );
    let server_rule =
        crate::tools::mcp::server_permission_rule_name("local_docs", SERVER_FINGERPRINT);
    rules.insert(server_rule, ToolPermissionRule::Allow);
    assert_eq!(
        crate::tools::mcp::server_permission_rule(&rules, "local_docs", SERVER_FINGERPRINT),
        ToolPermissionRule::Allow
    );
    assert_eq!(
        crate::tools::mcp::server_permission_rule(&rules, "local_docs", OTHER_SERVER_FINGERPRINT),
        ToolPermissionRule::Ask
    );
    assert_eq!(
        super::mcp_tool_permission_rule(
            &rules,
            "local_docs",
            SERVER_FINGERPRINT,
            "read-file",
            TOOL_FINGERPRINT,
        ),
        ToolPermissionRule::Ask
    );
    let tool_rule =
        crate::tools::mcp::tool_permission_rule_name("local_docs", "read-file", TOOL_FINGERPRINT);
    rules.insert(tool_rule, ToolPermissionRule::Allow);
    assert_eq!(
        super::mcp_tool_permission_rule(
            &rules,
            "local_docs",
            SERVER_FINGERPRINT,
            "read-file",
            TOOL_FINGERPRINT,
        ),
        ToolPermissionRule::Allow
    );
    rules.insert(
        "mcp__local_docs__read-file".to_owned(),
        ToolPermissionRule::Deny,
    );
    assert_eq!(
        super::mcp_tool_permission_rule(
            &rules,
            "local_docs",
            SERVER_FINGERPRINT,
            "read-file",
            TOOL_FINGERPRINT,
        ),
        ToolPermissionRule::Deny
    );
    assert_eq!(
        super::mcp_tool_permission_rule(
            &rules,
            "local_docs",
            SERVER_FINGERPRINT,
            "write-file",
            OTHER_TOOL_FINGERPRINT,
        ),
        ToolPermissionRule::Ask
    );
    rules.insert("mcp__local_docs__*".to_owned(), ToolPermissionRule::Deny);
    assert_eq!(
        super::mcp_tool_permission_rule(
            &rules,
            "local_docs",
            SERVER_FINGERPRINT,
            "read-file",
            TOOL_FINGERPRINT,
        ),
        ToolPermissionRule::Deny
    );
}

#[test]
fn trust_handoff_defense_requires_approval_even_in_full_access_mode() {
    use std::path::Path;
    let executor = ToolExecutor::new(None, Path::new("test"), ToolPermissionMode::FullAccess);
    let write_hook_call = ToolCall {
        id: "call_hook".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({"path": ".git/hooks/pre-commit", "content": "echo pwn"}),
    };
    let write_op = ToolOperation::Write {
        content: "echo pwn".to_owned(),
    };

    // In FullAccess, writing to .git/hooks must still result in Ask
    assert_eq!(
        executor.permission_rule(
            &write_hook_call,
            &write_op,
            Some(Path::new(".git/hooks/pre-commit"))
        ),
        ToolPermissionRule::Ask
    );

    // Normal files in FullAccess must be allowed
    let normal_call = ToolCall {
        id: "call_normal".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({"path": "src/main.rs", "content": "fn main() {}"}),
    };
    let normal_op = ToolOperation::Write {
        content: "fn main() {}".to_owned(),
    };
    assert_eq!(
        executor.permission_rule(&normal_call, &normal_op, Some(Path::new("src/main.rs"))),
        ToolPermissionRule::Allow
    );
}

#[test]
fn redact_tool_output_masks_live_credentials_in_json() {
    let mut output = json!({
        "stdout": "Loaded configuration:\nOPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz1234567890\nDone.",
        "keyVariants": "SECRET_KEY=upper-secret secretKey:camel-secret secret-key=hyphen-secret",
        "nested": {
            "token": "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "normal": "hello world"
        },
        "list": [
            "api_key: secret-value-1234",
            "normal item"
        ]
    });

    super::redact_tool_output(&mut output);

    let stdout = output["stdout"].as_str().unwrap();
    assert!(!stdout.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"));
    assert!(stdout.contains("[REDACTED]"));
    let key_variants = output["keyVariants"].as_str().unwrap();
    for secret in ["upper-secret", "camel-secret", "hyphen-secret"] {
        assert!(!key_variants.contains(secret));
    }

    let nested_token = output["nested"]["token"].as_str().unwrap();
    assert!(!nested_token.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
    assert_eq!(nested_token, "[REDACTED]");

    let list_item = output["list"][0].as_str().unwrap();
    assert!(!list_item.contains("secret-value-1234"));
    assert!(list_item.contains("[REDACTED]"));

    assert_eq!(output["nested"]["normal"], "hello world");
    assert_eq!(output["list"][1], "normal item");
}

#[test]
fn matching_project_rules_preflight_a_write_once_and_refresh_when_changed() {
    let project = TestDirectory::new();
    let root = fs::canonicalize(&project.0).expect("canonicalize project root");
    fs::create_dir_all(root.join(".openchat/rules")).expect("create project rules folder");
    fs::create_dir_all(root.join("src")).expect("create source folder");
    let rule_path = root.join(".openchat/rules/source.mdc");
    fs::write(
        &rule_path,
        "---\nglobs: \"src/**/*.rs\"\n---\nUse Rust style.\n",
    )
    .expect("write source rule");
    let target_path = root.join("src/new_module.rs");
    let prepared = PreparedToolCall {
        root: root.clone(),
        relative_path: "src/new_module.rs".to_owned(),
        requested_path: target_path.to_string_lossy().into_owned(),
        target_path,
        scope: ToolPathScope::Full,
        operation: ToolOperation::Write {
            content: "pub fn new_module() {}".to_owned(),
        },
    };
    let executor = ToolExecutor::new(Some(&root), Path::new("."), ToolPermissionMode::FullAccess)
        .with_workspace_roots(vec![root]);

    let first = executor
        .project_rules_required_for_write(&prepared)
        .expect("load project rule");
    assert_eq!(first.as_ref().map(Vec::len), Some(1));
    assert!(
        executor
            .project_rules_required_for_write(&prepared)
            .expect("recognize rule already supplied")
            .is_none()
    );

    fs::write(
        &rule_path,
        "---\nglobs: \"src/**/*.rs\"\n---\nUse the updated Rust style.\n",
    )
    .expect("update source rule");
    let updated = executor
        .project_rules_required_for_write(&prepared)
        .expect("reload changed project rule");
    assert_eq!(
        updated.as_ref().map(|rules| rules[0].instructions.as_str()),
        Some("Use the updated Rust style.")
    );
}
