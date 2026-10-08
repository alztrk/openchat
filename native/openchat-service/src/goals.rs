use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    protocol::{EventSink, Response, ServiceError},
    provider_schema::{ToolCall, ToolDefinition, ToolResult},
    storage::{
        AppStorage,
        user_questions::{self, AgentRun, GoalRunTransition},
    },
};

pub(crate) const CONTROL_TOOL_NAME: &str = "goal_update";
pub(crate) const START_GOAL_TOOL_NAME: &str = "start_goal";
pub(crate) const STOP_GOAL_TOOL_NAME: &str = "stop_goal";
pub(crate) const MAX_OBJECTIVE_BYTES: usize = 16 * 1024;
const MAX_PROGRESS_BYTES: usize = 8 * 1024;
const MAX_TODO_ITEMS: usize = 20;
const MAX_TODO_TEXT_BYTES: usize = 512;
const CONTINUATION_PROMPT: &str = "Continue working on the user's goal. Do not treat this continuation message as a new goal. Make concrete progress, use available tools when useful, and call goal_update with a concise summary and the current durable task list when there is progress, completion, or a need for user input.";

#[derive(Clone, Debug)]
pub(crate) enum GoalDecision {
    Continue,
    Completed,
    Paused,
    Stopped,
}

pub(crate) struct GoalExecution {
    pub(crate) run_id: String,
    pub(crate) conversation_id: String,
    pub(crate) objective: String,
    pub(crate) progress: String,
    pub(crate) todos: Vec<GoalTodo>,
    pub(crate) started_at_unix_ms: i64,
    pub(crate) checkpoint_revision: i64,
    pub(crate) checkpoint: Value,
    pub(crate) decision: GoalDecision,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GoalTodo {
    text: String,
    completed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GoalControlArguments {
    state: String,
    summary: String,
    #[serde(default)]
    todos: Vec<GoalTodo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartGoalArguments {
    objective: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StopGoalArguments {
    reason: String,
}

pub(crate) fn validate_objective(value: &str) -> Result<&str, ServiceError> {
    let objective = value.trim();
    if objective.is_empty() {
        return Err(ServiceError::new(
            "goal_objective_required",
            "Write a goal after /goal before sending it.",
            false,
        ));
    }
    if objective.len() > MAX_OBJECTIVE_BYTES {
        return Err(ServiceError::new(
            "goal_objective_too_long",
            "The goal exceeds the 16 KiB limit.",
            false,
        ));
    }
    Ok(objective)
}

pub(crate) fn initial_checkpoint(checkpoint: &mut Value, objective: &str, started_at_unix_ms: i64) {
    checkpoint["goal"] = json!({
        "objective": objective,
        "progress": "",
        "todos": [],
        "state": "running",
        "pauseReason": Value::Null,
        "startedAtUnixMs": started_at_unix_ms,
    });
}

pub(crate) fn execution_from_run(run: AgentRun) -> Result<GoalExecution, ServiceError> {
    let checkpoint = run.checkpoint.ok_or_else(checkpoint_error)?;
    let goal = checkpoint.get("goal").ok_or_else(checkpoint_error)?;
    let objective = goal
        .get("objective")
        .and_then(Value::as_str)
        .ok_or_else(checkpoint_error)?;
    let objective = validate_objective(objective)?.to_owned();
    let progress = goal
        .get("progress")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let todos = goal
        .get("todos")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| serde_json::from_value(item.clone()).map_err(|_| checkpoint_error()))
                .collect::<Result<Vec<GoalTodo>, ServiceError>>()
        })
        .transpose()?
        .unwrap_or_default();
    validate_todos(&todos).map_err(|_| checkpoint_error())?;
    let started_at_unix_ms = goal
        .get("startedAtUnixMs")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(checkpoint_error)?;
    Ok(GoalExecution {
        run_id: run.run_id,
        conversation_id: run.conversation_id,
        objective,
        progress,
        todos,
        started_at_unix_ms,
        checkpoint_revision: run.checkpoint_revision,
        checkpoint,
        decision: GoalDecision::Continue,
    })
}

pub(crate) fn control_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: CONTROL_TOOL_NAME.to_owned(),
        description: "Report goal progress, completion, a blocker, or a need for user input. Call this after a meaningful work step and before returning a final response.".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["continue", "completed", "blocked", "needs_user"]
                },
                "summary": {
                    "type": "string",
                    "maxLength": MAX_PROGRESS_BYTES
                },
                "todos": {
                    "type": "array",
                    "maxItems": MAX_TODO_ITEMS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "minLength": 1, "maxLength": MAX_TODO_TEXT_BYTES},
                            "completed": {"type": "boolean"}
                        },
                        "required": ["text", "completed"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["state", "summary"],
            "additionalProperties": false
        }),
    }
}

pub(crate) fn control_tool_definitions() -> [ToolDefinition; 3] {
    [
        ToolDefinition {
            name: START_GOAL_TOOL_NAME.to_owned(),
            description: "Start a persistent multi-turn goal for substantial work that the user asked you to carry out. Provide a clear, bounded objective. Do not use this for simple questions or work that fits in one response.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "objective": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": MAX_OBJECTIVE_BYTES
                    }
                },
                "required": ["objective"],
                "additionalProperties": false
            }),
        },
        control_tool_definition(),
        ToolDefinition {
            name: STOP_GOAL_TOOL_NAME.to_owned(),
            description: "Stop the active persistent goal when the user asks you to stop or continuing is no longer appropriate. After calling this, provide a concise final summary and take no further actions.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": MAX_PROGRESS_BYTES
                    }
                },
                "required": ["reason"],
                "additionalProperties": false
            }),
        },
    ]
}

pub(crate) fn append_goal_tool_instructions(instructions: &mut String) {
    instructions.push_str("\n\nPersistent goals: For substantial work the user asked you to carry out across multiple turns, call `start_goal` with a clear objective. Do not start a goal for simple questions or work that fits in one response. Only one active goal is allowed in each chat. While a goal is active, use `goal_update` after meaningful progress, when the goal is complete, or when you need user input. Use `stop_goal` when the user asks you to stop or continuing is no longer appropriate. After a terminal goal update or `stop_goal`, give a concise final summary and take no further actions.");
}

pub(crate) fn append_goal_instructions(instructions: &mut String, objective: &str) {
    instructions.push_str("\n\nGoal mode is active. Keep working on the current objective across model turns until it is complete or you cannot continue without user input. The objective may come from a user `/goal` message or a `start_goal` call. Use `goal_update` after meaningful progress with state `continue`; include a concise, current `todos` list of at most 20 objects with `text` and `completed` fields so the checklist survives interruptions. Use `completed` only after verifying the goal is done; use `blocked` or `needs_user` when a real external blocker or user decision prevents progress. After reporting `completed`, `blocked`, or `needs_user`, provide a concise visible summary and take no further actions. Do not claim completion without evidence. If a turn ends without a goal update, the client will ask you to continue.");
    instructions.push_str("\n\nGoal objective: ");
    instructions.push_str(objective);
}

pub(crate) fn split_control_calls(calls: &[ToolCall]) -> (Vec<ToolCall>, Vec<ToolCall>) {
    let mut control = Vec::new();
    let mut regular = Vec::new();
    for call in calls {
        if is_control_tool(&call.name) {
            control.push(call.clone());
        } else {
            regular.push(call.clone());
        }
    }
    (regular, control)
}

pub(crate) fn append_goal_instructions_to_messages(
    messages: &mut [Value],
    objective: &str,
) -> Result<(), ServiceError> {
    let Some(system_message) = messages
        .iter_mut()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("system"))
    else {
        return Err(ServiceError::new(
            "invalid_provider_request",
            "The provider request has no system instruction message.",
            false,
        ));
    };
    let Some(current_instructions) = system_message.get("content").and_then(Value::as_str) else {
        return Err(ServiceError::new(
            "invalid_provider_request",
            "The provider request instructions are invalid.",
            false,
        ));
    };
    let mut updated_instructions = current_instructions.to_owned();
    append_goal_instructions(&mut updated_instructions, objective);
    system_message["content"] = Value::String(updated_instructions);
    Ok(())
}

pub(crate) fn append_goal_instructions_to_responses_payload(
    payload: &mut Value,
    objective: &str,
) -> Result<(), ServiceError> {
    let Some(current_instructions) = payload.get("instructions").and_then(Value::as_str) else {
        return Err(ServiceError::new(
            "invalid_provider_request",
            "The provider request instructions are invalid.",
            false,
        ));
    };
    let mut updated_instructions = current_instructions.to_owned();
    append_goal_instructions(&mut updated_instructions, objective);
    payload["instructions"] = Value::String(updated_instructions);
    Ok(())
}

fn is_control_tool(name: &str) -> bool {
    matches!(
        name,
        CONTROL_TOOL_NAME | START_GOAL_TOOL_NAME | STOP_GOAL_TOOL_NAME
    )
}

pub(crate) fn validate_control_call_mix(
    control_calls: &[ToolCall],
    has_regular_calls: bool,
) -> Result<(), ServiceError> {
    if !has_regular_calls {
        return Ok(());
    }
    if control_calls.iter().any(|call| {
        call.name == STOP_GOAL_TOOL_NAME
            || (call.name == CONTROL_TOOL_NAME
                && call
                    .arguments
                    .get("state")
                    .and_then(Value::as_str)
                    .is_some_and(|state| state != "continue"))
    }) {
        return Err(ServiceError::new(
            "invalid_provider_response",
            "The provider returned a terminal goal update together with other actions.",
            false,
        ));
    }
    Ok(())
}

pub(crate) fn append_continuation_message(messages: &mut Vec<Value>) {
    messages.push(json!({"role": "user", "content": CONTINUATION_PROMPT}));
}

pub(crate) fn responses_continuation_input() -> Value {
    json!({
        "role": "user",
        "content": [{"type": "input_text", "text": CONTINUATION_PROMPT}],
    })
}

pub(crate) async fn process_control_calls(
    calls: &[ToolCall],
    execution: &mut Option<GoalExecution>,
    storage: &AppStorage,
    run_id: &str,
    conversation_id: &str,
    request_id: &Value,
    events: &EventSink,
) -> Result<Vec<ToolResult>, ServiceError> {
    if calls.len() > 1 {
        return Err(ServiceError::new(
            "invalid_provider_response",
            "The provider returned more than one goal control call in a single response.",
            false,
        ));
    }
    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let output = match call.name.as_str() {
            START_GOAL_TOOL_NAME => {
                let arguments: StartGoalArguments = parse_control_arguments(&call.arguments)?;
                let objective = validate_objective(&arguments.objective).map_err(|_| {
                    invalid_control_arguments("The provider returned an invalid goal objective.")
                })?;
                if execution.is_some() {
                    json!({"accepted": false, "reason": "goal_already_active"})
                } else {
                    let connection = storage.connect().map_err(|_| storage_error())?;
                    let run = user_questions::load_run_for_conversation(
                        &connection,
                        conversation_id,
                        run_id,
                    )
                    .map_err(|_| checkpoint_error())?;
                    let mut checkpoint = run.checkpoint.clone().ok_or_else(checkpoint_error)?;
                    initial_checkpoint(&mut checkpoint, objective, current_time_unix_ms()?);
                    match user_questions::activate_goal_run(
                        &connection,
                        conversation_id,
                        run_id,
                        run.checkpoint_revision,
                        &checkpoint,
                    )
                    .map_err(|_| goal_state_conflict())?
                    {
                        user_questions::GoalActivation::ActiveGoalExists => {
                            json!({"accepted": false, "reason": "goal_already_active"})
                        }
                        user_questions::GoalActivation::Started(run) => {
                            let started = execution_from_run(run)?;
                            send_goal_event(
                                events,
                                request_id,
                                goal_value(&started, "running", None),
                            )
                            .await?;
                            let value = goal_value(&started, "running", None);
                            *execution = Some(started);
                            json!({"accepted": true, "state": "running", "goal": value})
                        }
                    }
                }
            }
            CONTROL_TOOL_NAME => {
                let arguments: GoalControlArguments = parse_control_arguments(&call.arguments)?;
                if arguments.summary.len() > MAX_PROGRESS_BYTES {
                    return Err(invalid_control_arguments(
                        "The provider returned goal progress that exceeds the allowed size.",
                    ));
                }
                if let Some(active_goal) = execution.as_mut() {
                    let summary = arguments.summary.trim().to_owned();
                    let (decision, state, pause_reason) = match arguments.state.as_str() {
                        "continue" => (GoalDecision::Continue, "running", Value::Null),
                        "completed" => (GoalDecision::Completed, "completed", Value::Null),
                        "blocked" => (GoalDecision::Paused, "paused", json!("blocked")),
                        "needs_user" => (GoalDecision::Paused, "paused", json!("needs_user")),
                        _ => {
                            return Err(ServiceError::new(
                                "invalid_provider_response",
                                "The provider returned an unsupported goal state.",
                                false,
                            ));
                        }
                    };
                    active_goal.progress = summary;
                    validate_todos(&arguments.todos)
                        .map_err(|message| invalid_control_arguments(message))?;
                    active_goal.todos = arguments.todos;
                    active_goal.decision = decision;
                    set_checkpoint_goal_state(active_goal, state, pause_reason.clone());
                    save_progress(storage, active_goal)?;
                    send_goal_event(
                        events,
                        request_id,
                        goal_value(active_goal, state, pause_reason.as_str()),
                    )
                    .await?;
                    json!({"accepted": true, "state": state})
                } else {
                    json!({"accepted": false, "reason": "no_active_goal"})
                }
            }
            STOP_GOAL_TOOL_NAME => {
                let arguments: StopGoalArguments = parse_control_arguments(&call.arguments)?;
                let reason = arguments.reason.trim();
                if reason.is_empty() || reason.len() > MAX_PROGRESS_BYTES {
                    return Err(invalid_control_arguments(
                        "The provider returned an invalid goal stop reason.",
                    ));
                }
                let target_run_id = match execution.as_ref() {
                    Some(active_goal) => Some(active_goal.run_id.clone()),
                    None => {
                        let connection = storage.connect().map_err(|_| storage_error())?;
                        user_questions::latest_active_goal_run(&connection, conversation_id)
                            .map_err(|_| storage_error())?
                            .map(|run| run.run_id)
                    }
                };
                if let Some(target_run_id) = target_run_id {
                    let goal = stop_run_with_reason(
                        storage,
                        conversation_id,
                        &target_run_id,
                        "stopped_by_agent",
                        Some(reason),
                    )?;
                    if let Some(active_goal) = execution.as_mut() {
                        active_goal.decision = GoalDecision::Stopped;
                    }
                    send_goal_event(events, request_id, goal.clone()).await?;
                    json!({"accepted": true, "state": "cancelled", "reason": reason})
                } else {
                    json!({"accepted": false, "reason": "no_active_goal"})
                }
            }
            _ => {
                return Err(ServiceError::new(
                    "invalid_provider_response",
                    "The provider returned an unsupported goal control call.",
                    false,
                ));
            }
        };
        results.push(ToolResult {
            call_id: call.id.clone(),
            output,
        });
    }
    Ok(results)
}

fn parse_control_arguments<T: for<'de> Deserialize<'de>>(
    arguments: &Value,
) -> Result<T, ServiceError> {
    serde_json::from_value(arguments.clone())
        .map_err(|_| invalid_control_arguments("The provider returned invalid goal arguments."))
}

fn invalid_control_arguments(message: &'static str) -> ServiceError {
    ServiceError::new("invalid_provider_response", message, false)
}

async fn send_goal_event(
    events: &EventSink,
    request_id: &Value,
    goal: Value,
) -> Result<(), ServiceError> {
    events
        .send(&Response::event(
            request_id.clone(),
            "chat.goal.updated",
            goal,
        ))
        .await
        .map_err(|_| {
            ServiceError::new(
                "protocol_unavailable",
                "Goal progress could not be delivered to the application.",
                true,
            )
        })
}

pub(crate) fn save_final_state(
    storage: &AppStorage,
    execution: &mut GoalExecution,
    state: &str,
    pause_reason: Option<&str>,
) -> Result<(), ServiceError> {
    set_checkpoint_goal_state(
        execution,
        state,
        pause_reason.map_or(Value::Null, |reason| json!(reason)),
    );
    save_progress(storage, execution)
}

pub(crate) fn set_running(
    storage: &AppStorage,
    execution: &mut GoalExecution,
) -> Result<(), ServiceError> {
    execution.decision = GoalDecision::Continue;
    set_checkpoint_goal_state(execution, "running", Value::Null);
    save_progress(storage, execution)
}

pub(crate) fn pause_run(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
    reason: &str,
) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let run = user_questions::load_run_for_conversation(&connection, conversation_id, run_id)
        .map_err(|_| checkpoint_error())?;
    let mut execution = execution_from_run(run)?;
    execution.progress = execution.progress.trim().to_owned();
    execution.decision = GoalDecision::Paused;
    set_checkpoint_goal_state(&mut execution, "paused", json!(reason));
    let updated = user_questions::update_goal_run(
        &connection,
        conversation_id,
        run_id,
        execution.checkpoint_revision,
        &execution.checkpoint,
        GoalRunTransition::Pause,
    )
    .map_err(|_| goal_state_conflict())?;
    execution.checkpoint_revision = updated.checkpoint_revision;
    Ok(goal_value(&execution, "paused", Some(reason)))
}

pub(crate) fn stop_run(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
) -> Result<Value, ServiceError> {
    stop_run_with_reason(storage, conversation_id, run_id, "stopped_by_user", None)
}

fn stop_run_with_reason(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
    stop_reason: &str,
    progress: Option<&str>,
) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let run = user_questions::load_run_for_conversation(&connection, conversation_id, run_id)
        .map_err(|_| checkpoint_error())?;
    let mut execution = execution_from_run(run)?;
    if let Some(progress) = progress {
        execution.progress = progress.to_owned();
    }
    execution.decision = GoalDecision::Stopped;
    set_checkpoint_goal_state(&mut execution, "cancelled", json!(stop_reason));
    let updated = user_questions::update_goal_run(
        &connection,
        conversation_id,
        run_id,
        execution.checkpoint_revision,
        &execution.checkpoint,
        GoalRunTransition::Cancel,
    )
    .map_err(|_| goal_state_conflict())?;
    execution.checkpoint_revision = updated.checkpoint_revision;
    Ok(goal_value(&execution, "cancelled", Some(stop_reason)))
}

pub(crate) fn latest_active_goal(
    storage: &AppStorage,
    conversation_id: &str,
) -> Result<Option<Value>, ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let Some(run) = user_questions::latest_active_goal_run(&connection, conversation_id)
        .map_err(|_| storage_error())?
    else {
        return Ok(None);
    };
    let execution = execution_from_run(run.clone())?;
    let status = match run.status {
        user_questions::AgentRunStatus::Running => "running",
        user_questions::AgentRunStatus::Paused => "paused",
        user_questions::AgentRunStatus::Interrupted => "interrupted",
        _ => return Ok(None),
    };
    let reason = execution
        .checkpoint
        .pointer("/goal/pauseReason")
        .and_then(Value::as_str);
    Ok(Some(goal_value(&execution, status, reason)))
}

fn save_progress(storage: &AppStorage, execution: &mut GoalExecution) -> Result<(), ServiceError> {
    let connection = storage.connect().map_err(|_| storage_error())?;
    let updated = user_questions::update_goal_run(
        &connection,
        &execution.conversation_id,
        &execution.run_id,
        execution.checkpoint_revision,
        &execution.checkpoint,
        GoalRunTransition::KeepRunning,
    )
    .map_err(|_| goal_state_conflict())?;
    execution.checkpoint_revision = updated.checkpoint_revision;
    Ok(())
}

fn set_checkpoint_goal_state(execution: &mut GoalExecution, state: &str, pause_reason: Value) {
    if let Some(goal) = execution
        .checkpoint
        .get_mut("goal")
        .and_then(Value::as_object_mut)
    {
        goal.insert("progress".to_owned(), json!(execution.progress));
        goal.insert("todos".to_owned(), json!(execution.todos));
        goal.insert("state".to_owned(), json!(state));
        goal.insert("pauseReason".to_owned(), pause_reason);
    }
}

pub(crate) fn goal_value(
    execution: &GoalExecution,
    status: &str,
    pause_reason: Option<&str>,
) -> Value {
    json!({
        "conversationId": execution.conversation_id,
        "runId": execution.run_id,
        "objective": execution.objective,
        "progress": execution.progress,
        "todos": execution.todos,
        "startedAtUnixMs": execution.started_at_unix_ms,
        "status": status,
        "pauseReason": pause_reason,
    })
}

fn validate_todos(todos: &[GoalTodo]) -> Result<(), &'static str> {
    if todos.len() > MAX_TODO_ITEMS {
        return Err("The provider returned more goal tasks than allowed.");
    }
    if todos
        .iter()
        .any(|todo| todo.text.trim().is_empty() || todo.text.len() > MAX_TODO_TEXT_BYTES)
    {
        return Err("The provider returned an invalid goal task.");
    }
    Ok(())
}

fn checkpoint_error() -> ServiceError {
    ServiceError::new(
        "goal_checkpoint_invalid",
        "The saved goal could not be read. Stop it and start a new goal.",
        false,
    )
}

fn goal_state_conflict() -> ServiceError {
    ServiceError::new(
        "goal_state_changed",
        "The goal changed before this update could be saved. Reload the conversation to continue.",
        false,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "goal_storage_unavailable",
        "Goal progress could not be saved locally.",
        true,
    )
}

fn current_time_unix_ms() -> Result<i64, ServiceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_error())?
        .as_millis();
    i64::try_from(millis).map_err(|_| storage_error())
}

#[cfg(test)]
mod tests {
    use super::{GoalTodo, MAX_TODO_ITEMS, MAX_TODO_TEXT_BYTES, validate_todos};

    #[test]
    fn durable_todos_enforce_count_and_text_limits() {
        let valid = GoalTodo {
            text: "Inspect the affected route".to_owned(),
            completed: false,
        };
        assert!(validate_todos(&[valid.clone()]).is_ok());
        assert!(validate_todos(&vec![valid.clone(); MAX_TODO_ITEMS + 1]).is_err());
        assert!(
            validate_todos(&[GoalTodo {
                text: " ".to_owned(),
                completed: false,
            }])
            .is_err()
        );
        assert!(
            validate_todos(&[GoalTodo {
                text: "x".repeat(MAX_TODO_TEXT_BYTES + 1),
                completed: true,
            }])
            .is_err()
        );
    }

    #[test]
    fn durable_todos_require_only_known_fields() {
        let value = serde_json::json!({"text": "Review", "completed": false});
        assert!(serde_json::from_value::<GoalTodo>(value).is_ok());
        let value = serde_json::json!({
            "text": "Review",
            "completed": false,
            "access_token": "ignored"
        });
        assert!(serde_json::from_value::<GoalTodo>(value).is_err());
    }
}
