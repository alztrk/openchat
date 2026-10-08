use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot, watch};
use uuid::Uuid;

use crate::{
    protocol::{EventSink, Response, ServiceError},
    provider_schema::{ChatStreamEvent, ChatStreamSnapshot, ToolActivity},
    storage::{AppStorage, user_questions},
};

#[derive(Clone, Default)]
pub(crate) struct UserQuestionBroker {
    waiters: Arc<Mutex<HashMap<String, oneshot::Sender<Vec<user_questions::QuestionAnswer>>>>>,
    answered_runs: Arc<Mutex<HashSet<String>>>,
}

impl UserQuestionBroker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn ask(
        &self,
        storage: &AppStorage,
        run_id: &str,
        provider_id: &str,
        request_id: &Value,
        conversation_id: &str,
        assistant_message_id: &str,
        call_id: &str,
        call_name: &str,
        call_arguments: &Value,
        questions: Vec<user_questions::QuestionItem>,
        snapshot: &ChatStreamSnapshot,
        waiting_activity: &ToolActivity,
        tool_activities: &[ToolActivity],
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Vec<user_questions::QuestionAnswer>, ServiceError> {
        let connection = storage.connect().map_err(|_| unavailable_error())?;
        let run = user_questions::load_run_for_conversation(&connection, conversation_id, run_id)
            .map_err(storage_error)?;
        let sequence = user_questions::next_question_sequence(&connection, conversation_id, run_id)
            .map_err(storage_error)?;
        let group = user_questions::NewQuestionGroup {
            group_id: Uuid::new_v4().simple().to_string(),
            run_id: run_id.to_owned(),
            sequence,
            questions,
        };
        let question_checkpoint = json!({
            "version": 1,
            "assistantMessageId": assistant_message_id,
            "toolCallId": call_id,
            "toolName": call_name,
            "toolArguments": call_arguments,
        });
        let mut checkpoint = run.checkpoint.unwrap_or_else(|| json!({"version": 1}));
        let Some(checkpoint_object) = checkpoint.as_object_mut() else {
            return Err(unavailable_error());
        };
        checkpoint_object.insert("providerId".to_owned(), json!(provider_id));
        checkpoint_object.insert("conversationId".to_owned(), json!(conversation_id));
        checkpoint_object.insert("pendingQuestion".to_owned(), question_checkpoint);
        let (sender, receiver) = oneshot::channel();
        {
            let mut waiters = self.waiters.lock().await;
            if waiters.contains_key(&group.group_id) {
                return Err(ServiceError::new(
                    "question_waiter_conflict",
                    "The AI question could not be registered. Try again.",
                    true,
                ));
            }
            waiters.insert(group.group_id.clone(), sender);
        }

        let pending = match user_questions::pause_with_questions(
            &connection,
            conversation_id,
            run_id,
            run.checkpoint_revision,
            &checkpoint,
            &group,
        ) {
            Ok(pending) => pending,
            Err(error) => {
                self.remove_waiter(&group.group_id).await;
                return Err(storage_error(error));
            }
        };

        if crate::chatgpt_store::save_assistant_tool_checkpoint(storage, snapshot, tool_activities)
            .is_err()
        {
            self.remove_waiter(&group.group_id).await;
            return Err(ServiceError::new(
                "question_storage_unavailable",
                "The pending question could not be saved. Try again.",
                true,
            ));
        }

        if events
            .send(
                &ChatStreamEvent::ToolActivityUpdated {
                    snapshot: snapshot.clone(),
                    activity: waiting_activity.clone(),
                }
                .into_rpc(request_id.clone()),
            )
            .await
            .is_err()
        {
            self.remove_waiter(&group.group_id).await;
            return Err(ServiceError::new(
                "question_delivery_failed",
                "The AI question could not be delivered to the chat window.",
                true,
            ));
        }

        if events
            .send(&Response::event(
                request_id.clone(),
                "chat.question.requested",
                pending_group_value(&pending),
            ))
            .await
            .is_err()
        {
            self.remove_waiter(&group.group_id).await;
            return Err(ServiceError::new(
                "question_delivery_failed",
                "The AI question could not be delivered to the chat window.",
                true,
            ));
        }

        tokio::select! {
            biased;
            answer = receiver => {
                let answers = answer.map_err(|_| ServiceError::new(
                    "question_wait_interrupted",
                    "The AI response was interrupted while waiting for an answer.",
                    true,
                ))?;
                self.answered_runs.lock().await.insert(run_id.to_owned());
                self.resume_run(storage, conversation_id, run_id).await?;
                Ok(answers)
            }
            changed = cancellation.changed() => {
                let mut waiters = self.waiters.lock().await;
                waiters.remove(&group.group_id);
                if changed.is_ok() && *cancellation.borrow() {
                    Err(ServiceError::new(
                        "operation_cancelled",
                        "The response was stopped while waiting for an answer.",
                        false,
                    ))
                } else {
                    Err(ServiceError::new(
                        "question_wait_interrupted",
                        "The AI response was interrupted while waiting for an answer.",
                        true,
                    ))
                }
            }
        }
    }

    pub(crate) async fn submit(
        &self,
        storage: &AppStorage,
        conversation_id: &str,
        group_id: &str,
        expected_revision: i64,
        answers: &[user_questions::QuestionAnswer],
    ) -> Result<Value, ServiceError> {
        let mut waiters = self.waiters.lock().await;
        let connection = storage.connect().map_err(|_| unavailable_error())?;
        let submitted = user_questions::submit_answers(
            &connection,
            conversation_id,
            group_id,
            expected_revision,
            answers,
        )
        .map_err(storage_error)?;

        let mut execution_active = false;
        if !submitted.idempotent {
            let waiter = waiters.remove(group_id);
            if let Some(waiter) = waiter {
                execution_active = waiter
                    .send(submitted.group.answers.clone().unwrap_or_default())
                    .is_ok();
            }
        } else {
            execution_active = waiters.contains_key(group_id);
        }

        Ok(json!({
            "groupId": submitted.group.group_id,
            "runId": submitted.group.run_id,
            "conversationId": submitted.group.conversation_id,
            "status": "answered",
            "revision": submitted.group.revision,
            "idempotent": submitted.idempotent,
            "executionActive": execution_active,
            "question": pending_group_value(&submitted.group),
        }))
    }

    pub(crate) async fn list_pending(
        &self,
        storage: &AppStorage,
        conversation_id: &str,
    ) -> Result<Vec<Value>, ServiceError> {
        let connection = storage.connect().map_err(|_| unavailable_error())?;
        user_questions::list_recoverable_question_groups(&connection, conversation_id)
            .map(|groups| groups.iter().map(pending_group_value).collect())
            .map_err(storage_error)
    }

    pub(crate) async fn resume_run(
        &self,
        storage: &AppStorage,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<(), ServiceError> {
        let connection = storage.connect().map_err(|_| unavailable_error())?;
        user_questions::mark_run_resumed(&connection, conversation_id, run_id)
            .map(|_| ())
            .map_err(storage_error)
    }

    pub(crate) async fn finish_run(
        &self,
        storage: &AppStorage,
        conversation_id: &str,
        run_id: &str,
        outcome: RunOutcome,
    ) -> Result<(), ServiceError> {
        let outcome =
            if self.answered_runs.lock().await.remove(run_id) && outcome == RunOutcome::Failed {
                RunOutcome::Interrupted
            } else {
                outcome
            };
        let connection = storage.connect().map_err(|_| unavailable_error())?;
        let result = match outcome {
            RunOutcome::Completed => {
                user_questions::mark_run_completed(&connection, conversation_id, run_id)
            }
            RunOutcome::Cancelled => {
                user_questions::mark_run_cancelled(&connection, conversation_id, run_id)
            }
            RunOutcome::Paused => {
                user_questions::mark_run_paused(&connection, conversation_id, run_id)
            }
            RunOutcome::Interrupted => {
                user_questions::mark_run_interrupted(&connection, conversation_id, run_id)
            }
            RunOutcome::Failed => {
                user_questions::mark_run_failed(&connection, conversation_id, run_id)
            }
        };
        result.map(|_| ()).map_err(storage_error)
    }

    async fn remove_waiter(&self, group_id: &str) {
        self.waiters.lock().await.remove(group_id);
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use serde_json::{Value, json};
    use tokio::{sync::watch, time::timeout};
    use uuid::Uuid;

    use crate::{
        protocol::{EventSink, Response},
        provider_schema::{ChatStreamSnapshot, ToolActivity, ToolCall},
        storage::{AppStorage, user_questions},
    };

    use super::UserQuestionBroker;

    fn storage_with_run() -> (Arc<AppStorage>, PathBuf, String) {
        let root =
            std::env::temp_dir().join(format!("openchat-question-broker-{}", Uuid::new_v4()));
        let storage = Arc::new(AppStorage::open_at(root.clone()).expect("open test storage"));
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
                    UNIQUE(conversation_id, id)
                 );",
            )
            .expect("create desktop conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize test backend schema");
        let connection = storage.connect().expect("connect initialized storage");
        connection
            .execute(
                "INSERT INTO conversations (id, project_id, model_id, connection_id, workspace_id)
                 VALUES ('conversation', NULL, 'model', NULL, NULL)",
                [],
            )
            .expect("insert test conversation");
        let run_id = Uuid::new_v4().simple().to_string();
        user_questions::create_run(
            &connection,
            &user_questions::NewAgentRun {
                run_id: run_id.clone(),
                conversation_id: "conversation".to_owned(),
            },
        )
        .expect("create test run");
        (storage, root, run_id)
    }

    fn question_items() -> Vec<user_questions::QuestionItem> {
        serde_json::from_value(json!([{
            "id": "mode",
            "kind": "choice",
            "title": "Choose a mode",
            "options": [
                {"id": "safe", "label": "Safe"},
                {"id": "fast", "label": "Fast"}
            ],
            "required": true
        }]))
        .expect("parse valid question items")
    }

    fn answer() -> Vec<user_questions::QuestionAnswer> {
        serde_json::from_value(json!([{
            "questionId": "mode",
            "value": {"kind": "choice", "optionId": "safe"}
        }]))
        .expect("parse valid answer")
    }

    async fn requested_event(
        events: &mut tokio::sync::mpsc::UnboundedReceiver<Response>,
    ) -> Response {
        timeout(Duration::from_secs(2), async {
            loop {
                let event = events.recv().await.expect("event channel should stay open");
                if event.event.as_deref() == Some("chat.question.requested") {
                    break event;
                }
            }
        })
        .await
        .expect("question event should arrive")
    }

    fn waiting_tool_activity(
        message_id: &str,
    ) -> (ChatStreamSnapshot, ToolActivity, Vec<ToolActivity>) {
        let call = ToolCall {
            id: "call-1".to_owned(),
            name: "ask_user".to_owned(),
            arguments: json!({"questions": []}),
        };
        let activity = ToolActivity::waiting_for_user(&call);
        let snapshot = ChatStreamSnapshot::new("conversation", message_id, "", 1);
        (snapshot, activity.clone(), vec![activity])
    }

    #[tokio::test]
    async fn ask_pauses_run_and_answer_resumes_the_same_execution_once() {
        let (storage, root, run_id) = storage_with_run();
        let broker = UserQuestionBroker::default();
        let (events, mut event_receiver) = EventSink::test_channel();
        let (cancellation_sender, mut cancellation) = watch::channel(false);
        let (snapshot, waiting_activity, tool_activities) = waiting_tool_activity("assistant-1");
        let asking_broker = broker.clone();
        let asking_storage = Arc::clone(&storage);
        let asking_run_id = run_id.clone();
        let task = tokio::spawn(async move {
            asking_broker
                .ask(
                    &asking_storage,
                    &asking_run_id,
                    "chatgpt",
                    &json!("request-1"),
                    "conversation",
                    "assistant-1",
                    "call-1",
                    "ask_user",
                    &json!({"questions": []}),
                    question_items(),
                    &snapshot,
                    &waiting_activity,
                    &tool_activities,
                    &events,
                    &mut cancellation,
                )
                .await
        });
        let event = requested_event(&mut event_receiver).await;
        assert_eq!(event.event.as_deref(), Some("chat.question.requested"));
        let data = event.data.expect("question event data");
        let group_id = data
            .get("groupId")
            .and_then(Value::as_str)
            .expect("question group id")
            .to_owned();
        let connection = storage.connect().expect("check paused run");
        assert_eq!(
            user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
                .expect("load paused run")
                .status,
            user_questions::AgentRunStatus::Paused
        );
        let persisted_activities: String = connection
            .query_row(
                "SELECT tool_activities FROM messages WHERE id = 'assistant-1'",
                [],
                |row| row.get(0),
            )
            .expect("waiting tool activity is checkpointed before question event");
        assert!(persisted_activities.contains("waitingForUser"));
        drop(connection);

        let submitted = broker
            .submit(&storage, "conversation", &group_id, 0, &answer())
            .await
            .expect("submit answer");
        assert_eq!(submitted["executionActive"], true);
        assert_eq!(submitted["idempotent"], false);
        assert_eq!(
            task.await
                .expect("question task joins")
                .expect("answer resumes"),
            answer()
        );

        let duplicate = broker
            .submit(&storage, "conversation", &group_id, 0, &answer())
            .await
            .expect("accept an identical duplicate");
        assert_eq!(duplicate["executionActive"], false);
        assert_eq!(duplicate["idempotent"], true);
        let connection = storage.connect().expect("check resumed run");
        assert_eq!(
            user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
                .expect("load resumed run")
                .status,
            user_questions::AgentRunStatus::Running
        );
        drop(connection);
        broker
            .finish_run(&storage, "conversation", &run_id, super::RunOutcome::Failed)
            .await
            .expect("provider failure after an answer remains recoverable");
        let connection = storage.connect().expect("check interrupted run");
        assert_eq!(
            user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
                .expect("load interrupted run")
                .status,
            user_questions::AgentRunStatus::Interrupted
        );
        drop(connection);
        let groups = broker
            .list_pending(&storage, "conversation")
            .await
            .expect("list answered group after provider failure");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["status"], "answered");
        drop(cancellation_sender);
        drop(storage);
        fs::remove_dir_all(root).expect("remove test storage");
    }

    #[tokio::test]
    async fn cancellation_cancels_pending_question_before_later_answers() {
        let (storage, root, run_id) = storage_with_run();
        let broker = UserQuestionBroker::default();
        let (events, mut event_receiver) = EventSink::test_channel();
        let (cancellation_sender, mut cancellation) = watch::channel(false);
        let (snapshot, waiting_activity, tool_activities) = waiting_tool_activity("assistant-2");
        let asking_broker = broker.clone();
        let asking_storage = Arc::clone(&storage);
        let asking_run_id = run_id.clone();
        let task = tokio::spawn(async move {
            asking_broker
                .ask(
                    &asking_storage,
                    &asking_run_id,
                    "chatgpt",
                    &json!("request-2"),
                    "conversation",
                    "assistant-2",
                    "call-2",
                    "ask_user",
                    &json!({"questions": []}),
                    question_items(),
                    &snapshot,
                    &waiting_activity,
                    &tool_activities,
                    &events,
                    &mut cancellation,
                )
                .await
        });
        let event = requested_event(&mut event_receiver).await;
        let group_id = event
            .data
            .as_ref()
            .and_then(|value| value.get("groupId"))
            .and_then(Value::as_str)
            .expect("question group id")
            .to_owned();

        cancellation_sender.send(true).expect("cancel operation");
        let cancelled = task.await.expect("question task joins");
        assert_eq!(
            cancelled.expect_err("cancel should stop asking").code,
            "operation_cancelled"
        );
        broker
            .finish_run(
                &storage,
                "conversation",
                &run_id,
                super::RunOutcome::Cancelled,
            )
            .await
            .expect("finish cancelled run");
        let answer_result = broker
            .submit(&storage, "conversation", &group_id, 0, &answer())
            .await;
        assert_eq!(
            answer_result
                .expect_err("cancelled question rejects answer")
                .code,
            "question_conflict"
        );
        let connection = storage.connect().expect("check cancelled run");
        assert_eq!(
            user_questions::load_run_for_conversation(&connection, "conversation", &run_id)
                .expect("load cancelled run")
                .status,
            user_questions::AgentRunStatus::Cancelled
        );
        drop(connection);
        drop(cancellation_sender);
        drop(storage);
        fs::remove_dir_all(root).expect("remove test storage");
    }

    #[tokio::test]
    async fn interrupted_wait_keeps_question_recoverable_after_restart() {
        let (storage, root, run_id) = storage_with_run();
        let broker = UserQuestionBroker::default();
        let (events, mut event_receiver) = EventSink::test_channel();
        let (cancellation_sender, mut cancellation) = watch::channel(false);
        let (snapshot, waiting_activity, tool_activities) = waiting_tool_activity("assistant-3");
        let asking_broker = broker.clone();
        let asking_storage = Arc::clone(&storage);
        let asking_run_id = run_id.clone();
        let task = tokio::spawn(async move {
            asking_broker
                .ask(
                    &asking_storage,
                    &asking_run_id,
                    "chatgpt",
                    &json!("request-3"),
                    "conversation",
                    "assistant-3",
                    "call-1",
                    "ask_user",
                    &json!({"questions": []}),
                    question_items(),
                    &snapshot,
                    &waiting_activity,
                    &tool_activities,
                    &events,
                    &mut cancellation,
                )
                .await
        });
        let event = requested_event(&mut event_receiver).await;
        let group_id = event
            .data
            .as_ref()
            .and_then(|value| value.get("groupId"))
            .and_then(Value::as_str)
            .expect("question group id")
            .to_owned();

        cancellation_sender.send(true).expect("interrupt operation");
        assert_eq!(
            task.await
                .expect("question task joins")
                .expect_err("interruption leaves the question pending")
                .code,
            "operation_cancelled"
        );
        broker
            .finish_run(
                &storage,
                "conversation",
                &run_id,
                super::RunOutcome::Interrupted,
            )
            .await
            .expect("interrupt run during shutdown");

        drop(storage);
        let restarted_storage =
            Arc::new(AppStorage::open_at(root.clone()).expect("reopen storage"));
        restarted_storage
            .initialize_backend_schema()
            .expect("reinitialize backend schema");
        let restarted_broker = UserQuestionBroker::default();
        let groups = restarted_broker
            .list_pending(&restarted_storage, "conversation")
            .await
            .expect("list recoverable question after restart");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["groupId"], group_id);
        assert_eq!(groups[0]["status"], "pending");
        drop(cancellation_sender);
        drop(restarted_storage);
        fs::remove_dir_all(root).expect("remove test storage");
    }

    #[tokio::test]
    async fn assistant_checkpoint_failure_releases_question_waiter() {
        let (storage, root, run_id) = storage_with_run();
        storage
            .connect()
            .expect("connect test storage")
            .execute_batch("DROP TABLE messages")
            .expect("force assistant checkpoint failure");
        let broker = UserQuestionBroker::default();
        let (events, _event_receiver) = EventSink::test_channel();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let (snapshot, waiting_activity, tool_activities) = waiting_tool_activity("assistant-4");

        let error = broker
            .ask(
                &storage,
                &run_id,
                "chatgpt",
                &json!("request-4"),
                "conversation",
                "assistant-4",
                "call-4",
                "ask_user",
                &json!({"questions": []}),
                question_items(),
                &snapshot,
                &waiting_activity,
                &tool_activities,
                &events,
                &mut cancellation,
            )
            .await
            .expect_err("checkpoint failure must abort the wait");
        assert_eq!(error.code, "question_storage_unavailable");

        let groups = broker
            .list_pending(&storage, "conversation")
            .await
            .expect("list recoverable pending question");
        assert_eq!(groups.len(), 1);
        let group_id = groups[0]["groupId"]
            .as_str()
            .expect("question group id")
            .to_owned();
        let submitted = broker
            .submit(&storage, "conversation", &group_id, 0, &answer())
            .await
            .expect("persist answer without an active waiter");
        assert_eq!(submitted["executionActive"], false);

        drop(storage);
        fs::remove_dir_all(root).expect("remove test storage");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunOutcome {
    Completed,
    Cancelled,
    Paused,
    Interrupted,
    Failed,
}

pub(crate) fn parse_questions(
    value: &Value,
) -> Result<Vec<user_questions::QuestionItem>, ServiceError> {
    serde_json::from_value::<Vec<user_questions::QuestionItem>>(value.clone())
        .map_err(|_| invalid_questions_error())
}

pub(crate) fn parse_answers(
    value: &Value,
) -> Result<Vec<user_questions::QuestionAnswer>, ServiceError> {
    serde_json::from_value::<Vec<user_questions::QuestionAnswer>>(value.clone())
        .map_err(|_| invalid_answers_error())
}

pub(crate) fn pending_group_value(group: &user_questions::PendingQuestionGroup) -> Value {
    let checkpoint = group
        .checkpoint
        .as_ref()
        .and_then(|value| value.get("pendingQuestion"));
    json!({
        "groupId": group.group_id,
        "runId": group.run_id,
        "conversationId": group.conversation_id,
        "revision": group.revision,
        "status": match group.status {
            user_questions::QuestionGroupStatus::Pending => "pending",
            user_questions::QuestionGroupStatus::Answered => "answered",
            user_questions::QuestionGroupStatus::Cancelled => "cancelled",
            user_questions::QuestionGroupStatus::Expired => "expired",
        },
        "sequence": group.sequence,
        "assistantMessageId": checkpoint.and_then(|value| value.get("assistantMessageId")).and_then(Value::as_str),
        "toolCallId": checkpoint.and_then(|value| value.get("toolCallId")).and_then(Value::as_str),
        "toolName": checkpoint.and_then(|value| value.get("toolName")).and_then(Value::as_str),
        "toolArguments": checkpoint.and_then(|value| value.get("toolArguments")),
        "answers": group.answers,
        "questions": group.questions.iter().map(|question| json!({
            "id": question.id,
            "kind": question.kind,
            "title": question.title,
            "description": question.description,
            "options": question.options.iter().map(|option| json!({
                "id": option.id,
                "label": option.label,
            })).collect::<Vec<_>>(),
            "required": question.required,
            "placeholder": question.placeholder,
            "maxLength": question.max_length,
        })).collect::<Vec<_>>(),
    })
}

fn storage_error(error: user_questions::UserQuestionError) -> ServiceError {
    match error {
        user_questions::UserQuestionError::InvalidInput(_) => ServiceError::new(
            "question_data_invalid",
            "The question data is invalid. Ask the AI to try again.",
            false,
        ),
        user_questions::UserQuestionError::NotFound(_) => ServiceError::new(
            "question_not_found",
            "This question is no longer available. Refresh the conversation.",
            false,
        ),
        user_questions::UserQuestionError::Conflict(_) => ServiceError::new(
            "question_conflict",
            "This question can no longer accept an answer.",
            false,
        ),
        user_questions::UserQuestionError::StaleRevision { .. } => ServiceError::new(
            "question_revision_stale",
            "This question changed. Refresh it before answering.",
            true,
        ),
        user_questions::UserQuestionError::Database(_)
        | user_questions::UserQuestionError::CorruptStoredData(_) => unavailable_error(),
    }
}

fn unavailable_error() -> ServiceError {
    ServiceError::new(
        "question_storage_unavailable",
        "The pending question could not be saved. Try again.",
        true,
    )
}

fn invalid_questions_error() -> ServiceError {
    ServiceError::new(
        "question_data_invalid",
        "The AI sent an invalid question. Ask it to try again.",
        false,
    )
}

fn invalid_answers_error() -> ServiceError {
    ServiceError::new(
        "question_answer_invalid",
        "The answer does not match the pending question. Review it and try again.",
        false,
    )
}
