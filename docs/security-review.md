# Security Review

**Last review date:** 2026-11-05
**Status:** In progress. Records checks and fixes completed in the current review slice.

## Audit scope

| Area | Tool | Result |
|------|------|--------|
| Rust dependency audit | `cargo-audit` 0.22.2 (RustSec) | No advisory findings |
| Frontend dependency audit | `npm audit` (prod + dev) | Zero vulnerabilities |
| Static analysis | Semgrep 1.181.0 (`p/security-audit`, `p/secrets`) | Zero findings across 296 files |
| Rust unsafe review | Semgrep `p/rust` + manual | 107 `unsafe` usages — all documented via `clippy::undocumented_unsafe_blocks` |

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
| Max frame size | 24 MiB |
| Max concurrent operations | 64 |
| Aggregate request bytes | 64 MiB |
| Max parsed parameter values | 100,000 |
| Request ID length | 256 UTF-8 bytes |
| Method name length | 128 bytes |

Transport is stdio/stdout — no inbound network listener. Complex/capacity rejections drain frames to preserve stream alignment.

### Renderer → Tauri

- Native file dialogs: renderer provides content and proposed filename, **never an absolute path**
- Project folder selection: single-use UUID token, 5-minute expiry, canonical path revalidation
- Tauri capabilities are scoped; no `fs:*` or `shell:*` in renderer window

### Service → Storage

- Broker process: no network capability, bounded FIFO queue (8-second response wait)
- Database and cache live in AppContainer's private local-data directory
- Parent sidecar does **not** open database files directly

## Findings

### Documented — Credential exposure path

Provider API keys, MCP secrets, and OAuth token pairs are individually DPAPI-protected in the SQLCipher vault. API keys entered in provider settings pass through the renderer and Tauri IPC to the native host for storage. Credential-status commands return configuration state and redacted suffixes, not saved full keys.

### Documented — Retained rollback content

Conversation deletion and history clearing affect the active database. Verified migration backups have no automatic expiry or in-app restore flow. Historical filesystem snapshots may retain plaintext blocks from before SQLCipher migration.

### Documented — No vulnerability reporting channel

No verified security-reporting contact or repository disclosure policy is documented.

### Resolved — MCP permission fingerprinting

Project MCP `Allow` rules now use a separate `mcpv1__` namespace with truncated SHA-256 fingerprints. Server fingerprints cover validated configuration; tool fingerprints cover server identity, name, description, and input schema. Changed configuration or tool definitions produce different keys and return to the approval path.

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
| Archive-bomb protections | ZIP traversal, size limits (4 GiB per entry, 8 GiB total), symlink rejection |
| SSE event finalization | Processes buffered events at stream end; explicit size limits on compaction state |
| Schema initialization stops | Stays at every requested target version; aligns user-question validation |
| Response API index validation | Rejects malformed tool-call events with invalid or out-of-range output index |
| Tool call limits | Applies per-turn limit to every event that can create a call |
