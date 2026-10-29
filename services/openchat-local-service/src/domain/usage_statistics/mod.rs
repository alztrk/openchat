//! Records provider usage and builds filtered cost, token, and quota reports.
//!
//! Usage is persisted locally; estimated costs are included only when valid
//! pricing metadata is available for the request's provider and model.

mod models_dev_pricing;

use rusqlite::{params, params_from_iter};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
};
use tokio::sync::watch;

#[derive(Clone, Debug, Default)]
/// Usage and timing values reported for one provider response.
pub(crate) struct UsageData {
    /// Total input tokens, including cache tokens reported separately by a provider.
    pub(crate) input_tokens: Option<i64>,
    /// Output tokens reported by the provider.
    pub(crate) output_tokens: Option<i64>,
    /// Reasoning tokens when the provider reports them separately.
    pub(crate) reasoning_tokens: Option<i64>,
    /// Input tokens served from provider cache, when reported.
    pub(crate) cached_input_tokens: Option<i64>,
    /// Tokens written to provider cache, when reported.
    pub(crate) cache_write_tokens: Option<i64>,
    /// Provider-reported request cost in USD, when available.
    pub(crate) reported_cost_usd: Option<f64>,
    /// Provider service tier associated with the response.
    pub(crate) service_tier: Option<String>,
    /// Bounded provider response reason, such as a stop or tool-call reason.
    pub(crate) completion_reason: Option<String>,
    /// Provider-reported request processing time in milliseconds, when available.
    pub(crate) provider_processing_ms: Option<i64>,
    /// Provider-reported request queue time in milliseconds, when available.
    pub(crate) provider_queue_time_ms: Option<i64>,
    /// Provider-reported input processing time in milliseconds, when available.
    pub(crate) provider_input_processing_ms: Option<i64>,
    /// Provider-reported output generation time in milliseconds, when available.
    pub(crate) provider_output_processing_ms: Option<i64>,
    /// Provider-reported rate-limit values observed on the response, when available.
    pub(crate) provider_rate_limits: Option<ProviderRateLimitObservation>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
/// Rate-limit values observed on one provider response.
pub(crate) struct ProviderRateLimitObservation {
    pub(crate) observed_at_unix_ms: i64,
    pub(crate) request_limit: Option<i64>,
    pub(crate) request_remaining: Option<i64>,
    pub(crate) request_reset_at_unix_ms: Option<i64>,
    pub(crate) token_limit: Option<i64>,
    pub(crate) token_remaining: Option<i64>,
    pub(crate) token_reset_at_unix_ms: Option<i64>,
}

impl UsageData {
    /// Extracts usage fields from a Responses API event.
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
            completion_reason: provider_reason(response.pointer("/incomplete_details/reason")),
            provider_processing_ms: groq_duration_ms(provider_id, usage, "total_time"),
            provider_queue_time_ms: groq_duration_ms(provider_id, usage, "queue_time"),
            provider_input_processing_ms: groq_duration_ms(provider_id, usage, "prompt_time"),
            provider_output_processing_ms: groq_duration_ms(provider_id, usage, "completion_time"),
            provider_rate_limits: None,
        }
    }

    /// Extracts usage fields from a Chat Completions response event.
    pub(crate) fn from_chat_completion_event(value: &Value, provider_id: &str) -> Self {
        if provider_id == "gemini"
            && let Some(usage) = value.get("usageMetadata").filter(|usage| usage.is_object())
        {
            return Self {
                input_tokens: nonnegative_token(usage.get("promptTokenCount")),
                output_tokens: nonnegative_token(usage.get("candidatesTokenCount")),
                reasoning_tokens: nonnegative_token(usage.get("thoughtsTokenCount")),
                cached_input_tokens: nonnegative_token(usage.get("cachedContentTokenCount")),
                cache_write_tokens: None,
                reported_cost_usd: None,
                service_tier: usage
                    .get("serviceTier")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                completion_reason: provider_reason(value.pointer("/candidates/0/finishReason")),
                provider_processing_ms: None,
                provider_queue_time_ms: None,
                provider_input_processing_ms: None,
                provider_output_processing_ms: None,
                provider_rate_limits: None,
            };
        }

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
            completion_reason: provider_reason(
                value
                    .pointer("/choices/0/finish_reason")
                    .or_else(|| value.pointer("/candidates/0/finishReason")),
            ),
            provider_processing_ms: groq_duration_ms(provider_id, usage, "total_time"),
            provider_queue_time_ms: groq_duration_ms(provider_id, usage, "queue_time"),
            provider_input_processing_ms: groq_duration_ms(provider_id, usage, "prompt_time"),
            provider_output_processing_ms: groq_duration_ms(provider_id, usage, "completion_time"),
            provider_rate_limits: None,
        }
    }

    /// Extracts usage fields from an Anthropic Messages response.
    pub(crate) fn from_messages_event(value: &Value, provider_id: &str) -> Self {
        let usage = value.get("usage");
        Self {
            input_tokens: messages_total_input_tokens(usage),
            output_tokens: nonnegative_token(usage.and_then(|usage| usage.get("output_tokens"))),
            reasoning_tokens: None,
            cached_input_tokens: nonnegative_token(
                usage.and_then(|usage| usage.get("cache_read_input_tokens")),
            ),
            cache_write_tokens: nonnegative_token(
                usage.and_then(|usage| usage.get("cache_creation_input_tokens")),
            ),
            reported_cost_usd: reported_cost(usage, provider_id),
            service_tier: None,
            completion_reason: provider_reason(
                value
                    .pointer("/delta/stop_reason")
                    .or_else(|| value.get("stop_reason")),
            ),
            provider_processing_ms: None,
            provider_queue_time_ms: None,
            provider_input_processing_ms: None,
            provider_output_processing_ms: None,
            provider_rate_limits: None,
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

pub(crate) fn provider_reason(value: Option<&Value>) -> Option<String> {
    let reason = value?.as_str()?;
    if reason.is_empty()
        || reason.len() > 64
        || !reason
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return None;
    }
    Some(reason.to_owned())
}

fn groq_duration_ms(provider_id: &str, usage: Option<&Value>, field: &str) -> Option<i64> {
    if provider_id != "groq" {
        return None;
    }
    let seconds = usage?.get(field)?.as_f64()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let milliseconds = (seconds * 1_000.0).round();
    if milliseconds >= i64::MAX as f64 {
        return None;
    }
    Some(milliseconds as i64)
}

fn nonnegative_token(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).filter(|tokens| *tokens >= 0)
}

fn messages_total_input_tokens(usage: Option<&Value>) -> Option<i64> {
    let usage = usage?;
    let input_tokens = nonnegative_token(usage.get("input_tokens"))?;
    let cache_read_tokens = optional_nonnegative_token(usage.get("cache_read_input_tokens"))?;
    let cache_write_tokens = optional_nonnegative_token(usage.get("cache_creation_input_tokens"))?;

    // Anthropic reports cache counts separately, while local pricing and context tiers use total input.
    input_tokens
        .checked_add(cache_read_tokens)?
        .checked_add(cache_write_tokens)
}

fn optional_nonnegative_token(value: Option<&Value>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => Some(0),
        Some(value) => nonnegative_token(Some(value)),
    }
}

fn reported_cost(usage: Option<&Value>, provider_id: &str) -> Option<f64> {
    let usage = usage?;
    if provider_id == "xai"
        && let Some(ticks) = usage.get("cost_in_usd_ticks").and_then(Value::as_i64)
        && ticks >= 0
    {
        return Some(ticks as f64 / 10_000_000_000.0);
    }
    usage
        .get("cost_usd")
        .or_else(|| usage.get("total_cost_usd"))
        .or_else(|| {
            (provider_id == "openrouter")
                .then(|| usage.get("cost"))
                .flatten()
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

/// Owns a pending usage event until completion or interruption.
pub(crate) struct UsageRequestTracker<'a> {
    storage: &'a AppStorage,
    event_id: String,
    first_output_latency: FirstOutputLatencyRecorder,
    is_finished: bool,
}

/// Shares the first streamed-output measurement between request tracking and stream parsing.
#[derive(Clone, Debug)]
pub(crate) struct FirstOutputLatencyRecorder(Arc<AtomicI64>);

impl Default for FirstOutputLatencyRecorder {
    fn default() -> Self {
        Self(Arc::new(AtomicI64::new(-1)))
    }
}

impl FirstOutputLatencyRecorder {
    pub(crate) fn record(&self, elapsed: Duration) {
        let milliseconds = i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX);
        let _ = self
            .0
            .compare_exchange(-1, milliseconds, Ordering::Relaxed, Ordering::Relaxed);
    }

    pub(crate) fn value(&self) -> Option<i64> {
        let milliseconds = self.0.load(Ordering::Relaxed);
        (milliseconds >= 0).then_some(milliseconds)
    }
}

/// Describes the provider request whose usage should be recorded.
pub(crate) struct UsageRequestDetails<'a> {
    /// Conversation whose provider request should be attributed.
    pub(crate) conversation_id: &'a str,
    /// Assistant message created by the request, when known.
    pub(crate) assistant_message_id: Option<&'a str>,
    /// Provider route identifier.
    pub(crate) provider_id: &'a str,
    /// Model identifier used for the request.
    pub(crate) model_id: &'a str,
    /// Requested reasoning level, if set.
    pub(crate) reasoning_effort: Option<&'a str>,
    /// Operation category, such as chat, title, or image generation.
    pub(crate) operation: &'a str,
    /// Whether fast mode was requested.
    pub(crate) fast_requested: bool,
    /// Active execution identifier, when the request belongs to a run.
    pub(crate) run_id: Option<&'a str>,
    /// Provider connection selected for this request, when available.
    pub(crate) connection_id: Option<&'a str>,
    /// Provider workspace selected for this request, when available.
    pub(crate) workspace_id: Option<&'a str>,
    /// Project selected for this request, when available.
    pub(crate) project_id: Option<&'a str>,
    /// Parent run identifier for a delegated child run, when applicable.
    pub(crate) parent_run_id: Option<&'a str>,
}

#[derive(Clone, Debug, Default)]
/// Source identifiers used to explain context included in a provider request.
pub(crate) struct RequestDataSources {
    /// Categories of system, project, or goal instructions included.
    pub(crate) instruction_sources: Vec<String>,
    /// Message IDs read directly into this request.
    pub(crate) message_ids: Vec<String>,
    /// Archived message IDs represented by retrieved excerpts.
    pub(crate) archived_message_ids: Vec<String>,
    /// Last message included before local summarization, if any.
    pub(crate) summarized_through_message_id: Option<String>,
    /// Attachments included in this request.
    pub(crate) attachments: Vec<RequestAttachmentSource>,
}

/// Categorizes instruction sources without retaining their contents.
pub(crate) fn instruction_source_categories(
    custom_instructions: Option<&str>,
    project_instructions: Option<&str>,
    project_skills: &[crate::project_skills::ProjectSkill],
    project_memory_context: Option<&str>,
    project_history_context: Option<&str>,
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
    if project_memory_context.is_some_and(|value| !value.trim().is_empty()) {
        sources.push("project_memory".to_owned());
    }
    if project_history_context.is_some_and(|value| !value.trim().is_empty()) {
        sources.push("project_history".to_owned());
    }
    if goal_mode {
        sources.push("goal_mode".to_owned());
    }
    sources
}

#[derive(Clone, Debug)]
/// Attachment metadata included with request-source diagnostics.
pub(crate) struct RequestAttachmentSource {
    /// Stable attachment identifier.
    pub(crate) id: String,
    /// Message that owns the attachment.
    pub(crate) message_id: String,
    /// User-visible attachment name.
    pub(crate) name: String,
    /// Attachment category, such as text or image.
    pub(crate) kind: String,
    /// Validated MIME type associated with the attachment.
    pub(crate) mime_type: String,
}

impl<'a> UsageRequestTracker<'a> {
    /// Inserts a pending request event and returns a tracker for its lifecycle.
    pub(crate) fn start(
        storage: &'a AppStorage,
        request: UsageRequestDetails<'_>,
    ) -> rusqlite::Result<Self> {
        let event_id = Uuid::new_v4().simple().to_string();
        let database = storage.connect()?;
        database.execute(
            "INSERT INTO usage_events (
                event_id, event_kind, conversation_id, assistant_message_id,
                provider_id, model_id, reasoning_effort, operation, status,
                fast_requested, usage_source, started_at_unix_ms, run_id,
                connection_id, workspace_id, project_id, parent_run_id
             ) VALUES (
                ?1, 'request', ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, 'unavailable', ?9, ?10,
                ?11, ?12, COALESCE(?13, (SELECT project_id FROM conversations WHERE id = ?2)), ?14
             )",
            params![
                event_id,
                request.conversation_id,
                request.assistant_message_id,
                request.provider_id,
                request.model_id,
                request.reasoning_effort,
                request.operation,
                request.fast_requested,
                now_unix_millis()?,
                request.run_id,
                request.connection_id,
                request.workspace_id,
                request.project_id,
                request.parent_run_id,
            ],
        )?;
        Ok(Self {
            storage,
            event_id,
            first_output_latency: FirstOutputLatencyRecorder::default(),
            is_finished: false,
        })
    }

    /// Records the local request-to-first-output duration once when an output delta arrives.
    pub(crate) fn record_first_output(&mut self, elapsed: Duration) {
        self.first_output_latency.record(elapsed);
    }

    pub(crate) fn first_output_recorder(&self) -> FirstOutputLatencyRecorder {
        self.first_output_latency.clone()
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
        self.cancel_with_usage(&UsageData::default())
    }

    pub(crate) fn cancel_with_usage(&mut self, usage: &UsageData) -> rusqlite::Result<()> {
        self.finish("cancelled", usage)
    }

    fn finish(&mut self, status: &str, usage: &UsageData) -> rusqlite::Result<()> {
        let cost = cost_nano_usd(usage.reported_cost_usd);
        let provider_rate_limits_json = usage
            .provider_rate_limits
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let usage_source = if usage.has_reported_usage() {
            "provider_reported"
        } else {
            "unavailable"
        };
        let mut database = self.storage.connect()?;
        let transaction = database.transaction()?;
        let updated = transaction.execute(
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
                 finished_at_unix_ms = ?11,
                 first_output_latency_ms = ?12,
                 provider_processing_ms = ?13,
                 provider_queue_time_ms = ?14,
                 provider_input_processing_ms = ?15,
                 provider_output_processing_ms = ?16,
                 completion_reason = ?17,
                 provider_rate_limits_json = ?18
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
                self.first_output_latency.value(),
                usage.provider_processing_ms.filter(|value| *value >= 0),
                usage.provider_queue_time_ms.filter(|value| *value >= 0),
                usage
                    .provider_input_processing_ms
                    .filter(|value| *value >= 0),
                usage
                    .provider_output_processing_ms
                    .filter(|value| *value >= 0),
                usage.completion_reason,
                provider_rate_limits_json,
            ],
        )?;
        transaction.commit()?;
        self.is_finished = true;
        if updated > 0 {
            invalidate_cached_event_cost(&database, &self.event_id)?;
        }
        Ok(())
    }
}

fn invalidate_cached_event_cost(
    database: &StorageConnection<'_>,
    event_id: &str,
) -> rusqlite::Result<()> {
    match database.execute(
        "DELETE FROM temp.models_dev_event_costs WHERE event_id = ?1",
        [event_id],
    ) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(failure, Some(message)))
            if failure.code == rusqlite::ErrorCode::Unknown
                && matches!(
                    message.as_str(),
                    "no such table: temp.models_dev_event_costs"
                        | "no such table: models_dev_event_costs"
                ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
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

    if let Some(instructions) = payload.get("instructions").and_then(Value::as_str)
        && !instructions.is_empty()
    {
        instruction_sources.push("instructions");
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
                    | "project_memory"
                    | "project_history"
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
                 SET status = 'failed', finished_at_unix_ms = ?2,
                     first_output_latency_ms = COALESCE(first_output_latency_ms, ?3)
                 WHERE event_id = ?1 AND status = 'pending'",
                params![
                    self.event_id,
                    now_unix_millis()?,
                    self.first_output_latency.value()
                ],
            )
        });
        if result.is_err() {
            eprintln!("usage_statistics_request_finalize_failed");
        }
    }
}

/// Marks pending requests as interrupted during service startup recovery.
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
/// Time range, filters, and page bounds for usage statistics queries.
pub(crate) struct StatisticsQuery {
    /// Inclusive range start in Unix milliseconds.
    pub(crate) from_unix_ms: i64,
    /// Exclusive range end in Unix milliseconds.
    pub(crate) to_unix_ms: i64,
    /// Optional provider route filter.
    pub(crate) provider_id: Option<String>,
    /// Optional model identifier filter.
    pub(crate) model_id: Option<String>,
    /// Optional operation category filter.
    pub(crate) operation: Option<String>,
    /// Optional reasoning-effort filter.
    pub(crate) reasoning_effort: Option<String>,
    /// Optional fast-mode filter.
    pub(crate) fast_mode: Option<bool>,
    /// Number of matching request details to skip.
    pub(crate) offset: i64,
    /// Maximum number of request details to return.
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
const PAGE_CATALOG_COST_JOIN: &str =
    "LEFT JOIN models_dev_page_event_costs AS mc ON mc.event_id = e.event_id";

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

const STATISTICS_AGGREGATION_PAGE_SIZE: i64 = 20_000;
const STATUS_CODE_COMPLETED: i64 = 1;
const STATUS_CODE_FAILED: i64 = 2;
const STATUS_CODE_CANCELLED: i64 = 3;
const STATUS_CODE_INTERRUPTED: i64 = 4;
const STATUS_CODE_PENDING: i64 = 5;

struct AggregationEvent<'a> {
    is_request: bool,
    is_legacy_output: bool,
    event_id: &'a str,
    conversation_id: &'a str,
    conversation_exists: bool,
    provider_id: Option<&'a str>,
    model_id: Option<&'a str>,
    reasoning_effort: Option<&'a str>,
    operation: &'a str,
    status_code: i64,
    fast_requested: Option<bool>,
    service_tier: Option<&'a str>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    usage_source: &'a str,
    estimated_cost_usd: Option<f64>,
    started_at_unix_ms: i64,
    day: Option<&'a str>,
}

#[derive(Default)]
struct UsageTotals {
    count: i64,
    input_tokens: i64,
    output_tokens: i64,
    reasoning_tokens: i64,
    cached_input_tokens: i64,
    cache_write_tokens: i64,
    cache_share_input_tokens: i64,
    cache_share_cached_tokens: i64,
    cache_share_requests: i64,
    estimated_cost_usd: f64,
    estimated_cost_requests: i64,
}

impl UsageTotals {
    fn add(&mut self, event: &AggregationEvent<'_>) -> rusqlite::Result<()> {
        increment(&mut self.count)?;
        add_optional(&mut self.input_tokens, event.input_tokens)?;
        add_optional(&mut self.output_tokens, event.output_tokens)?;
        add_optional(&mut self.reasoning_tokens, event.reasoning_tokens)?;
        add_optional(&mut self.cached_input_tokens, event.cached_input_tokens)?;
        add_optional(&mut self.cache_write_tokens, event.cache_write_tokens)?;
        if event.is_request
            && event.usage_source == "provider_reported"
            && let (Some(input_tokens), Some(cached_tokens)) =
                (event.input_tokens, event.cached_input_tokens)
            && cached_tokens <= input_tokens
        {
            add_optional(&mut self.cache_share_input_tokens, Some(input_tokens))?;
            add_optional(&mut self.cache_share_cached_tokens, Some(cached_tokens))?;
            increment(&mut self.cache_share_requests)?;
        }
        if let Some(cost) = event.estimated_cost_usd {
            self.estimated_cost_usd += cost;
            increment(&mut self.estimated_cost_requests)?;
        }
        Ok(())
    }

    fn to_breakdown_json(&self, key: &str, value: &str) -> Value {
        json!({
            key: value,
            "requests": self.count,
            "inputTokens": self.input_tokens,
            "outputTokens": self.output_tokens,
            "reasoningTokens": self.reasoning_tokens,
            "cachedInputTokens": self.cached_input_tokens,
            "cacheWriteTokens": self.cache_write_tokens,
            "cacheShareInputTokens": self.cache_share_input_tokens,
            "cacheShareCachedTokens": self.cache_share_cached_tokens,
            "cacheShareRequests": self.cache_share_requests,
            "estimatedCostUsd": self.estimated_cost_usd,
            "estimatedCostRequests": self.estimated_cost_requests,
        })
    }
}

#[derive(Default)]
struct TrendTotals {
    usage: UsageTotals,
    requests: i64,
}

#[derive(Default)]
struct ConversationTotals {
    conversation_exists: bool,
    usage: UsageTotals,
    request_count: i64,
    last_event_at_unix_ms: Option<i64>,
}

#[derive(Default)]
struct StatisticsAggregation {
    all_events: UsageTotals,
    total_requests: i64,
    successful_requests: i64,
    failed_requests: i64,
    cancelled_requests: i64,
    interrupted_requests: i64,
    pending_requests: i64,
    conversations: HashMap<String, ConversationTotals>,
    fast_requested_requests: i64,
    input_usage_requests: i64,
    output_usage_requests: i64,
    reasoning_usage_requests: i64,
    any_usage_requests: i64,
    legacy_output_messages: i64,
    providers: HashMap<String, UsageTotals>,
    models: HashMap<String, UsageTotals>,
    reasoning_efforts: HashMap<String, UsageTotals>,
    operations: HashMap<String, UsageTotals>,
    fast_modes: HashMap<Option<bool>, UsageTotals>,
    service_tiers: HashMap<String, UsageTotals>,
    daily_trends: HashMap<String, TrendTotals>,
    daily_trend_without_date: Option<TrendTotals>,
    monthly_trends: HashMap<String, TrendTotals>,
    monthly_trend_without_date: Option<TrendTotals>,
}

impl StatisticsAggregation {
    fn add(&mut self, event: &AggregationEvent<'_>) -> rusqlite::Result<()> {
        self.all_events.add(event)?;
        let is_request = event.is_request;
        let conversation = group_for::<_, str, _>(&mut self.conversations, event.conversation_id)?;
        conversation.conversation_exists |= event.conversation_exists;
        conversation.usage.add(event)?;
        if is_request {
            increment(&mut conversation.request_count)?;
        }
        conversation.last_event_at_unix_ms = Some(
            conversation
                .last_event_at_unix_ms
                .map_or(event.started_at_unix_ms, |last| {
                    last.max(event.started_at_unix_ms)
                }),
        );
        if is_request {
            increment(&mut self.total_requests)?;
            match event.status_code {
                STATUS_CODE_COMPLETED => increment(&mut self.successful_requests)?,
                STATUS_CODE_FAILED => increment(&mut self.failed_requests)?,
                STATUS_CODE_CANCELLED => increment(&mut self.cancelled_requests)?,
                STATUS_CODE_INTERRUPTED => increment(&mut self.interrupted_requests)?,
                STATUS_CODE_PENDING => increment(&mut self.pending_requests)?,
                _ => {}
            }
            if event.fast_requested == Some(true) {
                increment(&mut self.fast_requested_requests)?;
            }
            if event.input_tokens.is_some() {
                increment(&mut self.input_usage_requests)?;
            }
            if event.output_tokens.is_some() {
                increment(&mut self.output_usage_requests)?;
            }
            if event.reasoning_tokens.is_some() {
                increment(&mut self.reasoning_usage_requests)?;
            }
            if event.input_tokens.is_some()
                || event.output_tokens.is_some()
                || event.reasoning_tokens.is_some()
                || event.cached_input_tokens.is_some()
                || event.cache_write_tokens.is_some()
            {
                increment(&mut self.any_usage_requests)?;
            }
            if let Some(provider_id) = event.provider_id {
                group_for::<_, str, _>(&mut self.providers, provider_id)?.add(event)?;
            }
            if let Some(model_id) = event.model_id {
                group_for::<_, str, _>(&mut self.models, model_id)?.add(event)?;
            }
            let reasoning_effort = event.reasoning_effort.unwrap_or("unspecified");
            group_for::<_, str, _>(&mut self.reasoning_efforts, reasoning_effort)?.add(event)?;
            group_for::<_, str, _>(&mut self.operations, event.operation)?.add(event)?;
            group_for(&mut self.fast_modes, &event.fast_requested)?.add(event)?;
            if let Some(service_tier) = event.service_tier {
                group_for::<_, str, _>(&mut self.service_tiers, service_tier)?.add(event)?;
            }
        } else if event.is_legacy_output {
            increment(&mut self.legacy_output_messages)?;
        }

        let day = match event.day {
            Some(day) => group_for::<_, str, _>(&mut self.daily_trends, day)?,
            None => self
                .daily_trend_without_date
                .get_or_insert_with(TrendTotals::default),
        };
        day.usage.add(event)?;
        if is_request {
            increment(&mut day.requests)?;
        }
        let month = match event.day.and_then(|day| day.get(..7)) {
            Some(month) => group_for::<_, str, _>(&mut self.monthly_trends, month)?,
            None => self
                .monthly_trend_without_date
                .get_or_insert_with(TrendTotals::default),
        };
        month.usage.add(event)?;
        if is_request {
            increment(&mut month.requests)?;
        }
        Ok(())
    }

    fn summary_json(&self) -> Value {
        json!({
            "inputTokens": self.all_events.input_tokens,
            "outputTokens": self.all_events.output_tokens,
            "totalTokens": self.all_events.input_tokens.saturating_add(self.all_events.output_tokens),
            "reasoningTokens": self.all_events.reasoning_tokens,
            "cachedInputTokens": self.all_events.cached_input_tokens,
            "cacheWriteTokens": self.all_events.cache_write_tokens,
            "estimatedCostUsd": self.all_events.estimated_cost_usd,
            "estimatedCostRequests": self.all_events.estimated_cost_requests,
            "totalRequests": self.total_requests,
            "successfulRequests": self.successful_requests,
            "failedRequests": self.failed_requests,
            "cancelledRequests": self.cancelled_requests,
            "interruptedRequests": self.interrupted_requests,
            "pendingRequests": self.pending_requests,
            "conversationCount": self.conversations.len(),
            "fastRequestedRequests": self.fast_requested_requests,
            "inputUsageRequests": self.input_usage_requests,
            "outputUsageRequests": self.output_usage_requests,
            "reasoningUsageRequests": self.reasoning_usage_requests,
            "anyUsageRequests": self.any_usage_requests,
            "legacyOutputMessages": self.legacy_output_messages,
        })
    }
}

fn increment(value: &mut i64) -> rusqlite::Result<()> {
    *value = value.checked_add(1).ok_or_else(integer_overflow)?;
    Ok(())
}

fn add_optional(total: &mut i64, value: Option<i64>) -> rusqlite::Result<()> {
    if let Some(value) = value {
        *total = total.checked_add(value).ok_or_else(integer_overflow)?;
    }
    Ok(())
}

fn integer_overflow() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
        Some("integer overflow".to_owned()),
    )
}

fn group_for<'a, K, Q, V>(groups: &'a mut HashMap<K, V>, key: &Q) -> rusqlite::Result<&'a mut V>
where
    K: Eq + std::hash::Hash + std::borrow::Borrow<Q>,
    Q: Eq + std::hash::Hash + ToOwned<Owned = K> + ?Sized,
    V: Default,
{
    if !groups.contains_key(key) {
        groups.insert(key.to_owned(), V::default());
    }
    groups.get_mut(key).ok_or(rusqlite::Error::InvalidQuery)
}

fn aggregate_usage_events(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<StatisticsAggregation> {
    // Encode finite status labels before crossing the storage boundary for each page.
    let first_page_sql = format!(
        "SELECT e.event_id,
            e.event_kind = 'request', e.event_kind = 'legacy_output', e.conversation_id,
            e.provider_id,
            e.model_id, e.reasoning_effort, e.operation,
            CASE e.status
                WHEN 'completed' THEN 1 WHEN 'failed' THEN 2 WHEN 'cancelled' THEN 3
                WHEN 'interrupted' THEN 4 WHEN 'pending' THEN 5 ELSE 0
            END, e.fast_requested,
            e.service_tier, e.input_tokens, e.output_tokens, e.reasoning_tokens,
            e.cached_input_tokens, e.cache_write_tokens, mc.cost_usd,
            e.started_at_unix_ms,
            strftime('%Y-%m-%d', e.started_at_unix_ms / 1000, 'unixepoch', 'localtime'),
            conversation.id IS NOT NULL, e.usage_source
         FROM usage_events AS e {CATALOG_COST_JOIN}
         LEFT JOIN conversations AS conversation ON conversation.id = e.conversation_id
         {} ORDER BY e.started_at_unix_ms DESC, e.event_id DESC LIMIT ?8",
        FILTER
    );
    let next_page_sql = format!(
        "SELECT e.event_id,
            e.event_kind = 'request', e.event_kind = 'legacy_output', e.conversation_id,
            e.provider_id,
            e.model_id, e.reasoning_effort, e.operation,
            CASE e.status
                WHEN 'completed' THEN 1 WHEN 'failed' THEN 2 WHEN 'cancelled' THEN 3
                WHEN 'interrupted' THEN 4 WHEN 'pending' THEN 5 ELSE 0
            END, e.fast_requested,
            e.service_tier, e.input_tokens, e.output_tokens, e.reasoning_tokens,
            e.cached_input_tokens, e.cache_write_tokens, mc.cost_usd,
            e.started_at_unix_ms,
            strftime('%Y-%m-%d', e.started_at_unix_ms / 1000, 'unixepoch', 'localtime'),
            conversation.id IS NOT NULL, e.usage_source
         FROM usage_events AS e {CATALOG_COST_JOIN}
         LEFT JOIN conversations AS conversation ON conversation.id = e.conversation_id
         {} AND (e.started_at_unix_ms < ?8 OR (
            e.started_at_unix_ms = ?8 AND e.event_id < ?9
         ))
         ORDER BY e.started_at_unix_ms DESC, e.event_id DESC LIMIT ?10",
        FILTER
    );

    let mut aggregate = StatisticsAggregation::default();
    let mut cursor: Option<(i64, String)> = None;
    loop {
        let mut parameters = filter_params(query).to_vec();
        let sql = if let Some((started_at_unix_ms, event_id)) = cursor.as_ref() {
            parameters.extend([
                (*started_at_unix_ms).into(),
                event_id.clone().into(),
                STATISTICS_AGGREGATION_PAGE_SIZE.into(),
            ]);
            &next_page_sql
        } else {
            parameters.push(STATISTICS_AGGREGATION_PAGE_SIZE.into());
            &first_page_sql
        };
        let mut statement = database.prepare(sql)?;
        let mut rows = statement.query(params_from_iter(parameters.iter()))?;
        let mut row_count = 0_i64;
        let mut last_event = None;
        while let Some(row) = rows.next()? {
            let event = AggregationEvent {
                event_id: row.get_str(0)?,
                is_request: row.get(1)?,
                is_legacy_output: row.get(2)?,
                conversation_id: row.get_str(3)?,
                conversation_exists: row.get(19)?,
                usage_source: row.get_str(20)?,
                provider_id: row.get_optional_str(4)?,
                model_id: row.get_optional_str(5)?,
                reasoning_effort: row.get_optional_str(6)?,
                operation: row.get_str(7)?,
                status_code: row.get(8)?,
                fast_requested: row.get(9)?,
                service_tier: row.get_optional_str(10)?,
                input_tokens: row.get(11)?,
                output_tokens: row.get(12)?,
                reasoning_tokens: row.get(13)?,
                cached_input_tokens: row.get(14)?,
                cache_write_tokens: row.get(15)?,
                estimated_cost_usd: row.get(16)?,
                started_at_unix_ms: row.get(17)?,
                day: row.get_optional_str(18)?,
            };
            aggregate.add(&event)?;
            row_count += 1;
            if row_count == STATISTICS_AGGREGATION_PAGE_SIZE {
                last_event = Some((event.started_at_unix_ms, event.event_id.to_owned()));
            }
        }
        drop(rows);
        drop(statement);
        if row_count < STATISTICS_AGGREGATION_PAGE_SIZE {
            break;
        }
        cursor = last_event;
    }
    Ok(aggregate)
}

/// Returns aggregated request usage and costs for the requested filters.
///
/// Statistics use cached pricing only so a remote catalog refresh cannot delay
/// local usage data. Estimated costs are included only when cached pricing
/// metadata matches; local storage failures are returned as service errors.
pub(crate) async fn get_statistics(
    storage: &AppStorage,
    query: &StatisticsQuery,
    _cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let catalog = models_dev_pricing::load_cached(storage)?;
    let mut database = storage.connect().map_err(|_| statistics_storage_error())?;
    catalog
        .install_temporary_prices(&mut database, query, None)
        .map_err(|_| statistics_storage_error())?;
    get_statistics_from_database(&database, query, catalog.metadata())
        .map_err(|_| statistics_storage_error())
}

/// Returns a page of filtered request details for CSV export.
pub(crate) fn get_statistics_details(
    storage: &AppStorage,
    query: &StatisticsQuery,
    include_total: bool,
) -> Result<Value, ServiceError> {
    let catalog = models_dev_pricing::load_cached(storage)?;
    let mut database = storage.connect().map_err(|_| statistics_storage_error())?;
    catalog
        .install_temporary_prices(&mut database, query, Some((query.limit, query.offset)))
        .map_err(|_| statistics_storage_error())?;
    let detail_count = include_total
        .then(|| count_details(&database, query))
        .transpose()
        .map_err(|_| statistics_storage_error())?;
    let request_details =
        request_details_for_export(&database, query).map_err(|_| statistics_storage_error())?;
    Ok(json!({
        "requestDetailCount": detail_count,
        "requestDetails": request_details,
    }))
}

/// Returns cached pricing and context metadata for one provider model.
pub(crate) async fn get_model_details(
    storage: &AppStorage,
    provider_id: &str,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    models_dev_pricing::model_details(storage, provider_id, model_id, cancellation).await
}

/// Returns pricing and context metadata for multiple provider models.
pub(crate) async fn get_model_details_batch(
    storage: &AppStorage,
    models: &[(String, String)],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    models_dev_pricing::model_details_batch(storage, models, cancellation).await
}

/// Fills missing per-model capabilities from the local models.dev cache.
/// Provider-reported values remain authoritative when they are present.
pub(crate) async fn enrich_model_catalog_capabilities(
    storage: &AppStorage,
    provider_id: &str,
    models: &mut [Value],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    if models.is_empty() {
        return Ok(());
    }
    if *cancellation.borrow_and_update() {
        return Err(model_catalog_enrichment_cancelled_error());
    }
    let requested = models
        .iter()
        .filter_map(|model| {
            model
                .get("id")
                .or_else(|| model.get("slug"))
                .and_then(Value::as_str)
                .map(|model_id| (provider_id.to_owned(), model_id.to_owned()))
        })
        .collect::<Vec<_>>();
    let details = models_dev_pricing::cached_model_details_batch(storage, &requested)?;
    tokio::task::yield_now().await;
    if *cancellation.borrow_and_update() {
        return Err(model_catalog_enrichment_cancelled_error());
    }
    let Some(details) = details.get("models").and_then(Value::as_array) else {
        return Ok(());
    };
    let mut details_by_model_id = HashMap::with_capacity(details.len());
    for entry in details {
        if let Some(model_id) = entry.get("modelId").and_then(Value::as_str) {
            details_by_model_id.entry(model_id).or_insert(entry);
        }
    }

    for model in models {
        let Some(model_id) = model
            .get("id")
            .or_else(|| model.get("slug"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let catalog_model = details_by_model_id
            .get(model_id)
            .and_then(|entry| entry.get("model"));
        let Some(catalog_model) = catalog_model else {
            continue;
        };
        for capability in ["supportsImages", "supportsTools", "supportsReasoning"] {
            if model.get(capability).is_none_or(Value::is_null)
                && let Some(value) = catalog_model
                    .get(capability)
                    .filter(|value| value.is_boolean())
            {
                model[capability] = value.clone();
            }
        }
        for (field, catalog_field) in [
            ("contextWindow", "contextWindow"),
            ("inputTokenLimit", "inputTokenLimit"),
            ("maxOutputTokens", "maximumOutput"),
        ] {
            if model.get(field).is_none_or(Value::is_null)
                && let Some(value) = catalog_model
                    .get(catalog_field)
                    .filter(|value| value.as_i64().is_some_and(|limit| limit > 0))
            {
                model[field] = value.clone();
            }
        }
    }
    Ok(())
}

fn model_catalog_enrichment_cancelled_error() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The model request was cancelled.",
        true,
    )
}

pub(crate) fn cached_model_supports_images(
    storage: &AppStorage,
    provider_id: &str,
    model_id: &str,
) -> Option<bool> {
    models_dev_pricing::cached_model_supports_images(storage, provider_id, model_id)
}

pub(crate) fn cached_models_support_images(
    storage: &AppStorage,
    provider_id: &str,
    model_ids: &[String],
) -> HashMap<String, bool> {
    models_dev_pricing::cached_models_support_images(storage, provider_id, model_ids)
}

fn statistics_storage_error() -> ServiceError {
    ServiceError::new(
        "usage_statistics_unavailable",
        "Local usage statistics could not be loaded.",
        true,
    )
}

fn get_statistics_from_database(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
    pricing_metadata: Value,
) -> rusqlite::Result<Value> {
    let aggregate = aggregate_usage_events(database, query)?;
    let local_model_measurements = local_model_measurements(database, query)?;
    let summary = aggregate.summary_json();
    let detail_count = aggregate.all_events.count;
    let provider_breakdown = sorted_breakdown(aggregate.providers, "providerId", 200);
    let model_breakdown = sorted_breakdown(aggregate.models, "modelId", 200);
    let reasoning_breakdown = sorted_breakdown(aggregate.reasoning_efforts, "reasoningEffort", 200);
    let operation_breakdown = sorted_breakdown(aggregate.operations, "operation", 200);
    let fast_mode_breakdown = fast_mode_values(aggregate.fast_modes);
    let service_tier_breakdown = sorted_service_tiers(aggregate.service_tiers);
    let daily_trend = trend_values(
        aggregate.daily_trends,
        aggregate.daily_trend_without_date,
        "day",
    );
    let monthly_trend = trend_values(
        aggregate.monthly_trends,
        aggregate.monthly_trend_without_date,
        "month",
    );
    let conversation_ranking = conversation_ranking(database, aggregate.conversations)?;
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
        "localModelMeasurements": local_model_measurements,
        "requestDetailCount": detail_count,
        "requestDetails": request_details,
        "filterOptions": filter_options,
        "quotaHistory": quota_history,
    }))
}

fn local_model_measurements(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<Value> {
    let measurable_request = "e.status = 'completed'
        AND e.usage_source = 'provider_reported'
        AND e.output_tokens > 0
        AND e.finished_at_unix_ms IS NOT NULL
        AND e.first_output_latency_ms IS NOT NULL
        AND e.first_output_latency_ms >= 0
        AND e.finished_at_unix_ms > e.started_at_unix_ms
        AND e.finished_at_unix_ms - e.started_at_unix_ms > e.first_output_latency_ms";
    let sql = format!(
        "SELECT COUNT(*),
            COALESCE(SUM(CASE WHEN {measurable_request} THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN {measurable_request} THEN e.output_tokens ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN {measurable_request}
                THEN e.finished_at_unix_ms - e.started_at_unix_ms - e.first_output_latency_ms
                ELSE 0 END), 0)
         FROM usage_events AS e
         {} AND e.event_kind = 'request' AND e.provider_id = 'llama_cpp'",
        FILTER
    );
    let values = filter_params(query);
    let (sampled_requests, measured_requests, output_tokens, generation_time_ms) = database
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
    let output_tokens_per_second = (measured_requests > 0 && generation_time_ms > 0)
        .then(|| (output_tokens as f64 * 1_000.0) / generation_time_ms as f64);
    Ok(json!({
        "localRequests": sampled_requests,
        "measuredRequests": measured_requests,
        "outputTokens": output_tokens,
        "generationTimeMs": generation_time_ms,
        "outputTokensPerSecond": output_tokens_per_second,
    }))
}

fn sorted_breakdown(groups: HashMap<String, UsageTotals>, key: &str, limit: usize) -> Vec<Value> {
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left_key, left), (right_key, right)| {
        (i128::from(right.input_tokens) + i128::from(right.output_tokens))
            .cmp(&(i128::from(left.input_tokens) + i128::from(left.output_tokens)))
            .then_with(|| left_key.cmp(right_key))
    });
    groups
        .into_iter()
        .take(limit)
        .map(|(group, totals)| totals.to_breakdown_json(key, &group))
        .collect()
}

fn fast_mode_values(groups: HashMap<Option<bool>, UsageTotals>) -> Vec<Value> {
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left, _), (right, _)| right.cmp(left));
    groups
        .into_iter()
        .map(|(fast_requested, totals)| {
            json!({
                "fastRequested": fast_requested,
                "requests": totals.count,
                "inputTokens": totals.input_tokens,
                "outputTokens": totals.output_tokens,
                "estimatedCostUsd": totals.estimated_cost_usd,
                "estimatedCostRequests": totals.estimated_cost_requests,
            })
        })
        .collect()
}

fn sorted_service_tiers(groups: HashMap<String, UsageTotals>) -> Vec<Value> {
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left_key, left), (right_key, right)| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left_key.cmp(right_key))
    });
    groups
        .into_iter()
        .map(|(service_tier, totals)| totals.to_breakdown_json("serviceTier", &service_tier))
        .collect()
}

fn trend_values(
    groups: HashMap<String, TrendTotals>,
    without_date: Option<TrendTotals>,
    key: &str,
) -> Vec<Value> {
    let mut groups = groups
        .into_iter()
        .map(|(date, totals)| (Some(date), totals))
        .collect::<Vec<_>>();
    if let Some(totals) = without_date {
        groups.push((None, totals));
    }
    groups.sort_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));
    groups
        .into_iter()
        .map(|(date, totals)| {
            json!({
                key: date,
                "inputTokens": totals.usage.input_tokens,
                "outputTokens": totals.usage.output_tokens,
                "estimatedCostUsd": totals.usage.estimated_cost_usd,
                "estimatedCostRequests": totals.usage.estimated_cost_requests,
                "requests": totals.requests,
            })
        })
        .collect()
}
fn conversation_ranking(
    database: &StorageConnection<'_>,
    groups: HashMap<String, ConversationTotals>,
) -> rusqlite::Result<Vec<Value>> {
    let mut groups = groups
        .into_iter()
        .filter(|(_, totals)| totals.conversation_exists)
        .collect::<Vec<_>>();
    groups.sort_by(|(left_id, left), (right_id, right)| {
        (i128::from(right.usage.input_tokens) + i128::from(right.usage.output_tokens))
            .cmp(&(i128::from(left.usage.input_tokens) + i128::from(left.usage.output_tokens)))
            .then_with(|| right.last_event_at_unix_ms.cmp(&left.last_event_at_unix_ms))
            .then_with(|| left_id.cmp(right_id))
    });
    let groups = groups.into_iter().take(20).collect::<Vec<_>>();
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let parameters = groups
        .iter()
        .map(|(conversation_id, _)| conversation_id.clone().into())
        .collect::<Vec<rusqlite::types::Value>>();
    let placeholders = (1..=groups.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT id, title FROM conversations WHERE id IN ({placeholders})");
    let titles = database
        .query_map(&sql, params_from_iter(parameters.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;

    Ok(groups
        .into_iter()
        .filter_map(|(conversation_id, totals)| {
            let title = titles.get(&conversation_id)?.clone();
            let last_request_at = totals.last_event_at_unix_ms?;
            Some(json!({
                "conversationId": conversation_id,
                "title": title,
                "requests": totals.request_count,
                "inputTokens": totals.usage.input_tokens,
                "outputTokens": totals.usage.output_tokens,
                "estimatedCostUsd": totals.usage.estimated_cost_usd,
                "estimatedCostRequests": totals.usage.estimated_cost_requests,
                "lastRequestAtUnixMs": last_request_at,
            }))
        })
        .collect())
}
fn count_details(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM usage_events AS e {}", FILTER);
    database.query_row(&sql, filter_params(query), |row| row.get(0))
}

fn request_details(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.event_id, e.event_kind, e.conversation_id, conversation.title,
            e.assistant_message_id, e.provider_id, e.model_id, e.reasoning_effort,
            e.operation, e.status, e.fast_requested, e.service_tier,
            e.input_tokens, e.output_tokens, e.reasoning_tokens, e.cached_input_tokens,
            e.cache_write_tokens, e.reported_cost_nano_usd, e.cost_source, e.usage_source,
            e.started_at_unix_ms, e.finished_at_unix_ms, mc.cost_usd,
            e.request_manifest_json, e.run_id, e.first_output_latency_ms,
            e.connection_id, e.workspace_id, e.project_id, e.parent_run_id,
            e.provider_processing_ms, e.provider_queue_time_ms,
            e.provider_input_processing_ms, e.provider_output_processing_ms,
            e.completion_reason
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
                "usageSource": row.get::<_, String>(19)?,
                "startedAtUnixMs": row.get::<_, i64>(20)?,
                "finishedAtUnixMs": row.get::<_, Option<i64>>(21)?,
                "estimatedCostUsd": row.get::<_, Option<f64>>(22)?,
                "requestManifest": request_manifest,
                "runId": row.get::<_, Option<String>>(24)?,
                "firstOutputLatencyMs": row.get::<_, Option<i64>>(25)?,
                "connectionId": row.get::<_, Option<String>>(26)?,
                "workspaceId": row.get::<_, Option<String>>(27)?,
                "projectId": row.get::<_, Option<String>>(28)?,
                "parentRunId": row.get::<_, Option<String>>(29)?,
                "providerProcessingMs": row.get::<_, Option<i64>>(30)?,
                "providerQueueTimeMs": row.get::<_, Option<i64>>(31)?,
                "providerInputProcessingMs": row.get::<_, Option<i64>>(32)?,
                "providerOutputProcessingMs": row.get::<_, Option<i64>>(33)?,
                "completionReason": row.get::<_, Option<String>>(34)?,
            }))
        })?
        .collect()
}

fn request_details_for_export(
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT e.event_kind, e.started_at_unix_ms, conversation.id, conversation.title,
            e.provider_id, e.model_id, e.reasoning_effort, e.operation, e.status,
            e.fast_requested, e.input_tokens, e.output_tokens, e.reasoning_tokens,
            e.cached_input_tokens, e.cache_write_tokens, mc.cost_usd,
            e.usage_source, e.run_id, e.first_output_latency_ms, e.finished_at_unix_ms,
            e.connection_id, e.workspace_id, e.project_id, e.parent_run_id,
            e.provider_processing_ms, e.provider_queue_time_ms,
            e.provider_input_processing_ms, e.provider_output_processing_ms,
            e.completion_reason
         FROM usage_events AS e
         {PAGE_CATALOG_COST_JOIN}
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
            Ok(json!({
                "eventKind": row.get::<_, String>(0)?,
                "startedAtUnixMs": row.get::<_, i64>(1)?,
                "conversationId": row.get::<_, String>(2)?,
                "conversationTitle": row.get::<_, String>(3)?,
                "providerId": row.get::<_, Option<String>>(4)?,
                "modelId": row.get::<_, Option<String>>(5)?,
                "reasoningEffort": row.get::<_, Option<String>>(6)?,
                "operation": row.get::<_, String>(7)?,
                "status": row.get::<_, String>(8)?,
                "fastRequested": row.get::<_, Option<bool>>(9)?,
                "inputTokens": row.get::<_, Option<i64>>(10)?,
                "outputTokens": row.get::<_, Option<i64>>(11)?,
                "reasoningTokens": row.get::<_, Option<i64>>(12)?,
                "cachedInputTokens": row.get::<_, Option<i64>>(13)?,
                "cacheWriteTokens": row.get::<_, Option<i64>>(14)?,
                "estimatedCostUsd": row.get::<_, Option<f64>>(15)?,
                "usageSource": row.get::<_, String>(16)?,
                "runId": row.get::<_, Option<String>>(17)?,
                "firstOutputLatencyMs": row.get::<_, Option<i64>>(18)?,
                "finishedAtUnixMs": row.get::<_, Option<i64>>(19)?,
                "connectionId": row.get::<_, Option<String>>(20)?,
                "workspaceId": row.get::<_, Option<String>>(21)?,
                "projectId": row.get::<_, Option<String>>(22)?,
                "parentRunId": row.get::<_, Option<String>>(23)?,
                "providerProcessingMs": row.get::<_, Option<i64>>(24)?,
                "providerQueueTimeMs": row.get::<_, Option<i64>>(25)?,
                "providerInputProcessingMs": row.get::<_, Option<i64>>(26)?,
                "providerOutputProcessingMs": row.get::<_, Option<i64>>(27)?,
                "completionReason": row.get::<_, Option<String>>(28)?,
            }))
        })?
        .collect()
}

fn filter_options(database: &StorageConnection<'_>) -> rusqlite::Result<Value> {
    fn distinct_values(
        database: &StorageConnection<'_>,
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
    database: &StorageConnection<'_>,
    query: &StatisticsQuery,
) -> rusqlite::Result<Vec<Value>> {
    const MAX_QUOTA_HISTORY_SNAPSHOTS: i64 = 100;
    let mut statement = database.prepare(
        "SELECT snapshot.id, snapshot.connection_id, snapshot.workspace_id,
            connection.display_name, workspace.display_name, snapshot.fetched_at_unix_ms,
            snapshot.freshness, snapshot.ordinary_usage_allowed,
            snapshot.reset_credit_count, snapshot.reset_credit_details_state,
            bucket.limit_id, bucket.used_percent, bucket.window_seconds,
            bucket.reset_at_unix_ms
         FROM chatgpt_usage_snapshots AS snapshot
         JOIN chatgpt_connections AS connection ON connection.id = snapshot.connection_id
         JOIN chatgpt_workspaces AS workspace
           ON workspace.connection_id = snapshot.connection_id
          AND workspace.id = snapshot.workspace_id
         LEFT JOIN chatgpt_usage_buckets AS bucket ON bucket.snapshot_id = snapshot.id
         WHERE snapshot.id IN (
            SELECT id FROM chatgpt_usage_snapshots
            WHERE fetched_at_unix_ms >= ?1 AND fetched_at_unix_ms < ?2
              AND (?3 IS NULL OR ?3 = 'chatgpt_api')
            ORDER BY fetched_at_unix_ms DESC, id DESC LIMIT ?4
         )
         ORDER BY snapshot.fetched_at_unix_ms DESC, snapshot.id, bucket.limit_id",
    )?;
    let rows = statement
        .query_map(
            params![
                query.from_unix_ms,
                query.to_unix_ms,
                query.provider_id.as_deref(),
                MAX_QUOTA_HISTORY_SNAPSHOTS
            ],
            |row| {
                let bucket = match row.get::<_, Option<String>>(10)? {
                    Some(limit_id) => Some(json!({
                        "limitId": limit_id,
                        "limit": null,
                        "remaining": null,
                        "usedPercent": row.get::<_, Option<f64>>(11)?,
                        "windowSeconds": row.get::<_, Option<i64>>(12)?,
                        "resetAtUnixMs": row.get::<_, Option<i64>>(13)?,
                    })),
                    None => None,
                };
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(5)?,
                    json!({
                        "source": "chatgpt",
                        "providerId": null,
                        "modelId": null,
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
                    bucket,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let mut snapshots: Vec<(String, i64, Value, Vec<Value>)> = Vec::new();
    for (snapshot_id, fetched_at, snapshot, bucket) in rows {
        if snapshots
            .last()
            .is_none_or(|(current_id, _, _, _)| current_id != &snapshot_id)
        {
            snapshots.push((snapshot_id.clone(), fetched_at, snapshot, Vec::new()));
        }
        if let Some(bucket) = bucket
            && let Some((_, _, _, buckets)) = snapshots.last_mut()
        {
            buckets.push(bucket);
        }
    }

    let mut history = snapshots
        .into_iter()
        .map(|(_, fetched_at, mut snapshot, buckets)| {
            snapshot["buckets"] = json!(buckets);
            (fetched_at, snapshot)
        })
        .collect::<Vec<_>>();

    let now = now_unix_millis()?;
    let mut statement = database.prepare(
        "SELECT e.provider_id, e.model_id, e.connection_id, e.workspace_id,
            json_extract(e.provider_rate_limits_json, '$.observedAtUnixMs'),
            json_extract(e.provider_rate_limits_json, '$.requestLimit'),
            json_extract(e.provider_rate_limits_json, '$.requestRemaining'),
            json_extract(e.provider_rate_limits_json, '$.requestResetAtUnixMs'),
            json_extract(e.provider_rate_limits_json, '$.tokenLimit'),
            json_extract(e.provider_rate_limits_json, '$.tokenRemaining'),
            json_extract(e.provider_rate_limits_json, '$.tokenResetAtUnixMs')
         FROM usage_events AS e
         WHERE e.provider_rate_limits_json IS NOT NULL
           AND e.provider_id IN ('openai', 'groq')
           AND json_extract(e.provider_rate_limits_json, '$.observedAtUnixMs') >= ?1
           AND json_extract(e.provider_rate_limits_json, '$.observedAtUnixMs') < ?2
           AND (?3 IS NULL OR e.provider_id = ?3)
           AND (?4 IS NULL OR e.model_id = ?4)
           AND json_type(e.provider_rate_limits_json, '$.observedAtUnixMs') = 'integer'
           AND json_extract(e.provider_rate_limits_json, '$.observedAtUnixMs') >= 0
         ORDER BY json_extract(e.provider_rate_limits_json, '$.observedAtUnixMs') DESC,
            e.event_id DESC LIMIT ?5",
    )?;
    let provider_rows = statement
        .query_map(
            params![
                query.from_unix_ms,
                query.to_unix_ms,
                query.provider_id.as_deref(),
                query.model_id.as_deref(),
                MAX_QUOTA_HISTORY_SNAPSHOTS,
            ],
            |row| {
                let provider_id = row.get::<_, String>(0)?;
                let model_id = row.get::<_, Option<String>>(1)?;
                let connection_id = row.get::<_, Option<String>>(2)?;
                let workspace_id = row.get::<_, Option<String>>(3)?;
                let observed_at = row.get::<_, i64>(4)?;
                let request_limit = row.get::<_, Option<i64>>(5)?.filter(|value| *value >= 0);
                let request_remaining = row.get::<_, Option<i64>>(6)?.filter(|value| *value >= 0);
                let request_reset_at = row.get::<_, Option<i64>>(7)?.filter(|value| *value >= 0);
                let token_limit = row.get::<_, Option<i64>>(8)?.filter(|value| *value >= 0);
                let token_remaining = row.get::<_, Option<i64>>(9)?.filter(|value| *value >= 0);
                let token_reset_at = row.get::<_, Option<i64>>(10)?.filter(|value| *value >= 0);
                let bucket = |limit_id: &str,
                              limit: Option<i64>,
                              remaining: Option<i64>,
                              reset_at_unix_ms: Option<i64>| {
                    if limit.is_none() && remaining.is_none() && reset_at_unix_ms.is_none() {
                        return None;
                    }
                    let used_percent = match (limit, remaining) {
                        (Some(limit), Some(remaining)) if limit > 0 && remaining <= limit => {
                            Some((limit - remaining) as f64 / limit as f64 * 100.0)
                        }
                        _ => None,
                    };
                    Some(json!({
                        "limitId": limit_id,
                        "limit": limit,
                        "remaining": remaining,
                        "usedPercent": used_percent,
                        "windowSeconds": null,
                        "resetAtUnixMs": reset_at_unix_ms,
                    }))
                };
                let mut buckets = Vec::new();
                if let Some(bucket) = bucket(
                    "requests",
                    request_limit,
                    request_remaining,
                    request_reset_at,
                ) {
                    buckets.push(bucket);
                }
                if let Some(bucket) = bucket("tokens", token_limit, token_remaining, token_reset_at)
                {
                    buckets.push(bucket);
                }
                let freshness = if observed_at < 0 || observed_at > now {
                    "unknown"
                } else if now.saturating_sub(observed_at) <= 60_000 {
                    "current"
                } else {
                    "stale"
                };
                Ok((
                    observed_at,
                    json!({
                        "source": "provider_response",
                        "providerId": provider_id,
                        "modelId": model_id,
                        "connectionId": connection_id,
                        "workspaceId": workspace_id,
                        "connectionName": null,
                        "workspaceName": null,
                        "fetchedAtUnixMs": observed_at,
                        "freshness": freshness,
                        "ordinaryUsageAllowed": null,
                        "buckets": buckets,
                    }),
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    history.extend(provider_rows);
    history.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.0));
    history.truncate(MAX_QUOTA_HISTORY_SNAPSHOTS as usize);
    Ok(history.into_iter().map(|(_, snapshot)| snapshot).collect())
}

#[cfg(test)]
mod tests;
