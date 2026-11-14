use std::{fs, path::PathBuf};

use serde_json::json;
use uuid::Uuid;

use crate::storage::AppStorage;

use super::dispatch;

#[tokio::test]
async fn workflow_lifecycle_prompt_survives_encrypted_conversation_storage_reopen() {
    let directory = TestDirectory::new();
    let database_path = directory.0.join("data");
    {
        let storage = initialized_chat_storage(&database_path);
        dispatch(
            "storage.chat.conversations.create",
            &json!({
                "input": {
                    "id": "workflow-lifecycle-persistence",
                    "title": "Workflow lifecycle",
                    "providerId": "test-provider",
                    "modelId": "test-model",
                    "agent": {
                        "id": "a0000000-0000-4000-8000-000000000001",
                        "name": "Workflow",
                        "instructions": "",
                        "responseStyle": "balanced",
                        "toolAllowlist": null,
                        "workflowHooks": {
                            "afterResponsePrompt": "Review {{project}} next."
                        }
                    }
                }
            }),
            &storage,
        )
        .await
        .expect("create conversation with workflow lifecycle prompt");
    }

    let reopened_storage = initialized_chat_storage(&database_path);
    let page = dispatch(
        "storage.chat.conversations.list",
        &json!({"input": {"offset": 0}}),
        &reopened_storage,
    )
    .await
    .expect("list conversations after reopening encrypted storage");

    let conversations = page["conversations"]
        .as_array()
        .expect("conversation list array");
    let conversation = conversations
        .iter()
        .find(|conversation| conversation["id"] == "workflow-lifecycle-persistence")
        .expect("persisted workflow conversation");
    assert_eq!(
        conversation["agent"]["workflowHooks"]["afterResponsePrompt"],
        "Review {{project}} next."
    );
}

fn initialized_chat_storage(database_path: &std::path::Path) -> AppStorage {
    let storage =
        AppStorage::open_at(database_path.to_path_buf()).expect("open encrypted test storage");
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .expect("prepare current chat schema");
    storage
        .initialize_backend_schema()
        .expect("initialize current backend schema");
    storage
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("openchat-workflow-storage-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}
