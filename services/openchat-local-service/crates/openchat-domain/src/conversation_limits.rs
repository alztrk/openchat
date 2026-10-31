//! Shared attachment limits for renderer, desktop host, and local-service boundaries.

/// Maximum number of attachments accepted in one message.
pub const MAXIMUM_ATTACHMENT_FILE_COUNT: usize = 10;
/// Maximum number of images accepted in one message.
pub const MAXIMUM_ATTACHMENT_IMAGE_COUNT: usize = 3;
/// Maximum encoded image size in bytes.
pub const MAXIMUM_ATTACHMENT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// Maximum text attachment size in bytes.
pub const MAXIMUM_ATTACHMENT_TEXT_BYTES: usize = 1024 * 1024;
/// Maximum combined decoded attachment size in bytes.
pub const MAXIMUM_ATTACHMENT_TOTAL_BYTES: usize = 14 * 1024 * 1024;
/// Maximum combined base64 attachment size accepted during Tauri deserialization.
pub const MAXIMUM_ATTACHMENT_TOTAL_BASE64_BYTES: usize =
    (MAXIMUM_ATTACHMENT_TOTAL_BYTES.div_ceil(3) + MAXIMUM_ATTACHMENT_FILE_COUNT) * 4;
/// Maximum number of Unicode characters in an attachment name.
pub const MAXIMUM_ATTACHMENT_NAME_CHARS: usize = 255;
/// Maximum UTF-8 bytes for an attachment name with the supported character limit.
pub const MAXIMUM_ATTACHMENT_NAME_BYTES: usize = MAXIMUM_ATTACHMENT_NAME_CHARS * 4;
/// Maximum UTF-8 bytes for an attachment MIME type.
pub const MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES: usize = 128;
/// Maximum UTF-8 bytes for an attachment kind value.
pub const MAXIMUM_ATTACHMENT_KIND_BYTES: usize = 5;
/// Prefix that separates persisted message text from attachment metadata.
pub const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
