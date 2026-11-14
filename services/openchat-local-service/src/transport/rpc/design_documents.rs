//! RPC boundary for encrypted Design document storage and revision history.

use serde_json::{Value, json};

use crate::{domain::design_documents, protocol::ServiceError, storage::AppStorage};

pub(super) fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "design.styles.list" => {
            let limit = optional_u64(params, "limit", 2_000)?;
            let items = design_documents::list_shared_text_styles(storage, limit)?;
            Ok(json!({ "items": items }))
        }
        "design.styles.save" => {
            let id = required_string(params, "id")?;
            let name = required_string(params, "name")?;
            let color = required_string(params, "color")?;
            let font_size = required_number(params, "fontSize")?;
            let font_weight = required_i64(params, "fontWeight")?;
            let expected_revision = optional_i64(params, "expectedRevision")?;
            let revision = design_documents::save_shared_text_style(
                storage,
                id,
                name,
                color,
                font_size,
                font_weight,
                expected_revision,
            )?;
            Ok(json!({ "saved": true, "revision": revision }))
        }
        "design.styles.delete" => {
            let id = required_string(params, "id")?;
            let expected_revision = required_i64(params, "expectedRevision")?;
            let deleted =
                design_documents::delete_shared_text_style(storage, id, expected_revision)?;
            Ok(json!({ "deleted": deleted }))
        }
        "design.documents.list" => {
            let project_id = optional_string(params, "projectId")?;
            let limit = optional_u64(params, "limit", 50)?;
            let include_archived = optional_bool(params, "includeArchived", false)?;
            let items =
                design_documents::list(storage, project_id.as_deref(), include_archived, limit)?;
            Ok(json!({ "items": items }))
        }
        "design.documents.get" => {
            let id = required_string(params, "id")?;
            let document = design_documents::get(storage, id)?;
            Ok(json!({ "document": document }))
        }
        "design.documents.revisions.list" => {
            let id = required_string(params, "id")?;
            let limit = optional_u64(params, "limit", 30)?;
            let items = design_documents::revisions(storage, id, limit)?;
            Ok(json!({ "items": items }))
        }
        "design.documents.save" => {
            let id = required_string(params, "id")?;
            let name = required_string(params, "name")?;
            let project_id = optional_string(params, "projectId")?;
            let document_json = required_string(params, "documentJson")?;
            let expected_revision = optional_i64(params, "expectedRevision")?;
            let revision = design_documents::save(
                storage,
                id,
                name,
                project_id.as_deref(),
                document_json,
                expected_revision,
            )?;
            Ok(json!({ "saved": true, "revision": revision }))
        }
        "design.documents.archive" => {
            let id = required_string(params, "id")?;
            let archived = design_documents::archive(storage, id)?;
            Ok(json!({ "archived": archived }))
        }
        _ => Err(invalid_request()),
    }
}

fn required_string<'a>(params: &'a Value, key: &str) -> Result<&'a str, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)
}

fn optional_string(params: &Value, key: &str) -> Result<Option<String>, ServiceError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(invalid_request()),
    }
}

fn optional_i64(params: &Value, key: &str) -> Result<Option<i64>, ServiceError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value.as_i64().map(Some).ok_or_else(invalid_request),
        _ => Err(invalid_request()),
    }
}

fn required_i64(params: &Value, key: &str) -> Result<i64, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(invalid_request)
}

fn required_number(params: &Value, key: &str) -> Result<f64, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(invalid_request)
}

fn optional_u64(params: &Value, key: &str, default: u64) -> Result<u64, ServiceError> {
    match params.get(key) {
        None => Ok(default),
        Some(value) => value.as_u64().ok_or_else(invalid_request),
    }
}

fn optional_bool(params: &Value, key: &str, default: bool) -> Result<bool, ServiceError> {
    match params.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(invalid_request()),
    }
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The Design document request is invalid.",
        false,
    )
}
