# Sharing and Connectors

OpenChat keeps conversation data on the user's device. Sharing uses files the user creates and transfers; the app does not create hosted conversation links, sync archives to an OpenChat service, or provide live multi-user collaboration.

## Conversation archives

| Format | Contents | Encryption |
|--------|----------|------------|
| `.openchatbackup` | Selected conversations + attachments | age v1 scrypt (passphrase) |
| `.openchatprofilebackup` | Full database + attachments | age v1 scrypt (passphrase) |
| Markdown / HTML / JSON | Readable text exports | None |

### Selected-conversation archive

- Version 1 format; selective export of conversations and their attachments
- Passphrase: minimum 12 characters, never stored
- Validation before restore: paths, entry types, declared sizes, attachment hashes, format version
- Restore options: skip existing conversations, or import as copies

### Full-profile archive

- Contains local database (SQLCipher) and attachments
- Excludes: provider credentials, global preferences outside the database, external model/runtime binaries
- Staged restore: validated before activation, rollback preserved on failure
- Windows: stages and swaps files inside the broker's private profile

### Markdown export

- Readable exports, not encrypted
- Users should review destination and contents before sharing

## Connector catalogs

OpenChat supports installing and managing MCP server connector packages from user-pinned catalogs.

### Catalog format

| Field | Limit |
|-------|-------|
| Max sources per user | 10 |
| Verification | Ed25519 public key per source |
| Refresh | Manual |
| Max response size | 64 KiB |
| Max entries per refresh | 50 |

The response `payload` field contains a JSON string; the `signature` field is unpadded base64url. Signature is verified over the exact UTF-8 bytes of the payload.

### Version 1 manifest

Optional manifest declared by package publishers:

- Declared tools, permissions, providers, and configuration requirements
- Displayed **before** installation — users see what the package requests
- Runtime-discovered tools remain authoritative; manifest claims do not affect routing or permissions
- Connection check compares declared tool names with runtime-discovered names; differences are shown to the user

### Package installation

| Check | Details |
|-------|---------|
| Authenticode | Required on Windows; cached trust data, no revocation check |
| Signer fingerprint | Pinned from catalog; verified before download and launch |
| Digest | SHA-256 verified before and after download |
| Size | 128 MiB limit on PE structure |
| Trust | Binds to a signed catalog Authenticode signer; does not verify real-world identity |

Packages are added **disabled** by default. Their tools default to `Ask` permission mode.

### Lifecycle

- **Installation:** URL, size, digest, and signer fingerprint shown for confirmation
- **Launch:** Digest, PE structure, Authenticode trust, and signer fingerprint checked again
- **Updates:** Require explicit confirmation; reset rules to `Ask`
- **Removal:** Removes project reference and scoped credentials; visible retry on failure
- **Legacy packages:** Unsigned packages no longer launch but remain removable
