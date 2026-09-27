# ChatGPT integration plan

## Goal

Deliver the first Windows release of Zihora as a local-first chat application. The first provider is ChatGPT. Flutter presents the interface; a local Rust service owns provider authentication and requests; one SQLite file stores conversation and integration data under `%LOCALAPPDATA%\Zihora\db`.

This document is the working plan for the ChatGPT implementation. It records the agreed behavior, security boundaries, failure handling, and delivery order in one place.

## Scope

- Multiple ChatGPT OAuth connections, with explicit account and workspace selection.
- Dynamic ChatGPT model catalogs per connection and workspace.
- Text conversations through the ChatGPT Responses stream.
- Local conversation and message history.
- Account and plan information, usage limits, reset times, and reset-credit details when the service returns them.
- A separate, non-blocking mechanism for AI-generated conversation titles.
- Windows as the first supported release target. The service boundary should remain usable by future desktop and mobile clients.
- All durable application files under `%LOCALAPPDATA%\Zihora\`: the SQLite database in `db`, logs in `logs`, and cache in `cache`.

## Out of scope for this release

- Providers other than ChatGPT.
- File editing, tools, or agents.
- Cloud synchronization or a centrally hosted service.
- Treating an OpenAI Platform API key as ChatGPT OAuth, or silently using it as a fallback.
- Automatically consuming a banked reset credit.
- Silently switching a conversation to another ChatGPT account.

## Architecture

```text
Flutter UI
  ├─ conversation and settings presentation
  └─ line-oriented JSON RPC over the local service process pipes
       └─ Rust service
            ├─ OAuth PKCE and token refresh
            ├─ ChatGPT account, workspace, model, quota, and response clients
            ├─ title eligibility and background work
            ├─ Windows credential storage for OAuth tokens
            └─ SQLite: %LOCALAPPDATA%\Zihora\db\zihora.sqlite3
```

The service is a child process owned by Zihora. It communicates over standard input and output, so it does not open a network listener. Standard output is reserved for protocol messages. Diagnostics must never contain access tokens, refresh tokens, authorization codes, API keys, message contents, or full provider responses.

OAuth tokens belong in Windows Credential Manager. SQLite stores a credential reference and non-secret account metadata. The SQLite database uses WAL mode and a bounded busy timeout. The first implementation shares the one database file with the existing Drift conversation tables; Rust-owned integration tables have their own migration version table and do not use SQLite's global `user_version`, which Drift owns.

## ChatGPT integration behavior

### Credential storage

OAuth tokens belong in Windows Credential Manager. SQLite stores only a credential reference and non-secret account metadata. The Rust adapter stores access and refresh tokens as separate entries under a new generation reference, serializes access within the service process, zeroizes token strings when dropped, and removes a newly written access token if storing its matching refresh token fails. SQLite must switch the active credential reference only after both entries are stored. Windows limits each credential blob to 2,560 bytes; oversized tokens produce an explicit error rather than being truncated or stored elsewhere. The Flutter RPC surface does not expose token reads.

### OAuth and accounts

- Use Authorization Code with PKCE, a cryptographically random state and verifier, a loopback callback, and strict state validation.
- Use the public Codex OAuth client ID `app_EMoamEEZ73f0CkXaXp7hrann` as the initial ChatGPT OAuth client identifier. It matches the value already used by archived Tengra and exposed by the open-source Codex login flow. Keep it in one named backend constant so it can be replaced without changing UI or persistence code.
- Match the Codex Authorization Code with PKCE contract for the initial flow: `https://auth.openai.com/oauth/authorize`, `https://auth.openai.com/oauth/token`, the registered loopback callback, `openid profile email offline_access api.connectors.read api.connectors.invoke`, `id_token_add_organizations=true`, and `codex_cli_simplified_flow=true`. Keep the client identifier separate from any confidential credential; a native desktop client cannot keep a client secret.
- Treat this as an implementation reference, not a stable public ChatGPT API contract. OpenAI may change client registration, allowed callbacks, scopes, OAuth consent, or private ChatGPT/Codex endpoints. Surface those failures clearly and keep the OAuth client identifier replaceable.
- Keep each local OAuth connection distinct from the external ChatGPT user and from each workspace. Do not use email as a stable identity key.
- Require an explicit workspace choice when an account exposes multiple workspaces. Preserve the account/workspace selected for each conversation.
- Refresh an expired token under a per-connection lock. On an explicit authentication failure, allow one serialized refresh and one retry. Do not repeat an ambiguous request that may already have been accepted.
- Remove or mark a connection unusable when refresh is rejected; retain its history and show a recoverable reconnect state.

### Models

- Fetch the model catalog from the authenticated ChatGPT/Codex service for the selected account and workspace.
- Keep model identifiers and capabilities as returned by the current catalog. Do not ship a fabricated or static model list.
- Cache catalogs by connection, workspace, and client version. A failed refresh may leave an explicitly stale cached catalog visible, but it must not be represented as current.
- Disable sending when there is no selected, available model.

### Responses and message history

- Send text using the ChatGPT Responses protocol and process its SSE lifecycle events.
- Persist the user message before the network request. Persist partial assistant output and its terminal state so an interrupted stream is visible after restart.
- Distinguish completed, failed, incomplete, stopped, rate-limited, and authentication-required outcomes.
- Treat stream deltas as provider events, not as guaranteed individual token boundaries. Compute displayed token counts and throughput only from authoritative usage/measurement data; otherwise show them as unavailable.
- Preserve the exact account, workspace, and model used for each conversation. A changed default applies only to new conversations.

### Quota, account, and reset information

- Store each quota read with account, workspace, retrieval time, and freshness state.
- Preserve multiple rate-limit buckets, usage percentages, durations, and reset timestamps when supplied.
- Preserve `true`, `false`, and unavailable/null separately for ordinary usage eligibility. A reset time or percentage alone is not proof that a request can run.
- Treat an actual provider `429` as authoritative for that request, even if a recent snapshot said usage was available.
- Keep ChatGPT/Codex subscription usage separate from OpenAI Platform API billing.
- Preserve plan values as returned, including unknown future values. Do not infer payment, renewal, or billing details that the endpoints do not provide.
- Keep reset-credit summary count separate from reset-credit details. Distinguish details omitted/null from a confirmed empty list.
- Reading reset information is automatic. Consuming a reset credit requires a deliberate user action and an idempotency key; it is never part of sending a message or generating a title.

### AI-generated conversation titles

Title creation is a separate background operation. It never holds the chat response open.

1. Keep `Yeni sohbet` until an eligible title request succeeds.
2. Never overwrite a title the user has edited or otherwise marked manual.
3. Choose a title model different from the conversation model.
4. Use the conversation's ChatGPT connection and workspace when another model is available. A title-only connection is eligible only after the user explicitly selects it for that purpose. Do not silently change accounts or fall back to the Platform API key.
5. Require a fresh quota read that explicitly permits ordinary usage for the chosen title connection. Unknown, null, stale, failed, or denied quota means skip title generation and retain the default title.
6. Do not consume banked reset credits for titles.
7. Use a short prompt containing only the conversation context needed for a title. Store the job outcome and reason, not authorization data.
8. On a failed title request, keep the conversation and its messages intact. Do not automatically retry an ambiguous request. A single retry is allowed only after a definitive authentication failure and a successful serialized token refresh.

### Title eligibility matrix

| Situation | Action | Conversation result |
| --- | --- | --- |
| A different model is available on the conversation connection and fresh ordinary quota is allowed | Queue a background title request | Chat continues; title changes only after success |
| No different model is available on the conversation connection, but the user explicitly selected a title-only connection with a different model and fresh ordinary quota is allowed | Queue a background title request on that selected connection | Chat continues; title changes only after success |
| No different model is available and no title-only connection was explicitly selected | Skip | Keep `Yeni sohbet` or current title |
| Quota is absent, stale, null, or could not be fetched | Skip | Keep current title; no quota is spent |
| Quota explicitly denies ordinary usage | Skip | Keep current title |
| Only banked reset credits could make a request possible | Skip | Never consume a credit automatically |
| An explicitly selected title-only connection is not configured or needs user authorization | Skip | Keep current title; do not switch accounts |
| The user edited the title | Do not queue, or discard a late result | Preserve manual title |
| Provider returns a definitive 401 and refresh succeeds | Retry once under the same connection | Apply only a valid generated title |
| Provider returns 401 and refresh fails | Stop | Preserve chat and current title; mark connection for reconnect |
| Provider returns 429, timeout, malformed data, or ambiguous failure | Do not retry automatically | Preserve chat and current title |
| The chat stream fails or is cancelled | Title scheduling may use only saved context and still must meet eligibility | Never block or rewrite the chat outcome |

## Failure and recovery states

| Failure | Required behavior |
| --- | --- |
| OAuth rejects the client ID, redirect URI, scope, or grant | Do not create a connection; show a safe sign-in error and preserve existing connections |
| User cancels browser sign-in | Return to settings without creating a connection |
| OAuth state mismatch or callback error | Reject the callback, do not save tokens, and show a safe error |
| Token refresh is rejected | Mark only that connection as requiring sign-in; keep its conversations |
| Account/workspace listing fails | Show unavailable state; never choose the first returned workspace implicitly |
| Model catalog fails | Show an explicit unavailable or stale state; do not fabricate models |
| Quota endpoint fails or omits a field | Mark that field unavailable; do not infer permission from a percentage or reset time |
| Responses stream fails after partial output | Save the partial answer as failed/incomplete and let the user retry explicitly |
| Provider returns rate limit | Keep the account/model selection and explain the rate limit; do not spend reset credits or change accounts |
| SQLite or local service is unavailable | Disable operations that depend on it and show a recoverable storage/service error |
| Private endpoint schema changes | Validate responses, keep secrets and raw payloads out of diagnostics, and return an explicit unsupported-response error |

## Delivery order

1. **Local service foundation (implemented):** Rust child process, pipe-based RPC, Windows packaging, fixed data directories, one SQLite file, versioned Rust-owned schema, and a Flutter startup connection.
2. **Conversation routing and title protection (implemented):** persist connection, workspace, and model as one immutable selection per conversation; retain null routing for existing conversations; mark manual titles and apply generated titles only while the title remains automatic.
3. **Credential storage (implemented):** Windows Credential Manager entries for versioned access/refresh token pairs, serialized operations, safe cleanup, and zeroization.
4. **OAuth and account selection (implemented):** use the public Codex client ID in one backend constant, PKCE callback, multiple account records, account reauthentication by stable external user ID, serialized token refresh, and explicit account/workspace selection.
5. **Account and model catalog (implemented):** account/profile and workspace details, dynamic catalog refresh/cache, explicit selection, and stale/unavailable UI states.
6. **Quota and reset details (implemented):** usage snapshots, multi-bucket limits, plan metadata, reset-credit summary/details, and safe user-facing errors. Reset credits are never consumed automatically.
7. **Text chat (implemented):** Responses request, SSE streaming, cancellation, partial-output persistence, and request error mapping.
8. **Title generation (implemented):** different-model selection, fresh ordinary-usage gate, non-blocking generation, and the failure behavior from the matrix above.
9. **Windows UI and package completion (implemented):** connect settings/chat and title-target settings, bundle the Rust service, produce the Windows executable and portable package, and verify the packaged artifacts.

## Acceptance criteria

- Windows starts the packaged Rust service without exposing a listening port.
- The service and Flutter conversation storage use the same SQLite file at `%LOCALAPPDATA%\Zihora\db\zihora.sqlite3`; logs and cache stay under the matching `logs` and `cache` directories.
- No OAuth token or secret is written to SQLite, application preferences, protocol output, or logs.
- A user can add and select more than one ChatGPT connection and workspace without cross-account chat history or silent fallback.
- Models, account/plan information, usage buckets, and reset details are presented only when returned by the selected connection, with freshness/unavailable states retained.
- A text response streams, survives restart in local history, and exposes interruption, authentication, and quota failures accurately.
- Title generation never delays a response, never spends a reset credit, never silently changes accounts, and never overwrites a manual title.
- OpenAI Platform API-key behavior remains separate from ChatGPT OAuth behavior.
- Windows packaging includes the Rust service executable beside the app.
- OAuth uses the public Codex client ID `app_EMoamEEZ73f0CkXaXp7hrann`; provider acceptance of Zihora's callback and private ChatGPT endpoints must be confirmed by a real account sign-in.

## Compatibility and setup risks

The Codex repository is an implementation reference, not a guarantee that ChatGPT web backend endpoints are a supported public API. Model catalogs, Responses, account/profile, usage, and reset-credit routes can change without notice. Keep endpoint parsing isolated, validate all returned data, and show explicit compatibility errors. The public OpenAI Platform API has a separate API-key authentication and billing contract; it is not a drop-in replacement for ChatGPT OAuth.

The initial OAuth implementation uses the public Codex client ID already present in the Tengra archive and open-source Codex. It follows Codex's current loopback callback contract: `http://127.0.0.1:1455/auth/callback`, with its registered fallback port `1457`, Authorization Code with PKCE, and the current Codex scopes and authorization parameters. This does not make ChatGPT/Codex's private endpoints a supported public API or guarantee that OpenAI will accept Zihora's redirect and originator behavior. Keep the OAuth parameters isolated and handle provider rejection explicitly. A live account sign-in is required to establish provider acceptance; a successful local build alone cannot establish it.

## Windows build status

- Single-file portable release: `build/outputs/zihora.exe`.
- Build it with `tools/build_windows_portable.ps1`. The script builds the Flutter release, embeds the complete release folder in a small native launcher, and preserves the Zihora icon.
- On first launch, the launcher checks the embedded archive against its build-time SHA-256, rejects unsafe archive paths, and verifies required files. It extracts atomically to `%LOCALAPPDATA%\Zihora\cache\bundles\bundle-<sha256>`, then starts the Flutter app from that directory. The app starts its Rust service beside itself. The database, logs, and other application data remain under their agreed directories.
- Content-addressed cache bundles prevent updated executables from reusing a partial or stale extraction. Old bundle directories are retained; they can be removed from `%LOCALAPPDATA%\Zihora\cache\bundles` when Zihora is closed.
- `flutter analyze`, `cargo fmt --all --check`, and `cargo check --locked` completed successfully for the ChatGPT implementation. Automated tests were not run at Alican's request.
- A live ChatGPT OAuth sign-in and provider endpoint compatibility remain unverified until an account completes the browser flow in the packaged application.

## Research references

- [OpenAI Codex OAuth callback and PKCE implementation](https://github.com/openai/codex/blob/main/codex-rs/login/src/server.rs)
- [OpenAI Codex login source exports its default OAuth client ID](https://github.com/openai/codex/blob/main/codex-rs/login/src/lib.rs)
- [OpenAI Codex login URL showing the public client ID](https://github.com/openai/codex/issues/5673)
- [OpenAI Codex dynamic model catalog client](https://github.com/openai/codex/blob/main/codex-rs/codex-api/src/endpoint/models.rs)
- [OpenAI Codex Responses client](https://github.com/openai/codex/blob/main/codex-rs/core/src/client.rs)
- [OpenAI Codex usage and reset-credit client](https://github.com/openai/codex/blob/main/codex-rs/backend-client/src/client/rate_limit_resets.rs)
- [OpenAI Codex account rate-limit response types](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/account.rs)
- [OpenAI Codex reset-credit endpoint discussion](https://github.com/openai/codex/issues/29618)
- [OpenAI Responses streaming documentation for the separate Platform API](https://developers.openai.com/api/docs/guides/streaming-responses)
- [Rust keyring secure-store API and Windows feature](https://docs.rs/keyring/3.6.3/keyring/)
- [Rust keyring entry operations](https://docs.rs/keyring/3.6.3/keyring/struct.Entry.html)
- [Microsoft Windows credential blob size limit](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)
