# OpenChat

<p align="center">
  <img src="brand/variants/openchat-horizontal.svg" alt="OpenChat" width="560" />
</p>

OpenChat is a cross-platform AI chat app. It aims to give people one focused place to talk with hosted AI services and, over time, models running on their own devices. People will be able to choose an available model, have a conversation, and return to their chat history from the same app.

## Why OpenChat

AI models are accessed through different services, accounts, and local runtimes. OpenChat aims to bring those options into one chat experience, with a clear choice of model and support for the connection methods available for each integration.

## Product goals

- **One place for AI conversations:** Build a clear chat experience that can grow to support multiple hosted and local models.
- **Choice of model:** Let people select from the models available through their configured connections.
- **Conversation history:** Make previous chats available to return to.
- **Flexible connections:** Integrate both OAuth and API key connection methods, where supported by an integration.
- **Responsive performance:** Keep the app responsive as conversations and supported platforms grow.
- **Multiple platforms:** Start on Windows, with a path toward Linux, macOS, Android, and iOS.

## First release

The first release is planned as a Windows chat app. ChatGPT and OpenCode are the starting integrations, with additional API-key providers available as their account connections are configured. The initial scope is:

- Start and continue AI conversations.
- Choose an available model from a configured provider.
- Browse and reopen conversation history.
- Connect through the available OAuth or API key method.

This release is focused on conversation. Editing files is outside its scope.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the current implementation status, ordered next steps, and longer-term provider and platform plans. It distinguishes shipped code from work that still needs validation or implementation.

## First-release boundaries

- Chat is the product focus.
- File editing is not part of the first release.
- Windows is the only first-release platform target.

## Current status

The Windows app uses Flutter for the interface and a local Rust service for ChatGPT OAuth, account and workspace selection, model and quota reads, and streaming Responses. When a selected model returns a reasoning summary, OpenChat stores and displays that summary separately from the answer; the raw hidden reasoning is not exposed by the API. OAuth tokens are stored as byte blobs in Windows Credential Manager within its per-credential size limit. Conversations, messages, project names, favorite models, and project folders are stored locally in `%LOCALAPPDATA%\OpenChat\db\openchat.sqlite3`, with logs and cache directories under `%LOCALAPPDATA%\OpenChat\`. Startup retries transient local storage or service failures for up to three attempts; the chat sidebar offers a retry if initialization still fails. Startup phase, attempt, and safe error-code diagnostics are written to `%LOCALAPPDATA%\OpenChat\logs\openchat-app.log`. They do not include exception text, paths, tokens, or conversation data. Sanitized OAuth, model catalog, and usage diagnostics are written to `%LOCALAPPDATA%\OpenChat\logs\openchat-service.log`; they include safe error codes, HTTP statuses, durations, and item counts while excluding callback parameters, response bodies, tokens, and account details. File tools are enabled for every conversation. Settings offers `Onay İste` and `Tam erişim`; `Onay İste` is the default, asks once per supported file-tool call, and confines canonical paths to the attached project folder or `%LOCALAPPDATA%\OpenChat`. `Tam erişim` skips prompts and permits the same listing, search, read, file-information, write, and edit tools to operate anywhere. Shell commands are unsupported. The model catalog compatibility version is `0.157.0`, tracked independently of OpenChat's product version and aligned with the Codex `client_version` query parameter. The integration follows the public Codex OAuth client configuration and private ChatGPT endpoints; provider compatibility can change and requires a real account sign-in to confirm.

Data from earlier installations is not imported automatically. OpenChat reads app state only from `%LOCALAPPDATA%\OpenChat`.

Before each ChatGPT request, OpenChat checks the access token's JWT expiration and refreshes it when it is within five minutes. An unexpected `401` still triggers a refresh and one retry.

OpenCode Console is available as a chat provider. OpenChat caches the model catalogs for six hours; an explicit refresh bypasses the cache, and an expired catalog remains available as stale data if refresh fails. ChatGPT catalogs are scoped to an account and workspace. The OpenCode integration currently supports only the OpenAI Chat Completions family, including free models and listed paid models. Paid models require an OpenCode Console service API key, stored with the platform secure-storage plugin, and are billed per request by OpenCode. OpenCode's Responses, Anthropic Messages, Gemini, and System One protocol families are not supported by this OpenCode integration. See [OpenCode's inference documentation](https://opencode.ai/v2/docs/console/inference/) and [model list and pricing](https://opencode.ai/v2/docs/console/models/) for current endpoint and billing details.

Gemini, Groq, Cerebras, and OpenRouter are available through their official OpenAI-compatible chat APIs. Their API keys are stored by the platform secure-storage plugin. Provider model catalogs are cached for six hours per provider and key; the cache stores only a SHA-256 key hash with model metadata. Gemini, Groq, and Cerebras show the models their authenticated catalogs return, so model price and quota depend on the provider account. OpenRouter shows only text-chat models whose current catalog reports zero input and output prices and tool support. Gemini's unpaid API tier may use submitted content to improve Google products; its connection card shows this before a key is added. The integrations are implemented in source but still need verification with real provider accounts.

OpenCode's model endpoint supplies the available model IDs. Exact-ID matches in [Models.dev's OpenCode catalog](https://models.dev/) supply display names, descriptions, and context-window values. This metadata shares the six-hour catalog cache; if its source is unavailable, cached values are marked stale when available, while models remain selectable without metadata when no cached details exist.

OpenCode reasoning controls follow the model catalog's supported effort values. OpenAI-compatible reasoning selections are sent as `reasoning_effort`; models without request-time controls use their provider default.

New conversations use one provider and model picker. ChatGPT, OpenCode, Gemini, Groq, Cerebras, OpenRouter, and saved favorites appear in its provider panel. API-key providers stay disabled until a key is connected. A favorite can be selected from another provider to switch the new conversation to that provider.

Shared instructions are saved locally in Settings and sent with each chat request. The Rust service converts requests and events to OpenChat's shared chat, tool, and stream-event types, then each adapter maps those types to the provider's native request and response format. ChatGPT receives instructions through the Responses `instructions` field; OpenCode and the four OpenAI-compatible APIs receive them as a Chat Completions `system` message. Provider-native payloads stay inside the service and do not cross the Flutter RPC boundary. File tools are sent with every chat request and run in the local Rust service. In `Onay İste`, each supported call pauses until the user allows or denies it, and the backend independently restricts access to the project folder or `%LOCALAPPDATA%\OpenChat`; in `Tam erişim`, those prompts are skipped and listing, search, read, file-information, write, and edit tools can access any folder. Shell commands are unsupported. Tool activity, approval state, arguments, and results are saved with local conversation history. Tool calls are limited to six rounds, sixteen calls, and 16 KiB of arguments per call. Provider tool support still needs live verification for each selected model.

Appearance settings let people choose the conversation column width and app-wide text size, and set the typeface used throughout OpenChat. Code blocks keep a monospace typeface. These choices are saved locally.

People can delete individual conversations after confirmation, export a conversation with its exposed reasoning summaries and tool activity as Markdown, and retry the latest assistant response. A retry keeps the previous response until the replacement completes successfully and uses the conversation's saved provider and model route.

The Windows portable release is distributed as one `OpenChat.exe`. On first launch it unpacks the bundled release payload to a content-addressed directory under `%LOCALAPPDATA%\OpenChat\cache` and starts the app from there. The database and logs remain in their separate persistent directories. Build this release with `tools/build_windows_portable.ps1`.

## License

OpenChat is distributed under the MIT License. See [LICENSE](LICENSE) for details.
