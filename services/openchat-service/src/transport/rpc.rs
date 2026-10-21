//! Routes decoded sidecar requests to conversation, provider, project, and tool services.
//!
//! Each request receives explicit storage, cancellation, event, permission, and question state;
//! method-family dispatch is kept in focused submodules.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::{Value, json};
use tokio::sync::watch as tokio_watch;
use zeroize::Zeroizing;

use crate::{
    chat_operation::ChatSendContext,
    chatgpt::ChatGptService,
    goals, instructions, openai_compatible,
    permissions::ToolPermissionBroker,
    protocol::{EventSink, Request, Response, ServiceError},
    provider_credentials,
    service::OperationGate,
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
    usage_statistics,
    user_question_broker::UserQuestionBroker,
};

#[path = "rpc/chat_execution.rs"]
mod chat_execution;
#[path = "rpc/chat_request.rs"]
mod chat_request;
#[path = "rpc/conversations.rs"]
mod conversations;
#[path = "rpc/file_changes.rs"]
mod file_changes;
#[path = "rpc/local_engine_models.rs"]
mod local_engine_models;
#[path = "rpc/projects.rs"]
mod projects;
#[path = "rpc/providers.rs"]
mod providers;
#[path = "rpc/run_control.rs"]
mod run_control;
#[path = "rpc/statistics.rs"]
mod statistics;
#[path = "rpc/temporary_conversation.rs"]
mod temporary_conversation;
#[path = "rpc/tool_permissions.rs"]
mod tool_permissions;

/// Holds the request-scoped services and state required by the RPC dispatcher.
pub(crate) struct RpcDispatchContext<'a> {
    pub(crate) storage: &'a AppStorage,
    pub(crate) service: &'a Arc<ChatGptService>,
    pub(crate) request: Request,
    pub(crate) cancellation: tokio_watch::Receiver<bool>,
    pub(crate) cancellation_gate: Arc<OperationGate>,
    pub(crate) events: EventSink,
    pub(crate) permission_broker: ToolPermissionBroker,
    pub(crate) user_question_broker: UserQuestionBroker,
    pub(crate) shutdown_requested: Arc<AtomicBool>,
    pub(crate) startup_recovered: Arc<AtomicBool>,
}

/// Dispatches one request to its method family and returns the serialized result value.
pub(crate) async fn dispatch(context: RpcDispatchContext<'_>) -> Result<Value, ServiceError> {
    let RpcDispatchContext {
        storage,
        service,
        mut request,
        mut cancellation,
        cancellation_gate,
        events,
        permission_broker,
        user_question_broker,
        shutdown_requested,
        startup_recovered,
    } = context;
    let method = request.method.clone();
    if method.starts_with("chat.file_changes.") {
        return file_changes::dispatch(&method, &request.params, storage);
    }
    if method.starts_with("statistics.") || method.starts_with("models.pricing.") {
        return statistics::dispatch(&method, &request.params, storage, &mut cancellation).await;
    }
    if method.starts_with("project.") {
        return projects::dispatch(
            &method,
            &mut request.params,
            &request.id,
            storage,
            &permission_broker,
            &events,
            &mut cancellation,
        )
        .await;
    }
    if method.starts_with("tools.permissions.") {
        return tool_permissions::dispatch_global(&method, &request.params, storage);
    }
    if method.starts_with("local.engines.")
        || method.starts_with("local.models.")
        || method.starts_with("models.hub.")
    {
        return local_engine_models::dispatch(
            &method,
            &request.params,
            &request.id,
            storage,
            &events,
            &mut cancellation,
        )
        .await;
    }
    if method.starts_with("chatgpt.")
        || method.starts_with("copilot.")
        || method == "opencode.models.list"
        || method == "compatible.models.list"
    {
        return providers::dispatch(
            &method,
            &mut request.params,
            storage,
            service,
            &mut cancellation,
            &cancellation_gate,
        )
        .await;
    }
    if method.starts_with("chat.memory.")
        || method.starts_with("chat.history.")
        || method == "chat.context.usage.estimate"
        || method == "chat.document.extract"
        || method.starts_with("browser.")
        || method.starts_with("conversation.archive.")
        || method.starts_with("profile.archive.")
    {
        return conversations::dispatch(
            &method,
            &mut request.params,
            &request.id,
            storage,
            &events,
            &mut cancellation,
        )
        .await;
    }
    if method.starts_with("chat.goal.")
        || method.starts_with("chat.runs.")
        || method.starts_with("chat.questions.")
        || method == "chat.tool.permission.respond"
    {
        return run_control::dispatch(
            &method,
            &request.params,
            storage,
            &permission_broker,
            &user_question_broker,
        )
        .await;
    }
    match method.as_str() {
        "system.health" => Ok(json!({
            "status": "ready",
            "database_path": storage.database_path().to_string_lossy(),
            "schema_version": storage.schema_version(),
        })),
        "instructions.defaults.get" => Ok(json!({
            "instructions": instructions::base_instructions(),
        })),
        "system.database.prepare" => {
            let chat_schema_version = request
                .params
                .get("chatSchemaVersion")
                .and_then(Value::as_i64)
                .filter(|version| (1..=100).contains(version))
                .ok_or_else(|| {
                    ServiceError::new(
                        "invalid_request_params",
                        "The local database schema version is invalid.",
                        false,
                    )
                })?;
            let backup_created = storage
                .prepare_chat_schema_migration(chat_schema_version)
                .map_err(|_| {
                    ServiceError::new(
                        "database_prepare_failed",
                        "The local database could not be safely prepared for an update.",
                        false,
                    )
                })?;
            Ok(json!({"status": "ready", "backupCreated": backup_created}))
        }
        "system.initialize" => {
            let schema_version = storage.initialize_backend_schema().map_err(|_| {
                ServiceError::new(
                    "storage_initialization_failed",
                    "Local conversation storage could not be initialized.",
                    false,
                )
            })?;
            usage_statistics::mark_interrupted_requests(storage).map_err(|_| {
                ServiceError::new(
                    "usage_statistics_unavailable",
                    "Local usage statistics could not be recovered.",
                    true,
                )
            })?;
            if startup_recovered
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let recovery = (|| {
                    let connection = storage.connect().map_err(|_| ())?;
                    crate::storage::user_questions::interrupt_running_runs_after_restart(
                        &connection,
                        None,
                    )
                    .map(|_| ())
                    .map_err(|_| ())
                })();
                if recovery.is_err() {
                    startup_recovered.store(false, Ordering::SeqCst);
                    return Err(ServiceError::new(
                        "question_storage_unavailable",
                        "Pending AI questions could not be recovered.",
                        true,
                    ));
                }
            }
            storage.commit_pending_profile_restore().map_err(|_| {
                ServiceError::new(
                    "profile_archive_storage_failed",
                    "The restored profile could not be finalized safely.",
                    false,
                )
            })?;
            Ok(json!({"status": "ready", "schema_version": schema_version}))
        }
        "chat.temporary.send" => {
            temporary_conversation::dispatch(
                &mut request.params,
                request.id.clone(),
                storage,
                service,
                &mut cancellation,
                &events,
            )
            .await
        }
        "chat.send" => {
            let chat_request = chat_request::parse_send_request(&mut request.params)?;
            let chat_request::ChatSendParameters {
                conversation_id,
                resume_run_id,
                api_key_connection_id,
                requested_reasoning_effort,
                requested_fast_mode,
                context_limit_override,
                requested_goal_objective,
                requested_custom_instructions,
                requested_project_skill_ids,
                plan_mode_requested,
                profile_tool_allowlist,
                excluded_assistant_message_id,
                title_generation,
            } = chat_request;
            let (connection, route) = chat_execution::load_saved_route(storage, conversation_id)?;
            let permission_mode = if plan_mode_requested {
                ToolPermissionMode::Plan
            } else {
                tool_permissions::load_mode(&connection)?
            };
            if requested_goal_objective.is_some() && permission_mode == ToolPermissionMode::Plan {
                return Err(ServiceError::new(
                    "goal_tool_calls_unavailable",
                    "Plan mode cannot start a persistent goal because it disables all tools.",
                    false,
                ));
            }
            let tool_permission_rules = route
                .project_id
                .as_deref()
                .map(|project_id| tool_permissions::load_project_rules(&connection, project_id))
                .transpose()?
                .unwrap_or_default();
            let (api_key_secret, custom_provider) = route.resolve_credentials()?;
            let resuming_question_run = resume_run_id.is_some();
            let prepared_run = chat_execution::prepare_run(chat_execution::RunPreparation {
                storage,
                connection: &connection,
                conversation_id,
                route: &route,
                resume_run_id,
                requested_reasoning_effort,
                requested_fast_mode,
                requested_goal_objective,
                requested_custom_instructions,
                requested_project_skill_ids,
                permission_mode,
                profile_tool_allowlist,
            })?;
            let chat_execution::PreparedChatRun {
                run_id,
                custom_instructions,
                project_instructions,
                project_skills,
                project_index_context,
                reasoning_effort,
                fast_mode,
                goal_execution,
                profile_tool_allowlist,
            } = prepared_run;
            let chat_execution::SavedChatRoute {
                provider_id,
                project_root,
                project_id,
                stored_api_key_connection_id,
                model_id: _,
                connection_id: _,
                workspace_id: _,
            } = route;
            if let Some(goal) = goal_execution.as_ref() {
                events
                    .send(&Response::event(
                        request.id.clone(),
                        "chat.goal.updated",
                        goals::goal_value(goal, "running", None),
                    ))
                    .await
                    .map_err(|_| {
                        ServiceError::new(
                            "protocol_unavailable",
                            "The active goal could not be shown in the application.",
                            true,
                        )
                    })?;
            }
            #[cfg(windows)]
            let mcp_configs = if permission_mode != ToolPermissionMode::Plan
                && matches!(
                    provider_id.as_deref(),
                    Some(
                        "chatgpt"
                            | "chatgpt_api"
                            | "opencode"
                            | "copilot"
                            | "gemini"
                            | "groq"
                            | "cerebras"
                            | "openrouter"
                            | "mistral"
                            | "deepseek"
                            | "xai"
                            | "perplexity"
                            | "together"
                            | "sambanova"
                            | "anthropic"
                            | "fireworks"
                            | "deepinfra"
                            | "dashscope"
                            | "cohere"
                            | "llama_cpp"
                    )
                ) {
                match project_root.as_deref() {
                    Some(project_root) => {
                        match tools::mcp::load_server_configs(
                            Path::new(project_root),
                            project_id.as_deref().unwrap_or_default(),
                        ) {
                            Ok(configs) => configs,
                            Err(error) => {
                                crate::storage::user_questions::mark_run_failed(
                                    &connection,
                                    conversation_id,
                                    &run_id,
                                )
                                .map_err(map_question_storage_error)?;
                                return Err(error);
                            }
                        }
                    }
                    None => Vec::new(),
                }
            } else {
                Vec::new()
            };
            let context = ChatSendContext {
                request_id: request.id.clone(),
                run_id: run_id.clone(),
                goal: goal_execution,
                conversation_id,
                excluded_assistant_message_id,
                custom_instructions: custom_instructions.as_deref(),
                project_instructions: project_instructions.as_deref(),
                project_skills,
                project_index_context,
                project_root: project_root.as_deref().map(Path::new),
                data_root: storage.root(),
                storage,
                permission_mode,
                tool_permission_rules,
                profile_tool_allowlist,
                #[cfg(windows)]
                mcp_configs,
                #[cfg(windows)]
                mcp_registry: None,
                permission_broker: &permission_broker,
                user_question_broker: &user_question_broker,
                cancellation: &mut cancellation,
                events,
                context_limit_override,
                title_generation,
            };
            let api_key = api_key_secret.as_ref().map(|secret| secret.as_str());
            if provider_id
                .as_deref()
                .is_some_and(provider_credentials::requires_saved_api_key)
                && api_key.is_none()
            {
                return Err(provider_credentials::missing_credentials());
            }
            let result = match provider_id.as_deref() {
                Some(id) if id.starts_with("custom_") => {
                    openai_compatible::send_message(
                        service,
                        context,
                        openai_compatible::ProviderChatOptions {
                            api_key,
                            requested_api_key_connection_id: api_key_connection_id,
                            stored_api_key_connection_id,
                            custom_provider,
                            reasoning_effort: reasoning_effort.as_deref(),
                            fast_mode,
                        },
                    )
                    .await
                }
                Some(
                    "opencode" | "chatgpt_api" | "copilot" | "gemini" | "groq" | "cerebras"
                    | "openrouter" | "mistral" | "deepseek" | "xai" | "perplexity" | "together"
                    | "sambanova" | "anthropic" | "fireworks" | "deepinfra" | "dashscope"
                    | "cohere" | "llama_cpp",
                ) => {
                    openai_compatible::send_message(
                        service,
                        context,
                        openai_compatible::ProviderChatOptions {
                            api_key,
                            requested_api_key_connection_id: api_key_connection_id,
                            stored_api_key_connection_id,
                            custom_provider: None,
                            reasoning_effort: reasoning_effort.as_deref(),
                            fast_mode,
                        },
                    )
                    .await
                }
                Some(_) => Err(ServiceError::new(
                    "provider_unavailable",
                    "This provider is not available.",
                    false,
                )),
                None => {
                    service
                        .send_message(context, reasoning_effort.as_deref(), fast_mode)
                        .await
                }
            };
            chat_execution::finish_run(
                chat_execution::ChatRunFinalization {
                    storage,
                    connection,
                    conversation_id,
                    run_id: &run_id,
                    resuming_question_run,
                    shutdown_requested: &shutdown_requested,
                    user_question_broker: &user_question_broker,
                },
                result,
            )
            .await
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn required_string<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ServiceError::new(
                "invalid_request_params",
                "A required local service parameter is missing or invalid.",
                false,
            )
        })
}

fn bounded_route_id<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    let value = required_string(params, name)?;
    if value.len() > 256 || value.chars().any(char::is_control) {
        return Err(invalid_request_params());
    }
    Ok(value)
}

fn is_custom_provider_id(provider_id: &str) -> bool {
    let Some(suffix) = provider_id.strip_prefix("custom_") else {
        return false;
    };
    (12..=57).contains(&suffix.len())
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Moves an optional API key out of JSON params and enforces its input limits.
fn take_optional_api_key(
    params: &mut Value,
    parameter: &str,
) -> Result<Option<Zeroizing<String>>, ServiceError> {
    let value = params
        .as_object_mut()
        .and_then(|object| object.remove(parameter));
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(secret)) => {
            let secret = Zeroizing::new(secret);
            if secret.trim().is_empty() {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "An optional local service parameter has an invalid value.",
                    false,
                ));
            }
            if secret.len() > 4096 || secret.chars().any(char::is_control) {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The API key parameter is invalid.",
                    false,
                ));
            }
            Ok(Some(secret))
        }
        _ => Err(ServiceError::new(
            "invalid_request_params",
            "An optional local service parameter has an invalid value.",
            false,
        )),
    }
}

fn invalid_request_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "A required local service parameter is missing or invalid.",
        false,
    )
}

fn required_secret_string(
    params: &mut Value,
    name: &str,
) -> Result<Zeroizing<String>, ServiceError> {
    let value = params
        .as_object_mut()
        .and_then(|object| object.remove(name));
    match value {
        Some(Value::String(secret)) => Ok(Zeroizing::new(secret)),
        _ => Err(invalid_request_params()),
    }
}

fn map_question_storage_error(
    error: crate::storage::user_questions::UserQuestionError,
) -> ServiceError {
    use crate::storage::user_questions::UserQuestionError;
    match error {
        UserQuestionError::InvalidInput(_) => invalid_question_request("question data"),
        UserQuestionError::NotFound(_) => ServiceError::new(
            "question_run_not_found",
            "The pending AI response could not be found.",
            false,
        ),
        UserQuestionError::Conflict(_) | UserQuestionError::StaleRevision { .. } => {
            ServiceError::new(
                "question_run_unavailable",
                "The pending AI response is no longer available to resume.",
                false,
            )
        }
        UserQuestionError::Database(_) | UserQuestionError::CorruptStoredData(_) => {
            ServiceError::new(
                "question_storage_unavailable",
                "The pending AI response could not be resumed.",
                true,
            )
        }
    }
}

fn invalid_question_request(field: &str) -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        format!("The {field} value is invalid."),
        false,
    )
}

fn optional_string<'a>(params: &'a Value, name: &str) -> Result<Option<&'a str>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
        _ => Err(ServiceError::new(
            "invalid_request_params",
            "An optional local service parameter has an invalid value.",
            false,
        )),
    }
}

fn optional_bool(params: &Value, name: &str) -> Result<Option<bool>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        _ => Err(ServiceError::new(
            "invalid_request_params",
            "An optional local service parameter has an invalid value.",
            false,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool},
    };

    use rusqlite::params;
    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{
        chatgpt::ChatGptService,
        permissions::ToolPermissionBroker,
        protocol::{EventSink, Request},
        service::OperationGate,
        storage::{AppStorage, user_questions},
        user_question_broker::UserQuestionBroker,
    };

    use super::chat_request::{parse_project_skill_ids, parse_title_generation};
    use super::{RpcDispatchContext, dispatch, take_optional_api_key};

    #[test]
    fn project_skill_ids_are_bounded_before_owned_values_are_created() {
        assert_eq!(
            parse_project_skill_ids(&json!({})).expect("missing selection defaults to empty"),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_project_skill_ids(&json!({"projectSkillIds": ["rust-style"]}))
                .expect("single selection"),
            ["rust-style"]
        );

        let oversized = json!({
            "projectSkillIds": vec!["rust-style"; crate::project_skills::MAX_PROJECT_SKILLS + 1]
        });
        assert_eq!(
            parse_project_skill_ids(&oversized)
                .expect_err("oversized selection should fail before IDs are copied")
                .code,
            "project_skill_invalid_selection"
        );
    }

    #[test]
    fn legacy_api_key_parameters_are_moved_into_zeroizing_values() {
        let mut optional_params = json!({"apiKey": "provider-secret"});
        let optional_secret =
            take_optional_api_key(&mut optional_params, "apiKey").expect("valid optional API key");
        assert_eq!(
            optional_secret.as_ref().map(|secret| secret.as_str()),
            Some("provider-secret")
        );
        assert!(optional_params.get("apiKey").is_none());
    }

    #[test]
    fn title_generation_discards_renderer_api_key() {
        let mut params = json!({
            "titleGeneration": {
                "mode": "selectedModel",
                "target": {
                    "providerId": "chatgpt_api",
                    "modelId": "test-model",
                    "apiKey": "title-secret"
                }
            }
        });

        let title_generation =
            parse_title_generation(&mut params).expect("valid title-generation route");
        let crate::chat_operation::TitleGenerationRequest::Model(target) = title_generation else {
            panic!("selected model route should be retained");
        };
        assert!(target.api_key.is_none());
        assert!(params["titleGeneration"]["target"].get("apiKey").is_none());
    }

    #[test]
    fn invalid_api_keys_are_removed_before_validation_returns() {
        let mut optional_params = json!({"apiKey": " "});
        assert!(take_optional_api_key(&mut optional_params, "apiKey").is_err());
        assert!(optional_params.get("apiKey").is_none());

        let mut oversized_params = json!({"apiKey": "x".repeat(4097)});
        assert!(take_optional_api_key(&mut oversized_params, "apiKey").is_err());
        assert!(oversized_params.get("apiKey").is_none());
    }

    #[tokio::test]
    async fn durable_chat_send_rejects_temporary_conversation_ids() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.send".to_owned(),
            params: json!({
                "conversationId": "tmp_0123456789abcdef0123456789abcdef",
            }),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);

        let error = dispatch(RpcDispatchContext {
            storage: &storage,
            service: &service,
            request,
            cancellation,
            cancellation_gate: Arc::new(OperationGate::default()),
            events: EventSink::new(),
            permission_broker: ToolPermissionBroker::default(),
            user_question_broker: UserQuestionBroker::default(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            startup_recovered: Arc::new(AtomicBool::new(true)),
        })
        .await
        .expect_err("temporary IDs must not use the durable send route");

        assert_eq!(error.code, "invalid_request_params");
        drop(service);
        drop(storage);
    }

    #[tokio::test]
    async fn unscoped_file_rpcs_are_not_exposed() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );

        for method in ["list_files", "search_files", "read_file", "get_file_info"] {
            let request = Request {
                id: json!(1),
                method: method.to_owned(),
                params: json!({
                    "root": "C:/",
                    "path": "Windows/win.ini",
                    "query": "OpenChat"
                }),
            };
            let (_cancel_sender, cancellation) = watch::channel(false);

            let error = dispatch(RpcDispatchContext {
                storage: &storage,
                service: &service,
                request,
                cancellation,
                cancellation_gate: Arc::new(OperationGate::default()),
                events: EventSink::new(),
                permission_broker: ToolPermissionBroker::default(),
                user_question_broker: UserQuestionBroker::default(),
                shutdown_requested: Arc::new(AtomicBool::new(false)),
                startup_recovered: Arc::new(AtomicBool::new(true)),
            })
            .await
            .expect_err("unscoped file access must not be dispatched");

            assert_eq!(error.code, "method_not_found", "method: {method}");
        }

        drop(service);
        drop(storage);
    }

    #[test]
    fn file_change_rpc_family_preserves_validation_and_unknown_method_errors() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");

        for (method, params, expected_code) in [
            (
                "chat.file_changes.list",
                json!({}),
                "invalid_request_params",
            ),
            ("chat.file_changes.unknown", json!({}), "method_not_found"),
        ] {
            let error = super::file_changes::dispatch(method, &params, &storage)
                .expect_err("file-change routes must preserve their stable errors");

            assert_eq!(error.code, expected_code, "method: {method}");
        }

        drop(storage);
    }

    #[tokio::test]
    async fn chat_send_saves_initial_provider_checkpoint_before_resolving_saved_credentials() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        let project_config = project_root.join(".openchat");
        fs::create_dir_all(&project_config).expect("create project instruction directory");
        fs::write(
            project_config.join("instructions.md"),
            "Use the project's established terminology.",
        )
        .expect("write project instructions");
        let skill_directory = project_config.join("skills").join("rust-style");
        fs::create_dir_all(&skill_directory).expect("create project Skill directory");
        fs::write(
            skill_directory.join("SKILL.md"),
            "Use rustfmt for Rust edits.",
        )
        .expect("write project Skill");
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        initialize_chat_test_schema(&storage);
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "INSERT INTO conversations (
                    id, provider_id, project_id, model_id, connection_id, workspace_id,
                    api_key_connection_id
                 ) VALUES (?1, 'gemini', 'project-1', 'test-model', NULL, NULL, 'gemini')",
                params!["conversation"],
            )
            .expect("insert API-key provider conversation");
        connection
            .execute(
                "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                params!["project-1", project_root.to_string_lossy().as_ref()],
            )
            .expect("save project folder");
        drop(connection);

        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.send".to_owned(),
            params: json!({
                "conversationId": "conversation",
                "apiKeyConnectionId": "gemini",
                "projectSkillIds": ["rust-style"],
            }),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);
        let result = dispatch(RpcDispatchContext {
            storage: &storage,
            service: &service,
            request,
            cancellation,
            cancellation_gate: Arc::new(OperationGate::default()),
            events: EventSink::new(),
            permission_broker: ToolPermissionBroker::default(),
            user_question_broker: UserQuestionBroker::default(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            startup_recovered: Arc::new(AtomicBool::new(true)),
        })
        .await;

        let error = result.expect_err("saved provider credentials should be required");
        assert_eq!(error.code, "provider_credentials_missing");

        let connection = storage.connect().expect("reconnect test storage");
        let run_id: String = connection
            .query_row(
                "SELECT id FROM agent_runs WHERE conversation_id = ?1",
                ["conversation"],
                |row| row.get(0),
            )
            .expect("chat.send should create a run");
        let run = user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
            .expect("load chat.send run");
        let checkpoint = run
            .checkpoint
            .expect("initial route checkpoint should be saved");
        assert_eq!(checkpoint["providerConnectionId"], "gemini");
        assert_eq!(
            checkpoint["projectInstructions"],
            "Use the project's established terminology."
        );
        assert_eq!(checkpoint["projectSkills"][0]["id"], "rust-style");
        assert_eq!(
            checkpoint["projectSkills"][0]["content"],
            "Use rustfmt for Rust edits."
        );
        assert!(checkpoint.get("apiKey").is_none());
        assert!(checkpoint.get("apiKeyConnectionId").is_none());

        drop(connection);
        drop(service);
        drop(storage);
    }

    #[tokio::test]
    async fn chat_send_rejects_oversized_custom_instructions_without_creating_run() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        initialize_chat_test_schema(&storage);
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "INSERT INTO conversations (id) VALUES (?1)",
                ["conversation"],
            )
            .expect("insert conversation");
        drop(connection);

        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.send".to_owned(),
            params: json!({
                "conversationId": "conversation",
                "customInstructions": "x".repeat(16 * 1024 + 1),
            }),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);

        let error = dispatch(RpcDispatchContext {
            storage: &storage,
            service: &service,
            request,
            cancellation,
            cancellation_gate: Arc::new(OperationGate::default()),
            events: EventSink::new(),
            permission_broker: ToolPermissionBroker::default(),
            user_question_broker: UserQuestionBroker::default(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            startup_recovered: Arc::new(AtomicBool::new(true)),
        })
        .await
        .expect_err("oversized instructions must be rejected before starting a run");

        assert_eq!(error.code, "invalid_request_params");
        let connection = storage.connect().expect("reconnect test storage");
        let run_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_runs WHERE conversation_id = ?1",
                ["conversation"],
                |row| row.get(0),
            )
            .expect("count conversation runs");
        assert_eq!(run_count, 0, "rejected input must not leave a run record");

        drop(connection);
        drop(service);
        drop(storage);
    }

    #[tokio::test]
    async fn chat_run_list_rpc_returns_recoverable_runs() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.clone()).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
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
            .expect("create desktop conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize native conversation schema");
        let connection = storage.connect().expect("connect initialized storage");
        connection
            .execute(
                "INSERT INTO conversations (id, title) VALUES ('conversation', 'Background task')",
                [],
            )
            .expect("insert conversation");
        user_questions::create_run(
            &connection,
            &user_questions::NewAgentRun {
                run_id: "active-run".to_owned(),
                conversation_id: "conversation".to_owned(),
            },
        )
        .expect("create active run");
        user_questions::save_checkpoint(
            &connection,
            "conversation",
            "active-run",
            0,
            &json!({"providerId": "gemini", "modelId": "gemini-pro"}),
        )
        .expect("save run route");
        drop(connection);

        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize ChatGPT service"),
        );
        let request = Request {
            id: json!(1),
            method: "chat.runs.list".to_owned(),
            params: json!({}),
        };
        let (_cancel_sender, cancellation) = watch::channel(false);
        let response = dispatch(RpcDispatchContext {
            storage: &storage,
            service: &service,
            request,
            cancellation,
            cancellation_gate: Arc::new(OperationGate::default()),
            events: EventSink::new(),
            permission_broker: ToolPermissionBroker::default(),
            user_question_broker: UserQuestionBroker::default(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            startup_recovered: Arc::new(AtomicBool::new(true)),
        })
        .await
        .expect("list runs");

        assert_eq!(response["runs"][0]["runId"], "active-run");
        assert_eq!(response["runs"][0]["conversationId"], "conversation");
        assert_eq!(response["runs"][0]["conversationTitle"], "Background task");
        assert_eq!(response["runs"][0]["providerId"], "gemini");
        assert_eq!(response["runs"][0]["modelId"], "gemini-pro");

        drop(service);
        drop(storage);
    }

    #[test]
    fn project_catalog_rpcs_resolve_the_saved_project_path() {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("project");
        fs::create_dir(&project_root).expect("create project folder");
        fs::create_dir(project_root.join(".openchat")).expect("create project config folder");
        fs::write(
            project_root.join(".openchat/tasks.json"),
            r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":60}]}"#,
        )
        .expect("write project task catalog");
        let storage =
            Arc::new(AppStorage::open_at(directory.0.join("data")).expect("open test storage"));
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute(
                "CREATE TABLE projects (id TEXT PRIMARY KEY, folder_path TEXT)",
                [],
            )
            .expect("create project table");
        connection
            .execute(
                "INSERT INTO projects (id, folder_path) VALUES (?1, ?2)",
                params!["project-1", project_root.to_string_lossy().into_owned()],
            )
            .expect("save project folder");
        drop(connection);
        assert_eq!(
            super::projects::project_root_for_mcp(&storage, "project-1")
                .expect("resolve saved project"),
            fs::canonicalize(project_root).expect("canonicalize project")
        );
        assert_eq!(
            super::projects::project_tasks_for_project(&storage, "project-1")
                .expect("load saved project tasks")[0]
                .id,
            "verify"
        );
        assert!(super::projects::project_root_for_mcp(&storage, "missing").is_err());
        assert!(super::projects::project_tasks_for_project(&storage, "missing").is_err());
    }

    struct TestDirectory(PathBuf);

    /// Builds the desktop tables required by RPC dispatch tests.
    fn initialize_chat_test_schema(storage: &AppStorage) {
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
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
            .expect("create desktop conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize native conversation schema");
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("openchat-rpc-test-{}", Uuid::new_v4()));
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
