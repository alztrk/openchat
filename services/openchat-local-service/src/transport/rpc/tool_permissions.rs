//! Persists renderer-editable tool policy while keeping execution decisions service-owned.
//!
//! Values accepted from the renderer are treated as user preference updates or one-time imports;
//! durable and project-task execution always reloads the effective policy from SQLite.

use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
    tools::{self, ToolPermissionMode, ToolPermissionRule, ToolPermissionRules},
};

use super::{invalid_request_params, optional_string, required_string};

/// Routes global tool permission preference requests.
pub(super) fn dispatch_global(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(|_| settings_unavailable())?;
    match method {
        "tools.permissions.mode.get" => {
            let initial_mode = optional_string(params, "initialMode")?
                .map(parse_mode)
                .transpose()?;
            seed_mode_if_missing(&connection, initial_mode)?;
            let mode = load_mode(&connection)?;
            Ok(json!({"mode": mode_as_str(mode)}))
        }
        "tools.permissions.mode.set" => {
            let mode = parse_mode(required_string(params, "mode")?)?;
            save_mode(&connection, mode)?;
            Ok(json!({"mode": mode_as_str(mode)}))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

/// Loads a project policy and imports a renderer cache only when no service value exists.
pub(super) fn get_project_rules(
    storage: &AppStorage,
    project_id: &str,
    initial_rules: Option<&Value>,
) -> Result<ToolPermissionRules, ServiceError> {
    validate_project_id(project_id)?;
    let connection = storage.connect().map_err(|_| settings_unavailable())?;
    require_project(&connection, project_id)?;
    seed_project_rules_if_missing(&connection, project_id, initial_rules)?;
    load_project_rules(&connection, project_id)
}

/// Stores an explicit user update to a project's tool policy.
pub(super) fn save_project_rules(
    storage: &AppStorage,
    project_id: &str,
    value: &Value,
) -> Result<ToolPermissionRules, ServiceError> {
    let connection = storage.connect().map_err(|_| settings_unavailable())?;
    save_project_rules_in_connection(&connection, project_id, value)?;
    load_project_rules(&connection, project_id)
}

/// Validates and atomically stores one complete project policy on an existing database connection.
fn save_project_rules_in_connection(
    connection: &StorageConnection<'_>,
    project_id: &str,
    value: &Value,
) -> Result<(), ServiceError> {
    validate_project_id(project_id)?;
    tools::parse_tool_permission_rules(Some(value))?;
    let encoded_rules = encode_rules(value)?;
    require_project(connection, project_id)?;
    connection
        .execute(
            "INSERT INTO project_tool_permission_rules (project_id, rules_json)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO UPDATE SET rules_json = excluded.rules_json",
            rusqlite::params![project_id, encoded_rules],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Loads the effective global policy; an unset preference requires approval.
pub(super) fn load_mode(
    connection: &StorageConnection<'_>,
) -> Result<ToolPermissionMode, ServiceError> {
    let mode = connection
        .query_row(
            "SELECT mode FROM tool_permission_mode WHERE preference_id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| settings_unavailable())?;
    mode.map(|value| parse_mode(&value))
        .unwrap_or(Ok(ToolPermissionMode::RequireApproval))
}

/// Loads the saved project policy; an unset preference inherits global policy behavior.
pub(super) fn load_project_rules(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<ToolPermissionRules, ServiceError> {
    let encoded = connection
        .query_row(
            "SELECT rules_json FROM project_tool_permission_rules WHERE project_id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| settings_unavailable())?;
    let Some(encoded) = encoded else {
        return Ok(ToolPermissionRules::new());
    };
    let value: Value = serde_json::from_str(&encoded).map_err(|_| settings_unavailable())?;
    tools::parse_tool_permission_rules(Some(&value)).map_err(|_| settings_unavailable())
}

/// Persists the selected global policy.
pub(super) fn save_mode(
    connection: &StorageConnection<'_>,
    mode: ToolPermissionMode,
) -> Result<(), ServiceError> {
    connection
        .execute(
            "INSERT INTO tool_permission_mode (preference_id, mode)
             VALUES (1, ?1)
             ON CONFLICT(preference_id) DO UPDATE SET mode = excluded.mode",
            [mode_as_str(mode)],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Imports an existing renderer preference without replacing a service-owned value.
fn seed_mode_if_missing(
    connection: &StorageConnection<'_>,
    initial_mode: Option<ToolPermissionMode>,
) -> Result<(), ServiceError> {
    let Some(initial_mode) = initial_mode else {
        return Ok(());
    };
    connection
        .execute(
            "INSERT INTO tool_permission_mode (preference_id, mode)
             VALUES (1, ?1)
             ON CONFLICT(preference_id) DO NOTHING",
            [mode_as_str(initial_mode)],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Imports existing project rules without replacing a service-owned policy.
fn seed_project_rules_if_missing(
    connection: &StorageConnection<'_>,
    project_id: &str,
    initial_rules: Option<&Value>,
) -> Result<(), ServiceError> {
    let Some(initial_rules) = initial_rules else {
        return Ok(());
    };
    tools::parse_tool_permission_rules(Some(initial_rules))?;
    let encoded_rules = encode_rules(initial_rules)?;
    connection
        .execute(
            "INSERT INTO project_tool_permission_rules (project_id, rules_json)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO NOTHING",
            rusqlite::params![project_id, encoded_rules],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Converts validated rules to the stable public RPC representation.
pub(super) fn rules_to_value(rules: &ToolPermissionRules) -> Value {
    Value::Object(
        rules
            .iter()
            .map(|(name, rule)| {
                let value = match rule {
                    ToolPermissionRule::Ask => "ask",
                    ToolPermissionRule::Allow => "allow",
                    ToolPermissionRule::Deny => "deny",
                };
                (name.clone(), Value::String(value.to_owned()))
            })
            .collect::<Map<_, _>>(),
    )
}

/// Parses only persistable global modes; plan mode is a per-request restriction.
fn parse_mode(value: &str) -> Result<ToolPermissionMode, ServiceError> {
    match value {
        "require_approval" => Ok(ToolPermissionMode::RequireApproval),
        "approve_safe_operations" => Ok(ToolPermissionMode::ApproveSafeOperations),
        "full_access" => Ok(ToolPermissionMode::FullAccess),
        _ => Err(invalid_request_params()),
    }
}

/// Converts a validated policy mode to the stable SQLite and RPC representation.
fn mode_as_str(mode: ToolPermissionMode) -> &'static str {
    match mode {
        ToolPermissionMode::RequireApproval => "require_approval",
        ToolPermissionMode::ApproveSafeOperations => "approve_safe_operations",
        ToolPermissionMode::FullAccess => "full_access",
        ToolPermissionMode::Plan => "plan",
    }
}

/// Serializes an already validated rule object within the database field limit.
fn encode_rules(value: &Value) -> Result<String, ServiceError> {
    let encoded = serde_json::to_string(value).map_err(|_| invalid_request_params())?;
    if encoded.len() > 131_072 {
        return Err(invalid_request_params());
    }
    Ok(encoded)
}

/// Enforces the project ID syntax used by frontend project preference keys.
fn validate_project_id(project_id: &str) -> Result<(), ServiceError> {
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(invalid_request_params());
    }
    Ok(())
}

/// Confirms that a policy update belongs to a currently registered project.
fn require_project(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<(), ServiceError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| settings_unavailable())?;
    if !exists {
        return Err(ServiceError::new(
            "project_unavailable",
            "The selected project is no longer available.",
            false,
        ));
    }
    Ok(())
}

/// Creates the stable, non-sensitive error used for permission preference storage failures.
fn settings_unavailable() -> ServiceError {
    ServiceError::new(
        "tool_permission_settings_unavailable",
        "Tool permission settings could not be accessed safely.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::json;

    use super::{
        load_mode, load_project_rules, parse_mode, save_mode, save_project_rules_in_connection,
        seed_mode_if_missing, seed_project_rules_if_missing,
    };
    use crate::storage::StorageConnection;
    use crate::tools::{ToolPermissionMode, ToolPermissionRule};

    fn settings_connection() -> StorageConnection<'static> {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE tool_permission_mode (
                    preference_id INTEGER PRIMARY KEY CHECK (preference_id = 1),
                    mode TEXT NOT NULL CHECK (
                        mode IN ('require_approval', 'approve_safe_operations', 'full_access')
                    )
                );
                CREATE TABLE project_tool_permission_rules (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    rules_json TEXT NOT NULL
                );
                CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);",
            )
            .expect("create permission settings fixture");
        connection
            .execute("INSERT INTO projects (id) VALUES ('project-1')", [])
            .expect("insert project fixture");
        StorageConnection::standalone(connection)
    }

    #[test]
    fn unset_global_mode_requires_approval_and_saved_mode_is_read_back() {
        let connection = settings_connection();
        assert_eq!(
            load_mode(&connection).expect("read default mode"),
            ToolPermissionMode::RequireApproval
        );

        save_mode(&connection, ToolPermissionMode::FullAccess).expect("save selected mode");

        assert_eq!(
            load_mode(&connection).expect("read saved mode"),
            ToolPermissionMode::FullAccess
        );
    }

    #[test]
    fn parses_only_persistable_permission_modes() {
        assert_eq!(
            parse_mode("require_approval").expect("parse approval mode"),
            ToolPermissionMode::RequireApproval
        );
        assert_eq!(
            parse_mode("approve_safe_operations").expect("parse safe-operation mode"),
            ToolPermissionMode::ApproveSafeOperations
        );
        assert_eq!(
            parse_mode("full_access").expect("parse full-access mode"),
            ToolPermissionMode::FullAccess
        );
        assert_eq!(
            parse_mode("plan")
                .expect_err("plan is a request restriction, not a saved policy")
                .code,
            "invalid_request_params"
        );
    }

    #[test]
    fn one_time_import_does_not_overwrite_a_saved_global_mode() {
        let connection = settings_connection();
        seed_mode_if_missing(&connection, Some(ToolPermissionMode::FullAccess))
            .expect("import the initial preference");
        save_mode(&connection, ToolPermissionMode::RequireApproval).expect("update the preference");
        seed_mode_if_missing(&connection, Some(ToolPermissionMode::FullAccess))
            .expect("repeat import without replacing the saved preference");

        assert_eq!(
            load_mode(&connection).expect("read the saved mode"),
            ToolPermissionMode::RequireApproval
        );
    }

    #[test]
    fn project_rules_are_validated_and_saved_as_service_owned_policy() {
        let connection = settings_connection();
        let rules = json!({"run_project_task": "deny", "read_file": "ask"});
        save_project_rules_in_connection(&connection, "project-1", &rules)
            .expect("save project policy");
        let loaded = load_project_rules(&connection, "project-1").expect("load project rules");
        assert_eq!(
            loaded.get("run_project_task"),
            Some(&ToolPermissionRule::Deny)
        );
        assert_eq!(loaded.get("read_file"), Some(&ToolPermissionRule::Ask));
    }

    #[test]
    fn one_time_import_does_not_overwrite_saved_project_rules() {
        let connection = settings_connection();
        seed_project_rules_if_missing(
            &connection,
            "project-1",
            Some(&json!({"run_project_task": "deny"})),
        )
        .expect("import initial project rules");
        seed_project_rules_if_missing(
            &connection,
            "project-1",
            Some(&json!({"run_project_task": "allow"})),
        )
        .expect("repeat import without replacing saved project rules");

        let loaded =
            load_project_rules(&connection, "project-1").expect("read saved project rules");
        assert_eq!(
            loaded.get("run_project_task"),
            Some(&ToolPermissionRule::Deny)
        );
    }

    #[test]
    fn save_project_rules_rejects_invalid_rule_names_and_values() {
        for rules in [
            json!({"unknown_tool": "allow"}),
            json!({"read_file": "allow-all"}),
        ] {
            let connection = settings_connection();
            let result = save_project_rules_in_connection(&connection, "project-1", &rules);
            assert_eq!(
                result.expect_err("invalid rules must be rejected").code,
                "invalid_tool_permission_rules"
            );
        }
    }
}
