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
    ServiceError::new(
        "storage_chat_unavailable",
        "Local conversation storage is unavailable.",
        true,
    )
    .with_details(sqlite_error_code(&error))
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
