# OpenChat roadmap

This roadmap reflects the repository's current implementation. It has no calendar estimates. Numbered stages group related workstreams and suggest sequencing; a stage does not block unrelated work unless its dependency is stated. The Windows desktop experience remains the first release target.

## Current implementation

- Flutter desktop UI with a local Rust child service and SQLite conversation storage under `%LOCALAPPDATA%\OpenChat`.
- ChatGPT OAuth connections, account/workspace selection, model catalog, account and quota information, streamed Responses, cancellation, and background conversation titles.
- OpenCode Console through its OpenAI Chat Completions endpoint. Other OpenCode protocol families are not supported.
- Gemini, Groq, Cerebras, OpenRouter, and Mistral through their official OpenAI-compatible Chat Completions APIs, with API keys in platform secure storage and six-hour per-key model catalogs. Mistral exposes context, vision, and documented reasoning capabilities; OpenRouter models are filtered to current zero-price text-chat entries that advertise tool support.
- Shared provider request, tool, and stream event types across ChatGPT, OpenCode, Gemini, Groq, Cerebras, OpenRouter, and Mistral, plus local shared instructions.
- Embedded release catalogs and a verified, cancellable installer for llama.cpp on Windows x64. vLLM and ExLlama are catalogued but installation stays blocked until their full runtime dependencies can be pinned and verified. Settings supports a model folder per engine and bounded discovery of unregistered GGUF and Transformers model files, with explicit confirmation before registration. Windows x64 llama.cpp can start a selected GGUF on demand, wait for health readiness, stream chat through the shared route, stop on cancellation or service shutdown, and show localized startup/runtime failures. The full service path has been exercised with a real GGUF in an isolated profile, including response persistence and process shutdown; CUDA offload and the CPU-specific package remain unverified. Runtime diagnostics now record only bounded, safe lifecycle fields. GGUF-declared and runtime-active context windows plus reported image and tool-template capabilities are implemented. Local image input still needs projector discovery and launch support; accelerator recommendation remains open.
- Local workspace tools for file listing, search, reading, metadata, writing, and editing, plus web search, URL reading, and terminal command/session tools. Tool calls, arguments, progress, and results are stored with assistant messages and shown in the conversation UI.
- Global `Ask for approval` and `Full access` settings govern local file and terminal calls. Approval is per call; canonical path checks apply to filesystem tools and do not sandbox terminal processes.
- Conversation history, project grouping, model favorites, rename/delete/pin actions, retry, and Markdown export. Tool activity is included in Markdown exports.
- Per-conversation memory inspection, bounded hybrid FTS5 and optional local semantic archive search with dated source excerpts, best-effort credential redaction in derived tool indexes, and a confirmed reset for compacted context that preserves full history.
- A Windows portable executable build script and local data directories for the database, logs, and cache.

These bullets describe code present in the repository. They do not mean that every provider endpoint or account flow has been verified against a live service.

## 1. Confirm the Windows baseline

**Status: verified by the user.** The Windows app, ChatGPT, and OpenCode have been exercised repeatedly and are currently working. Do not treat baseline release validation as an open implementation task.

- Recheck these workflows as regression coverage when a change affects them.
- Validate API-key providers with live accounts when credentials become available; that work remains in the provider-specific stage below and does not block the confirmed Windows baseline.

**Exit criteria:** met for the current Windows, ChatGPT, and OpenCode baseline. Reopen this stage only if a regression or release-specific change requires it.

## 2. Stabilize the provider contract

**Status: in progress.** Keep provider-specific wire formats inside their adapters and make unsupported capabilities explicit to the user.

- Define which shared capabilities a provider/model can use, including streaming, reasoning summaries, and tools.
- Avoid sending tools to models that do not support them; show a clear reason when capability information is unavailable or a provider rejects the request.
- Models with confirmed tool-call support keep the existing tool flow; confirmed-unsupported models receive no tool definitions, while unknown support is disclosed in the composer and remains optimistic for compatibility. HTTP 400/422 responses to requests containing tool definitions or tool history use a sanitized, localized error; provider response bodies are not shown or retried automatically.
- Keep cancellation, partial responses, tool errors, quota limits, and stale model catalogs consistent across providers.
- Recheck ChatGPT's private endpoints when they change; they are compatibility-sensitive and are not a stable public API contract.

**Exit criteria:** both existing providers report or handle capabilities and failures through the shared OpenChat contract without leaking provider payloads into Flutter.

## 3. Validate API-key providers

**Status: implemented in source; live account validation deferred until the first GitHub release at the user's request.** Gemini, Groq, Cerebras, OpenRouter, and Mistral are connected through their official OpenAI-compatible endpoints.

- Verify model catalog loading, streaming, cancellation, errors, local history, and tool calls with real accounts for each provider.
- Confirm current pricing, quota, tool-use capability, and data handling against each provider's account terms.
- Keep credentials in platform secure storage and provider requests in the local Rust service.

**Exit criteria:** every provider completes those real workflows without changing the shared chat UI or leaking keys and provider payloads into logs.

## 4. Build managed local inference

**Status: in progress.** OpenChat owns the engine release catalog, installation, model files, process lifecycle, and local request route. Users do not need to install a separate engine or configure a third-party inference service.

- Keep release metadata tied to immutable upstream versions. Verify each downloadable engine and model asset by exact size and SHA-256 before publishing it into OpenChat's managed runtime or model directory.
- Finish llama.cpp runtime selection for NVIDIA CUDA and CPU. Detect enough GPU and driver information to recommend only compatible variants; keep unsupported packages visible with a clear reason.
- Keep vLLM and ExLlama unavailable until every runtime dependency, including accelerator-specific libraries and transitive packages, is pinned to verifiable artifacts. Do not install from mutable package indexes or run an unpinned setup script.
- Add model catalogs and model downloads separately from engine binaries. Show file size, destination, progress, cancellation, integrity verification, and recovery after interrupted downloads.
- Validate managed llama.cpp startup, model loading, streamed chat, cancellation, model switching, and shutdown with real CPU and NVIDIA CUDA GGUF models on Windows x64. The isolated app-service flow has passed with a real GGUF, including saved response and shutdown; verify CPU package behavior and actual CUDA offload separately. Bounded, sanitized lifecycle diagnostics are implemented without model paths or prompt content.
- Add local context-window metadata and only expose image, reasoning, and tool capabilities when the selected model/runtime supports them. Keep provider payloads and local process details inside the Rust service.
- Extend package selection, secure storage, process supervision, and data directories to Linux and macOS. Treat WSL2 as a separate managed Linux runtime for vLLM rather than claiming native Windows support.

**Exit criteria:** a user can select a hardware-compatible engine and model, install and verify both through OpenChat, start and stop the local runtime, send and cancel a streamed chat request, and reopen its conversation without a separate CLI or external inference API.

## 5. Broaden API-key provider access

**Status: planned.** Make compatible provider connections configurable and expose provider/model support accurately before adding more provider-specific behavior.

- Add a user-configured OpenAI-compatible endpoint with a base URL, API key, catalog refresh, and manual model entry when a catalog is unavailable. Keep credentials in platform secure storage and requests in the local Rust service.
- Treat context limits, streaming, reasoning, vision, tool calling, and prices as model capabilities. Keep unknown values explicit and do not expose an unsupported feature as available.
- Add a DeepSeek preset using its documented API and verify streamed responses, reasoning output, and tool calls through the shared provider contract.
- Extend OpenRouter beyond its current zero-price model filter. Show current input/output prices and supported capabilities for priced models, and make the cost implications clear before a user selects or calls one.

**Exit criteria:** a user can configure a compatible endpoint or connect DeepSeek without exposing credentials; the model selector only offers verified capabilities; and OpenRouter model pricing and tool support are visible before use.

## 6. Add provider-native API capabilities

**Status: planned.** Add native API adapters where OpenAI-compatible Chat Completions does not expose a provider's documented capabilities.

- Add the public OpenAI Responses API for OpenAI API-key connections, including streaming, function calling, and optional hosted tools such as web search. Keep this route distinct from ChatGPT OAuth and its private Codex endpoints.
- Evaluate Anthropic Messages and xAI Responses as separate provider adapters, with model catalogs and capabilities obtained from their documented APIs.
- Evaluate native Gemini and Mistral APIs for provider-hosted tools that their compatibility endpoints do not expose. Add only documented features that fit the shared request and event contract.
- Label provider-hosted tools, the information sent to them, citations or returned sources, and any provider-side billing. Never enable these tools implicitly when a user selects a model.
- Handle streaming, cancellation, tool-call errors, usage, context limits, and unavailable capabilities consistently across every adapter.

**Exit criteria:** each released native adapter passes the provider-specific account checks and shared streaming/tool/error contract; provider-hosted tools require explicit enablement and clearly describe their data and cost behavior.

## 7. Add MCP and safer coding-agent workflows

**Status: planned.** Expand beyond the current built-in tools with discoverable integrations and clearer, more reversible workspace actions.

- Add an MCP client for local stdio and remote Streamable HTTP servers. Provide explicit server setup, connection status, tool discovery, namespacing, and clear startup, timeout, and protocol errors. Load tool schemas only when needed to keep ordinary requests bounded.
- Require users to enable each server and apply per-server and per-tool `Ask`, `Allow`, or `Deny` controls before a discovered tool can run. Keep server credentials in secure storage and treat tool results as untrusted input.
- Add a reviewable patch/diff workflow with checkpoints and undo for file changes. Keep terminal permissions distinct: the current terminal runs with OpenChat's operating-system permissions, and path checks on file tools do not sandbox it. Investigate a real process sandbox before offering stronger isolation claims.
- Evaluate project-scoped instructions and reusable Skills, plan/Todo tracking, and LSP diagnostics/navigation as follow-on coding workflows. Reuse the existing local shared-instructions behavior where it fits.
- Defer browser/computer control and isolated subagents until tool permission boundaries and process isolation are established.

**Exit criteria:** MCP tools are discoverable but cannot run before explicit server enablement and the applicable permission decision; file edits can be reviewed and reversed; and the UI clearly distinguishes filesystem checks from terminal process isolation.

## 8. Add Gemini API OAuth

**Status: planned.** Let users authenticate to the Gemini Developer API with their Google account, without entering a Gemini API key. This is the Gemini API's documented OAuth flow and uses OpenChat's own OAuth client; it is separate from Gemini CLI and Code Assist authentication.

- Confirm the required Google Cloud project setup, Generative Language API enablement, OAuth consent configuration, scopes, and any app verification requirements for a distributable desktop client.
- Store and refresh OAuth credentials through the existing platform secure-storage boundary. Associate each connection with the Google Cloud project that owns its Gemini API quota and billing.
- Keep OAuth and API-key connections distinguishable in Settings and the model selector, while routing both through the shared provider contract.
- Verify account cancellation, consent denial, expired/revoked credentials, project configuration errors, quota exhaustion, and real model requests.
- Document that OAuth alone does not guarantee free use. Gemini API quota and billing follow the selected Cloud project and are not the Gemini CLI or Code Assist free quota.

**Exit criteria:** a user can connect a Google account through OpenChat's OAuth client and send Gemini API requests without an API key; setup, quota, billing, and authentication failures are clear, and no Gemini CLI or Code Assist OAuth credentials are reused.

## 9. Evaluate a compliant Antigravity integration

**Status: research required.** Do not use consumer Antigravity OAuth from OpenChat. Evaluate only a Google-documented Enterprise or Cloud integration, such as a supported Vertex AI or ADC route, if its product terms authorize a third-party desktop client.

- Confirm the permitted authentication flow, organization/admin requirements, project and license prerequisites, and whether an external client may send inference requests under the applicable Enterprise terms.
- Keep any approved Enterprise integration separate from personal Antigravity accounts and consumer quotas.
- If Google does not document and permit a suitable external-client flow, record Antigravity as unsupported instead of attempting to reuse its consumer OAuth session.

**Exit criteria:** either a documented, terms-compliant Enterprise integration path has concrete implementation requirements, or the roadmap records that Antigravity cannot be integrated as an external provider.

## 10. Expand desktop platforms

**Status: later.** Windows remains the supported first target. Bring the local Rust service and credential storage to Linux and macOS before mobile.

- Add platform-specific packaging, secure credential storage, data-directory resolution, and service lifecycle handling.
- Preserve the same SQLite conversation format and migration behavior across desktop platforms.
- Document installation, upgrade, and recovery paths for each supported platform.

**Exit criteria:** each desktop build packages its service, stores secrets in the platform's secure store, and keeps durable application data in that platform's user data directory.

## 11. Evaluate mobile support

**Status: later.** Android and iOS require a service lifecycle and secure-storage design that fits mobile process and permission limits. Start this work after desktop provider behavior is stable.

- Decide how Flutter invokes the Rust provider layer on mobile and how long-running requests survive app lifecycle changes.
- Define local data and credential handling for Android and iOS without assuming the desktop child-process model.
- Keep the UI responsive to mobile layouts, keyboards, connectivity changes, and interrupted background work.

## 12. Harden network access and indexed data

**Status: implemented in source and covered by deterministic tests.** The URL policy, archive-index controls, untrusted retrieval boundary, and best-effort credential redaction are in place.

- Add an outbound URL policy for read_url_content. Reject loopback, private, link-local, multicast, reserved, and cloud metadata destinations; validate every redirect and protect DNS resolution from rebinding between validation and connection.
- Give users clear control over which tool results enter full-text and semantic indexes. Support excluding selected tool types or conversations, clearing derived indexes, and documenting what is stored locally.
- Keep tool output and historical excerpts marked as untrusted. Never treat retrieved or indexed content as instructions.
- Redact common credential field names and visible provider token formats from derived keyword and semantic indexes. The migration rebuilds old tool-index rows, removes their old vectors, queues sanitized content for re-embedding, and prunes stale semantic cache files when a new index is loaded. Original message and tool data stay unchanged. Detection does not cover every custom, encoded, or transformed secret.

**Exit criteria:** URL fetches cannot reach prohibited network destinations through direct URLs or redirects; sensitive tool results can be excluded and removed from derived indexes; and existing MCP/browser plans inherit the same network and data-handling policy.

## 13. Close provider, storage, and runtime correctness gaps

**Status: implemented in source and covered by deterministic tests.** The audited provider parsing, migration, metadata, download, and process-lifecycle gaps have focused fixes and regression coverage.

- Reject Responses API tool-call events with a missing, invalid, or out-of-range output index. Apply the per-turn tool-call limit to every event that can create a call.
- Make schema initialization stop at every requested target version. Align user-question item identity between group-level validation and database uniqueness.
- Validate provider metadata by JSON type and value, including treating null capability fields as unknown rather than fresh, valid metadata.
- Verify prepared semantic model files against their expected digests and reject non-finite embedding components before quantization.
- Add bounded total and idle timeouts to model downloads, throttle progress events, and retain safe range-resume and integrity checks.
- Avoid holding the local-runtime state lock through long readiness polling. Make port allocation resistant to free-then-bind races and tie installer asset URLs to their declared repository and release.
- Process the final buffered SSE event at end of stream and apply explicit size limits to encrypted compaction state.

**Exit criteria:** malformed or oversized provider events fail with stable errors; migrations reach each requested schema version; cache and model metadata cannot claim readiness from invalid values; downloads and runtime operations have bounded waits and observable failures.

## 14. Automate integration and release validation

**Status: implemented in source and covered by local validation.** Offline checks cover the Flutter/Rust boundary, provider fixtures, migrations, local lifecycle behavior, and the portable launcher. GitHub Actions is configured; a remote run still requires a push or manual workflow dispatch.

- Move the live web-search smoke test and fixed-duration file benchmark out of the default test suite. Keep manual network checks opt-in and run performance benchmarks separately from correctness tests.
- Add local HTTP fixtures for Chat Completions, Responses, SSE, tool-call round trips, malformed events, cancellation, timeouts, and 401/429/5xx responses.
- Add Flutter-to-Rust process-level RPC tests, local-engine startup/shutdown checks, schema migration coverage, model-download resume/integrity cases, and portable launcher extraction smoke tests.
- Add CI for Flutter analysis and tests, Rust formatting/checks/tests, Windows release builds, required artifact contents, and release smoke validation.
- Keep live-account provider validation separate from deterministic pull-request checks.
- Reconcile provider and tool scope across README, roadmap, contribution guidance, and historical integration plans so documentation does not contradict the implementation.

**Exit criteria:** routine CI requires no provider credentials or public network access; protocol, process, migration, and release boundaries have deterministic checks; live tests and benchmarks run only when explicitly requested.

## 15. Add portable backup, import, and restore

**Status: selected-conversation archive v1 implemented.** Users can export selected conversations with attachments, inspect and validate archives, and restore with duplicate handling and rollback. Full-profile database backup and broader interchange formats remain planned.

- Create a versioned archive for selected conversations and their attachments; full-profile database backup and global settings are not included.
- Encrypt archives with a passphrase and an age v1 stream. Never include provider API keys, OAuth tokens, linked account/workspace identifiers, projects, or global settings.
- Inspect and validate archive contents and integrity before restore. Detect duplicate conversation IDs and offer skip-existing or import-as-copy behavior.
- Restore database rows in one transaction and create attachments exclusively with rollback cleanup so failed imports leave the current profile usable.
- Keep Markdown export separate. JSON/ZIP interchange, schema-upgrading restore, and full-profile backup remain future work.

**Exit criteria for selected-conversation v1:** a user can inspect, export, and restore selected conversations and attachments into another local profile; failed restore is recoverable; and linked credentials remain in platform secure storage. Full-profile migration is not part of this stage's delivered scope.

## 16. Build a searchable conversation library

**Status: planned.** Make long-term chat history practical to browse and maintain.

- Extend sidebar title search to messages and completed tool activity, with snippets, match highlighting, and navigation to the matching message.
- Add provider, model, project, status, and date filters. Keep attachment-content indexing separately configurable.
- Add archive/unarchive, tags, saved searches, bookmarks, multi-select, and transactional bulk move/export/archive actions.
- Keep the existing per-conversation memory search as a focused context tool; make the broader library search a separate history workflow.

**Exit criteria:** search results identify the source conversation and message, can open that message directly, and honor the user's indexing and archive settings.

## 17. Add project profiles and incremental code context

**Status: planned.** Extend project-scoped instructions into a user-configurable workspace profile.

- Allow a project to select default provider/model, reasoning preference, instructions, tool policy, and working directory.
- Add an incremental project index that respects .gitignore, reports stale or unavailable state, and can be disabled or cleared per project.
- Keep LSP symbol navigation and diagnostics as a separate code-intelligence capability that can use the project index where useful.
- Make profile precedence visible when global and project settings both apply.

**Exit criteria:** a project can restore its own model and tool defaults when reopened; ignored files stay out of project indexing; stale or disabled indexes are shown explicitly.

## 18. Add Git-aware tasks and isolated workspaces

**Status: planned.** Connect coding-agent work to reviewable repository state and project-defined checks.

- Add structured read-only Git status, diff, changed-file, and recent-history views.
- Let users declare named project tasks such as test, lint, or build. Run them through the existing terminal boundary with the configured approval and output limits.
- Offer a Git worktree for isolated agent tasks after the existing file permission and patch review controls are in place.
- Show the task diff and check results before the user decides whether to keep, discard, or continue the worktree.

**Exit criteria:** repository state is visible without parsing arbitrary shell output; named tasks have bounded execution and clear exit status; isolated work can be reviewed before it is retained.

## 19. Add reusable workflows and lifecycle hooks

**Status: planned.** Make repeated work easy to launch while keeping deterministic actions governed by explicit permissions.

- Add parameterized workflow recipes that package an initial prompt, project context, provider/model choice, enabled tools, and expected output.
- Keep recipes distinct from Skills: recipes launch a configured task, while Skills supply reusable instructions or reference material.
- Add optional hooks for defined lifecycle events such as before/after a tool call or after a file edit. Show installed hooks, scope, actions, and failures.
- Require explicit user configuration for hooks with side effects; apply timeouts, output limits, cancellation, and permission checks.

**Exit criteria:** a recipe can be reviewed and re-run with its settings visible; hooks run only for their declared events and scopes; failures are reported without hiding the original task result.

## 20. Add message branches and durable runs

**Status: planned.** Preserve alternate answers and let independent work continue across conversations and app restarts.

- Support editing a user message and starting a branch from that point while preserving the previous message and response history.
- Keep regenerated or alternative model responses as versions that can be compared and selected.
- Model generation and tool execution as durable runs with run IDs, cancellation, provider concurrency limits, and explicit retry semantics.
- Allow users to switch conversations while a run continues, inspect pending runs, and receive completion notifications.
- Recover or clearly mark interrupted runs after service or app restart; never retry an ambiguous provider request automatically.

**Exit criteria:** branches and response versions remain addressable; run state survives restart or is reported as interrupted; provider limits and cancellation apply per run.

## 21. Add usage and model evaluation

**Status: planned.** Help users compare actual provider and local-model behavior and manage spending.

- Store provider-reported input, output, and cached token counts, cost when known, first-token latency, total latency, throughput, retries, and tool outcomes.
- Show usage by conversation, project, provider, and model with budget thresholds and limit warnings.
- Make unknown prices and estimated costs explicit. Do not fabricate prices when a provider omits them.
- Add an opt-in evaluation workspace that replays local fixtures or user-selected prompts and compares response quality, latency, cost, tool success, cancellation, and context handling.
- Add local model benchmarks and memory estimates while distinguishing measured values from estimates.

**Exit criteria:** usage records identify their provider/model and whether values are reported or estimated; evaluations are separated from ordinary conversations and require clear opt-in before sending billable requests.

## 22. Add citations and request-data visibility

**Status: planned.** Preserve source provenance across built-in web tools and provider-native search.

- Store web and provider-returned sources as structured records linked to the response that used them.
- Add citation anchors in assistant output and let users open the source card with title, domain, URL, and retrieval time.
- Show which provider received the request and which messages, attachments, instructions, and tool results were included.
- Keep provider-hosted sources distinct from local web_search and read_url_content sources.

**Exit criteria:** displayed citations resolve to stored source records; the user can inspect the request route and included data for a response; missing provenance is shown as unavailable.

## 23. Expand document and media input

**Status: planned.** Add common research and work documents with bounded parsing and source location tracking.

- Evaluate PDF, DOCX, and XLSX extraction, including page, sheet, and cell references where available.
- Add OCR for image-only documents and opt-in audio transcription with timestamps and speaker labels when supported.
- Evaluate video input through bounded frame sampling and transcript alignment.
- Keep extracted content local by default where possible; show size, extraction status, and which provider receives the resulting content.
- Apply archive-bomb, malformed-file, memory, time, and output-size limits to parsers.

**Exit criteria:** supported files produce reviewable extracted text with source locations; malformed, oversized, or unsupported files show a specific error and recovery path.

## 24. Expand local and managed provider connections

**Status: planned.** Make existing local model servers and enterprise gateways easier to connect without duplicating their model runtimes.

- Add dedicated Ollama and LM Studio connections for health checks, model discovery, and documented model lifecycle controls. Keep these distinct from OpenChat-managed llama.cpp installation.
- Evaluate Amazon Bedrock Converse/ConverseStream with regional configuration, AWS authentication, model availability, and tool-use capability checks.
- Evaluate Microsoft AI Foundry endpoints with deployment identifiers, supported authentication modes, and per-model API compatibility.
- Reuse the shared provider contract and make region, deployment, privacy, pricing, and unavailable capability information visible.

**Exit criteria:** each connection reports supported models and capabilities from its documented endpoint; credentials stay in secure storage; unsupported models and regions fail with actionable errors.

## 25. Package and distribute extensions

**Status: later.** Package MCP servers, Skills, recipes, and provider presets after their schemas and permission boundaries stabilize.

- Define versioned manifests with declared tools, permissions, provider requirements, and configuration needs.
- Support local install, update, disable, and removal before creating a shared catalog.
- Add signature or trusted-source verification and explain the access each package requests.
- Keep package permissions separate from the user's built-in tool permissions and require explicit enablement.

**Exit criteria:** packages are version-pinned, their requested access is visible before installation, and removal leaves no active server or hook behind.

## 26. Add signed distribution and safe updates

**Status: later.** Establish publisher authenticity and recovery before broad public distribution.

- Sign Windows executables and release manifests; publish checksums and a software bill of materials.
- Define stable and preview channels, supported-version policy, and update metadata.
- Verify update signatures and payload integrity before installation.
- Preserve the previous working bundle and provide a user-visible rollback path.

**Exit criteria:** users can verify the publisher and release contents; interrupted or invalid updates do not replace the working version.

## 27. Add user-controlled encrypted sync

**Status: later.** Offer optional multi-device data movement while preserving OpenChat's local-first model.

- Sync encrypted archives to a user-selected folder or compatible storage service without introducing a central OpenChat account.
- Define device keys, recovery, conflict handling, attachment deduplication, and offline behavior.
- Keep API keys, OAuth credentials, and other provider secrets out of synchronized data.
- Build on the portable archive format and integrity checks from the backup stage.

**Exit criteria:** sync is opt-in, users control the destination and recovery material, conflicts are reviewable, and credentials never leave platform secure storage.

## 28. Establish an accessibility and locale quality matrix

**Status: planned.** Verify that core workflows remain usable across supported languages, input methods, and Windows display settings.

- Cover keyboard-only navigation, focus visibility and restoration, screen-reader labels, high contrast, and reduced motion.
- Test text scaling, 125–200% Windows display scaling, narrow layouts, long translations, and input composition.
- Add semantic accessibility assertions alongside screenshot tests for Turkish, English, German, Spanish, and French.

**Exit criteria:** core chat, settings, model selection, tool approval, and local-model workflows pass the accessibility matrix in all supported locales.

## Explicit boundaries

- No centralized account, message, or credential service; OpenChat remains local-first and self-hostable in its app model.
- No OS process sandbox for local terminal tools; commands run with the same operating-system permissions as OpenChat.
- No automatic use of ChatGPT reset credits.
- No additional OpenCode wire protocols until the shared provider contract stage is complete.
- Do not reuse Gemini CLI/Code Assist or consumer Antigravity OAuth credentials or quotas in OpenChat; use only provider-documented flows permitted for external clients.

## Research references

- [Gemini API OAuth guide](https://ai.google.dev/gemini-api/docs/oauth)
- [Gemini API terms of service](https://ai.google.dev/gemini-api/terms)
- [Google OAuth production readiness and verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification)
- [Antigravity terms of service](https://antigravity.google/terms)
- [Antigravity Enterprise documentation](https://antigravity.google/docs/enterprise)
- [Antigravity SDK authentication overview](https://antigravity.google/docs/sdk/overview)
- [Mistral Models API](https://docs.mistral.ai/api/endpoint/models)
- [Mistral reasoning with Chat Completions](https://docs.mistral.ai/studio/conversations/reasoning)
- [DeepSeek API compatibility](https://api-docs.deepseek.com/guides/codex)
- [DeepSeek tool calls](https://api-docs.deepseek.com/guides/tool_calls)
- [OpenAI Responses API tools](https://developers.openai.com/api/docs/guides/tools-web-search)
- [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages/create)
- [Gemini API tools](https://ai.google.dev/gemini-api/docs/tools)
- [Mistral Agents tools](https://docs.mistral.ai/studio/agents/agent-tools)
- [xAI tools](https://docs.x.ai/developers/tools/overview)
- [OpenCode providers](https://opencode.ai/docs/providers)
- [OpenCode tools](https://dev.opencode.ai/docs/tools/)
- [OpenCode MCP servers](https://opencode.ai/v2/docs/mcp-servers)
- [Claude Code features](https://code.claude.com/docs/en/features-overview)
- [Goose extensions](https://goose-docs.ai/docs/getting-started/using-extensions/)
- [Goose subagents](https://goose-docs.ai/docs/guides/subagents/)
- [Cline task management](https://docs.cline.bot/core-workflows/task-management)
- [Goose recipes](https://goose-docs.ai/docs/guides/recipes/session-recipes/)
- [LM Studio REST API](https://lmstudio.ai/docs/developer/rest)
- [Ollama OpenAI compatibility](https://github.com/ollama/ollama/blob/main/docs/api/openai-compatibility.mdx)
- [Amazon Bedrock Converse API](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)
- [Microsoft AI Foundry model endpoints](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints)
- [llama.cpp releases](https://github.com/ggml-org/llama.cpp/releases)
- [vLLM installation guide](https://docs.vllm.ai/en/latest/getting_started/installation/)
- [ExLlamaV3 releases](https://github.com/turboderp-org/exllamav3/releases)
