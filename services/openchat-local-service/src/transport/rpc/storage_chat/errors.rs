//! Storage error constructors.

use super::*;

pub(super) fn unix_time_millis() -> Result<i64, ServiceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| storage_unavailable())?
        .as_millis();
    i64::try_from(millis).map_err(|_| storage_unavailable())
}

pub(super) fn storage_error(error: rusqlite::Error) -> ServiceError {
    let detail_code = sqlite_error_code(&error);
    ServiceError::new(
        "storage_chat_unavailable",
        "Local conversation storage is unavailable.",
        is_retryable_storage_error_code(detail_code),
    )
    .with_details(detail_code)
}

pub(super) fn storage_unavailable() -> ServiceError {
    ServiceError::new(
        "storage_chat_unavailable",
        "Local conversation storage is unavailable.",
        true,
    )
}

pub(super) fn not_found(resource: &str) -> ServiceError {
    ServiceError::new(
        "storage_chat_not_found",
        format!("The requested {resource} was not found."),
        false,
    )
}
