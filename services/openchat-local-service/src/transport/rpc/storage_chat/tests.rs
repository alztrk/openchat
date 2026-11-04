use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{protocol::MAX_REQUEST_PARAMETER_NODES, storage::AppStorage};

use super::attachments::StagedAttachments;
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
async fn conversation_todos_are_scoped_persistent_and_deleted_with_the_conversation() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .expect("prepare chat schema");
    storage
        .initialize_backend_schema()
        .expect("initialize backend schema");

    for id in ["todo-conversation-a", "todo-conversation-b"] {
        dispatch(
            "storage.chat.conversations.create",
            &json!({"input":{"id":id,"title":"Todo test","providerId":"provider","modelId":"model"}}),
            &storage,
        )
        .await
        .expect("create conversation");
    }
    let created = dispatch(
        "storage.chat.todos.create",
        &json!({"conversationId":"todo-conversation-a","items":[{"title":"First step"},{"title":"Second step","description":"Details"}]}),
        &storage,
    ).await.expect("create todos");
    assert_eq!(created["todos"].as_array().map(Vec::len), Some(2));
    let todo_id = created["todos"][0]["id"].as_str().expect("todo id");

    let other_chat = dispatch(
        "storage.chat.todos.list",
        &json!({"conversationId":"todo-conversation-b"}),
        &storage,
    )
    .await
    .expect("list other conversation todos");
    assert_eq!(other_chat["todos"], json!([]));
    dispatch(
        "storage.chat.todos.create",
        &json!({"conversationId":"todo-conversation-b","items":[{"title":"Temporary todo"}]}),
        &storage,
    )
    .await
    .expect("create todo for clearing");
    let cleared = dispatch(
        "storage.chat.todos.clear",
        &json!({"conversationId":"todo-conversation-b"}),
        &storage,
    )
    .await
    .expect("clear conversation todos");
    assert_eq!(cleared["deletedCount"], 1);
    assert!(
        dispatch(
            "storage.chat.todos.update",
            &json!({"conversationId":"todo-conversation-b","todoId":todo_id,"status":"completed"}),
            &storage
        )
        .await
        .is_err()
    );

    dispatch(
        "storage.chat.todos.update",
        &json!({"conversationId":"todo-conversation-a","todoId":todo_id,"status":"completed"}),
        &storage,
    )
    .await
    .expect("complete todo");
    dispatch(
        "storage.chat.conversations.delete",
        &json!({"conversationId":"todo-conversation-a"}),
        &storage,
    )
    .await
    .expect("delete conversation");
    let connection = storage.connect().expect("connect to storage");
    let remaining: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_todos WHERE conversation_id = 'todo-conversation-a'",
            [],
            |row| row.get(0),
        )
        .expect("count deleted conversation todos");
    assert_eq!(remaining, 0);
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

async fn create_attachment_test_conversation(storage: &AppStorage) -> String {
    let conversation_id = format!("attachment-conversation-{}", Uuid::new_v4().simple());
    dispatch(
        "storage.chat.conversations.create",
        &json!({
            "input": {
                "id": conversation_id,
                "title": "Attachment save",
                "providerId": "test-provider",
                "modelId": "test-model"
            }
        }),
        storage,
    )
    .await
    .expect("create attachment test conversation");
    conversation_id
}

fn attachment_message_input(
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    bytes: &[u8],
) -> Value {
    json!({
        "input": {
            "id": message_id,
            "conversationId": conversation_id,
            "role": "user",
            "content": "Attached text file",
            "status": "completed",
            "attachments": [{
                "id": attachment_id,
                "name": "notes.txt",
                "mimeType": "text/plain",
                "kind": "text",
                "sizeBytes": bytes.len(),
                "dataBase64": STANDARD.encode(bytes)
            }]
        }
    })
}

fn initialized_chat_storage(directory: &TestDirectory) -> AppStorage {
    let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .expect("prepare current chat schema");
    storage
        .initialize_backend_schema()
        .expect("initialize current backend schema");
    storage
}

#[tokio::test]
async fn message_and_attachment_are_persisted_together_on_success() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let bytes = b"attachment content";

    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(&conversation_id, &message_id, &attachment_id, bytes),
        &storage,
    )
    .await
    .expect("save message with attachment");

    let attachment_path = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(&message_id)
        .join(format!("{attachment_id}.data"));
    assert_eq!(
        fs::read(attachment_path).expect("read saved attachment"),
        bytes
    );
    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": conversation_id}),
        &storage,
    )
    .await
    .expect("list saved message");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn message_save_rejects_attachment_metadata_that_cannot_be_read_later() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;

    for (name, mime_type) in [
        ("notes.txt", "application/octet-stream"),
        ("notes.bin", "text/plain"),
    ] {
        let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
        let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
        let mut input = attachment_message_input(
            &conversation_id,
            &message_id,
            &attachment_id,
            b"attachment content",
        );
        input["input"]["attachments"][0]["name"] = json!(name);
        input["input"]["attachments"][0]["mimeType"] = json!(mime_type);

        let error = dispatch("storage.chat.messages.save", &input, &storage)
            .await
            .expect_err("unsupported attachment metadata must be rejected");

        assert_eq!(error.code, "invalid_request_params");
        assert!(
            !storage
                .root()
                .join("attachments")
                .join(&conversation_id)
                .join(message_id)
                .exists()
        );
    }

    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": conversation_id}),
        &storage,
    )
    .await
    .expect("list messages after rejected attachment saves");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn message_commit_failure_removes_the_newly_published_attachment_directory() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let bytes = b"attachment content";
    storage
        .connect()
        .expect("connect before installing deferred commit failure")
        .execute_batch(
            "CREATE TABLE attachment_commit_parent (id INTEGER PRIMARY KEY);
             CREATE TABLE attachment_commit_child (
                 parent_id INTEGER REFERENCES attachment_commit_parent(id)
                     DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TRIGGER reject_attachment_message_commit
             AFTER INSERT ON messages
             BEGIN
                 INSERT INTO attachment_commit_child (parent_id) VALUES (1);
             END;",
        )
        .expect("install deferred commit failure");

    let error = dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(&conversation_id, &message_id, &attachment_id, bytes),
        &storage,
    )
    .await
    .expect_err("deferred foreign key must fail the message commit");

    assert_eq!(error.code, "storage_chat_unavailable");
    let attachment_directory = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(&message_id);
    assert!(!attachment_directory.exists());
    let conversation_attachment_directory = attachment_directory
        .parent()
        .expect("conversation attachment directory");
    let remaining_entries = fs::read_dir(conversation_attachment_directory)
        .expect("read attachment staging parent")
        .collect::<Result<Vec<_>, _>>()
        .expect("read attachment staging entries");
    assert!(remaining_entries.is_empty());

    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": conversation_id}),
        &storage,
    )
    .await
    .expect("list messages after rollback");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn rollback_cleanup_preserves_published_attachments_when_message_exists() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    dispatch(
        "storage.chat.messages.save",
        &json!({
            "input": {
                "id": message_id,
                "conversationId": conversation_id,
                "role": "user",
                "content": "Previously persisted message",
                "status": "completed"
            }
        }),
        &storage,
    )
    .await
    .expect("save existing message");

    let destination = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(&message_id);
    fs::create_dir_all(&destination).expect("create published attachment directory");
    let attachment_path = destination.join("attachment.data");
    fs::write(&attachment_path, b"preserve committed attachment")
        .expect("write published attachment");
    let staged = StagedAttachments {
        staging: None,
        destination,
    };
    let mut connection = storage.connect().expect("connect for cleanup");

    super::conversations::rollback_published_attachment_directory(
        &storage,
        &mut connection,
        &staged,
        &conversation_id,
        &message_id,
    );

    assert_eq!(
        fs::read(attachment_path).expect("read preserved attachment"),
        b"preserve committed attachment"
    );
}

#[tokio::test]
async fn deleting_a_conversation_reports_pending_cleanup_after_database_delete() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    fs::write(
        storage.root().join("attachments"),
        b"invalid attachment root",
    )
    .expect("create invalid attachment root");

    let result = dispatch(
        "storage.chat.conversations.delete",
        &json!({"input": {"conversationId": conversation_id}}),
        &storage,
    )
    .await
    .expect("report committed delete with pending attachment cleanup");

    assert_eq!(result["deleted"], true);
    assert_eq!(result["attachmentCleanupPending"], true);
    let conversation_exists = storage
        .connect()
        .expect("connect after delete")
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
            [&conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .expect("check conversation after delete");
    assert!(!conversation_exists);
}

#[tokio::test]
async fn clearing_history_reports_pending_cleanup_after_database_clear() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    create_attachment_test_conversation(&storage).await;
    fs::write(
        storage.root().join("attachments"),
        b"invalid attachment root",
    )
    .expect("create invalid attachment root");

    let result = dispatch("storage.chat.conversations.clear", &json!({}), &storage)
        .await
        .expect("report committed history clear with pending attachment cleanup");

    assert_eq!(result["cleared"], true);
    assert_eq!(result["attachmentCleanupPending"], true);
    let conversation_count = storage
        .connect()
        .expect("connect after history clear")
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("count conversations after clear");
    assert_eq!(conversation_count, 0);
}

#[tokio::test]
async fn conversation_delete_without_attachment_root_has_no_pending_cleanup() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;

    let result = dispatch(
        "storage.chat.conversations.delete",
        &json!({"input": {"conversationId": conversation_id}}),
        &storage,
    )
    .await
    .expect("delete a conversation without attachments");

    assert_eq!(result["deleted"], true);
    assert_eq!(result["attachmentCleanupPending"], false);
}

#[tokio::test]
async fn startup_attachment_cleanup_removes_orphans_and_preserves_live_conversation_files() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let live_conversation_id = create_attachment_test_conversation(&storage).await;
    let attachments_root = storage.root().join("attachments");
    let live_directory = attachments_root.join(&live_conversation_id);
    let orphan_directory = attachments_root.join("orphaned-conversation");
    fs::create_dir_all(&live_directory).expect("create live attachment directory");
    fs::create_dir_all(&orphan_directory).expect("create orphan attachment directory");
    let live_attachment = live_directory.join("message.data");
    let orphan_attachment = orphan_directory.join("message.data");
    fs::write(&live_attachment, b"live attachment").expect("write live attachment");
    fs::write(&orphan_attachment, b"orphan attachment").expect("write orphan attachment");

    super::cleanup_orphaned_attachment_directories(&storage)
        .expect("recover orphaned attachment directories");

    assert!(live_attachment.exists());
    assert!(!orphan_directory.exists());
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
