//! Provides local archived-memory and conversation-history retrieval.
//!
//! Keyword, preference-aware, and semantic retrieval are split into focused
//! submodules and return bounded excerpts for provider context.

mod index;
mod preferences;
mod search;
mod semantic;
pub mod turkish_search;
pub mod vector_math;

use crate::storage::AppStorage;

#[derive(Clone, Debug)]
pub struct ArchivedMemoryExcerpt {
    pub message_id: String,
    pub role: String,
    pub content: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct HistorySearchResult {
    pub conversation_id: String,
    pub conversation_title: String,
    pub is_conversation_title_match: bool,
    pub message_id: Option<String>,
    pub role: Option<String>,
    pub excerpt: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectHistoryExcerpt {
    pub conversation_id: String,
    pub conversation_title: String,
    pub message_id: String,
    pub role: String,
    pub excerpt: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct HistorySearchFilters {
    pub from_unix_ms: Option<i64>,
    pub through_unix_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub project_id: Option<String>,
    pub excluded_conversation_id: Option<String>,
    pub is_archived: Option<bool>,
    pub result_type: String,
    pub sort_order: String,
}

#[derive(Clone, Debug)]
pub struct HistorySearchFilterOptions {
    pub provider_ids: Vec<String>,
    pub model_ids: Vec<String>,
}

pub use preferences::{ArchiveIndexSettings, ArchiveIndexTool};

pub async fn set_conversation_archive_included(
    storage: &AppStorage,
    conversation_id: &str,
    included: bool,
) -> rusqlite::Result<ArchiveIndexSettings> {
    let (settings, changed) = {
        let mut connection = storage.connect()?;
        preferences::set_conversation_included(&mut connection, conversation_id, included)?
    };
    if changed {
        semantic::invalidate_cached_index(
            storage.database_queue_handle(),
            storage.semantic_memory_cache_directory(),
        )
        .await
        .map_err(semantic_database_error)?;
    }
    Ok(settings)
}

pub async fn set_archive_tool_included(
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    included: bool,
) -> rusqlite::Result<ArchiveIndexSettings> {
    let (settings, changed) = {
        let mut connection = storage.connect()?;
        preferences::set_tool_included(&mut connection, conversation_id, tool_name, included)?
    };
    if changed {
        semantic::invalidate_cached_index(
            storage.database_queue_handle(),
            storage.semantic_memory_cache_directory(),
        )
        .await
        .map_err(semantic_database_error)?;
    }
    Ok(settings)
}

pub async fn retrieve_archived_memories(
    storage: &AppStorage,
    conversation_id: &str,
    through_message_id: &str,
    query: &str,
    context_window: Option<i64>,
) -> rusqlite::Result<Vec<ArchivedMemoryExcerpt>> {
    if search::search_expression(conversation_id, query).is_none() {
        return Ok(Vec::new());
    }
    let Some((result_limit, snippet_words)) = search::retrieval_limits(context_window) else {
        return Ok(Vec::new());
    };
    let database = storage.database_queue_handle();
    let conversation_for_search = conversation_id.to_owned();
    let boundary_for_search = through_message_id.to_owned();
    let query_for_search = query.to_owned();
    let lexical_results = tokio::task::spawn_blocking(move || {
        let search_expression =
            search::search_expression(&conversation_for_search, &query_for_search);
        let Some(search_expression) = search_expression else {
            return Ok(Vec::new());
        };
        let mut connection = database.connect()?;
        search::retrieve_archived_memories_from_connection(
            &mut connection,
            &conversation_for_search,
            &boundary_for_search,
            &search_expression,
            result_limit,
            snippet_words,
        )
    })
    .await
    .map_err(worker_join_error)??;
    let Some(semantic_results) = semantic_results_for_compaction(
        storage,
        conversation_id,
        through_message_id,
        query,
        result_limit as usize,
    )
    .await?
    else {
        return Ok(lexical_results);
    };
    Ok(search::combine_archive_results(
        lexical_results,
        semantic_results,
        result_limit as usize,
    ))
}

pub async fn search_conversation_archive(
    storage: &AppStorage,
    conversation_id: &str,
    query: &str,
) -> rusqlite::Result<Vec<ArchivedMemoryExcerpt>> {
    if query.chars().count() > search::MAX_ARCHIVE_SEARCH_QUERY_CHARS {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if search::search_expression(conversation_id, query).is_none() {
        return Ok(Vec::new());
    }
    let cache_directory = storage.semantic_memory_cache_directory();
    let semantic_ready = semantic::is_ready(&cache_directory).await;
    let database = storage.database_queue_handle();
    let conversation_for_search = conversation_id.to_owned();
    let query_for_search = query.to_owned();
    let lexical_results = tokio::task::spawn_blocking(move || {
        let Some(search_expression) =
            search::search_expression(&conversation_for_search, &query_for_search)
        else {
            return Ok(Vec::new());
        };
        let mut connection = database.connect()?;
        search::search_archived_memories_from_connection(
            &mut connection,
            &conversation_for_search,
            None,
            &search_expression,
            search::MAX_ARCHIVE_SEARCH_RESULTS,
            search::ARCHIVE_SEARCH_SNIPPET_WORDS,
        )
    })
    .await
    .map_err(worker_join_error)??;
    if !semantic_ready {
        return Ok(lexical_results);
    }
    let semantic_results = semantic::search_archive(
        storage.database_queue_handle(),
        cache_directory,
        conversation_id.to_owned(),
        None,
        query.to_owned(),
        search::MAX_ARCHIVE_SEARCH_RESULTS as usize,
    )
    .await
    .map_err(semantic_database_error)?;
    Ok(search::combine_archive_results(
        lexical_results,
        semantic_results,
        search::MAX_ARCHIVE_SEARCH_RESULTS as usize,
    ))
}

pub async fn search_chat_history(
    storage: &AppStorage,
    query: &str,
    filters: HistorySearchFilters,
) -> rusqlite::Result<Vec<HistorySearchResult>> {
    if query.chars().count() > search::MAX_ARCHIVE_SEARCH_QUERY_CHARS {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let search_expression = search::content_search_expression(query);
    if search_expression.is_none() && filters.result_type != "conversations" {
        return Ok(Vec::new());
    }
    let database = storage.database_queue_handle();
    let query = query.to_owned();
    tokio::task::spawn_blocking(move || {
        let connection = database.connect()?;
        search::search_chat_history_from_connection(
            &connection,
            search_expression.as_deref(),
            &query,
            &filters,
            search::MAX_HISTORY_SEARCH_RESULTS,
        )
    })
    .await
    .map_err(worker_join_error)?
}

/// Retrieves bounded history from other conversations in one project using keyword and existing
/// local semantic indexes. Semantic retrieval never triggers history backfill from a chat send.
pub async fn search_project_history(
    storage: &AppStorage,
    project_id: &str,
    excluded_conversation_id: &str,
    query: &str,
) -> rusqlite::Result<Vec<ProjectHistoryExcerpt>> {
    let query = query
        .chars()
        .take(search::MAX_ARCHIVE_SEARCH_QUERY_CHARS)
        .collect::<String>();
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let lexical_results = search_chat_history(
        storage,
        &query,
        HistorySearchFilters {
            from_unix_ms: None,
            through_unix_ms: None,
            provider_id: None,
            model_id: None,
            project_id: Some(project_id.to_owned()),
            excluded_conversation_id: Some(excluded_conversation_id.to_owned()),
            is_archived: None,
            result_type: "messages".to_owned(),
            sort_order: "relevance".to_owned(),
        },
    )
    .await?;

    let cache_directory = storage.semantic_memory_cache_directory();
    let semantic_ready = semantic::is_ready(&cache_directory).await;
    let project_conversations = if semantic_ready {
        let database = storage.database_queue_handle();
        let project_id = project_id.to_owned();
        let excluded_conversation_id = excluded_conversation_id.to_owned();
        tokio::task::spawn_blocking(move || {
            let connection = database.connect()?;
            let mut statement = connection.prepare(
                "SELECT conversation.id, conversation.title
                 FROM conversations AS conversation
                 WHERE conversation.project_id = ?1 AND conversation.id != ?2
                   AND COALESCE((
                        SELECT included FROM conversation_memory_archive_settings
                        WHERE conversation_id = conversation.id
                   ), 1) = 1
                 ORDER BY COALESCE(conversation.updated_at, conversation.created_at, 0) DESC,
                          conversation.id
                 LIMIT 8",
            )?;
            statement
                .query_map([project_id, excluded_conversation_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .await
        .map_err(worker_join_error)??
    } else {
        Vec::new()
    };

    let mut candidates = Vec::<(ProjectHistoryExcerpt, u32)>::new();
    let mut candidate_positions = std::collections::HashMap::<(String, String), usize>::new();
    for (rank, result) in lexical_results.into_iter().enumerate() {
        let (Some(message_id), Some(role)) = (result.message_id, result.role) else {
            continue;
        };
        let excerpt = ProjectHistoryExcerpt {
            conversation_id: result.conversation_id,
            conversation_title: result.conversation_title,
            message_id,
            role,
            excerpt: result.excerpt,
            created_at_unix_ms: result.created_at_unix_ms,
        };
        merge_project_history_candidate(&mut candidates, &mut candidate_positions, excerpt, rank);
    }

    for (conversation_id, conversation_title) in project_conversations {
        // Semantic history is an optional ranking source; keyword results remain usable.
        if let Ok(results) = semantic::search_archive_existing(
            storage.database_queue_handle(),
            cache_directory.clone(),
            conversation_id.clone(),
            query.clone(),
            6,
        )
        .await
        {
            for (rank, result) in results.into_iter().enumerate() {
                let excerpt = ProjectHistoryExcerpt {
                    conversation_id: conversation_id.clone(),
                    conversation_title: conversation_title.clone(),
                    message_id: result.message_id,
                    role: result.role,
                    excerpt: result.content,
                    created_at_unix_ms: result.created_at_unix_ms,
                };
                merge_project_history_candidate(
                    &mut candidates,
                    &mut candidate_positions,
                    excerpt,
                    rank,
                );
            }
        }
    }

    candidates.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.created_at_unix_ms.cmp(&left.created_at_unix_ms))
    });
    Ok(candidates
        .into_iter()
        .take(6)
        .map(|(excerpt, _)| excerpt)
        .collect())
}

fn merge_project_history_candidate(
    candidates: &mut Vec<(ProjectHistoryExcerpt, u32)>,
    positions: &mut std::collections::HashMap<(String, String), usize>,
    excerpt: ProjectHistoryExcerpt,
    rank: usize,
) {
    let key = (excerpt.conversation_id.clone(), excerpt.message_id.clone());
    let score = 10_000_u32 / (60 + u32::try_from(rank).unwrap_or(u32::MAX));
    if let Some(position) = positions.get(&key).copied() {
        candidates[position].1 = candidates[position].1.saturating_add(score);
    } else {
        positions.insert(key, candidates.len());
        candidates.push((excerpt, score));
    }
}

pub fn history_search_filter_options(
    storage: &AppStorage,
) -> rusqlite::Result<HistorySearchFilterOptions> {
    let connection = storage.connect()?;
    let provider_ids = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT provider_id FROM messages
             WHERE provider_id IS NOT NULL AND trim(provider_id) != ''
             ORDER BY provider_id LIMIT 100",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let model_ids = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT model_id FROM messages
             WHERE model_id IS NOT NULL AND trim(model_id) != ''
             ORDER BY model_id LIMIT 200",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(HistorySearchFilterOptions {
        provider_ids,
        model_ids,
    })
}

pub async fn semantic_search_is_ready(storage: &AppStorage) -> bool {
    semantic::is_ready(&storage.semantic_memory_cache_directory()).await
}

pub async fn prepare_semantic_search(
    storage: &AppStorage,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
    progress: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
) -> rusqlite::Result<()> {
    semantic::prepare(
        storage.database_queue_handle(),
        storage.semantic_memory_cache_directory(),
        cancellation,
        progress,
    )
    .await
    .map_err(semantic_database_error)
}

async fn semantic_results_for_compaction(
    storage: &AppStorage,
    conversation_id: &str,
    through_message_id: &str,
    query: &str,
    result_limit: usize,
) -> rusqlite::Result<Option<Vec<ArchivedMemoryExcerpt>>> {
    let cache_directory = storage.semantic_memory_cache_directory();
    if !semantic::is_ready(&cache_directory).await {
        return Ok(None);
    }
    semantic::search_archive(
        storage.database_queue_handle(),
        cache_directory,
        conversation_id.to_owned(),
        Some(through_message_id.to_owned()),
        query.to_owned(),
        result_limit,
    )
    .await
    .map(Some)
    .map_err(semantic_database_error)
}

fn semantic_database_error(error: semantic::SemanticMemoryError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

fn worker_join_error(error: tokio::task::JoinError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error.to_string())))
}

pub(super) fn unix_time_millis() -> rusqlite::Result<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

#[cfg(test)]
mod tests;
