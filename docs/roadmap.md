# OpenChat Roadmap

OpenChat is a local-first desktop AI workspace. This roadmap captures planned work organized by priority and current implementation status. There are no fixed calendar estimates; work proceeds based on product value, safety requirements, and dependencies.

## Current state

- **Platform:** Windows 10/11 (64-bit), with Linux/macOS planned
- **Stack:** React 19 frontend, Tauri v2 desktop host, Rust local service (SQLite + SQLCipher)
- **Core experience:** Chat, projects, model discovery, settings — working and validated

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

## In progress

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
- **Cross-platform** — Linux/macOS packaging, credential storage, data directories

### Deferred
- **Reusable workflows with lifecycle hooks** (after Priority 1 baseline)
- **Mobile support** (Android, iOS) — after desktop is stable
- **Generalized delegated-agent architecture** — abstract subagent framework
- **Additional local engines** — separate evaluation before implementation

## Notes

- Live provider-account validation is a release gate, not current feature work
- Confirmed Windows baseline is stable; re-check as regression coverage
- Linux/macOS support follows Windows once the core experience is stable
- Existing audit findings (2026-10-27 source review) are tracked in [security-review.md](./security-review.md)
