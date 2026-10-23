//! Domain entities, bounded request payloads, and error contracts for local conversation storage.

use serde::{
    de::{self, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::request_bounds::{BoundedText, BoundedVec};

pub const MINIMUM_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
pub const MAX_MESSAGE_CHARS: usize = 1_000_000;
pub const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
pub const MAXIMUM_ATTACHMENT_FILE_COUNT: usize = 10;
pub const MAXIMUM_ATTACHMENT_IMAGE_COUNT: usize = 3;
pub const MAXIMUM_ATTACHMENT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
pub const MAXIMUM_ATTACHMENT_TEXT_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_ATTACHMENT_TOTAL_BYTES: usize = 14 * 1024 * 1024;
pub const MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES: usize =
    (MAXIMUM_ATTACHMENT_TOTAL_BYTES.div_ceil(3) + MAXIMUM_ATTACHMENT_FILE_COUNT) * 4;
pub const MAXIMUM_ATTACHMENT_NAME_BYTES: usize = 255 * 4;
pub const MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES: usize = 64;
pub const MAXIMUM_ATTACHMENT_KIND_BYTES: usize = 5;
pub const MAXIMUM_CONVERSATION_IDS_PER_MUTATION: usize = 500;
pub const MAXIMUM_CONVERSATION_TITLE_CHARS: usize = 512;
pub const MAXIMUM_SAFE_IDENTIFIER_BYTES: usize = 128;
pub const MAXIMUM_MESSAGE_CONTENT_BYTES: usize = MAX_MESSAGE_CHARS * 4;
pub const MAXIMUM_NEW_CONVERSATION_TITLE_BYTES: usize = 500 * 4;
pub const MAXIMUM_CONVERSATION_TITLE_BYTES: usize = MAXIMUM_CONVERSATION_TITLE_CHARS * 4;
pub const MAXIMUM_PROJECT_NAME_BYTES: usize = MAXIMUM_CONVERSATION_TITLE_BYTES;
pub const MAXIMUM_ROUTE_MODEL_ID_BYTES: usize = 256;
pub const MAXIMUM_MODEL_DISPLAY_NAME_BYTES: usize = 256 * 4;
pub const MAXIMUM_MESSAGE_METADATA_BYTES: usize = MAXIMUM_MESSAGE_CONTENT_BYTES;
pub const MAXIMUM_MESSAGE_STATUS_BYTES: usize = 16;
pub const MAXIMUM_MESSAGE_FAILURE_CODE_BYTES: usize = MAXIMUM_SAFE_IDENTIFIER_BYTES;
pub const MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES: usize = 255;
pub const MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES: usize = 128;
pub const MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES: usize = 20 * 1024 * 1024;
pub const MAXIMUM_SETTING_KEY_BYTES: usize = 128;
pub const MAXIMUM_SETTING_VALUE_BYTES: usize = 2 * 1024 * 1024;
pub const MAXIMUM_FACT_ID_BYTES: usize = 128;
pub const MAXIMUM_FACT_TEXT_BYTES: usize = 64 * 1024;
pub const MAXIMUM_FACT_CATEGORY_BYTES: usize = 128;

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
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl ChatStorageError {
    pub(crate) fn unavailable() -> Self {
        Self {
            code: "storage_unavailable",
            message: "Local conversation storage is unavailable.",
        }
    }

    pub(crate) fn invalid_input() -> Self {
        Self {
            code: "invalid_request_params",
            message: "The conversation data is invalid.",
        }
    }

    pub(crate) fn unsupported_sqlite() -> Self {
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
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) title_source: String,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) connection_id: Option<String>,
    pub(crate) workspace_id: Option<String>,
    pub(crate) api_key_connection_id: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) agent: Option<ConversationAgentSnapshot>,
    pub(crate) is_pinned: bool,
    pub(crate) is_archived: bool,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Agent instructions and tool constraints copied into a conversation when it starts.
pub struct ConversationAgentSnapshot {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_assistant_agent_name")]
    pub(crate) name: String,
    #[serde(deserialize_with = "deserialize_assistant_agent_instructions")]
    pub(crate) instructions: String,
    pub(crate) response_style: AssistantAgentResponseStyle,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_assistant_agent_tools"
    )]
    pub(crate) tool_allowlist: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AssistantAgentResponseStyle {
    Concise,
    Balanced,
    Detailed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Project metadata returned to the desktop frontend.
pub struct StoredProject {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) folder_path: String,
    pub(crate) workspace_roots: Vec<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Persisted message content and normalized response metadata.
pub struct StoredMessage {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) created_at: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) tokens_per_second: Option<f64>,
    pub(crate) elapsed_microseconds: Option<i64>,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) citation_sources: String,
    pub(crate) personal_memory_fact_ids: String,
    pub(crate) reasoning_summaries: String,
    pub(crate) tool_activities: String,
    pub(crate) status: String,
    pub(crate) failure_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Model favorite entry with its provider and optional connection scope.
pub struct StoredModelFavorite {
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: String,
    pub(crate) source_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Validated routing and project metadata used when starting a conversation.
pub struct CreateConversationInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_new_conversation_title")]
    pub(crate) title: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    pub(crate) model_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) workspace_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) api_key_connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) project_id: Option<String>,
    #[serde(default)]
    pub(crate) agent: Option<ConversationAgentSnapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// New model route assigned to an existing conversation.
pub struct SetConversationRouteInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) conversation_id: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    pub(crate) model_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) connection_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) workspace_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) api_key_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Inputs for creating a project from a native folder-picker selection.
pub struct CreateProjectInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_project_name")]
    pub(crate) name: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) folder_selection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// User message and response metadata persisted with a conversation.
pub struct SaveMessageInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) conversation_id: String,
    #[serde(deserialize_with = "deserialize_message_discriminator")]
    pub(crate) role: String,
    #[serde(deserialize_with = "deserialize_message_content")]
    pub(crate) content: String,
    pub(crate) created_at: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) tokens_per_second: Option<f64>,
    pub(crate) elapsed_microseconds: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) provider_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_model_id")]
    pub(crate) model_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    pub(crate) citation_sources: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    pub(crate) personal_memory_fact_ids: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    pub(crate) reasoning_summaries: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_message_metadata")]
    pub(crate) tool_activities: Option<String>,
    #[serde(deserialize_with = "deserialize_message_discriminator")]
    pub(crate) status: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_message_failure_code"
    )]
    pub(crate) failure_code: Option<String>,
    #[serde(default)]
    pub(crate) attachments: BoundedIncomingAttachments<
        MAXIMUM_ATTACHMENT_FILE_COUNT,
        MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES,
    >,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// User-selected model favorite state.
pub struct SetModelFavoriteInput {
    #[serde(deserialize_with = "deserialize_safe_identifier")]
    pub(crate) provider_id: String,
    #[serde(deserialize_with = "deserialize_model_id")]
    pub(crate) model_id: String,
    #[serde(deserialize_with = "deserialize_model_display_name")]
    pub(crate) display_name: String,
    pub(crate) is_favorite: bool,
    #[serde(default, deserialize_with = "deserialize_optional_safe_identifier")]
    pub(crate) source_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Encoded attachment supplied with a persisted message; decoded data is size-checked before use.
pub(crate) struct IncomingMessageAttachment {
    #[serde(deserialize_with = "deserialize_attachment_id")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_attachment_name")]
    pub(crate) name: String,
    #[serde(deserialize_with = "deserialize_attachment_mime_type")]
    pub(crate) mime_type: String,
    pub(crate) size_bytes: usize,
    #[serde(deserialize_with = "deserialize_attachment_kind")]
    pub(crate) kind: String,
    #[serde(deserialize_with = "deserialize_attachment_data")]
    pub(crate) data_base64: String,
}

/// Deserializes a text field while enforcing its existing UTF-8 byte bound.
pub(crate) fn deserialize_request_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedText::<MAX_BYTES>::deserialize(deserializer).map(BoundedText::into_inner)
}

/// Deserializes message content within the UTF-8 maximum for the existing character limit.
pub(crate) fn deserialize_message_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MESSAGE_CONTENT_BYTES>(deserializer)
}

/// Deserializes an optional text value while enforcing its UTF-8 byte bound.
pub(crate) fn deserialize_optional_request_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoundedText<MAX_BYTES>>::deserialize(deserializer)
        .map(|value| value.map(BoundedText::into_inner))
}

/// Deserializes conversation and project identifiers within their 128-byte storage limit.
pub(crate) fn deserialize_safe_identifier<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes an optional conversation or project identifier within its 128-byte limit.
pub(crate) fn deserialize_optional_safe_identifier<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes agent names within their UTF-8 and character limits.
pub(crate) fn deserialize_assistant_agent_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let name = deserialize_request_text::<D, { 64 * 4 }>(deserializer)?;
    if name.trim().is_empty() || name.trim() != name || name.chars().count() > 64 {
        return Err(de::Error::custom("invalid assistant agent name"));
    }
    Ok(name)
}

/// Deserializes agent instructions within their persisted 16 KiB limit.
pub(crate) fn deserialize_assistant_agent_instructions<'de, D>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, { 16 * 1024 }>(deserializer)
}

/// Deserializes a nullable list of at most 32 bounded agent tool names.
pub(crate) fn deserialize_optional_assistant_agent_tools<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoundedVec<BoundedText<64>, 32>>::deserialize(deserializer)
        .map(|tools| tools.map(|tools| tools.into_strings()))
}

/// Deserializes route model IDs within their existing 256-byte validation limit.
pub(crate) fn deserialize_model_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ROUTE_MODEL_ID_BYTES>(deserializer)
}

/// Deserializes an optional route model ID within its existing 256-byte validation limit.
pub(crate) fn deserialize_optional_model_id<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_ROUTE_MODEL_ID_BYTES>(deserializer)
}

/// Deserializes a new conversation title within the existing 500-character limit.
pub(crate) fn deserialize_new_conversation_title<'de, D>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_NEW_CONVERSATION_TITLE_BYTES>(deserializer)
}

/// Deserializes project names within the existing 512-character validation limit.
pub(crate) fn deserialize_project_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_PROJECT_NAME_BYTES>(deserializer)
}

/// Deserializes model display names within the existing 256-character limit.
pub(crate) fn deserialize_model_display_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MODEL_DISPLAY_NAME_BYTES>(deserializer)
}

/// Deserializes message role and status values within their small fixed-value bound.
pub(crate) fn deserialize_message_discriminator<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_MESSAGE_STATUS_BYTES>(deserializer)
}

/// Deserializes optional message metadata within the 4 MiB per-field persistence limit.
pub(crate) fn deserialize_optional_message_metadata<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_MESSAGE_METADATA_BYTES>(deserializer)
}

/// Deserializes optional message failure codes within the safe identifier byte limit.
pub(crate) fn deserialize_optional_message_failure_code<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_request_text::<D, MAXIMUM_MESSAGE_FAILURE_CODE_BYTES>(deserializer)
}

/// Deserializes an attachment identifier within the persisted identifier bound.
pub(crate) fn deserialize_attachment_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_SAFE_IDENTIFIER_BYTES>(deserializer)
}

/// Deserializes an attachment name within the existing character limit's UTF-8 maximum.
pub(crate) fn deserialize_attachment_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_NAME_BYTES>(deserializer)
}

/// Deserializes an attachment MIME type within the accepted value set's byte bound.
pub(crate) fn deserialize_attachment_mime_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES>(deserializer)
}

/// Deserializes an attachment kind within the accepted `image` and `text` values.
pub(crate) fn deserialize_attachment_kind<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_KIND_BYTES>(deserializer)
}

/// Deserializes attachment data within the maximum cumulative encoded payload size.
pub(crate) fn deserialize_attachment_data<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_request_text::<D, MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES>(deserializer)
}

/// Incoming attachments bounded by item count and cumulative encoded content size.
#[derive(Debug, Default)]
pub(crate) struct BoundedIncomingAttachments<const MAX_ITEMS: usize, const MAX_BASE64_BYTES: usize>(
    pub(crate) Vec<IncomingMessageAttachment>,
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
pub(crate) struct StoredAttachmentMetadata {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: usize,
    pub(crate) kind: String,
}

#[derive(Debug, Deserialize)]
/// Database envelope used to separate message text from attachment metadata.
pub(crate) struct StoredMessageContentEnvelope {
    #[serde(rename = "content")]
    pub(crate) _content: String,
    pub(crate) attachments: Vec<StoredAttachmentMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// One saved conversation attachment and the conversation where it was added.
pub struct ConversationAttachmentSummary {
    pub(crate) conversation_id: String,
    pub(crate) conversation_title: String,
    pub(crate) project_id: Option<String>,
    pub(crate) message_id: String,
    pub(crate) attachment_id: String,
    pub(crate) file_name: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: usize,
    pub(crate) created_at: i64,
    pub(crate) is_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A bounded page of saved conversation attachments.
pub struct ConversationAttachmentPage {
    pub(crate) items: Vec<ConversationAttachmentSummary>,
    pub(crate) next_offset: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// State and storage use for the optional local attachment text index.
pub struct AttachmentTextIndexStatus {
    pub(crate) enabled: bool,
    pub(crate) indexed_file_count: u32,
    pub(crate) indexed_bytes: u64,
    pub(crate) truncated: bool,
}

/// Decoded attachment bytes and metadata after all request and content bounds pass.
pub(crate) struct ValidatedAttachment {
    pub(crate) metadata: StoredAttachmentMetadata,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Database location and schema version after a successful integrity check and preparation.
pub struct PreparedDatabase {
    pub(crate) database_path: String,
    pub(crate) schema_version: i64,
}
