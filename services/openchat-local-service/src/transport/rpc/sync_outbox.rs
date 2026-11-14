//! Local RPC boundary for the durable encrypted sync outbox.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{domain::sync_outbox, protocol::ServiceError, storage::AppStorage};

const MAXIMUM_ENVELOPE_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "sync.outbox.enqueue" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let event_id = required_uuid(params, "eventId")?;
            let encoded = required_string(params, "envelopeBase64")?;
            let maximum_encoded_bytes = MAXIMUM_ENVELOPE_BYTES.div_ceil(3) * 4;
            if encoded.is_empty() || encoded.len() > maximum_encoded_bytes {
                return Err(invalid_request());
            }
            let envelope = STANDARD.decode(encoded).map_err(|_| invalid_request())?;
            let result = sync_outbox::enqueue(storage, account_id, device_id, event_id, &envelope)?;
            Ok(json!({ "result": result }))
        }
        "sync.outbox.claim" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let limit = optional_u64(params, "limit", 10)?;
            let items = sync_outbox::claim_due(storage, account_id, device_id, limit)?;
            Ok(json!({ "items": items }))
        }
        "sync.outbox.retry" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let event_id = required_uuid(params, "eventId")?;
            let lease_id = required_uuid(params, "leaseId")?;
            sync_outbox::record_retry(storage, account_id, device_id, event_id, lease_id)?;
            Ok(json!({ "recorded": true }))
        }
        "sync.outbox.requeue" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let event_id = required_uuid(params, "eventId")?;
            let requeued =
                sync_outbox::requeue_exhausted(storage, account_id, device_id, event_id)?;
            Ok(json!({ "requeued": requeued }))
        }
        "sync.outbox.acknowledge" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let event_id = required_uuid(params, "eventId")?;
            let lease_id = required_uuid(params, "leaseId")?;
            let server_cursor = required_u64(params, "serverCursor")?;
            sync_outbox::acknowledge(
                storage,
                account_id,
                device_id,
                event_id,
                lease_id,
                server_cursor,
            )?;
            Ok(json!({ "acknowledged": true }))
        }
        "sync.outbox.cursor" => {
            let account_id = required_uuid(params, "accountId")?;
            let device_id = required_uuid(params, "deviceId")?;
            let cursor = sync_outbox::server_cursor(storage, account_id, device_id)?;
            Ok(json!({ "serverCursor": cursor }))
        }
        _ => Err(invalid_request()),
    }
}

fn required_uuid(params: &Value, key: &str) -> Result<Uuid, ServiceError> {
    Uuid::parse_str(required_string(params, key)?).map_err(|_| invalid_request())
}

fn required_string<'a>(params: &'a Value, key: &str) -> Result<&'a str, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)
}

fn required_u64(params: &Value, key: &str) -> Result<u64, ServiceError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)
}

fn optional_u64(params: &Value, key: &str, default: u64) -> Result<u64, ServiceError> {
    match params.get(key) {
        None => Ok(default),
        Some(value) => value.as_u64().map_or_else(|| Err(invalid_request()), Ok),
    }
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The sync outbox request is invalid.",
        false,
    )
}
