mod models_dev_pricing;

use rusqlite::{params, params_from_iter};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{protocol::ServiceError, storage::AppStorage};
use tokio::sync::watch;

#[derive(Clone, Debug, Default)]
pub(crate) struct UsageData {
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) reasoning_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) cache_write_tokens: Option<i64>,
    pub(crate) reported_cost_usd: Option<f64>,
    pub(crate) service_tier: Option<String>,
}

impl UsageData {
    pub(crate) fn from_responses_event(value: &Value, provider_id: &str) -> Self {
        let response = value.get("response").unwrap_or(value);
        let usage = response.get("usage");
        Self {
            input_tokens: nonnegative_token(usage.and_then(|usage| usage.get("input_tokens"))),
            output_tokens: nonnegative_token(usage.and_then(|usage| usage.get("output_tokens"))),
            reasoning_tokens: nonnegative_token(
                usage.and_then(|usage| usage.pointer("/output_tokens_details/reasoning_tokens")),
            ),
            cached_input_tokens: nonnegative_token(
                usage.and_then(|usage| usage.pointer("/input_tokens_details/cached_tokens")),
            ),
            cache_write_tokens: nonnegative_token(
                usage.and_then(|usage| usage.pointer("/input_tokens_details/cache_write_tokens")),
            ),
            reported_cost_usd: reported_cost(usage, provider_id),
            service_tier: response
                .get("service_tier")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }

    pub(crate) fn from_chat_completion_event(value: &Value, provider_id: &str) -> Self {
        let usage = value.get("usage");
        Self {
            input_tokens: nonnegative_token(usage.and_then(|usage| usage.get("prompt_tokens"))),
            output_tokens: nonnegative_token(
                usage.and_then(|usage| usage.get("completion_tokens")),
            ),
            reasoning_tokens: nonnegative_token(
                usage
                    .and_then(|usage| usage.pointer("/completion_tokens_details/reasoning_tokens")),
            ),
            cached_input_tokens: nonnegative_token(
                usage.and_then(|usage| usage.pointer("/prompt_tokens_details/cached_tokens")),
            ),
            cache_write_tokens: nonnegative_token(usage.and_then(|usage| {
                usage
                    .pointer("/prompt_tokens_details/cache_write_tokens")
                    .or_else(|| usage.get("cache_creation_input_tokens"))
            })),
            reported_cost_usd: reported_cost(usage, provider_id),
            service_tier: value
                .get("service_tier")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }

    fn has_reported_usage(&self) -> bool {
        self.input_tokens.is_some()
            || self.output_tokens.is_some()
            || self.reasoning_tokens.is_some()
            || self.cached_input_tokens.is_some()
            || self.cache_write_tokens.is_some()
            || self.reported_cost_usd.is_some()
    }
}

fn nonnegative_token(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).filter(|tokens| *tokens >= 0)
}

fn reported_cost(usage: Option<&Value>, provider_id: &str) -> Option<f64> {
    usage
        .and_then(|usage| {
            usage
                .get("cost_usd")
                .or_else(|| usage.get("total_cost_usd"))
                .or_else(|| {
                    (provider_id == "openrouter")
                        .then(|| usage.get("cost"))
                        .flatten()
                })
        })
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
}

fn now_unix_millis() -> rusqlite::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn cost_nano_usd(cost: Option<f64>) -> Option<i64> {
    let nano_usd = cost?.max(0.0) * 1_000_000_000.0;
    (nano_usd.is_finite() && nano_usd <= i64::MAX as f64).then(|| nano_usd.round() as i64)
}

pub(crate) struct UsageRequestTracker<'a> {
    storage: &'a AppStorage,
    event_id: String,
    is_finished: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RequestDataSources {
    pub(crate) instruction_sources: Vec<String>,
    pub(crate) message_ids: Vec<String>,
    pub(crate) archived_message_ids: Vec<String>,
    pub(crate) summarized_through_message_id: Option<String>,
    pub(crate) attachments: Vec<RequestAttachmentSource>,
}

pub(crate) fn instruction_source_categories(
    custom_instructions: Option<&str>,
    project_instructions: Option<&str>,
    project_skills: &[crate::project_skills::ProjectSkill],
    project_index_context: Option<&str>,
    goal_mode: bool,
) -> Vec<String> {
    let mut sources = Vec::new();
    if custom_instructions.is_some_and(|value| !value.trim().is_empty()) {
        sources.push("shared_preferences".to_owned());
    }
    if project_instructions.is_some_and(|value| !value.trim().is_empty()) {
        sources.push("project_instructions".to_owned());
    }
    if !project_skills.is_empty() {
        sources.push("project_skills".to_owned());
    }
    if project_index_context.is_some_and(|value| !value.trim().is_empty()) {
        sources.push("project_index".to_owned());
    }
    if goal_mode {
        sources.push("goal_mode".to_owned());
    }
    sources
}

#[derive(Clone, Debug)]
pub(crate) struct RequestAttachmentSource {
    pub(crate) id: String,
    pub(crate) message_id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) mime_type: String,
}

impl<'a> UsageRequestTracker<'a> {
    pub(crate) fn start(
        storage: &'a AppStorage,
        conversation_id: &str,
        assistant_message_id: Option<&str>,
        provider_id: &str,
        model_id: &str,
        reasoning_effort: Option<&str>,
        operation: &str,
        fast_requested: bool,
    ) -> rusqlite::Result<Self> {
        let event_id = Uuid::new_v4().simple().to_string();
        let database = storage.connect()?;
        database.execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, assistant_message_id,
                provider_id, model_id, reasoning_effort, operation, status,
                fast_requested, usage_source, started_at_unix_ms
             ) VALUES (?1, 'request', ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, 'unavailable', ?9)",
            params![
                event_id,
                conversation_id,
                assistant_message_id,
                provider_id,
                model_id,
                reasoning_effort,
                operation,
                fast_requested,
                now_unix_millis()?,
            ],
        )?;
        Ok(Self {
            storage,
            event_id,
            is_finished: false,
        })
    }

    pub(crate) fn start_with_run(
        storage: &'a AppStorage,
        conversation_id: &str,
        assistant_message_id: Option<&str>,
        provider_id: &str,
        model_id: &str,
        reasoning_effort: Option<&str>,
        operation: &str,
        fast_requested: bool,
        run_id: &str,
    ) -> rusqlite::Result<Self> {
        let event_id = Uuid::new_v4().simple().to_string();
        let database = storage.connect()?;
        database.execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, assistant_message_id,
                provider_id, model_id, reasoning_effort, operation, status,
                fast_requested, usage_source, started_at_unix_ms, run_id
             ) VALUES (?1, 'request', ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, 'unavailable', ?9, ?10)",
            params![
                event_id,
                conversation_id,
                assistant_message_id,
                provider_id,
                model_id,
                reasoning_effort,
                operation,
                fast_requested,
                now_unix_millis()?,
                run_id,
            ],
        )?;
        Ok(Self {
            storage,
            event_id,
            is_finished: false,
        })
    }

    pub(crate) fn complete(&mut self, usage: &UsageData) -> rusqlite::Result<()> {
        self.finish("completed", usage)
    }

    pub(crate) fn record_request_manifest(
        &mut self,
        payload: &Value,
        sources: Option<&RequestDataSources>,
    ) -> rusqlite::Result<()> {
        let manifest = request_data_manifest(payload, sources);
        let encoded = serde_json::to_string(&manifest)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let database = self.storage.connect()?;
        let updated = database.execute(
            "UPDATE usage_events SET request_manifest_json = ?2
             WHERE event_id = ?1 AND status = 'pending'",
            params![self.event_id, encoded],
        )?;
        if updated != 1 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }

    pub(crate) fn fail(&mut self, usage: &UsageData) -> rusqlite::Result<()> {
        self.finish("failed", usage)
    }

    pub(crate) fn cancel(&mut self) -> rusqlite::Result<()> {
        self.finish("cancelled", &UsageData::default())
    }

    fn finish(&mut self, status: &str, usage: &UsageData) -> rusqlite::Result<()> {
        let cost = cost_nano_usd(usage.reported_cost_usd);
        let usage_source = if usage.has_reported_usage() {
            "provider_reported"
        } else {
            "unavailable"
        };
        let database = self.storage.connect()?;
        database.execute(
            "UPDATE usage_events
             SET status = ?2,
                 input_tokens = ?3,
                 output_tokens = ?4,
                 reasoning_tokens = ?5,
                 cached_input_tokens = ?6,
                 cache_write_tokens = ?7,
                 reported_cost_nano_usd = ?8,
                 cost_source = CASE WHEN ?8 IS NULL THEN NULL ELSE 'provider_reported' END,
                 service_tier = ?9,
                 usage_source = ?10,
                 finished_at_unix_ms = ?11
             WHERE event_id = ?1 AND status = 'pending'",
            params![
                self.event_id,
                status,
                usage.input_tokens.filter(|value| *value >= 0),
                usage.output_tokens.filter(|value| *value >= 0),
                usage.reasoning_tokens.filter(|value| *value >= 0),
                usage.cached_input_tokens.filter(|value| *value >= 0),
                usage.cache_write_tokens.filter(|value| *value >= 0),
                cost,
                usage.service_tier,
                usage_source,
                now_unix_millis()?,
            ],
        )?;
        self.is_finished = true;
        Ok(())
    }
}

fn request_data_manifest(payload: &Value, sources: Option<&RequestDataSources>) -> Value {
    let messages = payload
        .get("messages")
        .or_else(|| payload.get("input"))
        .and_then(Value::as_array);
    let mut role_counts = BTreeMap::<String, usize>::new();
    let mut image_count = 0usize;
    let mut tool_result_count = 0usize;
    let mut instruction_bytes = payload
        .get("instructions")
        .and_then(Value::as_str)
        .map_or(0, str::len);
    let mut instruction_sources = Vec::new();

    if let Some(instructions) = payload.get("instructions").and_then(Value::as_str) {
        if !instructions.is_empty() {
            instruction_sources.push("instructions");
        }
    }
    if let Some(messages) = messages {
        for message in messages.iter().take(4096) {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .or_else(|| message.get("type").and_then(Value::as_str))
                .unwrap_or("untyped");
            *role_counts.entry(role.to_owned()).or_default() += 1;
            if role == "tool" || role == "function_call_output" {
                tool_result_count += 1;
            }
            if matches!(role, "system" | "developer") {
                let bytes = message
                    .get("content")
                    .and_then(Value::as_str)
                    .map_or(0, str::len);
                instruction_bytes = instruction_bytes.saturating_add(bytes);
                if bytes > 0 && !instruction_sources.contains(&role) {
                    instruction_sources.push(role);
                }
            }
            if let Some(content) = message.get("content").and_then(Value::as_array) {
                image_count = image_count.saturating_add(
                    content
                        .iter()
                        .filter(|part| {
                            matches!(
                                part.get("type").and_then(Value::as_str),
                                Some("input_image" | "image_url")
                            )
                        })
                        .count(),
                );
            }
        }
    }
    if let Some(sources) = sources {
        for source in sources.instruction_sources.iter().take(8) {
            if matches!(
                source.as_str(),
                "shared_preferences"
                    | "project_instructions"
                    | "project_skills"
                    | "project_index"
                    | "goal_mode"
            ) && !instruction_sources.contains(&source.as_str())
            {
                instruction_sources.push(source.as_str());
            }
        }
    }

    let mut tool_names = payload
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(64)
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(Value::as_str)
                .filter(|name| {
                    !name.is_empty()
                        && name.len() <= 128
                        && name.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '_' | '-' | '.')
                        })
                })
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    tool_names.sort();
    tool_names.dedup();

    let cache_controls = [
        "prompt_cache_key",
        "prompt_cache_retention",
        "cache_control",
        "cache_key",
    ]
    .into_iter()
    .filter(|key| payload.get(*key).is_some())
    .collect::<Vec<_>>();

    let message_ids = sources
        .into_iter()
        .flat_map(|sources| &sources.message_ids)
        .take(64)
        .cloned()
        .collect::<Vec<_>>();
    let archived_message_ids = sources
        .into_iter()
        .flat_map(|sources| &sources.archived_message_ids)
        .take(16)
        .cloned()
        .collect::<Vec<_>>();
    let attachments = sources
        .into_iter()
        .flat_map(|sources| &sources.attachments)
        .take(32)
        .map(|attachment| {
            json!({
                "id": attachment.id,
                "messageId": attachment.message_id,
                "name": attachment.name.chars().take(128).collect::<String>(),
                "kind": attachment.kind,
                "mimeType": attachment.mime_type,
            })
        })
        .collect::<Vec<_>>();
    let source_limits = json!({
        "messageIdsTruncated": sources.is_some_and(|sources| sources.message_ids.len() > 64),
        "archivedMessageIdsTruncated": sources.is_some_and(|sources| sources.archived_message_ids.len() > 16),
        "attachmentsTruncated": sources.is_some_and(|sources| sources.attachments.len() > 32),
    });

    json!({
        "messageCount": messages.map_or(0, |messages| messages.len().min(4096)),
        "messageRoles": role_counts,
        "imageCount": image_count,
        "toolResultCount": tool_result_count,
        "instructionBytes": instruction_bytes,
        "instructionSources": instruction_sources,
        "toolDefinitions": tool_names,
        "cacheControls": cache_controls,
        "sourceMessageIds": message_ids,
        "archivedMessageIds": archived_message_ids,
        "summarizedThroughMessageId": sources
            .and_then(|sources| sources.summarized_through_message_id.as_deref()),
        "sourceAttachments": attachments,
        "sourceLimits": source_limits,
        "available": messages.is_some()
            || payload.get("instructions").is_some()
            || payload.get("tools").is_some(),
    })
}

impl Drop for UsageRequestTracker<'_> {
    fn drop(&mut self) {
        if self.is_finished {
            return;
        }
        let result = self.storage.connect().and_then(|database| {
            database.execute(
                "UPDATE usage_events
                 SET status = 'failed', finished_at_unix_ms = ?2
                 WHERE event_id = ?1 AND status = 'pending'",
                params![self.event_id, now_unix_millis()?],
            )
        });
        if result.is_err() {
            eprintln!("usage_statistics_request_finalize_failed");
        }
    }
}

pub(crate) fn mark_interrupted_requests(storage: &AppStorage) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE usage_events
         SET status = 'interrupted', finished_at_unix_ms = COALESCE(finished_at_unix_ms, started_at_unix_ms)
         WHERE event_kind = 'request' AND status = 'pending'",
        [],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct StatisticsQuery {
    pub(crate) from_unix_ms: i64,
    pub(crate) to_unix_ms: i64,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) operation: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) fast_mode: Option<bool>,
    pub(crate) offset: i64,
    pub(crate) limit: i64,
}

const FILTER: &str = "
    WHERE e.started_at_unix_ms >= ?1 AND e.started_at_unix_ms < ?2
      AND (?3 IS NULL OR e.provider_id = ?3)
      AND (?4 IS NULL OR e.model_id = ?4)
      AND (?5 IS NULL OR e.operation = ?5)
      AND (?6 IS NULL OR (?6 = 'unspecified' AND e.reasoning_effort IS NULL) OR e.reasoning_effort = ?6)
      AND (?7 IS NULL OR e.fast_requested = ?7)
";

const CATALOG_COST_JOIN: &str =
    "LEFT JOIN models_dev_event_costs AS mc ON mc.event_id = e.event_id";

fn filter_params(query: &StatisticsQuery) -> [rusqlite::types::Value; 7] {
    [
        query.from_unix_ms.into(),
        query.to_unix_ms.into(),
        query.provider_id.as_deref().map(str::to_owned).into(),
        query.model_id.as_deref().map(str::to_owned).into(),
        query.operation.as_deref().map(str::to_owned).into(),
        query.reasoning_effort.as_deref().map(str::to_owned).into(),
        query.fast_mode.map(i64::from).into(),
    ]
}

fn sum_fields() -> &'static str {
    "COALESCE(SUM(e.input_tokens), 0),
     COALESCE(SUM(e.output_tokens), 0),
     COALESCE(SUM(e.reasoning_tokens), 0),
     COALESCE(SUM(e.cached_input_tokens), 0),
     COALESCE(SUM(e.cache_write_tokens), 0),
     COALESCE(SUM(e.reported_cost_nano_usd), 0) / 1000000000.0,
     COALESCE(SUM(CASE WHEN e.reported_cost_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0),
     COALESCE(SUM(mc.cost_usd), 0),
     COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0)"
}

pub(crate) async fn get_statistics(
    storage: &AppStorage,
    query: &StatisticsQuery,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let catalog = models_dev_pricing::load(storage, cancellation).await?;
    let database = storage.connect().map_err(|_| statistics_storage_error())?;
    catalog
        .install_temporary_prices(&database)
        .map_err(|_| statistics_storage_error())?;
    get_statistics_from_database(&database, query, catalog.metadata())
        .map_err(|_| statistics_storage_error())
}

fn statistics_storage_error() -> ServiceError {
    ServiceError::new(
        "usage_statistics_unavailable",
        "Local usage statistics could not be loaded.",
        true,
    )
}

fn get_statistics_from_database(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
    pricing_metadata: Value,
) -> rusqlite::Result<Value> {
    let filter = filter_params(query);
    let summary_sql = format!(
        "SELECT {},
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.status = 'completed' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.status = 'failed' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.status = 'cancelled' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.status = 'interrupted' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.status = 'pending' THEN 1 ELSE 0 END), 0),
            COUNT(DISTINCT e.conversation_id),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.fast_requested = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.input_tokens IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.output_tokens IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND e.reasoning_tokens IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' AND (
                e.input_tokens IS NOT NULL OR e.output_tokens IS NOT NULL
                OR e.reasoning_tokens IS NOT NULL OR e.cached_input_tokens IS NOT NULL
                OR e.cache_write_tokens IS NOT NULL
            ) THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'legacy_output' THEN 1 ELSE 0 END), 0)
         FROM usage_events AS e {CATALOG_COST_JOIN} {}",
        sum_fields(),
        FILTER
    );
    let summary = database.query_row(&summary_sql, filter, |row| {
        let input_tokens = row.get::<_, i64>(0)?;
        let output_tokens = row.get::<_, i64>(1)?;
        Ok(json!({
            "inputTokens": input_tokens,
            "outputTokens": output_tokens,
            "totalTokens": input_tokens.saturating_add(output_tokens),
            "reasoningTokens": row.get::<_, i64>(2)?,
            "cachedInputTokens": row.get::<_, i64>(3)?,
            "cacheWriteTokens": row.get::<_, i64>(4)?,
            "providerReportedCostUsd": row.get::<_, f64>(5)?,
            "costReportedRequests": row.get::<_, i64>(6)?,
            "modelsDevCatalogCostUsd": row.get::<_, f64>(7)?,
            "modelsDevCatalogCostRequests": row.get::<_, i64>(8)?,
            "totalRequests": row.get::<_, i64>(9)?,
            "successfulRequests": row.get::<_, i64>(10)?,
            "failedRequests": row.get::<_, i64>(11)?,
            "cancelledRequests": row.get::<_, i64>(12)?,
            "interruptedRequests": row.get::<_, i64>(13)?,
            "pendingRequests": row.get::<_, i64>(14)?,
            "conversationCount": row.get::<_, i64>(15)?,
            "fastRequestedRequests": row.get::<_, i64>(16)?,
            "inputUsageRequests": row.get::<_, i64>(17)?,
            "outputUsageRequests": row.get::<_, i64>(18)?,
            "reasoningUsageRequests": row.get::<_, i64>(19)?,
            "anyUsageRequests": row.get::<_, i64>(20)?,
            "legacyOutputMessages": row.get::<_, i64>(21)?,
        }))
    })?;

    let provider_breakdown = breakdown(database, query, "provider_id", "providerId")?;
    let model_breakdown = breakdown(database, query, "model_id", "modelId")?;
    let reasoning_breakdown = breakdown(database, query, "reasoning_effort", "reasoningEffort")?;
    let operation_breakdown = breakdown(database, query, "operation", "operation")?;
    let fast_mode_breakdown = fast_mode_breakdown(database, query)?;
    let service_tier_breakdown = service_tier_breakdown(database, query)?;
    let daily_trend = trend(database, query, "%Y-%m-%d", "day")?;
    let monthly_trend = trend(database, query, "%Y-%m", "month")?;
    let conversation_ranking = conversation_ranking(database, query)?;
    let detail_count = count_details(database, query)?;
    let request_details = request_details(database, query)?;
    let filter_options = filter_options(database)?;
    let quota_history = quota_history(database, query)?;

    Ok(json!({
        "summary": summary.clone(),
        "modelsDevPricing": pricing_metadata,
        "coverage": {
            "totalRequests": summary["totalRequests"],
            "inputUsageRequests": summary["inputUsageRequests"],
            "outputUsageRequests": summary["outputUsageRequests"],
            "reasoningUsageRequests": summary["reasoningUsageRequests"],
            "anyUsageRequests": summary["anyUsageRequests"],
            "legacyOutputMessages": summary["legacyOutputMessages"],
        },
        "providerBreakdown": provider_breakdown,
        "modelBreakdown": model_breakdown,
        "reasoningBreakdown": reasoning_breakdown,
        "operationBreakdown": operation_breakdown,
        "fastModeBreakdown": fast_mode_breakdown,
        "serviceTierBreakdown": service_tier_breakdown,
        "dailyTrend": daily_trend,
        "monthlyTrend": monthly_trend,
        "conversationRanking": conversation_ranking,
        "requestDetailCount": detail_count,
        "requestDetails": request_details,
        "filterOptions": filter_options,
        "quotaHistory": quota_history,
    }))
}

fn breakdown(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
    group_column: &str,
    output_key: &str,
) -> rusqlite::Result<Vec<Value>> {
    let (group_expression, group_condition) = match group_column {
        "provider_id" => ("e.provider_id", "AND e.provider_id IS NOT NULL"),
        "model_id" => ("e.model_id", "AND e.model_id IS NOT NULL"),
        "reasoning_effort" => ("COALESCE(e.reasoning_effort, 'unspecified')", ""),
        "operation" => ("e.operation", "AND e.operation IS NOT NULL"),
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let sql = format!(
        "SELECT {group_expression},
            COUNT(*),
            COALESCE(SUM(e.input_tokens), 0),
            COALESCE(SUM(e.output_tokens), 0),
            COALESCE(SUM(e.reasoning_tokens), 0),
            COALESCE(SUM(e.cached_input_tokens), 0),
            COALESCE(SUM(e.cache_write_tokens), 0),
            COALESCE(SUM(e.reported_cost_nano_usd), 0) / 1000000000.0,
            COALESCE(SUM(CASE WHEN e.reported_cost_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(mc.cost_usd), 0),
            COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0)
         FROM usage_events AS e {CATALOG_COST_JOIN} {}
           AND e.event_kind = 'request' {group_condition}
         GROUP BY {group_expression}
         ORDER BY (COALESCE(SUM(e.input_tokens), 0) + COALESCE(SUM(e.output_tokens), 0)) DESC, {group_expression}
         LIMIT 200",
        FILTER
    );
    let mut statement = database.prepare(&sql)?;
    statement
        .query_map(filter_params(query), |row| {
            Ok(json!({
                output_key: row.get::<_, String>(0)?,
                "requests": row.get::<_, i64>(1)?,
                "inputTokens": row.get::<_, i64>(2)?,
                "outputTokens": row.get::<_, i64>(3)?,
                "reasoningTokens": row.get::<_, i64>(4)?,
                "cachedInputTokens": row.get::<_, i64>(5)?,
                "cacheWriteTokens": row.get::<_, i64>(6)?,
                "providerReportedCostUsd": row.get::<_, f64>(7)?,
                "costReportedRequests": row.get::<_, i64>(8)?,
                "modelsDevCatalogCostUsd": row.get::<_, f64>(9)?,
                "modelsDevCatalogCostRequests": row.get::<_, i64>(10)?,
            }))
        })?
        .collect()
}

fn fast_mode_breakdown(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.fast_requested, COUNT(*),
            COALESCE(SUM(e.input_tokens), 0), COALESCE(SUM(e.output_tokens), 0),
            COALESCE(SUM(e.reported_cost_nano_usd), 0) / 1000000000.0,
            COALESCE(SUM(CASE WHEN e.reported_cost_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(mc.cost_usd), 0),
            COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0)
         FROM usage_events AS e {CATALOG_COST_JOIN} {}
           AND e.event_kind = 'request'
         GROUP BY e.fast_requested ORDER BY e.fast_requested DESC",
        FILTER
    );
    let mut statement = database.prepare(&sql)?;
    statement
        .query_map(filter_params(query), |row| {
            Ok(json!({
                "fastRequested": row.get::<_, Option<bool>>(0)?,
                "requests": row.get::<_, i64>(1)?,
                "inputTokens": row.get::<_, i64>(2)?,
                "outputTokens": row.get::<_, i64>(3)?,
                "providerReportedCostUsd": row.get::<_, f64>(4)?,
                "costReportedRequests": row.get::<_, i64>(5)?,
                "modelsDevCatalogCostUsd": row.get::<_, f64>(6)?,
                "modelsDevCatalogCostRequests": row.get::<_, i64>(7)?,
            }))
        })?
        .collect()
}

fn service_tier_breakdown(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.service_tier, COUNT(*),
            COALESCE(SUM(e.input_tokens), 0),
            COALESCE(SUM(e.output_tokens), 0),
            COALESCE(SUM(e.reasoning_tokens), 0),
            COALESCE(SUM(e.cached_input_tokens), 0),
            COALESCE(SUM(e.cache_write_tokens), 0),
            COALESCE(SUM(e.reported_cost_nano_usd), 0) / 1000000000.0,
            COALESCE(SUM(CASE WHEN e.reported_cost_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(mc.cost_usd), 0),
            COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0)
         FROM usage_events AS e {CATALOG_COST_JOIN} {}
           AND e.event_kind = 'request' AND e.service_tier IS NOT NULL
         GROUP BY e.service_tier ORDER BY COUNT(*) DESC, e.service_tier",
        FILTER
    );
    let mut statement = database.prepare(&sql)?;
    statement
        .query_map(filter_params(query), |row| {
            Ok(json!({
                "serviceTier": row.get::<_, String>(0)?,
                "requests": row.get::<_, i64>(1)?,
                "inputTokens": row.get::<_, i64>(2)?,
                "outputTokens": row.get::<_, i64>(3)?,
                "reasoningTokens": row.get::<_, i64>(4)?,
                "cachedInputTokens": row.get::<_, i64>(5)?,
                "cacheWriteTokens": row.get::<_, i64>(6)?,
                "providerReportedCostUsd": row.get::<_, f64>(7)?,
                "costReportedRequests": row.get::<_, i64>(8)?,
                "modelsDevCatalogCostUsd": row.get::<_, f64>(9)?,
                "modelsDevCatalogCostRequests": row.get::<_, i64>(10)?,
            }))
        })?
        .collect()
}

fn trend(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
    date_format: &str,
    key: &str,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT strftime('{date_format}', e.started_at_unix_ms / 1000, 'unixepoch', 'localtime'),
            COALESCE(SUM(e.input_tokens), 0),
            COALESCE(SUM(e.output_tokens), 0),
            COALESCE(SUM(mc.cost_usd), 0),
            COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN e.event_kind = 'request' THEN 1 ELSE 0 END), 0)
         FROM usage_events AS e {CATALOG_COST_JOIN} {}
         GROUP BY 1 ORDER BY 1",
        FILTER
    );
    database
        .prepare(&sql)?
        .query_map(filter_params(query), |row| {
            Ok(json!({
                key: row.get::<_, Option<String>>(0)?,
                "inputTokens": row.get::<_, i64>(1)?,
                "outputTokens": row.get::<_, i64>(2)?,
                "modelsDevCatalogCostUsd": row.get::<_, f64>(3)?,
                "modelsDevCatalogCostRequests": row.get::<_, i64>(4)?,
                "requests": row.get::<_, i64>(5)?,
            }))
        })?
        .collect()
}

fn conversation_ranking(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.conversation_id, conversation.title,
            COUNT(CASE WHEN e.event_kind = 'request' THEN 1 END),
            COALESCE(SUM(e.input_tokens), 0),
            COALESCE(SUM(e.output_tokens), 0),
            COALESCE(SUM(mc.cost_usd), 0),
            COALESCE(SUM(CASE WHEN mc.event_id IS NOT NULL THEN 1 ELSE 0 END), 0),
            MAX(e.started_at_unix_ms)
         FROM usage_events AS e
         {CATALOG_COST_JOIN}
         JOIN conversations AS conversation ON conversation.id = e.conversation_id
         {}
         GROUP BY e.conversation_id, conversation.title
         ORDER BY (COALESCE(SUM(e.input_tokens), 0) + COALESCE(SUM(e.output_tokens), 0)) DESC,
                  MAX(e.started_at_unix_ms) DESC
         LIMIT 20",
        FILTER
    );
    let mut statement = database.prepare(&sql)?;
    statement
        .query_map(filter_params(query), |row| {
            Ok(json!({
                "conversationId": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "requests": row.get::<_, i64>(2)?,
                "inputTokens": row.get::<_, i64>(3)?,
                "outputTokens": row.get::<_, i64>(4)?,
                "modelsDevCatalogCostUsd": row.get::<_, f64>(5)?,
                "modelsDevCatalogCostRequests": row.get::<_, i64>(6)?,
                "lastRequestAtUnixMs": row.get::<_, i64>(7)?,
            }))
        })?
        .collect()
}

fn count_details(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM usage_events AS e {}", FILTER);
    database.query_row(&sql, filter_params(query), |row| row.get(0))
}

fn request_details(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.event_id, e.event_kind, e.conversation_id, conversation.title,
            e.assistant_message_id, e.provider_id, e.model_id, e.reasoning_effort,
            e.operation, e.status, e.fast_requested, e.service_tier,
            e.input_tokens, e.output_tokens, e.reasoning_tokens, e.cached_input_tokens,
            e.cache_write_tokens, e.reported_cost_nano_usd, e.cost_source, e.usage_source,
            e.started_at_unix_ms, e.finished_at_unix_ms, mc.cost_usd,
            e.request_manifest_json, e.run_id
         FROM usage_events AS e
         {CATALOG_COST_JOIN}
         JOIN conversations AS conversation ON conversation.id = e.conversation_id
         {} ORDER BY e.started_at_unix_ms DESC, e.event_id DESC
         LIMIT ?8 OFFSET ?9",
        FILTER
    );
    let mut values = filter_params(query).to_vec();
    values.push(query.limit.into());
    values.push(query.offset.into());
    let mut statement = database.prepare(&sql)?;
    statement
        .query_map(params_from_iter(values.iter()), |row| {
            let cost_nano_usd = row.get::<_, Option<i64>>(17)?;
            let manifest_json = row.get::<_, Option<String>>(23)?;
            let request_manifest: Option<Value> = manifest_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        23,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            Ok(json!({
                "eventId": row.get::<_, String>(0)?,
                "eventKind": row.get::<_, String>(1)?,
                "conversationId": row.get::<_, String>(2)?,
                "conversationTitle": row.get::<_, String>(3)?,
                "assistantMessageId": row.get::<_, Option<String>>(4)?,
                "providerId": row.get::<_, Option<String>>(5)?,
                "modelId": row.get::<_, Option<String>>(6)?,
                "reasoningEffort": row.get::<_, Option<String>>(7)?,
                "operation": row.get::<_, String>(8)?,
                "status": row.get::<_, String>(9)?,
                "fastRequested": row.get::<_, Option<bool>>(10)?,
                "serviceTier": row.get::<_, Option<String>>(11)?,
                "inputTokens": row.get::<_, Option<i64>>(12)?,
                "outputTokens": row.get::<_, Option<i64>>(13)?,
                "reasoningTokens": row.get::<_, Option<i64>>(14)?,
                "cachedInputTokens": row.get::<_, Option<i64>>(15)?,
                "cacheWriteTokens": row.get::<_, Option<i64>>(16)?,
                "providerReportedCostUsd": cost_nano_usd.map(|cost| cost as f64 / 1_000_000_000.0),
                "costSource": row.get::<_, Option<String>>(18)?,
                "usageSource": row.get::<_, String>(19)?,
                "startedAtUnixMs": row.get::<_, i64>(20)?,
                "finishedAtUnixMs": row.get::<_, Option<i64>>(21)?,
                "modelsDevCatalogCostUsd": row.get::<_, Option<f64>>(22)?,
                "requestManifest": request_manifest,
                "runId": row.get::<_, Option<String>>(24)?,
            }))
        })?
        .collect()
}

#[cfg(test)]
mod request_manifest_tests {
    use serde_json::json;

    use super::{
        RequestAttachmentSource, RequestDataSources, instruction_source_categories,
        request_data_manifest,
    };

    #[test]
    fn instruction_source_categories_exclude_empty_and_unconfigured_sources() {
        assert_eq!(
            instruction_source_categories(Some("shared"), Some("project"), &[], None, true),
            ["shared_preferences", "project_instructions", "goal_mode"]
        );
        assert!(instruction_source_categories(Some(" \n"), None, &[], None, false).is_empty());
    }

    #[test]
    fn instruction_source_categories_record_project_skills_separately() {
        let skills = [crate::project_skills::ProjectSkill {
            id: "rust-style".to_owned(),
            content: "Use rustfmt.".to_owned(),
        }];
        assert_eq!(
            instruction_source_categories(None, None, &skills, None, false),
            ["project_skills"]
        );
    }

    #[test]
    fn instruction_source_categories_record_project_index_separately() {
        assert_eq!(
            instruction_source_categories(None, None, &[], Some("excerpt"), false),
            ["project_index"]
        );
    }

    #[test]
    fn request_manifest_records_sent_data_categories_without_content() {
        let payload = json!({
            "instructions": "private system instruction",
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "private user prompt"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,secret"}}
                ]},
                {"role": "assistant", "content": "private answer"},
                {"role": "tool", "tool_call_id": "call-1", "content": "private tool output"}
            ],
            "tools": [
                {"type": "function", "function": {"name": "read_file"}},
                {"type": "web_search", "name": "web_search"}
            ],
            "prompt_cache_key": "conversation-key"
        });

        let sources = RequestDataSources {
            instruction_sources: vec!["project_instructions".to_owned()],
            message_ids: vec!["message-user".to_owned()],
            archived_message_ids: vec!["message-archive".to_owned()],
            summarized_through_message_id: Some("message-summary-boundary".to_owned()),
            attachments: vec![RequestAttachmentSource {
                id: "attachment-id".to_owned(),
                message_id: "message-user".to_owned(),
                name: "notes.txt".to_owned(),
                kind: "text".to_owned(),
                mime_type: "text/plain".to_owned(),
            }],
        };
        let manifest = request_data_manifest(&payload, Some(&sources));

        assert_eq!(manifest["messageCount"], 3);
        assert_eq!(manifest["messageRoles"]["user"], 1);
        assert_eq!(manifest["messageRoles"]["assistant"], 1);
        assert_eq!(manifest["messageRoles"]["tool"], 1);
        assert_eq!(manifest["imageCount"], 1);
        assert_eq!(manifest["toolResultCount"], 1);
        assert_eq!(manifest["instructionBytes"], 26);
        assert_eq!(manifest["instructionSources"][0], "instructions");
        assert_eq!(manifest["instructionSources"][1], "project_instructions");
        assert_eq!(manifest["toolDefinitions"][0], "read_file");
        assert_eq!(manifest["toolDefinitions"][1], "web_search");
        assert_eq!(manifest["cacheControls"][0], "prompt_cache_key");
        assert_eq!(manifest["sourceMessageIds"][0], "message-user");
        assert_eq!(manifest["archivedMessageIds"][0], "message-archive");
        assert_eq!(
            manifest["summarizedThroughMessageId"],
            "message-summary-boundary"
        );
        assert_eq!(manifest["sourceAttachments"][0]["name"], "notes.txt");
        let encoded = serde_json::to_string(&manifest).expect("serialize manifest");
        assert!(!encoded.contains("private"));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("text/plain\".*private"));
    }
}

fn filter_options(database: &rusqlite::Connection) -> rusqlite::Result<Value> {
    fn distinct_values(
        database: &rusqlite::Connection,
        column: &str,
    ) -> rusqlite::Result<Vec<String>> {
        let sql = format!(
            "SELECT DISTINCT {column} FROM usage_events
             WHERE event_kind = 'request' AND {column} IS NOT NULL
             ORDER BY {column}"
        );
        let mut statement = database.prepare(&sql)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect()
    }

    let reasoning_efforts = {
        let mut statement = database.prepare(
            "SELECT DISTINCT COALESCE(reasoning_effort, 'unspecified')
             FROM usage_events WHERE event_kind = 'request' ORDER BY 1",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(json!({
        "providers": distinct_values(database, "provider_id")?,
        "models": distinct_values(database, "model_id")?,
        "reasoningEfforts": reasoning_efforts,
    }))
}

fn quota_history(
    database: &rusqlite::Connection,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = database.prepare(
        "SELECT snapshot.id, snapshot.connection_id, snapshot.workspace_id,
            connection.display_name, workspace.display_name, snapshot.fetched_at_unix_ms,
            snapshot.freshness, snapshot.ordinary_usage_allowed,
            snapshot.reset_credit_count, snapshot.reset_credit_details_state
         FROM chatgpt_usage_snapshots AS snapshot
         JOIN chatgpt_connections AS connection ON connection.id = snapshot.connection_id
         JOIN chatgpt_workspaces AS workspace
           ON workspace.connection_id = snapshot.connection_id
          AND workspace.id = snapshot.workspace_id
         WHERE snapshot.fetched_at_unix_ms >= ?1 AND snapshot.fetched_at_unix_ms < ?2
         ORDER BY snapshot.fetched_at_unix_ms DESC, snapshot.id",
    )?;
    let snapshots = statement
        .query_map(params![query.from_unix_ms, query.to_unix_ms], |row| {
            Ok((
                row.get::<_, String>(0)?,
                json!({
                    "connectionId": row.get::<_, String>(1)?,
                    "workspaceId": row.get::<_, String>(2)?,
                    "connectionName": row.get::<_, Option<String>>(3)?,
                    "workspaceName": row.get::<_, Option<String>>(4)?,
                    "fetchedAtUnixMs": row.get::<_, i64>(5)?,
                    "freshness": row.get::<_, String>(6)?,
                    "ordinaryUsageAllowed": row.get::<_, Option<bool>>(7)?,
                    "resetCreditCount": row.get::<_, Option<i64>>(8)?,
                    "resetCreditDetailsState": row.get::<_, String>(9)?,
                }),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    snapshots
        .into_iter()
        .map(|(snapshot_id, mut snapshot)| {
            let mut bucket_statement = database.prepare(
                "SELECT limit_id, used_percent, window_seconds, reset_at_unix_ms
                 FROM chatgpt_usage_buckets WHERE snapshot_id = ?1 ORDER BY limit_id",
            )?;
            let buckets = bucket_statement
                .query_map([snapshot_id], |row| {
                    Ok(json!({
                        "limitId": row.get::<_, String>(0)?,
                        "usedPercent": row.get::<_, Option<f64>>(1)?,
                        "windowSeconds": row.get::<_, Option<i64>>(2)?,
                        "resetAtUnixMs": row.get::<_, Option<i64>>(3)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            snapshot["buckets"] = json!(buckets);
            Ok(snapshot)
        })
        .collect()
}
