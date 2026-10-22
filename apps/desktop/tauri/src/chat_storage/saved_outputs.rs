//! Saved outputs persistence, bounded history search, and versioning operations.

use rusqlite::{params, Connection};

use crate::request_bounds::BoundedText;

use super::{
    is_safe_identifier, is_valid_route_model_id, open_database, parse_stored_message_content,
    unix_time_milliseconds, ChatStorageError, SavedOutputMessageKey, SavedOutputSearchInput,
    SavedOutputSearchResult, SavedOutputSearchSortOrder, SavedOutputSummary, SavedOutputVersion,
    SavedOutputVersionSummary, MAXIMUM_CONVERSATION_TITLE_CHARS,
    MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES, MAXIMUM_HISTORY_SEARCH_QUERY_CHARS,
    MAXIMUM_HISTORY_SEARCH_RESULTS, MAXIMUM_HISTORY_SEARCH_TIMESTAMP,
    MAXIMUM_SAFE_IDENTIFIER_BYTES, MESSAGE_CONTENT_PREFIX,
};

const MAXIMUM_SAVED_OUTPUT_CONTENT_BYTES: usize = 2 * 1024 * 1024;
const MAXIMUM_SAVED_OUTPUT_VERSIONS: u32 = 1_000;
const MAXIMUM_SAVED_OUTPUT_OFFSET: u32 = 100_000;
const MAXIMUM_SAVED_OUTPUT_PAGE_SIZE: u32 = 50;

#[tauri::command]
/// Returns message keys for outputs that are saved outside normal transcript rendering.
///
/// # Errors
/// Returns a storage error if the database query fails.
pub fn list_saved_output_message_keys() -> Result<Vec<SavedOutputMessageKey>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT conversation_id, message_id
             FROM saved_outputs
             ORDER BY conversation_id ASC, message_id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SavedOutputMessageKey {
                conversation_id: row.get(0)?,
                message_id: row.get(1)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Searches saved outputs with conversation filters in one bounded database request.
///
/// Message bodies are decoded locally, and only matching excerpts are returned to the renderer.
///
/// # Errors
/// Returns a validation or storage error when the request or persisted content is invalid.
pub fn search_saved_outputs(
    input: SavedOutputSearchInput,
) -> Result<Vec<SavedOutputSearchResult>, ChatStorageError> {
    let connection = open_database()?;
    search_saved_outputs_from_connection(&connection, input)
}

pub(crate) fn search_saved_outputs_from_connection(
    connection: &Connection,
    input: SavedOutputSearchInput,
) -> Result<Vec<SavedOutputSearchResult>, ChatStorageError> {
    let normalized_query = input.query.to_lowercase();
    if input.query.trim() != input.query
        || input.query.chars().count() < 2
        || input.query.chars().count() > MAXIMUM_HISTORY_SEARCH_QUERY_CHARS
        || input
            .provider_id
            .as_deref()
            .is_some_and(|value| !is_safe_identifier(value))
        || input
            .model_id
            .as_deref()
            .is_some_and(|value| !is_valid_route_model_id(value))
        || input
            .project_id
            .as_deref()
            .is_some_and(|value| !is_safe_identifier(value))
        || input
            .from_unix_ms
            .is_some_and(|value| !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&value))
        || input
            .through_unix_ms
            .is_some_and(|value| !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&value))
        || input
            .from_unix_ms
            .zip(input.through_unix_ms)
            .is_some_and(|(from, through)| from >= through)
    {
        return Err(ChatStorageError::invalid_input());
    }

    let order_clause = match input.sort_order {
        SavedOutputSearchSortOrder::Relevance => "c.id ASC, m.created_at ASC, m.id ASC",
        SavedOutputSearchSortOrder::Newest => {
            "COALESCE(m.created_at, c.updated_at) DESC, c.id ASC, m.created_at ASC, m.id ASC"
        }
        SavedOutputSearchSortOrder::Oldest => {
            "COALESCE(m.created_at, c.updated_at) ASC, c.id ASC, m.created_at ASC, m.id ASC"
        }
    };
    let sql = format!(
        "SELECT c.id, c.title, c.project_id, c.is_archived, c.updated_at,
                m.id, m.role, m.content, m.created_at, m.provider_id, m.model_id
         FROM saved_outputs s
         JOIN conversations c ON c.id = s.conversation_id
         JOIN messages m ON m.conversation_id = s.conversation_id AND m.id = s.message_id
         WHERE (?1 IS NULL OR c.project_id = ?1)
           AND (?2 IS NULL OR c.is_archived = ?2)
           AND (?3 IS NULL OR m.provider_id = ?3)
           AND (?4 IS NULL OR m.model_id = ?4)
           AND (?5 IS NULL OR (m.created_at IS NOT NULL AND m.created_at >= ?5))
           AND (?6 IS NULL OR (m.created_at IS NOT NULL AND m.created_at < ?6))
         ORDER BY {order_clause}"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut rows = statement
        .query(params![
            input.project_id.as_deref(),
            input.is_archived.map(i64::from),
            input.provider_id.as_deref(),
            input.model_id.as_deref(),
            input.from_unix_ms,
            input.through_unix_ms,
        ])
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut results = Vec::with_capacity(MAXIMUM_HISTORY_SEARCH_RESULTS);

    while let Some(row) = rows.next().map_err(|_| ChatStorageError::unavailable())? {
        let conversation_id = row
            .get::<_, String>(0)
            .map_err(|_| ChatStorageError::unavailable())?;
        let conversation_title = row
            .get::<_, String>(1)
            .map_err(|_| ChatStorageError::unavailable())?;
        let project_id = row
            .get::<_, Option<String>>(2)
            .map_err(|_| ChatStorageError::unavailable())?;
        let is_archived = row
            .get::<_, bool>(3)
            .map_err(|_| ChatStorageError::unavailable())?;
        let conversation_updated_at = row
            .get::<_, i64>(4)
            .map_err(|_| ChatStorageError::unavailable())?;
        let message_id = row
            .get::<_, String>(5)
            .map_err(|_| ChatStorageError::unavailable())?;
        let role = row
            .get::<_, String>(6)
            .map_err(|_| ChatStorageError::unavailable())?;
        let stored_content = row
            .get::<_, String>(7)
            .map_err(|_| ChatStorageError::unavailable())?;
        let message_created_at = row
            .get::<_, Option<i64>>(8)
            .map_err(|_| ChatStorageError::unavailable())?;

        if input
            .is_archived
            .is_some_and(|expected| expected != is_archived)
        {
            continue;
        }
        if !is_safe_identifier(&conversation_id)
            || !is_safe_identifier(&message_id)
            || conversation_title.trim().is_empty()
            || conversation_title.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
            || (role != "user" && role != "assistant")
        {
            return Err(ChatStorageError::unavailable());
        }

        let content = if stored_content.starts_with(MESSAGE_CONTENT_PREFIX) {
            parse_stored_message_content(&stored_content)?._content
        } else {
            stored_content
        };
        let title_matches = conversation_title
            .to_lowercase()
            .contains(&normalized_query);
        let content_matches = content.to_lowercase().contains(&normalized_query);
        if !title_matches && !content_matches {
            continue;
        }
        let created_at_unix_ms = message_created_at.unwrap_or(conversation_updated_at);
        if !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&created_at_unix_ms) {
            return Err(ChatStorageError::unavailable());
        }

        results.push(SavedOutputSearchResult {
            conversation_id,
            conversation_title,
            is_conversation_title_match: title_matches && !content_matches,
            message_id,
            role,
            excerpt: bounded_history_search_excerpt(&content),
            created_at_unix_ms,
            project_id,
        });
        if results.len() == MAXIMUM_HISTORY_SEARCH_RESULTS {
            break;
        }
    }

    Ok(results)
}

pub(crate) fn bounded_history_search_excerpt(content: &str) -> String {
    let mut end = 0;
    for (start, character) in content.char_indices() {
        let next = start + character.len_utf8();
        if next > MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES {
            break;
        }
        end = next;
    }
    content[..end].to_owned()
}

#[tauri::command]
/// Adds or removes the selected message from saved outputs.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_saved_output(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    is_saved: bool,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    if !is_safe_identifier(&conversation_id) || !is_safe_identifier(&message_id) {
        return Err(ChatStorageError::invalid_input());
    }

    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;

    if is_saved {
        let now = unix_time_milliseconds()?;
        let inserted_rows = transaction
            .execute(
                "INSERT OR IGNORE INTO saved_outputs (
                    conversation_id, message_id, conversation_title, content,
                    provider_id, model_id, saved_at, selected_version
                 )
                 SELECT c.id, m.id, c.title, m.content, m.provider_id, m.model_id, ?1, 1
                 FROM messages AS m
                 INNER JOIN conversations AS c ON c.id = m.conversation_id
                 WHERE c.id = ?2 AND m.id = ?3
                    AND m.role = 'assistant' AND m.status = 'completed'
                    AND length(trim(m.content)) > 0",
                params![now, conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if inserted_rows == 0 {
            let already_saved = transaction
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM saved_outputs
                        WHERE conversation_id = ?1 AND message_id = ?2
                    )",
                    params![conversation_id, message_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|_| ChatStorageError::unavailable())?;
            if !already_saved {
                return Err(ChatStorageError::invalid_input());
            }
        } else {
            transaction
                .execute(
                    "INSERT INTO saved_output_versions (
                        conversation_id, message_id, version_number, content, created_at
                     )
                     SELECT conversation_id, message_id, 1, content, saved_at
                     FROM saved_outputs
                     WHERE conversation_id = ?1 AND message_id = ?2",
                    params![conversation_id, message_id],
                )
                .map_err(|_| ChatStorageError::unavailable())?;
        }
    } else {
        transaction
            .execute(
                "DELETE FROM saved_output_versions
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        transaction
            .execute(
                "DELETE FROM saved_outputs
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![conversation_id, message_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }

    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Returns one bounded page of saved assistant responses.
pub fn list_saved_outputs(
    offset: u32,
    limit: u32,
) -> Result<Vec<SavedOutputSummary>, ChatStorageError> {
    if offset > MAXIMUM_SAVED_OUTPUT_OFFSET || limit == 0 || limit > MAXIMUM_SAVED_OUTPUT_PAGE_SIZE
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT s.conversation_id, c.title, s.message_id, s.content, s.saved_at,
                    s.selected_version, COUNT(v.version_number)
             FROM saved_outputs s
             INNER JOIN conversations c ON c.id = s.conversation_id
             LEFT JOIN saved_output_versions v
               ON v.conversation_id = s.conversation_id AND v.message_id = s.message_id
             GROUP BY s.conversation_id, s.message_id
             ORDER BY s.saved_at DESC, s.conversation_id ASC, s.message_id ASC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut rows = statement
        .query(rusqlite::params![i64::from(limit), i64::from(offset)])
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut results = Vec::with_capacity(limit as usize);
    while let Some(row) = rows.next().map_err(|_| ChatStorageError::unavailable())? {
        let conversation_id = row
            .get::<_, String>(0)
            .map_err(|_| ChatStorageError::unavailable())?;
        let conversation_title = row
            .get::<_, String>(1)
            .map_err(|_| ChatStorageError::unavailable())?;
        let message_id = row
            .get::<_, String>(2)
            .map_err(|_| ChatStorageError::unavailable())?;
        let stored_content = row
            .get::<_, String>(3)
            .map_err(|_| ChatStorageError::unavailable())?;
        let created_at_unix_ms = row
            .get::<_, i64>(4)
            .map_err(|_| ChatStorageError::unavailable())?;
        let selected_version = row
            .get::<_, i64>(5)
            .map_err(|_| ChatStorageError::unavailable())?;
        let version_count = row
            .get::<_, i64>(6)
            .map_err(|_| ChatStorageError::unavailable())?;
        if !is_safe_identifier(&conversation_id)
            || !is_safe_identifier(&message_id)
            || conversation_title.trim().is_empty()
            || conversation_title.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
            || !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&created_at_unix_ms)
            || !(1..=i64::from(MAXIMUM_SAVED_OUTPUT_VERSIONS)).contains(&selected_version)
            || !(1..=i64::from(MAXIMUM_SAVED_OUTPUT_VERSIONS)).contains(&version_count)
        {
            return Err(ChatStorageError::unavailable());
        }
        let content = decode_saved_output_content(&stored_content)?;
        results.push(SavedOutputSummary {
            conversation_id,
            conversation_title,
            message_id,
            excerpt: bounded_history_search_excerpt(&content),
            created_at_unix_ms,
            selected_version: selected_version as u32,
            version_count: version_count as u32,
        });
    }
    Ok(results)
}

#[tauri::command]
/// Lists version metadata without loading every saved response body.
pub fn list_saved_output_versions(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<Vec<SavedOutputVersionSummary>, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    validate_saved_output_key(&conversation_id, &message_id)?;
    let connection = open_database()?;
    ensure_saved_output_exists(&connection, &conversation_id, &message_id)?;
    let mut statement = connection
        .prepare(
            "SELECT version_number, created_at
             FROM saved_output_versions
             WHERE conversation_id = ?1 AND message_id = ?2
             ORDER BY version_number ASC
             LIMIT ?3",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut rows = statement
        .query(rusqlite::params![
            conversation_id,
            message_id,
            i64::from(MAXIMUM_SAVED_OUTPUT_VERSIONS) + 1,
        ])
        .map_err(|_| ChatStorageError::unavailable())?;
    let mut versions = Vec::new();
    while let Some(row) = rows.next().map_err(|_| ChatStorageError::unavailable())? {
        let version_number = row
            .get::<_, i64>(0)
            .map_err(|_| ChatStorageError::unavailable())?;
        let created_at_unix_ms = row
            .get::<_, i64>(1)
            .map_err(|_| ChatStorageError::unavailable())?;
        if !(1..=i64::from(MAXIMUM_SAVED_OUTPUT_VERSIONS)).contains(&version_number)
            || !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&created_at_unix_ms)
            || versions.len() >= MAXIMUM_SAVED_OUTPUT_VERSIONS as usize
        {
            return Err(ChatStorageError::unavailable());
        }
        versions.push(SavedOutputVersionSummary {
            version_number: version_number as u32,
            created_at_unix_ms,
        });
    }
    if versions.is_empty() {
        return Err(ChatStorageError::unavailable());
    }
    Ok(versions)
}

#[tauri::command]
/// Loads one saved response version after confirming it belongs to a saved output.
pub fn get_saved_output_version(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    version_number: u32,
) -> Result<SavedOutputVersion, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    validate_saved_output_key(&conversation_id, &message_id)?;
    if !(1..=MAXIMUM_SAVED_OUTPUT_VERSIONS).contains(&version_number) {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    ensure_saved_output_exists(&connection, &conversation_id, &message_id)?;
    let (stored_content, created_at_unix_ms) = connection
        .query_row(
            "SELECT content, created_at
             FROM saved_output_versions
             WHERE conversation_id = ?1 AND message_id = ?2 AND version_number = ?3",
            rusqlite::params![conversation_id, message_id, i64::from(version_number)],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if !(0..=MAXIMUM_HISTORY_SEARCH_TIMESTAMP).contains(&created_at_unix_ms) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(SavedOutputVersion {
        version_number,
        created_at_unix_ms,
        content: decode_saved_output_content(&stored_content)?,
    })
}

#[tauri::command]
/// Stores an edited response as a new version and selects it without changing the transcript.
pub fn save_saved_output_version(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    content: BoundedText<MAXIMUM_SAVED_OUTPUT_CONTENT_BYTES>,
) -> Result<SavedOutputVersion, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    let content = content.into_inner();
    validate_saved_output_key(&conversation_id, &message_id)?;
    if content.trim().is_empty() || content.contains('\0') {
        return Err(ChatStorageError::invalid_input());
    }
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    ensure_saved_output_exists(&transaction, &conversation_id, &message_id)?;
    let maximum_version = transaction
        .query_row(
            "SELECT COALESCE(MAX(version_number), 0)
             FROM saved_output_versions
             WHERE conversation_id = ?1 AND message_id = ?2",
            rusqlite::params![conversation_id, message_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let next_version = maximum_version
        .checked_add(1)
        .ok_or_else(ChatStorageError::unavailable)?;
    if !(1..=i64::from(MAXIMUM_SAVED_OUTPUT_VERSIONS)).contains(&next_version) {
        return Err(ChatStorageError::invalid_input());
    }
    let created_at_unix_ms = unix_time_milliseconds()?;
    transaction
        .execute(
            "INSERT INTO saved_output_versions
             (conversation_id, message_id, version_number, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                conversation_id,
                message_id,
                next_version,
                content,
                created_at_unix_ms
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "UPDATE saved_outputs
             SET content = ?1, selected_version = ?2
             WHERE conversation_id = ?3 AND message_id = ?4",
            rusqlite::params![content, next_version, conversation_id, message_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(SavedOutputVersion {
        version_number: next_version as u32,
        created_at_unix_ms,
        content,
    })
}

#[tauri::command]
/// Changes the selected saved response version without editing conversation history.
pub fn select_saved_output_version(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    version_number: u32,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    validate_saved_output_key(&conversation_id, &message_id)?;
    if !(1..=MAXIMUM_SAVED_OUTPUT_VERSIONS).contains(&version_number) {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE saved_outputs
             SET selected_version = ?1,
                 content = (
                   SELECT content FROM saved_output_versions
                   WHERE conversation_id = ?2 AND message_id = ?3 AND version_number = ?1
                 )
             WHERE conversation_id = ?2 AND message_id = ?3
               AND EXISTS (
                 SELECT 1 FROM saved_output_versions
                 WHERE conversation_id = ?2 AND message_id = ?3 AND version_number = ?1
               )",
            rusqlite::params![i64::from(version_number), conversation_id, message_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_saved_output_key(
    conversation_id: &str,
    message_id: &str,
) -> Result<(), ChatStorageError> {
    if is_safe_identifier(conversation_id) && is_safe_identifier(message_id) {
        Ok(())
    } else {
        Err(ChatStorageError::invalid_input())
    }
}

fn ensure_saved_output_exists(
    connection: &Connection,
    conversation_id: &str,
    message_id: &str,
) -> Result<(), ChatStorageError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM saved_outputs WHERE conversation_id = ?1 AND message_id = ?2
             )",
            rusqlite::params![conversation_id, message_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if exists {
        Ok(())
    } else {
        Err(ChatStorageError::invalid_input())
    }
}

fn decode_saved_output_content(content: &str) -> Result<String, ChatStorageError> {
    if content.starts_with(MESSAGE_CONTENT_PREFIX) {
        Ok(parse_stored_message_content(content)?._content)
    } else {
        Ok(content.to_owned())
    }
}

#[cfg(test)]
mod saved_output_search_benchmarks {
    use std::{
        collections::{BTreeMap, HashSet},
        time::{Duration, Instant},
    };

    use rusqlite::{params, Connection};
    use serde_json::json;

    use super::{
        search_saved_outputs_from_connection, SavedOutputSearchInput, SavedOutputSearchSortOrder,
        MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES,
    };
    use crate::chat_storage::{
        migrate_chat_schema, stored_conversation_from_row, StoredMessage,
        MAXIMUM_HISTORY_SEARCH_QUERY_BYTES,
    };

    const CONVERSATION_COUNT: usize = 250;
    const MESSAGES_PER_CONVERSATION: usize = 200;

    #[derive(Debug)]
    struct LegacySearchMetrics {
        elapsed: Duration,
        database_queries: usize,
        messages_materialized: usize,
        serialized_bytes: usize,
        matching_saved_outputs: usize,
    }

    #[derive(Debug)]
    struct SavedOutputSearchMetrics {
        elapsed: Duration,
        database_queries: usize,
        messages_returned: usize,
        serialized_bytes: usize,
    }

    fn search_input(query: &str) -> SavedOutputSearchInput {
        SavedOutputSearchInput {
            query: query.to_owned(),
            provider_id: None,
            model_id: None,
            project_id: None,
            is_archived: None,
            sort_order: SavedOutputSearchSortOrder::Relevance,
            from_unix_ms: None,
            through_unix_ms: None,
        }
    }

    fn empty_search_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        migrate_chat_schema(&mut connection).expect("current chat schema should be created");
        connection
            .execute_batch("ALTER TABLE conversations ADD COLUMN model_id TEXT;")
            .expect("service-owned route metadata should be added");
        connection
    }

    fn insert_search_conversation(
        connection: &Connection,
        id: &str,
        title: &str,
        project_id: Option<&str>,
        tags: &[&str],
        archived: bool,
        updated_at: i64,
    ) {
        let tags_json = serde_json::to_string(tags).expect("conversation tags should serialize");
        connection
            .execute(
                "INSERT INTO conversations (
                    id, title, title_source, provider_id, model_id, project_id, tags,
                    is_archived, created_at, updated_at
                 ) VALUES (?1, ?2, 'manual', 'chatgpt', 'fixture-model', ?3, ?4, ?5, 1, ?6)",
                params![id, title, project_id, tags_json, archived, updated_at],
            )
            .expect("conversation search fixture should insert");
    }

    struct SearchMessageFixture<'a> {
        message_id: &'a str,
        role: &'a str,
        content: &'a str,
        created_at: Option<i64>,
        provider_id: &'a str,
        model_id: &'a str,
        conversation_title: &'a str,
    }

    fn insert_saved_search_message(
        connection: &Connection,
        conversation_id: &str,
        message: SearchMessageFixture<'_>,
    ) {
        connection
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, created_at,
                    provider_id, model_id, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'completed')",
                params![
                    message.message_id,
                    conversation_id,
                    message.role,
                    message.content,
                    message.created_at,
                    message.provider_id,
                    message.model_id
                ],
            )
            .expect("message search fixture should insert");
        connection
            .execute(
                "INSERT INTO saved_outputs (
                    conversation_id, message_id, conversation_title, content,
                    provider_id, model_id, saved_at, selected_version
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 1)",
                params![
                    conversation_id,
                    message.message_id,
                    message.conversation_title,
                    message.content,
                    message.provider_id,
                    message.model_id
                ],
            )
            .expect("saved output search fixture should insert");
    }

    fn seeded_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        migrate_chat_schema(&mut connection).expect("current chat schema should be created");
        connection
            .execute_batch("ALTER TABLE conversations ADD COLUMN model_id TEXT;")
            .expect("service-owned route metadata should be added");
        let transaction = connection
            .transaction()
            .expect("fixture transaction should start");
        {
            let mut insert_conversation = transaction
                .prepare_cached(
                    "INSERT INTO conversations (id, title, title_source, created_at, updated_at)
                     VALUES (?1, ?2, 'manual', 1, ?3)",
                )
                .expect("conversation insert should prepare");
            let mut insert_message = transaction
                .prepare_cached(
                    "INSERT INTO messages (id, conversation_id, role, content, created_at, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'completed')",
                )
                .expect("message insert should prepare");
            let mut insert_saved_output = transaction
                .prepare_cached(
                    "INSERT INTO saved_outputs (
                        conversation_id, message_id, conversation_title, content,
                        provider_id, model_id, saved_at, selected_version
                     ) VALUES (?1, ?2, ?3, ?4, 'chatgpt', 'fixture-model', ?5, 1)",
                )
                .expect("saved-output insert should prepare");

            for conversation_number in 0..CONVERSATION_COUNT {
                let conversation_id = format!("conversation-{conversation_number:04}");
                let title = format!("Conversation {conversation_number:04}");
                insert_conversation
                    .execute(params![conversation_id, title, conversation_number as i64])
                    .expect("conversation fixture should insert");

                for message_number in 0..MESSAGES_PER_CONVERSATION {
                    let message_id = format!("message-{message_number:04}");
                    let content = if message_number % 100 == 99 {
                        format!(
                            "Saved output needle {conversation_number}-{message_number} {}",
                            "x".repeat(192)
                        )
                    } else {
                        format!(
                            "Ordinary response {conversation_number}-{message_number} {}",
                            "x".repeat(192)
                        )
                    };
                    let created_at =
                        (conversation_number * MESSAGES_PER_CONVERSATION + message_number) as i64;
                    let role = if message_number % 2 == 1 {
                        "assistant"
                    } else {
                        "user"
                    };
                    insert_message
                        .execute(params![
                            message_id,
                            conversation_id,
                            role,
                            content,
                            created_at
                        ])
                        .expect("message fixture should insert");
                    if message_number % 100 == 99 {
                        insert_saved_output
                            .execute(params![
                                conversation_id,
                                message_id,
                                title,
                                content,
                                created_at
                            ])
                            .expect("saved-output fixture should insert");
                    }
                }
            }
        }
        transaction
            .commit()
            .expect("fixture transaction should commit");
        connection
    }

    fn run_legacy_search(connection: &Connection) -> LegacySearchMetrics {
        let started = Instant::now();
        let saved_keys = {
            let mut statement = connection
                .prepare(
                    "SELECT conversation_id, message_id FROM saved_outputs
                     ORDER BY conversation_id ASC, message_id ASC",
                )
                .expect("saved keys query should prepare");
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .expect("saved keys query should run");
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .expect("saved keys should decode")
        };
        let mut saved_by_conversation = BTreeMap::<String, HashSet<String>>::new();
        for (conversation_id, message_id) in &saved_keys {
            saved_by_conversation
                .entry(conversation_id.clone())
                .or_default()
                .insert(message_id.clone());
        }
        let conversations = {
            let mut statement = connection
                .prepare(
                    "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id,
                            is_pinned, is_archived, created_at, updated_at
                     FROM conversations
                     ORDER BY is_pinned DESC, updated_at DESC, id ASC",
                )
                .expect("conversation list query should prepare");
            let rows = statement
                .query_map([], stored_conversation_from_row)
                .expect("conversation list query should run");
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .expect("conversation rows should decode")
        };
        let mut serialized_bytes = serde_json::to_vec(&saved_keys)
            .expect("saved keys should serialize")
            .len()
            + serde_json::to_vec(&conversations)
                .expect("conversations should serialize")
                .len();
        let mut messages_materialized = 0;
        let mut matching_saved_outputs = 0;

        for (conversation_id, saved_message_ids) in &saved_by_conversation {
            let messages = {
                let mut statement = connection
                    .prepare(
                        "SELECT id, conversation_id, role, content, created_at, output_tokens,
                                tokens_per_second, elapsed_microseconds, provider_id, model_id,
                                citation_sources, personal_memory_fact_ids, reasoning_summaries,
                                tool_activities, status, failure_code
                         FROM messages WHERE conversation_id = ?1
                         ORDER BY created_at ASC, id ASC LIMIT 10000",
                    )
                    .expect("per-conversation message query should prepare");
                let rows = statement
                    .query_map([conversation_id], |row| {
                        Ok(StoredMessage {
                            id: row.get(0)?,
                            conversation_id: row.get(1)?,
                            role: row.get(2)?,
                            content: row.get(3)?,
                            created_at: row.get(4)?,
                            output_tokens: row.get(5)?,
                            tokens_per_second: row.get(6)?,
                            elapsed_microseconds: row.get(7)?,
                            provider_id: row.get(8)?,
                            model_id: row.get(9)?,
                            citation_sources: row.get(10)?,
                            personal_memory_fact_ids: row.get(11)?,
                            reasoning_summaries: row.get(12)?,
                            tool_activities: row.get(13)?,
                            status: row.get(14)?,
                            failure_code: row.get(15)?,
                        })
                    })
                    .expect("per-conversation message query should run");
                rows.collect::<rusqlite::Result<Vec<_>>>()
                    .expect("messages should decode")
            };
            messages_materialized += messages.len();
            serialized_bytes += serde_json::to_vec(&messages)
                .expect("per-conversation messages should serialize")
                .len();
            matching_saved_outputs += messages
                .iter()
                .filter(|message| {
                    saved_message_ids.contains(&message.id)
                        && message.content.to_lowercase().contains("needle")
                })
                .count();
        }

        LegacySearchMetrics {
            elapsed: started.elapsed(),
            database_queries: 2 + saved_by_conversation.len(),
            messages_materialized,
            serialized_bytes,
            matching_saved_outputs,
        }
    }

    fn run_saved_output_search(connection: &Connection) -> SavedOutputSearchMetrics {
        let started = Instant::now();
        let results = search_saved_outputs_from_connection(connection, search_input("needle"))
            .expect("saved-output history search should succeed");
        let elapsed = started.elapsed();
        let serialized_bytes = serde_json::to_vec(&results)
            .expect("saved-output search results should serialize")
            .len();

        SavedOutputSearchMetrics {
            elapsed,
            database_queries: 1,
            messages_returned: results.len(),
            serialized_bytes,
        }
    }

    #[test]
    fn saved_output_search_decodes_visible_attachment_content_and_matches_titles() {
        let connection = empty_search_connection();
        insert_search_conversation(
            &connection,
            "conversation-title",
            "Needle in the title",
            None,
            &[],
            false,
            10,
        );
        insert_saved_search_message(
            &connection,
            "conversation-title",
            SearchMessageFixture {
                message_id: "message-title",
                role: "assistant",
                content: "This body does not contain the term.",
                created_at: Some(10),
                provider_id: "chatgpt",
                model_id: "fixture-model",
                conversation_title: "Needle in the title",
            },
        );
        insert_search_conversation(
            &connection,
            "conversation-visible-content",
            "A different title",
            None,
            &[],
            false,
            20,
        );
        let attachment_envelope = format!(
            "{}{}",
            super::MESSAGE_CONTENT_PREFIX,
            json!({
                "content": "Needle in visible attachment conversation text",
                "attachments": [{
                    "id": "attachment-1",
                    "name": "notes.txt",
                    "mimeType": "text/plain",
                    "sizeBytes": 4,
                    "kind": "text"
                }]
            })
        );
        insert_saved_search_message(
            &connection,
            "conversation-visible-content",
            SearchMessageFixture {
                message_id: "message-visible-content",
                role: "user",
                content: &attachment_envelope,
                created_at: Some(20),
                provider_id: "chatgpt",
                model_id: "fixture-model",
                conversation_title: "A different title",
            },
        );

        let results = search_saved_outputs_from_connection(&connection, search_input("needle"))
            .expect("saved outputs should be searched");

        assert_eq!(results.len(), 2);
        assert!(results[0].is_conversation_title_match);
        assert_eq!(results[0].excerpt, "This body does not contain the term.");
        assert!(!results[1].is_conversation_title_match);
        assert_eq!(
            results[1].excerpt,
            "Needle in visible attachment conversation text"
        );
    }

    #[test]
    fn saved_output_search_applies_conversation_message_date_and_sort_filters() {
        let connection = empty_search_connection();
        connection
            .execute(
                "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
                 VALUES ('project-1', 'Project 1', 'C:/project-1', 1, 1)",
                [],
            )
            .expect("project fixture should insert");
        insert_search_conversation(
            &connection,
            "conversation-filtered",
            "Filtered conversation",
            Some("project-1"),
            &["work"],
            true,
            50,
        );
        for (message_id, created_at) in [("message-old", 20), ("message-new", 40)] {
            insert_saved_search_message(
                &connection,
                "conversation-filtered",
                SearchMessageFixture {
                    message_id,
                    role: "assistant",
                    content: "Needle in this response",
                    created_at: Some(created_at),
                    provider_id: "chatgpt",
                    model_id: "fixture-model",
                    conversation_title: "Filtered conversation",
                },
            );
        }
        let mut input = search_input("needle");
        input.provider_id = Some("chatgpt".to_owned());
        input.model_id = Some("fixture-model".to_owned());
        input.project_id = Some("project-1".to_owned());
        input.is_archived = Some(true);
        input.sort_order = SavedOutputSearchSortOrder::Newest;
        input.from_unix_ms = Some(20);
        input.through_unix_ms = Some(50);

        let results = search_saved_outputs_from_connection(&connection, input)
            .expect("all saved-output filters should be applied");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].message_id, "message-new");
        assert_eq!(results[1].message_id, "message-old");

        let mut input = search_input("needle");
        input.sort_order = SavedOutputSearchSortOrder::Oldest;
        input.from_unix_ms = Some(20);
        input.through_unix_ms = Some(50);
        let results = search_saved_outputs_from_connection(&connection, input)
            .expect("oldest ordering should complete");
        assert_eq!(results[0].message_id, "message-old");
        assert_eq!(results[1].message_id, "message-new");

        let mut input = search_input("needle");
        input.project_id = Some("another-project".to_owned());
        assert!(search_saved_outputs_from_connection(&connection, input)
            .expect("project filter should complete")
            .is_empty());

        let mut input = search_input("needle");
        input.provider_id = Some("another-provider".to_owned());
        assert!(search_saved_outputs_from_connection(&connection, input)
            .expect("provider filter should complete")
            .is_empty());

        let mut input = search_input("needle");
        input.model_id = Some("another-model".to_owned());
        assert!(search_saved_outputs_from_connection(&connection, input)
            .expect("model filter should complete")
            .is_empty());

        let mut input = search_input("needle");
        input.is_archived = Some(false);
        assert!(search_saved_outputs_from_connection(&connection, input)
            .expect("archive filter should complete")
            .is_empty());

        let mut input = search_input("needle");
        input.from_unix_ms = Some(40);
        input.through_unix_ms = Some(41);
        let results = search_saved_outputs_from_connection(&connection, input)
            .expect("date range should complete");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message_id, "message-new");
    }

    #[test]
    fn saved_output_search_bounds_result_count_and_excerpt_size() {
        let connection = empty_search_connection();
        insert_search_conversation(
            &connection,
            "conversation-many",
            "Many matching messages",
            None,
            &[],
            false,
            100,
        );
        let long_content = format!(
            "Needle {}",
            "x".repeat(MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES * 2)
        );
        for index in 0..35 {
            let message_id = format!("message-{index:02}");
            insert_saved_search_message(
                &connection,
                "conversation-many",
                SearchMessageFixture {
                    message_id: &message_id,
                    role: "assistant",
                    content: &long_content,
                    created_at: Some(index),
                    provider_id: "chatgpt",
                    model_id: "fixture-model",
                    conversation_title: "Many matching messages",
                },
            );
        }

        let results = search_saved_outputs_from_connection(&connection, search_input("needle"))
            .expect("bounded saved-output search should succeed");

        assert_eq!(results.len(), 30);
        assert!(results
            .iter()
            .all(|result| { result.excerpt.len() <= MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES }));
    }

    #[test]
    fn saved_output_search_rejects_invalid_queries_ranges_and_attachment_envelopes() {
        let connection = empty_search_connection();
        let too_short = search_input("x");
        assert!(search_saved_outputs_from_connection(&connection, too_short).is_err());

        let mut reversed_range = search_input("needle");
        reversed_range.from_unix_ms = Some(20);
        reversed_range.through_unix_ms = Some(20);
        assert!(search_saved_outputs_from_connection(&connection, reversed_range).is_err());

        insert_search_conversation(
            &connection,
            "conversation-invalid-envelope",
            "Invalid attachment envelope",
            None,
            &[],
            false,
            10,
        );
        insert_saved_search_message(
            &connection,
            "conversation-invalid-envelope",
            SearchMessageFixture {
                message_id: "message-invalid-envelope",
                role: "assistant",
                content: &format!("{}{{invalid", super::MESSAGE_CONTENT_PREFIX),
                created_at: Some(10),
                provider_id: "chatgpt",
                model_id: "fixture-model",
                conversation_title: "Invalid attachment envelope",
            },
        );
        let result = search_saved_outputs_from_connection(&connection, search_input("needle"));
        assert!(result.is_err());

        let oversized_query = json!({
            "query": "x".repeat(MAXIMUM_HISTORY_SEARCH_QUERY_BYTES + 1),
            "sortOrder": "relevance"
        });
        assert!(serde_json::from_value::<SavedOutputSearchInput>(oversized_query).is_err());
    }

    #[test]
    #[ignore = "manual synthetic benchmark for saved-output history search"]
    fn benchmark_legacy_saved_output_history_search() {
        let connection = seeded_connection();
        let mut legacy_samples = Vec::with_capacity(3);
        let mut optimized_samples = Vec::with_capacity(3);
        for _ in 0..3 {
            legacy_samples.push(run_legacy_search(&connection));
            optimized_samples.push(run_saved_output_search(&connection));
        }
        legacy_samples.sort_by_key(|sample| sample.elapsed);
        optimized_samples.sort_by_key(|sample| sample.elapsed);
        let legacy_median = &legacy_samples[1];
        let optimized_median = &optimized_samples[1];

        assert_eq!(legacy_median.matching_saved_outputs, CONVERSATION_COUNT * 2);
        assert_eq!(
            legacy_median.messages_materialized,
            CONVERSATION_COUNT * MESSAGES_PER_CONVERSATION
        );
        assert_eq!(optimized_median.messages_returned, 30);
        eprintln!(
            "legacy saved-output search: conversations={CONVERSATION_COUNT} messages={} saved={} matches={} database_queries={} median_ms={:.3} serialized_bytes={}",
            CONVERSATION_COUNT * MESSAGES_PER_CONVERSATION,
            CONVERSATION_COUNT * 2,
            legacy_median.matching_saved_outputs,
            legacy_median.database_queries,
            legacy_median.elapsed.as_secs_f64() * 1_000.0,
            legacy_median.serialized_bytes,
        );
        eprintln!(
            "bounded saved-output search: conversations={CONVERSATION_COUNT} saved={} returned={} database_queries={} median_ms={:.3} serialized_bytes={}",
            CONVERSATION_COUNT * 2,
            optimized_median.messages_returned,
            optimized_median.database_queries,
            optimized_median.elapsed.as_secs_f64() * 1_000.0,
            optimized_median.serialized_bytes,
        );
    }
}
