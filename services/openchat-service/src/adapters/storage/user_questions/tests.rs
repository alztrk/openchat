//! Tests for agent runs, checkpoints, and user questions storage.

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
use serde_json::json;
use uuid::Uuid;

use super::operations::*;
use super::types::*;

fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open in-memory database");
    prepare_connection(&connection);
    connection
}

fn prepare_connection(connection: &Connection) {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
                 CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL, title TEXT);
                 CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    output_tokens INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                 );",
        )
        .expect("create question storage fixture");
    super::super::schema::initialize_schema(connection, 15)
        .expect("apply schema through question storage baseline");
    super::super::schema::initialize_schema(connection, super::super::schema::SCHEMA_VERSION)
        .expect("apply question storage schema");
    connection
        .execute(
            "INSERT INTO conversations (id, title) VALUES ('conversation', 'Roadmap work')",
            [],
        )
        .expect("insert question storage conversation");
}

#[test]
fn invalid_initial_checkpoint_does_not_create_a_run() {
    let connection = connection();
    let error = create_run_with_checkpoint(
        &connection,
        &NewAgentRun {
            run_id: "invalid-checkpoint-run".to_owned(),
            conversation_id: "conversation".to_owned(),
        },
        &json!({"apiKey": "test-secret"}),
    )
    .expect_err("credentials must not be saved in a run checkpoint");

    assert!(matches!(error, UserQuestionError::InvalidInput(_)));
    let run_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM agent_runs WHERE id = ?1",
            ["invalid-checkpoint-run"],
            |row| row.get(0),
        )
        .expect("count rejected run");
    assert_eq!(run_count, 0);
}

#[test]
fn run_summaries_include_only_recoverable_runs_and_safe_route_metadata() {
    let connection = connection();
    run(&connection, "active");
    save_checkpoint(
        &connection,
        "conversation",
        "active",
        0,
        &json!({"providerId": "gemini", "modelId": "model-a"}),
    )
    .expect("save run route checkpoint");
    run(&connection, "failed");
    mark_run_failed(&connection, "conversation", "failed").expect("fail run");

    let summaries = list_run_summaries(&connection, 50).expect("list run summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].run_id, "active");
    assert_eq!(summaries[0].conversation_id, "conversation");
    assert_eq!(
        summaries[0].conversation_title.as_deref(),
        Some("Roadmap work")
    );
    assert_eq!(summaries[0].status, AgentRunStatus::Running);
    assert_eq!(summaries[0].provider_id.as_deref(), Some("gemini"));
    assert_eq!(summaries[0].model_id.as_deref(), Some("model-a"));
    let serialized = serde_json::to_value(&summaries[0]).expect("serialize run summary");
    assert_eq!(serialized["runId"], "active");
    assert_eq!(serialized["conversationTitle"], "Roadmap work");
    assert_eq!(
        serialized["updatedAtUnixMs"],
        summaries[0].updated_at_unix_ms
    );
    assert_eq!(serialized["status"], "running");
    assert!(list_run_summaries(&connection, 0).is_err());
    assert!(list_run_summaries(&connection, 51).is_err());
}

fn run(connection: &Connection, id: &str) {
    create_run(
        connection,
        &NewAgentRun {
            run_id: id.to_owned(),
            conversation_id: "conversation".to_owned(),
        },
    )
    .expect("create agent run");
}

fn questions(group_id: &str, run_id: &str, sequence: i64) -> NewQuestionGroup {
    NewQuestionGroup {
        group_id: group_id.to_owned(),
        run_id: run_id.to_owned(),
        sequence,
        questions: vec![
            QuestionItem {
                id: "mode".to_owned(),
                kind: QuestionKind::Choice,
                title: "Choose a mode".to_owned(),
                description: None,
                options: vec![
                    QuestionOption {
                        id: "fast".to_owned(),
                        label: "Fast".to_owned(),
                    },
                    QuestionOption {
                        id: "safe".to_owned(),
                        label: "Safe".to_owned(),
                    },
                ],
                required: true,
                placeholder: None,
                max_length: None,
            },
            QuestionItem {
                id: "note".to_owned(),
                kind: QuestionKind::Text,
                title: "Optional note".to_owned(),
                description: None,
                options: Vec::new(),
                required: false,
                placeholder: Some("Write a note".to_owned()),
                max_length: Some(200),
            },
        ],
    }
}

fn answers(note: &str) -> Vec<QuestionAnswer> {
    vec![
        QuestionAnswer {
            question_id: "mode".to_owned(),
            value: QuestionAnswerValue::Choice {
                option_id: "safe".to_owned(),
            },
        },
        QuestionAnswer {
            question_id: "note".to_owned(),
            value: QuestionAnswerValue::Text {
                text: note.to_owned(),
            },
        },
    ]
}

#[test]
fn pause_list_and_submit_support_multiple_question_items_and_idempotent_duplicates() {
    let connection = connection();
    run(&connection, "run-1");
    assert_eq!(
        next_question_sequence(&connection, "conversation", "run-1")
            .expect("first question sequence"),
        1
    );
    let paused = pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1, "messages": ["before pause"]}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause run");
    assert_eq!(paused.status, QuestionGroupStatus::Pending);
    assert_eq!(paused.questions.len(), 2);
    assert_eq!(
        paused.checkpoint,
        Some(json!({"turn": 1, "messages": ["before pause"]}))
    );
    assert_eq!(
        load_run(&connection, "run-1").unwrap().checkpoint_revision,
        1
    );
    assert_eq!(
        list_pending_groups(&connection, Some("conversation"))
            .expect("list pending groups")
            .len(),
        1
    );

    let first = submit_answers(&connection, "conversation", "group-1", 0, &answers("done"))
        .expect("submit answers");
    assert!(!first.idempotent);
    assert_eq!(first.group.status, QuestionGroupStatus::Answered);
    assert_eq!(
        list_recoverable_question_groups(&connection, "conversation")
            .expect("list answered but unresumed question groups")
            .len(),
        1
    );
    let mut reordered_answers = answers("done");
    reordered_answers.reverse();
    let duplicate = submit_answers(
        &connection,
        "conversation",
        "group-1",
        0,
        &reordered_answers,
    )
    .expect("accept identical duplicate");
    assert!(duplicate.idempotent);
    assert!(matches!(
        submit_answers(
            &connection,
            "conversation",
            "group-1",
            0,
            &answers("different"),
        ),
        Err(UserQuestionError::Conflict(_))
    ));

    mark_run_resumed(&connection, "conversation", "run-1").expect("resume after answers");
    assert_eq!(
        next_question_sequence(&connection, "conversation", "run-1")
            .expect("next question sequence"),
        2
    );
    let mut second_group = questions("group-2", "run-1", 2);
    second_group.questions[0].id = "mode-2".to_owned();
    second_group.questions[1].id = "note-2".to_owned();
    let second = pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        1,
        &json!({"turn": 2}),
        &second_group,
    )
    .expect("pause a consecutive question group");
    assert_eq!(second.sequence, 2);
    assert_eq!(
        list_pending_groups(&connection, Some("conversation"))
            .expect("list consecutive pending group")
            .iter()
            .map(|group| group.group_id.as_str())
            .collect::<Vec<_>>(),
        vec!["group-2"]
    );
}

#[test]
fn recoverable_question_groups_keep_only_latest_consecutive_resume_point() {
    let connection = connection();
    run(&connection, "run-1");

    pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause first question group");
    submit_answers(&connection, "conversation", "group-1", 0, &answers("first"))
        .expect("answer first question group");
    mark_run_resumed(&connection, "conversation", "run-1").expect("resume after first group");

    let mut second_group = questions("group-2", "run-1", 2);
    second_group.questions[0].id = "mode-2".to_owned();
    second_group.questions[1].id = "note-2".to_owned();
    pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        1,
        &json!({"turn": 2}),
        &second_group,
    )
    .expect("pause second question group");

    let pending_recoverable = list_recoverable_question_groups(&connection, "conversation")
        .expect("list current pending question group");
    assert_eq!(
        pending_recoverable
            .iter()
            .map(|group| group.group_id.as_str())
            .collect::<Vec<_>>(),
        vec!["group-2"]
    );
    assert_eq!(pending_recoverable[0].status, QuestionGroupStatus::Pending);

    let second_answers = vec![
        QuestionAnswer {
            question_id: "mode-2".to_owned(),
            value: QuestionAnswerValue::Choice {
                option_id: "safe".to_owned(),
            },
        },
        QuestionAnswer {
            question_id: "note-2".to_owned(),
            value: QuestionAnswerValue::Text {
                text: "second".to_owned(),
            },
        },
    ];
    submit_answers(&connection, "conversation", "group-2", 0, &second_answers)
        .expect("answer second question group");

    let answered_recoverable = list_recoverable_question_groups(&connection, "conversation")
        .expect("list latest answered question group");
    assert_eq!(
        answered_recoverable
            .iter()
            .map(|group| group.group_id.as_str())
            .collect::<Vec<_>>(),
        vec!["group-2"]
    );
    assert_eq!(
        answered_recoverable[0].status,
        QuestionGroupStatus::Answered
    );
}

#[test]
fn checkpoint_rejects_credentials_and_oversized_values() {
    let connection = connection();
    run(&connection, "run-1");
    assert!(matches!(
        save_checkpoint(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"access_token": "secret"}),
        ),
        Err(UserQuestionError::InvalidInput(_))
    ));
    assert!(matches!(
        save_checkpoint(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"api_key": "must not be stored"}),
        ),
        Err(UserQuestionError::InvalidInput(_))
    ));
    save_checkpoint(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"providerConnectionId": "gemini"}),
    )
    .expect("provider connection metadata is not a credential");
    let oversized = "x".repeat(super::MAX_CHECKPOINT_BYTES);
    assert!(matches!(
        save_checkpoint(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"state": oversized}),
        ),
        Err(UserQuestionError::InvalidInput(_))
    ));
}

#[test]
fn mutations_are_scoped_to_the_run_conversation() {
    let connection = connection();
    connection
        .execute(
            "INSERT INTO conversations (id) VALUES ('other-conversation')",
            [],
        )
        .expect("insert second conversation");
    run(&connection, "run-1");

    assert!(matches!(
        save_checkpoint(
            &connection,
            "other-conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
        ),
        Err(UserQuestionError::NotFound("agent run"))
    ));
    assert!(matches!(
        pause_with_questions(
            &connection,
            "other-conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        ),
        Err(UserQuestionError::NotFound("agent run"))
    ));

    pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause scoped run");
    assert!(matches!(
        submit_answers(
            &connection,
            "other-conversation",
            "group-1",
            0,
            &answers("wrong conversation"),
        ),
        Err(UserQuestionError::NotFound("question group"))
    ));
    assert!(matches!(
        mark_run_cancelled(&connection, "other-conversation", "run-1"),
        Err(UserQuestionError::NotFound("agent run"))
    ));
    assert_eq!(
        list_pending_groups(&connection, Some("conversation"))
            .expect("list scoped group")
            .len(),
        1
    );
}

#[test]
fn reads_reject_semantically_invalid_stored_json() {
    let invalid_checkpoint_connection = connection();
    run(&invalid_checkpoint_connection, "run-1");
    let invalid_checkpoint = json!({"access_token": "must not be stored"}).to_string();
    invalid_checkpoint_connection
        .execute(
            "UPDATE agent_runs
                 SET status = 'paused', checkpoint_json = ?1,
                     checkpoint_updated_at_unix_ms = 0
                 WHERE id = 'run-1'",
            [&invalid_checkpoint],
        )
        .expect("write syntactically valid invalid checkpoint fixture");
    assert!(matches!(
        list_recoverable_runs(&invalid_checkpoint_connection, Some("conversation")),
        Err(UserQuestionError::CorruptStoredData(_))
    ));

    let invalid_question_connection = connection();
    run(&invalid_question_connection, "run-1");
    pause_with_questions(
        &invalid_question_connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause run for invalid question fixture");
    let invalid_question = json!({
        "id": "mode",
        "kind": "choice",
        "title": "Broken choice",
        "options": [],
        "required": true,
    })
    .to_string();
    invalid_question_connection
        .execute(
            "UPDATE pending_question_items
                 SET question_json = ?1 WHERE item_id = 'mode'",
            [&invalid_question],
        )
        .expect("write syntactically valid invalid question fixture");
    assert!(matches!(
        list_pending_groups(&invalid_question_connection, Some("conversation")),
        Err(UserQuestionError::CorruptStoredData(_))
    ));
}

#[test]
fn database_byte_guards_reject_oversized_unicode_payloads() {
    let connection = connection();
    run(&connection, "run-1");
    let oversized_checkpoint = format!(
        "{{\"state\":\"{}\"}}",
        "é".repeat((super::MAX_CHECKPOINT_BYTES / 2) + 1)
    );
    assert!(
        connection
            .execute(
                "UPDATE agent_runs
                     SET checkpoint_json = ?1, checkpoint_updated_at_unix_ms = 0
                     WHERE id = 'run-1'",
                [&oversized_checkpoint],
            )
            .is_err()
    );
}

#[test]
fn restart_recovery_interrupts_running_runs_and_failed_runs_are_terminal() {
    let connection = connection();
    run(&connection, "run-1");
    let recovered = interrupt_running_runs_after_restart(&connection, Some("conversation"))
        .expect("interrupt running run after restart");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, super::AgentRunStatus::Interrupted);
    assert!(
        interrupt_running_runs_after_restart(&connection, Some("conversation"))
            .expect("repeat restart recovery")
            .is_empty()
    );
    let failed =
        mark_run_failed(&connection, "conversation", "run-1").expect("mark interrupted run failed");
    assert_eq!(failed.status, super::AgentRunStatus::Failed);
    assert!(matches!(
        mark_run_resumed(&connection, "conversation", "run-1"),
        Err(UserQuestionError::Conflict(_))
    ));
    assert!(
        list_recoverable_runs(&connection, Some("conversation"))
            .expect("list recoverable runs")
            .is_empty()
    );
}

#[test]
fn invalid_answers_and_stale_revisions_are_rejected() {
    let connection = connection();
    run(&connection, "run-1");
    pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause run");
    let missing_required = vec![QuestionAnswer {
        question_id: "note".to_owned(),
        value: QuestionAnswerValue::Text {
            text: "optional".to_owned(),
        },
    }];
    assert!(matches!(
        submit_answers(&connection, "conversation", "group-1", 0, &missing_required),
        Err(UserQuestionError::InvalidInput(_))
    ));
    assert!(matches!(
        submit_answers(&connection, "conversation", "group-1", 1, &answers("done")),
        Err(UserQuestionError::StaleRevision { .. })
    ));
}

#[test]
fn interrupted_run_and_pending_group_survive_a_restart() {
    let path = temporary_database_path();
    {
        let connection = Connection::open(&path).expect("open durable database");
        prepare_connection(&connection);
        run(&connection, "run-1");
        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause durable run");
        mark_run_interrupted(&connection, "conversation", "run-1").expect("interrupt durable run");
    }
    {
        let connection = Connection::open(&path).expect("reopen durable database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable durable foreign keys");
        let pending = list_pending_groups(&connection, None).expect("recover pending group");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].run_id, "run-1");
        assert_eq!(pending[0].checkpoint, Some(json!({"turn": 1})));
        assert_eq!(
            load_run(&connection, "run-1").unwrap().checkpoint_revision,
            1
        );
    }
    let _ = fs::remove_file(path);
}

#[test]
fn cancelling_a_run_cancels_pending_groups_and_blocks_resume() {
    let connection = connection();
    run(&connection, "run-1");
    pause_with_questions(
        &connection,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause run");
    assert!(
        connection
            .execute(
                "UPDATE agent_runs
                     SET status = 'cancelled', finished_at_unix_ms = 1
                     WHERE id = 'run-1'",
                [],
            )
            .is_err()
    );
    mark_run_cancelled(&connection, "conversation", "run-1").expect("cancel run");
    assert!(
        list_pending_groups(&connection, None)
            .expect("list pending groups")
            .is_empty()
    );
    assert!(matches!(
        mark_run_resumed(&connection, "conversation", "run-1"),
        Err(UserQuestionError::Conflict(_))
    ));
}

#[test]
fn cancellation_race_preserves_answer_first_and_cancels_cancel_first() {
    let answer_first = connection();
    run(&answer_first, "run-1");
    pause_with_questions(
        &answer_first,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause answer-first run");
    submit_answers(
        &answer_first,
        "conversation",
        "group-1",
        0,
        &answers("answer-first"),
    )
    .expect("answer before cancellation");

    let interrupted = mark_run_cancelled(&answer_first, "conversation", "run-1")
        .expect("preserve answered group for resume");
    assert_eq!(interrupted.status, super::AgentRunStatus::Interrupted);
    let recoverable = list_recoverable_question_groups(&answer_first, "conversation")
        .expect("list answered group after cancellation race");
    assert_eq!(recoverable.len(), 1);
    assert_eq!(recoverable[0].group_id, "group-1");
    assert_eq!(recoverable[0].status, QuestionGroupStatus::Answered);
    mark_run_resumed(&answer_first, "conversation", "run-1")
        .expect("resume interrupted answer-first run");

    let cancel_first = connection();
    run(&cancel_first, "run-1");
    pause_with_questions(
        &cancel_first,
        "conversation",
        "run-1",
        0,
        &json!({"turn": 1}),
        &questions("group-1", "run-1", 1),
    )
    .expect("pause cancel-first run");
    let cancelled =
        mark_run_cancelled(&cancel_first, "conversation", "run-1").expect("cancel pending group");
    assert_eq!(cancelled.status, super::AgentRunStatus::Cancelled);
    assert!(
        list_pending_groups(&cancel_first, None)
            .expect("list pending groups after cancellation")
            .is_empty()
    );
    assert!(matches!(
        submit_answers(
            &cancel_first,
            "conversation",
            "group-1",
            0,
            &answers("cancel-first"),
        ),
        Err(UserQuestionError::Conflict(_))
    ));
}

#[test]
fn concurrent_identical_submissions_are_applied_once() {
    let path = temporary_database_path();
    {
        let connection = Connection::open(&path).expect("open concurrent database");
        prepare_connection(&connection);
        run(&connection, "run-1");
        pause_with_questions(
            &connection,
            "conversation",
            "run-1",
            0,
            &json!({"turn": 1}),
            &questions("group-1", "run-1", 1),
        )
        .expect("pause concurrent run");
    }
    let answers = Arc::new(answers("same"));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let path = path.clone();
        let answers = Arc::clone(&answers);
        handles.push(thread::spawn(move || {
            let connection = Connection::open(path).expect("open concurrent connection");
            connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .expect("configure concurrent timeout");
            submit_answers(&connection, "conversation", "group-1", 0, &answers)
                .expect("submit concurrent answers")
                .idempotent
        }));
    }
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("join answer worker"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|idempotent| !**idempotent).count(), 1);
    assert_eq!(results.iter().filter(|idempotent| **idempotent).count(), 1);
    let _ = fs::remove_file(path);
}

fn temporary_database_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "openchat-user-questions-{timestamp}-{}.sqlite3",
        Uuid::new_v4()
    ))
}
