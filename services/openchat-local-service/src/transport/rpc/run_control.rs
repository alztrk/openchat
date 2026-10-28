//! RPC routing for run status, goal controls, pending questions, and approvals.

use serde_json::{Value, json};

use crate::{
    goals,
    permissions::ToolPermissionBroker,
    protocol::ServiceError,
    storage::AppStorage,
    user_question_broker::{self, UserQuestionBroker},
};

use super::{invalid_question_request, required_string};

/// Dispatches run control and approval requests.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
    permission_broker: &ToolPermissionBroker,
    user_question_broker: &UserQuestionBroker,
) -> Result<Value, ServiceError> {
    match method {
        "chat.goal.active" => {
            let conversation_id = required_string(params, "conversationId")?;
            let goal = goals::latest_active_goal(storage, conversation_id)?;
            Ok(json!({"goal": goal}))
        }
        "chat.runs.list" => {
            let requested_limit = match params.get("limit") {
                Some(value) => value.as_u64().ok_or_else(invalid_run_list_request)?,
                None => 50,
            };
            let limit = usize::try_from(requested_limit)
                .ok()
                .filter(|limit| (1..=50).contains(limit))
                .ok_or_else(invalid_run_list_request)?;
            let connection = storage.connect().map_err(|_| {
                ServiceError::new(
                    "agent_runs_unavailable",
                    "Run status could not be loaded.",
                    true,
                )
            })?;
            let runs = crate::storage::user_questions::list_run_summaries(&connection, limit)
                .map_err(|_| {
                    ServiceError::new(
                        "agent_runs_unavailable",
                        "Run status could not be loaded.",
                        true,
                    )
                })?;
            Ok(json!({"runs": runs}))
        }
        "chat.subagents.list" => {
            let conversation_id = required_string(params, "conversationId")?;
            let requested_limit = match params.get("limit") {
                Some(value) => value.as_u64().ok_or_else(invalid_subagent_list_request)?,
                None => 50,
            };
            let limit = usize::try_from(requested_limit)
                .ok()
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(invalid_subagent_list_request)?;
            crate::chatgpt::subagents::list_child_runs(storage, conversation_id, limit)
        }
        "chat.subagents.get" => {
            let conversation_id = required_string(params, "conversationId")?;
            let run_id = required_string(params, "runId")?;
            crate::chatgpt::subagents::child_run_result(storage, conversation_id, run_id)
        }
        "chat.subagents.cancel" => {
            let conversation_id = required_string(params, "conversationId")?;
            let run_id = required_string(params, "runId")?;
            crate::chatgpt::subagents::cancel_child_analysis(storage, conversation_id, run_id)
        }
        "chat.goal.pause" => {
            let conversation_id = required_string(params, "conversationId")?;
            let run_id = required_string(params, "runId")?;
            let reason = match params.get("reason") {
                None => "user_paused",
                Some(Value::String(reason))
                    if matches!(reason.as_str(), "user_paused" | "budget_exceeded") =>
                {
                    reason.as_str()
                }
                _ => return Err(invalid_goal_pause_request()),
            };
            goals::pause_run(storage, conversation_id, run_id, reason)
        }
        "chat.goal.stop" => {
            let conversation_id = required_string(params, "conversationId")?;
            let run_id = required_string(params, "runId")?;
            goals::stop_run(storage, conversation_id, run_id)
        }
        "chat.questions.list" => {
            let conversation_id = required_string(params, "conversationId")?;
            let groups = user_question_broker
                .list_pending(storage, conversation_id)
                .await?;
            Ok(json!({"questions": groups}))
        }
        "chat.questions.respond" => {
            let conversation_id = required_string(params, "conversationId")?;
            let group_id = required_string(params, "groupId")?;
            let expected_revision = params
                .get("revision")
                .and_then(Value::as_i64)
                .filter(|revision| *revision >= 0)
                .ok_or_else(|| invalid_question_request("question revision"))?;
            let answers = user_question_broker::parse_answers(
                params
                    .get("answers")
                    .ok_or_else(|| invalid_question_request("question answers"))?,
            )?;
            user_question_broker
                .submit(
                    storage,
                    conversation_id,
                    group_id,
                    expected_revision,
                    &answers,
                )
                .await
        }
        "chat.tool.permission.respond" => {
            let approval_request_id = required_string(params, "approvalRequestId")?;
            let approved = params
                .get("approved")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    ServiceError::new(
                        "invalid_request_params",
                        "The tool permission decision is invalid.",
                        false,
                    )
                })?;
            permission_broker
                .respond(approval_request_id, approved)
                .await
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn invalid_goal_pause_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The goal pause request is invalid.",
        false,
    )
}

fn invalid_run_list_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The run list limit must be between 1 and 50.",
        false,
    )
}

fn invalid_subagent_list_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The child run list limit must be between 1 and 100.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        permissions::ToolPermissionBroker, storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };

    use super::dispatch;

    #[tokio::test]
    async fn run_control_dispatch_validates_limits_and_unknown_methods() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let permission_broker = ToolPermissionBroker::default();
        let user_question_broker = UserQuestionBroker::default();

        let invalid_limit = dispatch(
            "chat.runs.list",
            &json!({"limit": 0}),
            &storage,
            &permission_broker,
            &user_question_broker,
        )
        .await
        .expect_err("run limits below the supported range are rejected");
        assert_eq!(invalid_limit.code, "invalid_request_params");

        let invalid_question = dispatch(
            "chat.questions.respond",
            &json!({"conversationId": "conversation", "groupId": "group"}),
            &storage,
            &permission_broker,
            &user_question_broker,
        )
        .await
        .expect_err("missing question revisions are rejected before broker access");
        assert_eq!(invalid_question.code, "invalid_request_params");

        let unknown_method = dispatch(
            "chat.questions.unknown",
            &json!({}),
            &storage,
            &permission_broker,
            &user_question_broker,
        )
        .await
        .expect_err("unknown run-control methods are rejected");
        assert_eq!(unknown_method.code, "method_not_found");
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-run-control-rpc-test-{}", Uuid::new_v4()));
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
