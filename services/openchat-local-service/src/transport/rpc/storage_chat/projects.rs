//! Project and workspace-root RPC handlers.

use super::*;

pub(super) fn list_projects(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, folder_path, created_at, updated_at
             FROM projects ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let name = row.get::<_, String>(1)?;
            let folder_path = row.get::<_, String>(2)?;
            let created_at = row.get::<_, i64>(3)?;
            let updated_at = row.get::<_, i64>(4)?;
            Ok((id, name, folder_path, created_at, updated_at))
        })
        .map_err(storage_error)?;
    let mut projects = Vec::new();
    for row in rows {
        let (id, name, folder_path, created_at, updated_at) = row.map_err(storage_error)?;
        let mut roots = connection
            .prepare(
                "SELECT root_path FROM project_workspace_roots
                 WHERE project_id = ?1 ORDER BY position ASC, root_id ASC",
            )
            .map_err(storage_error)?;
        let roots = roots
            .query_map([&id], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        projects.push(json!({
            "id": id,
            "name": name,
            "folderPath": folder_path,
            "workspaceRoots": roots,
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }
    Ok(json!({"projects": projects}))
}

pub(super) fn create_project(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let name = required_bounded_string(input, "name", MAX_PROJECT_NAME_BYTES, true)?;
    if name.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid_request_params());
    }
    let folder_path = validate_project_folder_path(required_bounded_string(
        input,
        "folderPath",
        MAX_WORKSPACE_ROOT_BYTES,
        true,
    )?)?;
    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, name, folder_path, now],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO project_workspace_roots
                (root_id, project_id, root_path, position, created_at)
             VALUES (lower(hex(randomblob(16))), ?1, ?2, 0, ?3)",
            params![id, folder_path, now],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    drop(connection);
    Ok(json!({"saved": true}))
}

pub(super) fn add_project_workspace_root(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let project_id = required_safe_id(input, "projectId")?;
    let root_path = validate_project_folder_path(required_bounded_string(
        input,
        "folderPath",
        MAX_WORKSPACE_ROOT_BYTES,
        true,
    )?)?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let roots = {
        let mut statement = transaction
            .prepare(
                "SELECT root_path, position FROM project_workspace_roots
                 WHERE project_id = ?1 ORDER BY position ASC",
            )
            .map_err(storage_error)?;
        statement
            .query_map([project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?
    };
    if roots.is_empty()
        || roots.len() >= 8
        || roots
            .iter()
            .any(|(path, _)| paths_overlap(path, &root_path))
    {
        return Err(invalid_request_params());
    }
    let position = roots
        .iter()
        .map(|(_, position)| *position)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(invalid_request_params)?;
    transaction
        .execute(
            "INSERT INTO project_workspace_roots
                (root_id, project_id, root_path, position, created_at)
             VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4)",
            params![project_id, root_path, position, unix_time_millis()?],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true, "rootPath": root_path}))
}

pub(super) fn validate_project_folder_path(value: &str) -> Result<String, ServiceError> {
    let path = Path::new(value);
    if !path.is_absolute() || value.chars().any(char::is_control) {
        return Err(invalid_request_params());
    }
    let canonical_path = fs::canonicalize(path).map_err(|_| invalid_request_params())?;
    if !canonical_path.is_dir() {
        return Err(invalid_request_params());
    }
    let canonical_path = canonical_path
        .to_str()
        .filter(|path| path.len() <= MAX_WORKSPACE_ROOT_BYTES)
        .ok_or_else(invalid_request_params)?;
    Ok(canonical_path.to_owned())
}

pub(super) fn remove_project_workspace_root(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let project_id = required_safe_id(input, "projectId")?;
    let root_path = required_bounded_string(input, "rootPath", MAX_WORKSPACE_ROOT_BYTES, true)?;
    let connection = storage.connect().map_err(storage_error)?;
    let removed = connection
        .execute(
            "DELETE FROM project_workspace_roots
             WHERE project_id = ?1 AND root_path = ?2 AND position > 0",
            params![project_id, root_path],
        )
        .map_err(storage_error)?;
    if removed != 1 {
        return Err(not_found("workspace root"));
    }
    Ok(json!({"deleted": true}))
}
