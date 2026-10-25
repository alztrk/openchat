//! Compatibility re-exports for the service-owned chat schema migration.

#[cfg(test)]
pub use openchat_local_service::migrate_chat_schema;
pub use openchat_local_service::CHAT_SCHEMA_VERSION;
