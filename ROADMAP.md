# OpenChat roadmap

This roadmap reflects the repository's current implementation. It has no calendar estimates; each stage depends on the acceptance criteria of the stage before it. The Windows desktop experience remains the first release target.

## Current implementation

- Flutter desktop UI with a local Rust child service and SQLite conversation storage under `%LOCALAPPDATA%\OpenChat`.
- ChatGPT OAuth connections, account/workspace selection, model catalog, account and quota information, streamed Responses, cancellation, and background conversation titles.
- OpenCode Console through its OpenAI Chat Completions endpoint. Other OpenCode protocol families are not supported.
- Gemini, Groq, Cerebras, OpenRouter, and Mistral through their official OpenAI-compatible Chat Completions APIs, with API keys in platform secure storage and six-hour per-key model catalogs. Mistral exposes context, vision, and documented reasoning capabilities; OpenRouter models are filtered to current zero-price text-chat entries that advertise tool support.
- Shared provider request, tool, and stream event types across ChatGPT, OpenCode, Gemini, Groq, Cerebras, OpenRouter, and Mistral, plus local shared instructions.
- Embedded release catalogs and a verified, cancellable installer for llama.cpp on Windows x64. vLLM and ExLlama are catalogued but installation stays blocked until their full runtime dependencies can be pinned and verified. Settings supports a model folder per engine and bounded discovery of unregistered GGUF and Transformers model files, with explicit confirmation before registration. Windows x64 llama.cpp can start a selected GGUF on demand, wait for health readiness, stream chat through the shared route, stop on cancellation or service shutdown, and show localized startup/runtime failures. Real GGUF/device validation, bounded runtime logs, local context-window metadata, and image input remain open.
- Local workspace tools for file listing, search, reading, metadata, writing, and editing, plus web search, URL reading, and terminal command/session tools. Tool calls, arguments, progress, and results are stored with assistant messages and shown in the conversation UI.
- Global `Ask for approval` and `Full access` settings govern local file and terminal calls. Approval is per call; canonical path checks apply to filesystem tools and do not sandbox terminal processes.
- Conversation history, project grouping, model favorites, rename/delete/pin actions, retry, and Markdown export. Tool activity is included in Markdown exports.
- Per-conversation memory inspection, bounded hybrid FTS5 and optional local semantic archive search with dated source excerpts, and a confirmed reset for compacted context that preserves full history.
- A Windows portable executable build script and local data directories for the database, logs, and cache.

These bullets describe code present in the repository. They do not mean that every provider endpoint or account flow has been verified against a live service.

## 1. Confirm the Windows baseline

**Status: verified by the user.** The Windows app, ChatGPT, and OpenCode have been exercised repeatedly and are currently working. Do not treat baseline release validation as an open implementation task.

- Recheck these workflows as regression coverage when a change affects them.
- Validate API-key providers with live accounts when credentials become available; that work remains in the provider-specific stage below and does not block the confirmed Windows baseline.

**Exit criteria:** met for the current Windows, ChatGPT, and OpenCode baseline. Reopen this stage only if a regression or release-specific change requires it.

## 2. Stabilize the provider contract

**Status: next.** Keep provider-specific wire formats inside their adapters and make unsupported capabilities explicit to the user.

- Define which shared capabilities a provider/model can use, including streaming, reasoning summaries, and tools.
- Avoid sending tools to models that do not support them; show a clear reason when capability information is unavailable or a provider rejects the request.
- Keep cancellation, partial responses, tool errors, quota limits, and stale model catalogs consistent across providers.
- Recheck ChatGPT's private endpoints when they change; they are compatibility-sensitive and are not a stable public API contract.

**Exit criteria:** both existing providers report or handle capabilities and failures through the shared OpenChat contract without leaking provider payloads into Flutter.

## 3. Validate API-key providers

**Status: implemented in source; pending live validation.** Gemini, Groq, Cerebras, OpenRouter, and Mistral are connected through their official OpenAI-compatible endpoints.

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
- Verify managed llama.cpp startup, model loading, streamed chat, cancellation, model switching, and shutdown with real CPU and NVIDIA CUDA GGUF models on Windows x64. Add bounded, sanitized runtime diagnostics without exposing model paths or prompt content.
- Add local context-window metadata and only expose image, reasoning, and tool capabilities when the selected model/runtime supports them. Keep provider payloads and local process details inside the Rust service.
- Extend package selection, secure storage, process supervision, and data directories to Linux and macOS. Treat WSL2 as a separate managed Linux runtime for vLLM rather than claiming native Windows support.

**Exit criteria:** a user can select a hardware-compatible engine and model, install and verify both through OpenChat, start and stop the local runtime, send and cancel a streamed chat request, and reopen its conversation without a separate CLI or external inference API.

## 5. Add Gemini API OAuth

**Status: planned.** Let users authenticate to the Gemini Developer API with their Google account, without entering a Gemini API key. This is the Gemini API's documented OAuth flow and uses OpenChat's own OAuth client; it is separate from Gemini CLI and Code Assist authentication.

- Confirm the required Google Cloud project setup, Generative Language API enablement, OAuth consent configuration, scopes, and any app verification requirements for a distributable desktop client.
- Store and refresh OAuth credentials through the existing platform secure-storage boundary. Associate each connection with the Google Cloud project that owns its Gemini API quota and billing.
- Keep OAuth and API-key connections distinguishable in Settings and the model selector, while routing both through the shared provider contract.
- Verify account cancellation, consent denial, expired/revoked credentials, project configuration errors, quota exhaustion, and real model requests.
- Document that OAuth alone does not guarantee free use. Gemini API quota and billing follow the selected Cloud project and are not the Gemini CLI or Code Assist free quota.

**Exit criteria:** a user can connect a Google account through OpenChat's OAuth client and send Gemini API requests without an API key; setup, quota, billing, and authentication failures are clear, and no Gemini CLI or Code Assist OAuth credentials are reused.

## 6. Evaluate a compliant Antigravity integration

**Status: research required.** Do not use consumer Antigravity OAuth from OpenChat. Evaluate only a Google-documented Enterprise or Cloud integration, such as a supported Vertex AI or ADC route, if its product terms authorize a third-party desktop client.

- Confirm the permitted authentication flow, organization/admin requirements, project and license prerequisites, and whether an external client may send inference requests under the applicable Enterprise terms.
- Keep any approved Enterprise integration separate from personal Antigravity accounts and consumer quotas.
- If Google does not document and permit a suitable external-client flow, record Antigravity as unsupported instead of attempting to reuse its consumer OAuth session.

**Exit criteria:** either a documented, terms-compliant Enterprise integration path has concrete implementation requirements, or the roadmap records that Antigravity cannot be integrated as an external provider.

## 7. Expand desktop platforms

**Status: later.** Windows remains the supported first target. Bring the local Rust service and credential storage to Linux and macOS before mobile.

- Add platform-specific packaging, secure credential storage, data-directory resolution, and service lifecycle handling.
- Preserve the same SQLite conversation format and migration behavior across desktop platforms.
- Document installation, upgrade, and recovery paths for each supported platform.

**Exit criteria:** each desktop build packages its service, stores secrets in the platform's secure store, and keeps durable application data in that platform's user data directory.

## 8. Evaluate mobile support

**Status: later.** Android and iOS require a service lifecycle and secure-storage design that fits mobile process and permission limits. Start this work after desktop provider behavior is stable.

- Decide how Flutter invokes the Rust provider layer on mobile and how long-running requests survive app lifecycle changes.
- Define local data and credential handling for Android and iOS without assuming the desktop child-process model.
- Keep the UI responsive to mobile layouts, keyboards, connectivity changes, and interrupted background work.

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
- [llama.cpp releases](https://github.com/ggml-org/llama.cpp/releases)
- [vLLM installation guide](https://docs.vllm.ai/en/latest/getting_started/installation/)
- [ExLlamaV3 releases](https://github.com/turboderp-org/exllamav3/releases)
