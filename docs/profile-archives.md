# Encrypted profile backups

OpenChat can create a passphrase-protected backup of the local SQLite profile database and attachment files referenced by messages. The database is captured through SQLite's online backup API, which includes committed WAL data. Attachment files are copied into the archive only when the database snapshot references them.

## Included and excluded data

The database contains local conversations and application records stored in SQLite, including provider catalog and connection metadata. API keys and OAuth tokens remain in the operating system's secure credential store and are never read into the archive. Global preferences stored outside SQLite, model files, and runtime binaries are also excluded. When restoring on another computer, reconnect provider accounts and register local models again as needed.

Conversation and tool content, attachment text, and local file paths may be sensitive. Store the backup and its passphrase separately.

## Format and limits

The .openchatprofilebackup file is an age-encrypted v1 stream containing a versioned tar archive. The manifest records the database schema versions, conversation/message counts, attachment paths and sizes, and SHA-256 digests. Restore validates the encrypted stream, archive paths and entry types, manifest, database integrity and foreign keys, schema compatibility, message attachment metadata, file sizes, and content digests before it schedules any replacement.

The passphrase must have at least 12 Unicode characters and no more than 512 UTF-8 bytes. OpenChat does not store or recover it. A lost passphrase makes that backup unreadable.

Current limits are a 64 GiB decrypted archive, a 32 GiB database, 24 GiB total attachment data, 100,000 attachments, 100,000 conversations, and 2,000,000 messages. Export and validation stream database and archive files; attachment type validation uses each attachment's existing size limit.

## Restore and recovery

Restore requires explicit confirmation. Preparation decrypts and validates the complete backup into a private staging directory while leaving the active profile untouched. OpenChat then requires a full close. On the next launch, the local service moves the existing db and attachments directories to %LOCALAPPDATA%/OpenChat/backups/profile-restores, activates the staged data, and lets the application validate and initialize it before marking the restore complete.

The replaced profile is retained in that recovery folder. If database opening, migration, or startup validation fails before completion, the next service launch restores the previous database and attachments automatically. Do not delete or move the recovery folder until the restored profile has been checked.

Profile backup format v1 does not include preferences outside SQLite, provider credentials, model files, runtime packages, JSON/ZIP interchange, or automatic schema upgrading from a newer OpenChat build.
