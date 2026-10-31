//! Split from tests.rs (2 items).
use super::*;

#[test]
fn executor_prepares_read_tool_with_flexible_arguments() {
    use super::super::{ToolExecutor, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    directory.write("sample.txt", "line 1\nline 2\nline 3");

    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    let opencode_call = ToolCall {
        id: "call_read_1".to_owned(),
        name: "read".to_owned(),
        arguments: json!({
            "filePath": format!("{}/sample.txt", directory.root()),
            "offset": 2,
            "limit": 5
        }),
    };
    let prepared = executor
        .prepare_call(&opencode_call)
        .expect("prepare opencode read");
    assert!(matches!(
        prepared.operation,
        super::super::executor::ToolOperation::Read {
            start_line: 2,
            line_count: 5
        }
    ));

    let openchat_call = ToolCall {
        id: "call_read_2".to_owned(),
        name: "read_file".to_owned(),
        arguments: json!({
            "path": format!("{}/sample.txt", directory.root()),
            "startLine": 1,
            "lineCount": 10
        }),
    };
    let prepared_openchat = executor
        .prepare_call(&openchat_call)
        .expect("prepare openchat read_file");
    assert!(matches!(
        prepared_openchat.operation,
        super::super::executor::ToolOperation::Read {
            start_line: 1,
            line_count: 10
        }
    ));
}

#[test]
fn executor_prepares_and_handles_bash_tool() {
    use super::super::{ToolExecutor, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    let bash_call = ToolCall {
        id: "call_bash_1".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": "git status"
        }),
    };
    let prepared = executor
        .prepare_call(&bash_call)
        .expect("prepare bash call");
    assert!(matches!(
        prepared.operation,
        super::super::executor::ToolOperation::Bash { .. }
    ));

    let empty_bash_call = ToolCall {
        id: "call_bash_2".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": "   "
        }),
    };
    assert!(executor.prepare_call(&empty_bash_call).is_err());

    let send_input_call = ToolCall {
        id: "call_input_1".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": "term_test",
            "input": "yes\n"
        }),
    };
    let prepared_input = executor
        .prepare_call(&send_input_call)
        .expect("prepare send_terminal_input call");
    assert!(matches!(
        prepared_input.operation,
        super::super::executor::ToolOperation::SendTerminalInput { .. }
    ));
}
