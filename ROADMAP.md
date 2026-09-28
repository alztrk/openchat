# OpenChat roadmap

This roadmap reflects the repository's current implementation. It has no calendar estimates; each stage depends on the acceptance criteria of the stage before it. The Windows desktop experience remains the first release target.

## Current implementation

- Flutter desktop UI with a local Rust child service and SQLite conversation storage under `%LOCALAPPDATA%\OpenChat`.
- ChatGPT OAuth connections, account/workspace selection, model catalog, account and quota information, streamed Responses, cancellation, and background conversation titles.
- OpenCode Console through its OpenAI Chat Completions endpoint. Other OpenCode protocol families are not supported.
- Shared provider request, tool, and stream event types for ChatGPT and OpenCode, plus local shared instructions.
- Local projects with read-only file listing, search, reading, and file metadata tools. Tool calls, arguments, progress, and results are stored with assistant messages and shown in the conversation UI.
- Global `Onay İste` and `Tam erişim` settings for local file tools. Approval is per call and the Rust service enforces the selected path scope.
- Conversation history, project grouping, model favorites, rename/delete/pin actions, retry, and Markdown export. Tool activity is included in Markdown exports.
- A Windows portable executable build script and local data directories for the database, logs, and cache.

These bullets describe code present in the repository. They do not mean that every provider endpoint or account flow has been verified against a live service.

## 1. Close the Windows first release

**Status: in progress.** Finish and validate the ChatGPT-first Windows experience before adding another provider.

- Build a fresh portable executable from the current source.
- Manually verify the packaged app with a real ChatGPT account: sign in, load account/model/quota information, stream a response, stop a response, and reopen the saved conversation.
- Manually verify OpenCode's supported OpenAI-compatible models, including the unsupported-tool/error path where a model does not accept tool calls.
- Confirm that existing local databases upgrade without losing conversations, projects, favorites, or tool activity.
- Check that failures remain actionable and that credentials, message content, tool output, and private provider payloads stay out of service logs.

**Exit criteria:** the packaged Windows app completes those real workflows, existing local data survives upgrade, and unsupported or unavailable provider behavior is presented clearly.

## 2. Stabilize the provider contract

**Status: next.** Keep provider-specific wire formats inside their adapters and make unsupported capabilities explicit to the user.

- Define which shared capabilities a provider/model can use, including streaming, reasoning summaries, and tools.
- Avoid sending tools to models that do not support them; show a clear reason when capability information is unavailable or a provider rejects the request.
- Keep cancellation, partial responses, tool errors, quota limits, and stale model catalogs consistent across providers.
- Recheck ChatGPT's private endpoints when they change; they are compatibility-sensitive and are not a stable public API contract.

**Exit criteria:** both existing providers report or handle capabilities and failures through the shared OpenChat contract without leaking provider payloads into Flutter.

## 3. Add the next hosted provider

**Status: not started.** Gemini is the next planned provider after the shared contract is stable.

- Add Gemini through a dedicated adapter that maps its models, authentication, usage limits, streaming events, and tool calls to the shared schema.
- Preserve provider-native errors and capability differences as explicit shared states instead of assuming ChatGPT/OpenCode behavior.
- Keep credentials in platform secure storage and provider requests in the local Rust service.

**Exit criteria:** Gemini model selection, a streamed conversation, cancellation/error handling, local history, and supported tool behavior work without changing the chat UI's provider-specific assumptions.

## 4. Expand desktop platforms

**Status: later.** Windows remains the supported first target. Bring the local Rust service and credential storage to Linux and macOS before mobile.

- Add platform-specific packaging, secure credential storage, data-directory resolution, and service lifecycle handling.
- Preserve the same SQLite conversation format and migration behavior across desktop platforms.
- Document installation, upgrade, and recovery paths for each supported platform.

**Exit criteria:** each desktop build packages its service, stores secrets in the platform's secure store, and keeps durable application data in that platform's user data directory.

## 5. Evaluate mobile support

**Status: later.** Android and iOS require a service lifecycle and secure-storage design that fits mobile process and permission limits. Start this work after desktop provider behavior is stable.

- Decide how Flutter invokes the Rust provider layer on mobile and how long-running requests survive app lifecycle changes.
- Define local data and credential handling for Android and iOS without assuming the desktop child-process model.
- Keep the UI responsive to mobile layouts, keyboards, connectivity changes, and interrupted background work.

## Explicit boundaries

- No centralized account, message, or credential service; OpenChat remains local-first and self-hostable in its app model.
- No file editing or shell execution by project tools.
- No automatic use of ChatGPT reset credits.
- No Gemini or additional OpenCode wire protocols until the shared provider contract stage is complete.
