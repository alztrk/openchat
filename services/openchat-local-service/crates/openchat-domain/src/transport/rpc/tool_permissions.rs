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
            let native_confirmed = native_confirmation(params)?;
            let initial_mode = optional_string(params, "initialMode")?
                .map(parse_mode)
                .transpose()?;
            seed_mode_if_missing(&connection, initial_mode, native_confirmed)?;
            let mode = load_mode(&connection)?;
            Ok(json!({"mode": mode_as_str(mode)}))
        }
        "tools.permissions.mode.set" => {
            let mode = parse_mode(required_string(params, "mode")?)?;
            let native_confirmed = native_confirmation(params)?;
            save_mode(&connection, mode, native_confirmed)?;
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
    native_confirmed: bool,
) -> Result<ToolPermissionRules, ServiceError> {
    validate_project_id(project_id)?;
    let connection = storage.connect().map_err(|_| settings_unavailable())?;
    require_project(&connection, project_id)?;
    seed_project_rules_if_missing(&connection, project_id, initial_rules, native_confirmed)?;
    load_project_rules(&connection, project_id)
}

/// Stores an explicit user update to a project's tool policy.
pub(super) fn save_project_rules(
    storage: &AppStorage,
    project_id: &str,
    value: &Value,
    native_confirmed: bool,
) -> Result<ToolPermissionRules, ServiceError> {
    let connection = storage.connect().map_err(|_| settings_unavailable())?;
    save_project_rules_in_connection(&connection, project_id, value, native_confirmed)?;
    load_project_rules(&connection, project_id)
}

/// Validates and atomically stores one complete project policy on an existing database connection.
fn save_project_rules_in_connection(
    connection: &StorageConnection<'_>,
    project_id: &str,
    value: &Value,
    native_confirmed: bool,
) -> Result<(), ServiceError> {
    validate_project_id(project_id)?;
    let rules = tools::parse_tool_permission_rules(Some(value))?;
    reject_unbound_mcp_allows(&rules)?;
    let has_allow = rules
        .values()
        .any(|rule| *rule == ToolPermissionRule::Allow);
    if has_allow && !native_confirmed {
        return Err(native_confirmation_required());
    }
    let encoded_rules = encode_rules(value)?;
    require_project(connection, project_id)?;
    connection
        .execute(
            "INSERT INTO project_tool_permission_rules (
                project_id, rules_json, native_confirmed
             ) VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                rules_json = excluded.rules_json,
                native_confirmed = excluded.native_confirmed",
            rusqlite::params![project_id, encoded_rules, has_allow && native_confirmed],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Loads the effective global policy; an unset preference requires approval.
pub(super) fn load_mode(
    connection: &StorageConnection<'_>,
) -> Result<ToolPermissionMode, ServiceError> {
    let saved_mode = connection
        .query_row(
            "SELECT mode, native_confirmed FROM tool_permission_mode WHERE preference_id = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()
        .map_err(|_| settings_unavailable())?;
    match saved_mode {
        Some((value, native_confirmed)) => {
            let mode = parse_mode(&value)?;
            if mode == ToolPermissionMode::FullAccess && !native_confirmed {
                Ok(ToolPermissionMode::RequireApproval)
            } else {
                Ok(mode)
            }
        }
        None => Ok(ToolPermissionMode::RequireApproval),
    }
}

/// Loads the saved project policy; an unset preference inherits global policy behavior.
pub(super) fn load_project_rules(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<ToolPermissionRules, ServiceError> {
    let saved = connection
        .query_row(
            "SELECT rules_json, native_confirmed
             FROM project_tool_permission_rules WHERE project_id = ?1",
            [project_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()
        .map_err(|_| settings_unavailable())?;
    let Some((encoded, native_confirmed)) = saved else {
        return Ok(ToolPermissionRules::new());
    };
    let mut value: Value = serde_json::from_str(&encoded).map_err(|_| settings_unavailable())?;
    if let Some(rules) = value.as_object_mut() {
        rules.retain(|name, rule| {
            if rule.as_str() != Some("allow") {
                return true;
            }
            if !native_confirmed {
                return false;
            }
            !tools::mcp::is_mcp_permission_rule_name(name)
                || tools::mcp::is_bound_permission_rule_name(name)
        });
    }
    tools::parse_tool_permission_rules(Some(&value)).map_err(|_| settings_unavailable())
}

fn reject_unbound_mcp_allows(rules: &ToolPermissionRules) -> Result<(), ServiceError> {
    if rules.iter().any(|(name, rule)| {
        tools::mcp::is_mcp_permission_rule_name(name)
            && *rule == ToolPermissionRule::Allow
            && !tools::mcp::is_bound_permission_rule_name(name)
    }) {
        return Err(ServiceError::new(
            "invalid_tool_permission_rules",
            "The project tool permission rules are invalid.",
            false,
        ));
    }
    Ok(())
}

/// Persists the selected global policy.
pub(super) fn save_mode(
    connection: &StorageConnection<'_>,
    mode: ToolPermissionMode,
    native_confirmed: bool,
) -> Result<(), ServiceError> {
    if mode == ToolPermissionMode::FullAccess && !native_confirmed {
        return Err(native_confirmation_required());
    }
    connection
        .execute(
            "INSERT INTO tool_permission_mode (preference_id, mode, native_confirmed)
             VALUES (1, ?1, ?2)
             ON CONFLICT(preference_id) DO UPDATE SET
                mode = excluded.mode,
                native_confirmed = excluded.native_confirmed",
            rusqlite::params![mode_as_str(mode), mode == ToolPermissionMode::FullAccess],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Imports an existing renderer preference without replacing a service-owned value.
fn seed_mode_if_missing(
    connection: &StorageConnection<'_>,
    initial_mode: Option<ToolPermissionMode>,
    native_confirmed: bool,
) -> Result<(), ServiceError> {
    let Some(initial_mode) = initial_mode else {
        return Ok(());
    };
    if initial_mode == ToolPermissionMode::FullAccess && !native_confirmed {
        let exists = connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM tool_permission_mode WHERE preference_id = 1
                )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| settings_unavailable())?;
        if !exists {
            return Err(native_confirmation_required());
        }
        return Ok(());
    }
    connection
        .execute(
            "INSERT INTO tool_permission_mode (preference_id, mode, native_confirmed)
             VALUES (1, ?1, ?2)
             ON CONFLICT(preference_id) DO NOTHING",
            rusqlite::params![
                mode_as_str(initial_mode),
                initial_mode == ToolPermissionMode::FullAccess && native_confirmed
            ],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Imports existing project rules without replacing a service-owned policy.
fn seed_project_rules_if_missing(
    connection: &StorageConnection<'_>,
    project_id: &str,
    initial_rules: Option<&Value>,
    native_confirmed: bool,
) -> Result<(), ServiceError> {
    let Some(initial_rules) = initial_rules else {
        return Ok(());
    };
    let parsed_rules = tools::parse_tool_permission_rules(Some(initial_rules))?;
    reject_unbound_mcp_allows(&parsed_rules)?;
    let has_allow = parsed_rules
        .values()
        .any(|rule| *rule == ToolPermissionRule::Allow);
    if has_allow && !native_confirmed {
        let exists = connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM project_tool_permission_rules WHERE project_id = ?1
                )",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| settings_unavailable())?;
        if !exists {
            return Err(native_confirmation_required());
        }
        return Ok(());
    }
    let encoded_rules = encode_rules(initial_rules)?;
    connection
        .execute(
            "INSERT INTO project_tool_permission_rules (
                project_id, rules_json, native_confirmed
             ) VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO NOTHING",
            rusqlite::params![project_id, encoded_rules, has_allow && native_confirmed],
        )
        .map_err(|_| settings_unavailable())?;
    Ok(())
}

/// Handles permission-provenance requests sent only by the native host.
pub(super) fn dispatch_internal(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "internal.tool_permissions.global_status" => {
            require_internal_params(params, &[])?;
            let connection = storage.connect().map_err(|_| settings_unavailable())?;
            let saved = connection
                .query_row(
                    "SELECT mode, native_confirmed FROM tool_permission_mode
                     WHERE preference_id = 1",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
                )
                .optional()
                .map_err(|_| settings_unavailable())?;
            let has_saved_policy = saved.is_some();
            let needs_confirmation = saved
                .as_ref()
                .is_some_and(|(mode, confirmed)| mode == "full_access" && !confirmed);
            Ok(json!({
                "hasSavedPolicy": has_saved_policy,
                "needsConfirmation": needs_confirmation,
            }))
        }
        "internal.tool_permissions.global_confirm" => {
            require_internal_params(params, &[])?;
            let connection = storage.connect().map_err(|_| settings_unavailable())?;
            let updated = connection
                .execute(
                    "UPDATE tool_permission_mode SET native_confirmed = 1
                     WHERE preference_id = 1 AND mode = 'full_access'
                       AND native_confirmed = 0",
                    [],
                )
                .map_err(|_| settings_unavailable())?;
            Ok(json!({"confirmed": updated == 1}))
        }
        "internal.tool_permissions.project_status" => {
            require_internal_params(params, &["projectId"])?;
            let project_id = required_string(params, "projectId")?;
            validate_project_id(project_id)?;
            let connection = storage.connect().map_err(|_| settings_unavailable())?;
            require_project(&connection, project_id)?;
            let saved = connection
                .query_row(
                    "SELECT rules_json, native_confirmed
                     FROM project_tool_permission_rules WHERE project_id = ?1",
                    [project_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
                )
                .optional()
                .map_err(|_| settings_unavailable())?;
            let Some((rules_json, native_confirmed)) = saved else {
                return Ok(json!({
                    "hasSavedPolicy": false,
                    "needsConfirmation": false,
                }));
            };
            let rules: Value =
                serde_json::from_str(&rules_json).map_err(|_| settings_unavailable())?;
            let parsed = tools::parse_tool_permission_rules(Some(&rules))
                .map_err(|_| settings_unavailable())?;
            let needs_confirmation = parsed
                .values()
                .any(|rule| *rule == ToolPermissionRule::Allow)
                && !native_confirmed;
            Ok(json!({
                "hasSavedPolicy": true,
                "needsConfirmation": needs_confirmation,
                "rulesJson": rules_json,
            }))
        }
        "internal.tool_permissions.project_confirm" => {
            require_internal_params(params, &["projectId", "rulesJson"])?;
            let project_id = required_string(params, "projectId")?;
            validate_project_id(project_id)?;
            let expected_rules_json = required_string(params, "rulesJson")?;
            if expected_rules_json.len() > 131_072 {
                return Err(invalid_request_params());
            }
            let rules: Value =
                serde_json::from_str(expected_rules_json).map_err(|_| invalid_request_params())?;
            let parsed = tools::parse_tool_permission_rules(Some(&rules))?;
            if !parsed
                .values()
                .any(|rule| *rule == ToolPermissionRule::Allow)
            {
                return Err(invalid_request_params());
            }
            let connection = storage.connect().map_err(|_| settings_unavailable())?;
            let updated = connection
                .execute(
                    "UPDATE project_tool_permission_rules SET native_confirmed = 1
                     WHERE project_id = ?1 AND rules_json = ?2 AND native_confirmed = 0",
                    rusqlite::params![project_id, expected_rules_json],
                )
                .map_err(|_| settings_unavailable())?;
            Ok(json!({"confirmed": updated == 1}))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

pub(super) fn native_confirmation(params: &Value) -> Result<bool, ServiceError> {
    match params.get("nativeConfirmation") {
        None => Ok(false),
        Some(Value::Bool(confirmed)) => Ok(*confirmed),
        Some(_) => Err(invalid_request_params()),
    }
}

fn require_internal_params(params: &Value, expected: &[&str]) -> Result<(), ServiceError> {
    let Some(params) = params.as_object() else {
        return Err(invalid_request_params());
    };
    if params.len() != expected.len() || expected.iter().any(|key| !params.contains_key(*key)) {
        return Err(invalid_request_params());
    }
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

fn native_confirmation_required() -> ServiceError {
    ServiceError::new(
        "native_permission_confirmation_required",
        "The requested tool permission requires confirmation by the desktop host.",
        false,
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
                    ),
                    native_confirmed INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE project_tool_permission_rules (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    rules_json TEXT NOT NULL,
                    native_confirmed INTEGER NOT NULL DEFAULT 0
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

        save_mode(&connection, ToolPermissionMode::FullAccess, true)
            .expect("save confirmed full-access mode");

        assert_eq!(
            load_mode(&connection).expect("read saved mode"),
            ToolPermissionMode::FullAccess
        );
    }

    #[test]
    fn legacy_full_access_is_ineffective_until_native_confirmation() {
        let connection = settings_connection();
        connection
            .execute(
                "INSERT INTO tool_permission_mode (preference_id, mode)
                 VALUES (1, 'full_access')",
                [],
            )
            .expect("insert legacy full-access mode");

        assert_eq!(
            load_mode(&connection).expect("read effective legacy mode"),
            ToolPermissionMode::RequireApproval
        );
        assert_eq!(
            save_mode(&connection, ToolPermissionMode::FullAccess, false)
                .expect_err("unconfirmed full access must be rejected")
                .code,
            "native_permission_confirmation_required"
        );

        connection
            .execute(
                "UPDATE tool_permission_mode SET native_confirmed = 1
                 WHERE preference_id = 1",
                [],
            )
            .expect("record host confirmation");
        assert_eq!(
            load_mode(&connection).expect("read confirmed mode"),
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
        seed_mode_if_missing(&connection, Some(ToolPermissionMode::FullAccess), true)
            .expect("import the initial preference");
        save_mode(&connection, ToolPermissionMode::RequireApproval, false)
            .expect("update the preference");
        seed_mode_if_missing(&connection, Some(ToolPermissionMode::FullAccess), true)
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
        save_project_rules_in_connection(&connection, "project-1", &rules, false)
            .expect("save project policy");
        let loaded = load_project_rules(&connection, "project-1").expect("load project rules");
        assert_eq!(
            loaded.get("run_project_task"),
            Some(&ToolPermissionRule::Deny)
        );
        assert_eq!(loaded.get("read_file"), Some(&ToolPermissionRule::Ask));
    }

    #[test]
    fn legacy_project_allow_rules_are_hidden_until_native_confirmation() {
        let connection = settings_connection();
        connection
            .execute(
                "INSERT INTO project_tool_permission_rules (project_id, rules_json)
                 VALUES ('project-1', '{\"read_file\":\"allow\"}')",
                [],
            )
            .expect("insert legacy project allow rule");

        assert!(
            load_project_rules(&connection, "project-1")
                .expect("read effective legacy project rules")
                .is_empty()
        );
        connection
            .execute(
                "UPDATE project_tool_permission_rules SET native_confirmed = 1
                 WHERE project_id = 'project-1'",
                [],
            )
            .expect("record host confirmation");
        assert_eq!(
            load_project_rules(&connection, "project-1")
                .expect("read confirmed project rules")
                .get("read_file"),
            Some(&ToolPermissionRule::Allow)
        );
    }

    #[cfg(windows)]
    #[test]
    fn project_mcp_allows_require_server_and_tool_definition_bindings() {
        const SERVER_FINGERPRINT: &str = "0123456789abcdef0123456789abcdef";
        const TOOL_FINGERPRINT: &str = "abcdef0123456789abcdef0123456789";
        let connection = settings_connection();
        let unbound_server = json!({"mcp__local_docs__*": "allow"});
        let unbound_tool = json!({"mcp__local_docs__read-file": "allow"});
        for rules in [&unbound_server, &unbound_tool] {
            assert_eq!(
                save_project_rules_in_connection(&connection, "project-1", rules, true)
                    .expect_err("unbound MCP allows must not be persisted")
                    .code,
                "invalid_tool_permission_rules"
            );
        }

        let bound_server =
            crate::tools::mcp::server_permission_rule_name("local_docs", SERVER_FINGERPRINT);
        let bound_tool = crate::tools::mcp::tool_permission_rule_name(
            "local_docs",
            "read-file",
            TOOL_FINGERPRINT,
        );
        let rules = serde_json::Map::from_iter([
            (bound_server.clone(), json!("allow")),
            (bound_tool.clone(), json!("allow")),
        ]);
        save_project_rules_in_connection(
            &connection,
            "project-1",
            &serde_json::Value::Object(rules),
            true,
        )
        .expect("host-confirmed definition-bound allows must be saved");
        let saved = load_project_rules(&connection, "project-1").expect("load bound MCP allows");
        assert_eq!(saved.get(&bound_server), Some(&ToolPermissionRule::Allow));
        assert_eq!(saved.get(&bound_tool), Some(&ToolPermissionRule::Allow));
    }

    #[cfg(windows)]
    #[test]
    fn legacy_unbound_mcp_allows_are_ignored_but_denies_are_preserved() {
        let connection = settings_connection();
        connection
            .execute(
                "INSERT INTO project_tool_permission_rules (project_id, rules_json, native_confirmed)
                 VALUES ('project-1', '{\"mcp__local_docs__*\":\"allow\",\"mcp__local_docs__read-file\":\"allow\",\"mcp__remote_docs__*\":\"deny\"}', 1)",
                [],
            )
            .expect("insert legacy project MCP rules");

        let loaded = load_project_rules(&connection, "project-1")
            .expect("read legacy MCP policy after binding enforcement");
        assert!(!loaded.contains_key("mcp__local_docs__*"));
        assert!(!loaded.contains_key("mcp__local_docs__read-file"));
        assert_eq!(
            loaded.get("mcp__remote_docs__*"),
            Some(&ToolPermissionRule::Deny)
        );
    }

    #[test]
    fn project_allow_rule_writes_require_native_confirmation() {
        let connection = settings_connection();
        let rules = json!({"read_file": "allow"});
        assert_eq!(
            save_project_rules_in_connection(&connection, "project-1", &rules, false)
                .expect_err("unconfirmed allow rules must be rejected")
                .code,
            "native_permission_confirmation_required"
        );

        save_project_rules_in_connection(&connection, "project-1", &rules, true)
            .expect("save host-confirmed allow rule");
        assert_eq!(
            load_project_rules(&connection, "project-1")
                .expect("read confirmed project rules")
                .get("read_file"),
            Some(&ToolPermissionRule::Allow)
        );
    }

    #[test]
    fn one_time_import_does_not_overwrite_saved_project_rules() {
        let connection = settings_connection();
        seed_project_rules_if_missing(
            &connection,
            "project-1",
            Some(&json!({"run_project_task": "deny"})),
            false,
        )
        .expect("import initial project rules");
        seed_project_rules_if_missing(
            &connection,
            "project-1",
            Some(&json!({"run_project_task": "allow"})),
            false,
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
            let result = save_project_rules_in_connection(&connection, "project-1", &rules, false);
            assert_eq!(
                result.expect_err("invalid rules must be rejected").code,
                "invalid_tool_permission_rules"
            );
        }
    }
}
