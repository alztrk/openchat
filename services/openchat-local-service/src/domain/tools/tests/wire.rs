//! Split from tests.rs (5 items).
use super::*;

#[test]
fn opencode_wire_tools_include_bash_and_read_with_industry_standards() {
    let internal_defs = super::super::definitions();
    assert!(internal_defs.iter().any(|tool| tool.name == "read_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "write_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "edit_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "list_files"));
    assert!(internal_defs.iter().any(|tool| tool.name == "search_files"));
    assert!(
        internal_defs
            .iter()
            .any(|tool| tool.name == "execute_command")
    );

    let wire_tools = super::super::opencode_wire_tools(&internal_defs);
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("bash"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("read"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("write"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("edit"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("glob"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("grep"))
    );

    let responses_wire_tools = super::super::opencode_responses_wire_tools(&internal_defs);
    assert!(
        responses_wire_tools
            .iter()
            .any(|t| t.get("name").and_then(Value::as_str) == Some("bash")
                && t.get("type").and_then(Value::as_str) == Some("function")
                && t.get("parameters").is_some())
    );
    assert!(
        responses_wire_tools
            .iter()
            .any(|t| t.get("name").and_then(Value::as_str) == Some("read")
                && t.get("type").and_then(Value::as_str) == Some("function")
                && t.get("parameters").is_some())
    );

    // Bidirectional translation test
    assert_eq!(super::super::opencode_wire_name("read_file"), "read");
    assert_eq!(super::super::opencode_wire_name("write_file"), "write");
    assert_eq!(super::super::opencode_wire_name("edit_file"), "edit");
    assert_eq!(super::super::opencode_wire_name("list_files"), "glob");
    assert_eq!(super::super::opencode_wire_name("search_files"), "grep");
    assert_eq!(super::super::opencode_wire_name("execute_command"), "bash");

    assert_eq!(super::super::internal_tool_name(true, "read"), "read_file");
    assert_eq!(
        super::super::internal_tool_name(true, "write"),
        "write_file"
    );
    assert_eq!(super::super::internal_tool_name(true, "edit"), "edit_file");
    assert_eq!(super::super::internal_tool_name(true, "glob"), "list_files");
    assert_eq!(
        super::super::internal_tool_name(true, "grep"),
        "search_files"
    );
    assert_eq!(
        super::super::internal_tool_name(true, "bash"),
        "execute_command"
    );
    assert_eq!(super::super::internal_tool_name(false, "read"), "read");
}

#[test]
fn opencode_mcp_tool_names_are_preserved_for_round_trip_execution() {
    let tool = super::super::ToolDefinition {
        name: "mcp__local_docs__search".to_owned(),
        description: "Search local docs".to_owned(),
        parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
    };

    let wire = super::super::opencode_wire_tool(&tool);
    assert_eq!(
        wire.pointer("/function/name")
            .and_then(serde_json::Value::as_str),
        Some("mcp__local_docs__search")
    );
    assert_eq!(
        super::super::internal_tool_name(true, "mcp__local_docs__search"),
        tool.name
    );
}

#[test]
fn opencode_wire_tools_keeps_security_scan_bash_stub_inert_while_preserving_active_read() {
    let scan_defs = crate::security_scan::tool_definitions(false);
    let scan_wire = super::super::opencode_wire_tools(&scan_defs);

    let bash_desc = scan_wire
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("bash"))
        .and_then(|tool| tool.pointer("/function/description"))
        .and_then(Value::as_str)
        .expect("security scan wire tools include OpenCode bash gate stub");
    assert!(
        bash_desc.contains("unavailable"),
        "security scan bash gate stub must not advertise shell execution: {bash_desc}"
    );

    let read_desc = scan_wire
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("read"))
        .and_then(|tool| tool.pointer("/function/description"))
        .and_then(Value::as_str)
        .expect("security scan wire tools include active read tool");
    assert_eq!(read_desc, "Read a bounded range of lines from a file.");

    let chat_wire = super::super::opencode_wire_tools(&super::super::definitions());
    let chat_names = chat_wire
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(chat_names.iter().filter(|&&name| name == "read").count(), 1);
    assert_eq!(chat_names.iter().filter(|&&name| name == "bash").count(), 1);
    for forbidden in [
        "read_file",
        "execute_command",
        "write_file",
        "edit_file",
        "list_files",
        "search_files",
    ] {
        assert!(
            !chat_names.contains(&forbidden),
            "opencode wire tools must never send internal name '{forbidden}' alongside wire aliases"
        );
    }
    let chat_bash_desc = chat_wire
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("bash"))
        .and_then(|tool| tool.pointer("/function/description"))
        .and_then(Value::as_str)
        .expect("chat wire tools include active bash tool");
    assert!(chat_bash_desc.starts_with("Execute a shell command"));
}

#[tokio::test]
async fn opencode_read_and_bash_aliases_execute_allowed_read_file_and_execute_command() {
    use super::super::executor::{ToolExecutionContext, ToolExecutor, ToolPermissionMode};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    directory.write("hello.txt", "first line\nsecond line\n");
    let storage =
        AppStorage::open_at(PathBuf::from(directory.root())).expect("open isolated test storage");
    storage
        .connect()
        .expect("connect isolated test storage")
        .execute_batch(
            "CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                UNIQUE(conversation_id, id)
            );",
        )
        .expect("create assistant message activity schema");

    let executor = ToolExecutor::with_allowed_tool_names(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
        ["read_file".to_owned(), "execute_command".to_owned()],
    );
    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_opencode_aliases");
    let snapshot = ChatStreamSnapshot::new("conversation", "message", "", 0);
    let user_questions = UserQuestionBroker::default();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);

    let read_call = ToolCall {
        id: "call_opencode_read".to_owned(),
        name: "read".to_owned(),
        arguments: json!({
            "filePath": PathBuf::from(directory.root())
                .join("hello.txt")
                .to_string_lossy(),
            "offset": 1,
            "limit": 2
        }),
    };
    let read_result = executor
        .execute_call(
            &read_call,
            ToolExecutionContext {
                permissions: &broker,
                request_id: &request_id,
                snapshot: &snapshot,
                events: &events,
                cancellation: &mut cancellation,
                storage: &storage,
                run_id: "run",
                provider_id: "opencode",
                user_questions: &user_questions,
                image_generation: None,
            },
        )
        .await
        .expect("read alias executes read_file");
    assert!(read_result.output.get("error").is_none());
    let content = serde_json::to_string(&read_result.output).expect("serialize read output");
    assert!(content.contains("first line"));

    let bash_call = ToolCall {
        id: "call_opencode_bash".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": "echo opencode_bash_ok"
        }),
    };
    let bash_result = executor
        .execute_call(
            &bash_call,
            ToolExecutionContext {
                permissions: &broker,
                request_id: &request_id,
                snapshot: &snapshot,
                events: &events,
                cancellation: &mut cancellation,
                storage: &storage,
                run_id: "run",
                provider_id: "opencode",
                user_questions: &user_questions,
                image_generation: None,
            },
        )
        .await
        .expect("bash alias executes execute_command");
    assert!(bash_result.output.get("error").is_none());
    let bash_output = serde_json::to_string(&bash_result.output).expect("serialize bash output");
    assert!(bash_output.contains("opencode_bash_ok"));
}

#[tokio::test]
async fn unadvertised_tool_calls_return_denied_tool_result_until_turn_limit() {
    use super::super::executor::{ToolExecutionContext, ToolExecutor, ToolPermissionMode};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolActivityStatus, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    let canonical_root = std::fs::canonicalize(directory.root()).expect("canonicalize test root");
    let storage = AppStorage::open_at(canonical_root.clone()).expect("open isolated test storage");
    storage
        .connect()
        .expect("connect isolated test storage")
        .execute_batch(
            "CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                UNIQUE(conversation_id, id)
            );",
        )
        .expect("create assistant message activity schema");

    let executor = ToolExecutor::with_allowed_tool_names(
        Some(&canonical_root),
        &canonical_root,
        ToolPermissionMode::FullAccess,
        crate::security_scan::allowed_tool_names(false),
    )
    .with_workspace_roots(vec![canonical_root.clone()])
    .with_security_scan_scope(true, false);

    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_security_scan_unlisted");
    let snapshot = ChatStreamSnapshot::new("conversation", "message", "", 0);
    let user_questions = UserQuestionBroker::default();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);
    let sentinel = canonical_root.join("must_not_exist.txt");

    for index in 1..=8 {
        let disallowed_call = ToolCall {
            id: format!("call_disallowed_{index}"),
            name: "execute_command".to_owned(),
            arguments: json!({
                "command": format!("echo nope > \"{}\"", sentinel.display())
            }),
        };
        let result = executor
            .execute_call(
                &disallowed_call,
                ToolExecutionContext {
                    permissions: &broker,
                    request_id: &request_id,
                    snapshot: &snapshot,
                    events: &events,
                    cancellation: &mut cancellation,
                    storage: &storage,
                    run_id: "run",
                    provider_id: "opencode",
                    user_questions: &user_questions,
                    image_generation: None,
                },
            )
            .await
            .expect("disallowed tool returns structured tool error instead of aborting scan");
        assert_eq!(result.call_id, format!("call_disallowed_{index}"));
        assert_eq!(result.output["error"]["code"], "tool_not_available");
        let message = result.output["error"]["message"]
            .as_str()
            .expect("tool error message");
        assert!(message.contains("read"));
        assert!(!message.contains("read_file"));
        assert!(!sentinel.exists(), "disallowed command must never execute");
    }

    let activities = executor.recorded_activities().await;
    assert_eq!(activities.len(), 8);
    assert!(
        activities
            .iter()
            .all(|activity| matches!(activity.status, ToolActivityStatus::Denied))
    );

    let ninth_call = ToolCall {
        id: "call_disallowed_9".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({"command": "echo overflow"}),
    };
    let overflow_err = executor
        .execute_call(
            &ninth_call,
            ToolExecutionContext {
                permissions: &broker,
                request_id: &request_id,
                snapshot: &snapshot,
                events: &events,
                cancellation: &mut cancellation,
                storage: &storage,
                run_id: "run",
                provider_id: "opencode",
                user_questions: &user_questions,
                image_generation: None,
            },
        )
        .await
        .expect_err("repeated unadvertised calls past turn limit must stop runaway loop");
    assert_eq!(overflow_err.code, "invalid_provider_response");
}
