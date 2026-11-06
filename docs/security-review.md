# Security Review

**Last review date:** 2026-11-06
**Status:** In progress. Records checks and fixes completed in the current review slice.

## Audit scope

| Area | Last recorded result | Current review |
|------|---------------------|----------------|
| Rust dependency audit | `cargo-audit` 0.22.2 (RustSec): no advisory findings | Not rerun; `cargo-audit` is unavailable in this environment |
| Frontend dependency audit | `npm audit` (prod + dev): zero vulnerabilities | Reran 2026-11-06: zero vulnerabilities |
| Static analysis | Semgrep 1.181.0 (`p/security-audit`, `p/secrets`): zero findings across 296 files | Not rerun; Semgrep is unavailable in this environment |
| Rust unsafe review | Semgrep `p/rust` + manual: 107 `unsafe` usages, all documented via `clippy::undocumented_unsafe_blocks` | Scoped manual review; not a new exhaustive unsafe scan |

The latest completed Codex Security Standard scan covered eight high-risk surfaces at revision `dca1066` and returned zero validated findings. It was partial and predates the current revision. A later whole-repository scan started at revision `e04ce12` remains incomplete, so it is not treated as evidence of no findings.

The 2026-11-06 manual review used source inspection and repository checks only. It did not invoke Codex Security or a deep security tool.

### Rust static analysis notes

Semgrep `p/rust` reports 168 informational notices: 107 `unsafe` usages, 56 temporary-directory references, 4 `current_exe` references, and 1 `args_os` reference. These are **review prompts**, not confirmed vulnerabilities:

- **Temporary directories** — all in test-only code
- **`args_os`** — service argument parser, validates absolute data path
- **`current_exe`** — Windows notification registration and test subprocesses
- **`unsafe`** — Windows FFI operations with documented safety preconditions

## Trust boundaries

### Renderer → Service

| Limit | Value |
|-------|-------|
| Max inbound request frame | 24 MiB |
| Max outbound response frame | 48 MiB |
| Max concurrent operations | 64 |
| Aggregate request bytes | 64 MiB |
| Max parsed parameter values | 100,000 |
| Request ID length | 256 UTF-8 bytes |
| Method name length | 128 bytes |

Transport is stdio/stdout — no inbound network listener. Complex/capacity rejections drain frames to preserve stream alignment. Conversation and project lists use bounded pages. Message pages stop at a 12 MiB payload target while permitting one larger valid message within the outbound frame ceiling.

### Renderer → Tauri

- Native file dialogs: renderer provides content and proposed filename, **never an absolute path**
- Project folder selection: single-use UUID token, 5-minute expiry, canonical path revalidation
- Tauri capabilities are scoped; no `fs:*` or `shell:*` in renderer window
- Sidecar RPC input is bounded at the Tauri command boundary before it is forwarded
- The sidecar inherits only an allowlist of system, proxy, and certificate environment variables

### Service → Storage

- Broker process: no network capability, bounded FIFO queue (8-second response wait)
- The SQLCipher database lives in the broker's AppContainer profile. The parent service's cache, logs, and attachment files live under `%LOCALAPPDATA%\OpenChat`, outside that profile.
- Parent sidecar does **not** open database files directly

## Current manual review result

Reviewed the renderer-to-Tauri command boundary and capabilities, Tauri-to-sidecar framing and environment, service RPC limits, storage broker and SQLCipher key handling, provider credential storage, attachment encryption and migration, profile archive extraction, outbound URL fetching, OAuth loopback callback validation, and sandbox process setup.

No security vulnerability was validated in those reviewed paths. This is a scoped source review, not an exhaustive review of every unsafe block or every repository file. `npm audit` reported zero vulnerabilities. `cargo-audit` and Semgrep were unavailable, so their current results are unknown.

### Open — Large local storage collections

`security.scans.list`, `storage.chat.favorites.list`, and the all-settings read materialize complete SQLite result sets before the 48 MiB response encoder runs. Workspace-root loading also collects all rows before enforcing its eight-root limit. An unusually large or corrupted local profile can therefore cause avoidable memory pressure even though oversized RPC frames are rejected. These handlers are reachable through the local stdio service, not an inbound network listener. A follow-up should apply query-level and aggregate-byte bounds and return an explicit limit error.

### Open — Attachment-index cancellation/commit race

The attachment-index rebuild has a window between its final cancellation check and the SQL commit. Cancellation in that interval may still allow rebuilt index rows to commit. The affected attachment-index files were left untouched in this review, as requested.

## Findings

### Documented — Credential exposure path

Provider API keys, MCP secrets, and OAuth token pairs are individually DPAPI-protected in the SQLCipher vault. API keys entered in provider settings pass through the renderer and Tauri IPC to the native host for storage. Credential-status commands return configuration state and redacted suffixes, not saved full keys.

### Documented — Retained rollback content

Conversation deletion and history clearing affect the active database. Verified migration backups have no automatic expiry or in-app restore flow. Historical filesystem snapshots may retain plaintext blocks from before SQLCipher migration.

### Resolved — Message attachment blobs are outside database encryption

Incoming and generated conversation attachments under `%LOCALAPPDATA%\OpenChat\attachments` now use AES-256-GCM with a key derived from the profile database key using HKDF-SHA256. The authenticated data binds ciphertext to its relative attachment path. Startup converts existing `.data` files through verified sibling temporary files and atomic replacement; an interrupted conversion resumes from a verified temporary or retries from the preserved original. Unsupported or invalid envelopes fail startup before the service reports ready. Conversation and profile archive flows decrypt only into their existing encrypted archive format and re-encrypt restored attachments for their destination paths.

This finding is limited to the `attachments` tree. Image-library outputs use the separate `storage.image_files_root()/image-library` location under the broker profile and were not converted by this change; assess that storage boundary separately before making an at-rest encryption claim for those files.

### Documented — No vulnerability reporting channel

No verified security-reporting contact or repository disclosure policy is documented.

### Resolved — MCP permission fingerprinting

Project MCP `Allow` rules now use a separate `mcpv1__` namespace with truncated SHA-256 fingerprints. Server fingerprints cover validated configuration; tool fingerprints cover server identity, name, description, and input schema. Changed configuration or tool definitions produce different keys and return to the approval path.

### Resolved — Browser URL inspection SSRF

The `browser.inspect_url` RPC now resolves hostnames once, rejects any non-public destination except explicit `localhost` or `.localhost` names that resolve exclusively to loopback addresses, and pins the validated addresses to the HTTP client. Proxy use and redirects are disabled for inspection. DNS result collection is bounded. The regression tests cover public, loopback, private, and unique-local IPv6 decisions without making external network requests.

### Resolved — Statistics CSV formula injection

Statistics text fields can contain conversation titles, IDs, and provider metadata. CSV quoting alone does not stop spreadsheet formula evaluation. Exported text cells that start with a formula marker after leading whitespace are now prefixed with a text marker; numeric columns remain numeric. The regression test covers formula text, a tab-prefixed formula, and a negative numeric value.

### Resolved — Unbounded local-service response frames

The sidecar previously serialized storage responses without an output frame ceiling. Conversation and project list methods returned every stored row at once, and message pages were bounded by row count only. Because the Tauri shell plugin reads stdout by line, a large valid imported archive could force large allocations in the sidecar, host, and renderer before the UI could display history. Sidecar output is now capped at 48 MiB; conversation and project lists use bounded pages; message and search results use byte budgets; and the sidebar loads additional pages on request. Search reports when large matches are omitted. Response serialization stops at the frame ceiling before allocating an oversized output buffer. A single valid message may still occupy a large frame up to the protocol ceiling.

### Resolved — DPAPI secret-map cross-session coordination

Read-modify-write updates now use a profile-scoped `.lock` file with Windows exclusive file lock (`LockFileEx`). The empty lock file preserves a stable lock target across sessions. Write API requires `SecretMapUpdateLock`, preventing accidental persistence of stale reads.

### Resolved — Hard-link path escapes

Project file tools and metadata readers now open enumerated files through the selected root's directory capability via `cap-std`. Hard links after traversal cannot redirect reads. Reads and writes reject hard-linked files so an in-scope name cannot expose or mutate an inode linked outside the scope.

### Resolved — Authenticode package replacement race

Launch verification opens the installed package with write and delete sharing denied, checks its digest and Authenticode signer through that handle, and retains the handle until process creation succeeds. The Windows regression test `holds_the_verified_package_handle_through_process_creation` confirms that replacing the path fails while verification is held and succeeds after release. Trust data remains cache-only and revocation is not checked; same-user processes are outside the package-isolation boundary.

### Open — Vulnerability reporting

No verified vulnerability-reporting channel is documented. Selection and verification of a reporting route remains an open action item.

## Hardening actions taken

| Change | Impact |
|--------|--------|
| Outbound URL policy | Rejects loopback, private, link-local, multicast, reserved, and cloud-metadata destinations |
| DNS rebinding protection | Validates and pins public DNS results for each redirect |
| Browser inspection SSRF protection | Pins resolved destinations, blocks private network access, disables redirects and proxies, and permits only explicit loopback URLs |
| Statistics CSV export | Neutralizes formula markers in untrusted text cells while preserving numeric columns |
| RPC response framing | Caps sidecar output at 48 MiB and bounds conversation, project, and message list pages |
| Archive-bomb protections | ZIP traversal, size limits (4 GiB per entry, 8 GiB total), symlink rejection |
| SSE event finalization | Processes buffered events at stream end; explicit size limits on compaction state |
| Schema initialization stops | Stays at every requested target version; aligns user-question validation |
| Response API index validation | Rejects malformed tool-call events with invalid or out-of-range output index |
| Tool call limits | Applies per-turn limit to every event that can create a call |
| Task-scoped local environment loading | Only `dev` and `build` load the root `.env`; checks, tests, benchmarks, and maintenance tasks do not inherit those local values |
| Pending archive marker bounds | Requires the exact expected marker length before reading and rejects oversized marker files |
| Message attachment encryption | Profile-key-derived AES-256-GCM, path-bound authentication, and crash-recoverable migration for existing conversation attachment files |
| Tauri sidecar environment | Clears inherited variables and passes only the local-service allowlist |
| Turkish-normalized message search | Registers the SQLite normalization function used by the search query and covers it with a regression test |
