//! RPC boundary for listing and reading images saved in the Design library.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use crate::{image_library, protocol::ServiceError, storage::AppStorage};

pub(super) fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "design.generations.list" => {
            let limit = optional_u64(params, "limit", 20)?;
            let runs = image_library::list_generation_runs(storage, limit)?;
            Ok(json!({"items": runs}))
        }
        "design.images.list" => {
            let offset = optional_u64(params, "offset", 0)?;
            let limit = optional_u64(params, "limit", 30)?;
            if !(1..=50).contains(&limit) {
                return Err(invalid_request());
            }
            let query = optional_string(params, "query")?.unwrap_or_default();
            let sort = optional_string(params, "sort")?.unwrap_or_else(|| "newest".to_owned());
            let created_after = optional_timestamp(params, "createdAfter")?;
            let created_before = optional_timestamp(params, "createdBefore")?;
            if created_after
                .zip(created_before)
                .is_some_and(|(after, before)| after > before)
            {
                return Err(invalid_request());
            }
            let model_id = optional_string(params, "modelId")?;
            let provider_route = optional_string(params, "providerRoute")?;
            let tag = optional_string(params, "tag")?;
            let collection_id = optional_string(params, "collectionId")?;
            let filters = image_library::ImageLibraryFilters {
                query: &query,
                sort: &sort,
                created_after,
                created_before,
                model_id: model_id.as_deref(),
                provider_route: provider_route.as_deref(),
                favorites_only: optional_bool(params, "favoritesOnly", false)?,
                tag: tag.as_deref(),
                collection_id: collection_id.as_deref(),
            };
            let mut images = image_library::list(storage, offset, limit + 1, &filters)?;
            let has_more = images.len() as u64 > limit;
            images.truncate(limit as usize);
            let next_offset = has_more.then_some(offset + limit);
            Ok(json!({"items": images, "nextOffset": next_offset}))
        }
        "design.images.annotations.set" => {
            let source_kind = required_string(params, "sourceKind")?;
            let source_key = required_string(params, "sourceKey")?;
            let is_favorite = required_bool(params, "isFavorite")?;
            let tags = required_string_array(params, "tags", 20, 48)?;
            image_library::set_annotations(storage, source_kind, source_key, is_favorite, &tags)?;
            Ok(json!({"saved": true}))
        }
        "design.images.remove" => {
            let source_kind = required_string(params, "sourceKind")?;
            let source_key = required_string(params, "sourceKey")?;
            image_library::remove_from_library(storage, source_kind, source_key)?;
            Ok(json!({"removed": true}))
        }
        "design.images.clear" => {
            let removed_count = image_library::clear_library(storage)?;
            Ok(json!({"removedCount": removed_count}))
        }
        "design.collections.list" => {
            let items = image_library::list_collections(storage)?;
            Ok(json!({"items": items}))
        }
        "design.collections.save" => {
            let id = optional_string(params, "id")?;
            let name = required_string(params, "name")?;
            let collection = image_library::save_collection(storage, id.as_deref(), name)?;
            Ok(json!({"collection": collection}))
        }
        "design.collections.delete" => {
            let id = required_string(params, "id")?;
            image_library::delete_collection(storage, id)?;
            Ok(json!({"deleted": true}))
        }
        "design.collections.item.set" => {
            let collection_id = required_string(params, "collectionId")?;
            let source_kind = required_string(params, "sourceKind")?;
            let source_key = required_string(params, "sourceKey")?;
            let included = required_bool(params, "included")?;
            image_library::set_collection_item(
                storage,
                collection_id,
                source_kind,
                source_key,
                included,
            )?;
            Ok(json!({"saved": true}))
        }
        "design.templates.list" => {
            let items = image_library::list_prompt_templates(storage)?;
            Ok(json!({"items": items}))
        }
        "design.templates.save" => {
            let id = optional_string(params, "id")?;
            let name = required_string(params, "name")?;
            let prompt = required_string(params, "prompt")?;
            let template =
                image_library::save_prompt_template(storage, id.as_deref(), name, prompt)?;
            Ok(json!({"template": template}))
        }
        "design.templates.delete" => {
            let id = required_string(params, "id")?;
            image_library::delete_prompt_template(storage, id)?;
            Ok(json!({"deleted": true}))
        }
        "design.images.read" => {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(invalid_request)?;
            let offset = required_u64(params, "offset")?;
            let length = required_u64(params, "length")?;
            let (mime_type, total_bytes, bytes) =
                image_library::read_chunk(storage, id, offset, length)?;
            Ok(json!({
                "mimeType": mime_type,
                "totalBytes": total_bytes,
                "offset": offset,
                "dataBase64": STANDARD.encode(bytes),
            }))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested Design image library method is not available.",
            false,
        )),
    }
}

fn optional_u64(params: &Value, key: &str, default: u64) -> Result<u64, ServiceError> {
    match params.get(key) {
        None => Ok(default),
        Some(value) => value.as_u64().ok_or_else(invalid_request),
    }
}

fn required_u64(params: &Value, key: &str) -> Result<u64, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)
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

fn required_bool(params: &Value, key: &str) -> Result<bool, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request)
}

fn optional_bool(params: &Value, key: &str, default: bool) -> Result<bool, ServiceError> {
    match params.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(invalid_request()),
    }
}

fn optional_timestamp(params: &Value, key: &str) -> Result<Option<i64>, ServiceError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_i64()
            .filter(|value| *value >= 0)
            .map(Some)
            .ok_or_else(invalid_request),
        _ => Err(invalid_request()),
    }
}

fn required_string_array(
    params: &Value,
    key: &str,
    max_count: usize,
    max_chars: usize,
) -> Result<Vec<String>, ServiceError> {
    let values = params
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(invalid_request)?;
    if values.len() > max_count {
        return Err(invalid_request());
    }
    values
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or_else(invalid_request)?;
            if value.trim().is_empty()
                || value.chars().count() > max_chars
                || value.chars().any(char::is_control)
            {
                return Err(invalid_request());
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The Design image library request is invalid.",
        false,
    )
}
