# OpenChat roadmap

This roadmap reflects the current product implementation and has no calendar estimates. Some validation notes below record evidence collected for the retired Flutter client; they are historical and do not validate the current React/Tauri UI. The Windows desktop experience remains the first release target. Confirmed baseline and implemented milestones are listed separately. The numbered priorities are ordered from highest to lowest by product value, safety, and dependencies; a priority does not block unrelated work unless its dependency is stated.

## Current implementation

- React desktop UI hosted by Tauri, with a local Rust service and SQLite conversation storage under `%LOCALAPPDATA%\OpenChat`.
- ChatGPT OAuth connections, account/workspace selection, model catalog, account and quota information, streamed Responses, cancellation, and background conversation titles.
- OpenCode Console through its OpenAI Chat Completions endpoint. Other OpenCode protocol families are not supported.
- API-key providers: Gemini, Groq, Cerebras, OpenRouter, Mistral, DeepSeek, xAI, Perplexity, Together AI, SambaNova, Fireworks AI, DeepInfra, DashScope (Qwen), and Cohere, plus custom OpenAI- and Anthropic-compatible endpoints. Dedicated Ollama and LM Studio provider connections are planned. Mistral exposes context, vision, and documented reasoning capabilities; OpenRouter models are filtered to current zero-price text-chat entries that advertise tool support.
- Anthropic Claude Messages API adapter (`/messages`), supporting context limits up to 200K, vision, tools, and model-dependent reasoning controls.
- Shared provider request, tool, and stream event types across ChatGPT, OpenCode, Anthropic, and all compatible cloud and local providers, plus local shared instructions.
- Embedded release catalogs and a verified, cancellable installer for llama.cpp on Windows x64. Settings supports one llama.cpp model folder and bounded discovery of unregistered GGUF files, with explicit confirmation before registration. The managed llama.cpp path starts selected GGUF models on demand, waits for health readiness, streams chat through the shared route, and stops individual processes on cancellation or service shutdown. Manual Windows CPU and NVIDIA CUDA checks used real GGUF files; CUDA verification reported the RTX 3060, generated a text answer, and handled an attached PNG with its matching vision projector. These were manual runtime checks, not automated release tests. CUDA offload is recommended only when the detected driver meets the pinned catalog floor. llama.cpp is the only OpenChat-managed local inference engine. Runtime diagnostics record bounded, safe lifecycle fields. GGUF-declared and runtime-active context windows, projector pairing, and reported image and tool-template capabilities are implemented.
- Local workspace tools for file listing, search, reading, metadata, writing, and editing, plus web search, URL reading, and terminal command/session tools. Tool calls, arguments, progress, and results are stored with assistant messages and shown in the conversation UI.
- Global `Ask for approval`, `Approve safe operations`, and `Full access` settings govern built-in tool approvals. Filesystem calls use canonical path checks. Sensitive configuration files (`.git/hooks`, IDE configs) strictly require explicit user approval (`Ask`). On Windows, terminal sessions run in a temporary AppContainer by default, with optional WSL2 container execution (`bwrap --unshare-all`) for full kernel-level workspace isolation; non-Windows terminal execution uses process isolation.
- Real-time secret and credential redaction (DLP shield) scrubbed across terminal command output, MCP tool executions, and file reads to prevent live credentials from leaking into agent contexts or session streams.
- ChatGPT OAuth quota snapshots are stored per connection and workspace. Provider-reported input usage informs compaction where available, but there is no normalized per-request usage, cost, or prompt-cache ledger across providers; the context meter can use estimates.
- Conversation history, project grouping, model favorites, rename/delete/pin actions, retry, and Markdown export. Tool activity is included in Markdown exports.
- Per-conversation memory inspection, bounded hybrid FTS5 and optional local semantic archive search with dated source excerpts, best-effort credential redaction in derived tool indexes, and a confirmed reset for compacted context that preserves full history.
- A Windows Tauri NSIS installer and local data directories for the database, logs, and cache.

These bullets describe code present in the repository. They do not mean that every provider endpoint or account flow has been verified against a live service.

## Confirmed baseline and release gate

### Confirm the Windows baseline

**Status: verified by the user.** The Windows app, ChatGPT, and OpenCode have been exercised repeatedly and are currently working. Do not treat baseline release validation as an open implementation task.

- Recheck these workflows as regression coverage when a change affects them.
- Validate API-key providers with live accounts when credentials become available; that work remains in the provider-specific stage below and does not block the confirmed Windows baseline.

**Exit criteria:** met for the current Windows, ChatGPT, and OpenCode baseline. Reopen this baseline only if a regression or release-specific change requires it.

### Stabilize the provider contract

**Status: implemented in source and covered by deterministic cross-provider tests.** Keep provider-specific wire formats inside their adapters and make unsupported capabilities explicit to the user. Live API-key account validation remains a release gate below.

- Define which shared capabilities a provider/model can use, including streaming, reasoning summaries, and tools.
- Avoid sending tools to models that do not support them; show a clear reason when capability information is unavailable or a provider rejects the request.
- Models with confirmed tool-call support keep the existing tool flow; confirmed-unsupported models receive no tool definitions, while unknown support is disclosed in the composer and remains optimistic for compatibility. HTTP 400/422 responses to requests containing tool definitions or tool history use a sanitized, localized error; provider response bodies are not shown or retried automatically.
- Keep cancellation, partial responses, tool errors, quota limits, and stale model catalogs consistent across providers.
- Recheck ChatGPT's private endpoints when they change; they are compatibility-sensitive and are not a stable public API contract.

**Exit criteria:** all current shared-route adapters use the same explicit capability and safe-error contract; provider bodies and credentials do not leak into the React UI or logs. Met by source behavior and offline contract tests. Live provider-account checks remain a release gate below.

### Validate API-key providers

**Status: implemented in source; live account validation remains open after the first GitHub release.** Hosted cloud providers (Gemini, Groq, Cerebras, OpenRouter, Mistral, DeepSeek, xAI, Perplexity, Together AI, SambaNova, Anthropic, Fireworks AI, DeepInfra, DashScope, Cohere) are connected. Validation requires configured provider accounts and is not part of credential-free CI.

- Verify model catalog loading, streaming, cancellation, errors, local history, and tool calls with real accounts for each provider.
- Confirm current pricing, quota, tool-use capability, and data handling against each provider's account terms.
- Keep credentials in platform secure storage and provider requests in the local Rust service.

**Exit criteria:** every provider completes those real workflows without changing the shared chat UI or leaking keys and provider payloads into logs.

### Build managed local inference

**Status: llama.cpp is the only managed local inference engine.** OpenChat owns its release catalog, installation, model files, process lifecycle, and local request route. Linux/macOS support remains open.

- Keep release metadata tied to immutable upstream versions. Verify each downloadable engine and model asset by exact size and SHA-256 before publishing it into OpenChat's managed runtime or model directory.
- Keep Windows llama.cpp runtime selection for NVIDIA CUDA and CPU tied to detected driver compatibility. The Windows CPU package and CUDA path have passed manual runtime checks; repeat them only when a release-catalog or runtime change affects those paths.
- Evaluate future local engines as separate roadmap items before adding runtime code or dependencies.
- Add model catalogs and model downloads separately from engine binaries. Show file size, destination, progress, cancellation, integrity verification, and recovery after interrupted downloads.
- Keep the existing real-GGUF CPU/CUDA and vision-projector smoke checks as release regression validation when their inputs change. They are manual checks and are not repeated for unrelated provider or UI work. Bounded, sanitized lifecycle diagnostics are implemented without model paths or prompt content.
- Add local context-window metadata and only expose image, reasoning, and tool capabilities when the selected model/runtime supports them. Keep provider payloads and local process details inside the Rust service.
- Extend package selection, secure storage, process supervision, and data directories to Linux and macOS. Design platform-specific runtime support explicitly before adding another engine.

**Windows llama.cpp exit criteria:** met for the current CPU/CUDA release catalog, GGUF registration, streamed chat, cancellation, projector-backed image input, persistence, per-model process control, and shutdown. **Remaining scope:** Linux/macOS llama.cpp packaging.

## Completed implementation milestones

### Harden network access and indexed data

**Status: implemented in source and covered by deterministic tests.** The URL policy, archive-index controls, untrusted retrieval boundary, and best-effort credential redaction are in place.

- Add an outbound URL policy for read_url_content. Reject loopback, private, link-local, multicast, reserved, and cloud metadata destinations; validate every redirect and protect DNS resolution from rebinding between validation and connection.
- Give users clear control over which tool results enter full-text and semantic indexes. Support excluding selected tool types or conversations, clearing derived indexes, and documenting what is stored locally.
- Keep tool output and historical excerpts marked as untrusted. Never treat retrieved or indexed content as instructions.
- Redact common credential field names and visible provider token formats from derived keyword and semantic indexes. The migration rebuilds old tool-index rows, removes their old vectors, queues sanitized content for re-embedding, and prunes stale semantic cache files when a new index is loaded. Original message and tool data stay unchanged. Detection does not cover every custom, encoded, or transformed secret.

**Exit criteria:** URL fetches cannot reach prohibited network destinations through direct URLs or redirects; sensitive tool results can be excluded and removed from derived indexes; and existing MCP/browser plans inherit the same network and data-handling policy.

### Close provider, storage, and runtime correctness gaps

**Status: implemented in source and covered by deterministic tests.** The audited provider parsing, migration, metadata, download, and process-lifecycle gaps have focused fixes and regression coverage.

- Reject Responses API tool-call events with a missing, invalid, or out-of-range output index. Apply the per-turn tool-call limit to every event that can create a call.
- Make schema initialization stop at every requested target version. Align user-question item identity between group-level validation and database uniqueness.
- Validate provider metadata by JSON type and value, including treating null capability fields as unknown rather than fresh, valid metadata.
- Verify prepared semantic model files against their expected digests and reject non-finite embedding components before quantization.
- Add bounded total and idle timeouts to model downloads, throttle progress events, and retain safe range-resume and integrity checks.
- Avoid holding the local-runtime state lock through long readiness polling. Make port allocation resistant to free-then-bind races and tie installer asset URLs to their declared repository and release.
- Process the final buffered SSE event at end of stream and apply explicit size limits to encrypted compaction state.

**Exit criteria:** malformed or oversized provider events fail with stable errors; migrations reach each requested schema version; cache and model metadata cannot claim readiness from invalid values; downloads and runtime operations have bounded waits and observable failures.

### Automate integration and release validation

**Status: implemented in source and covered by local validation.** Offline checks cover the React/Tauri/Rust boundary, provider fixtures, migrations, and local lifecycle behavior. Windows CI builds the Tauri installer; a remote run requires a push or manual workflow dispatch.

- Move the live web-search smoke test and fixed-duration file benchmark out of the default test suite. Keep manual network checks opt-in and run performance benchmarks separately from correctness tests.
- Add local HTTP fixtures for Chat Completions, Responses, SSE, tool-call round trips, malformed events, cancellation, timeouts, and 401/429/5xx responses.
- Add Tauri-to-Rust process-level RPC tests, local-engine startup/shutdown checks, schema migration coverage, and model-download resume/integrity cases.
- Extend CI with rendered desktop smoke validation and checks for required installer contents.
- Keep live-account provider validation separate from deterministic pull-request checks.
- Reconcile provider and tool scope across README, roadmap, contribution guidance, and historical integration plans so documentation does not contradict the implementation.

**Exit criteria:** routine CI requires no provider credentials or public network access; protocol, process, migration, and release boundaries have deterministic checks; live tests and benchmarks run only when explicitly requested.

### Add portable backup, import, and restore

**Status: selected-conversation archive v1 and encrypted full-profile database/attachment backup are implemented in source.** Profile restore validates before scheduling, activates on the next launch, preserves the replaced profile, and rolls back if startup validation does not complete.

- Create a versioned archive for selected conversations and their attachments; full-profile database backup and global settings are not included in this selected-conversation format.
- Encrypt archives with a passphrase and an age v1 stream. Both formats omit API keys, OAuth tokens, and global preferences outside SQLite; selected-conversation archives also omit linked account/workspace identifiers and projects.
- Inspect and validate archive contents and integrity before restore. Detect duplicate conversation IDs and offer skip-existing or import-as-copy behavior.
- Restore database rows in one transaction and create attachments exclusively with rollback cleanup so failed imports leave the current profile usable.
- Create an age-encrypted, versioned full-profile archive using SQLite's online backup so committed WAL data is included; archive only attachment files referenced by the snapshot. Validate hashes, database integrity, foreign keys, schema versions, and attachment metadata before scheduling restore.
- Apply full-profile restore during service startup before the database is opened. Keep the old database and attachments under %LOCALAPPDATA%/OpenChat/backups/profile-restores; roll back an unconfirmed restore on a subsequent launch.
- Exclude platform-stored provider credentials, global preferences outside the database, and external model or runtime binaries. Require the user to confirm replacement and close OpenChat before activation.
- Keep Markdown export separate. JSON/ZIP interchange and schema-upgrading restore remain future work.

**Exit criteria for selected-conversation v1:** a user can inspect, export, and restore selected conversations and attachments into another local profile; failed restore is recoverable; and linked credentials remain in platform secure storage.

**Full-profile archive exit criteria:** a user can create a passphrase-protected database-and-attachment backup, validate it before restore, replace the active profile only after explicit confirmation, and recover the previous profile if startup validation fails. This archive does not include provider secrets, global preferences outside SQLite, or local model/runtime files.

## Prioritized roadmap

### 1. Add a first-party tool runtime and safer coding-agent workflows

**Status: implemented for the supported Windows target.** Built-in file, terminal, web, and read-only Git tools have centralized schemas, local Rust executors, validated arguments, bounded operations, and persisted activity. Windows terminal sessions use a temporary AppContainer; file changes have review and conditional revert support. Per-project `Ask`, `Allow`, and `Deny` rules now override the global permission mode for supported built-in tools. Bounded child runs can use a restricted read-only subset of the built-in tools under the same permission boundary. The first-party `run_project_task` executor is advertised only on Windows when an attached project's validated task catalog contains entries; its schema is restricted to declared task IDs, the catalog is read with a 64 KiB plus one byte cap only when the tool can be offered, so an inactive task tool does not block ordinary chat; execution uses the same approval, timeout, output, cancellation, activity, and Windows sandbox path as terminal commands. Windows project options manage local stdio and remote Streamable HTTP MCP catalogs, validate and atomically replace settings, disable saving after load errors, and check an enabled server on demand, including unsaved edits. Remote MCP connections require HTTPS, reject private or reserved destinations after DNS resolution, pin resolved addresses, disable redirects, and send optional bearer credentials from Windows Credential Manager. MCP server and discovered per-tool `Ask`, `Allow`, and `Deny` overrides are stored locally per project, outside the repository catalog; server `Deny` takes precedence. Client and service permission limits accommodate the maximum configured set of eight servers with up to 128 tools each. During chat, discovered MCP tools use namespaced provider schemas, per-call approval unless local settings allow or deny them, persisted activity, and untrusted-result handling. MCP environment-variable names stay in the project catalog while their secret values are scoped to project, server, and variable in Windows Credential Manager; stdio secrets enter only the sandboxed child environment and remote credentials are sent only as HTTPS bearer authorization. Remote MCP JSON success/error bodies are incrementally capped at 64 KiB, SSE events are bounded before parsing, and transport headers cannot override reserved protocol headers. Versioned project-scoped first-party executors are available on Windows through `.openchat/tools.json`, with schema validation, per-tool permissions, AppContainer execution, bounded input/output, timeout, cancellation, and persisted activity. Project instructions, reusable project Skills, profile settings, bounded child runs, and plan-mode restrictions are implemented. Non-Windows process isolation remains future platform work because Windows is the only supported desktop target.

- Keep LSP diagnostics and navigation as a separate code-intelligence capability. Non-Windows process isolation, browser/computer control, and other desktop-platform executors require separate platform support and security validation before being enabled.

**Exit criteria:** met for the supported Windows target. Built-in, user-configured, and MCP tools use explicit permission decisions and report their activity consistently; local process tools run inside an enforced sandbox; file edits can be reviewed and reversed; and denied, failed, timed-out, and cancelled calls have clear outcomes in the UI.

### 2. Add project profiles and incremental code context

**Status: implemented in source.** Project-scoped instructions are editable from project options and saved to `.openchat/instructions.md`. Project Skills in `.openchat/skills/<id>/SKILL.md` can be selected per project, previewed in project options, and are included in new provider requests. Skill IDs, UTF-8 content, project containment, per-file and total sizes, checkpoint restoration, and stale selections are validated. Skill selections are stored in local preferences outside repository catalogs and conversation/profile archives. Projects can save a local provider/model route and supported reasoning effort as their default for new conversations; the model picker marks the effective project default, existing conversations retain their saved route, and missing project models fall back to the global default. The optional incremental index is stored in the local cache, follows `.gitignore`, applies bounded scanning and redaction, and adds matching excerpts to new requests. Its options report indexed state and support enable, sync, and clear operations. Pending question runs retain the excerpts used by their initial request.

- Allow a project to select default provider/model, reasoning preference, instructions, tool policy, and working directory. Provider/model and reasoning defaults are implemented in local preferences and apply to new conversations started from the project when the model still supports the saved effort.
- Support project-scoped Skills and make the effective global, project, and Skill instruction sources inspectable. Skills are selected from project files and previewed in project options; shared and project instruction sources remain editable in Settings and project options.
- Add an incremental project index that respects `.gitignore`, reports stale or unavailable state, and can be disabled or cleared per project. Implemented with bounded scans, a local FTS5 cache, explicit project controls, and matching excerpts in requests.
- Keep LSP symbol navigation and diagnostics as a separate code-intelligence capability that can use the project index where useful.
- Make profile precedence visible when global and project settings both apply. The model picker labels the project default when it overrides the global route.
- Add a dedicated Projects tab for browsing and creating projects and starting project chats, rather than putting project creation in the composer project selector.

**Exit criteria:** a project can restore its own model and tool defaults when reopened; ignored files stay out of project indexing; stale or disabled indexes are shown explicitly.

### 3. Add Git-aware tasks and isolated workspaces

**Status: implemented in source.** Structured read-only Git status, staged/unstaged diff, changed-file, and history tools are available. Projects can declare named terminal tasks that run through the existing approval and process boundary. Users can create, inspect, open as a project, and remove isolated Git worktrees. The worktree view lists tasks from that worktree's `.openchat/tasks.json`, shows the exact command and timeout for confirmation, and displays cancellable execution status, exit code, and bounded output. Project-level deny rules are enforced by the service. A user can inspect both the worktree diff and named-check result before choosing to use the worktree, remove it, or continue working in it.

- Add structured read-only Git status, diff, changed-file, and recent-history views.
- Let users declare named project tasks such as test, lint, or build. Run them through the existing terminal boundary with the configured approval and output limits. The version 1 `.openchat/tasks.json` catalog is validated and task commands are displayed before approval.
- Offer a Git worktree for isolated agent tasks after the existing file permission and patch review controls are in place. The current worktree lifecycle retains branch commits on removal and warns that uncommitted/untracked worktree files are deleted.
- Show the task diff and check results before the user decides whether to keep, discard, or continue the worktree.

**Exit criteria:** repository state is visible without parsing arbitrary shell output; named tasks have bounded execution and clear exit status; isolated work can be reviewed before it is retained.

### 4. Add usage tracking and cost visibility

**Status: planned.** Help users track actual provider and local-model usage and manage spending.

- Store per-request provider-reported input, output, reasoning, cache-read, and cache-write counts when available, plus cost when known, first-token latency, total latency, throughput, retries, and tool outcomes. Normalize provider fields while retaining their source and avoiding double-counting cache tokens in total input.
- Attribute usage to connection/account, workspace, conversation, project, model, run, and subagent. Show plan quotas and API usage limits only when the provider supplies them; include freshness and reset information when available.
- Show actual provider usage separately from estimated context use and cost. Keep cached-token cost separate from plan quota semantics, which vary by provider and authentication route. Represent missing usage, pricing, or quota data as unavailable rather than inferring it.
- Calculate cache-hit ratios from provider-reported usage using that provider's documented denominator. Show the ratio's source and aggregation window, and avoid direct comparisons when providers or harnesses use different denominators or session scopes.
- Show usage with budget thresholds and limit warnings, including cache hit/write counts and effective cost where the provider reports enough data to calculate them.
- Add local model benchmarks and memory estimates while distinguishing measured values from estimates.

**Exit criteria:** usage records identify their connection, provider/model, run, and whether values are reported or estimated; cache reads/writes are not double-counted; and unavailable fields remain explicit.

### 5. Add message branches and durable runs

**Status: implemented in source and covered by deterministic tests.** User messages can be edited into a new conversation branch that copies the prior transcript and attachments while retaining the original. Adjacent retry responses can be selected as versions. Each assistant response stores its provider and model, and a selected version displays its own route. Runs and checkpoints are persisted for chat requests and goal/question continuation, with interrupted runs represented explicitly. The sidebar run manager lists running, paused, and interrupted runs across conversations, opens the related conversation, and the UI notifies users when a run ends in another chat. ChatGPT OAuth and OpenAI Responses API-key requests use their Responses routes for bounded child runs. Gemini, Groq, Cerebras, OpenRouter, and Mistral API-key requests use Chat Completions for bounded child runs when the selected model supports tools. Local llama.cpp requests use the selected model's loopback Chat Completions endpoint, with its configured local API key when present, if the model supports tools. Child runs inherit the selected model and reasoning setting; OAuth runs also inherit speed and workspace settings. Child runs have a read-only built-in tool allowlist, request/tool/output/time/concurrency caps, cancellation, durable run IDs, and separate usage attribution. MCP tools and provider-hosted tools are not available in child runs. The UI shows child objectives and final status/result, with live phase updates while the run manager is open and the last saved phase available when it opens later. On Windows, child runs can invoke user-declared named project tasks through the same permission rules and AppContainer process sandbox as the parent. Arbitrary shell commands, direct file writes, MCP tools, and provider-hosted tools remain outside the child allowlist.

- Support editing a user message and starting a branch from that point while preserving the previous message and response history.
- Keep regenerated or alternative model responses as versions that can be compared and selected; preserve provider/model provenance for each version.
- Model generation and tool execution as durable runs with run IDs, cancellation, provider concurrency limits, and explicit retry semantics.
- Allow users to switch conversations while a run continues, inspect pending runs, and receive completion notifications.
- Recover or clearly mark interrupted runs after service or app restart; never retry an ambiguous provider request automatically.
- Represent subagents as child runs with their own context, selected or inherited model, tool allowlist, permission scope, and usage attribution. Show child progress, tool activity, results, and the parent's summary in the UI.
- Bound subagent concurrency, time, and provider usage; propagate cancellation and report partial or failed child results without treating them as completed work. Require the first-party tool permission and process-isolation boundary before enabling local tools in child runs.

**Exit criteria:** branches and response versions remain addressable; run state survives restart or is reported as interrupted; provider limits and cancellation apply per run; child runs cannot exceed their declared tool, concurrency, or usage bounds.

### 6. Add reusable workflows and lifecycle hooks

**Status: planned.** Make repeated work easy to launch while keeping deterministic actions governed by explicit permissions.

- Add parameterized workflow recipes that package an initial prompt, project context, provider/model choice, enabled tools, and expected output.
- Keep recipes distinct from Skills: recipes launch a configured task, while Skills supply reusable instructions or reference material.
- Add optional hooks for defined lifecycle events such as before/after a tool call or after a file edit. Show installed hooks, scope, actions, and failures.
- Require explicit user configuration for hooks with side effects; apply timeouts, output limits, cancellation, and permission checks.

**Exit criteria:** a recipe can be reviewed and re-run with its settings visible; hooks run only for their declared events and scopes; failures are reported without hiding the original task result.

### 7. Broaden API-key provider access

**Status: implemented in source and covered by deterministic tests.** Custom OpenAI- and Anthropic-compatible endpoints and presets for DeepSeek, xAI, Perplexity, Together AI, SambaNova, Fireworks AI, DeepInfra, DashScope (Qwen), and Cohere are implemented in the local Rust service and React UI. Dedicated Ollama and LM Studio provider connections are planned. Live account validation remains open.

- Add a user-configured OpenAI-compatible endpoint with a base URL, API key, catalog refresh, and manual model entry when a catalog is unavailable. Implemented with support for both OpenAI and Anthropic protocol targets.
- Treat context limits, streaming, reasoning, vision, tool calling, and prices as model capabilities. Implemented with per-provider capability mappings and default limits.
- Add a DeepSeek preset using its documented API and verify streamed responses, reasoning output, and tool calls through the shared provider contract. Implemented.
- Extend OpenRouter beyond its current zero-price model filter. Show current input/output prices and supported capabilities for priced models, and make the cost implications clear before a user selects or calls one.

**Exit criteria:** a user can configure a compatible endpoint or connect DeepSeek without exposing credentials; the model selector only offers verified capabilities; and OpenRouter model pricing and tool support are visible before use. Met in source implementation; live verification remains open.

### 8. Add provider-native API capabilities

**Status: partially implemented in source.** Anthropic Claude Messages API adapter is implemented (`/messages` route, header-based auth, streaming, tools, vision, and reasoning). Public OpenAI Responses API remains planned. All providers use OpenChat's built-in local `web_search` and `read_url_content` system rather than provider-hosted search tools.

- Add the public OpenAI Responses API for OpenAI API-key connections, including streaming and function calling. Keep this route distinct from ChatGPT OAuth and its private Codex endpoints.
- Evaluate Anthropic Messages and xAI Responses as separate provider adapters, with model catalogs and capabilities obtained from their documented APIs. Anthropic Messages adapter is implemented in source and covered by deterministic unit tests.
- Treat prompt caching as a provider/model/runtime capability. Keep reusable instructions and tool schemas stable at the start of requests; add documented cache breakpoints or keys only in adapters that support them. Adapters can shape cache eligibility, but the provider/runtime owns cache storage and actual hits, and a marker or key does not guarantee reuse.
- Handle streaming, cancellation, tool-call errors, usage, context limits, prompt-cache reporting, and unavailable capabilities consistently across every adapter.

**Exit criteria:** each released native adapter passes the provider-specific account checks and shared streaming/tool/error contract.

### 9. Build a searchable conversation library

**Status: implemented in source.** Message and completed tool activity search supports provider, model, project, archive-status, date, and tag filters. Per-conversation tags, bookmarks, archive/unarchive, saved searches, transactional multi-select move/archive, encrypted selected-conversation bulk export, and opt-in local text-attachment indexing are implemented.

- Sidebar search covers indexed messages and completed tool activity, with snippets, match highlighting, lazy indexing of opted-in conversations, navigation to the matching message, and optional inclusive local-date, assistant-message provider/model, project, archive-status, and tag filters.
- Keep attachment-content indexing separately configurable. Implemented as off by default; users can rebuild or clear the derived local index in Settings > Local data. It indexes supported UTF-8 text attachments only, caps the index at 5,000 files or 100 MB, and never changes source files. Images are excluded because OCR is not available.
- Tags are stored per conversation, limited to 12 labels of 32 characters each, and shown accessibly in the sidebar. Bookmarks are stored per conversation and exposed with an accessible sidebar indicator and menu action. Up to 20 named searches preserve their query, filters, and date range in local preferences. Multi-select can move active conversations between projects and the general chat list or archive them atomically. The encrypted conversation archive supports exporting multiple selected conversations.
- Keep the existing per-conversation memory search as a focused context tool; make the broader library search a separate history workflow.

**Exit criteria:** search results identify the source conversation and message, can open that message directly, and honor the user's indexing and archive settings. Met by source behavior and deterministic tests.

### 10. Add citations and request-data visibility

**Status: implemented in source; live-provider validation remains open.** Built-in web-search and page-read results include local source IDs, retrieval times, and source cards linked from valid assistant citations. All providers execute `web_search` and `read_url_content` through OpenChat's local web tool engine. ChatGPT OAuth Responses URL annotations, OpenAI API-key Chat Completions URL annotations, OpenCode Responses API completed-output URL annotations, and OpenRouter URL citation annotations are stored per assistant message and linked to source cards with provider-supplied titles and URLs. Valid provider text end offsets place source links after the cited text; missing or invalid offsets place links at the end of the response. No retrieval time is shown because the responses do not provide one. Groq URL citations are stored as provider-native sources. The Gemini stream parser recognizes Google `groundingMetadata` web chunks and maps byte-based grounding segment offsets to visible source anchors when such metadata is present. Request records keep a privacy-preserving manifest of message-role counts, image/tool-result counts, instruction byte counts/source categories, tool names, cache-control field names, included message IDs, archived message IDs, the compaction boundary, and bounded attachment name/type/ID metadata. Exact request contents and attachment bytes are not retained. Usage request details and CSV exports include the durable run ID when available, including child-run attribution. Other provider formats are connected only when their documented response shapes are supported by deterministic parser tests.

- Store web and provider-returned sources as structured records linked to the response that used them.
- Add citation anchors in assistant output and let users open the source card with title, domain, URL, and retrieval time.
- Show which provider received the request and which messages, attachments, instructions, and tool results were included.
- Show which tool definitions and provider-side cache controls were sent when the adapter exposes them; distinguish cache usage reported by a response from cache behavior that cannot be observed.
- Keep provider-hosted sources distinct from local web_search and read_url_content sources.

**Exit criteria:** displayed citations resolve to stored source records; the user can inspect the request route, included data, and reported usage for a response; missing provenance or provider visibility is shown as unavailable.

### 11. Expand document input

**Status: implemented in source and covered by deterministic tests.** Common research and work documents (PDF, DOCX, XLSX, CSV, and HTML) are extracted locally with bounded parsing and source location tracking (page, sheet, cell/row, and section references). Audio, video, and image OCR are excluded by product decision.

- Extract text from PDF, DOCX, XLSX, CSV, and HTML documents locally, including page, sheet, and cell/row references where available.
- Keep extracted content local by default; show size, extraction status, and which provider receives the resulting content.
- Apply archive-bomb, malformed-file, memory, time, and output-size limits to parsers.

**Exit criteria:** supported files produce reviewable extracted text with source locations; malformed, oversized, or unsupported files show a specific error and recovery path.

### 12. Expand local and managed provider connections

**Status: planned.** Add dedicated Ollama and LM Studio local-server connections. Keep them separate from OpenChat-managed runtimes.

- Add dedicated Ollama and LM Studio connections for health checks, model discovery, and documented model lifecycle controls. Require an explicit local server address, show connection failures clearly, and keep these distinct from OpenChat-managed llama.cpp installation.
- Evaluate Amazon Bedrock Converse/ConverseStream with regional configuration, AWS authentication, model availability, and tool-use capability checks.
- Evaluate Microsoft AI Foundry endpoints with deployment identifiers, supported authentication modes, and per-model API compatibility.
- Reuse the shared provider contract and make region, deployment, privacy, pricing, and unavailable capability information visible.

**Exit criteria:** each connection reports supported models and capabilities from its documented endpoint; credentials stay in secure storage; unsupported models and regions fail with actionable errors.

### 13. Establish an accessibility and locale quality matrix

**Status: in progress.** Tool approvals, project permission controls, worktree controls (including named-task selection and command confirmation), the run manager, provider citations, conversation selection/bookmark labels, the chat composer send and stop actions, stop-disabled state, assistant response-version controls, and the localized editable name of the composer message field have semantic coverage across five locales and narrow/wide layouts. Temporary-chat notices and their end action now have locale and width coverage from 320 logical pixels through desktop; the compact empty-chat guide scrolls without losing its setup steps or composer, and its step labels wrap at narrow widths. Citation source actions open their stored source card by keyboard in all supported locales. The conversation sidebar and response-version selector tests cover widths down to 320 logical pixels and text scales through 200%; keyboard-only activation tests cover conversation selection, citation opening, tool approval and denial, response-version selection, and composer sending. The worktree and run-manager matrices also cover simulated display pixel ratios through 200% and reduced animation settings. System high-contrast light/dark themes and key text/focus contrast have focused checks. Flutter widget coverage also exercises settings, model routes, active messages, chat controls, and keyboard-revealed conversation actions at narrow and enlarged layouts. Native Windows UI Automation/screen-reader workflows and real Windows display-scaling validation remain open; widget tests do not substitute for those desktop checks.

- Cover keyboard-only navigation, focus visibility and restoration, screen-reader labels, high contrast, and reduced motion.
- Test text scaling, 125–200% Windows display scaling, narrow layouts, long translations, and input composition.
- Add semantic accessibility assertions alongside screenshot tests for Turkish, English, German, Spanish, and French.

**Exit criteria:** core chat, settings, model selection, tool approval, and local-model workflows pass the accessibility matrix in all supported locales.

### 14. Add signed distribution and safe updates

**Status: later.** Establish publisher authenticity and recovery before broad public distribution.

- Sign Windows executables and release manifests; publish checksums and a software bill of materials.
- Define stable and preview channels, supported-version policy, and update metadata.
- Verify update signatures and payload integrity before installation.
- Preserve the previous working bundle and provide a user-visible rollback path.

**Exit criteria:** users can verify the publisher and release contents; interrupted or invalid updates do not replace the working version.

### 15. Expand desktop platforms

**Status: later.** Windows remains the supported first target. Bring the local Rust service and credential storage to Linux and macOS before mobile.

- Add platform-specific packaging, secure credential storage, data-directory resolution, and service lifecycle handling.
- Preserve the same SQLite conversation format and migration behavior across desktop platforms.
- Document installation, upgrade, and recovery paths for each supported platform.

**Exit criteria:** each desktop build packages its service, stores secrets in the platform's secure store, and keeps durable application data in that platform's user data directory.

### 16. Evaluate mobile support

**Status: later.** Android and iOS require a service lifecycle and secure-storage design that fits mobile process and permission limits. Start this work after desktop provider behavior is stable.

- Decide whether a mobile client is in scope and how it would invoke the Rust provider layer while handling app lifecycle changes.
- Define local data and credential handling for Android and iOS without assuming the desktop child-process model.
- Keep the UI responsive to mobile layouts, keyboards, connectivity changes, and interrupted background work.

### 17. Package and distribute extensions

**Status: local app-wide MCP templates and user-pinned signed metadata sources implemented in source; independent publisher verification and package distribution remain later.** Settings can save, edit, search, and remove reusable MCP connection templates. User-added sources use a pinned Ed25519 key and bounded, user-initiated HTTPS retrieval. The app verifies catalog integrity against that key, not the publisher's real-world identity. Signed entries are limited to Streamable HTTP and are imported as templates. Adding any template copies it to the active project as a disabled MCP server, records `Ask` for its tools, and leaves credentials scoped to that project. Catalog templates do not download or execute software.

- Define versioned manifests with declared tools, permissions, provider requirements, and configuration needs.
- Extend the local template catalog with safe update and removal flows for servers already installed in the active project.
- [x] Add user-pinned Ed25519 source verification for Streamable HTTP connector metadata; publisher identity remains unverified.
- [ ] Define and review a package installation path before distributing executable connector packages. Signed metadata sources do not fetch binaries.
- Keep package permissions separate from the user's built-in tool permissions and require explicit enablement.
- Keep catalog entries disabled by default; listing or installing one must not connect it, grant credentials, or send conversation data.
- Keep connector configuration, permissions, and credentials scoped to each project when adding a catalog entry to that project.
- Require renewed consent when an update expands a package's declared capabilities. Removing a package must disable it and remove its local data and stored credentials, with cleanup failures shown clearly.

**Exit criteria:** verified packages are version-pinned, their requested access is visible before installation, and removal leaves no active server or hook behind. These criteria are not yet met for third-party package distribution.

### 18. Add user-controlled encrypted sync

**Status: later.** Offer optional multi-device data movement while preserving OpenChat's local-first model.

- Sync encrypted archives to a user-selected folder or compatible storage service without introducing a central OpenChat account.
- Define device keys, recovery, conflict handling, attachment deduplication, and offline behavior.
- Keep API keys, OAuth credentials, and other provider secrets out of synchronized data.
- Build on the portable archive format and integrity checks from the backup stage.

**Exit criteria:** sync is opt-in, users control the destination and recovery material, conflicts are reviewable, and credentials never leave platform secure storage.

## Product experience and capability audit

This section records user-facing gaps identified by the October 2026 product
and UX review. Competitor documentation is used to discover established
workflows, not as a commitment to match every hosted product. Keep OpenChat's
local-first boundary and Windows-first release scope unless a roadmap item
explicitly changes them. Existing work is tracked under priorities 4, 6–8,
11–13, and 15–18; the items below cover gaps not already represented there.

### 19. Guide first-time setup and provider recovery

**Status: partially implemented in source, P0.** The empty-chat screen shows a
four-step connection, provider catalog verification, model selection, and
message guide. Its active step advances as a provider is configured, a catalog
loads, and a model is selected; completed steps are announced accessibly.
Loading, catalog failure, and missing-model states have direct recovery actions.
API-compatible and OpenCode Console connection cards distinguish a saved key
from successful provider authentication and a usable model catalog.
Authentication rejection, rate limits, network failures, and empty catalogs
have separate recovery copy; cached catalogs never count as successful
verification, and quota is marked unavailable in OpenChat where no quota query
exists. ChatGPT OAuth verification forces a model-catalog refresh for the
selected account and workspace; each saved ChatGPT API key can also be checked
against a freshly requested model catalog without displaying its secret.
API-compatible and OpenCode Console checks now retain their result and check
time in local settings; changing or removing their saved key clears the old
result. ChatGPT OAuth results are scoped to the account and workspace, while
ChatGPT API-key results are scoped to each saved connection. Removing either
connection clears its verification record. Regular chat request outcomes and
timestamps are stored locally per provider route, account, and workspace, and
appear beside the matching connection's verification state. Temporary chats do
not update this durable summary. The record contains only a bounded outcome enum
and timestamp; it excludes prompts, provider error text, and credentials.
Failures to save a regular-chat summary are reported in chat without changing
the provider response. The empty-chat guide now explains ChatGPT sign-in,
provider API keys, and available local models. It distinguishes saved provider
credentials from a reachable connection and directs users to verify or refresh
the catalog before model selection. A stale cached catalog keeps the guide on
the verification step, announces that it could not verify the connection, and
offers a direct refresh action. Freshness is tracked per provider/model route,
so a cached list for another ChatGPT account does not mark a currently selected
route stale; with no selection, verification is requested only when every
available route is stale. Focused route-selection tests cover both cases. The
fourth “send” step stays visible after model selection until a completed
assistant response exists in durable history. Returning profiles with an
earlier completed response keep the direct-to-chat welcome state; temporary
chat responses do not alter that history. The default OpenCode route no longer
counts as a configured credential by itself. Repository and ChatScreen tests
cover new profiles, returning profiles, and the step's accessibility at 320 px
with 200% text scaling in all five supported locales.
Assistant failures caused by rejected credentials now include a direct action
to open Connections; rate-limit and network errors retain their retry path. A
widget regression test now follows the connection, verification, available
model, and ready-to-chat states in one sequence. The first real response still
needs an end-to-end run on a clean profile with a configured provider account.

- Provide a first-run path that explains connection choices, configures or
  connects one provider, verifies its authentication and model catalog, helps
  select a usable model, and opens the first chat. The choices and recovery
  guidance are now present on the empty-chat screen; full clean-profile
  validation against a real provider remains open.
- Show provider state as distinct steps: credentials configured, connection
  verified, catalog available, quota known or unavailable, and last request
  outcome. The connection cards now show verification and last request state,
  and the empty-chat guide exposes the send step. Never present saved credentials
  as proof that a provider is working.
- Give each failed step a localized explanation and a direct recovery action,
  such as reconnect, refresh the catalog, select another model, or open the
  relevant provider settings.
- Keep setup optional after the first connection and preserve the existing
  direct-to-chat path for returning users.

**Exit criteria:** a clean profile can reach a first response through the
guided path; invalid credentials, unavailable catalogs, unsupported models,
and provider outages each show the failed step and a usable next action.

### 20. Add private conversations and user-controlled memory

**Status: partially implemented in source, P1.** The temporary-chat retention
contract is documented, randomly generated `tmp_` IDs are rejected by durable
conversation creation, message saving, and the normal chat-send RPC, and an
in-memory session model validates transcript bounds, immutable save snapshots,
and discard behavior. A separate `chat.temporary.send` RPC validates bounded
user/assistant history and in-memory portable attachments, then streams
responses for compatible provider routes without creating conversation, run,
tool, or usage records. Text attachments are included as untrusted data and
images are offered only for models that support them. The chat sidebar can
start a temporary session, the first send explains provider-side retention,
messages and files remain in memory, and leaving offers save, discard, or
  continue. Saving sent messages and attachments is transactional and preserves
  the selected provider route, including ChatGPT OAuth account and workspace.
  The selected ChatGPT OAuth account and workspace are supported on this path,
  with model and image-capability validation and a memory-only response stream.
  Tool calls remain unsupported.
Personal memory now supports user-entered local facts, editing and removal,
with use disabled by default. When enabled, the facts are included as reference
data in new model requests and the settings explain that their text goes to the
selected provider. Users can now persist a per-conversation exclusion from the
chat header; excluded conversations do not include saved facts in requests.
New standard chats expose the same control before the first message. The
pending choice is applied to that first request and saved against the generated
conversation ID before the provider is called. If the privacy preference cannot
be saved, the request is stopped and the empty conversation is removed. The
exclusion decision is captured once at send start and reused for both request
construction and the local retention record. Standard sends also take a
synchronous preparation lock before asynchronous preference and credential
reads, and snapshot the selected route, reasoning effort, profile, project, and
workspace context before those reads can complete.
Temporary sends use the same preparation lock before credential reads and
provider-retention confirmation. Leaving a temporary session is blocked during
preparation and while its provider request is active, preventing the session
from being discarded before its first message is committed in memory.
Each assistant response now stores the IDs of personal-memory facts included
in its request and offers a disclosure that resolves the current saved text.
Edited or removed facts are identified as no longer available, and fact edits
create new IDs so an old response cannot show changed text as its source.
Temporary chats use the selected ChatGPT OAuth account and workspace without
creating local message, run, or usage records. Make local retention and
cross-conversation personalization explicit, separate choices.
The temporary-chat retention contract and provider/tool boundaries are defined
in [temporary conversations](docs/temporary-conversations.md).

- Continue opt-in, local-only personal memory so it can be paused and excluded
  per conversation. Show which saved facts informed a response; do not silently
  infer that archived content should become a durable memory.
- Keep conversation memory search scoped to its current conversation and
  project indexing scoped to its selected project unless the user enables a
  separate cross-conversation memory feature.
- Add a privacy settings group for these controls and make archive/index
  behavior understandable at the point where users enable it.

**Exit criteria:** temporary chats do not enter durable history, search,
project indexes, or exports unless explicitly saved; personal memory is off by
default and every remembered item and source can be reviewed or removed.

### 21. Add reusable assistant profiles and an editable response workspace

**Status: partially implemented in source, P1.** Assistant profiles can be
created, edited, activated, deactivated, and removed in settings. Their name,
bounded instructions, and concise/balanced/detailed response style persist in
local preferences. The active profile's style and instructions are included in
new standard chat requests alongside shared and project-specific instructions;
the active profile is shown in the conversation header before sending. Temporary
chats do not apply profiles, and the settings explain this boundary. Profiles
can save a preferred model from the currently selected available model. That
preference applies to new chats after project-specific defaults and retains its
reasoning effort where supported. Existing conversations keep their saved
model. Explicit per-profile tool allowlists and the editable response workspace
are partly implemented: profiles can restrict built-in tool groups; the service
validates the allowlist, filters advertised tools, and limits the executor. The
profile limit takes precedence over global and project tool grants. When a
project is active, named project tasks and configured project tools can also be
selected in the profile; the service advertises only entries present in that
project's validated catalogs. MCP tool discovery is user initiated, and only
servers containing an allowed tool are connected during the request. An absent
allowlist preserves existing behavior, while an empty list disables tools.
Before sending, the conversation header now shows whether the active profile
sets a tool limit, disables tools, or inherits the app and project permissions.
The accessible label and tooltip clarify that app and project permissions can
further limit a profile's selected tools.
The Saved responses page now opens a side workspace where users can edit a
working copy, preview safe Markdown (including fenced code blocks), save
numbered versions, switch the selected version, and export that version as
Markdown. The source editor is labeled as Markdown and uses a code font.
Edited versions remain linked to their source conversation and
message; the original transcript is unchanged. Schema 16 backfills existing
saved responses as version 1. Source editing now supports syntax highlighting
and language selection for Markdown, Dart, Go, JavaScript, TypeScript, Python,
Rust, JSON, YAML, HTML/XML, CSS, and SQL. Language selection only changes local
editor presentation; it does not alter the saved text or execute code. Profile
creation and removal restore the previous profile list if updating the active
profile fails.

- Add personal assistant profiles that combine a name, instructions, response
  style, preferred model, and explicitly enabled tools. Keep project-specific
  instructions and Skills available as narrower project context.
- Keep assistant profiles distinct from recipes in priority 6: profiles shape
  an ongoing assistant, while recipes launch a bounded, repeatable task.
- Add a side workspace for substantial documents, plans, and code. Support
  focused edits, version selection, and export without replacing the source
  conversation or saved-output collection.
- Show the active profile, model, and tool access before a request is sent.
  The profile's tool scope is shown in the conversation header, and the
  selected model remains visible in the composer. App and project permissions
  may further reduce the profile's tool allowlist.

**Exit criteria:** users can create, edit, select, and remove a profile; a
workspace output retains its link to the generating conversation and selected
version; unsupported editing or export formats fail explicitly.

### 22. Expand library search and reusable file access

**Status: partially implemented in source, P1.** The local-data settings now
offer a user-reviewed support diagnostics export. It contains only an explicit
allowlist of app/runtime, history-storage, service-health, and database schema
fields; the preview and saved JSON are identical. Chat content, credentials,
and filesystem paths are excluded, including path fields returned by the
service health RPC. Cross-surface search now combines conversation history
with project/workspace names, saved responses, and attachment names. Results
show their type; source conversations and messages can be opened directly.
The composer also offers a reusable library of conversation attachments. It
loads recent messages in bounded batches, can continue to older conversation
files on request, and copies a revalidated file into the new message when
selected. Result-type filters are implemented. Project results open the
latest related conversation or start a new conversation in that project.
Workspace results open the workspace organizer and select the matching
workspace; conversations created there retain that workspace assignment. The
dedicated file-library surface lists saved attachments in bounded pages,
filters available and missing files, and opens each file's source message.
Attachment-content indexing is optional, local, bounded, and removable without
deleting source files. Turning it off persists the opt-out before deleting the
derived index; if cleanup fails, the remaining count and retry action stay
available. Unified library queries treat `%` and `_` as literal search text
rather than SQL pattern wildcards. Build on priority 9's
conversation search and priority 11's document ingestion without mixing
unrelated content into chat history results.

- Add direct project/workspace destinations to the unified search entry point.
  Implemented: project results open their latest related conversation or begin
  a project conversation; workspace results open and select the matching
  workspace in the workspace organizer.
- Expand the reusable conversation-attachment picker beyond its bounded recent
  list. Implemented with on-demand message-page loading and explicit retry on
  page failure. The separate file-library surface distinguishes conversation
  attachments from project-indexed content and opens source messages.
- Make attachment-content indexing a separate opt-in control, consistent with
  priority 9; report which file types are indexed and provide a clear way to
  remove derived indexes without deleting original files.
- Keep support diagnostics user-initiated, previewed, and limited to the
  documented allowlist. Exclude prompts, conversation content, filesystem
  paths, and credentials by default. Implemented in local-data settings.

**Exit criteria:** every result identifies its content type and source;
attachment indexing is off until enabled; removing an index leaves original
files intact; support bundles pass a documented data-scope review before
export.

### 23. Make notifications, permissions, and keyboard help configurable

**Status: partially implemented in source, P2.** Settings now persist controls
for background response alerts, pending-question alerts, response-text
previews, and quick reply. Notification delivery reads these preferences before
showing an alert; defaults preserve the existing Windows behavior. Settings
replace response text with a generic body when response previews are off; the
delivery body and persisted defaults are covered by regression tests. A
focused notification-detail test verifies that quick reply adds the localized
input and action only when enabled, with a bounded conversation/message target.
Settings also expose the global tool-permission mode and link to the active
project's rules; the settings summary reports how many project tools are
allowed, require approval, or denied, and clarifies that other tools inherit
the global mode. A searchable keyboard reference lists the send, new-line, and
stop shortcuts that are active in the composer. Windows toast availability now
comes from the Windows `ToastNotifier` setting, with the reported block reason
and a direct link to Windows notification settings. The runner creates or
refreshes a per-user Start menu shortcut carrying the matching AUMID; it reports
notifications unavailable if that registration cannot be written. The app
rechecks the status after returning from Settings. When Windows reports
notifications enabled, the user can request a content-free test toast from
Settings to check actual delivery. Actual toast display can still be affected
by Focus Assist or other system-level presentation behavior.

- Add notification settings for background response completion and pending AI
  questions, including whether previews may show response text and whether
  quick reply is available. Respect operating-system notification permission
  and explain when notifications cannot be delivered.
- Add a settings overview of the global tool-permission mode, with links to
  per-project rules. Keep the composer control as a quick per-chat entry point
  and explain when project rules override the global default.
- Provide a searchable keyboard-shortcut reference. The composer currently
  lists Enter to send, Shift+Enter for a new line, and Esc to stop a response;
  user remapping is not included.
- Group settings by task: appearance and accessibility, chat behavior,
  providers and models, tools and permissions, notifications, and data and
  privacy. Keep search available and avoid adding a sidebar item for every
  individual toggle.

**Exit criteria:** notification changes are persisted and reflected in actual
delivery; permission summaries match the effective global/project rules; all
listed shortcuts are keyboard-operable. Windows-reported notification
availability, recovery navigation, keyboard help, and permission summaries
are implemented. Native presentation under Focus Assist and other system-level
notification modes remains a manual Windows validation item.

### 24. Evaluate sharing and first-party connectors within the local-first model

**Status: local encrypted file transfer and an application-wide connector catalog selected.** Alican chose local,
encrypted archives for conversation sharing. Hosted links, hosted conversation
storage, and multi-user collaboration are outside the current product scope.
Readable Markdown exports remain separate, explicitly user-directed exports;
they are not encrypted by OpenChat. The current transfer formats and
project-scoped MCP/tool boundaries and the selected local catalog scope are
documented in [sharing and connector boundaries](docs/sharing-and-connectors.md).
Connector installation restores the previous project permission map if saving
the project server catalog fails. Signed catalog refresh saves entries before
advancing the source revision and restores the previous entries if that source
metadata update fails.
The install confirmation also explains the endpoint or local process, the
project-scoped authentication path, what a signed-source refresh changes, and
what removing a local template leaves in project settings. These disclosures
are localized and covered across the supported locales.

- Keep conversation sharing in the existing encrypted archive flow. Review
  selected conversations and attachments before export; the passphrase stays
  user supplied and is never stored by OpenChat.
- Continue priority 17's application-wide catalog from locally defined MCP
  templates to signed, versioned metadata. Before installation, show
  each connector's publisher, requested access, destination, data sent,
  authentication method, update behavior, and removal behavior. Catalog
  retrieval may not imply hosted execution or send conversation content.
- Do not add hosted conversation links or live multi-user collaboration under
  the current local encrypted transfer decision. Revisit only after explicit
  product approval and a service, access, deletion, and abuse response model.

**Exit criteria:** encrypted local archives remain the conversation-sharing
path; connector entries are verified, opt-in, and show their data-flow and
permission model before use; no feature implies cloud sync or public access.
The user-managed local template catalog and pinned-key signed metadata sources
are implemented, but independent publisher verification and executable package
distribution remain open. Hosted sharing
remains out of scope under the current product decision.

### 25. Add persistent project agent memory and semantic context engine

**Status: planned, P1.** Maintain cross-session agent understanding, learned project conventions, and architecture decisions bounded per project.

- Add a dedicated `project_memory` storage layer scoped to each local project, keeping facts, architectural rules, and operational constraints persistent across conversation sessions.
- Automatically construct and inject project-relevant memory into new agent runs while strictly enforcing project isolation so memories never leak across distinct workspaces.
- Support hybrid lexical (FTS5) and semantic vector retrieval over project history and codebase context for fast, targeted recall.
- Allow users to inspect, curate, export, and selectively purge learned project memory items directly from project settings.

**Exit criteria:** agent runs preserve verified project conventions across session boundaries; memories are strictly isolated to their originating project; users can inspect and delete stored items; and memory injection does not degrade token budgets on small-context models.

### 26. Add task-isolated subagents

**Status: planned, P1.** Enable main agent runs to delegate bounded, heavy, or long-running tasks into isolated child processes without context pollution.

- Support delegating exploratory code searches, large test suites, or modular implementations to dedicated background subagents (`fork_task`).
- Hand off results through structured summaries, diffs, and artifact envelopes rather than forwarding complete conversation histories, keeping the parent context clean.
- Permit parallel child task execution for independent operations with strict per-task concurrency limits, cancellation propagation, and timeouts.

**Exit criteria:** child runs operate within isolated execution scopes; parent context windows remain uncluttered by child turn history; cancellation cascades properly; and completed results report clean diffs and outcomes.

### 27. Add deterministic lifecycle hooks and self-healing lint feedback

**Status: planned, P1.** Provide deterministic runtime interceptors before and after tool execution to enforce safety and automate verification.

- Add `PreToolUse` hooks to evaluate arguments against local deterministic policy before execution, instantly blocking disallowed shell commands or paths without LLM reasoning tokens.
- Add `PostToolUse` hooks that run configured linters, formatters, and type checkers (`cargo fmt`, `eslint`, `prettier`) immediately following file modifications.
- Feed actionable linter and test errors directly back into the agent's steering queue as immediate self-healing input when a modification breaks build contracts.

**Exit criteria:** hooks execute deterministically without consuming LLM inference; post-edit formatters maintain project style; and syntax/type regressions trigger instant self-correction turns.

### 28. Add Tree-sitter codebase mapping and glob-scoped rules

**Status: planned, P2.** Improve repository awareness and rule precision through structural parsing and path matching.

- Generate compact, token-efficient repository maps using Tree-sitter to index class hierarchies, function signatures, and interface definitions across the project.
- Expose the structural repo map to model planning so the agent understands cross-module architecture without reading entire files.
- Support glob-scoped rules (`.openchat/rules/**/*.mdc` or scoped instructions) that activate only when touching matching file paths, avoiding global prompt overhead.

**Exit criteria:** repository maps accurately reflect code structure within a bounded token budget; path-specific rules activate only for relevant target files; and rule parsing remains fast and local.

### 29. Broaden cross-platform kernel-level sandboxing and network policy

**Status: planned, P1.** Bring Linux and macOS process isolation up to Windows AppContainer and WSL2 container standards.

- Implement Linux kernel isolation using Landlock LSM and Seccomp-BPF for unprivileged filesystem path confinement and syscall filtering.
- Implement macOS process sandboxing using Apple Seatbelt (`sandbox-exec` profiles) to enforce restricted directory and device access.
- Add egress network policy with domain allowlisting so sandboxed terminal processes can access legitimate package registries (`crates.io`, `npmjs.org`) while blocking arbitrary external outbound connections.

**Exit criteria:** terminal tools run under native kernel isolation across Windows, Linux, and macOS; outbound network access is strictly bounded by policy; and unauthorized path access is blocked at the OS kernel boundary.

### 30. Add visual multi-file diff inspection with manual per-file approval in side panel

**Status: implemented in source and covered by deterministic tests.** Enhance developer control during code modifications by introducing an explicit right-side review panel where every file change supports granular user approval (`Approve` / `Reject`), batch actions (`Approve all` / `Reject all`), unified and split diff views, and hunk-level rejection.

- Provide a dedicated right-side changes panel in the UI displaying all staged and generated file modifications resulting from an agent turn.
- Require manual, per-file user confirmation (`Approve` / `Reject`) before any AI-generated change is finalized in the baseline or reverted on disk.
- Enable selective hunk rejection within the side panel, allowing users to inspect unified/split diffs and approve specific change blocks while discarding others.

**Exit criteria:** no multi-file AI modification is finalized without explicit per-file user consent; the right-side review panel accurately renders file-level diffs; and rejected files or hunks restore target workspace lines cleanly.

### 31. Add multi-chat tabs and split-view workspace

**Status: implemented in source and covered by deterministic tests.** Enable side-by-side chat workspace with direct sidebar drag-and-drop.

- Allow users to drag any conversation from the sidebar onto the chat workspace to open it (as primary if no conversation is open, or as secondary side-by-side chat if a primary chat is active, capped at 2 concurrent chats).
- Render the secondary split chat pane with the exact same component hierarchy, design tokens, typography, message list, and composer card styling as the primary chat view.
- Support an adjustable split-view ratio (30% to 70%) with pane swapping, promoting to primary, and header toggle actions.

**Exit criteria:** dragging a conversation from the sidebar opens it as primary or secondary; secondary pane matches primary design and composer capabilities; at most 2 chats open side by side; and split view resizes smoothly.

### 32. Migrate full conversation listing to sidebar navigation

**Status: implemented in source.** Full conversation history, project groupings, pinned chats, and recency categories are organized in the collapsible left sidebar with real-time search filtering, quick actions, drag-and-drop, and context menus. The top header displays only the active conversation title, active model and status indicator, and clean window and tab actions.

- Move the entire conversation list, history, and project groupings into a collapsible left sidebar, freeing vertical space in the main workspace.
- Group conversations vertically by project, recency, tags, and pinned status with fast search and action menus per item.
- Restrict the top header strictly to open, active working tabs (as defined in item 31), closing the tab without removing the conversation from the sidebar history.

**Exit criteria:** all historical chats and project hierarchies are easily navigable in the left sidebar; the top header displays only actively open tabs; and closing an active tab does not delete or alter conversation persistence.

### 33. Add AI-assisted visual design canvas and component generator

**Status: planned, P2.** Introduce a Figma-like interactive design surface where AI models can visually generate, inspect, and export modern UI components.

- Provide an interactive, infinite visual canvas for prototyping screens, component layouts, and design-system tokens directly within OpenChat.
- Support bidirectional generation: convert natural language prompts into live vector UI frames, and export visually selected canvas elements directly into clean React, HTML/Tailwind, or SVG code.
- Implement token-aware visual theming: define and manipulate color palettes, typography scales, spacing tokens, and responsive breakpoints visually, keeping generated code strictly bound to design-system standards.
- Enable direct canvas commenting and visual revision loops, allowing users to point-and-click to request layout or styling changes from the agent.

**Exit criteria:** users can generate and manipulate UI layouts on an interactive canvas; exported component code matches design-system tokens without arbitrary values; and changes can be inspected visually before merging into project files.

### 34. Add embedded browser tab in side panel for agent navigation and visual verification

**Status: implemented in source.** Integrate an embedded web browser directly into the right-side inspection panel alongside file changes, using an isolated session that never reads personal browser cookies and bridges to the user's OS default browser when requested.

- Provide a dedicated Browser tab in the right-side workspace panel, allowing live website navigation, local dev server rendering (`localhost`), and real-time frontend verification.
- Provide an isolated cookie/session mode selector (stateless ephemeral vs. isolated OpenChat profile) with one-click session clearing, plus a direct action to launch URLs in the user's OS default browser (`browser.open_default`).
- Include a local page inspector (`browser.inspect_url`) that extracts headings, links, HTTP status, and Markdown content even when a target site sends `X-Frame-Options` or restrictive CSP frame headers.
- Enforce strict network and iframe policies so embedded browsing honors the application's outbound security boundaries.

**Exit criteria:** the embedded browser renders smoothly within the right-side panel; users and agents can inspect DOM structure and launch the default browser cleanly; and personal browser cookies remain strictly isolated.

### 35. Add native computer use automation for desktop apps, mouse, and keyboard

**Status: planned, P2.** Equip the agent with system-level computer use capabilities to perceive screens and interact with native desktop applications.

- Implement the perception-action loop (`screenshot` -> `reasoning` -> `mouse_move` / `click` / `type` / `keypress` / `scroll`) modeled after Claude and Codex computer use specifications.
- Provide OS-level mouse and keyboard input simulation (via native Win32 SendInput APIs on Windows and virtual input servers on other platforms).
- Implement an explicit emergency abort shortcut (e.g. `Esc` or moving mouse to screen corner) that instantly disengages agent input control.
- Enforce granular user permission gates before initiating computer use sessions, keeping sensitive applications (passwords, banking, system settings) bounded.

**Exit criteria:** the agent accurately translates visual desktop state into targeted mouse and keyboard actions; input coordinates scale reliably across multi-monitor display resolutions; and emergency abort immediately revokes control.

### 36. Add smart ghost-text suggestions with one-key auto-completion

**Status: implemented in source and covered by deterministic tests.** Accelerate conversational flow by predicting contextual follow-ups directly in the composer.

- Display context-aware placeholder suggestions ("ghost text") in the message input field based on the assistant's latest response (e.g. "Run tests", "Apply changes", "Explain details").
- Enable instant insertion and submission when the user presses `Tab` or `Shift+Enter`, clearing the ghost text immediately when the user starts typing custom text.

**Exit criteria:** ghost-text suggestions match response context; pressing Tab reliably populates and dispatches the prompt; and manual typing dismisses the suggestion without glitching.

### 37. Add drag-and-drop file and image context ingestion

**Status: implemented in source.** Enable seamless file sharing from the desktop into the conversation thread.

- Allow users to drag and drop arbitrary files, folders, error logs, or screenshots directly into the active chat window or input composer.
- Automatically route images to multimodal models, and extract or reference code and text files as structured context attachments.

**Exit criteria:** dragged files are validated by size and type, uploaded or referenced smoothly, and visible in composer attachments before dispatch.

### 38. Add interactive message editing and branching (Edit & Fork Turn)

**Status: planned, P1.** Permit editing past user messages without destroying earlier conversational history.

- Provide an inline `Edit` action on prior user messages that forks the conversation into a new version branch.
- Allow switching between sibling branches (`v1 / v2`) with left/right arrows, preserving the complete transcript tree.

**Exit criteria:** editing an earlier turn forks cleanly without data loss; branch navigation preserves all subsequent assistant outputs; and branching state persists in SQLite.

### 39. Add quick clipboard copy and multi-format conversation export

**Status: implemented in source and covered by deterministic tests.** Make assistant answers easily portable into other applications.

- Add dedicated copy actions on code blocks and assistant messages for `Clean Markdown`, `Code Only`, or `Plain Text`.
- Support one-click conversation export to formatted Markdown (.md), clean HTML (.html), and structured JSON (.json) transcripts.

**Exit criteria:** clipboard actions copy the exact intended format; exports produce valid, clean documents with code highlighting; and exported files exclude internal system markers.

### 40. Add in-chat message search and filter (Ctrl+F)

**Status: implemented in source and covered by local validation.** Make long conversational threads easily searchable within the active viewport.

- Implement an inline search bar (`Ctrl+F`) within the chat view to highlight and jump between matching keywords across messages.
- Support quick filters to cycle specifically through code blocks, terminal outputs, error messages, or user prompts.

**Exit criteria:** in-chat search responds instantly; matches highlight accurately; and jumping between results scrolls smoothly to the target message.

### 41. Add live streaming interruption and mid-generation steering (Live Steer & Interrupt)

**Status: implemented in source and covered by deterministic tests.** Enable users to course-correct the assistant immediately without waiting for a flawed generation to finish.

- Keep the message input composer unlocked and active while the assistant is streaming an answer.
- Allow the user to submit an immediate correction (e.g. "Wait, use PostgreSQL instead"), automatically halting the current generation turn and feeding the correction into a seamless follow-up turn.

**Exit criteria:** sending a mid-stream message halts previous generation cleanly; token stream resources are released; and the assistant immediately incorporates the correction into its response.

### 42. Add automated conversational titling (Smart Renaming)

**Status: implemented in source and covered by deterministic tests.** Replace generic default conversation labels with concise, context-specific titles automatically.

- Automatically evaluate the first 1-2 message exchanges in the background to generate an accurate, localized title summarizing the core topic.
- Update the conversation title in the SQLite database and broadcast updates to the frontend sidebar without delaying response streaming or blocking user interactions.
- Protect manually edited and custom conversation titles so that automated titling never overwrites user-assigned titles.

**Exit criteria:** new chats update from default labels to descriptive titles within the first few turns; background titling never delays response streaming; and manually renamed titles are never overwritten.

### 44. Add conversation folders and priority pinning

**Status: implemented in source.** Enable custom hierarchical organization of conversation histories in the sidebar.

- Allow users to create custom folders (e.g. Work, Personal, Research) and drag-and-drop or move conversations into folders.
- Support pinning high-priority conversations to the top of the sidebar above folder hierarchies.

**Exit criteria:** folders and pin states persist cleanly; moving chats between folders is transactional; and the sidebar hierarchy scales smoothly.

### 45. Add configurable composer send key bindings

**Status: implemented in source and covered by deterministic tests.** Users can customize keyboard submission preferences in Settings (General settings / Chat behavior).

- Provide an explicit toggle between `Enter to Send / Shift+Enter for newline` and `Ctrl/Cmd+Enter to Send / Enter for newline`.
- Persist the preference locally across sessions and update composer tooltip hints accordingly.

**Exit criteria:** the selected key combination reliably dispatches messages without accidental line breaks; the alternative shortcut inserts newlines; and the preference persists across launches.

### 46. Add completion chimes and audio feedback settings

**Status: implemented in source and covered by deterministic tests.** Provide non-intrusive sound notifications when long-running responses or tasks complete.

- Offer an optional, subtle completion chime when an assistant response finishes, particularly when the application is unfocused or running in the background.
- Include a distinct warning tone for unexpected generation errors or task blockers.
- Provide a master mute switch and volume control under Settings > Notifications.

**Exit criteria:** chimes play smoothly upon completed streaming without audio clipping; audio respect system mute and app-level preferences; and background sound plays only when enabled.

### 47. Add custom agents and prompt template library

**Status: planned, P1.** Enable users to build, save, and share personalized AI agents and reusable prompt templates within an integrated Library tab.

- Provide a dedicated Library view where users can create custom AI agents configured with custom system instructions, default provider/model routes, temperature, and specific tool permissions.
- Support a reusable prompt template catalog with dynamic parameter placeholders (e.g. `{{code}}`, `{{topic}}`, `{{language}}`) that expand directly into the composer.
- Allow users to invoke custom agents directly from the sidebar or via composer shortcuts (e.g. `@AgentName`), maintaining agent configuration local in SQLite with export/import capability.

**Exit criteria:** custom agents retain their tailored instructions and model bindings across sessions; prompt templates expand correctly into the composer; and the agent catalog operates entirely locally without external dependencies.

### 48. Add hashline edits and drift-resilient file modification engine

**Status: implemented in source and covered by deterministic tests.** Eliminate code patch fragility caused by conventional string-matching and whitespace drift.

- Upgrade `read_file` to emit deterministic `hashlines` (`"{line}:{hash}| {text}"` using a 4-hex FNV-1a line content digest) alongside backwards-compatible `lines`, and upgrade `edit_file` to strip and validate hashline anchors automatically.
- Introduce line-drift tolerance and 4-stage fallback matching (`exact`, `normalized_whitespace`, `hashline_anchor`, and `indentation_drift` with automatic baseline indentation re-alignment) before returning an edit failure.
- Provide structured conflict feedback (`hashline_conflict` with `line:hash| text` snippet preview) to the agent when target lines cannot be resolved, identifying the exact mismatch and surrounding file context without modifying the target file.

**Exit criteria:** file edits succeed reliably across minor model formatting drift; hashline resolution avoids corrupting surrounding lines; and failed replacements return precise line diagnostics without modifying the target file.

### 49. Redesign brand identity and logo for OpenChat

**Status: planned, P2.** Replace the legacy organic/botanical logo with a modern, iconic brand identity reflecting an open, autonomous AI workspace.

- Redesign the application mark and typography to convey open dialogue, autonomous action, and local workstation power rather than rustic or environmental motifs.
- Ensure the new mark scales cleanly down to taskbar and favicon sizes (16x16, 32x32, 48x48) with high contrast and instant recognition, while providing dark and light theme variants.
- Update desktop application icons (`icon.ico`, NSIS installer banners, Tauri bundles) and document assets across `apps/desktop/assets/brand`.

**Exit criteria:** the new logo clearly communicates OpenChat's open and communicative identity; renders crisply across small desktop icon scales; and integrates consistently across UI assets and installer packages.

### 50. Elevate frontend design system to industry standard

**Status: planned, P2.** Refactor the UI design system into modular tokens, headless accessible primitives, and consistent elevation hierarchy matching Raycast, Linear, and Cursor standards.

- Modularize the 2,200-line monolithic `index.css` into dedicated stylesheets (`tokens.css`, `typography.css`, `animations.css`, `components.css`).
- Adopt headless accessibility primitives (e.g. Radix UI or Ariakit) for complex controls (`Select`, `Dialog`, `Tooltip`, `Popover`, `ContextMenu`) to ensure rigorous keyboard navigation, ARIA tagging, and roving tab indices.
- Introduce missing high-frequency primitives into `src/design-system`: `Badge/Pill`, `Kbd` (standardized keyboard shortcut badge), `Skeleton` (layout-shift free loading shimmers), and `DropdownMenu`.
- Enforce strict surface elevation tokens (`Canvas`, `Surface`, `Raised`, `Overlay`) paired with 1px border contrast hierarchy for professional dark/light modes.

**Exit criteria:** design tokens are neatly split and maintained; all interactive primitives achieve strict keyboard and screen reader accessibility; loading states eliminate layout shifts; and the UI visual rhythm is consistent across all panels.

### 51. Add shadow git checkpoints and instant timeline rollback

**Status: planned, P1.** Eliminate fear of catastrophic AI code corruption by maintaining an automatic, isolated shadow Git history for every agent turn.

- Maintain an internal shadow Git repository (completely decoupled from the user's primary `.git` working tree) that captures full snapshots before and after every file modification or terminal execution.
- Provide an interactive timeline in the conversation and side panel where users can review exact snapshot diffs and roll back to any past state with a single click.
- Ensure rollback operations restore files cleanly without truncating active conversation context, allowing the user to redirect the agent from any prior checkpoint.

**Exit criteria:** shadow checkpoints capture file states without polluting the user's git history; rollbacks restore workspace state instantaneously; and checkpoint diffs remain accessible across turns.

### 52. Implement prompt prefix freezing and deterministic cache optimization

**Status: planned, P1.** Maximize provider prompt caching hit rates (Anthropic, OpenAI, DeepSeek) to reduce latency and cut API costs by up to 80%.

- Restructure the prompt assembly pipeline so static system instructions, immutable tools, and project rules are permanently frozen at the prefix of requests.
- Prevent cache-busting mutations by appending volatile runtime variables, timestamps, and dynamic observations strictly at the suffix of the message stream.
- Provide observable prompt-cache metrics (cache read/write tokens, hit ratio percentage, estimated cost savings) directly in the run inspector and usage ledger.

**Exit criteria:** cache prefixes remain deterministic across turns within the same conversation; provider-reported cache-hit ratios consistently exceed 75% on sustained multi-turn sessions; and prompt structure prevents unnecessary invalidation.

### 53. Add trigger-scoped repository microagents

**Status: planned, P2.** Allow repositories to declare lightweight, modular instruction blocks that dynamically activate based on file globs, commands, or task keywords.

- Support `.openchat/microagents/*.md` files containing explicit YAML triggers (e.g., file extensions, target directories, slash commands, or task domain tags).
- Dynamically inject microagent rules into the agent context only when touched files or requested commands match triggers, keeping base system prompts minimal and focused.
- Provide project-level validation and an interactive catalog in the Library tab for discovering, authoring, and toggling active microagents.

**Exit criteria:** microagents activate strictly on matching triggers; irrelevant instructions do not consume prompt tokens; and authoring microagents requires zero configuration outside standard Markdown.

### 54. Implement concurrent read-only tool execution engine

**Status: planned, P1.** Eliminate multi-tool latency bottlenecks by executing independent, non-mutating tool calls in parallel using Tokio asynchronous batches.

- Categorize tool operations strictly into concurrent read-only tools (`read_file`, `list_files`, `search_files`, `get_file_info`, `git_status`, `git_diff`, `web_search`, `read_url_content`) versus serialized mutating tools (`write_file`, `edit_file`, `execute_command`, `send_terminal_input`, `ask_user`).
- Batch adjacent read-only tool requests from a single model response and execute them concurrently via `futures::future::join_all` / `tokio::task::JoinSet`, preserving deterministic ordering when returning `tool_result` bundles.
- Ensure mutating operations remain strictly serialized and wait for in-flight read batches to finish, completely preventing race conditions and file write corruption.
- Maintain seamless cancellation propagation so terminating a run halts all in-flight concurrent read operations simultaneously.

**Exit criteria:** multi-file reads and searches execute concurrently with measurable turn-latency reductions; mutating operations execute strictly sequentially; and no race conditions occur on shared workspaces.

### 55. Add unified keyboard-first command palette (Ctrl+K)

**Status: planned, P1.** Provide instant, keyboard-driven navigation, action execution, and configuration control across the entire desktop workspace.

- Implement an accessible modal command palette accessible anywhere via `Ctrl/Cmd+K` inspired by Raycast and Linear.
- Support instant fuzzy search over open conversations, project directories, model switching, settings toggles, custom agent invocation, and recent files.
- Enable direct execution of quick commands (e.g. `New Chat`, `Switch to Claude 3.7`, `Open Settings`, `Purge Project Memory`, `Export Markdown`) with clear keyboard shortcuts and category groupings.
- Deconstruct the monolithic `App.tsx` state to ensure the palette operates with zero perceptible input latency and full focus trapping.

**Exit criteria:** `Ctrl+K` triggers instantly with zero layout lag; keyboard arrow navigation and selection are completely accessible; and all core app workflows can be navigated without touching the mouse.

### 56. Add multi-root workspace support

**Status: planned, P2.** Allow users and agents to operate across multiple related project folders simultaneously within a single session.

- Extend project configuration to accept multiple workspace root directories (e.g., frontend + backend in monorepos or adjacent microservice repositories).
- Update file tools (`read_file`, `search_files`, `write_file`) and terminal path boundaries to validate relative and canonical paths against all active roots in the workspace.
- Provide clear multi-folder indicators in the sidebar and file pickers, enabling users to add or detach folders dynamically without resetting conversation history.

**Exit criteria:** agents navigate and edit files across distinct roots seamlessly; path boundary validation prevents access outside the declared roots; and multi-root configurations persist cleanly in SQLite project storage.







## Explicit boundaries

- No centralized account, message, or credential service; OpenChat remains local-first and self-hostable in its app model.
- Windows terminal execution defaults to AppContainer process sandboxing with opt-in WSL2 container (`bwrap`) isolation; non-Windows terminal execution uses process isolation.
- No automatic use of ChatGPT reset credits.
- No additional OpenCode wire protocols until the shared provider contract is complete.
- GitHub Copilot is an explicit experimental exception requested by Alican: use the Copilot CLI public OAuth client ID with Device Flow, keep OAuth tokens in Windows Credential Manager, and call Copilot's private user/model/inference endpoints directly. Do not ask for a PAT or import gh credentials. The account settings show only plan, access SKU, and recognized quota fields returned by the private user endpoint; no allowance or remaining balance is inferred. GitHub does not document third-party reuse of the CLI OAuth client or a stable external contract for these endpoints, so this display remains best-effort and may change.

## Research references

- [Gemini API terms of service](https://ai.google.dev/gemini-api/terms)
- [Gemini API OpenAI compatibility and feature limitations](https://ai.google.dev/gemini-api/docs/openai)
- [Gemini API GenerateContent grounding metadata](https://ai.google.dev/api/generate-content)
- [Gemini grounding with Google Search](https://ai.google.dev/gemini-api/docs/google-search)
- [Mistral Models API](https://docs.mistral.ai/api/endpoint/models)
- [Mistral reasoning with Chat Completions](https://docs.mistral.ai/studio/conversations/reasoning)
- [Mistral citations and references](https://docs.mistral.ai/studio/conversations/citations)
- [Mistral Chat Completions](https://docs.mistral.ai/studio/conversations/chat-completion)
- [Mistral hosted Web Search tool](https://docs.mistral.ai/studio/agents/agent-tools/websearch)
- [Mistral Conversations API](https://docs.mistral.ai/api/endpoint/beta/conversations)
- [DeepSeek API compatibility](https://api-docs.deepseek.com/guides/codex)
- [DeepSeek tool calls](https://api-docs.deepseek.com/guides/tool_calls)
- [OpenAI Responses API tools](https://developers.openai.com/api/docs/guides/tools-web-search)
- [OpenAI prompt caching and usage fields](https://developers.openai.com/api/docs/guides/prompt-caching)
- [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages/create)
- [Anthropic prompt caching and usage fields](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
- [Claude Code usage and prompt-cache statistics](https://code.claude.com/docs/en/costs)
- [Gemini API context caching and usage metadata](https://ai.google.dev/gemini-api/docs/generate-content/caching)
- [Gemini CLI token caching](https://geminicli.com/docs/cli/token-caching/)
- [Codex CLI usage commands](https://learn.chatgpt.com/docs/developer-commands)
- [Mistral Agents tools](https://docs.mistral.ai/studio/agents/agent-tools)
- [xAI tools](https://docs.x.ai/developers/tools/overview)
- [OpenCode providers](https://opencode.ai/docs/providers)
- [OpenCode cache-key configuration](https://docs.opencode.ai/docs/config/)
- [OpenCode CLI usage and cost statistics](https://dev.opencode.ai/docs/cli/)
- [OpenCode usage normalization source](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/session/session.ts)
- [OpenCode tools](https://dev.opencode.ai/docs/tools/)
- [OpenCode MCP servers](https://opencode.ai/v2/docs/mcp-servers)
- [Model Context Protocol Streamable HTTP transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [Claude Code features](https://code.claude.com/docs/en/features-overview)
- [Goose extensions](https://goose-docs.ai/docs/getting-started/using-extensions/)
- [Goose subagents](https://goose-docs.ai/docs/guides/subagents/)
- [Cline task management](https://docs.cline.bot/core-workflows/task-management)
- [Goose recipes](https://goose-docs.ai/docs/guides/recipes/session-recipes/)
- [LM Studio OpenAI-compatible API](https://lmstudio.ai/docs/developer/openai-compat)
- [Ollama OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility)
- [Amazon Bedrock Converse API](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)
- [Microsoft AI Foundry model endpoints](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints)
- [llama.cpp releases](https://github.com/ggml-org/llama.cpp/releases)
- [ChatGPT capabilities overview](https://help.openai.com/en/articles/9260256-chatgpt-capabilities-overview)
- [ChatGPT projects](https://help.openai.com/en/articles/10169521-projects-in-chatgpt)
- [ChatGPT temporary chat](https://help.openai.com/en/articles/8914046-temporary-chat-faq)
- [ChatGPT conversation search](https://help.openai.com/en/articles/10056348-finding-your-chats-projects-and-files-in-chatgpt)
- [ChatGPT shared conversations and scheduled tasks](https://help.openai.com/en/articles/7925741-sharing-conversations-and-scheduled-tasks-in-chatgpt)
- [Claude personalization features](https://support.anthropic.com/en/articles/10185728-understanding-claude-s-personalization-features)
- [Claude projects](https://support.anthropic.com/en/articles/9517075-what-are-projects)
- [Claude styles](https://support.anthropic.com/en/articles/10181068-configuring-and-using-styles)
- [Claude artifacts](https://support.anthropic.com/en/articles/9487310-what-are-artifacts-and-how-do-i-use-them)
- [Gemini file upload and analysis](https://support.google.com/gemini/answer/14903178)
- [Gemini Connected Apps](https://support.google.com/gemini/answer/13695044)
- [Open WebUI workspace models](https://docs.openwebui.com/features/workspace/models/)
