# OpenChat Roadmap

OpenChat is a local-first desktop AI workspace. This roadmap captures planned work organized by priority and current implementation status. There are no fixed calendar estimates; work proceeds based on product value, safety requirements, and dependencies.

## Current state

- **Platform:** Windows 10/11 (64-bit) and Linux x86_64 (WebKitGTK 4.1); macOS remains planned
- **Stack:** React 19 frontend, Tauri v2 desktop host, Rust local service (SQLite + SQLCipher)
- **Core experience:** Chat, projects, model discovery, and settings are implemented; the reliability review below has open follow-up work.

## Completed

### Core experience
- Chat with ChatGPT (OAuth), OpenAI (API key), Anthropic Claude, Gemini, and 15+ compatible providers
- Streamed responses, tool calls, citations, and reasoning summaries
- Model catalog, favorites, and per-conversation model selection
- Conversation history, search, archive, rename, pin, and Markdown export
- ChatGPT OAuth quota snapshots and usage tracking

### Local tools and sandboxing
- File read/write/edit with diff review and revert
- Terminal execution with AppContainer / bubblewrap isolation
- Web search and URL reading with outbound URL policy
- Git operations (status, diff, history, worktrees) — read-only
- Model Context Protocol (MCP) — stdio and Streamable HTTP with tool-level permissions
- Query Language Server — TypeScript, Python, Rust, Go, and more
- Real-time credential redaction (DLP) across tool output and file reads

### Projects
- Project-scoped instructions, skills, and rules (`.openchat/`)
- Project conversation search with hybrid FTS5 + semantic embeddings
- Named terminal tasks and isolated Git worktrees
- Worktree diff preview before retention

### Security and privacy
- SQLCipher (AES-256) encrypted database in AppContainer private folder
- DPAPI-protected credentials vault with cross-session file locking
- Hard-link rejection, capability-rooted file opens, no path re-resolution
- Portable archives (selected-conversation v1, full-profile encrypted)
- Security scan system with read-only tool allowlist and consent gating

### Developer tooling
- Cargo workspace (service + domain, transport, adapters crates)
- Feature-Sliced Design frontend with import validation
- CI/CD: Biome lint, TypeScript check, Rust formatting, RustSec audit, npm audit, Tauri build
- Modular CLI (`npm run openchat --`) with dev, build, check, maintain-target tasks

### Linux desktop support
- Linux x86_64 Debian and AppImage packaging with Secret Service profile-key storage, Bubblewrap terminal isolation, desktop notifications, and XDG autostart integration
- Linux project MCP supports local stdio servers sandboxed with Bubblewrap and remote Streamable HTTP; desktop computer-use automation remains unavailable
- Linux llama.cpp lifecycle and model management remain planned

## In progress

### Reliability and usability review (2026-11-04)

A static source review identified the following open work. Cleanup safety and npm workspace lockfile authority have been addressed; runtime checks, a real clean dependency installation, and other regression work remain outstanding.

#### Priority 1 — Reproducible install and validation
- The root `package-lock.json` matches the frontend workspace manifest and is the npm workspace authority. The stale nested `apps/desktop/frontend/package-lock.json` was removed, and CI now keys its npm cache to the root lockfile. A real clean install remains to be validated; only an offline `npm ci --dry-run` has run so far.
- The local `check` task now includes frontend unit tests, locale consistency, FSD import checks, and the TypeScript/Vite asset build. CI uses the central task for this shared validation set; dedicated CI security, Rust lint, documentation, and installer steps remain additional checks.

#### Priority 2 — Frontend workflow feedback
- Correct the new-chat welcome state when no model is selected.
- Restore a reachable entry point for the attachment library dialog.
- Reconcile optimistic folder drag-and-drop state after a failed save, and surface folder preference write failures in the affected UI.
- Make the startup-surface setting validator accept every supported surface.
- Add rendered workflow coverage for loading, empty, error, cancellation, retry, and state-recovery behavior.

#### Priority 2 — Desktop lifecycle and responsiveness
- Restore saved window geometry and avoid showing an uninitialized window.
- Verify the WebView2 recovery retry target and reset crash tracking after recovery.
- Bound native dialog waits and RPC event payloads; avoid blocking sidecar writes while process locks are held.
- Avoid synchronous disk flushes for every window move/resize event and report persistence failures.

#### Priority 2 — Storage consistency and error contracts
- Keep attachment files and database references consistent when save, update, delete, or transaction commit fails.
- Apply the same attachment MIME/type rules at save and read boundaries; cover GIF and malformed metadata consistently.
- Enforce message-size limits after attachment metadata is encoded, paginate or expose truncation for long histories, and prevent reserved-prefix collisions.
- Return a conversation-not-found error for stale send requests and preserve run-state persistence failures instead of reporting a completed response.

#### Priority 2 — Local development network boundary
- The Vite dev server listens on `127.0.0.1:5173`. It is reachable by a browser or process on the same machine while running; this bind does not expose it to LAN interfaces. Measure browser-origin, HMR, and LAN behavior, and inspect listening ports in a packaged build before changing the boundary.
- The managed llama.cpp server also binds to loopback on a dynamically selected port. OpenChat does not pass it an API key or explicit CORS setting. The pinned upstream `b11349` source defaults CORS to `*` with credentials enabled and reflects the requesting origin. A page on the same machine may be able to call the server if it discovers the port and browser local-network policy permits the request. Verify this in a controlled browser-origin check; the loopback bind does not expose the endpoint to LAN interfaces.

#### Priority 2 — Windows first-run verification
- The NSIS installer requires a WebView2 runtime at least `111.0.1661.0`. Tauri defaults to downloading its bootstrapper when the runtime is missing or outdated, so that install path needs network access. Validate missing, outdated, and offline-runtime cases.
- CI builds and uploads the NSIS installer but does not install or launch it. Add an installed-app smoke path that covers WebView2 startup, sidecar startup, and first-profile initialization.

These items are from static review. Treat them as open until focused runtime or regression checks establish the expected behavior.

### Provider usage coverage
- Map provider-reported usage fields (thinking tokens, cache reads) to the local ledger
- Validate usage statistics against live provider accounts

### Accessibility (A-013)
- Keyboard navigation, focus visibility, high contrast, reduced motion
- Text scaling (125–200%), narrow layouts, long translations
- Native Windows UI Automation and screen-reader validation (post-release)

## Planned

### Priority 1 — Agent quality and reliability
- **Durable runs** — persistent run state, cancellation, restart, interrupted-run recovery
- **Message branches** — edit a message to create a new branch; response version selection
- **Subagents** — child runs with context, tool allowlist, concurrency/timeout/usage bounds
- **Reusable workflows** — parameterized recipes: initial prompt, context, provider, tools
- **Side-by-side model comparison (Split Chat)** — concurrent multi-model execution, side-by-side response comparison, latency and token metrics
- **Voice interaction (STT / TTS)** — native speech-to-text input (local Whisper / web speech), text-to-speech response playback, and hands-free voice controls

### Priority 2 — Provider expansion
- **Ollama and LM Studio** — dedicated connections with health checks and model discovery
- **Amazon Bedrock** — Converse/ConverseStream with regional config and AWS auth
- **Microsoft AI Foundry** — deployment identifiers and per-model API compatibility

### Priority 3 — Local inference
- **Linux/macOS llama.cpp** — packaging, installation, model management, runtime lifecycle
- **Local model catalog** — file size, progress, integrity verification, interrupted download recovery
- **Context-window metadata** — per-model capabilities (image, reasoning, tools)

### Priority 4 — Sync and distribution
- **Encrypted sync** — opt-in multi-device sync, device keys, conflict handling, offline behavior
- **Signed distribution** — Authenticode signing, SBOM, stable/preview channels, update verification
- **Cross-platform** — macOS packaging, credential storage, data directories

### Deferred
- **Reusable workflows with lifecycle hooks** (after Priority 1 baseline)
- **Mobile support** (Android, iOS) — after desktop is stable
- **Generalized delegated-agent architecture** — abstract subagent framework
- **Additional local engines** — separate evaluation before implementation

## Notes

- Live provider-account validation is a release gate, not current feature work
- Confirmed Windows baseline is stable; re-check as regression coverage
- Linux support is available for the documented subset; macOS follows once the core experience is stable
- Existing audit findings (2026-10-27 source review) are tracked in [security-review.md](./security-review.md)
