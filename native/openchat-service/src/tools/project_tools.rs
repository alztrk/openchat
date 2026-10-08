use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

const CATALOG_PATH: &str = ".openchat/tools.json";
const MAX_CATALOG_BYTES: usize = 64 * 1024;
const MAX_TOOLS: usize = 32;
const MAX_ID_BYTES: usize = 48;
const MAX_DESCRIPTION_BYTES: usize = 2_000;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_SCHEMA_DEPTH: usize = 8;
const MAX_SCHEMA_PROPERTIES: usize = 64;
const MAX_ARRAY_ITEMS: usize = 128;
const MAX_OUTPUT_BYTES: usize = 128 * 1024;
const TOOL_NAME_PREFIX: &str = "project_tool__";
const PERMISSION_PREFIX: &str = "project_tool__";

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProjectTool {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) executable: String,
    pub(crate) input_schema: Value,
    pub(crate) timeout_seconds: u64,
    pub(crate) output_limit_bytes: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectToolCatalog {
    version: u32,
    tools: Vec<ProjectTool>,
}

impl ProjectTool {
    pub(crate) fn tool_name(&self) -> String {
        format!("{TOOL_NAME_PREFIX}{}", self.id)
    }

    pub(crate) fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.tool_name(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        }
    }

    pub(crate) fn executable_path(&self, project_root: &Path) -> Result<PathBuf, ServiceError> {
        let root = fs::canonicalize(project_root).map_err(|_| unavailable())?;
        let executable = root.join(&self.executable);
        let metadata = fs::symlink_metadata(&executable).map_err(|_| unavailable())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(invalid(
                "The configured tool executable must be a regular project file.",
            ));
        }
        let executable = fs::canonicalize(executable).map_err(|_| unavailable())?;
        if !executable.starts_with(&root) {
            return Err(invalid(
                "The configured tool executable resolves outside the project.",
            ));
        }
        Ok(executable)
    }

    pub(crate) fn validate_arguments(&self, arguments: &Value) -> Result<(), ServiceError> {
        let bytes = serde_json::to_vec(arguments)
            .map_err(|_| invalid("The tool arguments are invalid."))?;
        if bytes.len() > MAX_ARGUMENT_BYTES {
            return Err(invalid(
                "The configured tool arguments exceed the supported size.",
            ));
        }
        validate_value(&self.input_schema, arguments, 0)
            .map_err(|_| invalid("The configured tool arguments do not match its input schema."))
    }
}

pub(crate) fn load_if_present(root: &Path) -> Result<Vec<ProjectTool>, ServiceError> {
    let path = root.join(CATALOG_PATH);
    match fs::symlink_metadata(&path) {
        Ok(_) => load(root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(unavailable()),
    }
}

pub(crate) fn load(root: &Path) -> Result<Vec<ProjectTool>, ServiceError> {
    let path = root.join(CATALOG_PATH);
    let metadata = fs::symlink_metadata(&path).map_err(|_| unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid(
            "The configured project tool catalog must be a regular file.",
        ));
    }
    let canonical_root = fs::canonicalize(root).map_err(|_| unavailable())?;
    let canonical_catalog = fs::canonicalize(&path).map_err(|_| unavailable())?;
    if !canonical_catalog.starts_with(&canonical_root) {
        return Err(invalid(
            "The configured project tool catalog resolves outside the project.",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(canonical_catalog)
        .and_then(|file| {
            file.take((MAX_CATALOG_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| unavailable())?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(invalid(
            "The configured project tool catalog exceeds the 64 KiB limit.",
        ));
    }
    let catalog: ProjectToolCatalog = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("The configured project tool catalog is invalid."))?;
    if catalog.version != 1 || catalog.tools.len() > MAX_TOOLS {
        return Err(invalid(
            "The configured project tool catalog version or size is unsupported.",
        ));
    }
    let mut ids = HashSet::with_capacity(catalog.tools.len());
    for tool in &catalog.tools {
        if !valid_id(&tool.id)
            || !ids.insert(tool.id.as_str())
            || tool.description.trim().is_empty()
            || tool.description.len() > MAX_DESCRIPTION_BYTES
            || !valid_executable_path(&tool.executable)
            || !(5..=600).contains(&tool.timeout_seconds)
            || !(1..=MAX_OUTPUT_BYTES).contains(&tool.output_limit_bytes)
        {
            return Err(invalid(
                "The configured project tool catalog contains an invalid entry.",
            ));
        }
        validate_schema(&tool.input_schema, 0)
            .map_err(|_| invalid("A configured project tool has an unsupported input schema."))?;
    }
    Ok(catalog.tools)
}

pub(crate) fn find<'a>(tools: &'a [ProjectTool], tool_name: &str) -> Option<&'a ProjectTool> {
    let id = tool_name.strip_prefix(TOOL_NAME_PREFIX)?;
    tools.iter().find(|tool| tool.id == id)
}

pub(crate) fn is_permission_rule_name(name: &str) -> bool {
    name.strip_prefix(PERMISSION_PREFIX).is_some_and(valid_id)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_executable_path(path: &str) -> bool {
    let path = Path::new(path);
    path.is_relative()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn validate_schema(schema: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(());
    }
    let object = schema.as_object().ok_or(())?;
    let allowed = [
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
    ];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(());
    }
    match object.get("type").and_then(Value::as_str) {
        Some("object") => {
            if depth != 0 || object.get("additionalProperties") != Some(&Value::Bool(false)) {
                return Err(());
            }
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(())?;
            if properties.len() > MAX_SCHEMA_PROPERTIES {
                return Err(());
            }
            for schema in properties.values() {
                validate_schema(schema, depth + 1)?;
            }
            if let Some(required) = object.get("required") {
                let required = required.as_array().ok_or(())?;
                if required.len() > properties.len()
                    || required
                        .iter()
                        .any(|key| key.as_str().is_none_or(|key| !properties.contains_key(key)))
                {
                    return Err(());
                }
            }
        }
        Some("array") => {
            if object
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "items" | "minItems" | "maxItems"))
            {
                return Err(());
            }
            validate_schema(object.get("items").ok_or(())?, depth + 1)?;
            if bounded_usize(object.get("minItems"), 0, MAX_ARRAY_ITEMS).is_none()
                && object.contains_key("minItems")
                || bounded_usize(object.get("maxItems"), 0, MAX_ARRAY_ITEMS).is_none()
                    && object.contains_key("maxItems")
            {
                return Err(());
            }
        }
        Some("string") => {
            if object
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "enum" | "minLength" | "maxLength"))
            {
                return Err(());
            }
            validate_enum(object.get("enum"), Value::is_string)?;
            if bounded_usize(object.get("minLength"), 0, MAX_ARGUMENT_BYTES).is_none()
                && object.contains_key("minLength")
                || bounded_usize(object.get("maxLength"), 0, MAX_ARGUMENT_BYTES).is_none()
                    && object.contains_key("maxLength")
            {
                return Err(());
            }
        }
        Some("integer") | Some("number") => {
            if object
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "minimum" | "maximum"))
            {
                return Err(());
            }
            for key in ["minimum", "maximum"] {
                if object
                    .get(key)
                    .is_some_and(|value| value.as_f64().is_none())
                {
                    return Err(());
                }
            }
        }
        Some("boolean") => {
            if object.len() != 1 {
                return Err(());
            }
        }
        _ => return Err(()),
    }
    Ok(())
}

fn validate_enum(enum_value: Option<&Value>, predicate: fn(&Value) -> bool) -> Result<(), ()> {
    if let Some(values) = enum_value {
        let values = values.as_array().ok_or(())?;
        if values.is_empty()
            || values.len() > MAX_ARRAY_ITEMS
            || values.iter().any(|v| !predicate(v))
        {
            return Err(());
        }
    }
    Ok(())
}

fn validate_value(schema: &Value, value: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(());
    }
    let schema = schema.as_object().ok_or(())?;
    match schema.get("type").and_then(Value::as_str).ok_or(())? {
        "object" => {
            let value = value.as_object().ok_or(())?;
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(())?;
            let required = schema.get("required").and_then(Value::as_array);
            if value.keys().any(|key| !properties.contains_key(key))
                || required.is_some_and(|keys| {
                    keys.iter()
                        .any(|key| key.as_str().is_none_or(|key| !value.contains_key(key)))
                })
            {
                return Err(());
            }
            for (key, property_value) in value {
                validate_value(properties.get(key).ok_or(())?, property_value, depth + 1)?;
            }
        }
        "array" => {
            let values = value.as_array().ok_or(())?;
            if values.len() > MAX_ARRAY_ITEMS
                || schema
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .is_some_and(|min| values.len() < min as usize)
                || schema
                    .get("maxItems")
                    .and_then(Value::as_u64)
                    .is_some_and(|max| values.len() > max as usize)
            {
                return Err(());
            }
            let items = schema.get("items").ok_or(())?;
            for value in values {
                validate_value(items, value, depth + 1)?;
            }
        }
        "string" => {
            let value = value.as_str().ok_or(())?;
            if schema
                .get("minLength")
                .and_then(Value::as_u64)
                .is_some_and(|min| value.chars().count() < min as usize)
                || schema
                    .get("maxLength")
                    .and_then(Value::as_u64)
                    .is_some_and(|max| value.chars().count() > max as usize)
                || schema
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| !values.iter().any(|item| item.as_str() == Some(value)))
            {
                return Err(());
            }
        }
        "integer" | "number" => {
            let value = value.as_f64().ok_or(())?;
            if (schema.get("type").and_then(Value::as_str) == Some("integer")
                && value.fract() != 0.0)
                || schema
                    .get("minimum")
                    .and_then(Value::as_f64)
                    .is_some_and(|minimum| value < minimum)
                || schema
                    .get("maximum")
                    .and_then(Value::as_f64)
                    .is_some_and(|maximum| value > maximum)
            {
                return Err(());
            }
        }
        "boolean" if !value.is_boolean() => return Err(()),
        "boolean" => {}
        _ => return Err(()),
    }
    Ok(())
}

fn bounded_usize(value: Option<&Value>, minimum: usize, maximum: usize) -> Option<usize> {
    let value = value?.as_u64()?;
    let value = usize::try_from(value).ok()?;
    (minimum..=maximum).contains(&value).then_some(value)
}

fn unavailable() -> ServiceError {
    ServiceError::new(
        "project_tool_catalog_unavailable",
        "The configured project tool catalog could not be read.",
        false,
    )
}

fn invalid(message: &str) -> ServiceError {
    ServiceError::new("invalid_project_tool_catalog", message, false)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;

    use super::{ProjectTool, is_permission_rule_name, load};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-project-tools-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(path.join(".openchat")).expect("create test catalog directory");
            Self(path)
        }

        fn write_catalog(&self, catalog: serde_json::Value) {
            fs::write(self.0.join(".openchat/tools.json"), catalog.to_string())
                .expect("write project tools catalog");
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn valid_tool() -> serde_json::Value {
        json!({
            "id":"lint",
            "description":"Run the project linter.",
            "executable":"tools/lint.exe",
            "inputSchema":{"type":"object","properties":{"fix":{"type":"boolean"}},"required":["fix"],"additionalProperties":false},
            "timeoutSeconds":60,
            "outputLimitBytes":16384
        })
    }

    #[test]
    fn loads_a_project_scoped_tool_definition_and_validates_input() {
        let directory = TestDirectory::new();
        fs::create_dir_all(directory.0.join("tools")).expect("create executable folder");
        fs::write(directory.0.join("tools/lint.exe"), b"test").expect("create executable fixture");
        directory.write_catalog(json!({"version":1,"tools":[valid_tool()]}));

        let tools = load(&directory.0).expect("load a valid configured tool");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_name(), "project_tool__lint");
        assert_eq!(tools[0].definition().parameters["required"], json!(["fix"]));
        tools[0]
            .validate_arguments(&json!({"fix":true}))
            .expect("validate arguments");
        assert!(tools[0].validate_arguments(&json!({"fix":"yes"})).is_err());
        assert!(
            tools[0]
                .validate_arguments(&json!({"fix":true,"other":1}))
                .is_err()
        );
        assert!(tools[0].executable_path(&directory.0).is_ok());
    }

    #[test]
    fn rejects_duplicate_ids_invalid_program_paths_and_unsupported_schema() {
        for tools in [
            json!([valid_tool(), valid_tool()]),
            json!([{"id":"bad","description":"Bad","executable":"..\\outside.exe","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"timeoutSeconds":60,"outputLimitBytes":128}]),
            json!([{"id":"bad","description":"Bad","executable":"tools/run.exe","inputSchema":{"type":"object","properties":{},"additionalProperties":false,"patternProperties":{}},"timeoutSeconds":60,"outputLimitBytes":128}]),
        ] {
            let directory = TestDirectory::new();
            directory.write_catalog(json!({"version":1,"tools":tools}));
            assert!(load(&directory.0).is_err());
        }
    }

    #[test]
    fn configured_tool_permission_rule_ids_are_stable_and_bounded() {
        assert!(is_permission_rule_name("project_tool__lint"));
        assert!(!is_permission_rule_name("project_tool__Upper"));
    }

    #[test]
    fn missing_catalog_is_an_explicit_empty_registry() {
        let directory = TestDirectory::new();
        assert!(
            super::load_if_present(&directory.0)
                .expect("load absent catalog")
                .is_empty()
        );
    }

    #[test]
    fn tool_schema_objects_reject_unknown_properties_and_restrict_array_sizes() {
        let schema = json!({"type":"object","properties":{"names":{"type":"array","items":{"type":"string","maxLength":8},"maxItems":2}},"additionalProperties":false});
        let tool = ProjectTool {
            id: "x".to_owned(),
            description: "x".to_owned(),
            executable: "x.exe".to_owned(),
            input_schema: schema,
            timeout_seconds: 5,
            output_limit_bytes: 1,
        };
        assert!(
            tool.validate_arguments(&json!({"names":["one","two"]}))
                .is_ok()
        );
        assert!(
            tool.validate_arguments(&json!({"names":["one","two","three"]}))
                .is_err()
        );
        assert!(
            tool.validate_arguments(&json!({"names":["too-long-value"]}))
                .is_err()
        );
    }
}
