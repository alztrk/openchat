//! RPC handlers for project configuration, worktrees, tools, and MCP credentials.

use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    git_worktrees,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    storage::AppStorage,
    tools::{self, ToolPermissionRule},
};

use super::{invalid_request_params, required_secret_string, required_string, tool_permissions};

pub(super) async fn dispatch(
    method: &str,
    params: &mut Value,
    request_id: &Value,
    storage: &AppStorage,
    permission_broker: &ToolPermissionBroker,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "project.tool_permissions.get" => {
            let project_id = required_string(params, "projectId")?;
            let rules = tool_permissions::get_project_rules(
                storage,
                project_id,
                params.get("initialRules"),
            )?;
            Ok(json!({"rules": tool_permissions::rules_to_value(&rules)}))
        }
        "project.tool_permissions.set" => {
            let project_id = required_string(params, "projectId")?;
            let rules = params.get("rules").ok_or_else(invalid_request_params)?;
            let rules = tool_permissions::save_project_rules(storage, project_id, rules)?;
            Ok(json!({"rules": tool_permissions::rules_to_value(&rules)}))
        }
        "project.worktrees.list" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let project_root = project_root.to_string_lossy().into_owned();
            git_worktrees::list(storage, project_id, &project_root)
                .await
                .map_err(git_worktree_error)
        }
        "project.instructions.get" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let instructions = crate::project_instructions::load(&project_root)?;
            Ok(json!({"instructions": instructions.unwrap_or_default()}))
        }
        "project.instructions.save" => {
            let project_id = required_string(params, "projectId")?;
            let instructions = params
                .get("instructions")
                .and_then(Value::as_str)
                .ok_or_else(invalid_request_params)?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            crate::project_instructions::save(&project_root, instructions)?;
            Ok(json!({"saved": true}))
        }
        "project.skills.list" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let skills = crate::project_skills::list(&project_root)?;
            Ok(json!({
                "skills": skills
                    .iter()
                    .map(|skill| json!({"id": skill.id, "content": skill.content}))
                    .collect::<Vec<_>>()
            }))
        }
        "project.index.get" => {
            let project_id = required_string(params, "projectId")?;
            let index_path = storage.root().join("cache").join("project-index.sqlite3");
            Ok(json!({"index": crate::project_index::snapshot(&index_path, project_id)?}))
        }
        "project.index.set_enabled" => {
            let project_id = required_string(params, "projectId")?;
            let enabled = params
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(invalid_request_params)?;
            let index_path = storage.root().join("cache").join("project-index.sqlite3");
            Ok(json!({
                "index": crate::project_index::set_enabled(&index_path, project_id, enabled)?
            }))
        }
        "project.index.sync" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let index_path = storage.root().join("cache").join("project-index.sqlite3");
            Ok(json!({
                "index": crate::project_index::sync(&index_path, project_id, &project_root)?
            }))
        }
        "project.index.clear" => {
            let project_id = required_string(params, "projectId")?;
            let index_path = storage.root().join("cache").join("project-index.sqlite3");
            Ok(json!({"index": crate::project_index::clear(&index_path, project_id)?}))
        }
        "project.mcp.catalog.get" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            tools::mcp::server_catalog(&project_root)
        }
        "project.mcp.catalog.save" => {
            let project_id = required_string(params, "projectId")?;
            let catalog = params.get("catalog").ok_or_else(invalid_request_params)?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            tools::mcp::save_server_catalog(&project_root, catalog)
        }
        "project.mcp.server.check" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let server = params.get("server").ok_or_else(invalid_request_params)?;
            let config = tools::mcp::parse_server_config(&project_root, project_id, server)?;
            if !config.enabled {
                return Ok(json!({"status": "disabled", "toolCount": 0, "toolNames": []}));
            }
            let server_id = config.id.clone();
            let registry = tools::mcp::McpRegistry::connect(vec![config])
                .await
                .map_err(|_| {
                    ServiceError::new(
                        "mcp_server_unavailable",
                        "The MCP server could not start or provide its tool list.",
                        true,
                    )
                })?;
            let tool_names = registry
                .as_ref()
                .map(|registry| registry.tool_names_for_server(&server_id))
                .unwrap_or_default();
            Ok(json!({
                "status": "connected",
                "toolCount": tool_names.len(),
                "toolNames": tool_names
            }))
        }
        "project.mcp.credential.set" => {
            let project_id = required_string(params, "projectId")?.to_owned();
            let server_id = required_string(params, "serverId")?.to_owned();
            let environment_name = required_string(params, "environmentName")?.to_owned();
            let secret = required_secret_string(params, "secret")?;
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            crate::credentials::CredentialStore
                .store_mcp_secret(&reference, secret)
                .map_err(mcp_credential_error)?;
            Ok(json!({"stored": true}))
        }
        "project.mcp.credential.remove" => {
            let project_id = required_string(params, "projectId")?.to_owned();
            let server_id = required_string(params, "serverId")?.to_owned();
            let environment_name = required_string(params, "environmentName")?.to_owned();
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            crate::credentials::CredentialStore
                .delete_mcp_secret(&reference)
                .map_err(mcp_credential_error)?;
            Ok(json!({"removed": true}))
        }
        "project.mcp.credential.status" => {
            let project_id = required_string(params, "projectId")?.to_owned();
            let server_id = required_string(params, "serverId")?.to_owned();
            let environment_name = required_string(params, "environmentName")?.to_owned();
            project_root_for_mcp(storage, &project_id)?;
            let reference = crate::credentials::McpCredentialReference::new(
                project_id,
                server_id,
                environment_name,
            )
            .map_err(|_| invalid_request_params())?;
            let stored = crate::credentials::CredentialStore
                .has_mcp_secret(&reference)
                .map_err(mcp_credential_error)?;
            Ok(json!({"stored": stored}))
        }
        "project.worktrees.create" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let project_root = project_root.to_string_lossy().into_owned();
            git_worktrees::create(storage, project_id, &project_root)
                .await
                .map_err(git_worktree_error)
        }
        "project.worktrees.review" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let project_root = project_root.to_string_lossy().into_owned();
            let worktree_id = required_string(params, "worktreeId")?;
            git_worktrees::review(storage, project_id, &project_root, worktree_id)
                .await
                .map_err(git_worktree_error)
        }
        "project.worktrees.tasks" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let project_root = project_root.to_string_lossy().into_owned();
            let worktree_id = required_string(params, "worktreeId")?;
            let task_root = git_worktrees::project_root_for_worktree(
                storage,
                project_id,
                &project_root,
                worktree_id,
            )
            .await
            .map_err(git_worktree_error)?;
            let tasks = crate::tools::project_tasks::load_project_tasks(&task_root)?;
            Ok(json!({"tasks": tasks}))
        }
        "project.tasks.list" => {
            let project_id = required_string(params, "projectId")?;
            let tasks = project_tasks_for_project(storage, project_id)?;
            Ok(json!({"tasks": tasks}))
        }
        "project.tools.list" => {
            let project_id = required_string(params, "projectId")?;
            let root = project_root_for_mcp(storage, project_id)?;
            let tools = crate::tools::project_tools::load_if_present(&root)?;
            Ok(json!({
                "tools": tools
                    .iter()
                    .map(|tool| json!({"id": tool.id, "description": tool.description}))
                    .collect::<Vec<_>>()
            }))
        }
        "project.worktrees.run_task" => {
            let project_id = required_string(params, "projectId")?;
            let worktree_id = required_string(params, "worktreeId")?;
            let task_id = required_string(params, "taskId")?;
            let expected_command = required_string(params, "expectedCommand")?;
            if params.get("toolPermissionRules").is_some()
                || params.get("toolPermissionMode").is_some()
            {
                return Err(invalid_request_params());
            }
            let task_root = project_task_root(storage, project_id, worktree_id).await?;
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "tool_permission_settings_unavailable",
                    "Tool permission settings could not be accessed safely.",
                    true,
                )
            })?;
            let permission_rules = tool_permissions::load_project_rules(&connection, project_id)?;
            let task_permission_rule =
                tools::project_task_permission_rule(&permission_rules, task_id);
            if task_permission_rule == Some(&ToolPermissionRule::Deny) {
                return Err(ServiceError::new(
                    "permission_denied",
                    "The project permission rules deny named task execution.",
                    false,
                ));
            }
            let expected_timeout = params
                .get("expectedTimeoutSeconds")
                .and_then(Value::as_u64)
                .filter(|seconds| (5..=600).contains(seconds))
                .ok_or_else(invalid_request_params)?;
            let task = crate::tools::project_tasks::load_project_task(&task_root, task_id)?;
            if task.command != expected_command || task.timeout_seconds != expected_timeout {
                return Err(ServiceError::new(
                    "project_task_changed",
                    "The named task changed after it was displayed. Reload the task list and review it again.",
                    false,
                ));
            }
            let approved = permission_broker
                .request_approval(
                    request_id,
                    "run_project_task",
                    &task_root.to_string_lossy(),
                    &json!({
                        "taskId": task.id,
                        "command": task.command,
                        "timeoutSeconds": task.timeout_seconds,
                    }),
                    events,
                    cancellation,
                )
                .await?;
            if !approved {
                return Err(ServiceError::new(
                    "permission_denied",
                    "The named project task was not approved.",
                    false,
                ));
            }

            // Re-read after the approval wait so changed project tasks cannot execute unnoticed.
            let task_root = project_task_root(storage, project_id, worktree_id).await?;
            let task = crate::tools::project_tasks::load_project_task(&task_root, task_id)?;
            if task.command != expected_command || task.timeout_seconds != expected_timeout {
                return Err(ServiceError::new(
                    "project_task_changed",
                    "The named task changed while approval was pending. Review it again before running.",
                    false,
                ));
            }
            crate::tools::terminal::TerminalSessionManager::global()
                .execute_to_completion(
                    &task.command,
                    &task_root,
                    task.timeout_seconds,
                    cancellation,
                )
                .await
                .map_err(|_| {
                    ServiceError::new(
                        "project_task_execution_failed",
                        "The named task could not run inside the process sandbox.",
                        true,
                    )
                })
        }
        "project.worktrees.remove" => {
            let project_id = required_string(params, "projectId")?;
            let project_root = project_root_for_mcp(storage, project_id)?;
            let project_root = project_root.to_string_lossy().into_owned();
            let worktree_id = required_string(params, "worktreeId")?;
            git_worktrees::remove(storage, project_id, &project_root, worktree_id)
                .await
                .map_err(git_worktree_error)
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

/// Resolves a registered project's managed worktree before loading its task catalog.
async fn project_task_root(
    storage: &AppStorage,
    project_id: &str,
    worktree_id: &str,
) -> Result<PathBuf, ServiceError> {
    let project_root = project_root_for_mcp(storage, project_id)?;
    let project_root = project_root.to_string_lossy().into_owned();
    git_worktrees::project_root_for_worktree(storage, project_id, &project_root, worktree_id)
        .await
        .map_err(git_worktree_error)
}

/// Resolves and canonicalizes a project folder from the saved project record.
///
/// RPC callers must use this path instead of accepting a renderer-supplied project root.
pub(super) fn project_root_for_mcp(
    storage: &AppStorage,
    project_id: &str,
) -> Result<PathBuf, ServiceError> {
    let connection = storage.connect().map_err(|_| {
        ServiceError::new(
            "mcp_catalog_unavailable",
            "The project MCP configuration could not be accessed.",
            true,
        )
    })?;
    let folder_path = connection
        .query_row(
            "SELECT folder_path FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| {
            ServiceError::new(
                "mcp_catalog_unavailable",
                "The project MCP configuration could not be accessed.",
                true,
            )
        })?
        .ok_or_else(|| {
            ServiceError::new(
                "project_unavailable",
                "The selected project is no longer available.",
                false,
            )
        })?;
    let root = std::fs::canonicalize(folder_path).map_err(|_| {
        ServiceError::new(
            "project_unavailable",
            "The selected project folder is unavailable.",
            false,
        )
    })?;
    if !root.is_dir() {
        return Err(ServiceError::new(
            "project_unavailable",
            "The selected project folder is unavailable.",
            false,
        ));
    }
    Ok(root)
}

pub(super) fn project_tasks_for_project(
    storage: &AppStorage,
    project_id: &str,
) -> Result<Vec<crate::tools::project_tasks::ProjectTask>, ServiceError> {
    let connection = storage.connect().map_err(|_| {
        ServiceError::new(
            "storage_unavailable",
            "Project task permissions could not be read.",
            true,
        )
    })?;
    let project_root = connection
        .query_row(
            "SELECT folder_path FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| {
            ServiceError::new(
                "storage_unavailable",
                "Project task permissions could not be read.",
                true,
            )
        })?
        .ok_or_else(|| {
            ServiceError::new(
                "project_unavailable",
                "The selected project is no longer available.",
                false,
            )
        })?;
    crate::tools::project_tasks::load_project_tasks_if_present(Path::new(&project_root))
}

fn mcp_credential_error(error: crate::credentials::CredentialStoreError) -> ServiceError {
    use crate::credentials::CredentialStoreError;

    match error {
        CredentialStoreError::InvalidReference => invalid_request_params(),
        CredentialStoreError::EmptyToken => ServiceError::new(
            "mcp_credential_empty",
            "Enter a non-empty MCP credential value.",
            false,
        ),
        CredentialStoreError::TokenTooLarge => ServiceError::new(
            "mcp_credential_too_large",
            "An MCP credential exceeds the 2,500-byte limit.",
            false,
        ),
        _ => ServiceError::new(
            "mcp_credential_storage_unavailable",
            "The MCP credential could not be accessed in Windows Credential Manager.",
            true,
        ),
    }
}

fn git_worktree_error(error: git_worktrees::GitWorktreeError) -> ServiceError {
    use git_worktrees::GitWorktreeError;

    match error {
        GitWorktreeError::InvalidInput => ServiceError::new(
            "invalid_request_params",
            "The project worktree request is invalid.",
            false,
        ),
        GitWorktreeError::ProjectUnavailable => ServiceError::new(
            "project_unavailable",
            "The project folder or selected worktree is unavailable.",
            false,
        ),
        GitWorktreeError::NotRepository => ServiceError::new(
            "project_not_git_repository",
            "This project folder is not inside a Git repository.",
            false,
        ),
        GitWorktreeError::UnsafePath => ServiceError::new(
            "project_worktree_path_unsafe",
            "The project worktree path failed a safety check.",
            false,
        ),
        GitWorktreeError::GitFailed => ServiceError::new(
            "project_worktree_operation_failed",
            "Git could not complete the worktree operation. Check the repository state and try again.",
            false,
        ),
        GitWorktreeError::TimedOut => ServiceError::new(
            "project_worktree_timed_out",
            "Git did not finish the worktree operation before the time limit.",
            true,
        ),
        GitWorktreeError::Unavailable => ServiceError::new(
            "project_worktree_unavailable",
            "The Git worktree service is unavailable.",
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process::Command, sync::Arc};

    use serde_json::json;
    use tokio::sync::watch;
    use tokio::time::timeout;
    use uuid::Uuid;

    use crate::{
        git_worktrees, permissions::ToolPermissionBroker, protocol::EventSink, storage::AppStorage,
    };

    use super::dispatch;

    #[tokio::test]
    async fn project_dispatch_preserves_known_and_unknown_method_errors() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let (_sender, mut cancellation) = watch::channel(false);

        let mut known_params = json!({});
        let known_error = dispatch(
            "project.instructions.get",
            &mut known_params,
            &json!("known-request"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect_err("missing project id is rejected");
        assert_eq!(known_error.code, "invalid_request_params");

        let mut unknown_params = json!({});
        let unknown_error = dispatch(
            "project.unavailable.method",
            &mut unknown_params,
            &json!("unknown-request"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect_err("unknown project method is rejected");
        assert_eq!(unknown_error.code, "method_not_found");
    }

    #[tokio::test]
    async fn project_permission_rpc_imports_once_and_reads_service_owned_rules() {
        let (_directory, storage, _worktree_id, _worktree_path) = create_task_test_fixture().await;
        let (_sender, mut cancellation) = watch::channel(false);
        let mut first_read = json!({
            "projectId": "project-1",
            "initialRules": {"run_project_task": "deny"}
        });
        let first = dispatch(
            "project.tool_permissions.get",
            &mut first_read,
            &json!("first-read"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect("import and read project rules");
        assert_eq!(first["rules"]["run_project_task"], "deny");

        let mut repeated_read = json!({
            "projectId": "project-1",
            "initialRules": {"run_project_task": "allow"}
        });
        let repeated = dispatch(
            "project.tool_permissions.get",
            &mut repeated_read,
            &json!("second-read"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect("read existing service rules");
        assert_eq!(repeated["rules"]["run_project_task"], "deny");

        let mut update = json!({
            "projectId": "project-1",
            "rules": {"run_project_task": "ask"}
        });
        let saved = dispatch(
            "project.tool_permissions.set",
            &mut update,
            &json!("save-rules"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect("save a project rule update");
        assert_eq!(saved["rules"]["run_project_task"], "ask");
    }

    #[tokio::test]
    async fn worktree_rpc_uses_the_saved_project_root() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir(&project_root).expect("create project folder");
        let git_status = Command::new(crate::git_inspection::git_program())
            .args(["init", "--quiet"])
            .current_dir(&project_root)
            .status()
            .expect("initialize test repository");
        assert!(git_status.success(), "git init should succeed");

        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT)",
                [],
            )
            .expect("create project table");
        connection
            .execute(
                "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                rusqlite::params!["project-1", project_root.to_string_lossy().into_owned()],
            )
            .expect("save project folder");
        drop(connection);

        let (_sender, mut cancellation) = watch::channel(false);
        let mut params = json!({
            "projectId": "project-1",
            "projectRoot": "C:/",
        });
        let result = dispatch(
            "project.worktrees.list",
            &mut params,
            &json!("list-worktrees"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect("read worktrees from the registered project");

        assert_eq!(result, json!({"worktrees": [], "truncated": false}));
    }

    /// A renderer-supplied confirmation flag cannot replace the service approval broker.
    #[tokio::test]
    async fn named_project_task_requires_broker_approval_even_when_rpc_claims_confirmation() {
        let (_directory, storage, worktree_id, worktree_path) = create_task_test_fixture().await;
        let broker = ToolPermissionBroker::default();
        let responding_broker = broker.clone();
        let (events, mut event_receiver) = EventSink::test_channel();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let request_id = json!("run-task-request");
        let mut params = json!({
            "projectId": "project-1",
            "worktreeId": worktree_id,
            "taskId": "verify",
            "expectedCommand": "echo verify > must-not-run.txt",
            "expectedTimeoutSeconds": 30
        });
        let running = tokio::spawn(async move {
            dispatch(
                "project.worktrees.run_task",
                &mut params,
                &request_id,
                &storage,
                &broker,
                &events,
                &mut cancellation,
            )
            .await
        });

        let approval = timeout(std::time::Duration::from_secs(5), event_receiver.recv())
            .await
            .expect("the service requests approval")
            .expect("approval event is delivered");
        assert_eq!(
            approval.event.as_deref(),
            Some("chat.tool.permission.requested")
        );
        assert_eq!(
            approval.data.as_ref().and_then(|data| data.get("toolName")),
            Some(&json!("run_project_task"))
        );
        assert_eq!(
            approval
                .data
                .as_ref()
                .and_then(|data| data.get("arguments"))
                .and_then(|arguments| arguments.get("command")),
            Some(&json!("echo verify > must-not-run.txt"))
        );
        assert!(
            !running.is_finished(),
            "renderer confirmation alone must not start the task"
        );

        let approval_id = approval
            .data
            .as_ref()
            .and_then(|data| data.get("approvalRequestId"))
            .and_then(serde_json::Value::as_str)
            .expect("approval id");
        responding_broker
            .respond(approval_id, false)
            .await
            .expect("the service receives the user's denial");
        let error = running
            .await
            .expect("the dispatch task completes")
            .expect_err("a denied task is not executed");
        assert_eq!(error.code, "permission_denied");
        assert!(
            !worktree_path.join("must-not-run.txt").exists(),
            "the command must not run after the approval is denied"
        );
    }

    /// A task changed while approval is pending is rejected before command execution.
    #[tokio::test]
    async fn named_project_task_is_revalidated_after_approval_wait() {
        let (_directory, storage, worktree_id, worktree_path) = create_task_test_fixture().await;
        let broker = ToolPermissionBroker::default();
        let responding_broker = broker.clone();
        let (events, mut event_receiver) = EventSink::test_channel();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let request_id = json!("run-changed-task-request");
        let mut params = json!({
            "projectId": "project-1",
            "worktreeId": worktree_id,
            "taskId": "verify",
            "expectedCommand": "echo verify > must-not-run.txt",
            "expectedTimeoutSeconds": 30
        });
        let running = tokio::spawn(async move {
            dispatch(
                "project.worktrees.run_task",
                &mut params,
                &request_id,
                &storage,
                &broker,
                &events,
                &mut cancellation,
            )
            .await
        });

        let approval = timeout(std::time::Duration::from_secs(5), event_receiver.recv())
            .await
            .expect("the service requests approval")
            .expect("approval event is delivered");
        fs::write(
            worktree_path.join(".openchat/tasks.json"),
            r#"{"version":1,"tasks":[{"id":"verify","command":"echo changed > must-not-run.txt","timeoutSeconds":30}]}"#,
        )
        .expect("change task while approval is pending");
        let approval_id = approval
            .data
            .as_ref()
            .and_then(|data| data.get("approvalRequestId"))
            .and_then(serde_json::Value::as_str)
            .expect("approval id");
        responding_broker
            .respond(approval_id, true)
            .await
            .expect("the service receives approval");

        let error = running
            .await
            .expect("the dispatch task completes")
            .expect_err("changed task definitions are not executed");
        assert_eq!(error.code, "project_task_changed");
        assert!(!worktree_path.join("must-not-run.txt").exists());
    }

    #[tokio::test]
    async fn project_task_deny_rule_is_loaded_from_service_storage() {
        let (_directory, storage, worktree_id, _worktree_path) = create_task_test_fixture().await;
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "INSERT INTO project_tool_permission_rules (project_id, rules_json)
                 VALUES (?1, ?2)",
                rusqlite::params!["project-1", r#"{"run_project_task":"deny"}"#],
            )
            .expect("persist project deny rule");
        drop(connection);
        let (_sender, mut cancellation) = watch::channel(false);
        let mut params = json!({
            "projectId": "project-1",
            "worktreeId": worktree_id,
            "taskId": "verify",
            "expectedCommand": "cargo test",
            "expectedTimeoutSeconds": 30
        });

        let error = dispatch(
            "project.worktrees.run_task",
            &mut params,
            &json!("denied-task"),
            &storage,
            &ToolPermissionBroker::default(),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect_err("saved project denial blocks named task execution");

        assert_eq!(error.code, "permission_denied");
    }

    struct TestDirectory(PathBuf);

    /// Creates a registered repository and managed worktree containing one named task.
    async fn create_task_test_fixture() -> (TestDirectory, Arc<AppStorage>, String, PathBuf) {
        let directory = TestDirectory::new();
        let project_root = initialize_task_repository(&directory.0);
        let storage =
            Arc::new(AppStorage::open_at(directory.0.join("data")).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                CREATE TABLE messages (
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                );",
            )
            .expect("create host chat schema fixture");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize current permission schema");
        save_project_record(&storage, &project_root);
        let project_root_text = project_root.to_string_lossy();
        let worktree = git_worktrees::create(&storage, "project-1", &project_root_text)
            .await
            .expect("create an isolated project worktree");
        let worktree_id = worktree["id"].as_str().expect("worktree id").to_owned();
        let worktree_path = PathBuf::from(worktree["path"].as_str().expect("worktree path"));
        (directory, storage, worktree_id, worktree_path)
    }

    /// Initializes a local Git repository with a fixed project task catalog.
    fn initialize_task_repository(root: &std::path::Path) -> PathBuf {
        let repository = root.join("repo");
        fs::create_dir_all(repository.join(".openchat")).expect("create project task directory");
        fs::write(repository.join("README.md"), "test repository\n").expect("write project file");
        fs::write(
            repository.join(".openchat/tasks.json"),
            r#"{"version":1,"tasks":[{"id":"verify","command":"echo verify > must-not-run.txt","timeoutSeconds":30}]}"#,
        )
        .expect("write project task catalog");
        let git_status = Command::new(crate::git_inspection::git_program())
            .args(["init", "--quiet"])
            .current_dir(&repository)
            .status()
            .expect("initialize the task test repository");
        assert!(git_status.success(), "git init should succeed");
        let git_status = Command::new(crate::git_inspection::git_program())
            .args(["add", "README.md", ".openchat/tasks.json"])
            .current_dir(&repository)
            .status()
            .expect("stage task test files");
        assert!(git_status.success(), "git add should succeed");
        let git_status = Command::new(crate::git_inspection::git_program())
            .args([
                "-c",
                "user.name=OpenChat Test",
                "-c",
                "user.email=test@openchat.invalid",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ])
            .current_dir(&repository)
            .status()
            .expect("commit task test files");
        assert!(git_status.success(), "git commit should succeed");
        repository
    }

    /// Stores the repository path used by the project RPC resolver.
    fn save_project_record(storage: &AppStorage, project_root: &std::path::Path) {
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT)",
                [],
            )
            .expect("create project table");
        connection
            .execute(
                "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                rusqlite::params!["project-1", project_root.to_string_lossy().into_owned()],
            )
            .expect("save project folder");
    }

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-project-rpc-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
