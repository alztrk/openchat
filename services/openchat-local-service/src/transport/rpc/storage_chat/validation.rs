//! Request parameter validation and metadata sanitization helpers.

use super::*;

pub(super) fn input_object(params: &Value) -> Result<&Map<String, Value>, ServiceError> {
    params
        .get("input")
        .unwrap_or(params)
        .as_object()
        .ok_or_else(invalid_request_params)
}

pub(super) fn required_bounded_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    max_bytes: usize,
    non_empty: bool,
) -> Result<&'a str, ServiceError> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    if value.len() > max_bytes
        || value.chars().any(char::is_control)
        || (non_empty && value.trim().is_empty())
    {
        return Err(invalid_request_params());
    }
    Ok(value.trim())
}

pub(super) fn optional_bounded_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    max_bytes: usize,
    non_empty: bool,
) -> Result<Option<&'a str>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(_)) => {
            required_bounded_string(object, name, max_bytes, non_empty).map(Some)
        }
        _ => Err(invalid_request_params()),
    }
}

pub(super) fn required_safe_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)?;
    if !is_safe_identifier(value) {
        return Err(invalid_request_params());
    }
    Ok(value)
}

pub(super) fn optional_safe_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)? else {
        return Ok(None);
    };
    if !is_safe_identifier(value) {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

pub(super) fn required_title<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let title = required_bounded_string(object, name, MAX_TITLE_CHARS * 4, true)?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid_request_params());
    }
    Ok(title)
}

pub(super) fn required_model_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, MAX_MODEL_ID_CHARS * 4, true)?;
    if value.chars().count() > MAX_MODEL_ID_CHARS {
        return Err(invalid_request_params());
    }
    Ok(value)
}

pub(super) fn optional_model_id<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_MODEL_ID_CHARS * 4, true)? else {
        return Ok(None);
    };
    if value.chars().count() > MAX_MODEL_ID_CHARS {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

pub(super) fn required_id_array(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Vec<String>, ServiceError> {
    let values = object
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(invalid_request_params)?;
    if values.is_empty() || values.len() > MAX_CONVERSATION_IDS {
        return Err(invalid_request_params());
    }
    let mut ids = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let id = value.as_str().ok_or_else(invalid_request_params)?;
        if !is_safe_identifier(id) || !seen.insert(id.to_owned()) {
            return Err(invalid_request_params());
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}

pub(super) fn optional_json_object<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(_)) => Ok(object.get(name)),
        _ => Err(invalid_request_params()),
    }
}

pub(super) fn optional_json_array(
    object: &Map<String, Value>,
    name: &str,
    remaining_nodes: &mut usize,
) -> Result<String, ServiceError> {
    let mut value = match object.get(name) {
        None | Some(Value::Null) => return Ok("[]".to_owned()),
        Some(Value::Array(value)) => {
            let node_count = value.iter().fold(1usize, |count, item| {
                count.saturating_add(json_value_node_count(item))
            });
            if node_count > *remaining_nodes {
                return Err(invalid_request_params());
            }
            *remaining_nodes -= node_count;
            Value::Array(value.clone())
        }
        Some(Value::String(value)) => {
            if value.len() > MAX_MESSAGE_METADATA_BYTES {
                return Err(invalid_request_params());
            }
            let (parsed, node_count) = parse_bounded_json_value(value, *remaining_nodes)
                .map_err(|_| invalid_request_params())?
                .ok_or_else(invalid_request_params)?;
            *remaining_nodes -= node_count;
            if !parsed.is_array() {
                return Err(invalid_request_params());
            }
            parsed
        }
        _ => return Err(invalid_request_params()),
    };
    clear_sensitive_json_values(&mut value);
    if name == "toolActivities" {
        sanitize_tool_activity_metadata(&mut value, remaining_nodes, 0)?;
    }
    let encoded =
        Zeroizing::new(serde_json::to_string(&value).map_err(|_| invalid_request_params())?);
    if encoded.len() > MAX_MESSAGE_METADATA_BYTES {
        return Err(invalid_request_params());
    }
    Ok(encoded.to_string())
}

pub(super) fn serialize_json_without_sensitive_values(
    value: &Value,
) -> Result<String, ServiceError> {
    let mut sanitized = value.clone();
    clear_sensitive_json_values(&mut sanitized);
    let encoded =
        Zeroizing::new(serde_json::to_string(&sanitized).map_err(|_| invalid_request_params())?);
    if encoded.len() > MAX_AGENT_METADATA_BYTES {
        return Err(invalid_request_params());
    }
    Ok(encoded.to_string())
}

pub(super) fn json_value_node_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.iter().fold(1usize, |count, item| {
            count.saturating_add(json_value_node_count(item))
        }),
        Value::Object(fields) => fields.values().fold(1usize, |count, value| {
            count.saturating_add(json_value_node_count(value))
        }),
        _ => 1,
    }
}

pub(super) fn sanitize_tool_activity_metadata(
    value: &mut Value,
    remaining_nodes: &mut usize,
    encoded_json_depth: usize,
) -> Result<bool, ServiceError> {
    match value {
        Value::Array(items) => {
            let mut sanitized = false;
            for item in items {
                sanitized |=
                    sanitize_tool_activity_metadata(item, remaining_nodes, encoded_json_depth)?;
            }
            Ok(sanitized)
        }
        Value::Object(fields) => {
            let mut sanitized = false;
            for (name, field_value) in fields {
                if matches!(name.as_str(), "arguments" | "output") {
                    sanitized |= sanitize_tool_activity_payload(
                        field_value,
                        remaining_nodes,
                        encoded_json_depth,
                    )?;
                } else {
                    sanitized |= sanitize_tool_activity_metadata(
                        field_value,
                        remaining_nodes,
                        encoded_json_depth,
                    )?;
                }
            }
            Ok(sanitized)
        }
        _ => Ok(false),
    }
}

pub(super) fn sanitize_tool_activity_payload(
    value: &mut Value,
    remaining_nodes: &mut usize,
    encoded_json_depth: usize,
) -> Result<bool, ServiceError> {
    match value {
        Value::String(text) => {
            sanitize_tool_activity_json_text(text, remaining_nodes, encoded_json_depth)
        }
        Value::Array(_) | Value::Object(_) => {
            sanitize_tool_activity_metadata(value, remaining_nodes, encoded_json_depth)
        }
        _ => Ok(false),
    }
}

pub(super) fn sanitize_tool_activity_json_text(
    text: &mut String,
    remaining_nodes: &mut usize,
    encoded_json_depth: usize,
) -> Result<bool, ServiceError> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') && !trimmed.starts_with('"') {
        return Ok(false);
    }
    if encoded_json_depth >= MAX_EMBEDDED_METADATA_JSON_DEPTH
        || *remaining_nodes == 0
        || text.len() > MAX_EMBEDDED_METADATA_JSON_BYTES
        || (json_container_nesting_reaches_limit(text) && is_valid_json_value(text))
    {
        return Err(invalid_request_params());
    }
    let (mut parsed, node_count) = match parse_bounded_json_value(text, *remaining_nodes) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return Err(invalid_request_params()),
        // Strings that resemble JSON but are not valid JSON remain ordinary tool text.
        Err(_) => return Ok(false),
    };
    *remaining_nodes -= node_count;
    let mut sanitized = clear_sensitive_json_values(&mut parsed);
    match &mut parsed {
        Value::String(nested) => {
            sanitized |=
                sanitize_tool_activity_json_text(nested, remaining_nodes, encoded_json_depth + 1)?;
        }
        Value::Array(_) | Value::Object(_) => {
            sanitized |= sanitize_tool_activity_metadata(
                &mut parsed,
                remaining_nodes,
                encoded_json_depth + 1,
            )?;
        }
        _ => {}
    }
    if sanitized {
        let encoded = serde_json::to_string(&parsed).map_err(|_| invalid_request_params())?;
        text.zeroize();
        *text = encoded;
    }
    Ok(sanitized)
}

pub(super) fn json_container_nesting_reaches_limit(text: &str) -> bool {
    let mut nesting = 0usize;
    let mut inside_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if inside_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                inside_string = false;
            }
            continue;
        }
        match byte {
            b'"' => inside_string = true,
            b'{' | b'[' => {
                nesting += 1;
                if nesting >= MAX_STRUCTURED_METADATA_JSON_NESTING {
                    return true;
                }
            }
            b'}' | b']' => nesting = nesting.saturating_sub(1),
            _ => {}
        }
    }
    false
}

pub(super) fn is_valid_json_value(text: &str) -> bool {
    serde_json::from_str::<serde::de::IgnoredAny>(text).is_ok()
}

pub(super) fn parse_stored_agent_json(raw: String) -> rusqlite::Result<Value> {
    let conversion_error = || {
        rusqlite::Error::FromSqlConversionFailure(
            13,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                "stored agent metadata is invalid",
            )),
        )
    };
    if raw.len() > MAX_AGENT_METADATA_BYTES {
        return Err(conversion_error());
    }
    let mut agent = serde_json::from_str::<Value>(&raw).map_err(|_| conversion_error())?;
    clear_sensitive_json_values(&mut agent);
    Ok(agent)
}

pub(super) fn sanitize_stored_metadata(
    raw: String,
    column_index: usize,
    tool_activities: bool,
    remaining_nodes: &mut usize,
) -> rusqlite::Result<String> {
    let conversion_error = || {
        rusqlite::Error::FromSqlConversionFailure(
            column_index,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                "stored message metadata is invalid",
            )),
        )
    };
    if raw.len() > MAX_MESSAGE_METADATA_BYTES {
        return Err(conversion_error());
    }
    if !raw.contains("\\u")
        && !SENSITIVE_METADATA_KEY_MARKERS
            .iter()
            .any(|marker| raw.contains(marker))
    {
        return Ok(raw);
    }
    let (mut value, node_count) = parse_bounded_json_value(&raw, *remaining_nodes)
        .map_err(|_| conversion_error())?
        .ok_or_else(conversion_error)?;
    if !value.is_array() {
        return Err(conversion_error());
    }
    *remaining_nodes -= node_count;
    let mut sanitized = clear_sensitive_json_values(&mut value);
    if tool_activities {
        sanitized |= sanitize_tool_activity_metadata(&mut value, remaining_nodes, 0)
            .map_err(|_| conversion_error())?;
    }
    if !sanitized {
        return Ok(raw);
    }
    let encoded = serde_json::to_string(&value).map_err(|_| conversion_error())?;
    if encoded.len() > MAX_MESSAGE_METADATA_BYTES {
        return Err(conversion_error());
    }
    Ok(encoded)
}

pub(super) fn required_content(
    object: &Map<String, Value>,
    name: &str,
) -> Result<String, ServiceError> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request_params)?;
    if value.len() > MAX_MESSAGE_CONTENT_BYTES {
        return Err(invalid_request_params());
    }
    Ok(value.to_owned())
}

pub(super) fn required_message_role<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, 16, true)?;
    if !matches!(value, "user" | "assistant") {
        return Err(invalid_request_params());
    }
    Ok(value)
}

pub(super) fn required_message_status<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, ServiceError> {
    let value = required_bounded_string(object, name, 16, true)?;
    if !matches!(value, "streaming" | "completed" | "failed" | "stopped") {
        return Err(invalid_request_params());
    }
    Ok(value)
}

pub(super) fn optional_failure_code<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, MAX_IDENTIFIER_BYTES, true)? else {
        return Ok(None);
    };
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    }) {
        return Err(invalid_request_params());
    }
    Ok(Some(value))
}

pub(super) fn optional_nonnegative_i64(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<i64>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .filter(|value| *value >= 0)
            .map(Some)
            .ok_or_else(invalid_request_params),
        _ => Err(invalid_request_params()),
    }
}

pub(super) fn optional_bool(
    object: &Map<String, Value>,
    name: &str,
    default: bool,
) -> Result<bool, ServiceError> {
    match object.get(name) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(invalid_request_params()),
    }
}

pub(super) fn optional_timestamp(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<i64>, ServiceError> {
    optional_nonnegative_i64(object, name)
}

pub(super) fn optional_filter_string(
    object: &Map<String, Value>,
    name: &str,
    max_bytes: usize,
) -> Result<Option<String>, ServiceError> {
    let Some(value) = optional_bounded_string(object, name, max_bytes, true)? else {
        return Ok(None);
    };
    Ok(Some(value.to_owned()))
}

pub(super) fn is_safe_attachment_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn optional_nonnegative_f64(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<f64>, ServiceError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(Some)
            .ok_or_else(invalid_request_params),
        _ => Err(invalid_request_params()),
    }
}

pub(super) fn normalize_search_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            'İ' | 'I' | 'ı' => result.push('i'),
            'Ç' | 'ç' => result.push('c'),
            'Ş' | 'ş' => result.push('s'),
            'Ğ' | 'ğ' => result.push('g'),
            'Ü' | 'ü' => result.push('u'),
            'Ö' | 'ö' => result.push('o'),
            other => result.extend(other.to_lowercase()),
        }
    }
    result
}

pub(super) fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    let right = right
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    left == right
        || right
            .strip_prefix(&left)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || left
            .strip_prefix(&right)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
