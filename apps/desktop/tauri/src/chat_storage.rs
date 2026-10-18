//! Tauri commands for the local conversation, project, message, and saved-output database.
//!
//! Request values are bounded and validated before SQL or filesystem operations. Attachments
//! are stored as validated database content, while the database connection enforces the
//! minimum SQLite version required by the current schema.

use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{
    de::{self, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::json;

use crate::{
    file_ops::replace_file_atomically,
    project_folder::consume_project_folder_selection,
    request_bounds::{BoundedText, BoundedVec},
    save_dialog::{select_save_file, SaveFileDialogError},
};

mod maintenance;
mod migrations;
mod settings;

pub use maintenance::*;
pub use migrations::*;
pub use settings::*;
const MINIMUM_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
const MAX_MESSAGE_CHARS: usize = 1_000_000;
const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAXIMUM_ATTACHMENT_FILE_COUNT: usize = 10;
const MAXIMUM_ATTACHMENT_IMAGE_COUNT: usize = 3;
const MAXIMUM_ATTACHMENT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_TEXT_BYTES: usize = 1024 * 1024;
const MAXIMUM_ATTACHMENT_TOTAL_BYTES: usize = 14 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES: usize =
    (MAXIMUM_ATTACHMENT_TOTAL_BYTES.div_ceil(3) + MAXIMUM_ATTACHMENT_FILE_COUNT) * 4;
const MAXIMUM_ATTACHMENT_NAME_BYTES: usize = 255 * 4;
const MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES: usize = 64;
const MAXIMUM_ATTACHMENT_KIND_BYTES: usize = 5;
const MAXIMUM_CONVERSATION_IDS_PER_MUTATION: usize = 500;
const MAXIMUM_CONVERSATION_TAGS: usize = 12;
const MAXIMUM_CONVERSATION_TAG_CHARS: usize = 32;
const MAXIMUM_CONVERSATION_TAG_BYTES: usize = MAXIMUM_CONVERSATION_TAG_CHARS * 4;
const MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS: usize = 500;
const MAXIMUM_CONVERSATION_TITLE_CHARS: usize = 512;
const MAXIMUM_SAFE_IDENTIFIER_BYTES: usize = 128;
const MAXIMUM_MESSAGE_CONTENT_BYTES: usize = MAX_MESSAGE_CHARS * 4;
const MAXIMUM_NEW_CONVERSATION_TITLE_BYTES: usize = 500 * 4;
const MAXIMUM_CONVERSATION_TITLE_BYTES: usize = MAXIMUM_CONVERSATION_TITLE_CHARS * 4;
const MAXIMUM_PROJECT_NAME_BYTES: usize = MAXIMUM_CONVERSATION_TITLE_BYTES;
const MAXIMUM_ROUTE_MODEL_ID_BYTES: usize = 256;
const MAXIMUM_MODEL_DISPLAY_NAME_BYTES: usize = 256 * 4;
const MAXIMUM_MESSAGE_METADATA_BYTES: usize = MAXIMUM_MESSAGE_CONTENT_BYTES;
const MAXIMUM_MESSAGE_STATUS_BYTES: usize = 16;
const MAXIMUM_MESSAGE_FAILURE_CODE_BYTES: usize = MAXIMUM_SAFE_IDENTIFIER_BYTES;
const MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES: usize = 255;
const MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES: usize = 128;
const MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES: usize = 20 * 1024 * 1024;
const MAXIMUM_HISTORY_SEARCH_QUERY_BYTES: usize = 512 * 4;
const MAXIMUM_HISTORY_SEARCH_QUERY_CHARS: usize = 512;
const MAXIMUM_HISTORY_SEARCH_RESULTS: usize = 30;
const MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES: usize = 4096;
const MAXIMUM_HISTORY_SEARCH_TIMESTAMP: i64 = 8_640_000_000_000_000;
const MAXIMUM_SETTING_KEY_BYTES: usize = 128;
const MAXIMUM_SETTING_VALUE_BYTES: usize = 2 * 1024 * 1024;
const MAXIMUM_FACT_ID_BYTES: usize = 128;
const MAXIMUM_FACT_TEXT_BYTES: usize = 64 * 1024;
const MAXIMUM_FACT_CATEGORY_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Persisted personal memory fact representation.
pub struct PersonalMemoryFact {
    /// Unique fact identifier.
    pub id: String,
    /// Extracted fact content.
    pub fact_text: String,
    /// Optional categorization.
    pub category: Option<String>,
    /// Source conversation where the fact was derived.
    pub source_conversation_id: Option<String>,
    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,
    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Payload for migrating localStorage data to the unified database.
pub struct LocalStorageMigrationPayload {
    /// Settings key-value pairs (value is JSON string).
    pub settings: std::collections::HashMap<String, String>,
    /// Personal memory facts.
    pub facts: Vec<PersonalMemoryFact>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Safe error contract for local conversation persistence operations.
pub struct ChatStorageError {
    code: &'static str,
    message: &'static str,
}

impl ChatStorageError {
    fn unavailable() -> Self {
        Self {
            code: "storage_unavailable",
            message: "Local conversation storage is unavailable.",
        }
    }

    fn invalid_input() -> Self {
        Self {
            code: "invalid_request_params",
            message: "The conversation data is invalid.",
        }
    }

    fn unsupported_sqlite() -> Self {
        Self {
            code: "sqlite_version_unsupported",
            message: "This SQLite version is too old to safely open local conversation storage.",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Conversation list item with routing, project, and organization metadata.
pub struct StoredConversation {
    id: String,
    title: String,
    title_source: String,
    provider_id: Option<String>,
    model_id: Option<String>,
    connection_id: Option<String>,
    workspace_id: Option<String>,
    api_key_connection_id: Option<String>,
    project_id: Option<String>,
    tags: Vec<String>,
    is_pinned: bool,
    is_archived: bool,
    is_bookmarked: bool,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Project metadata returned to the desktop frontend.
pub struct StoredProject {
    id: String,
    name: String,
    folder_path: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Persisted message content and normalized response metadata.
pub struct StoredMessage {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    provider_id: Option<String>,
    model_id: Option<String>,
    citation_sources: String,
    personal_memory_fact_ids: String,
    reasoning_summaries: String,
    tool_activities: String,
    status: String,
    failure_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Model favorite entry with its provider and optional connection scope.
pub struct StoredModelFavorite {
    provider_id: String,
    model_id: String,
    display_name: String,
    source_connection_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Stable message identifiers for outputs saved outside the conversation transcript.
pub struct SavedOutputMessageKey {
    conversation_id: String,
    message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Bounded filters for searching saved assistant responses in local history.
pub struct SavedOutputSearchInput {
    #[serde(deserialize_with = "deserialize_saved_output_search_query")]
    query: String,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    provider_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_model_id")]
    model_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    project_id: Option<String>,
    #[serde(default)]
    is_archived: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_history_search_tag")]
    tag: Option<String>,
    sort_order: SavedOutputSearchSortOrder,
    #[serde(default)]
    from_unix_ms: Option<i64>,
    #[serde(default)]
    through_unix_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Stable ordering options supported by the saved-output history search.
pub enum SavedOutputSearchSortOrder {
    Relevance,
    Newest,
    Oldest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A saved response matching the local history search filters.
pub struct SavedOutputSearchResult {
    conversation_id: String,
    conversation_title: String,
    is_conversation_title_match: bool,
    message_id: String,
    role: String,
    excerpt: String,
    created_at_unix_ms: i64,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated routing and project metadata used when starting a conversation.
pub struct CreateConversationInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    id: String,
    #[serde(deserialize_with = "deserialize_new_conversation_title")]
    title: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    model_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    workspace_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    api_key_connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// New model route assigned to an existing conversation.
pub struct SetConversationRouteInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    conversation_id: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    model_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    workspace_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    api_key_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Inputs for creating a project from a native folder-picker selection.
pub struct CreateProjectInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    id: String,
    #[serde(deserialize_with = "deserialize_project_name")]
    name: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    folder_selection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// User message and response metadata persisted with a conversation.
pub struct SaveMessageInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    id: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    conversation_id: String,
    #[serde(deserialize_with = "deserialize_message_discriminator")]
    role: String,
    #[serde(deserialize_with = "deserialize_message_content")]
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    provider_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_model_id")]
    model_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    citation_sources: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    personal_memory_fact_ids: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    reasoning_summaries: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    tool_activities: Option<String>,
    #[serde(deserialize_with = "deserialize_message_discriminator")]
    status: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_message_failure_code"
    )]
    failure_code: Option<String>,
    #[serde(default)]
    attachments: BoundedIncomingAttachments<
        MAXIMUM_ATTACHMENT_FILE_COUNT,
        MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
    >,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// User-selected model favorite state.
pub struct SetModelFavoriteInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    model_id: String,
    #[serde(deserialize_with = "deserialize_model_display_name")]
    display_name: String,
    is_favorite: bool,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    source_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Encoded attachment supplied with a persisted message; decoded data is size-checked before use.
struct IncomingMessageAttachment {
    #[serde(deserialize_with = "deserialize_attachment_id")]
    id: String,
    #[serde(deserialize_with = "deserialize_attachment_name")]
    name: String,
    #[serde(deserialize_with = "deserialize_attachment_mime_type")]
    mime_type: String,
    size_bytes: usize,
    #[serde(deserialize_with = "deserialize_attachment_kind")]
    kind: String,
    #[serde(deserialize_with = "deserialize_attachment_data")]
    data_base64: String,
}

/// Deserializes a text field while enforcing its existing UTF-8 byte bound.
fn deserialize_request_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedText::<MAX_BYTES>::deserialize(deserializer).map(BoundedText::into_inner)
}

/// Deserializes message content within the UTF-8 maximum for the existing character limit.
fn deserialize_message_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MESSAGE_CONTENT_BYTES>(deserializer)
}

/// Deserializes an optional text value while enforcing its UTF-8 byte bound.
fn deserialize_optional_request_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoundedText<MAX_BYTES>>::deserialize(deserializer)
        .map(|value| value.map(BoundedText::into_inner))
}

/// Deserializes conversation and project identifiers within their 128-byte storage limit.
fn deserialize_safe_identifier<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes an optional conversation or project identifier within its 128-byte limit.
fn deserialize_optional_safe_identifier<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes route model IDs within their existing 256-byte validation limit.
fn deserialize_model_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ROUTE_MODEL_ID_BYTES>(deserializer)
}

/// Deserializes an optional route model ID within its existing 256-byte validation limit.
fn deserialize_optional_model_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_ROUTE_MODEL_ID_BYTES>(deserializer)
}

/// Deserializes saved-output search text within its existing 512-character limit.
fn deserialize_saved_output_search_query<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_HISTORY_SEARCH_QUERY_BYTES>(deserializer)
}

/// Deserializes a conversation tag within the persisted tag-size limit.
fn deserialize_optional_history_search_tag<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_CONVERSATION_TAG_BYTES>(deserializer)
}

/// Deserializes a new conversation title within the existing 500-character limit.
fn deserialize_new_conversation_title<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_NEW_CONVERSATION_TITLE_BYTES>(deserializer)
}

/// Deserializes project names within the existing 512-character validation limit.
fn deserialize_project_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_PROJECT_NAME_BYTES>(deserializer)
}

/// Deserializes model display names within the existing 256-character limit.
fn deserialize_model_display_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MODEL_DISPLAY_NAME_BYTES>(deserializer)
}

/// Deserializes message role and status values within their small fixed-value bound.
fn deserialize_message_discriminator<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MESSAGE_STATUS_BYTES>(deserializer)
}

/// Deserializes optional message metadata within the 4 MiB per-field persistence limit.
fn deserialize_optional_message_metadata<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_MESSAGE_METADATA_BYTES>(deserializer)
}

/// Deserializes optional message failure codes within the safe identifier byte limit.
fn deserialize_optional_message_failure_code<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_MESSAGE_FAILURE_CODE_BYTES>(deserializer)
}

/// Deserializes an attachment identifier within the persisted identifier bound.
fn deserialize_attachment_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes an attachment name within the existing character limit's UTF-8 maximum.
fn deserialize_attachment_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_NAME_BYTES>(deserializer)
}

/// Deserializes an attachment MIME type within the accepted value set's byte bound.
fn deserialize_attachment_mime_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES>(deserializer)
}

/// Deserializes an attachment kind within the accepted `image` and `text` values.
fn deserialize_attachment_kind<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_KIND_BYTES>(deserializer)
}

/// Deserializes attachment data within the maximum cumulative encoded payload size.
fn deserialize_attachment_data<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES>(deserializer)
}

/// Incoming attachments bounded by item count and cumulative encoded content size.
#[derive(Debug, Default)]
struct BoundedIncomingAttachments<const MAX_ITEMS: usize, const MAX_BASE64_BYTES: usize>(
    Vec<IncomingMessageAttachment>,
);

impl<'de, const MAX_ITEMS: usize, const MAX_BASE64_BYTES: usize> Deserialize<'de>
    for BoundedIncomingAttachments<MAX_ITEMS, MAX_BASE64_BYTES>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedIncomingAttachmentsVisitor<
            const MAX_ITEMS: usize,
            const MAX_BASE64_BYTES: usize,
        >;

        impl<'de, const MAX_ITEMS: usize, const MAX_BASE64_BYTES: usize> Visitor<'de>
            for BoundedIncomingAttachmentsVisitor<MAX_ITEMS, MAX_BASE64_BYTES>
        {
            type Value = BoundedIncomingAttachments<MAX_ITEMS, MAX_BASE64_BYTES>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    formatter,
                    "at most {MAX_ITEMS} attachments with at most {MAX_BASE64_BYTES} encoded bytes"
                )
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let size_hint = sequence.size_hint();
                if size_hint.is_some_and(|size_hint| size_hint > MAX_ITEMS) {
                    return Err(de::Error::custom("attachment list exceeds its item limit"));
                }

                let mut attachments = Vec::with_capacity(size_hint.unwrap_or(0).min(MAX_ITEMS));
                let mut total_base64_bytes = 0_usize;
                loop {
                    if attachments.len() == MAX_ITEMS {
                        if sequence.next_element::<IgnoredAny>()?.is_some() {
                            return Err(de::Error::custom(
                                "attachment list exceeds its item limit",
                            ));
                        }
                        return Ok(BoundedIncomingAttachments(attachments));
                    }

                    let Some(attachment) = sequence.next_element::<IncomingMessageAttachment>()?
                    else {
                        return Ok(BoundedIncomingAttachments(attachments));
                    };
                    total_base64_bytes = total_base64_bytes
                        .checked_add(attachment.data_base64.len())
                        .ok_or_else(|| {
                            de::Error::custom("attachment data exceeds its encoded size limit")
                        })?;
                    if total_base64_bytes > MAX_BASE64_BYTES {
                        return Err(de::Error::custom(
                            "attachment data exceeds its encoded size limit",
                        ));
                    }
                    attachments.push(attachment);
                }
            }
        }

        deserializer
            .deserialize_seq(BoundedIncomingAttachmentsVisitor::<MAX_ITEMS, MAX_BASE64_BYTES>)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Validated attachment metadata stored alongside a message content envelope.
struct StoredAttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
}

#[derive(Debug, Deserialize)]
/// Database envelope used to separate message text from attachment metadata.
struct StoredMessageContentEnvelope {
    #[serde(rename = "content")]
    _content: String,
    attachments: Vec<StoredAttachmentMetadata>,
}

/// Decoded attachment bytes and metadata after all request and content bounds pass.
struct ValidatedAttachment {
    metadata: StoredAttachmentMetadata,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Database location and schema version after a successful integrity check and preparation.
pub struct PreparedDatabase {
    database_path: String,
    schema_version: i64,
}

#[tauri::command]
/// Checks the local database, applies supported schema migrations, and returns its location.
///
/// # Errors
/// Returns a storage or SQLite compatibility error when the database cannot be prepared safely.
pub fn prepare_chat_database() -> Result<PreparedDatabase, ChatStorageError> {
    let database_path = database_path()?;
    let parent = database_path
        .parent()
        .ok_or_else(ChatStorageError::unavailable)?;
    fs::create_dir_all(parent).map_err(|_| ChatStorageError::unavailable())?;
    let mut connection = open_supported_connection(&database_path)?;

    let integrity: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if integrity != "ok" {
        return Err(ChatStorageError::unavailable());
    }

    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let auto_vacuum: i64 = connection
        .query_row("PRAGMA auto_vacuum", [], |row| row.get(0))
        .unwrap_or(0);
    if auto_vacuum == 0 {
        let _ = connection.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;");
    }
    migrate_chat_schema(&mut connection).map_err(|_| ChatStorageError::unavailable())?;

    // Maintenance: prune expired ephemeral cache entries and check freelist
    if let Ok(now) = unix_time_milliseconds() {
        let _ = purge_expired_cache(&connection, now);
    }
    let _ = perform_incremental_vacuum_if_needed(&connection);

    let integrity: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if integrity != "ok" {
        return Err(ChatStorageError::unavailable());
    }
    Ok(PreparedDatabase {
        database_path: database_path.to_string_lossy().into_owned(),
        schema_version: CHAT_SCHEMA_VERSION,
    })
}

#[tauri::command]
/// Reports whether the database contains a completed assistant response.
///
/// # Errors
/// Returns a storage error if the query cannot be completed.
pub fn has_completed_assistant_response() -> Result<bool, ChatStorageError> {
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM messages
                WHERE role = 'assistant' AND status = 'completed'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Returns conversations ordered for the application's history list.
///
/// # Errors
/// Returns a storage error if the database query fails.
pub fn list_conversations() -> Result<Vec<StoredConversation>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id, tags,
                    is_pinned, is_archived, is_bookmarked, created_at, updated_at
             FROM conversations
             ORDER BY is_bookmarked DESC, is_pinned DESC, updated_at DESC, id ASC
            ",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], stored_conversation_from_row)
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Returns projects stored in the local conversation database.
///
/// # Errors
/// Returns a storage error if the database query fails.
pub fn list_projects() -> Result<Vec<StoredProject>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, folder_path, created_at, updated_at
             FROM projects ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(StoredProject {
                id: row.get(0)?,
                name: row.get(1)?,
                folder_path: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Creates a project using a validated, single-use native folder selection.
///
/// # Errors
/// Returns a validation or storage error if the selection is invalid, expired, or already used.
pub fn create_project(input: CreateProjectInput) -> Result<(), ChatStorageError> {
    let id = input.id.trim();
    let name = input.name.trim();
    if !is_safe_identifier(id)
        || name.is_empty()
        || name.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let folder_path = consume_project_folder_selection(input.folder_selection_id.trim())
        .map_err(|_| ChatStorageError::invalid_input())?;
    let folder_path = folder_path
        .to_str()
        .ok_or_else(ChatStorageError::invalid_input)?
        .to_owned();
    let now = unix_time_milliseconds()?;
    let connection = open_database()?;
    connection
        .execute(
            "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, name, folder_path, now],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Deletes conversation history while preserving unrelated application settings and catalogs.
///
/// # Errors
/// Returns a storage error if the database transaction cannot complete.
pub fn clear_conversation_history() -> Result<(), ChatStorageError> {
    let connection = open_database()?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute_batch(
            "DELETE FROM attachment_text_index;
             DELETE FROM saved_output_versions;
             DELETE FROM saved_outputs;
             DELETE FROM conversations;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;

    let storage_root = storage_root()?;
    match checked_attachment_root(&storage_root, false)? {
        Some(attachment_directory) => {
            fs::remove_dir_all(attachment_directory).map_err(|_| ChatStorageError::unavailable())
        }
        None => Ok(()),
    }
}

#[tauri::command]
/// Returns model favorites in the order used by the model picker.
///
/// # Errors
/// Returns a storage error if the database query fails.
pub fn list_model_favorites() -> Result<Vec<StoredModelFavorite>, ChatStorageError> {
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT provider_id, model_id, display_name, source_connection_id
             FROM model_favorites
             ORDER BY provider_id ASC, display_name ASC, model_id ASC",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let favorites = statement
        .query_map([], |row| {
            Ok(StoredModelFavorite {
                provider_id: row.get(0)?,
                model_id: row.get(1)?,
                display_name: row.get(2)?,
                source_connection_id: row.get(3)?,
            })
        })
        .map_err(|_| ChatStorageError::unavailable())?;
    favorites
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Adds or removes a model from the user's favorites.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_model_favorite(input: SetModelFavoriteInput) -> Result<(), ChatStorageError> {
    validate_model_favorite_input(&input)?;
    let connection = open_database()?;
    if input.is_favorite {
        let favorited_at = unix_time_milliseconds()?;
        connection
            .execute(
                "INSERT INTO model_favorites (
                    provider_id, model_id, display_name, favorited_at, source_connection_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(provider_id, model_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    favorited_at = excluded.favorited_at,
                    source_connection_id = excluded.source_connection_id",
                params![
                    input.provider_id,
                    input.model_id,
                    input.display_name.trim(),
                    favorited_at,
                    input.source_connection_id,
                ],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    } else {
        connection
            .execute(
                "DELETE FROM model_favorites WHERE provider_id = ?1 AND model_id = ?2",
                params![input.provider_id, input.model_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
    }
    Ok(())
}

#[tauri::command]
/// Loads one conversation and its persisted organization metadata.
///
/// # Errors
/// Returns a validation or storage error when the ID is invalid or the query fails.
pub fn get_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<Option<StoredConversation>, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    if conversation_id.trim().is_empty() || conversation_id.len() > 128 {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    connection
        .query_row(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id, tags,
                    is_pinned, is_archived, is_bookmarked, created_at, updated_at
             FROM conversations WHERE id = ?1",
            [conversation_id],
            stored_conversation_from_row,
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())
}

fn stored_conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredConversation> {
    let tags_json = row.get::<_, String>(9)?;
    let tags = serde_json::from_str::<Vec<String>>(&tags_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(StoredConversation {
        id: row.get(0)?,
        title: row.get(1)?,
        title_source: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        connection_id: row.get(5)?,
        workspace_id: row.get(6)?,
        api_key_connection_id: row.get(7)?,
        project_id: row.get(8)?,
        tags,
        is_pinned: row.get(10)?,
        is_archived: row.get(11)?,
        is_bookmarked: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[tauri::command]
/// Replaces a conversation title after validating its length and content.
///
/// # Errors
/// Returns a validation or storage error.
pub fn rename_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    title: BoundedText<MAXIMUM_CONVERSATION_TITLE_BYTES>,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let title = title.into_inner();
    let title = title.trim();
    if !is_safe_identifier(&conversation_id)
        || title.is_empty()
        || title.chars().count() > MAXIMUM_CONVERSATION_TITLE_CHARS
        || title.chars().any(char::is_control)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations SET title = ?1, title_source = 'manual', updated_at = ?2
             WHERE id = ?3",
            params![title, unix_time_milliseconds()?, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
/// Changes the provider/model route and optional connection metadata for a conversation.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_route(input: SetConversationRouteInput) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&input.conversation_id)
        || !is_safe_identifier(&input.provider_id)
        || !is_valid_route_model_id(&input.model_id)
        || !is_optional_safe_identifier(input.connection_id.as_deref())
        || !is_optional_safe_identifier(input.workspace_id.as_deref())
        || !is_optional_safe_identifier(input.api_key_connection_id.as_deref())
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations
             SET provider_id = ?1, model_id = ?2, connection_id = ?3,
                 workspace_id = ?4, api_key_connection_id = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                input.provider_id,
                input.model_id,
                input.connection_id,
                input.workspace_id,
                input.api_key_connection_id,
                unix_time_milliseconds()?,
                input.conversation_id,
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
/// Sets whether a conversation is pinned in the local history list.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_pinned(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    is_pinned: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(&conversation_id.into_inner(), "is_pinned", is_pinned)
}

#[tauri::command]
/// Sets whether a conversation is bookmarked.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_bookmarked(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    is_bookmarked: bool,
) -> Result<(), ChatStorageError> {
    set_conversation_flag(
        &conversation_id.into_inner(),
        "is_bookmarked",
        is_bookmarked,
    )
}

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

fn search_saved_outputs_from_connection(
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
        || input.tag.as_deref().is_some_and(|value| {
            value.trim().is_empty()
                || value.chars().count() > MAXIMUM_CONVERSATION_TAG_CHARS
                || value.chars().any(char::is_control)
        })
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
        "SELECT c.id, c.title, c.project_id, c.tags, c.is_archived, c.updated_at,
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
        let tags_json = row
            .get::<_, String>(3)
            .map_err(|_| ChatStorageError::unavailable())?;
        let is_archived = row
            .get::<_, bool>(4)
            .map_err(|_| ChatStorageError::unavailable())?;
        let conversation_updated_at = row
            .get::<_, i64>(5)
            .map_err(|_| ChatStorageError::unavailable())?;
        let message_id = row
            .get::<_, String>(6)
            .map_err(|_| ChatStorageError::unavailable())?;
        let role = row
            .get::<_, String>(7)
            .map_err(|_| ChatStorageError::unavailable())?;
        let stored_content = row
            .get::<_, String>(8)
            .map_err(|_| ChatStorageError::unavailable())?;
        let message_created_at = row
            .get::<_, Option<i64>>(9)
            .map_err(|_| ChatStorageError::unavailable())?;
        let tags = serde_json::from_str::<Vec<String>>(&tags_json)
            .map_err(|_| ChatStorageError::unavailable())?;

        if input
            .is_archived
            .is_some_and(|expected| expected != is_archived)
            || input
                .tag
                .as_deref()
                .is_some_and(|expected| !tags.iter().any(|tag| tag == expected))
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

fn bounded_history_search_excerpt(content: &str) -> String {
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

fn set_conversation_flag(
    conversation_id: &str,
    column: &str,
    value: bool,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !matches!(column, "is_pinned" | "is_bookmarked") {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            &format!("UPDATE conversations SET {column} = ?1 WHERE id = ?2"),
            params![value, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
/// Replaces a conversation's bounded set of searchable tags.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversation_tags(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    tags: BoundedVec<
        BoundedText<MAXIMUM_CONVERSATION_TAG_BYTES>,
        MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS,
    >,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    if !is_safe_identifier(&conversation_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in tags.into_strings() {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > MAXIMUM_CONVERSATION_TAG_CHARS || tag.chars().any(char::is_control)
        {
            return Err(ChatStorageError::invalid_input());
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_owned());
        }
    }
    if normalized.len() > MAXIMUM_CONVERSATION_TAGS {
        return Err(ChatStorageError::invalid_input());
    }
    let tags_json =
        serde_json::to_string(&normalized).map_err(|_| ChatStorageError::invalid_input())?;
    let connection = open_database()?;
    let updated_rows = connection
        .execute(
            "UPDATE conversations SET tags = ?1 WHERE id = ?2",
            params![tags_json, conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if updated_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

#[tauri::command]
/// Moves a bounded set of conversations into or out of a project.
///
/// # Errors
/// Returns a validation or storage error if any ID or destination is invalid.
pub fn move_conversations(
    conversation_ids: BoundedVec<
        BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
    >,
    project_id: Option<BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>>,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids.into_strings())?;
    let project_id = project_id.map(BoundedText::into_inner);
    if project_id
        .as_deref()
        .is_some_and(|id| !is_safe_identifier(id))
    {
        return Err(ChatStorageError::invalid_input());
    }
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    if let Some(project_id) = project_id.as_deref() {
        let project_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if !project_exists {
            return Err(ChatStorageError::invalid_input());
        }
    }
    for conversation_id in ids {
        let updated_rows = transaction
            .execute(
                "UPDATE conversations SET project_id = ?1, is_pinned = 0 WHERE id = ?2",
                params![project_id, conversation_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if updated_rows != 1 {
            return Err(ChatStorageError::invalid_input());
        }
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Sets archived state for a bounded set of conversations.
///
/// # Errors
/// Returns a validation or storage error.
pub fn set_conversations_archived(
    conversation_ids: BoundedVec<
        BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
    >,
    is_archived: bool,
) -> Result<(), ChatStorageError> {
    let ids = normalize_conversation_ids(conversation_ids.into_strings())?;
    let mut connection = open_database()?;
    let transaction = connection
        .transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    for conversation_id in ids {
        let updated_rows = transaction
            .execute(
                "UPDATE conversations
                 SET is_archived = ?1,
                     is_pinned = CASE WHEN ?1 = 1 THEN 0 ELSE is_pinned END
                 WHERE id = ?2",
                params![is_archived, conversation_id],
            )
            .map_err(|_| ChatStorageError::unavailable())?;
        if updated_rows != 1 {
            return Err(ChatStorageError::invalid_input());
        }
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())
}

fn normalize_conversation_ids(ids: Vec<String>) -> Result<Vec<String>, ChatStorageError> {
    if ids.is_empty() || ids.len() > MAXIMUM_CONVERSATION_IDS_PER_MUTATION {
        return Err(ChatStorageError::invalid_input());
    }
    let mut unique = std::collections::HashSet::with_capacity(ids.len());
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        if !is_safe_identifier(&id) {
            return Err(ChatStorageError::invalid_input());
        }
        if unique.insert(id.clone()) {
            normalized.push(id);
        }
    }
    Ok(normalized)
}

#[tauri::command]
/// Permanently deletes one conversation and its dependent persisted content.
///
/// # Errors
/// Returns a validation or storage error.
pub fn delete_conversation(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<(), ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    if !is_safe_identifier(&conversation_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM attachment_text_index WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM saved_output_versions WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    transaction
        .execute(
            "DELETE FROM saved_outputs WHERE conversation_id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    let deleted_rows = transaction
        .execute(
            "DELETE FROM conversations WHERE id = ?1",
            [&conversation_id],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    if deleted_rows != 1 {
        return Err(ChatStorageError::invalid_input());
    }
    transaction
        .commit()
        .map_err(|_| ChatStorageError::unavailable())?;
    delete_conversation_attachments(&conversation_id)
}

fn delete_conversation_attachments(conversation_id: &str) -> Result<(), ChatStorageError> {
    let storage_root = storage_root()?;
    let Some(attachments_root) = checked_attachment_root(&storage_root, false)? else {
        return Ok(());
    };
    let Some(canonical_directory) =
        checked_attachment_subdirectory(&attachments_root, conversation_id, false)?
    else {
        return Ok(());
    };
    cleanup_attachment_directory(&canonical_directory)
}

#[tauri::command]
/// Exports one conversation as Markdown through a native save dialog.
///
/// Cancellation leaves the database unchanged. The output is written only after the selected
/// destination and generated content pass validation. A same-directory staging file is fully
/// written and synchronized before replacing the destination, so an interrupted write does not
/// leave a truncated export or follow a destination symlink.
///
/// # Errors
/// Returns a validation, dialog, or storage error.
pub async fn save_conversation_markdown(
    window: tauri::WebviewWindow,
    file_name: BoundedText<MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES>,
    title: BoundedText<MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES>,
    content: BoundedText<MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES>,
) -> Result<bool, ChatStorageError> {
    let file_name = file_name.into_inner();
    let title = title.into_inner();
    let content = content.into_inner();
    if !is_safe_markdown_export_file_name(&file_name)
        || title.is_empty()
        || title.len() > 128
        || title.chars().any(char::is_control)
        || content.is_empty()
        || content.len() > 20 * 1024 * 1024
    {
        return Err(ChatStorageError::invalid_input());
    }

    let Some(path) = select_save_file(&window, file_name, title, "Markdown", "md")
        .await
        .map_err(|error| match error {
            SaveFileDialogError::Unavailable => ChatStorageError::unavailable(),
        })?
    else {
        return Ok(false);
    };
    if !path.is_absolute()
        || path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("md"))
    {
        return Err(ChatStorageError::invalid_input());
    }
    write_markdown_export(&path, content.as_bytes())?;
    Ok(true)
}

/// Publishes Markdown bytes only after a complete same-directory staging write.
fn write_markdown_export(path: &Path, content: &[u8]) -> Result<(), ChatStorageError> {
    let parent = path.parent().ok_or_else(ChatStorageError::unavailable)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_parent.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let file_name = path
        .file_name()
        .ok_or_else(ChatStorageError::invalid_input)?;
    let destination = canonical_parent.join(file_name);
    let staging = canonical_parent.join(format!(".openchat-markdown-{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&staging);
        return Err(ChatStorageError::unavailable());
    }
    if replace_file_atomically(&staging, &destination).is_err() {
        let _ = fs::remove_file(&staging);
        return Err(ChatStorageError::unavailable());
    }
    Ok(())
}

fn is_safe_markdown_export_file_name(file_name: &str) -> bool {
    let path = Path::new(file_name);
    !file_name.is_empty()
        && file_name.len() <= 255
        && !file_name.chars().any(char::is_control)
        && !file_name.contains(['/', '\\', ':'])
        && path.components().count() == 1
        && path.file_name().and_then(std::ffi::OsStr::to_str) == Some(file_name)
        && path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod markdown_export_tests {
    use std::{fs, path::PathBuf};

    use super::{is_safe_markdown_export_file_name, write_markdown_export, CreateProjectInput};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-markdown-export-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated Markdown export directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn markdown_export_file_name_accepts_a_simple_markdown_name() {
        assert!(is_safe_markdown_export_file_name("conversation-notes.md"));
        assert!(is_safe_markdown_export_file_name("Sohbet Özeti.MD"));
    }

    #[test]
    fn markdown_export_file_name_rejects_paths_and_other_extensions() {
        assert!(!is_safe_markdown_export_file_name(""));
        assert!(!is_safe_markdown_export_file_name("..\\outside.md"));
        assert!(!is_safe_markdown_export_file_name(
            "C:\\Users\\Public\\outside.md"
        ));
        assert!(!is_safe_markdown_export_file_name("folder/notes.md"));
        assert!(!is_safe_markdown_export_file_name("notes.txt"));
    }

    #[test]
    fn markdown_export_replaces_existing_content_only_after_staging_write() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("conversation.md");
        fs::write(&destination, b"old export").expect("write existing export");

        write_markdown_export(&destination, b"complete new export")
            .expect("publish complete staged export");

        assert_eq!(
            fs::read(destination).expect("read published Markdown"),
            b"complete new export"
        );
    }

    #[test]
    fn project_creation_rejects_renderer_supplied_folder_paths() {
        let request = r#"{
            "id": "project-1",
            "name": "Project",
            "folderSelectionId": "550e8400-e29b-41d4-a716-446655440000",
            "folderPath": "C:\\private"
        }"#;

        assert!(serde_json::from_str::<CreateProjectInput>(request).is_err());
    }
}

#[cfg(test)]
mod attachment_path_tests {
    use std::{fs, path::PathBuf};

    use super::{checked_attachment_root, checked_attachment_subdirectory};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-attachment-paths-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated attachment path directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::windows::fs::symlink_dir(source, destination)
            .expect("create isolated directory symlink");
    }

    #[cfg(unix)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::unix::fs::symlink(source, destination).expect("create isolated directory symlink");
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_root_rejects_a_symlink_to_an_external_directory() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        let external_directory = directory.0.join("external");
        fs::create_dir(&storage_root).expect("create storage root");
        fs::create_dir(&external_directory).expect("create external directory");
        fs::write(external_directory.join("keep.txt"), b"preserve")
            .expect("write external sentinel");
        create_directory_symlink(&external_directory, &storage_root.join("attachments"));

        assert!(checked_attachment_root(&storage_root, true).is_err());
        assert_eq!(
            fs::read(external_directory.join("keep.txt")).expect("read external sentinel"),
            b"preserve"
        );
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_subdirectory_rejects_a_symlink_even_when_it_stays_inside_the_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");
        let attachments_root = checked_attachment_root(&storage_root, true)
            .expect("validate attachment root")
            .expect("created attachment root");
        fs::create_dir(attachments_root.join("other-conversation"))
            .expect("create another conversation directory");
        create_directory_symlink(
            &attachments_root.join("other-conversation"),
            &attachments_root.join("conversation-1"),
        );

        assert!(
            checked_attachment_subdirectory(&attachments_root, "conversation-1", true).is_err()
        );
    }

    #[test]
    fn attachment_root_is_created_only_inside_the_application_storage_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");

        let attachment_root = checked_attachment_root(&storage_root, true)
            .expect("create checked attachment root")
            .expect("created attachment root");

        assert_eq!(
            attachment_root,
            fs::canonicalize(storage_root.join("attachments"))
                .expect("canonicalize created attachment root")
        );
    }
}

#[tauri::command]
/// Loads messages for one conversation in transcript order.
///
/// # Errors
/// Returns a validation or storage error.
pub fn get_messages(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<Vec<StoredMessage>, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    if conversation_id.trim().is_empty() || conversation_id.len() > 128 {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, provider_id, model_id,
                    citation_sources, personal_memory_fact_ids, reasoning_summaries,
                    tool_activities, status, failure_code
             FROM messages WHERE conversation_id = ?1
             ORDER BY created_at ASC, id ASC LIMIT 10000",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
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
        .map_err(|_| ChatStorageError::unavailable())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| ChatStorageError::unavailable())
}

#[tauri::command]
/// Returns one validated attachment's bytes for a message in the specified conversation.
///
/// The lookup is scoped to the conversation/message pair and applies attachment size limits.
///
/// # Errors
/// Returns a validation or storage error when the message or attachment is unavailable.
pub fn read_message_attachment(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    attachment_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<String, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    let attachment_id = attachment_id.into_inner();
    if !is_safe_identifier(&conversation_id)
        || !is_safe_identifier(&message_id)
        || !is_safe_identifier(&attachment_id)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let stored_content = connection
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())?
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = parse_stored_message_content(&stored_content)?;
    let metadata = envelope
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or_else(ChatStorageError::unavailable)?;
    let storage_root = storage_root()?;
    let attachments_root =
        checked_attachment_root(&storage_root, false)?.ok_or_else(ChatStorageError::unavailable)?;
    let conversation_directory =
        checked_attachment_subdirectory(&attachments_root, &conversation_id, false)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let canonical_directory =
        checked_attachment_subdirectory(&conversation_directory, &message_id, false)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let attachment_path = canonical_directory.join(format!("{}.data", metadata.id));
    let attachment_metadata =
        fs::symlink_metadata(&attachment_path).map_err(|_| ChatStorageError::unavailable())?;
    if attachment_metadata.file_type().is_symlink() || !attachment_metadata.is_file() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_attachment =
        fs::canonicalize(&attachment_path).map_err(|_| ChatStorageError::unavailable())?;
    let expected_attachment = canonical_directory.join(format!("{}.data", metadata.id));
    if canonical_attachment != expected_attachment
        || !canonical_attachment.starts_with(&canonical_directory)
    {
        return Err(ChatStorageError::unavailable());
    }
    let bytes = fs::read(canonical_attachment).map_err(|_| ChatStorageError::unavailable())?;
    validate_stored_attachment(metadata, &bytes)?;
    Ok(STANDARD.encode(bytes))
}

#[tauri::command]
/// Creates a new conversation after validating its ID, route, and optional project.
///
/// # Errors
/// Returns a validation or storage error.
pub fn create_conversation(input: CreateConversationInput) -> Result<(), ChatStorageError> {
    validate_conversation_input(&input)?;
    let connection = open_database()?;
    let now = unix_time_milliseconds()?;
    connection
        .execute(
            "INSERT INTO conversations (
                id, title, title_source, provider_id, model_id, connection_id,
                workspace_id, api_key_connection_id, project_id, created_at, updated_at
             ) VALUES (?1, ?2, 'automatic', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                input.id.trim(),
                input.title.trim(),
                input.provider_id,
                input.model_id,
                input.connection_id,
                input.workspace_id,
                input.api_key_connection_id,
                input.project_id,
                now,
            ],
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[tauri::command]
/// Persists one message and its validated attachments using a database transaction.
///
/// # Errors
/// Returns a validation or storage error; oversized or malformed attachment data is rejected.
pub fn save_message(input: SaveMessageInput) -> Result<(), ChatStorageError> {
    let validated_attachments = validate_incoming_attachments(&input.attachments.0)?;
    validate_message_input(&input, !validated_attachments.is_empty())?;
    let attachment_metadata = validated_attachments
        .iter()
        .map(|attachment| attachment.metadata.clone())
        .collect::<Vec<_>>();
    let content = if attachment_metadata.is_empty() {
        input.content.clone()
    } else {
        if input.role != "user" {
            return Err(ChatStorageError::invalid_input());
        }
        let envelope = json!({
            "content": input.content,
            "attachments": attachment_metadata,
        });
        format!(
            "{MESSAGE_CONTENT_PREFIX}{}",
            serde_json::to_string(&envelope).map_err(|_| ChatStorageError::invalid_input())?
        )
    };
    if content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let created_at = input.created_at.unwrap_or(unix_time_milliseconds()?);
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| ChatStorageError::unavailable())?;
    let staged_attachments = if validated_attachments.is_empty() {
        None
    } else {
        Some(stage_message_attachments(
            &input.conversation_id,
            &input.id,
            &validated_attachments,
        )?)
    };
    if transaction
        .execute(
            "INSERT INTO messages (
                id, conversation_id, role, content, created_at, output_tokens,
                tokens_per_second, elapsed_microseconds, provider_id, model_id,
                citation_sources, personal_memory_fact_ids, reasoning_summaries,
                tool_activities, status, failure_code
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16
             ) ON CONFLICT(conversation_id, id) DO UPDATE SET
                role = excluded.role,
                content = excluded.content,
                created_at = excluded.created_at,
                output_tokens = excluded.output_tokens,
                tokens_per_second = excluded.tokens_per_second,
                elapsed_microseconds = excluded.elapsed_microseconds,
                provider_id = excluded.provider_id,
                model_id = excluded.model_id,
                citation_sources = excluded.citation_sources,
                personal_memory_fact_ids = excluded.personal_memory_fact_ids,
                reasoning_summaries = excluded.reasoning_summaries,
                tool_activities = excluded.tool_activities,
                status = excluded.status,
                failure_code = excluded.failure_code",
            params![
                input.id.trim(),
                input.conversation_id.trim(),
                input.role,
                content,
                created_at,
                input.output_tokens,
                input.tokens_per_second,
                input.elapsed_microseconds,
                input.provider_id,
                input.model_id,
                input.citation_sources.unwrap_or_else(|| "[]".to_owned()),
                input
                    .personal_memory_fact_ids
                    .unwrap_or_else(|| "[]".to_owned()),
                input.reasoning_summaries.unwrap_or_else(|| "[]".to_owned()),
                input.tool_activities.unwrap_or_else(|| "[]".to_owned()),
                input.status,
                input.failure_code,
            ],
        )
        .is_err()
    {
        drop(transaction);
        if let Some((staging, _)) = staged_attachments {
            cleanup_attachment_directory(&staging)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    if transaction
        .execute(
            "UPDATE conversations SET updated_at = MAX(updated_at, ?1) WHERE id = ?2",
            params![created_at, input.conversation_id.trim()],
        )
        .is_err()
    {
        drop(transaction);
        if let Some((staging, _)) = staged_attachments {
            cleanup_attachment_directory(&staging)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    let mut published_attachments = None;
    if let Some((staging, destination)) = staged_attachments {
        if fs::rename(&staging, &destination).is_err() {
            drop(transaction);
            cleanup_attachment_directory(&staging)?;
            return Err(ChatStorageError::unavailable());
        }
        published_attachments = Some(destination);
    }
    if transaction.commit().is_err() {
        if let Some(destination) = published_attachments {
            cleanup_attachment_directory(&destination)?;
        }
        return Err(ChatStorageError::unavailable());
    }
    Ok(())
}

fn validate_conversation_input(input: &CreateConversationInput) -> Result<(), ChatStorageError> {
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input.title.trim().is_empty()
        || input.title.chars().count() > 500
        || input.provider_id.trim().is_empty()
        || input.model_id.trim().is_empty()
        || input.model_id.len() > 256
        || input
            .project_id
            .as_deref()
            .is_some_and(|project_id| !is_safe_identifier(project_id))
    {
        return Err(ChatStorageError::invalid_input());
    }

    let provider_id = input.provider_id.as_str();
    let has_oauth_route = input.connection_id.is_some() || input.workspace_id.is_some();
    let has_api_route = input.api_key_connection_id.is_some();
    let api_provider = provider_id == "chatgpt_api"
        || [
            "gemini",
            "groq",
            "cerebras",
            "openrouter",
            "mistral",
            "deepseek",
            "xai",
            "perplexity",
            "together",
            "sambanova",
            "anthropic",
            "fireworks",
            "deepinfra",
            "dashscope",
            "cohere",
        ]
        .contains(&provider_id);
    let local_provider = provider_id == "llama_cpp";
    let custom_provider = is_custom_provider_id(provider_id);
    let known_provider = provider_id == "chatgpt"
        || provider_id == "opencode"
        || local_provider
        || api_provider
        || custom_provider;

    if !known_provider
        || (provider_id == "chatgpt"
            && (has_api_route || input.connection_id.is_none() || input.workspace_id.is_none()))
        || ((provider_id == "opencode" || local_provider) && (has_oauth_route || has_api_route))
        || (api_provider && (has_oauth_route || !has_api_route))
        || (custom_provider
            && (has_oauth_route
                || !has_api_route
                || input.api_key_connection_id.as_deref() != Some(provider_id)))
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_model_favorite_input(input: &SetModelFavoriteInput) -> Result<(), ChatStorageError> {
    const API_KEY_PROVIDERS: [&str; 16] = [
        "chatgpt_api",
        "gemini",
        "groq",
        "cerebras",
        "openrouter",
        "mistral",
        "deepseek",
        "xai",
        "perplexity",
        "together",
        "sambanova",
        "anthropic",
        "fireworks",
        "deepinfra",
        "dashscope",
        "cohere",
    ];
    let api_key_provider = API_KEY_PROVIDERS.contains(&input.provider_id.as_str())
        || is_custom_provider_id(&input.provider_id);
    let known_provider = api_key_provider
        || matches!(
            input.provider_id.as_str(),
            "chatgpt" | "opencode" | "llama_cpp"
        );
    let valid_source_connection = match (api_key_provider, input.source_connection_id.as_deref()) {
        (true, Some(source_connection_id)) => is_safe_identifier(source_connection_id),
        (false, None) => true,
        _ => false,
    };

    if !known_provider
        || input.provider_id.trim() != input.provider_id
        || input.model_id.trim().is_empty()
        || input.model_id.trim() != input.model_id
        || input.model_id.len() > 256
        || input.model_id.chars().any(char::is_control)
        || input.display_name.trim().is_empty()
        || input.display_name.chars().count() > 256
        || input.display_name.chars().any(char::is_control)
        || !valid_source_connection
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_message_input(
    input: &SaveMessageInput,
    has_attachments: bool,
) -> Result<(), ChatStorageError> {
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input.conversation_id.trim().is_empty()
        || input.conversation_id.len() > 128
        || !matches!(input.role.as_str(), "user" | "assistant")
        || !matches!(
            input.status.as_str(),
            "streaming" | "completed" | "stopped" | "failed"
        )
        || input.content.chars().count() > MAX_MESSAGE_CHARS
        || (input.role == "user" && !has_attachments && input.content.trim().is_empty())
        || input.output_tokens.is_some_and(|value| value < 0)
        || input.elapsed_microseconds.is_some_and(|value| value < 0)
        || input
            .tokens_per_second
            .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(ChatStorageError::invalid_input());
    }
    for json in [
        input.citation_sources.as_deref(),
        input.personal_memory_fact_ids.as_deref(),
        input.reasoning_summaries.as_deref(),
        input.tool_activities.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let value = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|_| ChatStorageError::invalid_input())?;
        if !value.is_array() {
            return Err(ChatStorageError::invalid_input());
        }
    }
    Ok(())
}

fn validate_incoming_attachments(
    attachments: &[IncomingMessageAttachment],
) -> Result<Vec<ValidatedAttachment>, ChatStorageError> {
    if attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT {
        return Err(ChatStorageError::invalid_input());
    }
    let mut ids = std::collections::HashSet::with_capacity(attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    let mut validated = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let metadata = StoredAttachmentMetadata {
            id: attachment.id.clone(),
            name: attachment.name.clone(),
            mime_type: attachment.mime_type.clone(),
            size_bytes: attachment.size_bytes,
            kind: attachment.kind.clone(),
        };
        validate_attachment_metadata(&metadata)?;
        if !ids.insert(metadata.id.clone()) {
            return Err(ChatStorageError::invalid_input());
        }
        let (_, _, maximum_bytes) =
            attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
        let maximum_base64_length = maximum_bytes.div_ceil(3).saturating_mul(4);
        if attachment.data_base64.len() > maximum_base64_length {
            return Err(ChatStorageError::invalid_input());
        }
        let bytes = STANDARD
            .decode(&attachment.data_base64)
            .map_err(|_| ChatStorageError::invalid_input())?;
        validate_stored_attachment(&metadata, &bytes)?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(ChatStorageError::invalid_input)?;
        if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES {
            return Err(ChatStorageError::invalid_input());
        }
        if metadata.kind == "image" {
            image_count += 1;
            if image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT {
                return Err(ChatStorageError::invalid_input());
            }
        }
        validated.push(ValidatedAttachment { metadata, bytes });
    }
    Ok(validated)
}

fn validate_attachment_metadata(
    metadata: &StoredAttachmentMetadata,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > 255
        || metadata.name.chars().any(char::is_control)
        || metadata.size_bytes == 0
    {
        return Err(ChatStorageError::invalid_input());
    }
    let (kind, mime_type, maximum_bytes) =
        attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
    if metadata.kind != kind
        || metadata.mime_type != mime_type
        || metadata.size_bytes > maximum_bytes
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn validate_stored_attachment(
    metadata: &StoredAttachmentMetadata,
    bytes: &[u8],
) -> Result<(), ChatStorageError> {
    validate_attachment_metadata(metadata)?;
    if bytes.is_empty() || bytes.len() != metadata.size_bytes {
        return Err(ChatStorageError::invalid_input());
    }
    let valid_content = match metadata.kind.as_str() {
        "image" => has_valid_image_signature(&metadata.mime_type, bytes),
        "text" => std::str::from_utf8(bytes).is_ok(),
        _ => false,
    };
    if !valid_content {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "html" => ("text", "text/html", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "css" => ("text", "text/css", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "js" | "jsx" => ("text", "text/javascript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "ts" | "tsx" => ("text", "text/typescript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "dart" | "rs" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h" | "cpp" | "hpp"
        | "cs" | "sh" | "ps1" | "yaml" | "yml" | "toml" | "ini" | "sql" => {
            ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES)
        }
        _ => return None,
    };
    Some(spec)
}

fn has_valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

fn parse_stored_message_content(
    stored_content: &str,
) -> Result<StoredMessageContentEnvelope, ChatStorageError> {
    let encoded = stored_content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = serde_json::from_str::<StoredMessageContentEnvelope>(encoded)
        .map_err(|_| ChatStorageError::unavailable())?;
    if envelope.attachments.is_empty() || envelope.attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    let mut ids = std::collections::HashSet::with_capacity(envelope.attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    for attachment in &envelope.attachments {
        validate_attachment_metadata(attachment).map_err(|_| ChatStorageError::unavailable())?;
        if !ids.insert(attachment.id.as_str()) {
            return Err(ChatStorageError::unavailable());
        }
        total_bytes = total_bytes.saturating_add(attachment.size_bytes);
        if attachment.kind == "image" {
            image_count += 1;
        }
    }
    if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES || image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    Ok(envelope)
}

fn stage_message_attachments(
    conversation_id: &str,
    message_id: &str,
    attachments: &[ValidatedAttachment],
) -> Result<(PathBuf, PathBuf), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let root = storage_root()?;
    let canonical_root =
        checked_attachment_root(&root, true)?.ok_or_else(ChatStorageError::unavailable)?;
    let canonical_conversation =
        checked_attachment_subdirectory(&canonical_root, conversation_id, true)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let destination = canonical_conversation.join(message_id);
    if destination.exists() {
        return Err(ChatStorageError::unavailable());
    }
    let staging =
        canonical_conversation.join(format!(".stage-{message_id}-{}", unix_time_milliseconds()?));
    fs::create_dir(&staging).map_err(|_| ChatStorageError::unavailable())?;
    for attachment in attachments {
        let path = staging.join(format!("{}.data", attachment.metadata.id));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(&attachment.bytes)?;
            file.sync_all()
        })();
        if write_result.is_err() {
            cleanup_attachment_directory(&staging)?;
            return Err(ChatStorageError::unavailable());
        }
    }
    Ok((staging, destination))
}

/// Resolves the attachment root only when it is a real directory under application storage.
///
/// When `create` is true, the directory is created if absent; an absent directory otherwise
/// remains an explicit `None` result so deletion can be idempotent.
fn checked_attachment_root(
    storage_root: &Path,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| ChatStorageError::unavailable())?;
    let expected_root = canonical_storage_root.join("attachments");
    if create {
        match fs::create_dir(&expected_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_root =
        fs::canonicalize(&expected_root).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_root != expected_root || !canonical_root.starts_with(&canonical_storage_root) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_root))
}

/// Resolves one identifier-named attachment directory without following path redirections.
///
/// When `create` is true, a missing directory is created beneath the already checked parent.
fn checked_attachment_subdirectory(
    parent: &Path,
    identifier: &str,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    if !is_safe_identifier(identifier) {
        return Err(ChatStorageError::invalid_input());
    }
    let expected_directory = parent.join(identifier);
    if create {
        match fs::create_dir(&expected_directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&expected_directory).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_directory != expected_directory || !canonical_directory.starts_with(parent) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_directory))
}

fn cleanup_attachment_directory(path: &std::path::Path) -> Result<(), ChatStorageError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ChatStorageError::unavailable()),
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_SAFE_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn is_optional_safe_identifier(value: Option<&str>) -> bool {
    value.is_none_or(is_safe_identifier)
}

fn is_valid_route_model_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 512 && !value.chars().any(char::is_control)
}

fn storage_root() -> Result<PathBuf, ChatStorageError> {
    database_path()?
        .parent()
        .and_then(std::path::Path::parent)
        .map(PathBuf::from)
        .ok_or_else(ChatStorageError::unavailable)
}

fn is_custom_provider_id(provider_id: &str) -> bool {
    let Some(suffix) = provider_id.strip_prefix("custom_") else {
        return false;
    };
    (12..=64).contains(&suffix.len())
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn database_path() -> Result<PathBuf, ChatStorageError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(ChatStorageError::unavailable)?;
    Ok(PathBuf::from(local_app_data)
        .join("OpenChat")
        .join("db")
        .join("openchat.sqlite3"))
}

fn open_database() -> Result<Connection, ChatStorageError> {
    open_supported_connection(&database_path()?)
}

fn open_supported_connection(path: &Path) -> Result<Connection, ChatStorageError> {
    if rusqlite::version_number() < MINIMUM_SQLITE_VERSION_NUMBER {
        return Err(ChatStorageError::unsupported_sqlite());
    }
    let connection = Connection::open(path).map_err(|_| ChatStorageError::unavailable())?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| ChatStorageError::unavailable())?;
    connection
        .pragma_update(None, "trusted_schema", false)
        .map_err(|_| ChatStorageError::unavailable())?;
    let trusted_schema_enabled = connection
        .pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))
        .map_err(|_| ChatStorageError::unavailable())?;
    if trusted_schema_enabled {
        return Err(ChatStorageError::unavailable());
    }
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    connection
        .create_scalar_function(
            "openchat_redact_credentials",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
                | rusqlite::functions::FunctionFlags::SQLITE_INNOCUOUS,
            |ctx| {
                let value = ctx.get::<Option<String>>(0)?;
                Ok(value)
            },
        )
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(connection)
}

#[cfg(test)]
mod database_connection_tests {
    use std::path::Path;

    use super::open_supported_connection;

    #[test]
    fn supported_connections_disable_trusted_schema() {
        let connection = open_supported_connection(Path::new(":memory:"))
            .expect("open configured in-memory database");
        let trusted_schema_enabled = connection
            .pragma_query_value(None, "trusted_schema", |row| row.get::<_, bool>(0))
            .expect("read trusted schema setting");

        assert!(!trusted_schema_enabled);
    }

    #[test]
    fn supported_connections_register_redact_function_and_pragmas() {
        let connection = open_supported_connection(Path::new(":memory:"))
            .expect("open configured in-memory database");
        let result: String = connection
            .query_row(
                "SELECT openchat_redact_credentials('test-key-123')",
                [],
                |row| row.get(0),
            )
            .expect("execute openchat_redact_credentials scalar function");
        assert_eq!(result, "test-key-123");
    }
}

fn unix_time_milliseconds() -> Result<i64, ChatStorageError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChatStorageError::unavailable())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ChatStorageError::unavailable())
}



#[tauri::command]
/// Runs SQLite's optimizer pragma to update query planner statistics before idle or shutdown.
///
/// # Errors
/// Returns a storage error if PRAGMA optimize fails.
pub fn optimize_database() -> Result<(), ChatStorageError> {
    let connection = open_database()?;
    connection
        .execute("PRAGMA optimize", [])
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}

#[cfg(test)]
mod request_bound_tests {
    use serde_json::{json, Value};

    use super::{
        BoundedIncomingAttachments, CreateConversationInput, SaveMessageInput,
        MAXIMUM_ATTACHMENT_FILE_COUNT, MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
        MAXIMUM_CONVERSATION_IDS_PER_MUTATION, MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS,
        MAXIMUM_MESSAGE_METADATA_BYTES, MAXIMUM_NEW_CONVERSATION_TITLE_BYTES,
        MAXIMUM_SAFE_IDENTIFIER_BYTES,
    };
    use crate::request_bounds::{BoundedText, BoundedVec};

    fn attachment(data_base64: &str) -> Value {
        json!({
            "id": "attachment-1",
            "name": "note.txt",
            "mimeType": "text/plain",
            "sizeBytes": 1,
            "kind": "text",
            "dataBase64": data_base64,
        })
    }

    #[test]
    fn conversation_mutation_deserialization_keeps_existing_item_limits() {
        let ids_at_limit = Value::Array(
            (0..MAXIMUM_CONVERSATION_IDS_PER_MUTATION)
                .map(|_| json!("conversation-1"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<
                BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
                MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
            >,
        >(ids_at_limit)
        .is_ok());

        let ids_over_limit = Value::Array(
            (0..=MAXIMUM_CONVERSATION_IDS_PER_MUTATION)
                .map(|_| json!("conversation-1"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<
                BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
                MAXIMUM_CONVERSATION_IDS_PER_MUTATION,
            >,
        >(ids_over_limit)
        .is_err());

        let tags_at_supported_input_limit = Value::Array(
            (0..MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS)
                .map(|_| json!("tag"))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedVec<BoundedText<128>, MAXIMUM_CONVERSATION_TAG_INPUT_ITEMS>,
        >(tags_at_supported_input_limit)
        .is_ok());
    }

    #[test]
    fn conversation_creation_deserializes_existing_renderer_fields_with_bounds() {
        let request = json!({
            "id": "conversation-1",
            "title": "A new conversation",
            "providerId": "opencode",
            "modelId": "gpt-6-luna",
            "connectionId": null,
            "workspaceId": null,
            "apiKeyConnectionId": null,
            "projectId": null
        });
        let parsed = serde_json::from_value::<CreateConversationInput>(request.clone())
            .expect("the existing renderer conversation shape should deserialize");
        assert_eq!(parsed.provider_id, "opencode");

        let mut oversized_id = request.clone();
        oversized_id["id"] = Value::String("x".repeat(MAXIMUM_SAFE_IDENTIFIER_BYTES + 1));
        assert!(serde_json::from_value::<CreateConversationInput>(oversized_id).is_err());

        let mut oversized_title = request;
        oversized_title["title"] =
            Value::String("x".repeat(MAXIMUM_NEW_CONVERSATION_TITLE_BYTES + 1));
        assert!(serde_json::from_value::<CreateConversationInput>(oversized_title).is_err());
    }

    #[test]
    fn attachment_deserialization_enforces_count_and_cumulative_size_limits() {
        let attachments_at_limit = Value::Array(
            (0..MAXIMUM_ATTACHMENT_FILE_COUNT)
                .map(|_| attachment(""))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedIncomingAttachments<
                MAXIMUM_ATTACHMENT_FILE_COUNT,
                MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
            >,
        >(attachments_at_limit)
        .is_ok());

        let attachments_over_count = Value::Array(
            (0..=MAXIMUM_ATTACHMENT_FILE_COUNT)
                .map(|_| attachment(""))
                .collect(),
        );
        assert!(serde_json::from_value::<
            BoundedIncomingAttachments<
                MAXIMUM_ATTACHMENT_FILE_COUNT,
                MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
            >,
        >(attachments_over_count)
        .is_err());

        let encoded_limit = Value::Array(vec![attachment("123456"), attachment("abcdef")]);
        assert!(serde_json::from_value::<BoundedIncomingAttachments<2, 12>>(encoded_limit).is_ok());
        let encoded_over_limit = Value::Array(vec![attachment("123456"), attachment("abcdefg")]);
        assert!(
            serde_json::from_value::<BoundedIncomingAttachments<2, 12>>(encoded_over_limit)
                .is_err()
        );
    }

    #[test]
    fn save_message_deserializes_the_existing_renderer_attachment_shape() {
        let request = json!({
            "id": "message-1",
            "conversationId": "conversation-1",
            "role": "user",
            "content": "Review this file.",
            "status": "completed",
            "attachments": [attachment("eA==")],
        });
        let message = serde_json::from_value::<SaveMessageInput>(request)
            .expect("the existing renderer message payload should deserialize");

        assert_eq!(message.content, "Review this file.");
        assert_eq!(message.attachments.0.len(), 1);
        assert_eq!(message.attachments.0[0].data_base64, "eA==");

        let oversized_metadata = json!({
            "id": "message-1",
            "conversationId": "conversation-1",
            "role": "assistant",
            "content": "Response",
            "status": "completed",
            "toolActivities": "x".repeat(MAXIMUM_MESSAGE_METADATA_BYTES + 1),
        });
        assert!(serde_json::from_value::<SaveMessageInput>(oversized_metadata).is_err());
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
        migrate_chat_schema, search_saved_outputs_from_connection, stored_conversation_from_row,
        SavedOutputSearchInput, SavedOutputSearchSortOrder, StoredMessage,
        MAXIMUM_HISTORY_SEARCH_EXCERPT_BYTES, MAXIMUM_HISTORY_SEARCH_QUERY_BYTES,
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
            tag: None,
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
                            workspace_id, api_key_connection_id, project_id, tags,
                            is_pinned, is_archived, is_bookmarked, created_at, updated_at
                     FROM conversations
                     ORDER BY is_bookmarked DESC, is_pinned DESC, updated_at DESC, id ASC",
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
        input.tag = Some("work".to_owned());
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
        input.tag = Some("another-tag".to_owned());
        assert!(search_saved_outputs_from_connection(&connection, input)
            .expect("tag filter should complete")
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

#[cfg(test)]
mod settings_and_unified_storage_tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        migrate_chat_schema(&mut connection).expect("migrate schema");
        connection
    }

    #[test]
    fn schema_migration_19_creates_all_unified_tables() {
        let conn = in_memory_db();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(tables.contains(&"app_settings".to_string()));
        assert!(tables.contains(&"personal_memory_facts".to_string()));
        assert!(tables.contains(&"secure_vault".to_string()));
        assert!(tables.contains(&"ephemeral_cache".to_string()));
    }

    #[test]
    fn schema_migration_20_creates_covering_and_sort_indexes() {
        let conn = in_memory_db();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type='index'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(indexes.contains(&"idx_conversations_sort_list".to_string()));
        assert!(indexes.contains(&"idx_messages_conversation_sort".to_string()));
    }

    #[test]
    fn app_settings_crud_works_inside_transaction() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            params!["openchat.theme", "\"dark\"", 1000],
        ).unwrap();

        let val: String = conn.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            ["openchat.theme"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(val, "\"dark\"");

        // Update
        conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params!["openchat.theme", "\"light\"", 2000],
        ).unwrap();
        let val2: String = conn.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            ["openchat.theme"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(val2, "\"light\"");
    }

    #[test]
    fn personal_memory_facts_round_trip() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO personal_memory_facts (id, fact_text, category, source_conversation_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["fact-1", "User prefers concise answers", "preferences", "conv-1", 1000, 1000],
        ).unwrap();

        let fact: String = conn.query_row(
            "SELECT fact_text FROM personal_memory_facts WHERE id = ?1",
            ["fact-1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(fact, "User prefers concise answers");
    }

    #[test]
    fn ephemeral_cache_ttl_filtering() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO ephemeral_cache (key, value_json, expires_at) VALUES (?1, ?2, ?3)",
            params!["cache-key", "{\"pricing\":1}", 5000],
        ).unwrap();

        // Valid when query time < expires_at
        let active: Option<String> = conn.query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params!["cache-key", 4000],
            |row| row.get(0),
        ).optional().unwrap();
        assert_eq!(active, Some("{\"pricing\":1}".to_string()));

        // Expired when query time >= expires_at
        let expired: Option<String> = conn.query_row(
            "SELECT value_json FROM ephemeral_cache WHERE key = ?1 AND expires_at > ?2",
            params!["cache-key", 6000],
            |row| row.get(0),
        ).optional().unwrap();
        assert_eq!(expired, None);
    }
}
