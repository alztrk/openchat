pub(crate) use super::*;

#[cfg(test)]
mod local_model_measurement_tests {
    use std::{fs, path::PathBuf};

    use crate::storage::AppStorage;

    use super::{StatisticsQuery, local_model_measurements};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-local-model-statistics-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    fn query(model_id: Option<&str>) -> StatisticsQuery {
        StatisticsQuery {
            from_unix_ms: 0,
            to_unix_ms: 10_000,
            provider_id: None,
            model_id: model_id.map(str::to_owned),
            operation: None,
            reasoning_effort: None,
            fast_mode: None,
            offset: 0,
            limit: 1,
        }
    }

    #[test]
    fn local_model_rate_uses_only_completed_provider_reported_output_and_timing() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        let database = storage.connect().expect("connect test storage");
        database
            .execute_batch(
                "CREATE TABLE usage_events (
                    event_kind TEXT NOT NULL,
                    provider_id TEXT,
                    model_id TEXT,
                    operation TEXT NOT NULL,
                    reasoning_effort TEXT,
                    fast_requested INTEGER,
                    status TEXT NOT NULL,
                    usage_source TEXT NOT NULL,
                    output_tokens INTEGER,
                    started_at_unix_ms INTEGER NOT NULL,
                    finished_at_unix_ms INTEGER,
                    first_output_latency_ms INTEGER,
                    provider_rate_limits_json TEXT
                 );
                 INSERT INTO usage_events (
                    event_kind, provider_id, model_id, operation, reasoning_effort,
                    fast_requested, status, usage_source, output_tokens,
                    started_at_unix_ms, finished_at_unix_ms, first_output_latency_ms
                 ) VALUES
                    ('request', 'llama_cpp', 'model-a', 'chat', NULL, 0, 'completed', 'provider_reported', 40, 1000, 3500, 500),
                    ('request', 'llama_cpp', 'model-a', 'chat', NULL, 0, 'completed', 'estimated', 100, 1000, 2500, 500),
                    ('request', 'llama_cpp', 'model-b', 'chat', NULL, 0, 'failed', 'provider_reported', 100, 1000, 2500, 500),
                    ('request', 'openai', 'model-a', 'chat', NULL, 0, 'completed', 'provider_reported', 400, 1000, 2000, 100);",
            )
            .expect("create isolated usage event schema");

        let all_local = local_model_measurements(&database, &query(None))
            .expect("aggregate local model measurements");
        assert_eq!(all_local["localRequests"], 3);
        assert_eq!(all_local["measuredRequests"], 1);
        assert_eq!(all_local["outputTokens"], 40);
        assert_eq!(all_local["generationTimeMs"], 2_000);
        assert_eq!(all_local["outputTokensPerSecond"], 20.0);

        let unmeasured_model = local_model_measurements(&database, &query(Some("model-b")))
            .expect("aggregate filtered local model measurements");
        assert_eq!(unmeasured_model["localRequests"], 1);
        assert_eq!(unmeasured_model["measuredRequests"], 0);
        assert_eq!(
            unmeasured_model["outputTokensPerSecond"],
            serde_json::Value::Null
        );
    }
}

#[cfg(test)]
mod usage_data_tests {
    use serde_json::json;

    use super::{AggregationEvent, STATUS_CODE_COMPLETED, UsageData, UsageTotals};

    #[test]
    fn cache_read_share_uses_only_complete_provider_reported_pairs() {
        fn event<'a>(
            event_id: &'a str,
            usage_source: &'a str,
            input_tokens: Option<i64>,
            cached_input_tokens: Option<i64>,
        ) -> AggregationEvent<'a> {
            AggregationEvent {
                is_request: true,
                is_legacy_output: false,
                event_id,
                conversation_id: "conversation",
                conversation_exists: true,
                provider_id: Some("provider"),
                model_id: Some("model"),
                reasoning_effort: None,
                operation: "chat",
                status_code: STATUS_CODE_COMPLETED,
                fast_requested: None,
                service_tier: None,
                input_tokens,
                output_tokens: None,
                reasoning_tokens: None,
                cached_input_tokens,
                cache_write_tokens: None,
                usage_source,
                estimated_cost_usd: None,
                started_at_unix_ms: 1,
                day: Some("2026-10-27"),
            }
        }

        let mut totals = UsageTotals::default();
        for request in [
            event("reported", "provider_reported", Some(100), Some(40)),
            event("missing-cache", "provider_reported", Some(200), None),
            event("estimated", "estimated", Some(80), Some(20)),
            event("inconsistent", "provider_reported", Some(10), Some(11)),
        ] {
            totals.add(&request).expect("aggregate usage totals");
        }

        assert_eq!(totals.cache_share_input_tokens, 100);
        assert_eq!(totals.cache_share_cached_tokens, 40);
        assert_eq!(totals.cache_share_requests, 1);
        let breakdown = totals.to_breakdown_json("providerId", "provider");
        assert_eq!(breakdown["cacheShareInputTokens"], json!(100));
        assert_eq!(breakdown["cacheShareCachedTokens"], json!(40));
        assert_eq!(breakdown["cacheShareRequests"], json!(1));
    }

    #[test]
    fn completion_reasons_are_read_only_from_bounded_provider_fields() {
        assert_eq!(
            UsageData::from_chat_completion_event(
                &json!({"choices": [{"finish_reason": "tool_calls"}]}),
                "openai",
            )
            .completion_reason
            .as_deref(),
            Some("tool_calls")
        );
        assert_eq!(
            UsageData::from_chat_completion_event(
                &json!({"candidates": [{"finishReason": "MAX_TOKENS"}]}),
                "gemini",
            )
            .completion_reason
            .as_deref(),
            Some("MAX_TOKENS")
        );
        assert_eq!(
            UsageData::from_messages_event(
                &json!({"delta": {"stop_reason": "tool_use"}}),
                "anthropic",
            )
            .completion_reason
            .as_deref(),
            Some("tool_use")
        );
        assert_eq!(
            UsageData::from_responses_event(
                &json!({"response": {"incomplete_details": {"reason": "max_output_tokens"}}}),
                "chatgpt_api",
            )
            .completion_reason
            .as_deref(),
            Some("max_output_tokens")
        );
        assert_eq!(
            UsageData::from_chat_completion_event(
                &json!({"choices": [{"finish_reason": "reason with spaces"}]}),
                "openai",
            )
            .completion_reason,
            None
        );
        assert_eq!(
            UsageData::from_chat_completion_event(
                &json!({"choices": [{"finish_reason": "x".repeat(65)}]}),
                "openai",
            )
            .completion_reason,
            None
        );
    }

    #[test]
    fn responses_usage_maps_reasoning_cache_and_service_tier() {
        let usage = UsageData::from_responses_event(
            &json!({
                "response": {
                    "usage": {
                        "input_tokens": 120,
                        "output_tokens": 35,
                        "output_tokens_details": {"reasoning_tokens": 9},
                        "input_tokens_details": {
                            "cached_tokens": 40,
                            "cache_write_tokens": 6
                        },
                        "cost_usd": 0.002
                    },
                    "service_tier": "priority"
                }
            }),
            "openai",
        );

        assert_eq!(usage.input_tokens, Some(120));
        assert_eq!(usage.output_tokens, Some(35));
        assert_eq!(usage.reasoning_tokens, Some(9));
        assert_eq!(usage.cached_input_tokens, Some(40));
        assert_eq!(usage.cache_write_tokens, Some(6));
        assert_eq!(usage.reported_cost_usd, Some(0.002));
        assert_eq!(usage.service_tier.as_deref(), Some("priority"));
    }

    #[test]
    fn chat_completion_usage_maps_reasoning_cache_and_reported_cost() {
        let usage = UsageData::from_chat_completion_event(
            &json!({
                "usage": {
                    "prompt_tokens": 200,
                    "completion_tokens": 25,
                    "completion_tokens_details": {"reasoning_tokens": 8},
                    "prompt_tokens_details": {
                        "cached_tokens": 70,
                        "cache_write_tokens": 4
                    },
                    "total_cost_usd": 0.003
                },
                "service_tier": "flex"
            }),
            "custom_openai",
        );

        assert_eq!(usage.input_tokens, Some(200));
        assert_eq!(usage.output_tokens, Some(25));
        assert_eq!(usage.reasoning_tokens, Some(8));
        assert_eq!(usage.cached_input_tokens, Some(70));
        assert_eq!(usage.cache_write_tokens, Some(4));
        assert_eq!(usage.reported_cost_usd, Some(0.003));
        assert_eq!(usage.service_tier.as_deref(), Some("flex"));
    }

    #[test]
    fn groq_chat_completion_usage_maps_provider_timing_fields() {
        let usage = UsageData::from_chat_completion_event(
            &json!({
                "usage": {
                    "prompt_tokens": 20,
                    "completion_tokens": 5,
                    "queue_time": 0.026959759,
                    "prompt_time": 0.009995497,
                    "completion_time": 5.57691751,
                    "total_time": 5.586913007
                }
            }),
            "groq",
        );

        assert_eq!(usage.provider_queue_time_ms, Some(27));
        assert_eq!(usage.provider_input_processing_ms, Some(10));
        assert_eq!(usage.provider_output_processing_ms, Some(5_577));
        assert_eq!(usage.provider_processing_ms, Some(5_587));

        let other_provider = UsageData::from_chat_completion_event(
            &json!({"usage": {"completion_time": 1.0, "total_time": 1.0}}),
            "openrouter",
        );
        assert_eq!(other_provider.provider_output_processing_ms, None);
        assert_eq!(other_provider.provider_processing_ms, None);
    }

    #[test]
    fn groq_timing_values_reject_negative_nonfinite_and_overflowing_durations() {
        let usage = UsageData::from_chat_completion_event(
            &json!({
                "usage": {
                    "queue_time": -1.0,
                    "prompt_time": "1.5",
                    "completion_time": 1e100,
                    "total_time": 0.0
                }
            }),
            "groq",
        );

        assert_eq!(usage.provider_queue_time_ms, None);
        assert_eq!(usage.provider_input_processing_ms, None);
        assert_eq!(usage.provider_output_processing_ms, None);
        assert_eq!(usage.provider_processing_ms, Some(0));
    }

    #[test]
    fn messages_usage_maps_cache_reads_and_writes_without_inventing_reasoning() {
        let usage = UsageData::from_messages_event(
            &json!({
                "usage": {
                    "input_tokens": 300,
                    "output_tokens": 45,
                    "cache_read_input_tokens": 100,
                    "cache_creation_input_tokens": 20
                }
            }),
            "anthropic",
        );

        assert_eq!(usage.input_tokens, Some(420));
        assert_eq!(usage.output_tokens, Some(45));
        assert_eq!(usage.reasoning_tokens, None);
        assert_eq!(usage.cached_input_tokens, Some(100));
        assert_eq!(usage.cache_write_tokens, Some(20));
        assert_eq!(usage.reported_cost_usd, None);
        assert_eq!(usage.service_tier, None);
    }

    #[test]
    fn messages_usage_requires_valid_complete_input_counts_for_total() {
        let missing_input = UsageData::from_messages_event(
            &json!({
                "usage": {
                    "cache_read_input_tokens": 100,
                    "cache_creation_input_tokens": 20
                }
            }),
            "anthropic",
        );
        assert_eq!(missing_input.input_tokens, None);
        assert_eq!(missing_input.cached_input_tokens, Some(100));
        assert_eq!(missing_input.cache_write_tokens, Some(20));

        let overflowing_total = UsageData::from_messages_event(
            &json!({
                "usage": {
                    "input_tokens": i64::MAX,
                    "cache_read_input_tokens": 1
                }
            }),
            "anthropic",
        );
        assert_eq!(overflowing_total.input_tokens, None);

        let invalid_cache_count = UsageData::from_messages_event(
            &json!({
                "usage": {
                    "input_tokens": 300,
                    "cache_read_input_tokens": -1
                }
            }),
            "anthropic",
        );
        assert_eq!(invalid_cache_count.input_tokens, None);
    }

    #[test]
    fn provider_specific_reported_costs_are_converted_or_scoped() {
        let xai = UsageData::from_chat_completion_event(
            &json!({"usage": {"cost_in_usd_ticks": 2_000_000}}),
            "xai",
        );
        let openrouter =
            UsageData::from_chat_completion_event(&json!({"usage": {"cost": 0.004}}), "openrouter");
        let other = UsageData::from_chat_completion_event(
            &json!({"usage": {"cost": 0.004}}),
            "custom_openai",
        );

        assert_eq!(xai.reported_cost_usd, Some(0.0002));
        assert_eq!(openrouter.reported_cost_usd, Some(0.004));
        assert_eq!(other.reported_cost_usd, None);
    }
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
            instruction_source_categories(Some("shared"), Some("project"), &[], None, None, true,),
            ["shared_preferences", "project_instructions", "goal_mode"]
        );
        assert!(
            instruction_source_categories(Some(" \n"), None, &[], None, None, false).is_empty()
        );
    }

    #[test]
    fn instruction_source_categories_record_project_skills_separately() {
        let skills = [crate::project_skills::ProjectSkill {
            id: "rust-style".to_owned(),
            content: "Use rustfmt.".to_owned(),
        }];
        assert_eq!(
            instruction_source_categories(None, None, &skills, None, None, false),
            ["project_skills"]
        );
    }

    #[test]
    fn instruction_source_categories_record_project_index_separately() {
        assert_eq!(
            instruction_source_categories(None, None, &[], None, Some("excerpt"), false),
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

#[cfg(test)]
mod request_tracker_tests {
    use std::{fs, path::PathBuf, time::Duration};

    use crate::storage::AppStorage;

    use super::{
        ProviderRateLimitObservation, StatisticsQuery, UsageData, UsageRequestDetails,
        UsageRequestTracker, quota_history, request_details, request_details_for_export,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-usage-request-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    #[test]
    fn request_tracker_records_optional_run_id() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage
            .connect()
            .expect("connect to test storage")
            .execute_batch(
                "CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    project_id TEXT
                 );
                 CREATE TABLE IF NOT EXISTS chatgpt_connections (
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name TEXT
                 );
                 CREATE TABLE IF NOT EXISTS chatgpt_workspaces (
                    connection_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    display_name TEXT,
                    PRIMARY KEY (connection_id, id)
                 );
                 CREATE TABLE IF NOT EXISTS chatgpt_usage_snapshots (
                    id TEXT PRIMARY KEY NOT NULL,
                    connection_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    freshness TEXT NOT NULL,
                    ordinary_usage_allowed INTEGER,
                    reset_credit_count INTEGER,
                    reset_credit_details_state TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS chatgpt_usage_buckets (
                    snapshot_id TEXT NOT NULL,
                    limit_id TEXT NOT NULL,
                    used_percent REAL,
                    window_seconds INTEGER,
                    reset_at_unix_ms INTEGER,
                    PRIMARY KEY (snapshot_id, limit_id)
                 );
                 CREATE TABLE IF NOT EXISTS usage_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    event_kind TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    assistant_message_id TEXT,
                    provider_id TEXT,
                    model_id TEXT,
                    reasoning_effort TEXT,
                    operation TEXT NOT NULL,
                    status TEXT NOT NULL,
                    fast_requested INTEGER,
                    service_tier TEXT,
                    input_tokens INTEGER,
                    output_tokens INTEGER,
                    reasoning_tokens INTEGER,
                    cached_input_tokens INTEGER,
                    cache_write_tokens INTEGER,
                    reported_cost_nano_usd INTEGER,
                    cost_source TEXT,
                    usage_source TEXT NOT NULL,
                    started_at_unix_ms INTEGER NOT NULL,
                    finished_at_unix_ms INTEGER,
                    request_manifest_json TEXT,
                    first_output_latency_ms INTEGER,
                    provider_processing_ms INTEGER,
                    provider_queue_time_ms INTEGER,
                    provider_input_processing_ms INTEGER,
                    provider_output_processing_ms INTEGER,
                    completion_reason TEXT,
                    run_id TEXT,
                    connection_id TEXT,
                    workspace_id TEXT,
                    project_id TEXT,
                    parent_run_id TEXT,
                    provider_rate_limits_json TEXT
                 );
                 CREATE TEMP TABLE models_dev_event_costs (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    cost_usd REAL NOT NULL
                 );
                 CREATE TEMP TABLE models_dev_page_event_costs (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    cost_usd REAL NOT NULL
                 );
                 INSERT INTO conversations (id, title, project_id)
                 VALUES ('conversation-1', 'Conversation', 'conversation-project');
                 INSERT INTO chatgpt_connections (
                    id, credential_reference, auth_status, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES (
                    'quota-connection', 'test-credential-reference', 'active', 1, 1
                 );
                 INSERT INTO chatgpt_workspaces (
                    id, connection_id, external_workspace_id, is_selected, fetched_at_unix_ms
                 ) VALUES (
                    'quota-workspace', 'quota-connection', 'external-workspace', 1, 1
                 );
                 INSERT INTO chatgpt_usage_snapshots (
                    id, connection_id, workspace_id, fetched_at_unix_ms, freshness,
                    ordinary_usage_allowed, reset_credit_details_state
                 ) VALUES (
                    'chatgpt-snapshot', 'quota-connection', 'quota-workspace', 10_500,
                    'current', 1, 'omitted'
                 );",
            )
            .expect("create isolated usage event schema");

        let requests = [
            UsageRequestDetails {
                conversation_id: "conversation-1",
                assistant_message_id: None,
                provider_id: "openai",
                model_id: "model",
                reasoning_effort: None,
                operation: "chat",
                fast_requested: false,
                run_id: None,
                connection_id: None,
                workspace_id: None,
                project_id: None,
                parent_run_id: None,
            },
            UsageRequestDetails {
                conversation_id: "conversation-1",
                assistant_message_id: None,
                provider_id: "groq",
                model_id: "model",
                reasoning_effort: None,
                operation: "tool_follow_up",
                fast_requested: false,
                run_id: Some("child-run-1"),
                connection_id: Some("connection-1"),
                workspace_id: Some("workspace-1"),
                project_id: Some("route-project"),
                parent_run_id: Some("parent-run-1"),
            },
        ];

        for (request, expected) in requests.into_iter().zip([
            (None, None, None, Some("conversation-project"), None),
            (
                Some("child-run-1"),
                Some("connection-1"),
                Some("workspace-1"),
                Some("route-project"),
                Some("parent-run-1"),
            ),
        ]) {
            let mut tracker =
                UsageRequestTracker::start(&storage, request).expect("record usage request");
            if expected.0.is_none() {
                tracker
                    .complete(&UsageData {
                        provider_processing_ms: Some(5_587),
                        provider_queue_time_ms: Some(27),
                        provider_input_processing_ms: Some(10),
                        provider_output_processing_ms: Some(5_577),
                        completion_reason: Some("tool_calls".to_owned()),
                        provider_rate_limits: Some(ProviderRateLimitObservation {
                            observed_at_unix_ms: 10_000,
                            request_limit: Some(60),
                            request_remaining: Some(58),
                            request_reset_at_unix_ms: Some(11_000),
                            token_limit: Some(150_000),
                            token_remaining: Some(149_984),
                            token_reset_at_unix_ms: Some(361_000),
                        }),
                        ..UsageData::default()
                    })
                    .expect("finish request with provider processing time");
            } else {
                tracker
                    .fail(&UsageData {
                        provider_rate_limits: Some(ProviderRateLimitObservation {
                            observed_at_unix_ms: 20_000,
                            request_limit: Some(60),
                            request_remaining: Some(0),
                            request_reset_at_unix_ms: Some(80_000),
                            token_limit: Some(150_000),
                            token_remaining: Some(0),
                            token_reset_at_unix_ms: Some(380_000),
                        }),
                        ..UsageData::default()
                    })
                    .expect("finish failed request with provider rate limits");
            }
            let database = storage.connect().expect("connect to test storage");
            if expected.0.is_none() {
                let rate_limits = database
                    .query_row(
                        "SELECT provider_rate_limits_json FROM usage_events WHERE event_id = ?1",
                        [&tracker.event_id],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .expect("read provider rate-limit observation")
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
                    .expect("decode provider rate-limit observation");
                assert_eq!(rate_limits["observedAtUnixMs"], 10_000);
                assert_eq!(rate_limits["requestRemaining"], 58);
                assert_eq!(rate_limits["tokenResetAtUnixMs"], 361_000);
                let history = quota_history(
                    &database,
                    &StatisticsQuery {
                        from_unix_ms: 9_000,
                        to_unix_ms: 11_000,
                        provider_id: Some("openai".to_owned()),
                        model_id: None,
                        operation: None,
                        reasoning_effort: None,
                        fast_mode: None,
                        offset: 0,
                        limit: 100,
                    },
                )
                .expect("load provider rate-limit history");
                assert_eq!(history.len(), 1);
                assert_eq!(history[0]["source"], "provider_response");
                assert_eq!(history[0]["providerId"], "openai");
                assert_eq!(history[0]["modelId"], "model");
                assert_eq!(history[0]["buckets"][0]["limitId"], "requests");
                assert_eq!(history[0]["buckets"][0]["limit"], 60);
                assert_eq!(history[0]["buckets"][0]["remaining"], 58);
                let chatgpt_history = quota_history(
                    &database,
                    &StatisticsQuery {
                        from_unix_ms: 9_000,
                        to_unix_ms: 11_000,
                        provider_id: Some("chatgpt_api".to_owned()),
                        model_id: None,
                        operation: None,
                        reasoning_effort: None,
                        fast_mode: None,
                        offset: 0,
                        limit: 100,
                    },
                )
                .expect("load ChatGPT quota history");
                assert_eq!(chatgpt_history.len(), 1);
                assert_eq!(chatgpt_history[0]["source"], "chatgpt");
                assert_eq!(chatgpt_history[0]["connectionId"], "quota-connection");
            } else {
                let (status, rate_limits) = database
                    .query_row(
                        "SELECT status, provider_rate_limits_json FROM usage_events WHERE event_id = ?1",
                        [&tracker.event_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .expect("read failed request with provider rate limits");
                assert_eq!(status, "failed");
                let rate_limits: serde_json::Value =
                    serde_json::from_str(&rate_limits).expect("decode failed request rate limits");
                assert_eq!(rate_limits["requestRemaining"], 0);
                let history = quota_history(
                    &database,
                    &StatisticsQuery {
                        from_unix_ms: 19_000,
                        to_unix_ms: 21_000,
                        provider_id: Some("groq".to_owned()),
                        model_id: None,
                        operation: None,
                        reasoning_effort: None,
                        fast_mode: None,
                        offset: 0,
                        limit: 100,
                    },
                )
                .expect("load failed request rate-limit history");
                assert_eq!(history.len(), 1);
                assert_eq!(history[0]["source"], "provider_response");
                assert_eq!(history[0]["providerId"], "groq");
                assert_eq!(history[0]["buckets"][0]["remaining"], 0);
            }
            let (run_id, connection_id, workspace_id, project_id, parent_run_id) = database
                .query_row(
                    "SELECT run_id, connection_id, workspace_id, project_id, parent_run_id
                     FROM usage_events WHERE event_id = ?1",
                    [&tracker.event_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )
                .expect("read recorded usage attribution");

            assert_eq!(run_id.as_deref(), expected.0);
            assert_eq!(connection_id.as_deref(), expected.1);
            assert_eq!(workspace_id.as_deref(), expected.2);
            assert_eq!(project_id.as_deref(), expected.3);
            assert_eq!(parent_run_id.as_deref(), expected.4);

            let query = StatisticsQuery {
                from_unix_ms: 0,
                to_unix_ms: i64::MAX,
                provider_id: None,
                model_id: None,
                operation: None,
                reasoning_effort: None,
                fast_mode: None,
                offset: 0,
                limit: 10,
            };
            let details = request_details(&database, &query).expect("read usage event details");
            let expected_run_id = expected
                .0
                .map_or(serde_json::Value::Null, serde_json::Value::from);
            let detail = details
                .iter()
                .find(|value| value["runId"] == expected_run_id)
                .expect("matching request details exist");
            assert_eq!(detail["conversationId"], "conversation-1");
            assert_eq!(
                detail["connectionId"],
                expected
                    .1
                    .map_or(serde_json::Value::Null, serde_json::Value::from)
            );
            assert_eq!(
                detail["workspaceId"],
                expected
                    .2
                    .map_or(serde_json::Value::Null, serde_json::Value::from)
            );
            assert_eq!(
                detail["projectId"],
                expected
                    .3
                    .map_or(serde_json::Value::Null, serde_json::Value::from)
            );
            assert_eq!(
                detail["parentRunId"],
                expected
                    .4
                    .map_or(serde_json::Value::Null, serde_json::Value::from)
            );
            assert_eq!(
                detail["providerProcessingMs"],
                if expected.0.is_none() {
                    serde_json::json!(5_587)
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(
                detail["providerQueueTimeMs"],
                if expected.0.is_none() {
                    serde_json::json!(27)
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(
                detail["providerInputProcessingMs"],
                if expected.0.is_none() {
                    serde_json::json!(10)
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(
                detail["providerOutputProcessingMs"],
                if expected.0.is_none() {
                    serde_json::json!(5_577)
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(
                detail["completionReason"],
                if expected.0.is_none() {
                    serde_json::json!("tool_calls")
                } else {
                    serde_json::Value::Null
                }
            );

            let export_details = request_details_for_export(&database, &query)
                .expect("read usage event export details");
            let export_detail = export_details
                .iter()
                .find(|value| value["runId"] == expected_run_id)
                .expect("matching export request details exist");
            assert_eq!(export_detail["conversationId"], "conversation-1");
            assert_eq!(export_detail["connectionId"], detail["connectionId"]);
            assert_eq!(export_detail["workspaceId"], detail["workspaceId"]);
            assert_eq!(export_detail["projectId"], detail["projectId"]);
            assert_eq!(export_detail["parentRunId"], detail["parentRunId"]);
            assert_eq!(
                export_detail["providerProcessingMs"],
                detail["providerProcessingMs"]
            );
            assert_eq!(
                export_detail["providerQueueTimeMs"],
                detail["providerQueueTimeMs"]
            );
            assert_eq!(
                export_detail["providerInputProcessingMs"],
                detail["providerInputProcessingMs"]
            );
            assert_eq!(
                export_detail["providerOutputProcessingMs"],
                detail["providerOutputProcessingMs"]
            );
            assert_eq!(
                export_detail["completionReason"],
                detail["completionReason"]
            );
        }
    }

    #[test]
    fn first_output_latency_survives_failed_and_cancelled_requests() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage
            .connect()
            .expect("connect to test storage")
            .execute_batch(
                "CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT
                 );
                 CREATE TABLE usage_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    event_kind TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    assistant_message_id TEXT,
                    provider_id TEXT,
                    model_id TEXT,
                    reasoning_effort TEXT,
                    operation TEXT NOT NULL,
                    status TEXT NOT NULL,
                    fast_requested INTEGER,
                    usage_source TEXT NOT NULL,
                    started_at_unix_ms INTEGER NOT NULL,
                    finished_at_unix_ms INTEGER,
                    input_tokens INTEGER,
                    output_tokens INTEGER,
                    reasoning_tokens INTEGER,
                    cached_input_tokens INTEGER,
                    cache_write_tokens INTEGER,
                    reported_cost_nano_usd INTEGER,
                    cost_source TEXT,
                    service_tier TEXT,
                    first_output_latency_ms INTEGER,
                    provider_processing_ms INTEGER,
                    provider_queue_time_ms INTEGER,
                    provider_input_processing_ms INTEGER,
                    provider_output_processing_ms INTEGER,
                    completion_reason TEXT,
                    run_id TEXT,
                    connection_id TEXT,
                    workspace_id TEXT,
                    project_id TEXT,
                    parent_run_id TEXT,
                    provider_rate_limits_json TEXT
                 );
                 INSERT INTO conversations (id, project_id) VALUES ('conversation-1', NULL);",
            )
            .expect("create isolated usage event schema");

        let make_request = || UsageRequestDetails {
            conversation_id: "conversation-1",
            assistant_message_id: None,
            provider_id: "provider",
            model_id: "model",
            reasoning_effort: None,
            operation: "chat",
            fast_requested: false,
            run_id: None,
            connection_id: None,
            workspace_id: None,
            project_id: None,
            parent_run_id: None,
        };

        let failed =
            UsageRequestTracker::start(&storage, make_request()).expect("record failed request");
        let failed_event_id = failed.event_id.clone();
        let failed_recorder = failed.first_output_recorder();
        failed_recorder.record(Duration::from_millis(120));
        failed_recorder.record(Duration::from_millis(2));
        drop(failed);

        let mut cancelled =
            UsageRequestTracker::start(&storage, make_request()).expect("record cancelled request");
        let cancelled_event_id = cancelled.event_id.clone();
        cancelled
            .first_output_recorder()
            .record(Duration::from_millis(80));
        cancelled.cancel().expect("finish cancelled request");

        let database = storage.connect().expect("connect to test storage");
        for (event_id, expected_status, expected_latency) in [
            (failed_event_id, "failed", Some(120)),
            (cancelled_event_id, "cancelled", Some(80)),
        ] {
            let (status, latency) = database
                .query_row(
                    "SELECT status, first_output_latency_ms
                     FROM usage_events WHERE event_id = ?1",
                    [&event_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
                )
                .expect("read completed usage event");
            assert_eq!(status, expected_status);
            assert_eq!(latency, expected_latency);
        }
    }

    #[test]
    fn finishing_a_request_invalidates_its_cached_estimated_cost() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage
            .connect()
            .expect("connect to test storage")
            .execute_batch(
                "CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT
                 );
                 CREATE TABLE usage_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    event_kind TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    assistant_message_id TEXT,
                    provider_id TEXT,
                    model_id TEXT,
                    reasoning_effort TEXT,
                    operation TEXT NOT NULL,
                    status TEXT NOT NULL,
                    fast_requested INTEGER,
                    usage_source TEXT NOT NULL,
                    started_at_unix_ms INTEGER NOT NULL,
                    finished_at_unix_ms INTEGER,
                    input_tokens INTEGER,
                    output_tokens INTEGER,
                    reasoning_tokens INTEGER,
                    cached_input_tokens INTEGER,
                    cache_write_tokens INTEGER,
                    reported_cost_nano_usd INTEGER,
                    cost_source TEXT,
                    service_tier TEXT,
                    first_output_latency_ms INTEGER,
                    provider_processing_ms INTEGER,
                    provider_queue_time_ms INTEGER,
                    provider_input_processing_ms INTEGER,
                    provider_output_processing_ms INTEGER,
                    completion_reason TEXT,
                    run_id TEXT,
                    connection_id TEXT,
                    workspace_id TEXT,
                    project_id TEXT,
                    parent_run_id TEXT,
                    provider_rate_limits_json TEXT
                 );
                 CREATE TEMP TABLE models_dev_event_costs (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    cost_usd REAL NOT NULL
                 );
                 INSERT INTO conversations (id, project_id) VALUES ('conversation-1', NULL);",
            )
            .expect("create isolated usage event schema");

        let mut tracker = UsageRequestTracker::start(
            &storage,
            UsageRequestDetails {
                conversation_id: "conversation-1",
                assistant_message_id: None,
                provider_id: "provider",
                model_id: "model",
                reasoning_effort: None,
                operation: "chat",
                fast_requested: false,
                run_id: None,
                connection_id: None,
                workspace_id: None,
                project_id: None,
                parent_run_id: None,
            },
        )
        .expect("record usage request");
        let database = storage.connect().expect("connect to test storage");
        database
            .execute(
                "INSERT INTO temp.models_dev_event_costs (event_id, cost_usd)
                 VALUES (?1, 0.01)",
                [&tracker.event_id],
            )
            .expect("cache estimated cost");
        drop(database);

        tracker.record_first_output(Duration::from_millis(120));
        tracker.record_first_output(Duration::from_millis(2));
        tracker
            .complete(&UsageData::default())
            .expect("finish usage request");

        let database = storage.connect().expect("connect to test storage");
        let latency = database
            .query_row(
                "SELECT first_output_latency_ms FROM usage_events WHERE event_id = ?1",
                [&tracker.event_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .expect("read first output latency");
        assert_eq!(latency, Some(120));
        let cached_cost_count = database
            .query_row(
                "SELECT COUNT(*) FROM temp.models_dev_event_costs WHERE event_id = ?1",
                [&tracker.event_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("read cached cost count");
        assert_eq!(cached_cost_count, 0);
    }
}

#[cfg(test)]
mod benchmark_tests {
    use std::{fs, path::PathBuf, time::Duration, time::Instant};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{StatisticsQuery, get_statistics, get_statistics_details};

    const EVENT_COUNT: i64 = 50_000;
    const EXPORT_PAGE_SIZE: i64 = 1_000;
    const WARM_STATISTICS_SAMPLES: usize = 7;

    #[tokio::test]
    #[ignore = "manual usage statistics and CSV page benchmark"]
    async fn benchmarks_usage_statistics_and_csv_export() {
        let directory = BenchmarkDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open benchmark storage");
        let database = storage.connect().expect("connect benchmark storage");
        database
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS models_dev_catalog_cache (
                    cache_id INTEGER PRIMARY KEY NOT NULL,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    catalog_json TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS usage_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    event_kind TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    assistant_message_id TEXT,
                    provider_id TEXT,
                    model_id TEXT,
                    reasoning_effort TEXT,
                    operation TEXT NOT NULL,
                    status TEXT NOT NULL,
                    fast_requested INTEGER,
                    service_tier TEXT,
                    input_tokens INTEGER,
                    output_tokens INTEGER,
                    reasoning_tokens INTEGER,
                    cached_input_tokens INTEGER,
                    cache_write_tokens INTEGER,
                    reported_cost_nano_usd INTEGER,
                    cost_source TEXT,
                    usage_source TEXT NOT NULL,
                    started_at_unix_ms INTEGER NOT NULL,
                    finished_at_unix_ms INTEGER,
                    first_output_latency_ms INTEGER,
                    provider_processing_ms INTEGER,
                    provider_queue_time_ms INTEGER,
                    provider_input_processing_ms INTEGER,
                    provider_output_processing_ms INTEGER,
                    request_manifest_json TEXT,
                    run_id TEXT,
                    connection_id TEXT,
                    workspace_id TEXT,
                    project_id TEXT,
                    parent_run_id TEXT,
                    provider_rate_limits_json TEXT
                 );
                 CREATE INDEX usage_events_started_idx
                    ON usage_events(started_at_unix_ms DESC, event_id);
                 CREATE INDEX usage_events_provider_model_idx
                    ON usage_events(provider_id, model_id, started_at_unix_ms DESC);
                 CREATE INDEX usage_events_conversation_idx
                    ON usage_events(conversation_id, started_at_unix_ms DESC);
                 CREATE INDEX usage_events_run_idx
                    ON usage_events(run_id, started_at_unix_ms DESC);
                 CREATE INDEX usage_events_started_desc_event_id_desc_idx
                    ON usage_events(started_at_unix_ms DESC, event_id DESC);",
            )
            .expect("create isolated benchmark schema");
        database
            .execute_batch(&format!(
                "INSERT INTO conversations (id, title, created_at, updated_at)
                 VALUES ('benchmark-conversation', 'Benchmark', 1, 1);
                 INSERT INTO models_dev_catalog_cache
                    (cache_id, fetched_at_unix_ms, catalog_json)
                 VALUES (1, {}, '{{\"openai\":{{\"models\":{{\"benchmark-model\":{{\"cost\":{{\"input\":1.0,\"output\":2.0}}}}}}}}}}');
                 WITH digits(value) AS (
                    VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
                 ), numbers(value) AS (
                    SELECT ones.value + tens.value * 10 + hundreds.value * 100
                         + thousands.value * 1000 + ten_thousands.value * 10000 + 1
                    FROM digits AS ones
                    CROSS JOIN digits AS tens
                    CROSS JOIN digits AS hundreds
                    CROSS JOIN digits AS thousands
                    CROSS JOIN digits AS ten_thousands
                 )
                 INSERT INTO usage_events (
                    event_id, event_kind, conversation_id, provider_id, model_id,
                    operation, status, fast_requested, input_tokens, output_tokens,
                    cached_input_tokens, usage_source, started_at_unix_ms, finished_at_unix_ms
                 )
                 SELECT 'benchmark-event-' || value, 'request',
                    'benchmark-conversation', 'chatgpt_api', 'benchmark-model',
                    'chat', 'completed', 0, 1000, 500, 250, 'provider_reported', 10, 11
                 FROM numbers WHERE value <= {EVENT_COUNT};",
                super::now_unix_millis().expect("benchmark clock"),
            ))
            .expect("seed isolated usage benchmark profile");
        drop(database);

        let query = StatisticsQuery {
            from_unix_ms: 0,
            to_unix_ms: 100,
            provider_id: None,
            model_id: None,
            operation: None,
            reasoning_effort: None,
            fast_mode: None,
            offset: 0,
            limit: 1,
        };
        let (_cancel_sender, mut cancellation) = watch::channel(false);
        let started = Instant::now();
        let cold = get_statistics(&storage, &query, &mut cancellation)
            .await
            .expect("cold statistics request");
        let cold_elapsed = started.elapsed();
        assert_eq!(cold["summary"]["totalRequests"], json!(EVENT_COUNT));
        assert_eq!(cold["summary"]["estimatedCostRequests"], json!(EVENT_COUNT));
        assert_eq!(cold["summary"]["totalTokens"], json!(75_000_000));
        assert_eq!(cold["summary"]["conversationCount"], json!(1));
        assert_eq!(cold["summary"]["successfulRequests"], json!(EVENT_COUNT));
        assert_eq!(cold["summary"]["legacyOutputMessages"], json!(0));
        assert_eq!(
            cold["providerBreakdown"][0]["providerId"],
            json!("chatgpt_api")
        );
        assert_eq!(cold["providerBreakdown"][0]["requests"], json!(EVENT_COUNT));
        assert_eq!(
            cold["providerBreakdown"][0]["cacheShareInputTokens"],
            json!(EVENT_COUNT * 1_000)
        );
        assert_eq!(
            cold["providerBreakdown"][0]["cacheShareCachedTokens"],
            json!(EVENT_COUNT * 250)
        );
        assert_eq!(
            cold["providerBreakdown"][0]["cacheShareRequests"],
            json!(EVENT_COUNT)
        );
        assert_eq!(
            cold["modelBreakdown"][0]["modelId"],
            json!("benchmark-model")
        );
        assert_eq!(
            cold["reasoningBreakdown"][0]["reasoningEffort"],
            json!("unspecified")
        );
        assert_eq!(cold["operationBreakdown"][0]["operation"], json!("chat"));
        assert_eq!(cold["fastModeBreakdown"][0]["fastRequested"], json!(false));
        assert!(
            cold["serviceTierBreakdown"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
        assert_eq!(cold["dailyTrend"][0]["requests"], json!(EVENT_COUNT));
        assert_eq!(cold["monthlyTrend"][0]["requests"], json!(EVENT_COUNT));
        assert_eq!(
            cold["conversationRanking"][0]["requests"],
            json!(EVENT_COUNT)
        );
        assert_eq!(cold["conversationRanking"][0]["title"], json!("Benchmark"));
        assert_eq!(cold["requestDetailCount"], json!(EVENT_COUNT));
        assert_eq!(cold["requestDetails"].as_array().map(Vec::len), Some(1));
        assert_eq!(cold["filterOptions"]["providers"][0], json!("chatgpt_api"));
        assert_eq!(cold["filterOptions"]["models"][0], json!("benchmark-model"));
        assert_eq!(
            cold["filterOptions"]["reasoningEfforts"][0],
            json!("unspecified")
        );

        let mut warm_samples = Vec::with_capacity(WARM_STATISTICS_SAMPLES);
        for _ in 0..WARM_STATISTICS_SAMPLES {
            let started = Instant::now();
            let statistics = get_statistics(&storage, &query, &mut cancellation)
                .await
                .expect("warm statistics request");
            warm_samples.push(started.elapsed());
            assert_eq!(statistics["summary"]["totalRequests"], json!(EVENT_COUNT));
        }

        let started = Instant::now();
        let catalog = super::models_dev_pricing::load_cached(&storage)
            .expect("load cached pricing for phase measurements");
        let catalog_load_elapsed = started.elapsed();
        let mut phase_database = storage.connect().expect("connect phase benchmark storage");
        let started = Instant::now();
        catalog
            .install_temporary_prices(&mut phase_database, &query, None)
            .expect("reuse cached event prices");
        let price_cache_elapsed = started.elapsed();
        let started = Instant::now();
        let aggregate = super::aggregate_usage_events(&phase_database, &query)
            .expect("aggregate benchmark events");
        let aggregate_elapsed = started.elapsed();
        let started = Instant::now();
        let _summary = aggregate.summary_json();
        let _providers = super::sorted_breakdown(aggregate.providers, "providerId", 200);
        let _models = super::sorted_breakdown(aggregate.models, "modelId", 200);
        let _reasoning =
            super::sorted_breakdown(aggregate.reasoning_efforts, "reasoningEffort", 200);
        let _operations = super::sorted_breakdown(aggregate.operations, "operation", 200);
        let _fast_modes = super::fast_mode_values(aggregate.fast_modes);
        let _service_tiers = super::sorted_service_tiers(aggregate.service_tiers);
        let _daily = super::trend_values(
            aggregate.daily_trends,
            aggregate.daily_trend_without_date,
            "day",
        );
        let _monthly = super::trend_values(
            aggregate.monthly_trends,
            aggregate.monthly_trend_without_date,
            "month",
        );
        let shaping_elapsed = started.elapsed();
        let started = Instant::now();
        let _ranking = super::conversation_ranking(&phase_database, aggregate.conversations)
            .expect("rank benchmark conversations");
        let ranking_elapsed = started.elapsed();
        let started = Instant::now();
        let _request_details = super::request_details(&phase_database, &query)
            .expect("load benchmark request details");
        let request_details_elapsed = started.elapsed();
        let started = Instant::now();
        let _filter_options =
            super::filter_options(&phase_database).expect("load benchmark filter options");
        let filter_options_elapsed = started.elapsed();
        let started = Instant::now();
        let _quota_history =
            super::quota_history(&phase_database, &query).expect("load benchmark quota history");
        let quota_history_elapsed = started.elapsed();
        let started = Instant::now();
        catalog
            .install_temporary_prices(
                &mut phase_database,
                &query,
                Some((query.limit, query.offset)),
            )
            .expect("install page-local event prices");
        let page_price_elapsed = started.elapsed();
        let started = Instant::now();
        let _export_details = super::request_details_for_export(&phase_database, &query)
            .expect("load benchmark export page");
        let export_page_elapsed = started.elapsed();
        drop(phase_database);

        let page_count = (EVENT_COUNT + EXPORT_PAGE_SIZE - 1) / EXPORT_PAGE_SIZE;
        let mut page_samples = Vec::with_capacity(page_count as usize);
        let mut exported_rows = 0_i64;
        for offset in (0..EVENT_COUNT).step_by(EXPORT_PAGE_SIZE as usize) {
            let page_query = StatisticsQuery {
                offset,
                limit: EXPORT_PAGE_SIZE,
                ..query.clone()
            };
            let started = Instant::now();
            let include_total = offset == 0;
            let page = get_statistics_details(&storage, &page_query, include_total)
                .expect("statistics export page");
            page_samples.push(started.elapsed());
            let rows = page["requestDetails"]
                .as_array()
                .expect("export detail array");
            exported_rows += rows.len() as i64;
            if include_total {
                assert_eq!(page["requestDetailCount"], json!(EVENT_COUNT));
            } else {
                assert!(page["requestDetailCount"].is_null());
            }
        }
        assert_eq!(exported_rows, EVENT_COUNT);

        eprintln!(
            "statistics benchmark: events={EVENT_COUNT} cold_get={:.3}ms warm_get_{}={}",
            cold_elapsed.as_secs_f64() * 1000.0,
            WARM_STATISTICS_SAMPLES,
            latency_summary(&warm_samples)
        );
        eprintln!(
            "statistics CSV benchmark: rows={exported_rows} page_size={EXPORT_PAGE_SIZE} pages={page_count} per_page={}",
            latency_summary(&page_samples)
        );
        eprintln!(
            "statistics phases: catalog_load={:.3}ms cached_price_install={:.3}ms aggregate_rows={:.3}ms json_shaping={:.3}ms conversation_ranking={:.3}ms request_details={:.3}ms filter_options={:.3}ms quota_history={:.3}ms export_price_install={:.3}ms export_page={:.3}ms",
            catalog_load_elapsed.as_secs_f64() * 1000.0,
            price_cache_elapsed.as_secs_f64() * 1000.0,
            aggregate_elapsed.as_secs_f64() * 1000.0,
            shaping_elapsed.as_secs_f64() * 1000.0,
            ranking_elapsed.as_secs_f64() * 1000.0,
            request_details_elapsed.as_secs_f64() * 1000.0,
            filter_options_elapsed.as_secs_f64() * 1000.0,
            quota_history_elapsed.as_secs_f64() * 1000.0,
            page_price_elapsed.as_secs_f64() * 1000.0,
            export_page_elapsed.as_secs_f64() * 1000.0,
        );
    }

    fn latency_summary(samples: &[Duration]) -> String {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let percentile = |fraction: f64| {
            let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
            sorted[index].as_secs_f64() * 1000.0
        };
        format!(
            "p50={:.3}ms p95={:.3}ms p99={:.3}ms",
            percentile(0.50),
            percentile(0.95),
            percentile(0.99)
        )
    }

    struct BenchmarkDirectory(PathBuf);

    impl BenchmarkDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-statistics-benchmark-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir(&path).expect("create isolated benchmark directory");
            Self(path)
        }
    }

    impl Drop for BenchmarkDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated benchmark directory");
        }
    }
}
