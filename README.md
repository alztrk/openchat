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

The first release is planned as a Windows chat app. ChatGPT is the starting provider. The initial scope is:

- Start and continue AI conversations.
- Choose an available ChatGPT model.
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

The Windows app uses Flutter for the interface and a local Rust service for ChatGPT OAuth, account and workspace selection, model and quota reads, and streaming Responses. When a selected model returns a reasoning summary, OpenChat stores and displays that summary separately from the answer; the raw hidden reasoning is not exposed by the API. OAuth tokens are stored as byte blobs in Windows Credential Manager within its per-credential size limit. Conversations, messages, project names, favorite models, and project folders are stored locally in `%LOCALAPPDATA%\OpenChat\db\openchat.sqlite3`, with logs and cache directories under `%LOCALAPPDATA%\OpenChat\`. File tools are enabled for every conversation. Settings offers `Onay İste` and `Tam erişim`; `Onay İste` is the default, asks once per tool call, and confines canonical paths to the attached project folder or `%LOCALAPPDATA%\OpenChat`. `Tam erişim` skips prompts and lets the same read-only listing, search, read, and file-information tools use absolute paths anywhere. OpenChat does not edit files or run commands. Sanitized OAuth, model catalog, and usage diagnostics are written to `%LOCALAPPDATA%\OpenChat\logs\openchat-service.log`; they include safe error codes, HTTP statuses, durations, and item counts while excluding callback parameters, response bodies, tokens, and account details. The model catalog compatibility version is `0.157.0`, tracked independently of OpenChat's product version and aligned with the Codex `client_version` query parameter. The integration follows the public Codex OAuth client configuration and private ChatGPT endpoints; provider compatibility can change and requires a real account sign-in to confirm.

Data from earlier installations is not imported automatically. OpenChat reads app state only from `%LOCALAPPDATA%\OpenChat`.

OpenCode Console is also available as a chat provider. OpenChat caches the model catalogs for six hours; an explicit refresh bypasses the cache, and an expired catalog remains available as stale data if refresh fails. ChatGPT catalogs are scoped to an account and workspace. The OpenCode integration currently supports only the OpenAI Chat Completions family, including free models and listed paid models. Paid models require an OpenCode Console service API key, stored with the platform secure-storage plugin, and are billed per request by OpenCode. Responses, Anthropic Messages, Gemini, and System One model families are not supported by this integration yet. See [OpenCode's inference documentation](https://opencode.ai/v2/docs/console/inference/) and [model list and pricing](https://opencode.ai/v2/docs/console/models/) for current endpoint and billing details.

New conversations use one provider and model picker. ChatGPT, OpenCode, and saved favorites appear in its provider panel, and the model panel shows the selected list. A favorite can be selected from another provider to switch the new conversation to that provider.

Shared instructions are saved locally in Settings and sent with each chat request. The Rust service converts both providers to OpenChat's shared chat, tool, and stream-event types, then each adapter maps those types to the provider's native request and response format. ChatGPT receives instructions through the Responses `instructions` field; OpenCode receives them as a Chat Completions `system` message. Provider-native payloads stay inside the service and do not cross the Flutter RPC boundary. File tools are sent with every chat request and run in the local Rust service. In `Onay İste`, each valid call pauses until the user allows or denies it, and the backend independently restricts access to the project folder or `%LOCALAPPDATA%\OpenChat`; in `Tam erişim`, those prompts are skipped and the existing read-only tools can access any folder. Tool activity, approval state, arguments, and results are saved with local conversation history. Tool calls are limited to six rounds, sixteen calls, and 16 KiB of arguments per call. OpenCode's inference documentation identifies the endpoint families, but does not promise tool support for every model; provider and model compatibility must be confirmed against the selected model. Gemini has not been added as a provider yet.

Appearance settings let people choose the conversation column width, chat text size, and chat typeface. These choices are saved locally.

People can delete individual conversations after confirmation, export a conversation with its exposed reasoning summaries and tool activity as Markdown, and retry the latest assistant response. A retry keeps the previous response until the replacement completes successfully and uses the conversation's saved provider and model route.

The Windows portable release is distributed as one `OpenChat.exe`. On first launch it unpacks the bundled release payload to a content-addressed directory under `%LOCALAPPDATA%\OpenChat\cache` and starts the app from there. The database and logs remain in their separate persistent directories. Build this release with `tools/build_windows_portable.ps1`.

## License

OpenChat is distributed under the MIT License. See [LICENSE](LICENSE) for details.
