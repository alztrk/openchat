//! Calls provider-owned compaction endpoints that do not compact automatically.

use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::watch;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, sqlite_error_code},
    usage_statistics::{UsageData, UsageRequestDetails, UsageRequestTracker},
};

use super::route::{ChatRoute, NativeCompaction};
use super::{
    cancelled_error, client, http_error, invalid_response_error, network_error_with_details,
};

const MAX_COMPACTION_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) async fn compact_xai_history(
    storage: &AppStorage,
    route: &ChatRoute,
    api_key: &str,
    conversation_id: &str,
    assistant_message_id: &str,
    run_id: &str,
    body: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if route.native_compaction != Some(NativeCompaction::XaiResponsesExplicit)
        || route.provider_id.as_deref() != Some("xai")
    {
        return Err(invalid_response_error());
    }
    let compact_url = route
        .chat_url
        .strip_suffix("/responses")
        .map(|base| format!("{base}/responses/compact"))
        .ok_or_else(invalid_response_error)?;
    let mut tracker = UsageRequestTracker::start(
        storage,
        UsageRequestDetails {
            conversation_id,
            assistant_message_id: Some(assistant_message_id),
            provider_id: "xai",
            model_id: &route.model_id,
            reasoning_effort: None,
            operation: "compaction",
            fast_requested: false,
            run_id: Some(run_id),
            connection_id: route.connection_id.as_deref(),
            workspace_id: None,
            project_id: route.project_id.as_deref(),
            parent_run_id: None,
        },
    )
    .map_err(|error| {
        super::log_send_storage_failure(
            storage,
            "chat.send.compatible_compaction_usage_start",
            sqlite_error_code(&error),
        );
        super::storage_error()
    })?;
    tracker
        .record_request_manifest(body, None)
        .map_err(|error| {
            super::log_send_storage_failure(
                storage,
                "chat.send.compatible_compaction_usage_manifest",
                sqlite_error_code(&error),
            );
            super::storage_error()
        })?;

    let result = compact_xai_request(&compact_url, api_key, body, cancellation).await;
    match result {
        Ok((item, usage)) => {
            tracker.complete(&usage).map_err(|error| {
                super::log_send_storage_failure(
                    storage,
                    "chat.send.compatible_compaction_usage_complete",
                    sqlite_error_code(&error),
                );
                super::storage_error()
            })?;
            Ok(item)
        }
        Err(error) if is_cancelled(&error) => {
            tracker.cancel().map_err(|storage_error| {
                super::log_send_storage_failure(
                    storage,
                    "chat.send.compatible_compaction_usage_cancel",
                    sqlite_error_code(&storage_error),
                );
                super::storage_error()
            })?;
            Err(error)
        }
        Err(error) => {
            tracker
                .fail(&UsageData::default())
                .map_err(|storage_error| {
                    super::log_send_storage_failure(
                        storage,
                        "chat.send.compatible_compaction_usage_fail",
                        sqlite_error_code(&storage_error),
                    );
                    super::storage_error()
                })?;
            Err(error)
        }
    }
}

async fn compact_xai_request(
    url: &str,
    api_key: &str,
    body: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(Value, UsageData), ServiceError> {
    let response = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        response = client()?
            .post(url)
            .bearer_auth(api_key)
            .json(body)
            .send() => response.map_err(network_error_with_details)?,
    };
    if !response.status().is_success() {
        return Err(http_error(response.status(), Some("xai")));
    }

    let mut stream = response.bytes_stream();
    let mut encoded = Vec::new();
    loop {
        let next = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(network_error_with_details)?;
        if encoded.len().saturating_add(chunk.len()) > MAX_COMPACTION_RESPONSE_BYTES {
            return Err(invalid_response_error());
        }
        encoded.extend_from_slice(&chunk);
    }

    let response: Value = serde_json::from_slice(&encoded).map_err(|_| invalid_response_error())?;
    parse_xai_compaction_response(&response)
}

fn parse_xai_compaction_response(response: &Value) -> Result<(Value, UsageData), ServiceError> {
    let usage = UsageData::from_responses_event(response, "xai");
    let outputs = response
        .get("output")
        .and_then(Value::as_array)
        .filter(|items| items.len() == 1)
        .ok_or_else(invalid_response_error)?;
    let item = outputs.first().ok_or_else(invalid_response_error)?;
    let valid_id = item
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        .is_some();
    let encrypted_content = item
        .get("encrypted_content")
        .and_then(Value::as_str)
        .filter(|content| !content.is_empty());
    if item.get("type").and_then(Value::as_str) != Some("compaction")
        || !valid_id
        || encrypted_content.is_none_or(|content| content.len() > MAX_COMPACTION_RESPONSE_BYTES)
    {
        return Err(invalid_response_error());
    }

    // Keep the complete opaque provider item unchanged for the next Responses request.
    Ok((item.clone(), usage))
}

fn is_cancelled(error: &ServiceError) -> bool {
    matches!(error.code, "operation_cancelled" | "request_cancelled")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{MAX_COMPACTION_RESPONSE_BYTES, parse_xai_compaction_response};

    #[test]
    fn compact_response_requires_an_opaque_compaction_item() {
        let valid = json!({
            "output": [{
                "type": "compaction",
                "id": "cmp_123",
                "encrypted_content": "opaque"
            }]
        });
        let (item, _) = parse_xai_compaction_response(&valid).expect("parse compact response");
        assert_eq!(item["type"], "compaction");
        assert_eq!(item["encrypted_content"], "opaque");
        assert_eq!(MAX_COMPACTION_RESPONSE_BYTES, 1024 * 1024);
    }

    #[test]
    fn rejects_malformed_or_multiple_xai_compaction_items() {
        assert!(
            parse_xai_compaction_response(&json!({
                "output": [{
                    "type": "compaction",
                    "id": "cmp_123",
                    "encrypted_content": ""
                }]
            }))
            .is_err()
        );
        assert!(
            parse_xai_compaction_response(&json!({
                "output": [
                    {"type": "compaction", "id": "cmp_1", "encrypted_content": "one"},
                    {"type": "compaction", "id": "cmp_2", "encrypted_content": "two"}
                ]
            }))
            .is_err()
        );
    }
}
