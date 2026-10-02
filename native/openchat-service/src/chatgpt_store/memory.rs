mod index;
mod search;
mod semantic;

use crate::storage::AppStorage;

#[derive(Clone, Debug)]
pub struct ArchivedMemoryExcerpt {
    pub message_id: String,
    pub role: String,
    pub content: String,
    pub created_at_unix_ms: i64,
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
    let database_path = storage.database_path().to_owned();
    let conversation_for_search = conversation_id.to_owned();
    let boundary_for_search = through_message_id.to_owned();
    let query_for_search = query.to_owned();
    let lexical_results = tokio::task::spawn_blocking(move || {
        let search_expression =
            search::search_expression(&conversation_for_search, &query_for_search);
        let Some(search_expression) = search_expression else {
            return Ok(Vec::new());
        };
        let connection = rusqlite::Connection::open(database_path)?;
        search::retrieve_archived_memories_from_connection(
            &connection,
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
    let database_path = storage.database_path().to_owned();
    let conversation_for_search = conversation_id.to_owned();
    let query_for_search = query.to_owned();
    let lexical_results = tokio::task::spawn_blocking(move || {
        let Some(search_expression) =
            search::search_expression(&conversation_for_search, &query_for_search)
        else {
            return Ok(Vec::new());
        };
        let connection = rusqlite::Connection::open(database_path)?;
        search::search_archived_memories_from_connection(
            &connection,
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
        storage.database_path().to_owned(),
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

pub async fn semantic_search_is_ready(storage: &AppStorage) -> bool {
    semantic::is_ready(&storage.semantic_memory_cache_directory()).await
}

pub async fn prepare_semantic_search(
    storage: &AppStorage,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
    progress: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
) -> rusqlite::Result<()> {
    semantic::prepare(
        storage.database_path().to_owned(),
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
        storage.database_path().to_owned(),
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
