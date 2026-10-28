//! Persistent records and enforced workspace scopes for deep security scans.

use std::{collections::HashSet, path::PathBuf};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{invalid_request_params, projects::project_workspace_roots_for_project};

const MAX_ROOTS: usize = 8;
const MAX_ROOT_PATH_BYTES: usize = 32 * 1024;
const MAX_MODEL_ID_CHARS: usize = 512;
const MAX_RETRY_DELAY_MS: i64 = 25 * 60 * 60 * 1000;

#[derive(Clone, Debug)]
pub(super) struct SecurityScanScope {
    pub(super) project_id: String,
    pub(super) roots: Vec<PathBuf>,
    pub(super) status: String,
    pub(super) public_web_research: bool,
}

/// Dispatches durable security-scan record operations.
pub(super) fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "security.scans.create" => create_scan(params, storage),
        "security.scans.list" => list_scans(storage),
        "security.scans.cancel" => cancel_scan(params, storage),
        "security.scans.retry.schedule" => schedule_retry(params, storage),
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested security-scan operation is not available.",
            false,
        )),
    }
}

/// Returns the immutable project and canonical roots associated with a scan conversation.
pub(super) fn scope_for_conversation(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Option<SecurityScanScope>, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let scan = connection
        .query_row(
            "SELECT project_id, selected_roots_json, status, public_web_research FROM security_scans
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| storage_error())?;
    let Some((project_id, roots_json, status, public_web_research)) = scan else {
        return Ok(None);
    };
    drop(connection);
    let roots = parse_and_validate_roots(storage, &project_id, &roots_json)?;
    Ok(Some(SecurityScanScope {
        project_id,
        roots,
        status,
        public_web_research,
    }))
}

/// Records the durable run and this provider attempt before provider work starts.
pub(super) fn mark_running(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
    attempt_id: &str,
) -> Result<bool, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let changed = connection
        .execute(
            "UPDATE security_scans
             SET status = 'running', active_run_id = ?1, active_attempt_id = ?2,
                 next_retry_at_unix_ms = NULL, updated_at_unix_ms = ?3,
                 finished_at_unix_ms = NULL, failure_code = NULL
             WHERE conversation_id = ?4
               AND status NOT IN ('completed', 'cancelled', 'failed')",
            params![run_id, attempt_id, now_unix_ms()?, conversation_id],
        )
        .map_err(|_| storage_error())?;
    Ok(changed == 1)
}

/// Persists the scan state produced by the durable goal-run finalizer.
pub(super) fn finish_run(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
    attempt_id: &str,
    status: &str,
    failure_code: Option<&str>,
) -> Result<(), ServiceError> {
    if !matches!(
        status,
        "waiting_provider" | "paused" | "interrupted" | "completed" | "cancelled" | "failed"
    ) {
        return Err(invalid_request_params());
    }
    let terminal = matches!(status, "completed" | "cancelled" | "failed");
    let connection = storage.connect().map_err(|_| storage_error())?;
    connection
        .execute(
            "UPDATE security_scans
             SET status = ?1, active_run_id = ?2, active_attempt_id = NULL,
                 updated_at_unix_ms = ?3,
                 finished_at_unix_ms = CASE WHEN ?4 THEN ?3 ELSE NULL END,
                 failure_code = ?5, next_retry_at_unix_ms = NULL
             WHERE conversation_id = ?6
               AND active_run_id = ?7 AND active_attempt_id = ?8
               AND status NOT IN ('completed', 'cancelled', 'failed')",
            params![
                status,
                run_id,
                now_unix_ms()?,
                terminal,
                failure_code,
                conversation_id,
                run_id,
                attempt_id
            ],
        )
        .map_err(|_| storage_error())?;
    Ok(())
}

fn create_scan(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let scan_id = required_id(input, "scanId")?;
    let conversation_id = required_id(input, "conversationId")?;
    let project_id = required_id(input, "projectId")?;
    let provider_id = required_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let connection_id = optional_id(input, "connectionId")?;
    let workspace_id = optional_id(input, "workspaceId")?;
    let api_key_connection_id = optional_id(input, "apiKeyConnectionId")?;
    let public_web_research = optional_bool(input, "publicWebResearch", false)?;
    let selected_roots = input
        .get("selectedRoots")
        .and_then(Value::as_array)
        .filter(|roots| !roots.is_empty() && roots.len() <= MAX_ROOTS)
        .ok_or_else(invalid_request_params)?;
    let registered_roots = project_workspace_roots_for_project(storage, project_id)?;
    let mut roots = Vec::with_capacity(selected_roots.len());
    let mut seen = HashSet::with_capacity(selected_roots.len());
    for value in selected_roots {
        let value = value
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= MAX_ROOT_PATH_BYTES)
            .ok_or_else(invalid_request_params)?;
        let canonical = std::fs::canonicalize(value).map_err(|_| invalid_root_scope())?;
        if !registered_roots.iter().any(|root| root == &canonical)
            || !seen.insert(canonical.clone())
        {
            return Err(invalid_root_scope());
        }
        roots.push(canonical);
    }
    let roots_for_response = roots
        .iter()
        .map(|root| root.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let roots_json =
        serde_json::to_string(&roots_for_response).map_err(|_| invalid_request_params())?;
    let now = now_unix_ms()?;
    let mut connection = storage.connect().map_err(|_| storage_error())?;
    let transaction = connection.transaction().map_err(|_| storage_error())?;
    let project_name = transaction
        .query_row(
            "SELECT name FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| storage_error())?
        .ok_or_else(|| not_found("project"))?;
    let title = format!("Security scan: {project_name}");
    transaction
        .execute(
            "INSERT INTO conversations (
                id, title, title_source, provider_id, model_id, connection_id,
                workspace_id, api_key_connection_id, project_id, created_at, updated_at
             ) VALUES (?1, ?2, 'automatic', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                conversation_id,
                title,
                provider_id,
                model_id,
                connection_id,
                workspace_id,
                api_key_connection_id,
                project_id,
                now
            ],
        )
        .map_err(|_| duplicate_scan())?;
    transaction
        .execute(
            "INSERT INTO security_scans (
                id, conversation_id, project_id, selected_roots_json, status, public_web_research,
                created_at_unix_ms, updated_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?6)",
            params![
                scan_id,
                conversation_id,
                project_id,
                roots_json,
                public_web_research,
                now
            ],
        )
        .map_err(|_| duplicate_scan())?;
    transaction.commit().map_err(|_| storage_error())?;
    Ok(json!({
        "scan": {
            "id": scan_id,
            "conversationId": conversation_id,
            "projectId": project_id,
            "projectName": project_name,
            "providerId": provider_id,
            "modelId": model_id,
            "selectedRoots": roots_for_response,
            "publicWebResearch": public_web_research,
            "status": "queued",
            "activeRunId": null,
            "retryCount": 0,
            "nextRetryAtUnixMs": null,
            "createdAtUnixMs": now,
            "updatedAtUnixMs": now,
            "finishedAtUnixMs": null,
            "failureCode": null
        }
    }))
}

fn list_scans(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let mut statement = connection
        .prepare(
            "SELECT scans.id, scans.conversation_id, scans.project_id, projects.name,
                    conversations.provider_id, conversations.model_id,
                    scans.selected_roots_json, scans.status, scans.active_run_id,
                    scans.retry_count, scans.next_retry_at_unix_ms,
                    scans.created_at_unix_ms, scans.updated_at_unix_ms,
                    scans.finished_at_unix_ms, scans.failure_code,
                    scans.public_web_research
             FROM security_scans AS scans
             INNER JOIN conversations ON conversations.id = scans.conversation_id
             INNER JOIN projects ON projects.id = scans.project_id
             ORDER BY scans.updated_at_unix_ms DESC, scans.id DESC",
        )
        .map_err(|_| storage_error())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, bool>(15)?,
            ))
        })
        .map_err(|_| storage_error())?;
    let records = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| storage_error())?;
    drop(statement);
    drop(connection);
    let mut scans = Vec::new();
    for row in records {
        let (
            id,
            conversation_id,
            project_id,
            project_name,
            provider_id,
            model_id,
            roots_json,
            status,
            active_run_id,
            retry_count,
            next_retry_at,
            created_at,
            updated_at,
            finished_at,
            failure_code,
            public_web_research,
        ) = row;
        let roots = parse_roots_for_display(&roots_json)?;
        scans.push(json!({
            "id": id,
            "conversationId": conversation_id,
            "projectId": project_id,
            "projectName": project_name,
            "providerId": provider_id,
            "modelId": model_id,
            "selectedRoots": roots,
            "publicWebResearch": public_web_research,
            "status": status,
            "activeRunId": active_run_id,
            "retryCount": retry_count,
            "nextRetryAtUnixMs": next_retry_at,
            "createdAtUnixMs": created_at,
            "updatedAtUnixMs": updated_at,
            "finishedAtUnixMs": finished_at,
            "failureCode": failure_code
        }));
    }
    Ok(json!({"scans": scans}))
}

fn schedule_retry(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let scan_id = required_id(input, "scanId")?;
    let retry_count = input
        .get("retryCount")
        .and_then(Value::as_i64)
        .filter(|count| (1..=100).contains(count))
        .ok_or_else(invalid_request_params)?;
    let retry_at = input
        .get("nextRetryAtUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(invalid_request_params)?;
    let now = now_unix_ms()?;
    if retry_at < now || retry_at > now.saturating_add(MAX_RETRY_DELAY_MS) {
        return Err(invalid_request_params());
    }
    let connection = storage.connect().map_err(|_| storage_error())?;
    let changed = connection
        .execute(
            "UPDATE security_scans
             SET retry_count = ?1, next_retry_at_unix_ms = ?2, updated_at_unix_ms = ?3
             WHERE id = ?4 AND status = 'waiting_provider'",
            params![retry_count, retry_at, now, scan_id],
        )
        .map_err(|_| storage_error())?;
    if changed != 1 {
        return Err(not_found("waiting security scan"));
    }
    Ok(json!({"saved": true}))
}

fn cancel_scan(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let scan_id = required_id(input, "scanId")?;
    let now = now_unix_ms()?;
    let connection = storage.connect().map_err(|_| storage_error())?;
    let changed = connection
        .execute(
            "UPDATE security_scans
             SET status = 'cancelled', updated_at_unix_ms = ?1,
                 active_attempt_id = NULL, finished_at_unix_ms = ?1,
                 next_retry_at_unix_ms = NULL, failure_code = NULL
             WHERE id = ?2 AND status NOT IN ('completed', 'cancelled', 'failed')",
            params![now, scan_id],
        )
        .map_err(|_| storage_error())?;
    if changed == 1 {
        return Ok(json!({"cancelled": true}));
    }
    let status = connection
        .query_row(
            "SELECT status FROM security_scans WHERE id = ?1",
            [scan_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| storage_error())?
        .ok_or_else(|| not_found("security scan"))?;
    if status == "cancelled" {
        return Ok(json!({"cancelled": true}));
    }
    Err(ServiceError::new(
        "security_scan_not_active",
        "The security scan has already finished.",
        false,
    ))
}

fn parse_and_validate_roots(
    storage: &AppStorage,
    project_id: &str,
    roots_json: &str,
) -> Result<Vec<PathBuf>, ServiceError> {
    let values: Vec<String> = serde_json::from_str(roots_json).map_err(|_| storage_error())?;
    if values.is_empty() || values.len() > MAX_ROOTS {
        return Err(invalid_root_scope());
    }
    let registered = project_workspace_roots_for_project(storage, project_id)?;
    let mut roots = Vec::with_capacity(values.len());
    let mut seen = HashSet::with_capacity(values.len());
    for value in values {
        if value.len() > MAX_ROOT_PATH_BYTES {
            return Err(invalid_root_scope());
        }
        let canonical = std::fs::canonicalize(value).map_err(|_| invalid_root_scope())?;
        if !registered.iter().any(|root| root == &canonical) || !seen.insert(canonical.clone()) {
            return Err(invalid_root_scope());
        }
        roots.push(canonical);
    }
    Ok(roots)
}

fn parse_roots_for_display(roots_json: &str) -> Result<Vec<String>, ServiceError> {
    let roots: Vec<String> = serde_json::from_str(roots_json).map_err(|_| storage_error())?;
    if roots.is_empty()
        || roots.len() > MAX_ROOTS
        || roots
            .iter()
            .any(|root| root.is_empty() || root.len() > MAX_ROOT_PATH_BYTES)
    {
        return Err(storage_error());
    }
    Ok(roots)
}

fn input_object(params: &Value) -> Result<&serde_json::Map<String, Value>, ServiceError> {
    params
        .get("input")
        .unwrap_or(params)
        .as_object()
        .ok_or_else(invalid_request_params)
}

fn required_id<'a>(
    input: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = input
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .ok_or_else(invalid_request_params)?;
    Ok(value)
}

fn optional_id<'a>(
    input: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    match input.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(_)) => required_id(input, name).map(Some),
        _ => Err(invalid_request_params()),
    }
}

fn optional_bool(
    input: &serde_json::Map<String, Value>,
    name: &str,
    default: bool,
) -> Result<bool, ServiceError> {
    match input.get(name) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(invalid_request_params()),
    }
}

fn required_model_id<'a>(
    input: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = input
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.trim().is_empty()
                && value.chars().count() <= MAX_MODEL_ID_CHARS
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(invalid_request_params)?;
    Ok(value.trim())
}

fn invalid_root_scope() -> ServiceError {
    ServiceError::new(
        "security_scan_scope_invalid",
        "The scan folders must be registered project folders that still exist.",
        false,
    )
}

fn duplicate_scan() -> ServiceError {
    ServiceError::new(
        "security_scan_already_exists",
        "The security scan could not be created because its identifier is already in use.",
        false,
    )
}

fn not_found(resource: &str) -> ServiceError {
    ServiceError::new(
        "security_scan_not_found",
        format!("The requested {resource} was not found."),
        false,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "security_scan_storage_unavailable",
        "Security scan state could not be read or saved.",
        true,
    )
}

fn now_unix_ms() -> Result<i64, ServiceError> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| storage_error())?
        .as_millis();
    i64::try_from(millis).map_err(|_| storage_error())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rusqlite::params;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::super::storage_chat;
    use super::{dispatch, finish_run, mark_running, now_unix_ms, scope_for_conversation};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("openchat-security-scan-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create fixture root");
            Self(root)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove fixture root");
        }
    }

    fn insert_test_project(
        storage: &AppStorage,
        project_id: &str,
        root: &std::path::Path,
    ) -> String {
        let root = fs::canonicalize(root)
            .expect("canonicalize project root")
            .to_string_lossy()
            .into_owned();
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![project_id, "Project", root],
            )
            .expect("insert project");
        connection
            .execute(
                "INSERT INTO project_workspace_roots
                    (root_id, project_id, root_path, position, created_at)
                 VALUES (?1, ?2, ?3, 0, 1)",
                params![format!("{project_id}-root"), project_id, root],
            )
            .expect("insert project workspace root");
        root
    }

    #[tokio::test]
    async fn scan_creation_persists_only_registered_roots_and_hides_its_conversation() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir_all(&project_root).expect("create project root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let project_root_text = insert_test_project(&storage, "project-1", &project_root);

        let created = dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-1",
                    "conversationId":"scan-conversation-1",
                    "projectId":"project-1",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[project_root_text]
                }
            }),
            &storage,
        )
        .expect("create scan");
        assert_eq!(
            created.pointer("/scan/status").and_then(Value::as_str),
            Some("queued")
        );
        assert_eq!(
            created.pointer("/scan/publicWebResearch"),
            Some(&json!(false))
        );

        let scope = scope_for_conversation(&storage, "scan-conversation-1")
            .expect("load scan scope")
            .expect("scan scope exists");
        assert!(!scope.public_web_research);
        assert_eq!(
            scope.roots,
            vec![fs::canonicalize(project_root).expect("canonical project root")]
        );
        dispatch(
            "security.scans.cancel",
            &json!({"scanId":"scan-1"}),
            &storage,
        )
        .expect("cancel queued scan");
        dispatch(
            "security.scans.cancel",
            &json!({"scanId":"scan-1"}),
            &storage,
        )
        .expect("cancel is idempotent");
        let scans = dispatch("security.scans.list", &json!({}), &storage)
            .expect("list scans after cancellation");
        assert_eq!(scans["scans"][0]["status"], "cancelled");
        assert_eq!(scans["scans"][0]["publicWebResearch"], false);
        assert!(scans["scans"][0]["finishedAtUnixMs"].as_i64().is_some());

        let conversations =
            storage_chat::dispatch("storage.chat.conversations.list", &json!({}), &storage)
                .await
                .expect("list visible conversations");
        assert_eq!(
            conversations["conversations"].as_array().map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn scan_creation_persists_explicit_public_web_research_consent() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir_all(&project_root).expect("create project root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let project_root = insert_test_project(&storage, "project-1", &project_root);

        dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-web",
                    "conversationId":"scan-web-conversation",
                    "projectId":"project-1",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[project_root],
                    "publicWebResearch":true
                }
            }),
            &storage,
        )
        .expect("create opted-in scan");

        let scope = scope_for_conversation(&storage, "scan-web-conversation")
            .expect("load scan scope")
            .expect("scan scope exists");
        assert!(scope.public_web_research);
        let scans = dispatch("security.scans.list", &json!({}), &storage).expect("list scans");
        assert_eq!(scans["scans"][0]["publicWebResearch"], true);
    }

    #[test]
    fn claimed_scan_attempt_can_be_failed_before_provider_work_starts() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir_all(&project_root).expect("create project root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let project_root = insert_test_project(&storage, "project-early-error", &project_root);
        dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-early-error",
                    "conversationId":"scan-early-error-conversation",
                    "projectId":"project-early-error",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[project_root]
                }
            }),
            &storage,
        )
        .expect("create scan");

        let attempt_id = "00000000000000000000000000000005";
        assert!(
            mark_running(
                &storage,
                "scan-early-error-conversation",
                "run-early-error",
                attempt_id,
            )
            .expect("claim scan before resolving provider credentials")
        );
        finish_run(
            &storage,
            "scan-early-error-conversation",
            "run-early-error",
            attempt_id,
            "failed",
            Some("missing_credentials"),
        )
        .expect("persist a pre-provider failure");

        let scan = dispatch("security.scans.list", &json!({}), &storage).expect("list failed scan");
        assert_eq!(scan["scans"][0]["status"], "failed");
        assert_eq!(scan["scans"][0]["failureCode"], "missing_credentials");
        assert_eq!(scan["scans"][0]["activeRunId"], "run-early-error");
    }

    #[test]
    fn stale_attempt_finalizer_cannot_overwrite_current_scan() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir_all(&project_root).expect("create project root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let project_root = insert_test_project(&storage, "project-stale-run", &project_root);
        dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-stale-run",
                    "conversationId":"scan-stale-run-conversation",
                    "projectId":"project-stale-run",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[project_root]
                }
            }),
            &storage,
        )
        .expect("create scan");

        assert!(
            mark_running(
                &storage,
                "scan-stale-run-conversation",
                "run-old",
                "00000000000000000000000000000001",
            )
            .expect("mark first run running")
        );
        assert!(
            mark_running(
                &storage,
                "scan-stale-run-conversation",
                "run-current",
                "00000000000000000000000000000002",
            )
            .expect("mark current run running")
        );
        finish_run(
            &storage,
            "scan-stale-run-conversation",
            "run-old",
            "00000000000000000000000000000001",
            "completed",
            None,
        )
        .expect("ignore stale finalizer");

        let connection = storage.connect().expect("connect to scan storage");
        let (status, active_run_id, active_attempt_id) = connection
            .query_row(
                "SELECT status, active_run_id, active_attempt_id FROM security_scans
                 WHERE conversation_id = ?1",
                ["scan-stale-run-conversation"],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("read scan state");
        assert_eq!(status, "running");
        assert_eq!(active_run_id.as_deref(), Some("run-current"));
        assert_eq!(
            active_attempt_id.as_deref(),
            Some("00000000000000000000000000000002")
        );
        drop(connection);

        assert!(
            mark_running(
                &storage,
                "scan-stale-run-conversation",
                "run-current",
                "00000000000000000000000000000003"
            )
            .expect("claim resumed attempt with the same durable run ID")
        );
        finish_run(
            &storage,
            "scan-stale-run-conversation",
            "run-current",
            "00000000000000000000000000000002",
            "failed",
            Some("stale_attempt"),
        )
        .expect("ignore stale finalizer for a resumed run");
        let connection = storage
            .connect()
            .expect("connect after stale resume finalizer");
        let (status, active_run_id, active_attempt_id) = connection
            .query_row(
                "SELECT status, active_run_id, active_attempt_id FROM security_scans
                 WHERE conversation_id = ?1",
                ["scan-stale-run-conversation"],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("read scan state after stale resume finalizer");
        assert_eq!(status, "running");
        assert_eq!(active_run_id.as_deref(), Some("run-current"));
        assert_eq!(
            active_attempt_id.as_deref(),
            Some("00000000000000000000000000000003")
        );
        drop(connection);

        finish_run(
            &storage,
            "scan-stale-run-conversation",
            "run-current",
            "00000000000000000000000000000003",
            "completed",
            None,
        )
        .expect("finish current run");
        let connection = storage.connect().expect("connect after current finalizer");
        let (status, active_run_id, active_attempt_id) = connection
            .query_row(
                "SELECT status, active_run_id, active_attempt_id FROM security_scans
                 WHERE conversation_id = ?1",
                ["scan-stale-run-conversation"],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("read completed scan state");
        assert_eq!(status, "completed");
        assert_eq!(active_run_id.as_deref(), Some("run-current"));
        assert_eq!(active_attempt_id, None);
    }

    #[tokio::test]
    async fn scan_creation_rejects_a_folder_outside_the_project_scope() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        let outside_root = directory.0.join("outside");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::create_dir_all(&outside_root).expect("create outside root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        insert_test_project(&storage, "project-1", &project_root);

        let error = dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-1",
                    "conversationId":"scan-conversation-1",
                    "projectId":"project-1",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[outside_root.display().to_string()]
                }
            }),
            &storage,
        )
        .expect_err("outside folders must be rejected");
        assert_eq!(error.code, "security_scan_scope_invalid");
    }

    #[test]
    fn quota_pause_and_retry_schedule_persist_until_the_scan_is_cancelled() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir_all(&project_root).expect("create project root");
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let project_root = insert_test_project(&storage, "project-1", &project_root);

        dispatch(
            "security.scans.create",
            &json!({
                "input": {
                    "scanId":"scan-quota",
                    "conversationId":"scan-quota-conversation",
                    "projectId":"project-1",
                    "providerId":"openrouter",
                    "modelId":"security-model",
                    "selectedRoots":[project_root]
                }
            }),
            &storage,
        )
        .expect("create scan");
        assert!(
            mark_running(
                &storage,
                "scan-quota-conversation",
                "run-quota-1",
                "00000000000000000000000000000004",
            )
            .expect("mark scan running")
        );
        finish_run(
            &storage,
            "scan-quota-conversation",
            "run-quota-1",
            "00000000000000000000000000000004",
            "waiting_provider",
            Some("provider_quota"),
        )
        .expect("persist provider quota pause");

        let retry_at = now_unix_ms()
            .expect("read current time")
            .saturating_add(2 * 60 * 60 * 1000);
        dispatch(
            "security.scans.retry.schedule",
            &json!({
                "input": {
                    "scanId":"scan-quota",
                    "retryCount":1,
                    "nextRetryAtUnixMs":retry_at
                }
            }),
            &storage,
        )
        .expect("persist provider retry schedule");

        let waiting =
            dispatch("security.scans.list", &json!({}), &storage).expect("list waiting scan");
        assert_eq!(waiting["scans"][0]["status"], "waiting_provider");
        assert_eq!(waiting["scans"][0]["retryCount"], 1);
        assert_eq!(waiting["scans"][0]["nextRetryAtUnixMs"], retry_at);

        let too_late = now_unix_ms()
            .expect("read current time")
            .saturating_add(26 * 60 * 60 * 1000);
        let error = dispatch(
            "security.scans.retry.schedule",
            &json!({
                "input": {
                    "scanId":"scan-quota",
                    "retryCount":2,
                    "nextRetryAtUnixMs":too_late
                }
            }),
            &storage,
        )
        .expect_err("retry schedules must remain time-bounded");
        assert_eq!(error.code, "invalid_request_params");

        dispatch(
            "security.scans.cancel",
            &json!({"scanId":"scan-quota"}),
            &storage,
        )
        .expect("cancel waiting scan");
        let cancelled =
            dispatch("security.scans.list", &json!({}), &storage).expect("list cancelled scan");
        assert_eq!(cancelled["scans"][0]["status"], "cancelled");
        assert_eq!(cancelled["scans"][0]["nextRetryAtUnixMs"], Value::Null);
        assert!(cancelled["scans"][0]["finishedAtUnixMs"].as_i64().is_some());
        assert!(
            !mark_running(
                &storage,
                "scan-quota-conversation",
                "run-quota-2",
                "00000000000000000000000000000006",
            )
            .expect("terminal scan must not resume")
        );
    }
}
