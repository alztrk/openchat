use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, params};

use super::{ArchivedMemoryExcerpt, HistorySearchFilters, HistorySearchResult, index};

const MAX_SEARCH_TERMS: usize = 12;
const MAX_SEARCH_TERM_CHARS: usize = 128;
const MAX_RESULTS: i64 = 16;
const MAX_SNIPPET_WORDS: i64 = 256;
pub(super) const MAX_ARCHIVE_SEARCH_RESULTS: i64 = 20;
pub(super) const MAX_HISTORY_SEARCH_RESULTS: i64 = 30;
pub(super) const ARCHIVE_SEARCH_SNIPPET_WORDS: i64 = 48;
pub(super) const MAX_ARCHIVE_SEARCH_QUERY_CHARS: usize = 512;
pub(super) const MAX_ARCHIVE_EXCERPT_CHARS: usize = 4096;
const MIN_MEMORY_CONTEXT_WINDOW: i64 = 2048;
const MEMORY_WORDS_PER_CONTEXT_TOKEN: i64 = 48;
const WORD_BUDGET_PER_RESULT: i64 = 128;
const MAX_MEMORY_CONTEXT_WORDS: i64 = MAX_RESULTS * MAX_SNIPPET_WORDS;

pub(super) fn combine_archive_results(
    lexical_results: Vec<ArchivedMemoryExcerpt>,
    semantic_results: Vec<ArchivedMemoryExcerpt>,
    result_limit: usize,
) -> Vec<ArchivedMemoryExcerpt> {
    let mut ranked = HashMap::<String, (ArchivedMemoryExcerpt, f64)>::new();
    for source_results in [lexical_results, semantic_results] {
        for (rank, excerpt) in source_results.into_iter().enumerate() {
            let reciprocal_rank = 1.0 / (60.0 + rank as f64 + 1.0);
            ranked
                .entry(excerpt.message_id.clone())
                .and_modify(|(_, score)| *score += reciprocal_rank)
                .or_insert((excerpt, reciprocal_rank));
        }
    }
    let mut results = ranked.into_values().collect::<Vec<_>>();
    results.sort_by(|(left_excerpt, left_score), (right_excerpt, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| {
                right_excerpt
                    .created_at_unix_ms
                    .cmp(&left_excerpt.created_at_unix_ms)
            })
            .then_with(|| left_excerpt.message_id.cmp(&right_excerpt.message_id))
    });
    results
        .into_iter()
        .take(result_limit)
        .map(|(excerpt, _)| excerpt)
        .collect()
}
pub(super) fn retrieve_archived_memories_from_connection(
    connection: &Connection,
    conversation_id: &str,
    through_message_id: &str,
    search_expression: &str,
    result_limit: i64,
    snippet_words: i64,
) -> rusqlite::Result<Vec<ArchivedMemoryExcerpt>> {
    search_archived_memories_from_connection(
        connection,
        conversation_id,
        Some(through_message_id),
        search_expression,
        result_limit,
        snippet_words,
    )
}

pub(super) fn search_archived_memories_from_connection(
    connection: &Connection,
    conversation_id: &str,
    through_message_id: Option<&str>,
    search_expression: &str,
    result_limit: i64,
    snippet_words: i64,
) -> rusqlite::Result<Vec<ArchivedMemoryExcerpt>> {
    if !index::archive_indexing_enabled(connection, conversation_id)? {
        return Ok(Vec::new());
    }
    index::ensure_archived_memory_indexes(connection, conversation_id)?;

    let mut statement = connection.prepare(
        "WITH matches AS (
             SELECT conversation_memory_fts.message_id, conversation_memory_fts.role,
                    snippet(conversation_memory_fts, 4, '[match]', '[/match]', ' … ', ?5) AS excerpt,
                    bm25(conversation_memory_fts, 0.0, 0.0, 0.0, 0.0, 1.0) AS score,
                    COALESCE(message.created_at, 0) AS created_at
             FROM conversation_memory_fts
             JOIN messages AS message ON message.rowid = conversation_memory_fts.rowid
             WHERE conversation_memory_fts MATCH ?1
               AND conversation_memory_fts.conversation_id = ?2
               AND (
                    ?3 IS NULL
                    OR (
                        EXISTS (
                            SELECT 1 FROM messages AS boundary
                            WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                        )
                        AND (
                            COALESCE(message.created_at, 0) < COALESCE((
                                SELECT boundary.created_at FROM messages AS boundary
                                WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                            ), 0)
                            OR (
                                COALESCE(message.created_at, 0) = COALESCE((
                                    SELECT boundary.created_at FROM messages AS boundary
                                    WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                                ), 0)
                                AND message.id <= ?3
                            )
                        )
                    )
               )
             UNION ALL
             SELECT conversation_memory_tools_fts.message_id, conversation_memory_tools_fts.role,
                    snippet(conversation_memory_tools_fts, 4, '[match]', '[/match]', ' … ', ?5) AS excerpt,
                    bm25(conversation_memory_tools_fts, 0.0, 0.0, 0.0, 0.0, 1.0) AS score,
                    COALESCE(message.created_at, 0) AS created_at
             FROM conversation_memory_tools_fts
             JOIN messages AS message ON message.rowid = conversation_memory_tools_fts.rowid
             WHERE conversation_memory_tools_fts MATCH ?1
               AND conversation_memory_tools_fts.conversation_id = ?2
               AND (
                    ?3 IS NULL
                    OR (
                        EXISTS (
                            SELECT 1 FROM messages AS boundary
                            WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                        )
                        AND (
                            COALESCE(message.created_at, 0) < COALESCE((
                                SELECT boundary.created_at FROM messages AS boundary
                                WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                            ), 0)
                            OR (
                                COALESCE(message.created_at, 0) = COALESCE((
                                    SELECT boundary.created_at FROM messages AS boundary
                                    WHERE boundary.conversation_id = ?2 AND boundary.id = ?3
                                ), 0)
                                AND message.id <= ?3
                            )
                        )
                    )
               )
         ), deduplicated AS (
             SELECT message_id, role, group_concat(excerpt, ' … ') AS content,
                    MIN(score) AS score, MAX(created_at) AS created_at
             FROM matches
             GROUP BY message_id, role
         )
         SELECT message_id, role, content, created_at FROM deduplicated
         ORDER BY score, created_at DESC, message_id DESC
         LIMIT ?4",
    )?;
    let rows = statement.query_map(
        params![
            search_expression,
            conversation_id,
            through_message_id,
            result_limit,
            snippet_words,
        ],
        |row| {
            Ok(ArchivedMemoryExcerpt {
                message_id: row.get(0)?,
                role: row.get(1)?,
                content: bounded_archive_excerpt(row.get(2)?),
                created_at_unix_ms: row.get(3)?,
            })
        },
    )?;
    rows.collect()
}

pub(super) fn search_chat_history_from_connection(
    connection: &Connection,
    search_expression: &str,
    filters: &HistorySearchFilters,
    result_limit: i64,
) -> rusqlite::Result<Vec<HistorySearchResult>> {
    index::ensure_all_archived_memory_indexes(connection)?;
    let mut statement = connection.prepare(
        "WITH matches AS (
             SELECT message.conversation_id, message.id AS message_id, message.role,
                    snippet(conversation_memory_fts, 4, '[match]', '[/match]', ' … ', 48) AS excerpt,
                    bm25(conversation_memory_fts, 0.0, 0.0, 0.0, 0.0, 1.0) AS score,
                    0 AS source,
                    COALESCE(message.created_at, 0) AS created_at
             FROM conversation_memory_fts
             JOIN messages AS message ON message.rowid = conversation_memory_fts.rowid
             WHERE conversation_memory_fts MATCH ?1
               AND COALESCE((
                    SELECT included FROM conversation_memory_archive_settings
                    WHERE conversation_id = message.conversation_id
               ), 1) = 1
             UNION ALL
             SELECT message.conversation_id, message.id AS message_id, message.role,
                    snippet(conversation_memory_tools_fts, 4, '[match]', '[/match]', ' … ', 48) AS excerpt,
                    bm25(conversation_memory_tools_fts, 0.0, 0.0, 0.0, 0.0, 1.0) AS score,
                    1 AS source,
                    COALESCE(message.created_at, 0) AS created_at
             FROM conversation_memory_tools_fts
             JOIN messages AS message ON message.rowid = conversation_memory_tools_fts.rowid
             WHERE conversation_memory_tools_fts MATCH ?1
               AND COALESCE((
                    SELECT included FROM conversation_memory_archive_settings
                    WHERE conversation_id = message.conversation_id
               ), 1) = 1
         ), ranked AS (
             SELECT conversation_id, message_id, role, excerpt, created_at,
                    row_number() OVER (
                        PARTITION BY source
                        ORDER BY score, created_at DESC, message_id DESC
                    ) AS source_rank
             FROM matches
         ), deduplicated AS (
             SELECT conversation_id, message_id, role,
                    group_concat(excerpt, ' … ') AS excerpt,
                    MAX(created_at) AS created_at,
                    SUM(1.0 / (60 + source_rank)) AS relevance
             FROM ranked
             GROUP BY conversation_id, message_id, role
         )
         SELECT deduplicated.conversation_id, conversation.title,
                deduplicated.message_id, deduplicated.role,
                deduplicated.excerpt, deduplicated.created_at
         FROM deduplicated
         JOIN conversations AS conversation
           ON conversation.id = deduplicated.conversation_id
         WHERE (?2 IS NULL OR deduplicated.created_at >= ?2)
           AND (?3 IS NULL OR deduplicated.created_at < ?3)
           AND (?4 IS NULL OR EXISTS (
                SELECT 1 FROM messages AS routed_message
                WHERE routed_message.conversation_id = deduplicated.conversation_id
                  AND routed_message.id = deduplicated.message_id
                  AND routed_message.provider_id = ?4
           ))
           AND (?5 IS NULL OR EXISTS (
                SELECT 1 FROM messages AS routed_message
                WHERE routed_message.conversation_id = deduplicated.conversation_id
                  AND routed_message.id = deduplicated.message_id
                  AND routed_message.model_id = ?5
           ))
           AND (?6 IS NULL OR conversation.project_id = ?6)
           AND (?7 IS NULL OR conversation.is_archived = ?7)
           AND (?8 IS NULL OR EXISTS (
                SELECT 1 FROM json_each(
                    CASE WHEN json_valid(conversation.tags)
                         THEN conversation.tags ELSE '[]' END
                ) AS conversation_tag
                WHERE typeof(conversation_tag.value) = 'text'
                  AND lower(conversation_tag.value) = lower(?8)
           ))
         ORDER BY deduplicated.relevance DESC,
                  deduplicated.created_at DESC,
                  deduplicated.message_id DESC
         LIMIT ?9",
    )?;
    let rows = statement.query_map(
        params![
            search_expression,
            filters.from_unix_ms,
            filters.through_unix_ms,
            filters.provider_id,
            filters.model_id,
            filters.project_id,
            filters.is_archived.map(i64::from),
            filters.tag,
            result_limit
        ],
        |row| {
            Ok(HistorySearchResult {
                conversation_id: row.get(0)?,
                conversation_title: row.get(1)?,
                message_id: row.get(2)?,
                role: row.get(3)?,
                excerpt: bounded_archive_excerpt(row.get(4)?),
                created_at_unix_ms: row.get(5)?,
            })
        },
    )?;
    rows.collect()
}

pub(super) fn bounded_archive_excerpt(content: String) -> String {
    let mut characters = content.chars();
    let mut excerpt = characters
        .by_ref()
        .take(MAX_ARCHIVE_EXCERPT_CHARS)
        .collect::<String>();
    if characters.next().is_none() {
        return excerpt;
    }

    for marker in ["[match]", "[/match]"] {
        for partial_length in (1..marker.len()).rev() {
            if excerpt.ends_with(&marker[..partial_length]) {
                let partial_start = excerpt.len() - partial_length;
                excerpt.truncate(partial_start);
                break;
            }
        }
    }
    if let Some(opening) = excerpt.rfind("[match]")
        && !excerpt[opening + "[match]".len()..].contains("[/match]")
    {
        excerpt.truncate(opening);
    }
    excerpt.push('…');
    excerpt
}

pub(super) fn retrieval_limits(context_window: Option<i64>) -> Option<(i64, i64)> {
    let context_window = context_window.filter(|window| *window >= MIN_MEMORY_CONTEXT_WINDOW)?;
    let word_budget =
        (context_window / MEMORY_WORDS_PER_CONTEXT_TOKEN).clamp(32, MAX_MEMORY_CONTEXT_WORDS);
    let result_limit = (word_budget / WORD_BUDGET_PER_RESULT).clamp(1, MAX_RESULTS);
    let snippet_words = (word_budget / result_limit).clamp(8, MAX_SNIPPET_WORDS);
    Some((result_limit, snippet_words))
}

pub(super) fn search_expression(conversation_id: &str, query: &str) -> Option<String> {
    let content_expression = content_search_expression(query)?;
    let mut scope_token = String::with_capacity(5 + conversation_id.len() * 2);
    scope_token.push_str("scope");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in conversation_id.as_bytes() {
        scope_token.push(HEX[(byte >> 4) as usize] as char);
        scope_token.push(HEX[(byte & 0x0f) as usize] as char);
    }

    Some(format!(
        "scope_token : {scope_token} AND {content_expression}"
    ))
}

pub(super) fn content_search_expression(query: &str) -> Option<String> {
    let mut terms = Vec::<String>::with_capacity(MAX_SEARCH_TERMS);
    let mut seen = HashSet::with_capacity(MAX_SEARCH_TERMS);
    for term in query.split(|character: char| !character.is_alphanumeric()) {
        let term_length = term.chars().count();
        if !(2..=MAX_SEARCH_TERM_CHARS).contains(&term_length) {
            continue;
        }
        let normalized = term.to_lowercase();
        if seen.contains(&normalized) {
            continue;
        }
        if terms.len() == MAX_SEARCH_TERMS {
            let shortest_index = terms
                .iter()
                .enumerate()
                .min_by_key(|(_, value)| value.chars().count())
                .map(|(index, _)| index)?;
            let shortest_length = terms[shortest_index].chars().count();
            if term_length <= shortest_length {
                continue;
            }
            seen.remove(&terms[shortest_index].to_lowercase());
            terms.swap_remove(shortest_index);
        }
        seen.insert(normalized);
        terms.push(term.to_owned());
    }
    terms.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
    if terms.is_empty() {
        return None;
    }

    let content_terms = terms
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    Some(format!("content : ({content_terms})"))
}
