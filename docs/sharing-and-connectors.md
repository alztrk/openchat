# Sharing and connector boundaries

OpenChat's current product boundary is local-first. Conversation content stays
in the local profile unless the user sends it to a selected provider or
deliberately exports it. There is no hosted conversation service, public-link
service, or multi-user collaboration service in the current source tree.

This document describes existing transfer and connector paths, then sets the
security conditions for the selected application-wide connector catalog.
Alican selected local, encrypted file transfer for conversation sharing and an
application-wide catalog that discovers and installs connectors locally. The
catalog decision does not authorize hosted connector execution, a hosted
conversation service, or any change to what data OpenChat sends.

## Existing transfer paths

| Action | Data leaving the local profile | Protection and credentials | Revocation and deletion |
| --- | --- | --- | --- |
| Encrypted conversation archive | User-selected conversations, message and tool content, reasoning summaries, and referenced attachments | Age-encrypted archive; user supplies a passphrase that OpenChat does not store; provider credentials and linked account identifiers are excluded | OpenChat can delete its local archive file if the user chooses to. A copy already moved or shared cannot be recalled or revoked. |
| Encrypted profile backup | Local SQLite profile data and referenced attachments, subject to the exclusions in the profile-backup contract | Age-encrypted archive; user supplies an unstored passphrase; provider credentials, global preferences outside SQLite, model files, and runtime binaries are excluded | Same file-level limit: deleting the local file does not delete copies made elsewhere. |
| Markdown conversation export | A readable conversation document; it can include tool activity and sensitive text or local paths | Not encrypted by OpenChat; the destination and any encryption are controlled by the user and operating system | Deleting the local export does not recall copies. |
| Saved-response export | The selected saved-response version as a Markdown file | Before choosing a destination, OpenChat previews the exact version and states that the source conversation is excluded. The response may contain sensitive information. The file is not encrypted by OpenChat. | Deleting the local export does not recall copies. |

Conversation and profile archive formats, included data, restore validation,
and recovery behavior are specified in [conversation archives](conversation-archives.md)
and [profile archives](profile-archives.md). Conversation text, tool activity,
attachments, and local paths can be sensitive. Before any archive export, the
UI must show which content is included and where the resulting file will be
written. Encrypted archive transfer is the sharing path. Readable Markdown
exports remain separate user-directed exports and are not encrypted by OpenChat.

### Selected-response snapshot

Implemented as the saved-response Markdown export. Before writing it, the user
reviews the exact selected version and its scope, then chooses a destination.
The export excludes the source conversation, tool activity, attachments,
credentials, and OAuth tokens. The UI warns that the response may contain
sensitive information and that deleting the local file cannot recall copies
made elsewhere.

## Existing connector paths

The current connector-like configuration is project-scoped rather than a
global marketplace:

- Project MCP servers are declared in `.openchat/mcp.json`. A server must be
  explicitly enabled. Opening its configuration does not start it; the
  connection check starts only the selected server. During chat, enabled
  servers connect only for tool-capable routes. The discovered tool definitions
  and results are part of the provider request flow.
- Local stdio servers use the Windows sandboxed child process. Streamable HTTP
  servers require HTTPS and a public network destination. Private, loopback,
  link-local, reserved, and metadata addresses are rejected after DNS
  resolution, DNS results are pinned, and redirects are disabled.
- Connector secrets are named in the project catalog but stored separately in
  Windows Credential Manager, scoped to project, server, and variable. HTTP
  credentials are sent as bearer tokens over HTTPS. Secrets must not be placed
  in arguments, query strings, or the project catalog.
- MCP permissions are explicit project-local rules. `Ask` is the default;
  server denial prevents the server from starting. Tool results are treated as
  untrusted input. Disable or remove a server and remove its saved credentials
  to stop future use. Previously saved messages and tool activity are not
  erased by disabling a connector.
- First-party project tools and named tasks use project-local catalogs and
  existing permission checks. They are not a downloadable connector
  marketplace.

The implementation contract and limits are maintained in [tools](tools.md).
The current application-wide catalog stores user-defined MCP templates in
local app preferences. A template records the user's publisher and source
claims, declared version, requested access, destination, and data disclosure;
the UI explicitly labels those claims unverified. Templates contain connection
configuration only and do not download server software. Installing a template
copies its MCP configuration to the active project with the server disabled
and its tool permission set to `Ask`. Project-specific configuration,
permissions, and credentials remain scoped to that project. Deleting a template
does not remove already installed project servers.

The app-wide catalog supports user-managed local templates. Installing a
template adds a disabled project server and sets its tool permission to `Ask`.
It does not grant credentials or connect the server. Catalog entries and their
publisher claims are user-provided and are not verified by OpenChat.

## Required contract for the application-wide connector catalog

Each distributed catalog entry must provide machine-readable capabilities and
a localized human-readable explanation. OpenChat must enforce the declared
access at the local service boundary; hiding a permission in the UI is not
enforcement. Current user-defined templates carry disclosures for review, but
those descriptions are not enforced access declarations.

- **Default off:** installing or listing an entry does not enable it, connect
  it, grant credentials, or send conversation content.
- **Least privilege:** request only the file, network, or account scopes needed
  for the selected operation. Do not grant access to all conversations or the
  whole filesystem by default.
- **Before enabling:** show publisher identity and verification state, exact
  access scopes, destination, data categories sent, authentication method,
  whether the provider receives tool results, and how to disable and remove
  credentials.
- **At invocation:** show the action and target that require approval. Send
  only the selected operation's necessary data; do not add conversation
  history or attachments implicitly. Keep the provider, connector, and local
  execution permission boundaries distinct.
- **Credentials:** keep secrets in platform secure storage, scoped to the
  connector and account. Never include them in catalogs, prompts, logs,
  exports, or diagnostic bundles. Removing a connector must offer credential
  deletion and report if cleanup fails.
- **Updates and removal:** verify catalog and package provenance, validate
  versions and declared scopes, and require renewed consent if an update adds
  access. Disablement must prevent future calls immediately. Removal must
  delete local connector data and credentials where possible; it must explain
  that data already sent to an external service is governed by that service.
- **Catalog availability:** catalog manifests and packages are obtained and
  installed locally. A catalog outage must not disable already installed
  connectors or cause OpenChat to send user data elsewhere. Do not add a
  centralized OpenChat account or connector execution service as part of this
  feature.
- **Untrusted output:** connector descriptions, returned text, and files are
  untrusted data. They cannot grant themselves permissions or override the
  user's request. Bound startup, request, response, and resource use; support
  cancellation and expose failures without leaking secrets.

## Sharing product decision

Alican selected local, encrypted file transfer. Public or invite-based links,
hosted conversation storage, and multi-user collaboration are outside the
current product scope. Do not implement them as ordinary local exports. The
separate application-wide connector catalog is approved for local discovery
and installation; this does not authorize hosted connector execution.

Revisiting the sharing decision requires a new explicit product approval and a
design for hosting, authentication, content scope, expiration, revocation,
deletion, abuse reporting, and incident response. Revoking a future server
link could stop later server access, but could not erase a recipient's
downloaded copy.

## Release decision

The currently supportable behavior is encrypted, deliberate file transfer,
project-scoped connector configuration, and an application-wide local template
catalog. Hosted links and multi-user collaboration are excluded by product
decision. Independent publisher verification, executable package
distribution, project-server updates from templates, and catalog-to-project
removal remain open under the provenance, privacy, permission, deletion, and
update contracts above.
