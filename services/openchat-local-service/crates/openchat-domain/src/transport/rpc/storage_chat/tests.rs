use std::{fs, path::PathBuf};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{protocol::MAX_REQUEST_PARAMETER_NODES, storage::AppStorage};

use super::{
    MAX_AGENT_METADATA_BYTES, MAX_STRUCTURED_METADATA_JSON_NESTING, dispatch, optional_json_array,
    parse_stored_agent_json, sanitize_stored_metadata, sanitize_tool_activity_metadata,
    serialize_json_without_sensitive_values,
};

#[test]
fn rejects_embedded_metadata_json_over_the_node_limit() {
    let nested_json = format!(
        "[{}]",
        std::iter::repeat_n("0", MAX_REQUEST_PARAMETER_NODES)
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut metadata = json!({"arguments": nested_json});
    let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;

    assert!(sanitize_tool_activity_metadata(&mut metadata, &mut remaining_nodes, 0).is_err());
}

#[test]
fn preserves_non_json_tool_activity_text_that_starts_with_a_delimiter() {
    let mut metadata = json!([{
        "arguments": "{tool output text",
        "output": "\"unterminated"
    }]);
    let original = metadata.clone();
    let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;

    assert!(
        !sanitize_tool_activity_metadata(&mut metadata, &mut remaining_nodes, 0)
            .expect("preserve ordinary tool text")
    );
    assert_eq!(metadata, original);
}

#[test]
fn preserves_invalid_deep_tool_activity_text() {
    let invalid_nested_text = [
        "[".repeat(MAX_STRUCTURED_METADATA_JSON_NESTING),
        format!(
            "{}x{}",
            "[".repeat(MAX_STRUCTURED_METADATA_JSON_NESTING),
            "]".repeat(MAX_STRUCTURED_METADATA_JSON_NESTING)
        ),
    ];

    for text in invalid_nested_text {
        let mut metadata = json!([{"arguments": text}]);
        let original = metadata.clone();
        let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;

        assert!(
            !sanitize_tool_activity_metadata(&mut metadata, &mut remaining_nodes, 0)
                .expect("preserve invalid deep tool text")
        );
        assert_eq!(metadata, original);
    }
}

#[test]
fn rejects_structured_tool_metadata_over_the_nesting_limit() {
    let nested_json = format!(
        "{}0{}",
        "[".repeat(MAX_STRUCTURED_METADATA_JSON_NESTING),
        "]".repeat(MAX_STRUCTURED_METADATA_JSON_NESTING)
    );
    let mut metadata = json!([{"arguments": nested_json}]);
    let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;

    assert!(sanitize_tool_activity_metadata(&mut metadata, &mut remaining_nodes, 0).is_err());
}

#[test]
fn metadata_json_strings_share_one_node_budget() {
    let values = std::iter::repeat_n("0", MAX_REQUEST_PARAMETER_NODES / 2)
        .collect::<Vec<_>>()
        .join(",");
    let input = json!({
        "citationSources": format!("[{values}]"),
        "personalMemoryFactIds": format!("[{values}]")
    });
    let object = input.as_object().expect("metadata input is an object");
    let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;

    optional_json_array(object, "citationSources", &mut remaining_nodes)
        .expect("first bounded metadata array");
    assert!(optional_json_array(object, "personalMemoryFactIds", &mut remaining_nodes).is_err());
}

#[test]
fn serialized_agent_size_is_bounded() {
    let value = json!({"instructions": "x".repeat(MAX_AGENT_METADATA_BYTES)});

    assert!(serialize_json_without_sensitive_values(&value).is_err());
}

#[test]
fn stored_agent_json_is_size_checked_before_parsing() {
    let oversized = " ".repeat(MAX_AGENT_METADATA_BYTES + 1);

    assert!(parse_stored_agent_json(oversized).is_err());
}

#[tokio::test]
async fn persisted_conversation_and_message_metadata_clear_credential_fields() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .expect("prepare current chat schema");
    storage
        .initialize_backend_schema()
        .expect("initialize current backend schema");

    dispatch(
        "storage.chat.conversations.create",
        &json!({
            "input": {
                "id": "credential-test-conversation",
                "title": "Credential cleanup",
                "providerId": "provider",
                "modelId": "model",
                "agent": {
                    "name": "Coding",
                    "instructions": "Use this JSON example: {\"credential\":\"placeholder\"}; preserve deeply nested JSON text: {\"a\":{\"b\":{\"c\":{\"d\":{\"e\":1}}}}}",
                    "credential": "agent-secret"
                }
            }
        }),
        &storage,
    )
    .await
    .expect("create conversation");

    let conversation = dispatch(
        "storage.chat.conversations.get",
        &json!({"conversationId": "credential-test-conversation"}),
        &storage,
    )
    .await
    .expect("read conversation");
    assert_eq!(conversation["conversation"]["agent"]["credential"], "");
    assert_eq!(conversation["conversation"]["agent"]["name"], "Coding");
    assert_eq!(
        conversation["conversation"]["agent"]["instructions"],
        "Use this JSON example: {\"credential\":\"placeholder\"}; preserve deeply nested JSON text: {\"a\":{\"b\":{\"c\":{\"d\":{\"e\":1}}}}}"
    );
    storage
        .connect()
        .expect("reopen test storage")
        .execute(
            "UPDATE conversations SET agent_json = ?1 WHERE id = ?2",
            [
                r#"{"name":"Coding","instructions":"Keep {\"credential\":\"example\"} as text.","credential":"legacy-agent-secret"}"#,
                "credential-test-conversation",
            ],
        )
        .expect("insert legacy conversation credential");
    let legacy_conversation = dispatch(
        "storage.chat.conversations.get",
        &json!({"conversationId": "credential-test-conversation"}),
        &storage,
    )
    .await
    .expect("read legacy conversation");
    assert_eq!(
        legacy_conversation["conversation"]["agent"]["credential"],
        ""
    );
    assert_eq!(
        legacy_conversation["conversation"]["agent"]["instructions"],
        "Keep {\"credential\":\"example\"} as text."
    );

    let tool_activities = r#"[{"name":"search","credential":"tool-secret","arguments":"{\"credential\":\"argument-secret\",\"query\":\"keep\"}","output":"{\"passphrase\":\"output-secret\",\"result\":\"keep\"}"},{"name":"double-encoded","arguments":"\"{\\\"credential\\\":\\\"double-encoded-secret\\\",\\\"query\\\":\\\"keep-double\\\"}\""}]"#;
    dispatch(
        "storage.chat.messages.save",
        &json!({
            "input": {
                "id": "assistant-message",
                "conversationId": "credential-test-conversation",
                "role": "assistant",
                "content": "Saved response",
                "status": "completed",
                "citationSources": [{
                    "credential": "citation-secret",
                    "label": "Reference"
                }],
                "toolActivities": tool_activities
            }
        }),
        &storage,
    )
    .await
    .expect("save message");

    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": "credential-test-conversation"}),
        &storage,
    )
    .await
    .expect("read messages");
    let message = &messages["messages"][0];
    let citations: Value = serde_json::from_str(
        message["citationSources"]
            .as_str()
            .expect("citation metadata is stored as JSON"),
    )
    .expect("parse citation metadata");
    let activities: Value = serde_json::from_str(
        message["toolActivities"]
            .as_str()
            .expect("tool activities are stored as JSON"),
    )
    .expect("parse tool activity metadata");
    let arguments: Value = serde_json::from_str(
        activities[0]["arguments"]
            .as_str()
            .expect("tool arguments remain JSON text"),
    )
    .expect("parse tool argument metadata");
    let output: Value = serde_json::from_str(
        activities[0]["output"]
            .as_str()
            .expect("tool output remains JSON text"),
    )
    .expect("parse tool output metadata");
    assert_eq!(citations[0]["credential"], "");
    assert_eq!(citations[0]["label"], "Reference");
    assert_eq!(activities[0]["credential"], "");
    assert_eq!(arguments["credential"], "");
    assert_eq!(arguments["query"], "keep");
    assert_eq!(output["passphrase"], "");
    assert_eq!(output["result"], "keep");
    let double_encoded_arguments: Value = serde_json::from_str(
        activities[1]["arguments"]
            .as_str()
            .expect("double-encoded arguments remain JSON text"),
    )
    .expect("parse double-encoded argument layer");
    let double_encoded_arguments: Value = serde_json::from_str(
        double_encoded_arguments
            .as_str()
            .expect("first decoded layer remains a string"),
    )
    .expect("parse double-encoded argument object");
    assert_eq!(double_encoded_arguments["credential"], "");
    assert_eq!(double_encoded_arguments["query"], "keep-double");

    let legacy_activity_json = r#"[{"arguments":"{\"credential\":\"legacy-secret\"}"}]"#;
    let mut legacy_node_budget = MAX_REQUEST_PARAMETER_NODES;
    let sanitized_legacy_json = sanitize_stored_metadata(
        legacy_activity_json.to_owned(),
        13,
        true,
        &mut legacy_node_budget,
    )
    .expect("sanitize legacy metadata JSON");
    let sanitized_legacy_activities: Value =
        serde_json::from_str(&sanitized_legacy_json).expect("parse sanitized legacy metadata JSON");
    let sanitized_legacy_arguments: Value = serde_json::from_str(
        sanitized_legacy_activities[0]["arguments"]
            .as_str()
            .expect("legacy arguments remain JSON text"),
    )
    .expect("parse sanitized legacy arguments");
    assert_eq!(sanitized_legacy_arguments["credential"], "");
    storage
        .connect()
        .expect("reopen test storage")
        .execute(
            "UPDATE messages
             SET tool_activities = ?1
             WHERE id = 'assistant-message'",
            [legacy_activity_json],
        )
        .expect("insert legacy credential metadata");
    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": "credential-test-conversation"}),
        &storage,
    )
    .await
    .expect("read legacy messages");
    let legacy_activities: Value = serde_json::from_str(
        messages["messages"][0]["toolActivities"]
            .as_str()
            .expect("legacy activity metadata is a string"),
    )
    .expect("parse sanitized legacy activity metadata");
    let legacy_arguments: Value = serde_json::from_str(
        legacy_activities[0]["arguments"]
            .as_str()
            .expect("legacy arguments remain JSON text"),
    )
    .expect("parse sanitized legacy arguments");
    assert_eq!(legacy_arguments["credential"], "");
}

#[tokio::test]
async fn added_workspace_root_returns_canonical_path_and_can_be_removed() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .expect("prepare current chat schema");
    storage
        .initialize_backend_schema()
        .expect("initialize current backend schema");

    let primary_folder = directory.0.join("primary");
    let secondary_folder = directory.0.join("secondary");
    fs::create_dir(&primary_folder).expect("create primary workspace root");
    fs::create_dir(&secondary_folder).expect("create secondary workspace root");
    let primary_path = fs::canonicalize(&primary_folder)
        .expect("canonicalize primary root")
        .to_str()
        .expect("primary path is valid UTF-8")
        .to_owned();
    let secondary_canonical_path = fs::canonicalize(&secondary_folder)
        .expect("canonicalize secondary root")
        .to_str()
        .expect("secondary path is valid UTF-8")
        .to_owned();
    let secondary_input_path = directory
        .0
        .join(".")
        .join("secondary")
        .to_str()
        .expect("secondary input path is valid UTF-8")
        .to_owned();

    {
        let connection = storage.connect().expect("open project fixture connection");
        connection
            .execute(
                "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 0)",
                rusqlite::params!["test-project", "Test project", &primary_path],
            )
            .expect("insert project fixture");
        connection
            .execute(
                "INSERT INTO project_workspace_roots
                    (root_id, project_id, root_path, position, created_at)
                 VALUES (lower(hex(randomblob(16))), ?1, ?2, 0, 0)",
                rusqlite::params!["test-project", &primary_path],
            )
            .expect("insert primary workspace root fixture");
    }

    let added = dispatch(
        "storage.chat.projects.workspace.add",
        &json!({
            "input": {
                "projectId": "test-project",
                "folderPath": secondary_input_path
            }
        }),
        &storage,
    )
    .await
    .expect("add secondary workspace root");
    let added_path = added["rootPath"]
        .as_str()
        .expect("added workspace root path");
    assert_eq!(added_path, secondary_canonical_path);
    assert_ne!(added_path, secondary_input_path);

    let projects = dispatch("storage.chat.projects.list", &Value::Null, &storage)
        .await
        .expect("list project workspace roots");
    assert_eq!(
        projects["projects"][0]["workspaceRoots"][0],
        json!(primary_path)
    );
    assert_eq!(
        projects["projects"][0]["workspaceRoots"][1],
        json!(added_path)
    );

    let duplicate = dispatch(
        "storage.chat.projects.workspace.add",
        &json!({
            "input": {
                "projectId": "test-project",
                "folderPath": primary_path
            }
        }),
        &storage,
    )
    .await
    .expect_err("overlapping workspace roots are rejected");
    assert_eq!(duplicate.code, "invalid_request_params");

    let protected_primary = dispatch(
        "storage.chat.projects.workspace.remove",
        &json!({
            "input": {
                "projectId": "test-project",
                "rootPath": primary_path
            }
        }),
        &storage,
    )
    .await
    .expect_err("primary workspace root cannot be removed");
    assert_eq!(protected_primary.code, "storage_chat_not_found");

    dispatch(
        "storage.chat.projects.workspace.remove",
        &json!({
            "input": {
                "projectId": "test-project",
                "rootPath": added_path
            }
        }),
        &storage,
    )
    .await
    .expect("remove secondary workspace root by canonical path");

    let projects = dispatch("storage.chat.projects.list", &Value::Null, &storage)
        .await
        .expect("list project workspace roots after removal");
    assert_eq!(
        projects["projects"][0]["workspaceRoots"],
        json!([primary_path])
    );
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("openchat-storage-chat-rpc-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}
