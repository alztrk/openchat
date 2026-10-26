//! Routes usage statistics and model pricing requests.

use serde_json::Value;
use tokio::sync::watch;

use crate::{
    protocol::ServiceError,
    storage::AppStorage,
    usage_statistics::{self, StatisticsQuery},
};

use super::{
    bounded_route_id, invalid_request_params, optional_bool, optional_string, required_string,
};

const MAX_STATISTICS_PAGE_SIZE: i64 = 200;
const MAX_STATISTICS_EXPORT_PAGE_SIZE: i64 = 1000;

/// Dispatches usage and pricing requests while preserving their bounded query contracts.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "statistics.get" => {
            let query = statistics_query(params, MAX_STATISTICS_PAGE_SIZE)?;
            usage_statistics::get_statistics(storage, &query, cancellation).await
        }
        "statistics.details" => {
            let query = statistics_query(params, MAX_STATISTICS_EXPORT_PAGE_SIZE)?;
            let include_total = optional_bool(params, "includeTotal")?.unwrap_or(true);
            usage_statistics::get_statistics_details(storage, &query, include_total)
        }
        "models.pricing.get" => {
            let provider_id = required_string(params, "providerId")?;
            let model_id = required_string(params, "modelId")?;
            usage_statistics::get_model_details(storage, provider_id, model_id, cancellation).await
        }
        "models.pricing.get_many" => {
            let requested_models = params
                .get("models")
                .and_then(Value::as_array)
                .filter(|models| !models.is_empty() && models.len() <= 1000)
                .ok_or_else(invalid_request_params)?;
            let models = requested_models
                .iter()
                .map(|model| {
                    let provider_id = bounded_route_id(model, "providerId")?;
                    let model_id = bounded_route_id(model, "modelId")?;
                    Ok((provider_id.to_owned(), model_id.to_owned()))
                })
                .collect::<Result<Vec<_>, ServiceError>>()?;
            usage_statistics::get_model_details_batch(storage, &models, cancellation).await
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

/// Validates date ranges, filters, and pagination before querying usage records.
fn statistics_query(params: &Value, max_limit: i64) -> Result<StatisticsQuery, ServiceError> {
    let from_unix_ms = params
        .get("fromUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(invalid_request_params)?;
    let to_unix_ms = params
        .get("toUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value > from_unix_ms)
        .ok_or_else(invalid_request_params)?;
    let provider_id = optional_statistics_filter(params, "providerId")?;
    let model_id = optional_statistics_filter(params, "modelId")?;
    let operation = optional_statistics_filter(params, "operation")?;
    let reasoning_effort = optional_statistics_filter(params, "reasoningEffort")?;
    if operation.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "chat" | "tool_follow_up" | "compaction" | "title_generation"
        )
    }) || reasoning_effort.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "none"
                | "minimal"
                | "low"
                | "medium"
                | "high"
                | "xhigh"
                | "max"
                | "ultra"
                | "unspecified"
        )
    }) {
        return Err(invalid_request_params());
    }
    let fast_mode = optional_bool(params, "fastMode")?;
    let offset = params.get("offset").and_then(Value::as_i64).unwrap_or(0);
    let limit = params.get("limit").and_then(Value::as_i64).unwrap_or(100);
    if !(0..=100_000_000).contains(&offset) || !(1..=max_limit).contains(&limit) {
        return Err(invalid_request_params());
    }
    Ok(StatisticsQuery {
        from_unix_ms,
        to_unix_ms,
        provider_id,
        model_id,
        operation,
        reasoning_effort,
        fast_mode,
        offset,
        limit,
    })
}

/// Accepts an optional bounded filter only when it has no edge whitespace or controls.
fn optional_statistics_filter(params: &Value, name: &str) -> Result<Option<String>, ServiceError> {
    optional_string(params, name)?
        .map(|value| {
            if value == value.trim()
                && value.chars().count() <= 256
                && !value.chars().any(char::is_control)
            {
                Ok(value.to_owned())
            } else {
                Err(invalid_request_params())
            }
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{MAX_STATISTICS_PAGE_SIZE, dispatch, statistics_query};

    #[test]
    fn statistics_query_keeps_range_filter_and_page_limits() {
        let valid_query = json!({
            "fromUnixMs": 10,
            "toUnixMs": 20,
            "providerId": "openai",
            "modelId": "gpt-test",
            "operation": "chat",
            "reasoningEffort": "high",
            "fastMode": true,
            "offset": 5,
            "limit": 25
        });
        let parsed = statistics_query(&valid_query, MAX_STATISTICS_PAGE_SIZE)
            .expect("valid statistics query");
        assert_eq!(parsed.from_unix_ms, 10);
        assert_eq!(parsed.to_unix_ms, 20);
        assert_eq!(parsed.provider_id.as_deref(), Some("openai"));
        assert_eq!(parsed.operation.as_deref(), Some("chat"));
        assert_eq!(parsed.offset, 5);
        assert_eq!(parsed.limit, 25);

        for override_values in [
            json!({"fromUnixMs": -1}),
            json!({"toUnixMs": 10}),
            json!({"operation": "unknown"}),
            json!({"reasoningEffort": "unknown"}),
            json!({"providerId": " openai"}),
            json!({"providerId": "x".repeat(257)}),
            json!({"fastMode": "true"}),
            json!({"offset": -1}),
            json!({"offset": 100_000_001}),
            json!({"limit": 0}),
            json!({"limit": 201}),
        ] {
            let mut params = valid_query.clone();
            for (key, value) in override_values.as_object().expect("test override object") {
                params[key] = value.clone();
            }
            assert!(
                statistics_query(&params, MAX_STATISTICS_PAGE_SIZE).is_err(),
                "accepted invalid query: {params}"
            );
        }
    }

    #[tokio::test]
    async fn statistics_dispatch_keeps_invalid_and_unknown_method_errors() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        let (_cancel_sender, mut cancellation) = watch::channel(false);

        let invalid_query = dispatch("statistics.get", &json!({}), &storage, &mut cancellation)
            .await
            .expect_err("statistics date ranges are required");
        assert_eq!(invalid_query.code, "invalid_request_params");

        let invalid_batch = dispatch(
            "models.pricing.get_many",
            &json!({"models": []}),
            &storage,
            &mut cancellation,
        )
        .await
        .expect_err("pricing batches cannot be empty");
        assert_eq!(invalid_batch.code, "invalid_request_params");

        for method in ["statistics.unknown", "models.pricing.unknown"] {
            let error = dispatch(method, &Value::Null, &storage, &mut cancellation)
                .await
                .expect_err("unknown statistics methods are rejected");
            assert_eq!(error.code, "method_not_found", "method: {method}");
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-statistics-rpc-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
