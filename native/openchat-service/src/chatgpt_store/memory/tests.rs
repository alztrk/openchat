use std::time::Instant;

use rusqlite::Connection;

use super::search::{
    MAX_ARCHIVE_EXCERPT_CHARS, bounded_archive_excerpt, combine_archive_results, retrieval_limits,
    retrieve_archived_memories_from_connection, search_archived_memories_from_connection,
    search_expression,
};

#[test]
fn hybrid_archive_search_combines_rankings_without_replacing_keyword_excerpts() {
    let lexical = vec![
        archive_excerpt("message-a", "lexical match", 10),
        archive_excerpt("message-b", "lexical fallback", 20),
    ];
    let semantic = vec![
        archive_excerpt("message-b", "semantic passage", 20),
        archive_excerpt("message-c", "semantic-only result", 30),
    ];

    let results = combine_archive_results(lexical, semantic, 3);

    assert_eq!(
        results
            .iter()
            .map(|excerpt| excerpt.message_id.as_str())
            .collect::<Vec<_>>(),
        ["message-b", "message-a", "message-c"]
    );
    assert_eq!(results[0].content, "lexical fallback");
}

fn archive_excerpt(
    message_id: &str,
    content: &str,
    created_at_unix_ms: i64,
) -> super::ArchivedMemoryExcerpt {
    super::ArchivedMemoryExcerpt {
        message_id: message_id.to_owned(),
        role: "user".to_owned(),
        content: content.to_owned(),
        created_at_unix_ms,
    }
}

#[test]
fn archived_excerpts_are_bounded_without_leaving_partial_match_markers() {
    let complete_match = format!(
        "prefix [match]target[/match]{}",
        " trailing text".repeat(400)
    );
    let bounded = bounded_archive_excerpt(complete_match);
    assert!(bounded.chars().count() <= MAX_ARCHIVE_EXCERPT_CHARS + 1);
    assert!(bounded.contains("[match]target[/match]"));

    let incomplete_match = format!("{}[match]target", "x".repeat(MAX_ARCHIVE_EXCERPT_CHARS - 8));
    let bounded = bounded_archive_excerpt(incomplete_match);
    assert!(bounded.chars().count() <= MAX_ARCHIVE_EXCERPT_CHARS + 1);
    assert!(!bounded.contains("[match]"));
    assert!(bounded.ends_with('…'));
}

#[test]
fn archived_search_backfills_once_and_stays_within_conversation_and_boundary() {
    let connection = Connection::open_in_memory().expect("open in-memory database");
    connection
            .execute_batch(
                "CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]'
                );
                CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                );
                CREATE TABLE conversation_memory_excluded_tools (
                    conversation_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, tool_name)
                );
                CREATE TABLE conversation_memory_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                CREATE TABLE conversation_memory_tool_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at)
                VALUES
                    ('old', 'conversation-a', 'user', 'Atlas archive detail', 'completed', 10),
                    ('boundary', 'conversation-a', 'assistant', 'Atlas boundary detail', 'completed', 20),
                    ('new', 'conversation-a', 'user', 'Atlas newer detail', 'completed', 30),
                    ('other', 'conversation-b', 'user', 'Atlas unrelated detail', 'completed', 5);",
            )
            .expect("create archive schema and data");
    let search = search_expression("conversation-a", "Atlas").expect("search term");

    let results = retrieve_archived_memories_from_connection(
        &connection,
        "conversation-a",
        "boundary",
        &search,
        6,
        48,
    )
    .expect("retrieve archived messages");

    let message_ids = results
        .iter()
        .map(|excerpt| excerpt.message_id.as_str())
        .collect::<Vec<_>>();
    assert!(message_ids.contains(&"old"));
    assert!(message_ids.contains(&"boundary"));
    assert!(!message_ids.contains(&"new"));
    assert!(!message_ids.contains(&"other"));

    let archive_results = search_archived_memories_from_connection(
        &connection,
        "conversation-a",
        None,
        &search,
        20,
        48,
    )
    .expect("search full conversation archive");
    let archive_message_ids = archive_results
        .iter()
        .map(|excerpt| excerpt.message_id.as_str())
        .collect::<Vec<_>>();
    assert!(archive_message_ids.contains(&"old"));
    assert!(archive_message_ids.contains(&"boundary"));
    assert!(archive_message_ids.contains(&"new"));
    assert!(!archive_message_ids.contains(&"other"));
    assert_eq!(
        archive_results
            .iter()
            .find(|excerpt| excerpt.message_id == "old")
            .map(|excerpt| excerpt.created_at_unix_ms),
        Some(10)
    );

    let saved_message_count = connection
        .query_row("SELECT COUNT(*) FROM messages", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("count saved messages");
    assert_eq!(saved_message_count, 4);

    let indexed_count = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_memory_fts
                 WHERE conversation_memory_fts.conversation_id = 'conversation-a'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("count indexed messages");
    let backfill_count = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_memory_index_state
                 WHERE conversation_id = 'conversation-a'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("count backfill markers");
    assert_eq!(indexed_count, 3);
    assert_eq!(backfill_count, 1);
}

#[test]
fn archived_search_retrieves_completed_tool_activity_details() {
    let connection = Connection::open_in_memory().expect("open in-memory database");
    connection
            .execute_batch(
                "CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]'
                );
                CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                );
                CREATE TABLE conversation_memory_excluded_tools (
                    conversation_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, tool_name)
                );
                CREATE TABLE conversation_memory_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                CREATE TABLE conversation_memory_tool_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                INSERT INTO messages
                    (id, conversation_id, role, content, status, created_at, tool_activities)
                VALUES
                    ('tool-message', 'tool-history', 'assistant', 'I checked the setting.',
                     'completed', 10,
                     json_array(json_object(
                         'callId', 'call-1', 'name', 'read',
                         'arguments', json_object('path', 'settings.json'),
                         'output', json_object('content', 'opalquartz enabled'),
                         'targetPath', 'privatepathmarker',
                         'status', 'completed'
                     ))),
                    ('boundary', 'tool-history', 'user', 'Continue from there.', 'completed', 20, '[]');",
            )
            .expect("create tool activity archive schema and data");
    let search = search_expression("tool-history", "opalquartz").expect("search term");

    let results = retrieve_archived_memories_from_connection(
        &connection,
        "tool-history",
        "boundary",
        &search,
        6,
        48,
    )
    .expect("retrieve archived tool activity");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].message_id, "tool-message");
    assert!(results[0].content.contains("opalquartz"));

    let archive_results = search_archived_memories_from_connection(
        &connection,
        "tool-history",
        None,
        &search,
        20,
        48,
    )
    .expect("search all archived tool activity");
    assert_eq!(archive_results.len(), 1);
    assert_eq!(archive_results[0].message_id, "tool-message");
    assert_eq!(archive_results[0].created_at_unix_ms, 10);

    let target_path_matches = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"privatepathmarker\"'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("ensure the display-only target path is not indexed");
    assert_eq!(target_path_matches, 0);
}

#[test]
fn archive_result_budget_scales_with_known_context_windows() {
    assert_eq!(retrieval_limits(None), None);
    assert_eq!(retrieval_limits(Some(1024)), None);
    assert_eq!(retrieval_limits(Some(2048)), Some((1, 42)));
    assert_eq!(retrieval_limits(Some(8192)), Some((1, 170)));
    assert_eq!(retrieval_limits(Some(32_768)), Some((5, 136)));
    assert_eq!(retrieval_limits(Some(200_000)), Some((16, 256)));
}

#[test]
#[ignore = "manual FTS archive benchmark; run with --ignored --nocapture"]
fn archived_memory_search_scales_to_large_conversations() {
    const MESSAGE_COUNT: i64 = 50_000;
    let connection = Connection::open_in_memory().expect("open benchmark database");
    connection
        .execute_batch(
            "CREATE TABLE messages (
                    rowid INTEGER PRIMARY KEY,
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    tool_activities TEXT NOT NULL DEFAULT '[]'
                );
                CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                );
                CREATE TABLE conversation_memory_excluded_tools (
                    conversation_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, tool_name)
                );
                CREATE TABLE conversation_memory_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                CREATE TABLE conversation_memory_tool_index_state (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    backfilled_at_unix_ms INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content,
                    tokenize = 'unicode61 remove_diacritics 2'
                );",
        )
        .expect("create FTS benchmark schema");
    let transaction = connection
        .unchecked_transaction()
        .expect("start conversation insert transaction");
    for index in 0..MESSAGE_COUNT {
        let content = if index == MESSAGE_COUNT / 2 {
            "The archive's distinctive parchmentquartz detail is here."
        } else {
            "Conversation record contains ordinary shared history context."
        };
        let tool_activities = if index % 100 == 0 {
            let tool_output = if index == MESSAGE_COUNT / 2 {
                "toolparchmentquartz appears in this archived tool result"
            } else {
                "ordinary indexed tool result"
            };
            serde_json::json!([{
                "callId": format!("call-{index}"),
                "name": "read",
                "arguments": {"path": "settings.json"},
                "output": {"content": tool_output},
                "targetPath": "privatepathmarker",
                "status": "completed"
            }])
            .to_string()
        } else {
            "[]".to_owned()
        };
        let role = if index % 100 == 0 {
            "assistant"
        } else {
            "user"
        };
        transaction
            .execute(
                "INSERT INTO messages
                        (id, conversation_id, role, content, status, created_at, tool_activities)
                     VALUES (?1, 'large-history', ?2, ?3, 'completed', ?4, ?5)",
                rusqlite::params![
                    format!("message-{index}"),
                    role,
                    content,
                    index,
                    tool_activities
                ],
            )
            .expect("insert conversation benchmark row");
    }
    transaction
        .commit()
        .expect("commit conversation benchmark rows");

    let search = search_expression("large-history", "parchmentquartz")
        .expect("build distinctive search query");
    let started = Instant::now();
    let backfilled_results = retrieve_archived_memories_from_connection(
        &connection,
        "large-history",
        &format!("message-{}", MESSAGE_COUNT - 1),
        &search,
        6,
        48,
    )
    .expect("backfill and search large conversation");
    let backfill_and_search_elapsed = started.elapsed();
    assert_eq!(backfilled_results.len(), 1);
    assert!(backfilled_results[0].content.contains("parchmentquartz"));

    let tool_search = search_expression("large-history", "toolparchmentquartz")
        .expect("build distinctive tool-result search query");
    let backfilled_tool_results = retrieve_archived_memories_from_connection(
        &connection,
        "large-history",
        &format!("message-{}", MESSAGE_COUNT - 1),
        &tool_search,
        6,
        48,
    )
    .expect("search archived tool results");
    assert_eq!(backfilled_tool_results.len(), 1);
    assert!(
        backfilled_tool_results[0]
            .content
            .contains("toolparchmentquartz")
    );

    let target_path_matches = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_memory_tools_fts
                 WHERE conversation_memory_tools_fts MATCH 'content : \"privatepathmarker\"'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("ensure the display-only target path is not indexed");
    assert_eq!(target_path_matches, 0);

    let started = Instant::now();
    let indexed_results = retrieve_archived_memories_from_connection(
        &connection,
        "large-history",
        &format!("message-{}", MESSAGE_COUNT - 1),
        &search,
        6,
        48,
    )
    .expect("search indexed large conversation");
    assert_eq!(indexed_results.len(), 1);
    assert!(indexed_results[0].content.contains("parchmentquartz"));

    let indexed_tool_results = retrieve_archived_memories_from_connection(
        &connection,
        "large-history",
        &format!("message-{}", MESSAGE_COUNT - 1),
        &tool_search,
        6,
        48,
    )
    .expect("search indexed tool results");
    assert_eq!(indexed_tool_results.len(), 1);
    assert!(
        indexed_tool_results[0]
            .content
            .contains("toolparchmentquartz")
    );
    let indexed_search_elapsed = started.elapsed();

    eprintln!(
        "FTS archive benchmark ({MESSAGE_COUNT} messages, {} tool activities): backfill + search {backfill_and_search_elapsed:?}; indexed message/tool searches {indexed_search_elapsed:?}",
        MESSAGE_COUNT / 100
    );
}
