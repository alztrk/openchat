# OpenChat architecture integration plan

## Goal

Move OpenChat from repository-root package folders to explicit application and
service boundaries, then place code by runtime responsibility. Keep user-visible
behavior, RPC method names and payloads, database schemas, and data locations
stable during the reorganization.

## Target structure

```text
Cargo.toml               # workspace manifest and shared build profiles
Cargo.lock               # single Rust workspace lockfile
rust-toolchain.toml
docs/
scripts/
apps/
  desktop/
    assets/
    frontend/
      src/
        bootstrap/       # React entry point and startup wiring
        shell/           # Window chrome, navigation, and top-level layout
        modules/
          conversations/
          projects/
          providers/
          model-library/
          settings/
          notifications/
        platform/
          tauri/         # Native commands and plugin access
          local-service/ # Sidecar RPC client and lifecycle
        design-system/   # Shared controls and visual primitives
        localization/
    tauri/                # Tauri host, capabilities, icons, and bundle config
services/
  openchat-local-service/ # Unified local service sidecar
    src/
      bootstrap/
      transport/
      rpc/
      modules/
      adapters/
      secrets/            # Protected credential and DPAPI secret store
      sandbox/            # Cross-platform sandbox (Windows AppContainer, Linux/macOS roadmap)
third_party_licenses/
```

The frontend modules own product workflows and their UI state. Platform modules
own Tauri and sidecar I/O. Product modules may depend on platform interfaces;
platform code must not import product stores. The Tauri host and local service
remain separate Rust processes with their existing storage and RPC boundaries.

## Current implementation status

- Package roots and shared desktop assets are under `apps/desktop/`; the local
  service is under `services/openchat-local-service/`.
- A repository-root Cargo workspace unifies the service and Tauri host under one
  lockfile, one target directory, and shared build profiles.
- React startup is under `apps/desktop/frontend/src/bootstrap/`, the top-level
  window UI is under `apps/desktop/frontend/src/shell/`, and translations are
  under `apps/desktop/frontend/src/localization/`.
- Conversation, project, model-library, settings, and notification workflows
  are grouped under `apps/desktop/frontend/src/modules/`. Tauri APIs and dialog
  access pass through `apps/desktop/frontend/src/platform/tauri/`, and the
  sidecar client and its singleton live under
  `apps/desktop/frontend/src/platform/local-service/`.
- Shared UI primitives now live under
  `apps/desktop/frontend/src/design-system/`; shared hooks, services, and
  utilities remain under `apps/desktop/frontend/src/shared/`.
- Provider routes, model types and catalogs, pricing, credential handling,
  verification, and model preferences now live under
  `apps/desktop/frontend/src/modules/providers/`. Provider setup UI is owned by
  that module; a small settings adapter supplies conversation model state so
  the provider module does not depend on the conversation store.
- Rust service startup, transport, domain files, and storage/credential access
  are grouped under `bootstrap/`, `transport/`, `modules/`, and `adapters/`.
- Project instructions, skills, indexing, MCP settings and credentials,
  worktrees, and named-task operations are dispatched by
  `services/openchat-local-service/src/transport/rpc/projects.rs`. The transport
  dispatcher routes the `project.*` namespace to this domain module.
- Local engine and model catalog routes, including Hugging Face model search
  and downloads, are dispatched by
  `services/openchat-local-service/src/transport/rpc/local_engine_models.rs`. The
  temporary conversation provider routing and bounded transcript parsing are
  dispatched by
  `services/openchat-local-service/src/transport/rpc/temporary_conversation.rs`.
  The transport dispatcher still owns system initialization and durable chat
  run orchestration; those flows stay centralized because they coordinate
  shared startup and cancellation state.
- Conversation file-change history, diffs, reverts, and cleanup routes are
  dispatched by `services/openchat-local-service/src/transport/rpc/file_changes.rs`;
  the module preserves the existing validation and stable error contract.
- Usage statistics and model-pricing routes are dispatched by
  `services/openchat-local-service/src/transport/rpc/statistics.rs`. Date ranges,
  filters, pagination, and batch size are validated at the RPC boundary.
- ChatGPT and Copilot account/authentication routes, plus OpenCode and
  OpenAI-compatible model discovery, are dispatched by
  `services/openchat-local-service/src/transport/rpc/providers.rs`. Provider secrets
  continue to resolve from saved credential references inside the service. Copilot
  connections are keyed by GitHub user ID, keep per-account credentials in Windows
  Credential Manager, and route requests through the explicitly selected account.
- Conversation memory, history search, context measurement, and archive routes
  are dispatched by
  `services/openchat-local-service/src/transport/rpc/conversations.rs`. Archive
  passphrases are removed from mutable request parameters and held in
  zeroizing values before file operations begin.
- Run listing, goal pause/stop, pending question responses, and tool approval
  responses are dispatched by
  `services/openchat-local-service/src/transport/rpc/run_control.rs`. Durable
  `ask_user` questions support single-choice, multi-choice, and free-text
  answers; multi-choice answers are stored as ordered option ID lists.
  `chat.send` request parsing and validation are handled by
  `services/openchat-local-service/src/transport/rpc/chat_request.rs`; project Skill
  selections are capped before their IDs are copied, and title credentials are
  resolved only after the request passes validation. New runs validate bounded
  renderer-supplied instructions before creating persistent run state and save
  the run with its initial checkpoint atomically; resumed runs continue to
  validate the saved checkpoint. Goal validation, project context preparation,
  checkpoint restoration, atomic run creation, saved route credential
  resolution, and run finalization are isolated in
  `services/openchat-local-service/src/transport/rpc/chat_execution.rs`. On
  Windows, the application sidecar launches the storage broker as a child
  process in the stable `OpenChatStorageBrokerV1` AppContainer profile. The
  broker has no network capability, receives only its inherited standard
  handles, and owns the SQLite connection, schema lifecycle, and bounded FIFO
  request queue. Its database and cache live in that AppContainer's private
  local-data directory; the parent sidecar does not open the database files.
  Windows AppContainer isolation prevents other applications from accessing
  that private data by default. The new NSIS package is per-user and does not
  install or start a Windows service. The previous `OpenChatStorage` service
  implementation and installer helper are retained in source but are not
  referenced or bundled by the current build. Tauri global MCP/provider commands use an allowlisted
  native-host RPC bridge; private secret-map methods and native response IDs
  are blocked from renderer requests, and private responses are consumed
  without forwarding them to the UI. The old Tauri SQLite command
  implementation is compiled only for tests. Window state remains in a
  separate JSON file.

  A new AppContainer profile imports the old `%LOCALAPPDATA%` database from a
  verified SQLite snapshot and retains the source as a rollback copy. The
  import is skipped with a clear startup error if the legacy Windows storage
  service is registered or its protected `%ProgramData%` storage directory
  remains, because that database may contain newer data than the old local
  copy. That service database is not readable by the unelevated desktop
  process, so moving an existing service-backed profile still requires a
  separately verified export path.
  When this conflict blocks startup, the user may explicitly confirm creating
  an empty AppContainer profile. The broker records that legacy sources were
  intentionally preserved, skips database, secret, local-model, and project
  index imports, and leaves old WebView localStorage untouched on later starts.
  This does not migrate or delete the protected profile.
  Legacy provider secrets are transferred in bounded zeroizing IPC frames and
  remain DPAPI-protected. The old project-index database contributes only
  per-project enabled settings; its derived text is marked stale and rebuilt
  through the broker on demand, and the old index file remains untouched. Both
  chat and backend schema versions migrate after a verified pre-migration
  backup. Startup checks integrity and the chat schema contract, repairs known
  missing chat columns and sort indexes, repairs known backend-derived indexes
  and triggers, and runs the backend migration ledger when needed. Migration
  steps use SQLite transactions so interrupted upgrades resume from the last
  committed version. Windows full-profile restore remains unavailable until a
  verified broker-owned database replacement protocol is implemented.
- The local service reads newline-delimited JSON-RPC frames with a 24 MiB
  per-request limit, accepts string request IDs up to 256 UTF-8 bytes, and
  accepts method names up to 128 bytes. The parser materializes at most 100,000
  values in one `params` object and drains remaining JSON without materializing
  additional parameter values after that limit is reached. Active work is
  limited to 64 operations, 64 MiB of combined request payloads, and 250,000
  parsed parameter values. These limits bound both input bytes and collection
  allocation; the parameter budget is released with its operation. The ID
  bound limits the extra copy retained for operation cancellation and response
  matching.
  Oversized frames are drained through their delimiter; complexity and
  capacity rejections preserve stream alignment.
- The renderer serializes sidecar writes and matches terminal responses and
  progress events by request ID. It applies the service's 64-operation and
  64 MiB aggregate request limits, and drops requests that time out before their
  queued write begins. A request deadline covers its queued pipe write and
  response wait; long-running operations bound startup writes while leaving the
  operation result open for streaming work. A timed-out operation removes its
  pending response handler and makes one best-effort `system.cancel` request; a
  timed-out cancellation request does not start another cancellation request.
  Diagnostic health fields and the service database-preparation response are
  validated and typed at the renderer boundary.
  Concurrent initialization and early diagnostic probes share one sidecar
  spawn, and late close events from an older process cannot clear the active
  process handle.
- Model-requested file reads, writes, edits, and search open relative paths
  through a `cap-std` directory handle rooted at the active tool scope, avoiding
  ambient path re-resolution after validation. Reads and writes reject
  hard-linked files so an in-scope name cannot expose or mutate an inode also
  linked outside the scope. Project instructions, Skills, task/tool/MCP
  catalogs, and index reads use the same hard-link check. If a hard link is
  encountered while indexing file content, that sync remains stale. Project
  instruction saves create and atomically replace files through the opened
  `.openchat` directory capability.
- Hard-link rejection was measured against the same synthetic search benchmark
  before and after the opened-handle check. Full no-match search p50 changed
  from 193.093 to 199.644 ms for 10,000 files (+3.4%), and from 1.988 to
  2.018 s for 100,000 files (+1.5%). Late-match search p50 changed from
  174.791 to 178.908 ms and from 946.600 to 957.441 ms, respectively. These
  are local latency measurements; CPU and resident memory were not profiled.
- Saved-output history search was measured before and after replacing the
  renderer's per-conversation message loading with one bounded Tauri database
  command. In the same in-memory fixture of 250 conversations, 50,000 messages,
  and 500 saved outputs, median latency changed from 1,142.195 ms to 32.374 ms
  (35.3x faster). Logical database queries changed from 252 to 1, renderer
  message objects materialized changed from 50,000 to 30, and serialized IPC
  payload changed from 28,406,532 bytes to 12,855 bytes. The optimized search
  returns at most 30 matches with excerpts capped at 4,096 UTF-8 bytes. This
  synthetic in-memory measurement does not profile CPU or resident memory and
  does not predict performance for every user database.
- Project indexing opens files relative to the selected project root capability
  and caps one sync at 128 MiB of file reads, 128 KiB per file, 20,000 indexed
  files, and 100,000 traversal entries. A scan that reaches a bound stays stale,
  so partial results are not used as model context.
- Read-only Git status, diff, and history use fixed arguments, bounded output,
  and a five-second process deadline. The repository-classification probe also
  has a deadline and kills its child when the task is dropped.
- Chat requests reject project skill selections above 32 before copying IDs
  out of the decoded RPC payload; syntax and duplicate checks remain in the
  project-skill boundary before project files are read.
- The bundled Tauri CSP allows only Tauri IPC and the local asset origin for
  network connections. Vite's `127.0.0.1:5173` HTTP and WebSocket sources are
  present only in `devCsp`.
- Search provider requests do not follow redirects and cap decoded response
  bodies at 1 MiB. Arbitrary page reads use a separate client that checks and
  pins public DNS results for each validated redirect.
- The local engine installer accepts only catalog-pinned assets, verifies their
  declared size and SHA-256 digest, and extracts into a staging directory.
  ZIP entries are streamed manually instead of using archive extraction helpers:
  absolute/traversal paths, duplicate paths, symlinks, entries over 4 GiB, and
  total output over 8 GiB are rejected. This avoids applying archive-provided
  permissions or link targets and bounds decompressed output.
- Project-scoped MCP stdio processes run through the local service's Windows
  AppContainer sandbox in approval and safe-operation modes. Full access starts
  them without AppContainer isolation, under the current Windows account; OS ACLs
  still apply and the service does not elevate privileges. The service requests user approval before starting an
  enabled server when its project permission is `ask`; only an explicit
  `allow` skips that startup prompt, and `deny` excludes the server. This
  startup approval is separate from approval for each MCP tool call. In the
  isolated modes, a project-scoped process can write only within its selected
  project root; full access uses the current account's filesystem permissions.
  In both modes, stdout is capped at 1 MiB per frame while being read, and its Job Object
  limits it to 32 active processes, 1 GiB per process, and 2 GiB total memory.
  User-confirmed signed Windows connector packages are pinned by version,
  digest, and catalog-key fingerprint in project configuration. The service
  rechecks the executable digest before launch and grants its package directory
  read-only access in the AppContainer when isolation is enabled. The pinned key identifies the signing
  key but does not verify the publisher's real-world identity.
  Global MCP tool discovery uses the shared Windows AppContainer process sandbox. It
  grants write access only to that server's validated install directory and
  read/execute access to the selected `npx`/Node/npm or `uv`/`uvx` runtime paths.
  The sandbox receives the `internetClient` capability for package downloads,
  but no private-network capability; a Windows test verifies that localhost
  remains unreachable with this capability enabled. Runtime paths and profile
  variables are explicitly set, npm user/global configuration paths are
  redirected into the per-server profile, host proxy and credential variables
  are not inherited, and only the selected server's configured secrets are added.
  The `npx` command uses the Windows system directory's `cmd.exe`, not the
  user's `ComSpec`. Stdout frames are capped at 1 MiB while being read, and
  stderr is drained without buffering. The process joins a kill-on-close Job
  Object before its primary thread resumes, limiting the process tree to 32
  active processes, 1 GiB per process, and 2 GiB total job memory. The protocol
  timeout is 12 seconds, and only one probe can run at a time. Sandbox setup
  fails closed: the probe does not retry in the Tauri host context. In interactive
  desktop sessions, child processes loading console and USER32 subsystems (such as
  PowerShell) also receive minimum read/execute DACL access on the WindowStation
  (`WinSta0`) and Desktop objects, managed under `APP_CONTAINER_ACL_LOCK` and revoked on
  cleanup. ACL updates
  are serialized within the process by a mutex and across processes sharing the
  same profile path by a stable lock file at
  `%LOCALAPPDATA%\OpenChat\appcontainer-acl.lock`. The lock file remains in place
  so all processes coordinate on the same file object; acquisition fails after
  30 seconds instead of writing without coordination. Null or absent DACLs are
  rejected because they cannot be safely narrowed.
- Windows provider keys and global MCP credentials use per-user DPAPI storage.
  Decrypted native plaintext and serialized secret-map buffers are zeroized;
  `SecretMap` also clears replaced, removed, and remaining Rust-owned values.
  A profile-scoped lock file and Windows exclusive file lock cover each full
  read-modify-write operation across processes and login sessions, so parallel
  provider and MCP settings updates cannot overwrite one another. The empty
  lock file remains beside the DPAPI map to preserve a stable lock target; lock
  acquisition fails after 30 seconds instead of writing without coordination.
  ChatGPT token endpoint responses are capped at 64 KiB before JSON parsing.
  The streaming read enforces this limit even when the endpoint omits its content
  length, while non-success HTTP statuses retain their existing stable errors.
  ChatGPT token requests do not follow redirects, preventing a 307/308 response
  from forwarding a credential-bearing form body to another host.
  ID tokens from that HTTPS token endpoint must use the advertised `RS256`
  algorithm and match the exact OpenAI issuer and this client's audience. The
  service also requires a subject, an unexpired `exp`, and a present `iat` no
  more than five minutes in the future; untrusted extra audiences are rejected.
  Signature verification relies on the authenticated TLS token-endpoint response,
  as allowed for this direct token-endpoint flow by OpenID Connect Core 3.1.3.7.
  Provider request keys remain in the service process: Tauri no longer exposes
  a command that returns them to the React webview. Any unexpected RPC `apiKey`
  field is removed into zeroizing memory and ignored; the service uses only the
  key associated with the saved provider route.
- Windows toast activation bounds UTF-16 scans before converting COM strings.
  Oversized reply text is reported as over-limit and is not retained in the
  activation payload.
- Conversation Markdown and support diagnostics exports open their save dialog
  from the Tauri host. The renderer supplies content and a proposed filename,
  never an absolute destination path; cancelling the native dialog writes
  nothing. Support diagnostics accept only the versioned runtime and storage
  health fields; history status is restricted to `loading`, `available`,
  `unavailable`, `corrupt`, or `backupUnavailable`. Chat content, credentials,
  and local filesystem paths are excluded. The frontend no longer has the
  `dialog:allow-save` permission.
- The sidecar health RPC returns its database path only for the renderer's
  startup consistency check and does not return the broader storage root.
  Support diagnostics use only health status and schema version, so local
  filesystem paths stay out of the exported bundle. Previous Windows installs
  used a LocalService storage service and protected ProgramData database. The
  current per-user build uses the AppContainer broker described above and does
  not start that service. The legacy store remains protected and must be
  exported by a trusted service process before it can be migrated without
  elevation; SYSTEM and administrators remain trusted principals.
- Conversation and profile archive export RPCs do not accept a destination path.
  The service stages one encrypted archive at a time in its application cache
  and returns a random, short-lived export identifier. The Tauri host validates
  and consumes that identifier, opens the native save dialog, and copies the
  archive through an atomic no-replace publish step, so a destination created
  after the initial existence check is preserved too. Cancelling or completing
  the dialog removes the staged archive; abandoned exports expire after six
  hours.
- Project creation does not accept a renderer-supplied folder path. The Tauri
  host opens the native directory picker, binds the selected directory to a
  five-minute single-use token, and revalidates that the directory still
  resolves to the same location before saving it. Worktree registration also
  requires the native picker to confirm the requested worktree directory.
  Worktree RPCs resolve the project folder from the saved project ID; renderer
  paths are not used to select the Git repository. Unscoped file-reading RPCs
  are not exposed. Interactive terminal sessions use random IDs and are bound
  to the conversation that created them, so another conversation cannot read
  output, send input, or terminate the process. The service permits at most 16
  active terminal sessions; each session holds a slot through process cleanup,
  bounding retained terminal output to 2 MiB across the manager.
- Windows interactive terminal commands and configured project tools run in
  kill-on-close Job Objects capped at 128 active processes. Their per-process
  committed memory limit is at most 4 GiB; the process-tree limit is half of
  physical memory, clamped between 512 MiB and 8 GiB. If Windows cannot report
  physical memory, command startup fails closed. Project-scoped MCP stdio
  servers retain their separate, stricter limits documented above.
- Conversation attachments are bounded by count, type, and size before storage
  or provider use. Tauri and service attachment reads and writes reject symbolic
  links or reparse-point redirection at the attachment root and conversation or
  message directories, keeping attachment paths inside the application store.
- Project search keeps candidate paths relative to an open root-directory
  capability and opens files through that capability. A file replaced with an
  outside symbolic link after traversal cannot redirect the search; failed
  opens are surfaced through the existing truncated-result state.
- Project task, project tool, and MCP catalogs are read with their existing
  byte limits through the selected project's directory capability. MCP catalog
  replacement creates and renames the temporary file relative to the opened
  `.openchat` directory, preventing path swaps from redirecting the write.
- Windows CI runs RustSec audits for the service, sandbox, provider secret
  store, and Tauri lockfiles, plus a full npm audit for frontend dependencies.
  The frontend moved to Tailwind CSS 4.3.3 with the dedicated
  Vite plugin, removing the development dependency paths to
  `GHSA-vfj7-8cjw-p6xm` in `braces@3.0.3`; both the full and production-only
  npm audits now report zero vulnerabilities. The Tailwind v4 browser floor is
  Chrome 111, and the Tauri NSIS bundle declares WebView2 111.0.1661.0 as its
  minimum so the installer can request a runtime update. Evergreen runtime
  updates may still be held back by offline operation or administrative
  policy. See the [GitHub advisory](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
  and the [Tailwind compatibility requirements](https://tailwindcss.com/docs/compatibility).
  The Windows pull-request workflow grants only `contents: read`, and checkout
  does not persist its token.
  CI also rejects unused direct Rust dependencies on the service, sandbox,
  secret-store, and Tauri compilation targets.
  A local RustSec 0.22.2 scan on 2026-10-18 reported no advisory for the
  Windows sandbox or provider secret store. It reports unmaintained `paste`
  in the service's compile-time `tokenizers` dependency and unmaintained
  `proc-macro-error` plus unsound `glib@0.18.5` in Tauri's GTK/WebKit dependency
  tree. The `glib` issue is fixed in `glib >=0.20.0`; Cargo confirms that the
  GTK/WebKit tree is not part of the supported `x86_64-pc-windows-msvc` target.
  Reassess this dependency chain before adding Linux support. The Windows
  full and production npm audits both report zero vulnerabilities after
  replacing the Tailwind 3 dependency tree. The upstream `braces` advisory
  remains published, but the affected package is absent from the frontend
  lockfile.
  RustSec lists no patched versions for `paste` or `proc-macro-error`. Track
  upstream dependency updates before changing these transitive dependencies.
  Windows CI also rejects missing public Rust API documentation and broken
  intra-doc links across the service, sandbox, secret store, and Tauri host.
  A local Semgrep 1.181.0 scan on 2026-10-18 covered 290 application source
  files with metrics disabled. The `p/security-audit` and `p/secrets` packs
  reported no findings. Adding `p/rust` produced 159 heuristic matches: 103
  unsafe-use, 51 temporary-directory, four executable-path, and one
  command-argument match. These are review prompts, not confirmed
  vulnerabilities. Temporary-directory matches point to uniquely named test
  fixtures; executable-path matches point to notification registration and
  test subprocesses; the command-line match is the service's validated
  `--data-root` parser. The affected Rust crates deny undocumented unsafe
  blocks, and their Clippy checks pass. Semgrep parsed about 99.9% of Rust
  source lines; two files were not fully parsed because the Semgrep Rust parser
  rejected valid `#[cfg(windows)]` attributes in
  `services/openchat-local-service/src/domain/providers/chatgpt/streaming.rs` and
  `services/openchat-local-service/src/domain/providers/openai_compatible/conversation.rs`.
  Manual inspection confirmed those attributes are Windows-only context
  destructuring patterns; neither file contains an `unsafe` block. The parser
  gap remains, so Semgrep findings from those two modules are incomplete.
  Semgrep's broad unsafe-use rule does not replace Rust-aware review.
- ChatGPT and OpenAI-compatible conversation streams cap raw provider response
  bytes at 16 MiB for each assistant response, including tool follow-up rounds.
  The existing 1 MiB per-event limit remains in place. Oversized assistant
  responses stop with a stable error while preserving partial chat content;
  automatic conversation title responses use a separate 64 KiB cap, and
  image-generation downloads use their own response limit.
- Global MCP settings currently support configuration, secret storage, and
  connection checks only. Their server descriptions and discovered tool names
  are not passed into chat prompts or runtime tool definitions. Project MCP
  servers remain the only MCP servers connected to chat tool execution.

## Performance evidence

Assistant text streaming sends append-only text fragments over the local RPC
channel. The renderer no longer receives a full copy of the accumulated answer
for every provider token update. The desktop message store checkpoints full
snapshots after each 256 KiB of new text and coalesces pending native writes to
one active and one latest snapshot. It also saves start, full-state, tool,
citation, completion, and controlled-stop updates. Durable ChatGPT conversations
apply the same text checkpoint synchronously in the service store and persist
terminal state on completion, cancellation, or handled failure. The service
store therefore limits uncheckpointed ChatGPT text to less than 256 KiB. The
desktop store flushes on controlled completion, stop, and failure, but an abrupt
process exit during a slow pending native write can widen the gap beyond one
checkpoint. Temporary conversations remain memory-only until the user saves
them.

The renderer coalesces text-only stream snapshots at a 50 ms cadence in both
regular and temporary conversations. Provider deltas still update the current
message immediately in the stream handler, and persistence checkpoints still
use that latest snapshot. Tool, citation, reasoning, and other message-state
events publish immediately. A pending text snapshot is flushed before a
controlled completion or stop, replaced by a terminal failure snapshot on a
normal-send error, and discarded for a failed retry or when the stream lifecycle
ends. This bounds text-only-driven renderer message-list updates to at most 20
per second during a continuous text stream; immediate state events can add
updates.
ChatGPT OAuth reasoning summaries continue to use provider-declared summary
groups. The direct OpenAI API route requests `reasoning.summary: "auto"` only
for documented reasoning model IDs, and maps summary events into those groups;
raw reasoning text events are ignored. The renderer displays these groups only
for ChatGPT OAuth and direct OpenAI API messages.
For an illustrative 100 text deltas in one second and a foreground timer, that
reduces full-list publications from 100 to at most 20. At 10,000 existing
messages, `upsertMessage` previously did a linear ID search and a full array map
for each publication, about 2,000,000 array visits across those 100 updates;
the coalesced path performs about 400,000 visits for 20 updates. Grouping,
virtual offsets, and context-usage estimation likewise rerun only when a
published message changes. These are operation-count estimates under the
stated event-rate scenario, not measured provider throughput or WebView CPU,
latency, or memory benchmarks.

The virtualized conversation renderer now indexes response-version metadata by
the virtual row's original message index. The previous per-row linear lookup
could perform `V × N - V × (V - 1) / 2` ID comparisons for `V` contiguous rows
near the end of an `N`-message conversation; direct indexing performs `V`
lookups and allocates no additional index. For `N = 100,000` and `V = 20`,
that is 1,999,810 comparisons versus 20 indexed reads. This is an operation
count comparison, not a wall-clock React/WebView benchmark. The frontend build
reported an `App` chunk of 355.01 kB (95.53 kB gzip) before and 354.98 kB
(95.51 kB gzip) after; Vite build time changed from 4.13 s to 3.97 s. Those
build measurements do not establish a runtime speedup.

Virtual message heights and prefix offsets are now memoized separately from
the viewport slice. A scroll-only update reuses the existing offsets, performs
two binary searches, and creates only the overscanned visible rows; it no
longer estimates every message height or rebuilds the `N + 1` offset array.
For 100,000 messages, the old scroll path performed 100,000 height lookups or
estimates plus 100,001 offset writes on every scroll update. The new scroll
path performs no height calculations and at most about 34 offset comparisons
for the two binary searches, plus work proportional to the visible slice. A
message, width, or measured-row-height change still rebuilds all offsets in
O(N). This is a deterministic operation-count comparison, not a measured
WebView latency or CPU/RAM profile.
The validating build produced an `App` chunk of 355.09 kB (95.54 kB gzip) in
4.00 s. Compared with the preceding build, the chunk increased by 0.11 kB
(0.03 kB gzip) and build time by 0.03 s; neither figure measures scroll-time
performance.

The first-height estimator previously split every unmeasured message on `\n`
and reduced the resulting array while rebuilding virtual offsets. It now scans
line boundaries with `indexOf`, preserving the same wrapping and blank-line counts
while removing the per-line temporary array. The added Node test compares the
new count with the prior split-based result for empty text, trailing and repeated
newlines, long lines, CRLF, and Unicode; it also checks 250,000 blank lines. All
four frontend rendering-limit tests passed. Across five invocations of the
five-sample local microbenchmark on a 4.1 MB / 100,000-line fixture, median estimate time
ranged from 6.00 to 9.29 ms with `split` and from 1.32 to 2.08 ms with the
boundary scan. This measures the line estimator only, not React rendering, peak
memory, or WebView responsiveness; the memory improvement follows from
eliminating the `O(L)` temporary split array, not from a heap-profile
measurement. Reproduce the tests and microbenchmark from the frontend directory
with `node --experimental-strip-types --test tests/virtualMessageLayout.test.mjs`.

Rich Markdown parsing is also bounded to 512 Ki UTF-16 code units per message.
Larger responses are rendered in full as plain selectable text, which avoids
constructing a large paragraph/list tree and splitting the entire message into
lines; citations remain visible below the response. The pure boundary test
checks the inclusive limit, but does not exercise React rendering. No
WebView-level memory or responsiveness measurement is available for this
fallback.

Virtual row callback refs now release their element and `ResizeObserver`
registration when React unmounts an offscreen row. The measured-height cache is
retained for layout calculations, while detached DOM elements and callback
closures are no longer retained for every message visited during a scroll.
This is a resource-lifecycle correction; no heap-profile measurement is
available to quantify its memory effect.

The synthetic RPC payload benchmark sends 10,000 eight-byte fragments in five
repeats. In Cargo's local test profile, the full-snapshot path serialized
401,360,000 bytes at an 11.635 s median; append-only deltas serialized 1,430,000
bytes at an 82.013 ms median, a 99.6% payload reduction and about 142x lower
event-construction plus serialization time. This isolates the RPC payload path;
it does not measure SQLite writes, end-to-end IPC latency, or frontend rendering.

The manual Windows filesystem benchmark is available with:

```powershell
cargo test --locked --manifest-path services/openchat-local-service/Cargo.toml benchmark_search_at_monorepo_scales -- --ignored --nocapture --test-threads=1
```

Reusing metadata already collected by the directory walker reduced per-entry
Windows filesystem queries. In the same local environment, the measured p50
latencies changed as follows:

| Dataset | Operation | Before | After |
| ---: | --- | ---: | ---: |
| 10,000 files | Directory listing | 379 ms | 119 ms |
| 10,000 files | Full no-match search | 482 ms | 202 ms |
| 100,000 files | Directory listing | 4,141 ms | 938 ms |
| 100,000 files | Full no-match search | 5,099 ms | 1,999 ms |

These are local latency measurements, not general hardware guarantees. The
latest after measurements were collected on 2026-10-17. The benchmark does not
record CPU or memory profiles.

The request-context estimator benchmark runs 20 warm-up iterations and five
repeats of 200 timed estimates against the same English, Turkish, and
JSON-shaped request. The measurements below are medians of each repeat's
percentile. They were collected on the same Windows host in Cargo's test
profile before and after routing ASCII characters through ASCII classification
instead of Unicode classification; token-estimation rules did not change.

| Metric | Before | After | Change |
| --- | ---: | ---: | ---: |
| p50 | 4,890 µs | 4,553 µs | -6.9% |
| p95 | 6,352 µs | 5,688 µs | -10.5% |
| p99 | 7,336 µs | 6,785 µs | -7.5% |

A current debug-profile recheck on 2026-10-18 measured median-of-five p50
4,415 µs, p95 5,101 µs, and p99 6,147 µs using the same benchmark. It did not
profile CPU or memory.

### Local development recheck

Service startup and health RPC baselines were remeasured on 2026-10-18 with
Cargo's Windows test profile and one test thread. File-search measurements below
use temporary synthetic datasets and were repeated on 2026-10-18 with one warm-up
iteration; service startup uses isolated profiles. These are local measurements,
not hardware-independent service guarantees.

| Benchmark | Dataset or sample | p50 | p95 | p99 |
| --- | --- | ---: | ---: | ---: |
| Service startup, cold | 100 debug starts | 30.052 ms | 33.043 ms | 55.646 ms |
| Service startup, warm | 100 debug starts | 18.661 ms | 20.123 ms | 21.142 ms |
| Service health RPC | 200 warm NDJSON round trips | 246.7 µs | 317.9 µs | 349.0 µs |
| Directory listing | 10,000 files | 81.511 ms | 92.183 ms | not sampled |
| Full no-match search | 10,000 files, direct walk | 158.395 ms | 159.214 ms | not sampled |
| Directory listing | 100,000 files | 785.920 ms | 797.903 ms | not sampled |
| Full no-match search | 100,000 files, repeated direct walk | 1.684 s | 1.722 s | not sampled |
| Common-term search | 100,000 files | 135.828 ms | 143.113 ms | not sampled |

The file-search benchmark re-run had no source changes between measurements.
Compared with the earlier same-day run, p50 varied by about 2.9–8.0% across
the four comparable listing and no-match rows, while p95 ranged from about 8.9%
lower to 9.2% higher. Treat this as a refreshed local baseline, not evidence of
a code speedup. The recheck records latency only; no CPU or resident-memory
profile was collected because this refactor changes module boundaries without
changing the measured algorithms or runtime ownership.

The 2026-10-18 search hardening now opens enumerated files through the selected
root's directory capability, preventing a file replaced with an outside
symbolic link from redirecting reads after traversal. On the same synthetic
datasets, full no-match search latency changed from the immediately preceding
implementation as follows:

| Dataset | Before p50 / p95 | After p50 / p95 | p50 change |
| ---: | ---: | ---: | ---: |
| 10,000 files | 161.446 / 162.162 ms | 190.109 / 193.810 ms | +17.8% |
| 100,000 files | 1.683 / 1.732 s | 1.951 / 1.988 s | +15.9% |

The additional capability checks have a measurable latency cost, while the
directory-listing recheck remained within ordinary run-to-run variation. This
is a security tradeoff, not a performance improvement. The benchmark measures
latency only; it does not sample CPU or resident memory.
Repeating the same-day service benchmark without source changes moved cold
startup p99 from 40.457 ms to 55.646 ms while p50 and p95 remained near 30 ms
and 33 ms. This is consistent with host scheduling noise; the p99 spread is
not evidence of a code regression.

An additional run after the 2026-10-18 attachment-path hardening measured cold
startup p50/p95/p99 at 32.557/36.212/37.019 ms, warm startup at
20.743/24.247/25.269 ms, and health RPC at 197.4/248.9/266.7 µs. The changed
path checks are not exercised by these startup or health requests, so this run
is a baseline recheck rather than evidence of a performance effect. No CPU or
resident-memory profile was collected.

The bounded RPC parameter parser was then compared with the last pre-change
run using the same manual benchmark. The after values are the median of three
runs; each used the Cargo Windows test profile, 100 starts per profile state,
20 RPC warm-ups, and 200 health round trips. The before column is one same-day
run, so this is a directional check rather than a controlled performance
claim:

| Metric | Before p50 / p95 / p99 | After p50 / p95 / p99 |
| --- | ---: | ---: |
| Fresh-profile service startup | 30.796 / 34.352 / 38.354 ms | 29.706 / 33.732 / 36.517 ms |
| Existing-profile service startup | 18.963 / 22.188 / 22.331 ms | 19.009 / 22.158 / 22.907 ms |
| Health RPC round trip | 218.2 / 291.4 / 326.8 µs | 223.8 / 280.8 / 321.5 µs |

Health RPC p50 increased by 2.6%; p95 and p99 decreased by 3.6% and 1.6%. The
mixed results and one-run baseline do not establish a parser regression or
speedup. These are process and local-pipe timings, not full desktop startup or
user-workflow timings. No CPU, allocation, or resident-memory profile was
collected.

After adding Windows process-resource sampling, the same debug benchmark ran
three times on 2026-10-18 with no service-runtime changes between runs. The
table reports the median of each run's p50, p95, and p99. Reproduce it with:

```powershell
cargo test --locked --manifest-path services/openchat-local-service/Cargo.toml --test process_rpc benchmarks_debug_service_startup_and_health_rpc -- --ignored --nocapture --test-threads=1
```

Startup is measured from process launch through the first successful health
response; CPU cycles
are sampled at readiness. Working set is the process peak reported at the same
point. Health RPC latency and process CPU were measured after 20 warm-ups and
over 200 health round trips:

| Metric | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Cold startup to ready | 29.634 ms | 33.051 ms | 35.581 ms |
| Cold CPU cycles through ready | 93.117 Mcycles | 96.951 Mcycles | 98.792 Mcycles |
| Cold peak working set | 17.81 MiB | 17.91 MiB | 17.95 MiB |
| Warm startup to ready | 18.848 ms | 20.233 ms | 22.306 ms |
| Warm CPU cycles through ready | 75.998 Mcycles | 80.889 Mcycles | 81.611 Mcycles |
| Warm peak working set | 17.77 MiB | 17.84 MiB | 17.88 MiB |
| Health RPC round trip | 219.8 µs | 273.4 µs | 305.8 µs |

Across those three runs, health RPC consumed a median 1,019.8 kcycles per
request; the median post-batch process peak working set was 18.61 MiB. CPU
cycles come from `QueryProcessCycleTime`, and working set comes from
`K32GetProcessMemoryInfo`. These are local debug-profile baselines, not an
optimization comparison. Cycle counts are hardware-dependent and do not
represent CPU utilization, allocations, private bytes, or a full desktop
WebView profile.

One additional run of the same benchmark on 2026-10-18 completed 100 cold and
100 warm starts, followed by 20 warm-ups and 200 health RPCs. It measured cold
startup p50/p95/p99 at 30.392/33.488/35.771 ms, cold CPU cycles at
92.321/96.856/97.738 Mcycles, and cold peak working set at 17.91/18.04/18.08
MiB. Warm startup measured 18.841/19.834/23.914 ms, warm CPU cycles measured
75.949/78.636/79.326 Mcycles, and warm peak working set measured
17.88/17.97/18.00 MiB. Health RPC latency measured 194.4/260.1/293.1 µs;
average process CPU was 927.5 kcycles per request, and post-batch peak working
set was 19.59 MiB. No source changed for this run, so it adds a current data
point rather than an optimization comparison. It still does not measure CPU
utilization, allocations, private bytes, or WebView behavior.

An uninstrumented current-worktree recheck ran the same benchmark three times
on 2026-10-18 with no service-runtime source changes between repetitions. The
table reports the median of each run's p50, p95, and p99 values:

| Metric | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Cold startup to ready | 34.314 ms | 40.449 ms | 45.736 ms |
| Cold CPU cycles through ready | 105.364 Mcycles | 116.226 Mcycles | 122.138 Mcycles |
| Cold peak working set | 17.98 MiB | 18.09 MiB | 18.13 MiB |
| Warm startup to ready | 22.172 ms | 26.429 ms | 28.705 ms |
| Warm CPU cycles through ready | 87.964 Mcycles | 94.364 Mcycles | 97.765 Mcycles |
| Warm peak working set | 17.95 MiB | 18.06 MiB | 18.11 MiB |
| Health RPC round trip | 245.6 µs | 319.4 µs | 393.7 µs |

Health RPC consumed a median 1,157.9 kcycles per request; post-batch peak
working set was 19.27 MiB. Compared with the previous single-run reference,
the current p50 values are 12.9% higher for cold startup, 17.7% higher for warm
startup, 26.3% higher for health RPC, 14.1% higher for cold CPU cycles, and
15.8% higher for warm CPU cycles. Cold and warm peak working-set p50 changed
by less than 0.5%. These same-host debug measurements are directional and do
not identify a code cause; no implementation change was made based on them.
The higher startup and RPC values warrant stage-level profiling before making
an optimization claim. CPU utilization, allocations, private bytes, and WebView
behavior remain unmeasured.

The manual benchmark now also samples private commit from Windows
`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`. Three repeats on the same day, again
with no service-runtime source changes, measured:

| Metric | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Cold startup to ready | 31.340 ms | 36.543 ms | 39.502 ms |
| Cold CPU cycles through ready | 95.952 Mcycles | 102.143 Mcycles | 110.795 Mcycles |
| Cold peak working set | 17.98 MiB | 18.12 MiB | 18.20 MiB |
| Cold private commit | 5.73 MiB | 6.15 MiB | 6.19 MiB |
| Warm startup to ready | 19.825 ms | 25.056 ms | 26.063 ms |
| Warm CPU cycles through ready | 79.338 Mcycles | 85.210 Mcycles | 87.859 Mcycles |
| Warm peak working set | 17.95 MiB | 18.04 MiB | 18.06 MiB |
| Warm private commit | 5.70 MiB | 6.05 MiB | 6.12 MiB |
| Health RPC round trip | 241.9 µs | 312.7 µs | 396.1 µs |

The median health RPC used 1,125.4 kcycles per request; post-batch peak working
set was 18.95 MiB and private commit was 6.46 MiB. Compared with the earlier
single-run reference, these p50s are 3.1% higher for cold startup, 5.2% higher
for warm startup, and 24.4% higher for health RPC. Cold and warm CPU cycles
are 3.9% and 4.5% higher, respectively. Compared with the preceding
uninstrumented three-run group, startup p50s are 8.7–10.6% lower and CPU-cycle
p50s are 8.9% lower while service-runtime code remained unchanged. This
run-to-run movement shows that
the earlier timing deltas are not sufficient to attribute a regression to a
source change. It does not replace stage-level profiling or measure CPU
utilization, allocations, or WebView behavior.

The tool-operation benchmark uses a 501-file, 3.24 MB temporary fixture, 10
warm-ups, and 500 measured iterations. It measures operation latency separately
from JSON serialization and does not touch the checkout:

| Operation | p50 | p95 | p99 | Response bytes |
| --- | ---: | ---: | ---: | ---: |
| Directory listing | 5.366 ms | 6.251 ms | 7.418 ms | 4,248 |
| Listing without app cache | 5.322 ms | 6.226 ms | 7.406 ms | 4,248 |
| Matching search | 6.930 ms | 8.797 ms | 9.731 ms | 5,046 |
| Search without app cache | 6.737 ms | 8.120 ms | 9.246 ms | 5,046 |
| No-match search | 17.126 ms | 18.980 ms | 21.043 ms | 50 |
| File read | 0.569 ms | 0.866 ms | 0.986 ms | 17,175 |
| Late-range file read | 0.527 ms | 0.762 ms | 1.014 ms | 8,669 |
| File metadata | 0.159 ms | 0.230 ms | 0.388 ms | 54 |

The search implementation now reserves per-file match storage only after its
first match. In the same 10,000-file no-match benchmark, p50 changed from
167.420 ms to 168.660 ms and p95 from 167.750 ms to 181.591 ms. At 100,000
files, p50 changed from 1.749 s to 1.701 s and p95 from 1.817 s to 1.748 s.
These latency differences do not establish a speedup; the change removes the
unused result-buffer allocation for files with no matches. Process memory was
not profiled, so no resident-memory reduction is claimed.

The current request-context benchmark's five repeat p50 values ranged from
4.395 to 4.428 ms; the median p50 was 4.415 ms. Creating a new HTTP client
measured 12.2–13.8 µs p50; retrieving the shared client measured 19–21 ns p50.

A 2026-10-18 experiment tried a single-pass ASCII byte path for request-text
estimation. Using the same ignored benchmark three times before and after, the
median of each run's five p50 samples changed from 4.316 ms to 4.594 ms, a
6.4% increase. The candidate was removed, so the current implementation keeps
the character-based estimator. This synthetic request benchmark is directional
and does not represent full chat latency.

Only the per-file search result-buffer allocation has a before-and-after
comparison in this recheck. The startup, health RPC, request-context, and tool
operation numbers are baselines without a corresponding code change. Full file
traversal still scales with the number of files; the 100,000-file no-match case
remains around 1.7 seconds. Before larger search changes, measure representative
real projects and preserve current ignore, ordering, pagination, and
filesystem-freshness semantics. This recheck did not sample process CPU or
memory, frontend WebView rendering, or JavaScript garbage collection; the
renderer was not launched for profiling.

### Incremental project search

Direct project search previously accumulated every candidate path before
starting parallel file reads. It now scans an initial group of 2,048 candidates
during traversal, then uses the existing 8,192-candidate group size if the
result page is not full. This bounds the candidate queue to 8,192 entries and
lets an early full page stop the remaining traversal. File ordering, pagination,
ignore and hidden-file rules, symlink handling, and the 128 MiB read cap are
unchanged. A regression test checks pagination across the initial batch edge.

The same Windows debug-profile ignored benchmark was run once before and once
after the change on 2026-10-18. Both runs used one warm-up and temporary
10,000-file and 100,000-file datasets. The no-match and common-term p50 values
are medians from repeated calls; late-match values use the benchmark's three
direct-search samples:

| Dataset | Operation | Before p50 | After p50 | Change |
| ---: | --- | ---: | ---: | ---: |
| 10,000 files | Full no-match search | 196.394 ms | 199.115 ms | +1.4% |
| 10,000 files | Late-match search | 176.893 ms | 55.678 ms | -68.5% |
| 10,000 files | Common-term search | 18.218 ms | 18.372 ms | +0.8% |
| 100,000 files | Full no-match search | 2.006 s | 2.040 s | +1.7% |
| 100,000 files | Late-match search | 952.418 ms | 183.144 ms | -80.8% |
| 100,000 files | Common-term search | 138.766 ms | 138.372 ms | -0.3% |

The matching-path gains are substantial on this synthetic fixture; full scans
and common-term search stayed near the previous run. The no-match p95 changed
from 198.031 to 200.888 ms at 10,000 files and from 2.058 to 2.055 s at
100,000 files. One run per version is directional evidence, not a hardware
independent guarantee. This benchmark reports latency only: it does not
measure candidate-queue memory, process working set, CPU utilization, or
allocations. The walker still sorts each directory's entries, so this change
does not bound all traversal memory for a single directory with very high
fan-out.

### Current same-day debug recheck

The existing ignored sidecar benchmark was run three times serially on 2026-10-18 with Cargo's Windows test profile after adding test-only startup phase timers. Each run used 100 cold starts, 100 warm starts, 20 RPC warm-ups, and 200 health RPCs. The table reports the median of the three runs' p50, p95, and p99 values. Reproduce it with:

```powershell
cargo test --locked --manifest-path services/openchat-local-service/Cargo.toml --test process_rpc benchmarks_debug_service_startup_and_health_rpc -- --ignored --nocapture --test-threads=1
cargo test --locked --manifest-path services/openchat-local-service/Cargo.toml --lib benchmarks_service_bootstrap_components -- --ignored --nocapture --test-threads=1
```

| Metric | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Cold startup to ready | 29.109 ms | 36.142 ms | 37.476 ms |
| Cold process spawn and pipe setup | 4.725 ms | 5.959 ms | 10.755 ms |
| Cold post-spawn to ready | 24.251 ms | 27.600 ms | 30.873 ms |
| Cold CPU cycles through ready | 90.319 Mcycles | 99.283 Mcycles | 114.236 Mcycles |
| Cold peak working set | 17.96 MiB | 18.09 MiB | 18.13 MiB |
| Cold private commit | 5.71 MiB | 6.13 MiB | 6.19 MiB |
| Warm startup to ready | 18.185 ms | 20.518 ms | 24.232 ms |
| Warm process spawn and pipe setup | 2.582 ms | 3.188 ms | 8.924 ms |
| Warm post-spawn to ready | 15.599 ms | 17.009 ms | 17.963 ms |
| Warm CPU cycles through ready | 73.205 Mcycles | 78.545 Mcycles | 81.555 Mcycles |
| Warm peak working set | 17.93 MiB | 18.02 MiB | 18.08 MiB |
| Warm private commit | 5.71 MiB | 6.07 MiB | 6.12 MiB |
| Health RPC round trip | 223.1 µs | 298.9 µs | 347.2 µs |

The median health RPC used 1,042.8 kcycles per request; post-batch peak working set was 19.52 MiB and private commit was 7.19 MiB. The phase timers show that most measured startup latency falls after process creation: the median cold post-spawn interval is 24.251 ms of 29.109 ms total, and the warm interval is 15.599 ms of 18.185 ms. `spawn and pipe setup` covers command creation, process spawn, and acquiring both standard-I/O pipes. `post-spawn to ready` includes service initialization, database open and schema checks, provider HTTP-client construction, and the first `system.health` round trip. The health handler only reports readiness and storage metadata; this benchmark does not separate its dispatch cost from bootstrap or isolate database connection setup. These results locate the next profiling boundary but do not establish a database bottleneck or a source-level cause.

The preceding three-run reference on this host measured 31.340 ms cold-start p50, 19.825 ms warm-start p50, and 241.9 µs health-RPC p50, while working-set p50 stayed within 0.3 MiB. In this latest recheck, per-run cold-start p50 ranged from 28.856 to 47.298 ms, warm-start p50 from 18.096 to 27.856 ms, and health-RPC p50 from 221.2 to 268.8 µs. The median differences are run-to-run variation, not measured optimization gains; startup and health code was not optimized between these runs. The benchmark is a Windows 11 Pro build 26200 debug run on an AMD Ryzen 7 5700X with Rust 1.95.0. CPU utilization, allocation counts, WebView rendering, and JavaScript garbage collection remain unmeasured.

One additional serial run after the MCP auth-token storage change measured cold startup p50/p95/p99 at 30.363/33.436/37.204 ms, warm startup at 19.375/20.776/26.125 ms, and health-RPC round trip at 228.2/307.7/468.2 µs. Cold CPU through readiness was 93.489/98.353/99.135 Mcycles, peak working set 17.99/18.11/18.16 MiB, and private commit 5.71/6.18/6.27 MiB. Warm CPU was 77.449/82.756/85.966 Mcycles, peak working set 17.91/18.04/18.09 MiB, and private commit 5.71/6.10/6.15 MiB. Health RPC averaged 1,070.3 kcycles per request; post-batch peak working set was 19.64 MiB and private commit 7.22 MiB. The startup and RPC latency p50 values fall within the prior observed ranges; this is a single-run baseline refresh, not evidence of a performance change. No performance optimization was made in this slice.

### Request context estimate microbenchmark

The ignored request-estimator benchmark uses repeated English, Turkish, and structured-text fields, with 20 warm-ups and five repeats of 200 samples. On the same Windows 11 debug host, the original character-based estimator measured a median of the repeat-level p50 values of 4.700 ms; the corresponding median p95 and p99 values were 4.993 ms and 5.173 ms. An ASCII byte-scanning candidate preserved the boundary-test results but raised median p50 to 6.514 ms, so it was discarded. After restoring the original implementation, median p50 was 4.734 ms; the final two repeats were slower than the first three, so this recheck does not establish a performance change. This synthetic debug benchmark measures wall-clock latency only; it does not measure production request throughput, CPU cycles, or allocations.

A second ignored benchmark isolates synchronous bootstrap components with 100 samples per component, repeated three times on the same host. The table reports the median of the three runs' p50, p95, and p99 values. Cold storage opens use a new temporary profile each time; warm opens reuse one small initialized database. Provider-service construction builds its HTTP clients without making network requests.

| Bootstrap component | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Fresh profile and database open | 10.028 ms | 12.016 ms | 12.234 ms |
| Existing small database open | 1.942 ms | 2.351 ms | 2.402 ms |
| Provider-service construction | 0.030 ms | 0.036 ms | 0.046 ms |

The fresh-profile result includes directory setup, SQLite integrity checks, and schema initialization; the existing-database result measures this benchmark's small fixture and does not represent a user's potentially larger database. Provider-service setup is negligible in this measurement. These isolated calls do not account for the rest of process startup, Tokio runtime creation, OS scheduling, or the first RPC. No runtime optimization is justified by these measurements alone.

An additional current-worktree recheck ran this ignored benchmark three times serially on 2026-10-18, with 100 samples per component in each run. The table reports the median of each run's p50, p95, and p99 values.

| Bootstrap component | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Fresh profile and database open | 11.729 ms | 13.502 ms | 15.337 ms |
| Existing small database open | 2.390 ms | 2.815 ms | 2.969 ms |
| Provider-service construction | 0.031 ms | 0.060 ms | 0.083 ms |

The prior stored p50 values were 10.028 ms, 1.942 ms, and 0.030 ms, respectively. No startup or storage change was made for this recheck, so the higher storage measurements are a baseline difference without an established source-level cause. Cold-open p99 varied from 14.027 to 43.219 ms across the three runs, indicating scheduling or I/O noise; these timings do not justify an optimization. No CPU, allocation, private-memory, or disk-I/O profile was collected by this component benchmark.

The Tauri saved-output search benchmark was also rerun on the same synthetic in-memory fixture: 250 conversations, 50,000 messages, and 500 saved outputs. Its three-sample medians were 1,216.492 ms for the legacy path, which issued 252 queries and serialized 28,406,532 bytes, and 32.237 ms for the bounded path, which issued one query, returned 30 results, and serialized 12,855 bytes. This rechecks the current implementation; it is not an equal-work comparison because the bounded path paginates the result set.

The frontend bundle was measured before and after removing the eager English fallback dictionary from startup. `npm run build` runs TypeScript checking and Vite bundling; no Tauri package or Rust release binary was built. Before the change, non-English startup initialized both English and the selected locale. The English chunk is 183.42 kB (50.66 kB gzip), so the non-English startup code path no longer requests it; this is a source-verified potential transfer reduction, not a browser network measurement. Locale changes still load a dictionary when needed. The locale loader's `LocaleDictionary` return type requires every supported dictionary to contain the English keys, and TypeScript checking passed. A direct key-path comparison also found no missing English keys in the current German, Spanish, French, or Turkish dictionaries.

| Build artifact | Raw size | Gzip size |
| --- | ---: | ---: |
| `App` JavaScript | 356.51 kB | 95.85 kB |
| React vendor JavaScript | 222.28 kB | 69.21 kB |
| i18n runtime | 85.29 kB | 26.77 kB |
| Main CSS | 83.58 kB | 15.73 kB |
| Selected Turkish dictionary | 190.46 kB | 51.85 kB |

The current Vite bundle took 4.15 s to build; the preceding one-run build took 3.90 s. Build duration varies between runs and is not a runtime performance comparison. The bundle still emits each locale for on-demand language switching. No browser request trace, WebView startup profile, CPU utilization, JavaScript allocation, or garbage-collection profile was collected.

Conversation history currently loads the full `conversations` table through one
Tauri command and keeps every metadata row in the frontend store. The tab header
filters, copies, and sorts that collection during render; the sidebar filters it
again, and project sections scan it once per project. This gives full-history
IPC and memory cost, an `O(C log C)` tab sort per header render, and up to
`O(P × C)` project grouping work for `C` conversations and `P` projects. The
per-conversation 10,000-message query bound does not limit the number of
conversations. These are source-derived complexity bounds, not measured user
impact. Changing this path requires preserving archived chats, project grouping,
selection, search, and the current newest-first tab order. The tab header and sidebar
sections now memoize active, pinned, unassigned, and project groupings with `useMemo`,
and the tab header bounds rendered DOM triggers to the active conversation plus the top
20 most recent chats while keeping full history accessible in the sidebar and search.

Local-engine downloads are catalog-size and SHA-256 checked before extraction;
the archive reader also caps entries at 100,000, each file at 4 GiB, and total
expanded data at 8 GiB. ZIP and TAR decoding plus synchronous file writes used to
run inline in the asynchronous install RPC, occupying a Tokio worker throughout
large installs. The installer now runs only this extraction stage in Tokio's
blocking pool. Download verification, cancellation checks, extraction limits,
staging cleanup, and atomic publication remain in their existing order.

A manual benchmark used the same 128 MiB stored ZIP and a single-thread Tokio
runtime before and after that change. The benchmark measured how late a 10 ms
timer fired while extraction ran. Before offloading, the three delays were
1.160, 1.267, and 1.293 seconds (median 1.267 seconds). After offloading, they
were 23.108, 23.188, and 24.866 ms (median 23.188 ms), about 54.6 times less
timer delay. This isolates executor responsiveness under a one-worker stress
case; it does not measure production RPC latency on the default multi-threaded
runtime, extraction throughput, CPU use, or memory use.

## Migration sequence

1. **Relocate package roots and assets.** `frontend/` and `src-tauri/` now live
   under `apps/desktop/`, `native/openchat-service/` under `services/`, and the
   brand, font, and icon files under `apps/desktop/assets/`. Vite, TypeScript,
   Tauri, CI, Dependabot, ignore rules, and contributor documentation were
   updated in the same change.
2. **Establish frontend runtime boundaries.** Move the React entry point and
   desktop shell into `bootstrap/` and `shell/`. Move Tauri APIs and the local
   service client out of feature stores into `platform/`; keep one sidecar
   client instance and preserve command/RPC contracts. The entry point, shell,
   Tauri API boundary, and sidecar client have been moved; provider-specific
   Tauri calls in product modules now pass through the platform boundary.
3. **Organize frontend product modules.** Split conversation and project
   ownership, then place model catalog, provider setup, settings, and
   notification surfaces in their respective modules. Put shared controls in
   `design-system/` and translations in `localization/`. Replace cross-feature
   UI imports with focused module exports or shared primitives. Conversation,
   project, provider, model-library, settings, notification, shared-control,
   and translation boundaries have been moved. The settings adapter composes
   provider setup with conversation-owned catalog state without making the
   provider module depend on conversations.
4. **Organize the Rust service by responsibility.** Separate startup and
   transport from RPC dispatch; extract request handlers by domain and keep
   database, provider, filesystem, and process access at explicit adapter
   boundaries. Preserve the NDJSON protocol and existing storage behavior.
5. **Close integration references.** Update build/package instructions and
   source links, then run the repository's configured frontend, Rust, Tauri,
   and Windows packaging checks where the host environment supports them.

## Invariants and exclusions

- Do not change RPC names, request/response shapes, Tauri command names, or
  database schemas as part of this reorganization. The credential-boundary
  hardening is a deliberate exception: provider request parameters now carry
  route identifiers instead of API-key values, and the insecure Tauri key
  getter was removed. File-export commands also intentionally changed their
  payloads to obtain destinations through native save dialogs rather than
  accepting renderer-supplied paths. Project creation likewise now takes a
  one-time native folder-selection token instead of a renderer-supplied path.
- Do not change database, credential, attachment, model, or cache locations.
- Do not redesign screens or change user-facing behavior.
- Keep the frontend package lockfile beside its manifest and a single Rust
  workspace lockfile at the repository root.
- Keep generated `build/`, Cargo `target/`, frontend `dist/`, and installed
  dependencies out of the source architecture; preserve existing local data.
- Keep `third_party_licenses/` at the repository root because it describes
  dependencies shared by the desktop application and service.

## Completion criteria

- No supported build, CI, dependency-update, or contributor command points to
  the old package roots.
- Imports resolve from the new frontend modules without cycles or feature-store
  access to platform I/O. Conversations consume provider data through the
  provider module, and the provider module does not import conversation stores.
- The sidecar and Tauri host build as workspace packages from the repository root.
- Existing RPC and persistence contracts remain unchanged except for the
  documented credential and native file-path boundary hardening.
- The documented commands run from the stated directories, and any unavailable
  platform check is reported with the exact command and reason.

## Roadmap

- **Persistent Project Agent Memory & Semantic Context Engine:**
  - Dedicated `project_memory` storage to preserve architectural decisions, user conventions, and learned operational constraints across conversation sessions.
  - Automatic prompt context injection on new runs without leaking memory across distinct projects.
  - Hybrid lexical (FTS5) and semantic retrieval for large-scale codebases.
