//! Split from tests.rs (7 items).
use super::*;

#[cfg(windows)]
#[test]
fn project_configuration_readers_reject_external_file_symlinks() {
    use std::os::windows::fs::symlink_file;

    let project = TestDirectory::new();
    let outside = TestDirectory::new();
    let project_config = Path::new(project.root()).join(".openchat");
    let task_catalog = project_config.join("tasks.json");
    let project_tool_catalog = project_config.join("tools.json");
    let mcp_catalog = project_config.join("mcp.json");

    outside.write(
        "tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );
    project.write(".openchat/tasks.json", "{}");
    fs::remove_file(&task_catalog).expect("remove the task catalog");
    symlink_file(outside.0.join("tasks.json"), &task_catalog)
        .expect("link task catalog to an external file");
    assert!(super::super::project_tasks::load_project_tasks(Path::new(project.root())).is_err());

    outside.write("tools.json", r#"{"version":1,"tools":[]}"#);
    project.write(".openchat/tools.json", "{}");
    fs::remove_file(&project_tool_catalog).expect("remove the project tool catalog");
    symlink_file(outside.0.join("tools.json"), &project_tool_catalog)
        .expect("link project tool catalog to an external file");
    assert!(super::super::project_tools::load(Path::new(project.root())).is_err());

    outside.write("mcp.json", r#"{"version":1,"servers":[]}"#);
    project.write(".openchat/mcp.json", "{}");
    fs::remove_file(&mcp_catalog).expect("remove the MCP catalog");
    symlink_file(outside.0.join("mcp.json"), &mcp_catalog)
        .expect("link MCP catalog to an external file");
    assert!(super::super::mcp::server_catalog(Path::new(project.root())).is_err());
}

#[cfg(windows)]
#[test]
fn project_configuration_readers_reject_hard_linked_files() {
    let project = TestDirectory::new();
    let outside = TestDirectory::new();
    let project_config = Path::new(project.root()).join(".openchat");
    fs::create_dir_all(&project_config).expect("create project settings directory");

    outside.write(
        "tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );
    fs::hard_link(
        outside.0.join("tasks.json"),
        project_config.join("tasks.json"),
    )
    .expect("link task catalog to an external file");
    assert!(super::super::project_tasks::load_project_tasks(Path::new(project.root())).is_err());

    outside.write("tools.json", r#"{"version":1,"tools":[]}"#);
    fs::hard_link(
        outside.0.join("tools.json"),
        project_config.join("tools.json"),
    )
    .expect("link tool catalog to an external file");
    assert!(super::super::project_tools::load(Path::new(project.root())).is_err());

    outside.write("mcp.json", r#"{"version":1,"servers":[]}"#);
    fs::hard_link(outside.0.join("mcp.json"), project_config.join("mcp.json"))
        .expect("link MCP catalog to an external file");
    assert!(super::super::mcp::server_catalog(Path::new(project.root())).is_err());
}

#[test]
fn project_task_tool_is_available_only_for_declared_catalog_entries() {
    let directory = TestDirectory::new();
    assert!(
        super::super::project_tasks::load_project_tasks_if_present(Path::new(directory.root()))
            .expect("missing task catalog means no named tasks")
            .is_empty()
    );

    directory.write(
        ".openchat/tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );
    let task_ids =
        super::super::project_tasks::load_project_tasks_if_present(Path::new(directory.root()))
            .expect("load task catalog")
            .into_iter()
            .map(|task| task.id)
            .collect::<Vec<_>>();
    let definition = super::super::project_tasks::tool_definition(&task_ids);
    assert_eq!(
        definition.parameters["properties"]["task"]["enum"],
        json!(["verify"])
    );
}

#[test]
fn disabled_project_task_tool_does_not_block_chat_on_invalid_catalog() {
    let directory = TestDirectory::new();
    directory.write(".openchat/tasks.json", "not json");

    assert!(
        super::super::project_tasks::load_project_task_ids_if_enabled(
            Some(Path::new(directory.root())),
            false,
        )
        .expect("disabled tools must not load their optional catalog")
        .is_empty()
    );
    assert_eq!(
        super::super::project_tasks::load_project_task_ids_if_enabled(
            Some(Path::new(directory.root())),
            true,
        )
        .expect_err("enabled tools must report invalid configuration")
        .code,
        "invalid_project_task_catalog"
    );
}

#[test]
fn oversized_project_task_catalog_is_rejected_after_a_bounded_read() {
    let directory = TestDirectory::new();
    directory.write(
        ".openchat/tasks.json",
        &format!("{{\"version\":1,\"tasks\":[]}}{}", " ".repeat(65 * 1024)),
    );

    let error = super::super::project_tasks::load_project_tasks(Path::new(directory.root()))
        .expect_err("oversized task catalogs must be rejected");
    assert_eq!(error.code, "invalid_project_task_catalog");
}

#[cfg(windows)]
/// Ensures a completed project tool cannot keep a spawned descendant alive through its output pipes.
#[tokio::test]
async fn completed_project_tool_terminates_descendants_before_waiting_for_pipe_eof() {
    use super::super::terminal::TerminalSessionManager;
    use std::path::PathBuf;
    use tokio::sync::watch;

    let system_root = std::env::var_os("SystemRoot").expect("Windows system root is available");
    let program = PathBuf::from(system_root).join("System32").join("cmd.exe");
    let directory = TestDirectory::new();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);
    let input = "start \"\" /b cmd.exe /c \"echo started > child-started.txt & for /l %i in (1,1,100000000) do @rem\"\r\nfor /l %i in (1,1,100) do @if not exist child-started.txt timeout /t 1 /nobreak\r\nexit";

    let execution = tokio::time::timeout(
        Duration::from_secs(5),
        TerminalSessionManager::global().execute_project_tool(
            super::super::terminal::ProjectToolRequest {
                program: &program,
                workdir: Path::new(directory.root()),
                input,
                timeout_seconds: 30,
                output_limit_bytes: 1024,
                cancellation: &mut cancellation,
                full_access: false,
            },
        ),
    )
    .await;

    assert!(
        directory.0.join("child-started.txt").exists(),
        "the fixture starts a descendant before the leader exits: {execution:?}"
    );
    let result = execution
        .expect("project tool pipes close after its process tree is stopped")
        .expect("project tool returns a completion result");
    assert_eq!(result["status"], "completed");
}

#[tokio::test]
async fn project_deny_rule_blocks_file_write_without_requesting_approval() {
    use super::super::executor::{ToolExecutor, ToolPermissionMode, ToolPermissionRule};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    let storage = Arc::new(
        AppStorage::open_at(PathBuf::from(directory.root())).expect("open isolated test storage"),
    );
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

    let mut permission_rules = HashMap::new();
    permission_rules.insert("write_file".to_owned(), ToolPermissionRule::Deny);
    let executor = ToolExecutor::with_permission_rules(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::RequireApproval,
        ["write_file".to_owned()],
        permission_rules,
    );
    let call = ToolCall {
        id: "write-blocked".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({
            "path": PathBuf::from(directory.root())
                .join("blocked.txt")
                .to_str()
                .expect("fixture path should be UTF-8"),
            "content": "must not be written"
        }),
    };
    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_request");
    let snapshot = ChatStreamSnapshot::new("conversation", "message", "", 0);
    let user_questions = UserQuestionBroker::default();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);

    let result = executor
        .execute_call(
            &call,
            ToolExecutionContext {
                permissions: &broker,
                request_id: &request_id,
                snapshot: &snapshot,
                events: &events,
                cancellation: &mut cancellation,
                storage: &storage,
                run_id: "run",
                provider_id: "test-provider",
                user_questions: &user_questions,
                image_generation: None,
            },
        )
        .await
        .expect("denied tool call returns a tool result");

    assert_eq!(result.output["error"]["code"], "permission_denied");
    assert!(!Path::new(directory.root()).join("blocked.txt").exists());
    assert!(
        broker
            .pending
            .lock()
            .expect("approval registry lock")
            .is_empty()
    );
}
