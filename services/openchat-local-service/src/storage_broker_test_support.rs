//! Helpers for testing encrypted profile restore through the Windows storage broker.

use std::{io, path::Path};

use zeroize::Zeroizing;

/// Creates a passphrase-encrypted profile archive containing one conversation and attachment.
///
/// The source profile must be a disposable test directory, and the archive path must not exist.
/// This API is available only when `storage-broker-test-support` is enabled.
///
/// # Errors
/// Returns an error when the fixture cannot be created or the archive cannot be written safely.
pub fn create_profile_archive(
    source_root: &Path,
    archive_path: &Path,
    conversation_id: &str,
    passphrase: Zeroizing<String>,
) -> io::Result<()> {
    crate::profile_archive::create_broker_test_archive(
        source_root,
        archive_path,
        conversation_id,
        passphrase,
    )
}

/// Prepares an encrypted profile archive for activation by the next broker startup.
///
/// The destination root must belong to a disposable storage-broker test profile. The supplied
/// database key is copied into zeroizing storage for database operations.
///
/// # Errors
/// Returns an error when the destination cannot be opened or the archive fails validation.
pub fn stage_profile_restore(
    destination_root: &Path,
    archive_path: &Path,
    database_key: &[u8; 32],
    passphrase: Zeroizing<String>,
) -> io::Result<()> {
    crate::profile_archive::stage_broker_test_restore(
        destination_root,
        archive_path,
        database_key,
        passphrase,
    )
}
