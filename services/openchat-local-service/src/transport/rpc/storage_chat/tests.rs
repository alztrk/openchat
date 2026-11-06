use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use tokio::sync::watch;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{protocol::MAX_REQUEST_PARAMETER_NODES, storage::AppStorage};

use super::attachments::{
    AttachmentDirectoryPublishError, StagedAttachments, publish_staged_attachment_directory,
};
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

#[tokio::test]
async fn plain_messages_starting_with_reserved_prefixes_are_escaped_before_persistence() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let contents = [
        format!(
            "{}ordinary message text",
            crate::conversation_limits::MESSAGE_CONTENT_PREFIX
        ),
        format!(
            "{}literal escape marker",
            crate::conversation_limits::MESSAGE_TEXT_PREFIX
        ),
    ];

    for (index, content) in contents.iter().enumerate() {
        let message_id = format!("reserved-prefix-message-{index}");
        dispatch(
            "storage.chat.messages.save",
            &json!({
                "input": {
                    "id": message_id,
                    "conversationId": conversation_id,
                    "role": "user",
                    "content": content,
                    "status": "completed"
                }
            }),
            &storage,
        )
        .await
        .expect("save plain message with reserved prefix");

        let stored_content = storage
            .connect()
            .expect("connect to persisted chat storage")
            .query_row(
                "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
                rusqlite::params![&conversation_id, message_id],
                |row| row.get::<_, String>(0),
            )
            .expect("read escaped plain message");
        assert_eq!(
            stored_content,
            format!(
                "{}{content}",
                crate::conversation_limits::MESSAGE_TEXT_PREFIX
            )
        );
        let (decoded, attachments) = crate::attachments::decode_message_content(
            stored_content,
            storage.root(),
            storage.database_key(),
            &conversation_id,
            &message_id,
        )
        .expect("decode escaped plain message");
        assert_eq!(decoded, *content);
        assert!(attachments.is_empty());
    }

    let legacy_content = format!(
        "{}legacy message text",
        crate::conversation_limits::MESSAGE_CONTENT_PREFIX
    );
    let connection = storage
        .connect()
        .expect("connect to legacy message storage");
    connection
        .execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, status)
             VALUES ('legacy-prefix-message', ?1, 'user', ?2, 1, 'completed')",
            rusqlite::params![&conversation_id, &legacy_content],
        )
        .expect("insert legacy plain message");
    let (decoded, attachments) = crate::attachments::decode_message_content(
        legacy_content.clone(),
        storage.root(),
        storage.database_key(),
        &conversation_id,
        "legacy-prefix-message",
    )
    .expect("malformed legacy prefix should remain readable as text");
    assert_eq!(decoded, legacy_content);
    assert!(attachments.is_empty());
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

fn read_encrypted_attachment(
    storage: &AppStorage,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    expected_plaintext: &[u8],
) -> Vec<u8> {
    let cipher =
        crate::attachment_crypto::AttachmentCipher::from_profile_key(storage.database_key())
            .expect("derive test attachment key");
    let context = crate::attachment_crypto::attachment_associated_data(std::path::Path::new(
        &format!("{conversation_id}/{message_id}/{attachment_id}.data"),
    ))
    .expect("build test attachment context");
    cipher
        .read_file(
            &storage
                .root()
                .join("attachments")
                .join(conversation_id)
                .join(message_id)
                .join(format!("{attachment_id}.data")),
            &context,
            expected_plaintext.len(),
        )
        .expect("decrypt test attachment")
        .to_vec()
}

fn write_encrypted_attachment(
    storage: &AppStorage,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    plaintext: &[u8],
    path: &std::path::Path,
) {
    let cipher =
        crate::attachment_crypto::AttachmentCipher::from_profile_key(storage.database_key())
            .expect("derive test attachment key");
    let context = crate::attachment_crypto::attachment_associated_data(std::path::Path::new(
        &format!("{conversation_id}/{message_id}/{attachment_id}.data"),
    ))
    .expect("build test attachment context");
    cipher
        .write_new_file(path, &context, plaintext)
        .expect("write encrypted test attachment");
}

#[tokio::test]
async fn attachment_library_returns_an_empty_page_for_fresh_storage() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);

    let page = dispatch(
        "storage.chat.attachments.list",
        &json!({
            "offset": 0,
            "limit": 30,
            "query": null,
            "availability": "available",
        }),
        &storage,
    )
    .await
    .expect("list attachments in fresh storage");

    assert_eq!(page, json!({"items": [], "nextOffset": null}));
}

#[tokio::test]
async fn attachment_library_lists_a_saved_attachment() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let bytes = b"saved library content";

    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(&conversation_id, &message_id, &attachment_id, bytes),
        &storage,
    )
    .await
    .expect("save message with attachment");

    let page = dispatch(
        "storage.chat.attachments.list",
        &json!({"limit": 30, "availability": "available"}),
        &storage,
    )
    .await
    .expect("list saved attachments");

    assert_eq!(page["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["items"][0]["conversationId"], conversation_id);
    assert_eq!(page["items"][0]["messageId"], message_id);
    assert_eq!(page["items"][0]["attachmentId"], attachment_id);
    assert_eq!(page["items"][0]["fileName"], "notes.txt");
    assert_eq!(page["items"][0]["sizeBytes"], bytes.len());
    assert_eq!(page["items"][0]["isAvailable"], true);
    assert_eq!(page["nextOffset"], Value::Null);
}

#[tokio::test]
async fn message_history_pages_cover_all_rows_in_stable_chronological_order() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let connection = storage.connect().expect("connect test storage");
    for index in 0..205 {
        connection
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, status)
                 VALUES (?1, ?2, 'user', ?3, 1, 'completed')",
                rusqlite::params![
                    format!("message-{index:04}"),
                    conversation_id,
                    format!("Message {index}")
                ],
            )
            .expect("insert paged message");
    }
    drop(connection);

    let mut before_message_id: Option<String> = None;
    let mut message_pages = Vec::new();
    loop {
        let mut input = json!({"conversationId": conversation_id, "limit": 100});
        if let Some(cursor) = before_message_id.as_deref() {
            input["beforeMessageId"] = json!(cursor);
        }
        let page = dispatch("storage.chat.messages.list", &input, &storage)
            .await
            .expect("read message page");
        message_pages.push(
            page["messages"]
                .as_array()
                .expect("messages array")
                .iter()
                .map(|message| message["id"].as_str().expect("message ID").to_owned())
                .collect::<Vec<_>>(),
        );

        if page["hasMore"] == false {
            assert_eq!(page["nextBeforeMessageId"], Value::Null);
            break;
        }
        before_message_id = Some(
            page["nextBeforeMessageId"]
                .as_str()
                .expect("cursor for the next older page")
                .to_owned(),
        );
    }

    assert_eq!(
        message_pages.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![100, 100, 5]
    );
    let message_ids = message_pages
        .into_iter()
        .rev()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(message_ids.len(), 205);
    assert_eq!(
        message_ids,
        (0..205)
            .map(|index| format!("message-{index:04}"))
            .collect::<Vec<_>>()
    );
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

    let synchronous = storage
        .connect()
        .expect("connect after durable attachment save")
        .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
        .expect("read restored SQLite synchronization mode");
    assert_eq!(synchronous, 1, "non-attachment writes retain NORMAL mode");

    let attachment_path = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(&message_id)
        .join(format!("{attachment_id}.data"));
    assert_ne!(
        fs::read(&attachment_path).expect("read encrypted attachment"),
        bytes
    );
    assert_eq!(
        read_encrypted_attachment(
            &storage,
            &conversation_id,
            &message_id,
            &attachment_id,
            bytes
        ),
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
async fn prefixed_attachment_envelope_must_match_the_staged_attachment_payload() {
    const BYTES: &[u8] = b"attachment content";

    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let attachment_id = "attachment-matching";
    let message_id = "prefixed-attachment-message";
    let mut matching_input =
        attachment_message_input(&conversation_id, message_id, attachment_id, BYTES);
    let envelope = format!(
        "{}{}",
        crate::conversation_limits::MESSAGE_CONTENT_PREFIX,
        json!({
            "content": "Attached text file",
            "attachments": [{
                "id": attachment_id,
                "name": "notes.txt",
                "mimeType": "text/plain",
                "sizeBytes": BYTES.len(),
                "kind": "text"
            }]
        })
    );
    matching_input["input"]["content"] = json!(&envelope);

    dispatch("storage.chat.messages.save", &matching_input, &storage)
        .await
        .expect("matching encoded metadata should persist with its bytes");

    let stored_content = storage
        .connect()
        .expect("connect to persisted chat storage")
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            rusqlite::params![&conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .expect("read persisted attachment envelope");
    assert_eq!(stored_content, envelope);
    let (content, attachments) = crate::attachments::decode_message_content(
        stored_content,
        storage.root(),
        storage.database_key(),
        &conversation_id,
        message_id,
    )
    .expect("decode matching attachment metadata and bytes");
    assert_eq!(
        content,
        "Attached text file\n\n[Attached text file: notes.txt]\nattachment content"
    );
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].content.as_deref(), Some(BYTES));

    for (index, (envelope_id, envelope_name)) in [
        ("unreferenced-attachment", "notes.txt"),
        (attachment_id, "different-name.txt"),
    ]
    .into_iter()
    .enumerate()
    {
        let mismatched_message_id = format!("mismatched-attachment-message-{index}");
        let mut input = attachment_message_input(
            &conversation_id,
            &mismatched_message_id,
            attachment_id,
            BYTES,
        );
        input["input"]["content"] = json!(format!(
            "{}{}",
            crate::conversation_limits::MESSAGE_CONTENT_PREFIX,
            json!({
                "content": "Attached text file",
                "attachments": [{
                    "id": envelope_id,
                    "name": envelope_name,
                    "mimeType": "text/plain",
                    "sizeBytes": BYTES.len(),
                    "kind": "text"
                }]
            })
        ));

        let error = dispatch("storage.chat.messages.save", &input, &storage)
            .await
            .expect_err("mismatched envelope metadata must be rejected");
        assert_eq!(error.code, "invalid_request_params");
        assert!(
            !storage
                .root()
                .join("attachments")
                .join(&conversation_id)
                .join(&mismatched_message_id)
                .exists()
        );
        let message_exists = storage
            .connect()
            .expect("connect after rejected attachment save")
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM messages WHERE conversation_id = ?1 AND id = ?2)",
                rusqlite::params![&conversation_id, &mismatched_message_id],
                |row| row.get::<_, bool>(0),
            )
            .expect("check that the rejected message is absent");
        assert!(!message_exists);
    }
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
async fn saved_gif_attachment_round_trips_through_history_decoder() {
    const GIF: &[u8] = b"GIF89aopenchat-test-image";

    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let mut input = attachment_message_input(&conversation_id, &message_id, &attachment_id, GIF);
    input["input"]["attachments"][0]["name"] = json!("sample.gif");
    input["input"]["attachments"][0]["mimeType"] = json!("image/gif");
    input["input"]["attachments"][0]["kind"] = json!("image");

    dispatch("storage.chat.messages.save", &input, &storage)
        .await
        .expect("save a GIF attachment");

    let stored_content = storage
        .connect()
        .expect("connect to persisted chat storage")
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            rusqlite::params![&conversation_id, &message_id],
            |row| row.get::<_, String>(0),
        )
        .expect("read persisted GIF message");
    let (content, attachments) = crate::attachments::decode_message_content(
        stored_content,
        storage.root(),
        storage.database_key(),
        &conversation_id,
        &message_id,
    )
    .expect("decode a persisted GIF attachment");

    assert!(content.contains("[Attached image: sample.gif]"));
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].mime_type, "image/gif");
    assert_eq!(attachments[0].content.as_deref(), Some(GIF));
}

#[tokio::test]
async fn message_upsert_to_plain_content_removes_old_attachment_directory() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let message_directory = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(&message_id);

    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(
            &conversation_id,
            &message_id,
            &attachment_id,
            b"old attachment",
        ),
        &storage,
    )
    .await
    .expect("save the original attachment message");
    assert!(message_directory.exists());

    let result = dispatch(
        "storage.chat.messages.save",
        &json!({
            "input": {
                "id": message_id,
                "conversationId": conversation_id,
                "role": "user",
                "content": "Replaced without attachment",
                "status": "completed"
            }
        }),
        &storage,
    )
    .await
    .expect("replace the message with plain content");

    assert_eq!(result["saved"], true);
    assert_eq!(result["attachmentCleanupPending"], false);
    assert!(!message_directory.exists());
    let messages = dispatch(
        "storage.chat.messages.list",
        &json!({"conversationId": conversation_id}),
        &storage,
    )
    .await
    .expect("list the replaced message");
    assert_eq!(
        messages["messages"][0]["content"],
        "Replaced without attachment"
    );
}

#[tokio::test]
async fn failed_plain_message_upsert_preserves_the_old_attachment_directory() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let bytes = b"old attachment";
    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(&conversation_id, &message_id, &attachment_id, bytes),
        &storage,
    )
    .await
    .expect("save the original attachment message");

    storage
        .connect()
        .expect("connect before installing deferred commit failure")
        .execute_batch(
            "CREATE TABLE attachment_upsert_commit_parent (id INTEGER PRIMARY KEY);
             CREATE TABLE attachment_upsert_commit_child (
                 parent_id INTEGER REFERENCES attachment_upsert_commit_parent(id)
                     DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TRIGGER reject_attachment_upsert_commit
             AFTER UPDATE ON messages
             BEGIN
                 INSERT INTO attachment_upsert_commit_child (parent_id) VALUES (1);
             END;",
        )
        .expect("install deferred update failure");

    let error = dispatch(
        "storage.chat.messages.save",
        &json!({
            "input": {
                "id": message_id,
                "conversationId": conversation_id,
                "role": "user",
                "content": "Rejected replacement",
                "status": "completed"
            }
        }),
        &storage,
    )
    .await
    .expect_err("deferred foreign key must fail the upsert commit");

    assert_eq!(error.code, "storage_chat_unavailable");
    assert_eq!(
        read_encrypted_attachment(
            &storage,
            &conversation_id,
            &message_id,
            &attachment_id,
            bytes
        ),
        bytes
    );
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
    let synchronous = storage
        .connect()
        .expect("connect after rolled-back durable attachment save")
        .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
        .expect("read restored SQLite synchronization mode");
    assert_eq!(synchronous, 1);
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
    write_encrypted_attachment(
        &storage,
        &conversation_id,
        &message_id,
        "attachment",
        b"preserve committed attachment",
        &attachment_path,
    );
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
        read_encrypted_attachment(
            &storage,
            &conversation_id,
            &message_id,
            "attachment",
            b"preserve committed attachment",
        ),
        b"preserve committed attachment"
    );
}

#[test]
fn attachment_publication_failure_keeps_the_staging_directory() {
    let directory = TestDirectory::new();
    let staging = directory.0.join(".stage-message-uuid");
    let destination = directory.0.join("message");
    fs::create_dir(&staging).expect("create staged attachment directory");
    fs::write(staging.join("attachment.data"), b"staged bytes")
        .expect("write staged attachment data");
    fs::create_dir(&destination).expect("create conflicting destination directory");
    fs::write(destination.join("existing.data"), b"existing bytes")
        .expect("write conflicting destination data");

    assert!(matches!(
        publish_staged_attachment_directory(&staging, &destination),
        Err(AttachmentDirectoryPublishError::Rename)
    ));
    assert!(
        staging.is_dir(),
        "failed publication keeps recoverable staging"
    );
    assert_eq!(
        fs::read(destination.join("existing.data")).expect("read destination contents"),
        b"existing bytes"
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
async fn startup_attachment_cleanup_removes_orphans_and_preserves_live_message_attachments() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let live_conversation_id = create_attachment_test_conversation(&storage).await;
    let attachments_root = storage.root().join("attachments");
    let live_directory = attachments_root.join(&live_conversation_id);
    let live_message_id = format!("attachment-message-{}", Uuid::new_v4().simple());
    let live_attachment_id = format!("attachment-{}", Uuid::new_v4().simple());
    let live_message_directory = live_directory.join(&live_message_id);
    let live_attachment = live_message_directory.join(format!("{live_attachment_id}.data"));
    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(
            &live_conversation_id,
            &live_message_id,
            &live_attachment_id,
            b"live attachment",
        ),
        &storage,
    )
    .await
    .expect("save a live attachment message");

    let stale_stage_directory =
        live_directory.join(format!(".stage-{live_message_id}-{}", Uuid::new_v4()));
    fs::create_dir_all(&stale_stage_directory).expect("create abandoned attachment stage");
    fs::write(
        stale_stage_directory.join("partial.data"),
        b"attachment data left before publication",
    )
    .expect("write partial attachment data");

    let orphan_message_directory = live_directory.join("orphaned-message");
    fs::create_dir_all(&orphan_message_directory).expect("create orphan message directory");
    fs::write(
        orphan_message_directory.join("orphan.data"),
        b"orphan message attachment",
    )
    .expect("write orphan message attachment");
    let orphan_directory = attachments_root.join("orphaned-conversation");
    fs::create_dir_all(&orphan_directory).expect("create orphan attachment directory");
    let orphan_attachment = orphan_directory.join("message.data");
    fs::write(&orphan_attachment, b"orphan attachment").expect("write orphan attachment");

    super::cleanup_orphaned_attachment_directories(&storage)
        .expect("recover orphaned attachment directories");
    super::recover_abandoned_attachment_staging_directories(&storage)
        .expect("remove abandoned attachment staging directories");

    assert!(live_attachment.exists());
    assert!(!stale_stage_directory.exists());
    assert!(!orphan_message_directory.exists());
    assert!(!orphan_directory.exists());
}

#[tokio::test]
async fn normal_and_cancellable_attachment_cleanup_preserve_staging_directories() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let stage_directory = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(format!(".stage-active-message-{}", Uuid::new_v4()));
    fs::create_dir_all(&stage_directory).expect("create active attachment stage");

    super::cleanup_orphaned_attachment_directories(&storage)
        .expect("normal cleanup should preserve an active stage");
    assert!(stage_directory.exists());

    let (_sender, cancellation) = watch::channel(false);
    super::cleanup_orphaned_attachment_directories_with_cancellation(&storage, &cancellation)
        .expect("cancellable cleanup should preserve an active stage");
    assert!(stage_directory.exists());
}

#[tokio::test]
async fn startup_attachment_cleanup_preserves_malformed_stage_names() {
    let directory = TestDirectory::new();
    let storage = initialized_chat_storage(&directory);
    let conversation_id = create_attachment_test_conversation(&storage).await;
    let malformed_stage = storage
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(".stage-message-not-a-generated-uuid");
    fs::create_dir_all(&malformed_stage).expect("create malformed staging directory");

    let result = super::recover_abandoned_attachment_staging_directories(&storage);

    assert!(result.is_err());
    assert!(malformed_stage.exists());
}

#[tokio::test]
async fn archive_restore_recovers_interrupted_attachment_and_stage_files_before_retry() {
    let source_directory = TestDirectory::new();
    let source = initialized_chat_storage(&source_directory);
    let conversation_id = create_attachment_test_conversation(&source).await;
    let message_id = "archive-recovery-message";
    let attachment_id = "archive-recovery-attachment";
    let attachment_bytes = b"restored after interrupted import";
    dispatch(
        "storage.chat.messages.save",
        &attachment_message_input(
            &conversation_id,
            message_id,
            attachment_id,
            attachment_bytes,
        ),
        &source,
    )
    .await
    .expect("save source archive attachment");
    let passphrase = "archive-recovery-passphrase-2026";
    let (_cancel_sender, cancellation) = watch::channel(false);
    let exported = crate::conversation_archive::export(
        &source,
        &json!(1),
        std::slice::from_ref(&conversation_id),
        Zeroizing::new(passphrase.to_owned()),
        &cancellation,
        &crate::service::OperationGate::default(),
    )
    .expect("export source archive");
    let export_id = exported["exportId"]
        .as_str()
        .expect("export should return its identifier");
    let archive_path = source
        .root()
        .join("cache/archive-exports")
        .join(format!("{export_id}.openchatbackup"));

    let destination_directory = TestDirectory::new();
    let destination = initialized_chat_storage(&destination_directory);
    let orphan_directory = destination
        .root()
        .join("attachments")
        .join(&conversation_id)
        .join(message_id);
    fs::create_dir_all(&orphan_directory).expect("create interrupted restore attachment path");
    fs::write(
        orphan_directory.join(format!("{attachment_id}.data")),
        b"partial restore before database commit",
    )
    .expect("write interrupted restore attachment");
    let stale_stage_directory = destination
        .root()
        .join("cache/conversation-archives/restore-interrupted-process");
    fs::create_dir_all(&stale_stage_directory).expect("create interrupted restore stage");
    fs::write(
        stale_stage_directory.join("staged.data"),
        b"staged archive data",
    )
    .expect("write interrupted restore stage");

    let (_sender, mut cancellation) = watch::channel(false);
    let mut params = json!({
        "path": archive_path.to_string_lossy().into_owned(),
        "passphrase": passphrase,
        "conflictPolicy": "skip_existing"
    });
    let restored = super::super::conversations::dispatch(
        "conversation.archive.restore",
        &mut params,
        &json!("archive-recovery-test"),
        &destination,
        &crate::protocol::EventSink::new(),
        &mut cancellation,
        &crate::service::OperationGate::default(),
    )
    .await
    .expect("restore should recover abandoned files and retry successfully");

    assert_eq!(restored["restoredConversationCount"], json!(1));
    assert_eq!(restored["restoredAttachmentCount"], json!(1));
    assert!(!stale_stage_directory.exists());
    assert_eq!(
        read_encrypted_attachment(
            &destination,
            &conversation_id,
            message_id,
            attachment_id,
            attachment_bytes,
        ),
        attachment_bytes
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
