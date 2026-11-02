use super::*;

fn has_only(input: &Map<String, Value>, allowed: &[&str]) -> bool {
    input.keys().all(|key| allowed.contains(&key.as_str()))
}

pub(super) fn list(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    if !has_only(input, &["conversationId"]) {
        return Err(invalid_request_params());
    }
    let conversation_id = required_safe_id(input, "conversationId")?;
    Ok(json!({"todos": crate::todos::list(storage, conversation_id)?}))
}

pub(super) fn create(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    if !has_only(input, &["conversationId", "items"]) {
        return Err(invalid_request_params());
    }
    let conversation_id = required_safe_id(input, "conversationId")?;
    let items = input
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(invalid_request_params)?;
    Ok(json!({"todos": crate::todos::create_many(storage, conversation_id, items)?}))
}

pub(super) fn update(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    if !has_only(
        input,
        &["conversationId", "todoId", "title", "description", "status"],
    ) {
        return Err(invalid_request_params());
    }
    let conversation_id = required_safe_id(input, "conversationId")?;
    let todo_id = required_safe_id(input, "todoId")?;
    let title = input
        .get("title")
        .map(|value| value.as_str().ok_or_else(invalid_request_params))
        .transpose()?;
    let description = input
        .get("description")
        .map(|value| match value {
            Value::Null => Ok(None),
            Value::String(text) => Ok(Some(text.as_str())),
            _ => Err(invalid_request_params()),
        })
        .transpose()?;
    let status = input
        .get("status")
        .map(|value| value.as_str().ok_or_else(invalid_request_params))
        .transpose()?;
    Ok(
        json!({"todo": crate::todos::update(storage, conversation_id, todo_id, title, description, status)?}),
    )
}

pub(super) fn delete(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    if !has_only(input, &["conversationId", "todoId"]) {
        return Err(invalid_request_params());
    }
    let conversation_id = required_safe_id(input, "conversationId")?;
    let todo_id = required_safe_id(input, "todoId")?;
    crate::todos::delete(storage, conversation_id, todo_id)?;
    Ok(json!({"deleted": true}))
}

pub(super) fn clear(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    if !has_only(input, &["conversationId"]) {
        return Err(invalid_request_params());
    }
    let conversation_id = required_safe_id(input, "conversationId")?;
    Ok(json!({"deletedCount": crate::todos::clear(storage, conversation_id)?}))
}
