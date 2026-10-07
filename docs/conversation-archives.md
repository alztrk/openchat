# Conversation archives

OpenChat can export selected conversations and restore them into the current local profile. The archive does not contain the entire profile or a copy of the SQLite database.

Conversations can also be archived from the chat list. Archiving removes a conversation from the active sidebar and keeps its messages and attachments on the device. Open the Archived section to continue a conversation or restore it to the active list. Archiving clears its pinned state. A conversation with an active response must be stopped before it can be archived. Permanent deletion remains available as a separate action.

## Included data

An archive contains the selected conversation titles and timestamps, provider and model labels, message text, reasoning summaries, tool inputs and outputs, attachment bytes, and per-conversation memory-index settings. The archive omits provider credentials, linked account and workspace identifiers, project links, and global application settings. After restore, select an available provider and model again before continuing the conversation.

Chat messages, tool inputs and outputs, and attachments can contain sensitive text or local filesystem paths. Review the selected conversations before exporting, and store the archive and passphrase separately.

## Encryption and recovery

The `.openchatbackup` file is an age-encrypted v1 stream containing a versioned OpenChat tar archive. The manifest includes attachment sizes and SHA-256 digests. Restore verifies the encrypted stream, archive structure, manifest, content digests, and attachment metadata before it changes the conversation database.

The passphrase must contain at least 12 Unicode characters and no more than 512 UTF-8 bytes. OpenChat does not save or recover it. If it is lost, the archive cannot be restored. Do not save the passphrase beside the archive.

The Rust `age` crate currently labels its API beta in its crate documentation. OpenChat pins the crate version and uses the age v1 file format; the dependency and interoperability should be reviewed again before the first public release.

## Restore behavior and limits

Inspect decrypts and validates the complete archive before showing the conversation, message, attachment, duplicate, and creation-date counts. Restore can skip conversations whose IDs already exist or import them as copies with new conversation IDs. New conversation records do not reconnect accounts, workspaces, or projects. New archive files preserve each conversation's archived state; older files without that field restore conversations to the active list.

Conversation rows and messages are written in one SQLite transaction. Attachments use exclusive file creation; if any copy or database write fails, the transaction rolls back and files created by that restore attempt are removed. Existing files are not overwritten. Restore uses the current application schema; it does not migrate an archive from a future format or replace the profile database.

The current limits are 5,000 conversations, 250,000 messages, 4 GiB of attachments, and an 8 GiB encrypted archive. Inspect and restore can take several minutes for large archives. Keep the app open while an operation runs. If the app reports that an operation timed out, check the conversation list before retrying.

Archive format v1 is implemented for selected-conversation transfer. Full-profile database-and-attachment backup is documented separately in [profile archives](profile-archives.md). Global preferences outside SQLite, schema-upgrading restore, and JSON/Markdown interchange remain separate roadmap work.
