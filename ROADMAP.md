# OpenChat roadmap

This roadmap reflects the repository's current implementation. It has no calendar estimates; each stage depends on the acceptance criteria of the stage before it. The Windows desktop experience remains the first release target.

## Current implementation

- Flutter desktop UI with a local Rust child service and SQLite conversation storage under `%LOCALAPPDATA%\OpenChat`.
- ChatGPT OAuth connections, account/workspace selection, model catalog, account and quota information, streamed Responses, cancellation, and background conversation titles.
- OpenCode Console through its OpenAI Chat Completions endpoint. Other OpenCode protocol families are not supported.
- Gemini, Groq, Cerebras, and OpenRouter through their official OpenAI-compatible Chat Completions APIs, with API keys in platform secure storage and six-hour per-key model catalogs. OpenRouter models are filtered to current zero-price text-chat entries that advertise tool support.
- Shared provider request, tool, and stream event types across ChatGPT, OpenCode, Gemini, Groq, Cerebras, and OpenRouter, plus local shared instructions.
- Local projects with read-only file listing, search, reading, and file metadata tools. Tool calls, arguments, progress, and results are stored with assistant messages and shown in the conversation UI.
- Global `Onay İste` and `Tam erişim` settings for local file tools. Approval is per call and the Rust service enforces the selected path scope.
- Conversation history, project grouping, model favorites, rename/delete/pin actions, retry, and Markdown export. Tool activity is included in Markdown exports.
- Per-conversation memory inspection, bounded hybrid FTS5 and optional local semantic archive search with dated source excerpts, and a confirmed reset for compacted context that preserves full history.
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

## 3. Validate API-key providers

**Status: implemented in source; pending live validation.** Gemini, Groq, Cerebras, and OpenRouter are connected through their official OpenAI-compatible endpoints.

- Verify model catalog loading, streaming, cancellation, errors, local history, and tool calls with real accounts for each provider.
- Confirm current pricing, quota, tool-use capability, and data handling against each provider's account terms.
- Keep credentials in platform secure storage and provider requests in the local Rust service.

**Exit criteria:** every provider completes those real workflows without changing the shared chat UI or leaking keys and provider payloads into logs.

## 4. Add Gemini API OAuth

**Status: planned.** Let users authenticate to the Gemini Developer API with their Google account, without entering a Gemini API key. This is the Gemini API's documented OAuth flow and uses OpenChat's own OAuth client; it is separate from Gemini CLI and Code Assist authentication.

- Confirm the required Google Cloud project setup, Generative Language API enablement, OAuth consent configuration, scopes, and any app verification requirements for a distributable desktop client.
- Store and refresh OAuth credentials through the existing platform secure-storage boundary. Associate each connection with the Google Cloud project that owns its Gemini API quota and billing.
- Keep OAuth and API-key connections distinguishable in Settings and the model selector, while routing both through the shared provider contract.
- Verify account cancellation, consent denial, expired/revoked credentials, project configuration errors, quota exhaustion, and real model requests.
- Document that OAuth alone does not guarantee free use. Gemini API quota and billing follow the selected Cloud project and are not the Gemini CLI or Code Assist free quota.

**Exit criteria:** a user can connect a Google account through OpenChat's OAuth client and send Gemini API requests without an API key; setup, quota, billing, and authentication failures are clear, and no Gemini CLI or Code Assist OAuth credentials are reused.

## 5. Evaluate a compliant Antigravity integration

**Status: research required.** Do not use consumer Antigravity OAuth from OpenChat. Evaluate only a Google-documented Enterprise or Cloud integration, such as a supported Vertex AI or ADC route, if its product terms authorize a third-party desktop client.

- Confirm the permitted authentication flow, organization/admin requirements, project and license prerequisites, and whether an external client may send inference requests under the applicable Enterprise terms.
- Keep any approved Enterprise integration separate from personal Antigravity accounts and consumer quotas.
- If Google does not document and permit a suitable external-client flow, record Antigravity as unsupported instead of attempting to reuse its consumer OAuth session.

**Exit criteria:** either a documented, terms-compliant Enterprise integration path has concrete implementation requirements, or the roadmap records that Antigravity cannot be integrated as an external provider.

## 6. Expand desktop platforms

**Status: later.** Windows remains the supported first target. Bring the local Rust service and credential storage to Linux and macOS before mobile.

- Add platform-specific packaging, secure credential storage, data-directory resolution, and service lifecycle handling.
- Preserve the same SQLite conversation format and migration behavior across desktop platforms.
- Document installation, upgrade, and recovery paths for each supported platform.

**Exit criteria:** each desktop build packages its service, stores secrets in the platform's secure store, and keeps durable application data in that platform's user data directory.

## 7. Evaluate mobile support

**Status: later.** Android and iOS require a service lifecycle and secure-storage design that fits mobile process and permission limits. Start this work after desktop provider behavior is stable.

- Decide how Flutter invokes the Rust provider layer on mobile and how long-running requests survive app lifecycle changes.
- Define local data and credential handling for Android and iOS without assuming the desktop child-process model.
- Keep the UI responsive to mobile layouts, keyboards, connectivity changes, and interrupted background work.

## Explicit boundaries

- No centralized account, message, or credential service; OpenChat remains local-first and self-hostable in its app model.
- No file editing or shell execution by project tools.
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
