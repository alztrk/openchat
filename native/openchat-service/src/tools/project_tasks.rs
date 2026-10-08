use std::{collections::HashSet, fs, io::Read, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

const TASK_CATALOG_PATH: &str = ".openchat/tasks.json";
const MAX_CATALOG_BYTES: usize = 64 * 1024;
const MAX_TASKS: usize = 32;
const MAX_TASK_ID_BYTES: usize = 64;
const MAX_COMMAND_BYTES: usize = 4096;
const TASK_PERMISSION_PREFIX: &str = "run_project_task__";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProjectTask {
    pub(crate) id: String,
    pub(crate) command: String,
    pub(crate) timeout_seconds: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectTaskCatalog {
    version: u32,
    tasks: Vec<ProjectTask>,
}

pub(crate) fn tool_definition(task_ids: &[String]) -> ToolDefinition {
    ToolDefinition {
        name: "run_project_task".to_owned(),
        description: "Run a named task from the attached project's .openchat/tasks.json file through the configured terminal permission and output limits. When approval is required, the resolved command is shown before it runs.".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "maxLength": MAX_TASK_ID_BYTES,
                    "description": "The id of a task declared in .openchat/tasks.json.",
                    "enum": task_ids
                }
            },
            "required": ["task"],
            "additionalProperties": false
        }),
    }
}

pub(crate) fn load_project_tasks_if_present(root: &Path) -> Result<Vec<ProjectTask>, ServiceError> {
    let config_path = root.join(TASK_CATALOG_PATH);
    match fs::symlink_metadata(&config_path) {
        Ok(_) => load_project_tasks(root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(ServiceError::new(
            "project_task_catalog_unavailable",
            "The project task file `.openchat/tasks.json` could not be read.",
            false,
        )),
    }
}

pub(crate) fn load_project_task_ids_if_enabled(
    root: Option<&Path>,
    enabled: bool,
) -> Result<Vec<String>, ServiceError> {
    if !enabled {
        return Ok(Vec::new());
    }
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    Ok(load_project_tasks_if_present(root)?
        .into_iter()
        .map(|task| task.id)
        .collect())
}

pub(crate) fn load_project_task(
    root: &Path,
    requested_id: &str,
) -> Result<ProjectTask, ServiceError> {
    if !is_valid_task_id(requested_id) {
        return Err(invalid_task("The requested project task id is invalid."));
    }

    load_project_tasks(root)?
        .into_iter()
        .find(|task| task.id == requested_id)
        .ok_or_else(|| {
            ServiceError::new(
                "project_task_not_found",
                "The requested task is not declared in `.openchat/tasks.json`.",
                false,
            )
        })
}

pub(crate) fn load_project_tasks(root: &Path) -> Result<Vec<ProjectTask>, ServiceError> {
    let config_path = root.join(TASK_CATALOG_PATH);
    let metadata = fs::symlink_metadata(&config_path).map_err(|_| {
        ServiceError::new(
            "project_task_catalog_unavailable",
            "The project task file `.openchat/tasks.json` could not be read.",
            false,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_task(
            "The project task file must be a regular file inside the attached project.",
        ));
    }

    let canonical_root = fs::canonicalize(root).map_err(|_| {
        ServiceError::new(
            "project_task_catalog_unavailable",
            "The attached project folder could not be resolved.",
            false,
        )
    })?;
    let canonical_config = fs::canonicalize(&config_path).map_err(|_| {
        ServiceError::new(
            "project_task_catalog_unavailable",
            "The project task file `.openchat/tasks.json` could not be read.",
            false,
        )
    })?;
    if !canonical_config.starts_with(&canonical_root) {
        return Err(invalid_task(
            "The project task file resolves outside the attached project.",
        ));
    }

    let mut bytes = Vec::new();
    fs::File::open(canonical_config)
        .and_then(|file| {
            file.take((MAX_CATALOG_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| {
            ServiceError::new(
                "project_task_catalog_unavailable",
                "The project task file `.openchat/tasks.json` could not be read.",
                false,
            )
        })?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(invalid_task(
            "The project task file exceeds the 64 KiB limit.",
        ));
    }

    let catalog: ProjectTaskCatalog = serde_json::from_slice(&bytes).map_err(|_| {
        invalid_task("The project task file must contain a valid version 1 task catalog.")
    })?;
    if catalog.version != 1 || catalog.tasks.is_empty() || catalog.tasks.len() > MAX_TASKS {
        return Err(invalid_task(
            "The project task catalog version or task count is unsupported.",
        ));
    }

    let mut task_ids = HashSet::with_capacity(catalog.tasks.len());
    for task in &catalog.tasks {
        if !is_valid_task_id(&task.id)
            || !task_ids.insert(task.id.as_str())
            || task.command.trim().is_empty()
            || task.command.len() > MAX_COMMAND_BYTES
            || task
                .command
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            || !(5..=600).contains(&task.timeout_seconds)
        {
            return Err(invalid_task(
                "The project task catalog contains an invalid task id, command, or timeout.",
            ));
        }
    }

    Ok(catalog.tasks)
}

fn is_valid_task_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_TASK_ID_BYTES
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

pub(crate) fn permission_rule_name(task_id: &str) -> Option<String> {
    is_valid_task_id(task_id).then(|| format!("{TASK_PERMISSION_PREFIX}{task_id}"))
}

pub(crate) fn is_permission_rule_name(name: &str) -> bool {
    name.strip_prefix(TASK_PERMISSION_PREFIX)
        .is_some_and(is_valid_task_id)
}

fn invalid_task(message: &str) -> ServiceError {
    ServiceError::new("invalid_project_task_catalog", message, false)
}
