# Zihora

<p align="center">
  <img src="brand/variants/zihora-horizontal.svg" alt="Zihora" width="560" />
</p>

Zihora is a cross-platform AI chat app. It aims to give people one focused place to talk with hosted AI services and, over time, models running on their own devices. People will be able to choose an available model, have a conversation, and return to their chat history from the same app.

## Why Zihora

AI models are accessed through different services, accounts, and local runtimes. Zihora aims to bring those options into one chat experience, with a clear choice of model and support for the connection methods available for each integration.

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

1. **Windows chat:** Build the first usable version with ChatGPT, model selection, chat history, and the existing OAuth and API key connection options.
2. **More model choices:** Expand to additional hosted services and models that users run locally.
3. **More platforms:** Bring Zihora to Linux, macOS, Android, and iOS.

The roadmap describes project direction; it does not imply that a feature or platform is already available.

## First-release boundaries

- Chat is the product focus.
- File editing is not part of the first release.
- Windows is the only first-release platform target.

## Current status

The Windows app uses Flutter for the interface and a local Rust service for ChatGPT OAuth, account and workspace selection, model and quota reads, and streaming Responses. When a selected model returns a reasoning summary, Zihora stores and displays that summary separately from the answer; the raw hidden reasoning is not exposed by the API. OAuth tokens are stored as byte blobs in Windows Credential Manager within its per-credential size limit. Conversations, messages, project names, and selected project folder paths are stored locally in `%LOCALAPPDATA%\Zihora\db\zihora.sqlite3`, with logs and cache directories under `%LOCALAPPDATA%\Zihora\`. A project folder is only an organization reference; Zihora does not read or modify its files. Sanitized OAuth, model catalog, and usage diagnostics are written to `%LOCALAPPDATA%\Zihora\logs\zihora-service.log`; they include safe error codes, HTTP statuses, durations, and item counts while excluding callback parameters, response bodies, tokens, and account details. The model catalog compatibility version is `0.157.0`, tracked independently of Zihora's product version and aligned with the Codex `client_version` query parameter. The integration follows the public Codex OAuth client configuration and private ChatGPT endpoints; provider compatibility can change and requires a real account sign-in to confirm.

The Windows portable release is distributed as one `zihora.exe`. On first launch it unpacks the bundled release payload to a content-addressed directory under `%LOCALAPPDATA%\Zihora\cache` and starts the app from there. The database and logs remain in their separate persistent directories. Build this release with `tools/build_windows_portable.ps1`.

## License

Zihora is distributed under the MIT License. See [LICENSE](LICENSE) for details.
