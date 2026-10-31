//! Tests for semantic model downloads, chunking, and vector indexing.

use std::{fs, path::PathBuf};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc as tokio_mpsc, watch},
};

use super::{
    INDEX_KEY_ID_BITS, ModelFile, SemanticMemoryError, SemanticRuntime, StorageConnection,
    download_file_from_url, embedding_content, index_key, new_index, next_backfill_messages,
    parse_content_range, pending_messages, quantize_embedding, semantic_excerpt,
};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openchat-semantic-download-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).expect("create isolated download test directory");
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn model_file_for(payload: &[u8]) -> ModelFile {
    use sha2::{Digest, Sha256};

    let sha256 = format!("{:x}", Sha256::digest(payload));
    ModelFile {
        path: "model.bin",
        local_name: "model.bin",
        size: payload.len() as u64,
        sha256: Box::leak(sha256.into_boxed_str()),
    }
}

#[test]
fn releasing_idle_semantic_runtime_clears_cached_model_and_index_state() {
    let mut runtime = SemanticRuntime {
        model_directory: Some(PathBuf::from("loaded-model")),
        index_generation: Some(7),
        ..SemanticRuntime::default()
    };

    runtime.release_cached_resources();

    assert!(runtime.model.is_none());
    assert!(runtime.index.is_none());
    assert!(runtime.model_directory.is_none());
    assert!(runtime.index_generation.is_none());
}

async fn spawn_download_server(
    payload: Vec<u8>,
    ignore_range: bool,
    truncate_after: Option<usize>,
) -> (String, tokio::task::JoinHandle<Option<u64>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local model download test server");
    let address = listener
        .local_addr()
        .expect("read local model test server address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("accept local model download request");
        let mut request = Vec::new();
        loop {
            let mut buffer = [0_u8; 1024];
            let length = stream
                .read(&mut buffer)
                .await
                .expect("read local model download request");
            if length == 0 {
                return None;
            }
            request.extend_from_slice(&buffer[..length]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        let requested_start = request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("range")
                .then(|| {
                    value
                        .trim()
                        .strip_prefix("bytes=")?
                        .strip_suffix('-')?
                        .parse()
                        .ok()
                })
                .flatten()
        });
        let resume_from = if ignore_range {
            0
        } else {
            requested_start.unwrap_or(0) as usize
        };
        let is_partial = requested_start.is_some() && !ignore_range;
        let response_body = &payload[resume_from..];
        let body_to_send = truncate_after
            .map(|length| response_body.len().min(length))
            .unwrap_or(response_body.len());
        let status = if is_partial {
            "206 Partial Content"
        } else {
            "200 OK"
        };
        let mut headers = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
            response_body.len()
        );
        if is_partial {
            headers.push_str(&format!(
                "Content-Range: bytes {}-{}/{}\r\n",
                resume_from,
                payload.len() - 1,
                payload.len()
            ));
        }
        headers.push_str("\r\n");
        stream
            .write_all(headers.as_bytes())
            .await
            .expect("write local model download headers");
        stream
            .write_all(&response_body[..body_to_send])
            .await
            .expect("write local model download body");
        requested_start
    });
    (format!("http://{address}/model"), server)
}

async fn spawn_redirect_server(location: String) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local redirect fixture");
    let address = listener
        .local_addr()
        .expect("read local redirect fixture address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("accept local redirect request");
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write local redirect response");
    });
    (format!("http://{address}/model"), server)
}

async fn spawn_redirect_target() -> (String, tokio::task::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind redirect target fixture");
    let address = listener
        .local_addr()
        .expect("read redirect target fixture address");
    let server = tokio::spawn(async move {
        let accepted =
            tokio::time::timeout(std::time::Duration::from_secs(1), listener.accept()).await;
        let Ok(Ok((mut stream, _))) = accepted else {
            return false;
        };
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("write redirect target response");
        true
    });
    (format!("http://{address}/target"), server)
}

async fn download_for_test(
    url: &str,
    destination: &std::path::Path,
    asset: ModelFile,
    cancellation: &mut watch::Receiver<bool>,
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
) -> Result<(), SemanticMemoryError> {
    download_file_from_url(
        url,
        destination,
        asset,
        0,
        asset.size,
        progress,
        cancellation,
    )
    .await
}

#[tokio::test]
async fn interrupted_model_download_resumes_from_the_saved_partial_file() {
    let payload = vec![b'x'; 512 * 1024];
    let asset = model_file_for(&payload);
    let temporary_directory = TemporaryDirectory::new();
    let destination = temporary_directory.0.join(asset.local_name);
    let (progress_sender, mut progress_receiver) = tokio_mpsc::unbounded_channel();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);
    let (first_url, first_server) =
        spawn_download_server(payload.clone(), false, Some(128 * 1024)).await;

    let first_result = download_for_test(
        &first_url,
        &destination,
        asset,
        &mut cancellation,
        &progress_sender,
    )
    .await;
    assert!(matches!(first_result, Err(SemanticMemoryError::Http(_))));
    assert_eq!(first_server.await.expect("join interrupted download"), None);
    let partial_path = destination.with_extension("partial");
    let partial_size = fs::metadata(&partial_path)
        .expect("partial download remains after the connection ends")
        .len();
    assert_eq!(partial_size, 128 * 1024);

    let (resume_url, resume_server) = spawn_download_server(payload.clone(), false, None).await;
    download_for_test(
        &resume_url,
        &destination,
        asset,
        &mut cancellation,
        &progress_sender,
    )
    .await
    .expect("resume the model download");
    assert_eq!(
        resume_server.await.expect("join resumed download"),
        Some(partial_size)
    );
    assert_eq!(
        fs::read(&destination).expect("read verified model"),
        payload
    );
    let mut saw_complete_progress = false;
    while let Ok(data) = progress_receiver.try_recv() {
        if data["downloadedBytes"].as_u64() == Some(asset.size) {
            saw_complete_progress = true;
        }
    }
    assert!(saw_complete_progress);
}

#[tokio::test]
async fn semantic_model_download_does_not_follow_redirects_to_local_hosts() {
    let payload = b"verified model";
    let asset = model_file_for(payload);
    let temporary_directory = TemporaryDirectory::new();
    let destination = temporary_directory.0.join(asset.local_name);
    let (progress_sender, _progress_receiver) = tokio_mpsc::unbounded_channel();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);
    let (target_url, target_server) = spawn_redirect_target().await;
    let (source_url, source_server) = spawn_redirect_server(target_url).await;

    let result = download_for_test(
        &source_url,
        &destination,
        asset,
        &mut cancellation,
        &progress_sender,
    )
    .await;

    assert!(matches!(result, Err(SemanticMemoryError::Http(_))));
    source_server.await.expect("join redirect source");
    assert!(
        !target_server.await.expect("join redirect target"),
        "a redirect to a local host must not receive the model request"
    );
    assert!(!destination.with_extension("partial").exists());
}

#[tokio::test]
async fn model_download_restarts_safely_when_host_ignores_range() {
    let payload = vec![b'y'; 256 * 1024];
    let asset = model_file_for(&payload);
    let temporary_directory = TemporaryDirectory::new();
    let destination = temporary_directory.0.join(asset.local_name);
    let partial_path = destination.with_extension("partial");
    fs::write(&partial_path, &payload[..64 * 1024]).expect("create partial model file");
    let (url, server) = spawn_download_server(payload.clone(), true, None).await;
    let (progress_sender, _progress_receiver) = tokio_mpsc::unbounded_channel();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);

    download_for_test(
        &url,
        &destination,
        asset,
        &mut cancellation,
        &progress_sender,
    )
    .await
    .expect("restart from the beginning after a full response");

    assert_eq!(
        server.await.expect("join ignored-range download"),
        Some(64 * 1024)
    );
    assert_eq!(
        fs::read(destination).expect("read downloaded model"),
        payload
    );
}

#[test]
fn content_range_must_match_the_requested_offset_and_pinned_file_size() {
    assert_eq!(parse_content_range("bytes 20-99/100", 20, 100), Some(80));
    assert_eq!(parse_content_range("bytes 19-99/100", 20, 100), None);
    assert_eq!(parse_content_range("bytes 20-99/101", 20, 100), None);
    assert_eq!(parse_content_range("bytes 20-100/100", 20, 100), None);
    assert_eq!(parse_content_range("items 20-99/100", 20, 100), None);
}

#[test]
fn semantic_source_keeps_message_text_and_archived_tool_details_together() {
    assert_eq!(
        embedding_content("assistant summary", "read config.json feature flags"),
        "assistant summary\n\nread config.json feature flags"
    );
    assert_eq!(
        embedding_content("assistant summary", ""),
        "assistant summary"
    );
    assert_eq!(embedding_content("", "tool result"), "tool result");
    assert_eq!(embedding_content("", ""), "");
}

#[test]
fn semantic_pending_and_backfill_queries_skip_excluded_conversations() {
    let connection =
        rusqlite::Connection::open_in_memory().expect("open semantic archive preference fixture");
    connection
        .execute_batch(
            "CREATE TABLE messages (
                conversation_id TEXT NOT NULL,
                id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER,
                PRIMARY KEY (conversation_id, id)
            );
            CREATE TABLE conversation_memory_embedding_pending (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id)
            );
            CREATE TABLE conversation_memory_archive_settings (
                conversation_id TEXT PRIMARY KEY NOT NULL,
                included INTEGER NOT NULL
            );
            CREATE TABLE conversation_memory_embedding_state (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id, model_id)
            );
            CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                scope_token, content
            );
            INSERT INTO messages VALUES
                ('excluded', 'pending', 'user', 'should not embed', 'completed', 1),
                ('included', 'pending', 'user', 'can embed', 'completed', 1);
            INSERT INTO conversation_memory_archive_settings VALUES ('excluded', 0);
            INSERT INTO conversation_memory_embedding_pending VALUES ('excluded', 'pending');
            INSERT INTO conversation_memory_embedding_pending VALUES ('included', 'pending');",
        )
        .expect("create pending and backfill archive fixture");
    let connection = StorageConnection::standalone(connection);

    assert!(
        pending_messages(&connection, "excluded", None)
            .expect("read excluded pending messages")
            .is_empty()
    );
    assert_eq!(
        pending_messages(&connection, "included", None)
            .expect("read included pending messages")
            .len(),
        1
    );
    assert!(
        next_backfill_messages(&connection, "excluded", None, None, None)
            .expect("read excluded backfill messages")
            .is_empty()
    );
    assert_eq!(
        next_backfill_messages(&connection, "included", None, None, None)
            .expect("read included backfill messages")
            .len(),
        1
    );
}

#[test]
fn semantic_archive_rebuilds_tool_excerpts_from_the_filtered_tool_index() {
    let connection =
        rusqlite::Connection::open_in_memory().expect("open in-memory semantic archive");
    connection
        .execute_batch(
            "CREATE TABLE messages (
                conversation_id TEXT NOT NULL,
                id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER,
                PRIMARY KEY (conversation_id, id)
            );
            CREATE TABLE conversation_memory_embedding_pending (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                PRIMARY KEY (conversation_id, message_id)
            );
            CREATE TABLE conversation_memory_archive_settings (
                conversation_id TEXT PRIMARY KEY NOT NULL,
                included INTEGER NOT NULL
            );
            CREATE TABLE conversation_memory_embeddings (
                id INTEGER PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                start_byte INTEGER NOT NULL,
                end_byte INTEGER NOT NULL
            );
            CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                conversation_id UNINDEXED,
                message_id UNINDEXED,
                role UNINDEXED,
                scope_token,
                content
            );
            INSERT INTO messages
                (conversation_id, id, role, content, status, created_at)
            VALUES ('conversation', 'message', 'assistant', 'I checked the settings.', 'completed', 1);
            INSERT INTO conversation_memory_embedding_pending
                (conversation_id, message_id) VALUES ('conversation', 'message');",
        )
        .expect("create semantic archive fixture");
    let message_rowid = connection
        .query_row(
            "SELECT rowid FROM messages WHERE conversation_id = 'conversation' AND id = 'message'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("read semantic archive source rowid");
    let tool_content = "read settings.json semantic memory enabled";
    connection
        .execute(
            "INSERT INTO conversation_memory_tools_fts (
                rowid, conversation_id, message_id, role, scope_token, content
             ) VALUES (?1, 'conversation', 'message', 'assistant', 'scope', ?2)",
            rusqlite::params![message_rowid, tool_content],
        )
        .expect("insert filtered tool archive content");
    let connection = StorageConnection::standalone(connection);

    let pending = pending_messages(&connection, "conversation", None)
        .expect("read pending semantic archive message");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool_content, tool_content);

    let source = embedding_content(&pending[0].content, &pending[0].tool_content);
    let start_byte = source
        .find(tool_content)
        .expect("find tool archive excerpt") as i64;
    let end_byte = start_byte + tool_content.len() as i64;
    connection
        .execute(
            "INSERT INTO conversation_memory_embeddings (
                conversation_id, message_id, start_byte, end_byte
             ) VALUES ('conversation', 'message', ?1, ?2)",
            rusqlite::params![start_byte, end_byte],
        )
        .expect("insert tool excerpt offsets");
    let last_insert_rowid = connection
        .query_row("SELECT last_insert_rowid()", [], |row| row.get::<_, i64>(0))
        .expect("read last inserted embedding row");

    let excerpt = semantic_excerpt(&connection, "conversation", last_insert_rowid, None)
        .expect("read semantic tool excerpt")
        .expect("semantic tool excerpt exists");
    assert_eq!(excerpt.content, tool_content);
}

#[test]
fn semantic_index_filters_hits_by_conversation_namespace() {
    let mut index = new_index().expect("create semantic test index");
    index.reserve(2).expect("reserve semantic test vectors");
    let mut conversation_vector = vec![0_i8; super::VECTOR_DIMENSIONS];
    conversation_vector[0] = 100;
    let mut other_vector = vec![0_i8; super::VECTOR_DIMENSIONS];
    other_vector[1] = 100;
    let conversation_key = index_key(3, 1).expect("encode current conversation key");
    index
        .add(conversation_key, &conversation_vector)
        .expect("add current conversation vector");
    index
        .add(
            index_key(4, 2).expect("encode other conversation key"),
            &other_vector,
        )
        .expect("add other conversation vector");

    let matches = index
        .filtered_search(&conversation_vector, 2, |key| key >> INDEX_KEY_ID_BITS == 3)
        .expect("search within current conversation namespace");

    assert_eq!(matches.keys, [conversation_key]);
}

#[test]
fn quantization_scales_each_embedding_and_rejects_invalid_vectors() {
    let mut embedding = vec![0.0; super::VECTOR_DIMENSIONS];
    embedding[0] = 0.25;
    embedding[1] = -0.125;

    let quantized = quantize_embedding(embedding).expect("quantize finite embedding");

    assert_eq!(quantized[0], i8::MAX);
    assert_eq!(quantized[1], -64);
    assert!(matches!(
        quantize_embedding(vec![0.0; super::VECTOR_DIMENSIONS]),
        Err(SemanticMemoryError::Model(_))
    ));
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut invalid_embedding = vec![0.0; super::VECTOR_DIMENSIONS];
        invalid_embedding[super::VECTOR_DIMENSIONS - 1] = invalid;
        assert!(matches!(
            quantize_embedding(invalid_embedding),
            Err(SemanticMemoryError::Model(_))
        ));
    }
}

#[test]
fn embedding_ids_are_packed_with_their_conversation_namespace() {
    let first = index_key(1, 9).expect("encode first key");
    let second = index_key(2, 9).expect("encode second key");

    assert_ne!(first, second);
    assert_eq!(first >> INDEX_KEY_ID_BITS, 1);
    assert!(index_key(16_777_216, 9).is_err());
    assert!(index_key(1, 0).is_err());
}

#[test]
fn prunes_obsolete_semantic_index_files_after_rebuild() {
    let temporary_directory = TemporaryDirectory::new();
    let index_directory = temporary_directory.0.join("semantic-memory");
    fs::create_dir_all(&index_directory).expect("create semantic index directory");
    let old_index = index_directory.join("index-3.usearch");
    let current_index = index_directory.join("index-4.usearch");
    let interrupted_index = index_directory.join("index-5.usearch.partial");
    let unrelated_file = index_directory.join("model.bin");
    for path in [
        &old_index,
        &current_index,
        &interrupted_index,
        &unrelated_file,
    ] {
        fs::write(path, b"cache").expect("create semantic cache fixture");
    }

    super::prune_cached_indexes(&index_directory, Some(&current_index))
        .expect("prune obsolete indexes");

    assert!(!old_index.exists());
    assert!(current_index.exists());
    assert!(!interrupted_index.exists());
    assert!(unrelated_file.exists());
}

#[test]
fn vector_index_filtered_search_orders_by_similarity_and_respects_count() {
    let mut index = super::VectorIndex::new(super::VECTOR_DIMENSIONS);
    let mut v1 = vec![0_i8; super::VECTOR_DIMENSIONS];
    v1[0] = 100;
    let mut v2 = vec![0_i8; super::VECTOR_DIMENSIONS];
    v2[0] = 50;
    v2[1] = 50;
    let mut v3 = vec![0_i8; super::VECTOR_DIMENSIONS];
    v3[1] = 100;

    index.add(1, &v1).expect("add v1");
    index.add(2, &v2).expect("add v2");
    index.add(3, &v3).expect("add v3");

    let query = v1.clone();
    let matches = index
        .filtered_search(&query, 2, |_| true)
        .expect("search all vectors");
    assert_eq!(matches.keys.len(), 2);
    assert_eq!(matches.keys[0], 1);
    assert_eq!(matches.keys[1], 2);
    assert!(matches.distances[0] <= matches.distances[1]);
}

#[test]
fn vector_index_persists_and_restores_cleanly() {
    let temp_dir = TemporaryDirectory::new();
    let file_path = temp_dir.0.join("test_index.bin");
    let path_str = file_path.to_string_lossy().into_owned();

    let mut index = super::VectorIndex::new(super::VECTOR_DIMENSIONS);
    let mut vec1 = vec![0_i8; super::VECTOR_DIMENSIONS];
    vec1[0] = 42;
    let mut vec2 = vec![0_i8; super::VECTOR_DIMENSIONS];
    vec2[1] = -84;

    index.add(101, &vec1).expect("add key 101");
    index.add(202, &vec2).expect("add key 202");
    index.save(&path_str).expect("save vector index");

    let restored = super::VectorIndex::restore(&path_str).expect("restore vector index");
    assert_eq!(restored.size(), 2);

    let search = restored
        .filtered_search(&vec1, 1, |_| true)
        .expect("search restored");
    assert_eq!(search.keys, [101]);
}

#[test]
fn vector_index_restore_rejects_invalid_magic() {
    let temp_dir = TemporaryDirectory::new();
    let file_path = temp_dir.0.join("corrupted.bin");
    fs::write(&file_path, b"CORRUPTED_FILE_DATA_HERE").expect("write bad file");

    let result = super::VectorIndex::restore(&file_path.to_string_lossy());
    assert!(result.is_err());
}
