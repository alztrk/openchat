//! Durable Design documents and bounded immutable document revisions.

use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageRow},
};

const MAXIMUM_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_DOCUMENT_NODES: usize = 10_000;
const MAXIMUM_DOCUMENT_COMMENTS: usize = 10_000;
const MAXIMUM_DESIGN_MOTION_TRACKS: usize = 5_000;
const MAXIMUM_PROTOTYPE_FLOWS: usize = 100;
const MAXIMUM_PROTOTYPE_INTERACTIONS: usize = 10_000;
const MAXIMUM_DESIGN_COMPONENTS: usize = 2_000;
const MAXIMUM_REVISIONS_PER_DOCUMENT: i64 = 100;
const MAXIMUM_SHARED_TEXT_STYLES: u64 = 2_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesignDocumentSummary {
    id: String,
    name: String,
    project_id: Option<String>,
    revision: i64,
    created_at_unix_ms: i64,
    updated_at_unix_ms: i64,
    archived_at_unix_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesignDocumentRecord {
    #[serde(flatten)]
    summary: DesignDocumentSummary,
    document_json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesignDocumentRevision {
    revision: i64,
    document_json: String,
    created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedDesignTextStyle {
    id: String,
    name: String,
    color: String,
    font_size: f64,
    font_weight: i64,
    revision: i64,
    updated_at_unix_ms: i64,
}

pub(crate) fn list_shared_text_styles(
    storage: &AppStorage,
    limit: u64,
) -> Result<Vec<SharedDesignTextStyle>, ServiceError> {
    if !(1..=MAXIMUM_SHARED_TEXT_STYLES).contains(&limit) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_failure)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, color, font_size, font_weight, revision, updated_at_unix_ms
             FROM design_text_styles ORDER BY name COLLATE NOCASE, id LIMIT ?1",
        )
        .map_err(storage_failure)?;
    statement
        .query_map([limit as i64], read_shared_text_style)
        .map_err(storage_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_failure)
}

pub(crate) fn save_shared_text_style(
    storage: &AppStorage,
    id: &str,
    name: &str,
    color: &str,
    font_size: f64,
    font_weight: i64,
    expected_revision: Option<i64>,
) -> Result<i64, ServiceError> {
    validate_id(id)?;
    let normalized_name = name.trim();
    if normalized_name.is_empty()
        || normalized_name.chars().count() > 128
        || normalized_name.chars().any(char::is_control)
        || !valid_color(Some(&Value::String(color.to_owned())))
        || !font_size.is_finite()
        || !(1.0..=4096.0).contains(&font_size)
        || !(100..=900).contains(&font_weight)
        || font_weight % 100 != 0
        || expected_revision.is_some_and(|revision| revision <= 0)
    {
        return Err(invalid_request());
    }
    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    let current = transaction
        .query_row(
            "SELECT revision FROM design_text_styles WHERE id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?;
    let revision = match (current, expected_revision) {
        (None, None) => {
            transaction
                .execute(
                    "INSERT INTO design_text_styles
                        (id, name, color, font_size, font_weight, revision, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                    params![
                        id,
                        normalized_name,
                        color.to_ascii_uppercase(),
                        font_size,
                        font_weight,
                        now
                    ],
                )
                .map_err(storage_failure)?;
            1
        }
        (Some(current), Some(expected)) if current == expected => {
            let revision = current.checked_add(1).ok_or_else(invalid_request)?;
            transaction
                .execute(
                    "UPDATE design_text_styles SET name = ?2, color = ?3, font_size = ?4,
                        font_weight = ?5, revision = ?6, updated_at_unix_ms = ?7
                     WHERE id = ?1 AND revision = ?8",
                    params![
                        id,
                        normalized_name,
                        color.to_ascii_uppercase(),
                        font_size,
                        font_weight,
                        revision,
                        now,
                        expected
                    ],
                )
                .map_err(storage_failure)?;
            revision
        }
        _ => return Err(shared_style_conflict()),
    };
    transaction.commit().map_err(storage_failure)?;
    Ok(revision)
}

pub(crate) fn delete_shared_text_style(
    storage: &AppStorage,
    id: &str,
    expected_revision: i64,
) -> Result<bool, ServiceError> {
    validate_id(id)?;
    if expected_revision <= 0 {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_failure)?;
    let current = connection
        .query_row(
            "SELECT revision FROM design_text_styles WHERE id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?;
    let Some(current) = current else {
        return Ok(false);
    };
    if current != expected_revision {
        return Err(shared_style_conflict());
    }
    let deleted = connection
        .execute(
            "DELETE FROM design_text_styles WHERE id = ?1 AND revision = ?2",
            params![id, expected_revision],
        )
        .map_err(storage_failure)?;
    if deleted == 0 {
        return Err(shared_style_conflict());
    }
    Ok(true)
}

pub(crate) fn list(
    storage: &AppStorage,
    project_id: Option<&str>,
    include_archived: bool,
    limit: u64,
) -> Result<Vec<DesignDocumentSummary>, ServiceError> {
    if !(1..=100).contains(&limit) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_failure)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, project_id, revision, created_at_unix_ms,
                    updated_at_unix_ms, archived_at_unix_ms
             FROM design_documents
             WHERE (?1 IS NULL OR project_id = ?1)
               AND (?2 = 1 OR archived_at_unix_ms IS NULL)
             ORDER BY updated_at_unix_ms DESC, id
             LIMIT ?3",
        )
        .map_err(storage_failure)?;
    statement
        .query_map(
            params![project_id, include_archived, limit as i64],
            read_summary,
        )
        .map_err(storage_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_failure)
}

pub(crate) fn get(
    storage: &AppStorage,
    id: &str,
) -> Result<Option<DesignDocumentRecord>, ServiceError> {
    validate_id(id)?;
    let connection = storage.connect().map_err(storage_failure)?;
    connection
        .query_row(
            "SELECT id, name, project_id, revision, created_at_unix_ms,
                    updated_at_unix_ms, archived_at_unix_ms, document_json
             FROM design_documents WHERE id = ?1 AND archived_at_unix_ms IS NULL",
            [id],
            |row| {
                Ok(DesignDocumentRecord {
                    summary: read_summary(row)?,
                    document_json: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(storage_failure)
}

pub(crate) fn revisions(
    storage: &AppStorage,
    id: &str,
    limit: u64,
) -> Result<Vec<DesignDocumentRevision>, ServiceError> {
    validate_id(id)?;
    if !(1..=100).contains(&limit) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_failure)?;
    let mut statement = connection
        .prepare(
            "SELECT revision, document_json, created_at_unix_ms
             FROM design_document_revisions
             WHERE document_id = ?1
             ORDER BY revision DESC LIMIT ?2",
        )
        .map_err(storage_failure)?;
    statement
        .query_map(params![id, limit as i64], |row| {
            Ok(DesignDocumentRevision {
                revision: row.get(0)?,
                document_json: row.get(1)?,
                created_at_unix_ms: row.get(2)?,
            })
        })
        .map_err(storage_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_failure)
}

pub(crate) fn save(
    storage: &AppStorage,
    id: &str,
    name: &str,
    project_id: Option<&str>,
    document_json: &str,
    expected_revision: Option<i64>,
) -> Result<i64, ServiceError> {
    validate_id(id)?;
    let normalized_name = name.trim();
    if normalized_name.is_empty()
        || normalized_name.chars().count() > 256
        || normalized_name.chars().any(char::is_control)
        || project_id.is_some_and(|value| value.is_empty() || value.len() > 128)
    {
        return Err(invalid_request());
    }
    if document_json.len() > MAXIMUM_DOCUMENT_BYTES {
        return Err(invalid_document());
    }
    let document = serde_json::from_str::<Value>(document_json).map_err(|_| invalid_document())?;
    if !valid_document_payload(&document, id, project_id) {
        return Err(invalid_document());
    }
    if expected_revision.is_some_and(|revision| revision <= 0) {
        return Err(invalid_request());
    }

    let now = unix_time_millis()?;
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    let image_assets = document
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(invalid_document)?
        .iter()
        .filter(|node| node.get("type").and_then(Value::as_str) == Some("image"))
        .map(|node| {
            let id = node
                .get("assetId")
                .and_then(Value::as_str)
                .ok_or_else(invalid_document)?;
            let mime_type = node
                .get("mimeType")
                .and_then(Value::as_str)
                .ok_or_else(invalid_document)?;
            let size_bytes = node
                .get("sizeBytes")
                .and_then(Value::as_i64)
                .ok_or_else(invalid_document)?;
            Ok((id.to_owned(), mime_type.to_owned(), size_bytes))
        })
        .collect::<Result<HashSet<_>, ServiceError>>()?;
    let mut image_query = transaction
        .prepare("SELECT mime_type, size_bytes FROM generated_images WHERE id = ?1")
        .map_err(storage_failure)?;
    for (asset_id, mime_type, size_bytes) in image_assets {
        let stored = image_query
            .query_row([asset_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .optional()
            .map_err(storage_failure)?;
        if stored != Some((mime_type, size_bytes)) {
            return Err(invalid_document());
        }
    }
    drop(image_query);
    if let Some(project_id) = project_id {
        let project_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage_failure)?;
        if !project_exists {
            return Err(project_not_found());
        }
    }
    let current_revision = transaction
        .query_row(
            "SELECT revision FROM design_documents WHERE id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?;

    let revision = match (current_revision, expected_revision) {
        (None, None) => {
            transaction
                .execute(
                    "INSERT INTO design_documents
                        (id, name, project_id, document_json, revision, created_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                    params![id, normalized_name, project_id, document_json, now],
                )
                .map_err(storage_failure)?;
            1
        }
        (Some(current), Some(expected)) if current == expected => {
            let revision = current.checked_add(1).ok_or_else(invalid_request)?;
            transaction
                .execute(
                    "UPDATE design_documents
                     SET name = ?2, project_id = ?3, document_json = ?4, revision = ?5,
                         updated_at_unix_ms = ?6, archived_at_unix_ms = NULL
                     WHERE id = ?1 AND revision = ?7",
                    params![
                        id,
                        normalized_name,
                        project_id,
                        document_json,
                        revision,
                        now,
                        expected
                    ],
                )
                .map_err(storage_failure)?;
            revision
        }
        _ => return Err(document_conflict()),
    };

    transaction
        .execute(
            "INSERT INTO design_document_revisions
                (document_id, revision, document_json, created_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, revision, document_json, now],
        )
        .map_err(storage_failure)?;
    transaction
        .execute(
            "DELETE FROM design_document_revisions
             WHERE document_id = ?1 AND revision NOT IN (
                SELECT revision FROM design_document_revisions
                WHERE document_id = ?1 ORDER BY revision DESC LIMIT ?2
             )",
            params![id, MAXIMUM_REVISIONS_PER_DOCUMENT],
        )
        .map_err(storage_failure)?;
    transaction.commit().map_err(storage_failure)?;
    Ok(revision)
}

fn valid_document_payload(value: &Value, id: &str, project_id: Option<&str>) -> bool {
    let Some(document) = value.as_object() else {
        return false;
    };
    let stored_project_id = document.get("projectId");
    let project_matches = match (project_id, stored_project_id) {
        (None, Some(Value::Null)) => true,
        (Some(expected), Some(Value::String(actual))) => expected == actual,
        _ => false,
    };
    if !matches!(
        document.get("version").and_then(Value::as_u64),
        Some(7 | 8 | 9)
    ) || document.get("id").and_then(Value::as_str) != Some(id)
        || !project_matches
    {
        return false;
    }
    let Some(nodes) = document.get("nodes").and_then(Value::as_array) else {
        return false;
    };
    let Some(selected) = document.get("selectedNodeIds").and_then(Value::as_array) else {
        return false;
    };
    let Some(pages) = document.get("pages").and_then(Value::as_array) else {
        return false;
    };
    let Some(active_page_id) = document.get("activePageId").and_then(Value::as_str) else {
        return false;
    };
    if pages.is_empty() || pages.len() > 100 {
        return false;
    }
    let mut page_ids = HashSet::with_capacity(pages.len());
    for page in pages {
        let Some(page) = page.as_object() else {
            return false;
        };
        let Some(page_id) = page.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(name) = page.get("name").and_then(Value::as_str) else {
            return false;
        };
        if page_id.is_empty()
            || page_id.len() > 128
            || page_id.chars().any(char::is_control)
            || name.trim().is_empty()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
            || !page_ids.insert(page_id.to_owned())
        {
            return false;
        }
    }
    if !page_ids.contains(active_page_id) {
        return false;
    }
    let mut variable_ids = HashSet::new();
    if let Some(variables) = document.get("variables") {
        let Some(variables) = variables.as_array() else {
            return false;
        };
        if variables.len() > 1_000 {
            return false;
        }
        for variable in variables {
            let Some(variable) = variable.as_object() else {
                return false;
            };
            let Some(variable_id) = variable.get("id").and_then(Value::as_str) else {
                return false;
            };
            let Some(name) = variable.get("name").and_then(Value::as_str) else {
                return false;
            };
            if variable_id.is_empty()
                || variable_id.len() > 128
                || name.trim().is_empty()
                || name.chars().count() > 128
                || !valid_color(variable.get("value"))
                || !variable_ids.insert(variable_id.to_owned())
            {
                return false;
            }
        }
    }
    let Some(text_style_ids) = valid_text_styles(document.get("textStyles")) else {
        return false;
    };
    if nodes.len() > MAXIMUM_DOCUMENT_NODES
        || selected.len() > MAXIMUM_DOCUMENT_NODES
        || !nodes.iter().all(|node| {
            valid_node_payload(node, &variable_ids, &page_ids)
                && valid_text_style_reference(node, &text_style_ids)
        })
    {
        return false;
    }
    let mut identifiers = HashSet::with_capacity(nodes.len());
    let mut parents = HashMap::with_capacity(nodes.len());
    let mut frame_ids = HashSet::new();
    let mut node_pages = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let Some(node) = node.as_object() else {
            return false;
        };
        let Some(node_id) = node.get("id").and_then(Value::as_str) else {
            return false;
        };
        if !identifiers.insert(node_id) {
            return false;
        }
        let Some(page_id) = node.get("pageId").and_then(Value::as_str) else {
            return false;
        };
        node_pages.insert(node_id, page_id);
        if node.get("type").and_then(Value::as_str) == Some("frame") {
            frame_ids.insert(node_id);
        }
        match node.get("parentId") {
            Some(Value::Null) => {}
            Some(Value::String(parent_id)) => {
                parents.insert(node_id, parent_id.as_str());
            }
            _ => return false,
        }
    }
    if !valid_components(document.get("components"), nodes, &identifiers, &frame_ids) {
        return false;
    }
    if !valid_motion_tracks(document.get("motionTracks"), &identifiers) {
        return false;
    }
    if parents.iter().any(|(node_id, parent_id)| {
        !identifiers.contains(parent_id)
            || !frame_ids.contains(parent_id)
            || node_pages.get(node_id) != node_pages.get(parent_id)
    }) {
        return false;
    }
    if !valid_prototype_payload(
        document.get("prototypeFlows"),
        document.get("prototypeInteractions"),
        &page_ids,
        &identifiers,
        &frame_ids,
        &node_pages,
    ) {
        return false;
    }
    if !valid_comments(document.get("comments"), &page_ids, &frame_ids, &node_pages) {
        return false;
    }
    let mut completed = HashSet::with_capacity(nodes.len());
    for node_id in &identifiers {
        let mut path = HashSet::new();
        let mut current = Some(*node_id);
        while let Some(current_id) = current {
            if completed.contains(current_id) {
                break;
            }
            if !path.insert(current_id) {
                return false;
            }
            current = parents.get(current_id).copied();
        }
        completed.extend(path);
    }
    let selected_ids: Vec<_> = selected.iter().filter_map(Value::as_str).collect();
    let selected_set: HashSet<_> = selected_ids.iter().copied().collect();
    selected_ids.len() == selected.len()
        && selected_set.len() == selected_ids.len()
        && selected_ids.iter().all(|selected_id| {
            identifiers.contains(selected_id)
                && node_pages.get(selected_id).copied() == Some(active_page_id)
        })
}

fn valid_text_styles(value: Option<&Value>) -> Option<HashSet<String>> {
    let Some(value) = value else {
        return Some(HashSet::new());
    };
    let styles = value.as_array()?;
    if styles.len() > 2_000 {
        return None;
    }
    let mut ids = HashSet::with_capacity(styles.len());
    let mut names = HashSet::with_capacity(styles.len());
    for style in styles {
        let style = style.as_object()?;
        let id = style.get("id")?.as_str()?;
        let name = style.get("name")?.as_str()?;
        let color = style.get("color")?;
        let font_size = style.get("fontSize")?.as_f64()?;
        let font_weight = style.get("fontWeight")?.as_u64()?;
        if id.is_empty()
            || id.len() > 128
            || id.chars().any(char::is_control)
            || name.trim().is_empty()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
            || !valid_color(Some(color))
            || !font_size.is_finite()
            || !(1.0..=4096.0).contains(&font_size)
            || !(100..=900).contains(&font_weight)
            || font_weight % 100 != 0
            || !ids.insert(id.to_owned())
            || !names.insert(name.to_lowercase())
            || style.len() != 5
        {
            return None;
        }
    }
    Some(ids)
}

fn valid_text_style_reference(node: &Value, style_ids: &HashSet<String>) -> bool {
    let Some(node) = node.as_object() else {
        return false;
    };
    match node.get("textStyleId") {
        None => true,
        Some(Value::String(id)) => {
            node.get("type").and_then(Value::as_str) == Some("text") && style_ids.contains(id)
        }
        _ => false,
    }
}

fn valid_motion_tracks(value: Option<&Value>, node_ids: &HashSet<&str>) -> bool {
    let Some(tracks) = value.and_then(Value::as_array) else {
        return false;
    };
    if tracks.len() > MAXIMUM_DESIGN_MOTION_TRACKS {
        return false;
    }
    let mut track_ids = HashSet::with_capacity(tracks.len());
    let mut node_properties = HashSet::with_capacity(tracks.len());
    for value in tracks {
        let Some(track) = value.as_object() else {
            return false;
        };
        let Some(id) = track.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(node_id) = track.get("nodeId").and_then(Value::as_str) else {
            return false;
        };
        let Some(property) = track.get("property").and_then(Value::as_str) else {
            return false;
        };
        let Some(keyframes) = track.get("keyframes").and_then(Value::as_array) else {
            return false;
        };
        if id.is_empty()
            || id.len() > 128
            || !node_ids.contains(node_id)
            || !matches!(property, "x" | "y" | "rotation" | "opacity" | "scale")
            || keyframes.len() < 2
            || keyframes.len() > 100
            || !track_ids.insert(id.to_owned())
            || !node_properties.insert((node_id.to_owned(), property.to_owned()))
            || track
                .keys()
                .any(|key| !matches!(key.as_str(), "id" | "nodeId" | "property" | "keyframes"))
        {
            return false;
        }
        let mut keyframe_ids = HashSet::with_capacity(keyframes.len());
        let mut previous_time = None;
        for (index, value) in keyframes.iter().enumerate() {
            let Some(keyframe) = value.as_object() else {
                return false;
            };
            let Some(keyframe_id) = keyframe.get("id").and_then(Value::as_str) else {
                return false;
            };
            let Some(time_ms) = keyframe.get("timeMs").and_then(Value::as_u64) else {
                return false;
            };
            let Some(number) = keyframe.get("value").and_then(Value::as_f64) else {
                return false;
            };
            let easing = keyframe.get("easing").and_then(Value::as_str);
            let valid_value = match property {
                "opacity" => (0.0..=1.0).contains(&number),
                "scale" => (0.01..=100.0).contains(&number),
                "rotation" => number.abs() <= 3600.0,
                _ => number.abs() <= 9_007_199_254_740_991.0,
            };
            if keyframe_id.is_empty()
                || keyframe_id.len() > 128
                || time_ms > 10_000
                || (index == 0 && time_ms != 0)
                || previous_time.is_some_and(|previous| time_ms <= previous)
                || !valid_value
                || !matches!(
                    easing,
                    Some("linear" | "ease-in" | "ease-out" | "ease-in-out")
                )
                || !keyframe_ids.insert(keyframe_id.to_owned())
                || keyframe
                    .keys()
                    .any(|key| !matches!(key.as_str(), "id" | "timeMs" | "value" | "easing"))
            {
                return false;
            }
            previous_time = Some(time_ms);
        }
    }
    true
}

fn valid_components(
    value: Option<&Value>,
    nodes: &[Value],
    node_ids: &HashSet<&str>,
    frame_ids: &HashSet<&str>,
) -> bool {
    let components = match value {
        None => &[][..],
        Some(value) => match value.as_array() {
            Some(items) => items.as_slice(),
            None => return false,
        },
    };
    if components.len() > MAXIMUM_DESIGN_COMPONENTS {
        return false;
    }
    let mut component_ids = HashSet::with_capacity(components.len());
    let mut component_names = HashSet::with_capacity(components.len());
    let mut variants_by_component: HashMap<String, HashSet<String>> = HashMap::new();
    let mut roots = Vec::new();
    for value in components {
        let Some(component) = value.as_object() else {
            return false;
        };
        let Some(id) = component.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(name) = component.get("name").and_then(Value::as_str) else {
            return false;
        };
        let Some(variants) = component.get("variants").and_then(Value::as_array) else {
            return false;
        };
        if id.is_empty()
            || id.len() > 128
            || name.trim().is_empty()
            || name.chars().count() > 128
            || variants.is_empty()
            || variants.len() > 100
            || !component_ids.insert(id.to_owned())
            || !component_names.insert(name.to_lowercase())
            || component
                .keys()
                .any(|key| !matches!(key.as_str(), "id" | "name" | "variants"))
        {
            return false;
        }
        let mut variant_ids = HashSet::with_capacity(variants.len());
        let mut variant_names = HashSet::with_capacity(variants.len());
        for value in variants {
            let Some(variant) = value.as_object() else {
                return false;
            };
            let Some(variant_id) = variant.get("id").and_then(Value::as_str) else {
                return false;
            };
            let Some(variant_name) = variant.get("name").and_then(Value::as_str) else {
                return false;
            };
            let Some(root_node_id) = variant.get("rootNodeId").and_then(Value::as_str) else {
                return false;
            };
            if variant_id.is_empty()
                || variant_id.len() > 128
                || variant_name.trim().is_empty()
                || variant_name.chars().count() > 128
                || !frame_ids.contains(root_node_id)
                || !variant_ids.insert(variant_id.to_owned())
                || !variant_names.insert(variant_name.to_lowercase())
                || variant
                    .keys()
                    .any(|key| !matches!(key.as_str(), "id" | "name" | "rootNodeId"))
            {
                return false;
            }
            roots.push((id, variant_id, root_node_id));
        }
        variants_by_component.insert(id.to_owned(), variant_ids);
    }
    for node_value in nodes {
        let Some(node) = node_value.as_object() else {
            return false;
        };
        let Some(reference) = node.get("componentRef") else {
            continue;
        };
        let Some(reference) = reference.as_object() else {
            return false;
        };
        let Some(component_id) = reference.get("componentId").and_then(Value::as_str) else {
            return false;
        };
        let Some(source_node_id) = reference.get("sourceNodeId").and_then(Value::as_str) else {
            return false;
        };
        let Some(variant_id) = reference.get("variantId").and_then(Value::as_str) else {
            return false;
        };
        let Some(role) = reference.get("role").and_then(Value::as_str) else {
            return false;
        };
        let Some(node_id) = node.get("id").and_then(Value::as_str) else {
            return false;
        };
        if !variants_by_component
            .get(component_id)
            .is_some_and(|variants| variants.contains(variant_id))
            || !matches!(role, "master" | "variant" | "instance")
            || (role == "master" && source_node_id != node_id)
            || (role != "master" && !node_ids.contains(source_node_id))
            || reference.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "componentId" | "sourceNodeId" | "variantId" | "role"
                )
            })
        {
            return false;
        }
        if role != "master" {
            let source = nodes.iter().find(|candidate| {
                candidate.get("id").and_then(Value::as_str) == Some(source_node_id)
            });
            let Some(source_ref) = source
                .and_then(|candidate| candidate.get("componentRef"))
                .and_then(Value::as_object)
            else {
                return false;
            };
            if source_ref.get("role").and_then(Value::as_str) != Some("master")
                || source_ref.get("componentId").and_then(Value::as_str) != Some(component_id)
            {
                return false;
            }
        }
    }
    roots
        .into_iter()
        .all(|(component_id, variant_id, root_id)| {
            nodes
                .iter()
                .find(|node| node.get("id").and_then(Value::as_str) == Some(root_id))
                .and_then(|node| node.get("componentRef"))
                .and_then(Value::as_object)
                .is_some_and(|reference| {
                    reference.get("componentId").and_then(Value::as_str) == Some(component_id)
                        && reference.get("variantId").and_then(Value::as_str) == Some(variant_id)
                        && matches!(
                            reference.get("role").and_then(Value::as_str),
                            Some("master" | "variant")
                        )
                })
        })
}

fn valid_prototype_payload(
    flows_value: Option<&Value>,
    interactions_value: Option<&Value>,
    page_ids: &HashSet<String>,
    node_ids: &HashSet<&str>,
    frame_ids: &HashSet<&str>,
    node_pages: &HashMap<&str, &str>,
) -> bool {
    let flows = match flows_value {
        None => &[][..],
        Some(value) => match value.as_array() {
            Some(items) => items.as_slice(),
            None => return false,
        },
    };
    let interactions = match interactions_value {
        None => &[][..],
        Some(value) => match value.as_array() {
            Some(items) => items.as_slice(),
            None => return false,
        },
    };
    if flows.len() > MAXIMUM_PROTOTYPE_FLOWS || interactions.len() > MAXIMUM_PROTOTYPE_INTERACTIONS
    {
        return false;
    }
    let mut flow_ids = HashSet::with_capacity(flows.len());
    let mut flow_names = HashSet::with_capacity(flows.len());
    for value in flows {
        let Some(flow) = value.as_object() else {
            return false;
        };
        let Some(id) = flow.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(name) = flow.get("name").and_then(Value::as_str) else {
            return false;
        };
        let Some(page_id) = flow.get("pageId").and_then(Value::as_str) else {
            return false;
        };
        let Some(start_node_id) = flow.get("startNodeId").and_then(Value::as_str) else {
            return false;
        };
        if id.is_empty()
            || id.len() > 128
            || name.trim().is_empty()
            || name.chars().count() > 128
            || !page_ids.contains(page_id)
            || !frame_ids.contains(start_node_id)
            || node_pages.get(start_node_id).copied() != Some(page_id)
            || !flow_ids.insert(id.to_owned())
            || !flow_names.insert(name.to_lowercase())
            || flow
                .keys()
                .any(|key| !matches!(key.as_str(), "id" | "name" | "pageId" | "startNodeId"))
        {
            return false;
        }
    }
    let mut interaction_ids = HashSet::with_capacity(interactions.len());
    let mut source_ids = HashSet::with_capacity(interactions.len());
    for value in interactions {
        let Some(interaction) = value.as_object() else {
            return false;
        };
        let Some(id) = interaction.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(source) = interaction.get("sourceNodeId").and_then(Value::as_str) else {
            return false;
        };
        let Some(target) = interaction.get("targetNodeId").and_then(Value::as_str) else {
            return false;
        };
        let Some(duration) = interaction.get("durationMs").and_then(Value::as_u64) else {
            return false;
        };
        let transition = interaction.get("transition").and_then(Value::as_str);
        let valid_duration = match transition {
            Some("instant") => duration == 0,
            Some("dissolve" | "slide") => (1..=2_000).contains(&duration),
            _ => false,
        };
        if id.is_empty()
            || id.len() > 128
            || !node_ids.contains(source)
            || !frame_ids.contains(target)
            || source == target
            || node_pages.get(source).is_none()
            || node_pages.get(source) != node_pages.get(target)
            || !valid_duration
            || !matches!(
                interaction.get("trigger").and_then(Value::as_str),
                Some("click" | "hover")
            )
            || !matches!(
                interaction.get("presentation").and_then(Value::as_str),
                None | Some("navigate" | "overlay")
            )
            || !interaction_ids.insert(id.to_owned())
            || !source_ids.insert(source.to_owned())
            || interaction.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "id" | "sourceNodeId"
                        | "targetNodeId"
                        | "trigger"
                        | "presentation"
                        | "transition"
                        | "durationMs"
                )
            })
        {
            return false;
        }
    }
    true
}

fn valid_comments(
    value: Option<&Value>,
    page_ids: &HashSet<String>,
    frame_ids: &HashSet<&str>,
    node_pages: &HashMap<&str, &str>,
) -> bool {
    let Some(value) = value else {
        return true;
    };
    let Some(comments) = value.as_array() else {
        return false;
    };
    if comments.len() > MAXIMUM_DOCUMENT_COMMENTS {
        return false;
    }
    let mut comment_ids = HashSet::with_capacity(comments.len());
    comments.iter().all(|comment| {
        let Some(comment) = comment.as_object() else {
            return false;
        };
        let Some(id) = comment.get("id").and_then(Value::as_str) else {
            return false;
        };
        let Some(page_id) = comment.get("pageId").and_then(Value::as_str) else {
            return false;
        };
        let Some(text) = comment.get("text").and_then(Value::as_str) else {
            return false;
        };
        let valid_node = match comment.get("nodeId") {
            Some(Value::Null) => true,
            Some(Value::String(node_id)) => {
                frame_ids.contains(node_id.as_str())
                    && node_pages.get(node_id.as_str()).copied() == Some(page_id)
            }
            _ => false,
        };
        let valid_point = match comment.get("point") {
            Some(Value::Null) => true,
            Some(Value::Object(point)) => ["x", "y"].iter().all(|axis| {
                point
                    .get(*axis)
                    .and_then(Value::as_f64)
                    .is_some_and(|coordinate| (0.0..=1.0).contains(&coordinate))
            }),
            _ => false,
        };
        id.len() <= 128
            && !id.is_empty()
            && comment_ids.insert(id.to_owned())
            && page_ids.contains(page_id)
            && text.trim().len() > 0
            && text.chars().count() <= 2_000
            && comment
                .get("createdAt")
                .and_then(Value::as_f64)
                .is_some_and(|created_at| created_at.is_finite())
            && comment.get("resolved").and_then(Value::as_bool).is_some()
            && valid_node
            && valid_point
            && comment.keys().all(|key| {
                matches!(
                    key.as_str(),
                    "id" | "pageId" | "nodeId" | "text" | "point" | "createdAt" | "resolved"
                )
            })
    })
}

fn valid_node_payload(
    value: &Value,
    variable_ids: &HashSet<String>,
    page_ids: &HashSet<String>,
) -> bool {
    let Some(node) = value.as_object() else {
        return false;
    };
    let bounded_string = |key: &str, maximum: usize| {
        node.get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| value.chars().count() <= maximum)
    };
    let non_empty_string = |key: &str, maximum: usize| {
        node.get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty() && value.chars().count() <= maximum)
    };
    let number = |key: &str| {
        node.get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
    };
    let valid_parent = matches!(
        node.get("parentId"),
        Some(Value::Null) | Some(Value::String(_))
    );
    let valid_page = node
        .get("pageId")
        .and_then(Value::as_str)
        .is_some_and(|page_id| page_ids.contains(page_id));
    let Some(node_type) = node.get("type").and_then(Value::as_str) else {
        return false;
    };
    if !non_empty_string("id", 128)
        || !bounded_string("name", 256)
        || !valid_page
        || !number("x").is_some_and(|value| value.abs() <= 9_007_199_254_740_991.0)
        || !number("y").is_some_and(|value| value.abs() <= 9_007_199_254_740_991.0)
        || !number("width").is_some_and(|value| (0.0..=1_000_000.0).contains(&value) && value > 0.0)
        || !number("height")
            .is_some_and(|value| (0.0..=1_000_000.0).contains(&value) && value > 0.0)
        || !valid_parent
        || !number("rotation").is_some_and(|value| value.abs() <= 360_000.0)
        || node.get("visible").and_then(Value::as_bool).is_none()
        || node.get("locked").and_then(Value::as_bool).is_none()
        || !valid_effects(node)
        || !valid_constraints(node)
        || !valid_responsive_overrides(node)
    {
        return false;
    }
    match node_type {
        "frame" => {
            matches!(node.get("scrollable"), None | Some(Value::Bool(_)))
                && valid_color(node.get("fill"))
                && valid_variable_reference(node, "fillVariableId", variable_ids)
                && valid_stroke(node)
                && valid_auto_layout(node)
        }
        "ellipse" => {
            valid_color(node.get("fill"))
                && valid_variable_reference(node, "fillVariableId", variable_ids)
                && valid_stroke(node)
        }
        "path" => {
            let Some(points) = node.get("points").and_then(Value::as_array) else {
                return false;
            };
            let closed = node.get("closed").and_then(Value::as_bool);
            let minimum = if closed == Some(true) { 3 } else { 2 };
            points.len() >= minimum
                && points.len() <= 512
                && closed.is_some()
                && points.iter().all(|point| {
                    let Some(point) = point.as_object() else {
                        return false;
                    };
                    point.len() == 2
                        && point
                            .get("x")
                            .and_then(Value::as_f64)
                            .is_some_and(|value| value.is_finite() && value.abs() <= 1_000_000.0)
                        && point
                            .get("y")
                            .and_then(Value::as_f64)
                            .is_some_and(|value| value.is_finite() && value.abs() <= 1_000_000.0)
                })
                && valid_color(node.get("fill"))
                && valid_variable_reference(node, "fillVariableId", variable_ids)
                && valid_stroke(node)
        }
        "rectangle" => {
            valid_color(node.get("fill"))
                && valid_variable_reference(node, "fillVariableId", variable_ids)
                && number("radius").is_some_and(|value| value >= 0.0)
                && valid_stroke(node)
        }
        "text" => {
            bounded_string("content", 100_000)
                && valid_color(node.get("color"))
                && valid_variable_reference(node, "colorVariableId", variable_ids)
                && number("fontSize")
                    .is_some_and(|value| (0.0..=4096.0).contains(&value) && value > 0.0)
                && node
                    .get("fontWeight")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| (100..=900).contains(&value) && value % 100 == 0)
        }
        "imported-svg" => {
            non_empty_string("svg", MAXIMUM_DOCUMENT_BYTES)
                && node.get("tokens").and_then(Value::as_object).is_some()
        }
        "image" => {
            non_empty_string("assetId", 32)
                && node
                    .get("assetId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| {
                        id.len() == 32
                            && id
                                .bytes()
                                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                    })
                && matches!(
                    node.get("mimeType").and_then(Value::as_str),
                    Some("image/png" | "image/jpeg" | "image/webp")
                )
                && node
                    .get("sizeBytes")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| (1..=33_554_432).contains(&value))
                && matches!(
                    node.get("fit").and_then(Value::as_str),
                    Some("cover" | "contain")
                )
                && bounded_string("altText", 256)
                && node
                    .get("altText")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.chars().any(char::is_control))
        }
        _ => false,
    }
}

fn valid_responsive_overrides(node: &serde_json::Map<String, Value>) -> bool {
    let Some(value) = node.get("responsiveOverrides") else {
        return true;
    };
    let Some(overrides) = value.as_object() else {
        return false;
    };
    if overrides
        .keys()
        .any(|key| key != "mobile" && key != "tablet")
    {
        return false;
    }
    overrides.values().all(|geometry| {
        let Some(geometry) = geometry.as_object() else {
            return false;
        };
        let number = |key: &str| {
            geometry
                .get(key)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
        };
        number("x").is_some_and(|value| value.abs() <= 9_007_199_254_740_991.0)
            && number("y").is_some_and(|value| value.abs() <= 9_007_199_254_740_991.0)
            && number("width").is_some_and(|value| (1.0..=1_000_000.0).contains(&value))
            && number("height").is_some_and(|value| (1.0..=1_000_000.0).contains(&value))
            && geometry.len() == 4
    })
}

fn valid_auto_layout(node: &serde_json::Map<String, Value>) -> bool {
    let Some(layout) = node.get("autoLayout") else {
        return true;
    };
    let Some(layout) = layout.as_object() else {
        return false;
    };
    matches!(
        layout.get("direction").and_then(Value::as_str),
        Some("horizontal" | "vertical")
    ) && matches!(
        layout.get("alignment").and_then(Value::as_str),
        Some("start" | "center" | "end" | "stretch")
    ) && layout
        .get("gap")
        .and_then(Value::as_f64)
        .is_some_and(|value| (0.0..=4096.0).contains(&value))
        && layout
            .get("padding")
            .and_then(Value::as_f64)
            .is_some_and(|value| (0.0..=4096.0).contains(&value))
}

fn valid_effects(node: &serde_json::Map<String, Value>) -> bool {
    let Some(effects) = node.get("effects") else {
        return true;
    };
    let Some(effects) = effects.as_array() else {
        return false;
    };
    effects.len() <= 8
        && effects.iter().all(|effect| {
            let Some(effect) = effect.as_object() else {
                return false;
            };
            effect.get("type").and_then(Value::as_str) == Some("shadow")
                && valid_color(effect.get("color"))
                && effect
                    .get("offsetX")
                    .and_then(Value::as_f64)
                    .is_some_and(|value| value.abs() <= 4096.0)
                && effect
                    .get("offsetY")
                    .and_then(Value::as_f64)
                    .is_some_and(|value| value.abs() <= 4096.0)
                && effect
                    .get("blur")
                    .and_then(Value::as_f64)
                    .is_some_and(|value| (0.0..=4096.0).contains(&value))
        })
}

fn valid_constraints(node: &serde_json::Map<String, Value>) -> bool {
    let Some(constraints) = node.get("constraints") else {
        return true;
    };
    let Some(constraints) = constraints.as_object() else {
        return false;
    };
    let valid_mode = |key: &str| {
        matches!(
            constraints.get(key).and_then(Value::as_str),
            Some("start" | "end" | "start-end" | "center" | "scale")
        )
    };
    valid_mode("horizontal") && valid_mode("vertical")
}

fn valid_stroke(node: &serde_json::Map<String, Value>) -> bool {
    let color_is_valid = node
        .get("stroke")
        .is_none_or(|value| valid_color(Some(value)));
    let width_is_valid = node.get("strokeWidth").is_none_or(|value| {
        value
            .as_f64()
            .is_some_and(|width| (0.0..=4096.0).contains(&width))
    });
    color_is_valid && width_is_valid
}

fn valid_variable_reference(
    node: &serde_json::Map<String, Value>,
    key: &str,
    variable_ids: &HashSet<String>,
) -> bool {
    match node.get(key) {
        None | Some(Value::Null) => true,
        Some(Value::String(id)) => variable_ids.contains(id),
        _ => false,
    }
}

fn valid_color(value: Option<&Value>) -> bool {
    value.and_then(Value::as_str).is_some_and(|value| {
        value.len() == 7
            && value.starts_with('#')
            && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

pub(crate) fn archive(storage: &AppStorage, id: &str) -> Result<bool, ServiceError> {
    validate_id(id)?;
    let connection = storage.connect().map_err(storage_failure)?;
    let updated = connection
        .execute(
            "UPDATE design_documents SET archived_at_unix_ms = ?2, updated_at_unix_ms = ?2
             WHERE id = ?1 AND archived_at_unix_ms IS NULL",
            params![id, unix_time_millis()?],
        )
        .map_err(storage_failure)?;
    Ok(updated > 0)
}

fn read_summary(row: &StorageRow) -> rusqlite::Result<DesignDocumentSummary> {
    Ok(DesignDocumentSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        project_id: row.get(2)?,
        revision: row.get(3)?,
        created_at_unix_ms: row.get(4)?,
        updated_at_unix_ms: row.get(5)?,
        archived_at_unix_ms: row.get(6)?,
    })
}

fn read_shared_text_style(row: &StorageRow) -> rusqlite::Result<SharedDesignTextStyle> {
    Ok(SharedDesignTextStyle {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        font_size: row.get(3)?,
        font_weight: row.get(4)?,
        revision: row.get(5)?,
        updated_at_unix_ms: row.get(6)?,
    })
}

fn validate_id(id: &str) -> Result<(), ServiceError> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    Ok(())
}

fn unix_time_millis() -> Result<i64, ServiceError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_failure(rusqlite::Error::InvalidQuery))?;
    i64::try_from(duration.as_millis()).map_err(|_| invalid_request())
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The Design document request is invalid.",
        false,
    )
}

fn invalid_document() -> ServiceError {
    ServiceError::new(
        "design_document_invalid",
        "The Design document data is invalid or exceeds its size limit.",
        false,
    )
}

fn document_conflict() -> ServiceError {
    ServiceError::new(
        "design_document_revision_conflict",
        "The Design document changed in another save. Reload it before saving again.",
        true,
    )
}

fn shared_style_conflict() -> ServiceError {
    ServiceError::new(
        "design_text_style_revision_conflict",
        "The shared text style changed elsewhere. Reload the library before saving again.",
        true,
    )
}

fn project_not_found() -> ServiceError {
    ServiceError::new(
        "design_document_project_not_found",
        "The selected project is no longer available.",
        false,
    )
}

fn storage_failure(_error: impl std::fmt::Display) -> ServiceError {
    ServiceError::new(
        "design_document_storage_unavailable",
        "Design document storage is unavailable.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        archive, delete_shared_text_style, get, list, list_shared_text_styles, revisions, save,
        save_shared_text_style, valid_document_payload,
    };
    use crate::storage::AppStorage;
    use serde_json::{Value, json};
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Barrier},
        thread,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-design-documents-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create isolated Design storage directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated Design storage directory");
        }
    }

    fn open_storage() -> (TestDirectory, AppStorage) {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open isolated storage");
        let connection = storage.connect().expect("connect isolated storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL, title TEXT);
                 ALTER TABLE conversations ADD COLUMN project_id TEXT;
                 ALTER TABLE conversations ADD COLUMN model_id TEXT;
                 ALTER TABLE conversations ADD COLUMN connection_id TEXT;
                 ALTER TABLE conversations ADD COLUMN workspace_id TEXT;
                 CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT);
                 CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    UNIQUE (conversation_id, id)
                 );",
            )
            .expect("create prerequisite desktop schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize current Design storage schema");
        (directory, storage)
    }

    fn document(id: &str, name: &str, project_id: Option<&str>) -> String {
        serde_json::to_string(&json!({
            "version": 8,
            "id": id,
            "name": name,
            "projectId": project_id,
            "pages": [{"id": "page-1", "name": "Page 1"}],
            "activePageId": "page-1",
            "nodes": [],
            "selectedNodeIds": [],
            "variables": [],
            "comments": [],
            "prototypeFlows": [],
            "prototypeInteractions": [],
            "motionTracks": [],
            "components": []
        }))
        .expect("serialize valid Design document")
    }

    fn error_code(error: &crate::protocol::ServiceError) -> &str {
        error.code
    }

    #[test]
    fn durable_documents_create_update_revision_list_and_archive_without_losing_history() {
        let (_directory, storage) = open_storage();
        let first = document("design-1", "First", None);

        assert_eq!(
            save(&storage, "design-1", "First", None, &first, None).unwrap(),
            1
        );
        assert_eq!(
            get(&storage, "design-1").unwrap().unwrap().document_json,
            first
        );

        let updated = document("design-1", "Updated", None);
        assert_eq!(
            save(&storage, "design-1", "Updated", None, &updated, Some(1)).unwrap(),
            2
        );
        assert_eq!(list(&storage, None, false, 100).unwrap().len(), 1);
        assert_eq!(revisions(&storage, "design-1", 100).unwrap().len(), 2);
        assert_eq!(
            revisions(&storage, "design-1", 100).unwrap()[0].document_json,
            updated
        );

        assert!(archive(&storage, "design-1").unwrap());
        assert!(get(&storage, "design-1").unwrap().is_none());
        assert!(list(&storage, None, false, 100).unwrap().is_empty());
        assert_eq!(list(&storage, None, true, 100).unwrap().len(), 1);
        assert_eq!(revisions(&storage, "design-1", 100).unwrap().len(), 2);
    }

    #[test]
    fn shared_text_style_library_uses_validated_revisioned_encrypted_storage() {
        let (_directory, storage) = open_storage();
        assert_eq!(
            save_shared_text_style(&storage, "style-1", "Heading", "#123456", 28.0, 600, None)
                .unwrap(),
            1
        );
        let listed = list_shared_text_styles(&storage, 2_000).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Heading");
        assert_eq!(listed[0].color, "#123456");
        assert_eq!(listed[0].revision, 1);
        assert!(
            save_shared_text_style(&storage, "style-1", "Heading", "#123456", 28.0, 600, None)
                .is_err()
        );
        assert_eq!(
            save_shared_text_style(
                &storage,
                "style-1",
                "Display",
                "#ABCDEF",
                32.0,
                700,
                Some(1)
            )
            .unwrap(),
            2
        );
        let conflict =
            save_shared_text_style(&storage, "style-1", "Stale", "#000000", 20.0, 500, Some(1))
                .unwrap_err();
        assert_eq!(error_code(&conflict), "design_text_style_revision_conflict");
        assert!(
            save_shared_text_style(&storage, "style-2", "Invalid", "red", 20.0, 500, None).is_err()
        );
        assert!(delete_shared_text_style(&storage, "style-1", 1).is_err());
        assert!(delete_shared_text_style(&storage, "style-1", 2).unwrap());
        assert!(list_shared_text_styles(&storage, 2_000).unwrap().is_empty());
    }

    #[test]
    fn durable_document_saves_reject_stale_revisions_and_nonexistent_projects() {
        let (_directory, storage) = open_storage();
        let first = document("design-1", "First", None);
        save(&storage, "design-1", "First", None, &first, None).unwrap();

        let stale = save(&storage, "design-1", "Stale", None, &first, Some(0)).unwrap_err();
        assert_eq!(error_code(&stale), "invalid_request_params");
        let stale = save(&storage, "design-1", "Stale", None, &first, None).unwrap_err();
        assert_eq!(error_code(&stale), "design_document_revision_conflict");

        let linked = document("design-2", "Linked", Some("missing-project"));
        let missing_project = save(
            &storage,
            "design-2",
            "Linked",
            Some("missing-project"),
            &linked,
            None,
        )
        .unwrap_err();
        assert_eq!(
            error_code(&missing_project),
            "design_document_project_not_found"
        );
        assert!(get(&storage, "design-2").unwrap().is_none());
    }

    #[test]
    fn concurrent_saves_from_the_same_revision_have_exactly_one_winner() {
        let (directory, storage) = open_storage();
        let initial = document("design-1", "First", None);
        save(&storage, "design-1", "First", None, &initial, None).unwrap();

        let storage = Arc::new(storage);
        let barrier = Arc::new(Barrier::new(3));
        let writers = ["Second", "Third"].map(|name| {
            let storage = Arc::clone(&storage);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let value = document("design-1", name, None);
                barrier.wait();
                save(&storage, "design-1", name, None, &value, Some(1))
            })
        });
        barrier.wait();
        let results = writers.map(|writer| writer.join().expect("join Design save worker"));

        assert_eq!(
            results
                .iter()
                .filter(|result| result.as_ref().is_ok())
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .map(error_code)
                .collect::<Vec<_>>(),
            ["design_document_revision_conflict"]
        );
        assert_eq!(
            get(&storage, "design-1").unwrap().unwrap().summary.revision,
            2
        );
        assert_eq!(revisions(&storage, "design-1", 100).unwrap().len(), 2);
        drop(storage);
        drop(directory);
    }

    #[test]
    fn document_payload_must_match_rpc_identity_and_project() {
        let valid = json!({
            "version": 7,
            "id": "design-1",
            "name": "Design",
            "projectId": "project-1",
            "pages": [{"id": "page-1", "name": "Page 1"}],
            "activePageId": "page-1",
            "nodes": [],
            "selectedNodeIds": [],
            "motionTracks": []
        });

        assert!(valid_document_payload(
            &valid,
            "design-1",
            Some("project-1")
        ));
        assert!(!valid_document_payload(
            &valid,
            "design-2",
            Some("project-1")
        ));
        assert!(!valid_document_payload(&valid, "design-1", None));
    }

    #[test]
    fn version_nine_text_styles_are_bounded_and_node_references_must_resolve() {
        let text = json!({
            "id": "text-1", "pageId": "page-1", "type": "text", "name": "Title",
            "x": 0, "y": 0, "width": 200, "height": 40, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "content": "Welcome", "color": "#000000",
            "fontSize": 24, "fontWeight": 500, "textStyleId": "style-1"
        });
        let mut document = json!({
            "version": 9, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [text], "selectedNodeIds": [], "variables": [], "textStyles": [{
                "id": "style-1", "name": "Heading", "color": "#000000", "fontSize": 24, "fontWeight": 500
            }], "motionTracks": []
        });

        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["textStyleId"] = json!("missing-style");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["textStyleId"] = json!("style-1");
        document["textStyles"][0]["fontSize"] = json!(5000);
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn responsive_overrides_accept_bounded_geometry_and_reject_invalid_values() {
        let frame = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Screen",
            "x": 0, "y": 0, "width": 1024, "height": 768, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF",
            "responsiveOverrides": {
                "mobile": {"x": 0, "y": 0, "width": 360, "height": 640},
                "tablet": {"x": 0, "y": 0, "width": 768, "height": 1024}
            }
        });
        let mut document = json!({
            "version": 8, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [frame], "selectedNodeIds": [], "motionTracks": []
        });

        assert!(valid_document_payload(&document, "design-1", None));
        document["version"] = json!(7);
        assert!(valid_document_payload(&document, "design-1", None));
        document["version"] = json!(8);
        document["nodes"][0]["responsiveOverrides"]["desktop"] = json!({
            "x": 0, "y": 0, "width": 1280, "height": 720
        });
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["responsiveOverrides"] = json!({
            "mobile": {"x": 0, "y": 0, "width": 0, "height": 640}
        });
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["responsiveOverrides"] = json!({
            "tablet": {"x": 0, "y": 0, "width": 768, "height": 1024, "extra": true}
        });
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn vector_path_payloads_require_bounded_finite_points() {
        let path = json!({
            "id": "path-1", "pageId": "page-1", "type": "path", "name": "Triangle",
            "x": 0, "y": 0, "width": 160, "height": 120, "parentId": null, "rotation": 0,
            "visible": true, "locked": false,
            "points": [{"x": 80, "y": 0}, {"x": 160, "y": 120}, {"x": 0, "y": 120}],
            "closed": true, "fill": "#D9DDE2", "stroke": "#000000", "strokeWidth": 2
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [path], "selectedNodeIds": [], "motionTracks": []
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["points"][1]["x"] = json!(1_000_001);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["points"][1]["x"] = json!(160);
        document["nodes"][0]["points"] = json!([{"x": 0, "y": 0}]);
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn document_payload_rejects_unsupported_versions_and_oversized_node_lists() {
        let invalid_version = json!({
            "version": 1,
            "id": "design-1",
            "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}],
            "activePageId": "page-1",
            "nodes": [],
            "selectedNodeIds": [],
            "motionTracks": []
        });
        assert!(!valid_document_payload(&invalid_version, "design-1", None));

        let excessive_nodes = json!({
            "version": 7,
            "id": "design-1",
            "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}],
            "activePageId": "page-1",
            "nodes": vec![json!({}); 10_001],
            "selectedNodeIds": []
        });
        assert!(!valid_document_payload(&excessive_nodes, "design-1", None));
    }

    #[test]
    fn document_payload_requires_valid_frame_hierarchy_and_selection() {
        let frame = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Frame", "x": 0, "y": 0,
            "width": 320, "height": 640, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF"
        });
        let child = json!({
            "id": "shape-1", "pageId": "page-1", "type": "rectangle", "name": "Shape", "x": 4, "y": 4,
            "width": 80, "height": 48, "parentId": "frame-1", "rotation": 0,
            "visible": true, "locked": false, "fill": "#000000", "radius": 4
        });
        let mut document = json!({
            "version": 7,
            "id": "design-1",
            "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}],
            "activePageId": "page-1",
            "nodes": [frame, child],
            "selectedNodeIds": ["shape-1"],
            "motionTracks": []
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["autoLayout"] = json!({
            "direction": "vertical", "alignment": "stretch", "gap": 8, "padding": 12
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["autoLayout"]["gap"] = json!(4097);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["autoLayout"]["gap"] = json!(8);
        document["nodes"][1]["constraints"] = json!({
            "horizontal": "start-end", "vertical": "center"
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["constraints"]["vertical"] = json!("free");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["constraints"]["vertical"] = json!("center");

        document["nodes"][1]["stroke"] = json!("#336699");
        document["nodes"][1]["strokeWidth"] = json!(3);
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["strokeWidth"] = json!(4097);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["strokeWidth"] = json!(3);
        document["nodes"][1]["stroke"] = json!("red");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["stroke"] = json!("#336699");

        document["pages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "page-2", "name": "Page 2"}));
        document["nodes"][1]["pageId"] = json!("page-2");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["parentId"] = Value::Null;
        assert!(!valid_document_payload(&document, "design-1", None));
        document["activePageId"] = json!("page-2");
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["parentId"] = json!("frame-1");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][1]["pageId"] = json!("page-1");
        document["nodes"][1]["parentId"] = json!("frame-1");
        document["activePageId"] = json!("page-1");

        document["nodes"][1]["parentId"] = json!("missing-frame");
        assert!(!valid_document_payload(&document, "design-1", None));

        document["nodes"][1]["parentId"] = json!("frame-1");
        document["nodes"][0]["parentId"] = json!("shape-1");
        assert!(!valid_document_payload(&document, "design-1", None));

        document["nodes"][0]["parentId"] = Value::Null;
        document["nodes"][1]["parentId"] = json!("frame-1");
        document["selectedNodeIds"] = json!(["missing-node"]);
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn document_comments_must_reference_valid_pages_and_frames() {
        let frame = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Frame", "x": 0, "y": 0,
            "width": 320, "height": 640, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF"
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [frame], "selectedNodeIds": [], "motionTracks": [],
            "comments": [{"id": "comment-1", "pageId": "page-1", "nodeId": "frame-1", "text": "Review spacing", "point": {"x": 0.5, "y": 0.25}, "createdAt": 123, "resolved": false}]
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["comments"][0]["nodeId"] = json!("missing-frame");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["comments"][0]["nodeId"] = json!("frame-1");
        document["comments"][0]["point"]["x"] = json!(2.0);
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn prototype_links_must_target_frames_on_the_source_page() {
        let start = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Start", "x": 0, "y": 0,
            "width": 320, "height": 640, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF"
        });
        let target = json!({
            "id": "frame-2", "pageId": "page-1", "type": "frame", "name": "Target", "x": 400, "y": 0,
            "width": 320, "height": 640, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF"
        });
        let button = json!({
            "id": "button-1", "pageId": "page-1", "type": "rectangle", "name": "Continue", "x": 20, "y": 20,
            "width": 80, "height": 32, "parentId": "frame-1", "rotation": 0,
            "visible": true, "locked": false, "fill": "#000000", "radius": 0
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [start, target, button], "selectedNodeIds": [], "motionTracks": [],
            "prototypeFlows": [{"id": "flow-1", "name": "Checkout", "pageId": "page-1", "startNodeId": "frame-1"}],
            "prototypeInteractions": [{"id": "link-1", "sourceNodeId": "button-1", "targetNodeId": "frame-2", "trigger": "click", "transition": "dissolve", "durationMs": 240}]
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["trigger"] = json!("hover");
        assert!(valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["presentation"] = json!("overlay");
        assert!(valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["presentation"] = json!("unsupported");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["presentation"] = json!("navigate");
        document["prototypeInteractions"][0]["trigger"] = json!("unsupported");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["trigger"] = json!("click");
        document["prototypeInteractions"][0]["targetNodeId"] = json!("button-1");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["prototypeInteractions"][0]["targetNodeId"] = json!("frame-2");
        document["nodes"][0]["scrollable"] = json!("yes");
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn component_variants_must_reference_their_frame_roots() {
        let frame = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Button", "x": 0, "y": 0,
            "width": 120, "height": 48, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF",
            "componentRef": {"componentId": "component-1", "sourceNodeId": "frame-1", "variantId": "variant-1", "role": "master"}
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [frame], "selectedNodeIds": [], "motionTracks": [],
            "components": [{"id": "component-1", "name": "Button", "variants": [{"id": "variant-1", "name": "Default", "rootNodeId": "frame-1"}]}]
        });
        assert!(valid_document_payload(&document, "design-1", None));
        document["components"][0]["variants"][0]["rootNodeId"] = json!("missing-frame");
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn motion_tracks_require_bounded_ordered_keyframes_for_existing_nodes() {
        let frame = json!({
            "id": "frame-1", "pageId": "page-1", "type": "frame", "name": "Screen", "x": 0, "y": 0,
            "width": 320, "height": 640, "parentId": null, "rotation": 0,
            "visible": true, "locked": false, "fill": "#FFFFFF"
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [frame], "selectedNodeIds": [],
            "motionTracks": [{
                "id": "track-1", "nodeId": "frame-1", "property": "opacity",
                "keyframes": [
                    {"id": "key-1", "timeMs": 0, "value": 0.0, "easing": "ease-in"},
                    {"id": "key-2", "timeMs": 600, "value": 1.0, "easing": "ease-out"}
                ]
            }]
        });

        assert!(valid_document_payload(&document, "design-1", None));
        document["motionTracks"][0]["nodeId"] = json!("missing-node");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["motionTracks"][0]["nodeId"] = json!("frame-1");
        document["motionTracks"][0]["keyframes"][1]["timeMs"] = json!(0);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["motionTracks"][0]["keyframes"][1]["timeMs"] = json!(600);
        document["motionTracks"][0]["keyframes"][1]["value"] = json!(1.5);
        assert!(!valid_document_payload(&document, "design-1", None));
    }

    #[test]
    fn document_payload_validates_bounded_image_asset_references() {
        let image = json!({
            "id": "image-node", "pageId": "page-1", "type": "image", "name": "Photo", "x": 0, "y": 0,
            "width": 320, "height": 220, "parentId": null, "rotation": 0, "visible": true, "locked": false,
            "assetId": "0123456789abcdef0123456789abcdef", "mimeType": "image/png", "sizeBytes": 2048,
            "fit": "cover", "altText": "Product photo"
        });
        let mut document = json!({
            "version": 7, "id": "design-1", "projectId": null,
            "pages": [{"id": "page-1", "name": "Page 1"}], "activePageId": "page-1",
            "nodes": [image], "selectedNodeIds": [], "motionTracks": []
        });
        assert!(valid_document_payload(&document, "design-1", None));

        document["nodes"][0]["effects"] = json!([{
            "type": "shadow", "color": "#112233", "offsetX": 2, "offsetY": 4, "blur": 12
        }]);
        assert!(valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["effects"][0]["blur"] = json!(4097);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]
            .as_object_mut()
            .unwrap()
            .remove("effects");

        document["nodes"][0]["mimeType"] = json!("image/svg+xml");
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["mimeType"] = json!("image/png");
        document["nodes"][0]["sizeBytes"] = json!(33_554_433);
        assert!(!valid_document_payload(&document, "design-1", None));
        document["nodes"][0]["sizeBytes"] = json!(2048);
        document["nodes"][0]["assetId"] = json!("INVALID-ASSET-ID");
        assert!(!valid_document_payload(&document, "design-1", None));
    }
}
