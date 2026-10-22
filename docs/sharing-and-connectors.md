# Sharing and connectors

OpenChat keeps conversation data on the user's device. Sharing uses files the
user creates and transfers; the app does not create hosted conversation links,
sync archives to an OpenChat service, or provide live multi-user collaboration.

## Conversation and profile archives

The conversation archive lets a user select conversations, review the export
summary, and create an encrypted `.openchatbackup` file. Its versioned archive
contains the selected conversations and their saved attachments. OpenChat uses
the age scrypt passphrase format; the passphrase is supplied for each operation,
must contain at least 12 characters, and is not saved. The archive is validated
before restore, including its paths, entry types, declared sizes, attachment
hashes, and format version. Restore can skip existing conversations or import
them as copies.

The separate `.openchatprofilebackup` format is a full-profile backup containing
the local database and attachments. It is also encrypted with an age scrypt
passphrase of at least 12 characters. It excludes provider credentials and
global preferences stored outside the database. Restore is staged and checked
before activation; OpenChat retains recovery data so an interrupted or
unconfirmed restore can be rolled back on startup.

Markdown, HTML, and JSON conversation exports are readable exports. They are
not encrypted, and users should review their destination and contents before
sharing them.

## Signed connector metadata sources

The connector catalog is local to the app. A user may pin up to ten HTTPS
sources, each with an Ed25519 public key. Refresh is manual. OpenChat accepts a
bounded JSON response whose `payload` field is a JSON string and whose
`signature` field is unpadded base64url. The signature is checked over the
exact UTF-8 bytes of `payload`. The version 1 payload contains a positive,
monotonic revision and up to 50 connector entries; the full response is capped
at 64 KiB. URLs cannot contain credentials, a query, or a fragment. Fetching
uses the local service's public-address checks and does not permit an HTTPS to
HTTP redirect downgrade.

The source key fingerprint is displayed after refresh. A valid signature shows
that the catalog was signed by the pinned key; it does not prove who controls
that key or whether the publisher is legitimate. Users must compare the
fingerprint through a separate trusted channel before trusting a source.
OpenChat retains the last accepted revision and payload hash, rejects rollback,
and rejects different content at the same revision.

Importing an entry saves a local template. Adding a template to a project creates
a disabled MCP server and sets its tools to `Ask` by default. Connecting it
requires the user to enable and configure it in that project. Project-scoped
credentials remain in the platform credential store. Removing an app-wide
template does not remove a copy already added to a project; remove that project
server separately.

Signed catalogs may also describe a version-pinned Windows stdio executable.
The app downloads it only after the user confirms installation. The local
service fetches the signed catalog again, verifies the pinned Ed25519 key,
requires HTTPS without redirects, bounds the file to 128 MiB, and checks its
declared size, SHA-256, and PE executable format before saving it under the
project's local app-data package directory. A project reference records the
version, digest, size, and key fingerprint rather than an absolute machine path.
Before launch, the service checks the file again and grants its directory
read-only access in the Windows AppContainer. The MCP server remains disabled
after installation and its tools require approval by default. An update needs
another user confirmation, installs a separate version, resets that connector's
tool rules to `Ask`, then removes the previous package. Removal failures stay
visible and can be retried; cleanup never recursively deletes an unexpected
directory.

A valid signature proves that the catalog was signed by the pinned key. It does
not establish the publisher's real-world identity. The confirmation shows the
catalog key fingerprint, package URL, size, digest, requested access, and data
disclosure. Users should verify the fingerprint through a separate trusted
channel before installing. No centrally hosted or first-party public catalog is
provided by OpenChat. Windows Authenticode publisher identity, a first-party
catalog, and release/update signing remain separate release work.
