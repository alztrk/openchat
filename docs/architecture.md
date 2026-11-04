# Architecture

## Overview

OpenChat is a local-first desktop application built on three layers:

```
┌─────────────────────────────────────────────────────────────┐
│  Frontend (React 19, Tailwind CSS, FSD architecture)        │
│  apps/desktop/frontend/                                      │
└─────────────────────────────────────────────────────────────┘
                            │ NDJSON-RPC over stdio
┌─────────────────────────────────────────────────────────────┐
│  Local Service (Rust, SQLCipher, sandboxing)                │
│  services/openchat-local-service/                            │
└─────────────────────────────────────────────────────────────┘
                            │ Tauri IPC
┌─────────────────────────────────────────────────────────────┐
│  Desktop Host (Tauri v2, NSIS installer)                    │
│  apps/desktop/tauri/                                         │
└─────────────────────────────────────────────────────────────┘
```

### Frontend

The React frontend follows **Feature-Sliced Design (FSD)** with five layers:

| Layer | Purpose |
|-------|---------|
| `@shared` | UI primitives, i18n, platform APIs, utilities |
| `@entities` | Domain type definitions (conversation, model, project) |
| `@features` | Vertical slices: chat, settings, providers, security scans |
| `@widgets` | Composed UI sections built from features |
| `@app` | Routing, bootstrap, global state managers |

Import rules are validated automatically (`npm run check:imports`).

### Local Service

The Rust sidecar owns all provider connections, tool execution, storage, and security. It runs as a stdio process spawned by the Tauri host. Key responsibilities:

- **Provider adapters** — ChatGPT (OAuth), OpenAI (API key), Anthropic, Gemini, and 15+ compatible providers
- **Tool execution** — File I/O, terminal, web search, MCP stdio, with sandboxing via Windows AppContainer or Linux bubblewrap
- **Storage** — SQLCipher (AES-256) encrypted SQLite database managed by an AppContainer child broker
- **Credentials** — DPAPI-protected vault in the database, with Windows Credential Manager for the profile key

### Desktop Host

Tauri v2 wraps the React frontend and spawns the local service. It handles:

- Native file dialogs (save, directory picker) — never exposed to the renderer
- Process lifecycle management (spawn, monitor, kill)
- Capability scoping in `capabilities/default.json` — no unrestricted `fs:*` or `shell:*`

## Data flow

```
┌──────────┐    NDJSON-RPC     ┌──────────────────┐    SQLite    ┌──────────┐
│  Renderer│ ────────────────►│  Local Service   │ ────────────►│  Storage │
│  (React) │ ◄────────────────│  (Rust)          │ ◄───────────│(SQLCipher)│
└──────────┘   stdio/stdout   └──────────────────┘              └──────────┘
     │                                                    ▲
     │   Native dialogs                                   │
     ▼                                                    │
┌──────────┐                                          ┌──────────────┐
│  Tauri   │─────────────────────────────────────────│ AppContainer │
│  Host    │         Broker communication              │   Broker     │
└──────────┘                                          └──────────────┘
```

- **Renderer → Service:** Bounded RPC frames (max 24 MiB, 64 concurrent operations)
- **Service → Storage:** Broker process with no network capability, bounded FIFO queue
- **Renderer → Tauri:** Native dialogs, file picker — renderer never sees absolute paths

## Security model

| Boundary | Mechanism |
|----------|-----------|
| **Process isolation** | Windows AppContainer / Linux bubblewrap for tool processes |
| **WebView2 GPU process** | WebView2's default GPU, renderer, and AudioService sandbox settings remain enabled |
| **Storage encryption** | SQLCipher AES-256, broker-owned in AppContainer private folder |
| **Secrets** | DPAPI-protected vault, zeroized on read, cross-session file-lock |
| **Network** | Outbound URL policy rejects private/reserved/cloud-metadata; DNS rebinding protection |
| **Capabilities** | Tauri capabilities are scoped; no `fs:*` or `shell:*` in renderer |
| **File safety** | Hard-link rejection, capability-rooted opens, no path re-resolution after validation |

See [security-review.md](./security-review.md) for audit findings and status.

## Cargo workspace

```
services/openchat-local-service/
├── Cargo.toml          # Workspace manifest + package
├── src/                # Service code (lib.rs, main.rs, domain/, transport/, adapters/)
└── crates/
    ├── openchat-domain/   # Domain types (placeholder — actual code in src/domain/)
    ├── openchat-transport/ # RPC protocol types (placeholder)
    └── openchat-adapters/  # Storage/filesystem adapters (placeholder)
```

The workspace compiles as `cargo check --workspace` from `services/openchat-local-service/`.

## Build system

| Task | Command |
|------|---------|
| Local check | `npm run openchat -- check` |
| Type check only | `npm run openchat -- check-types` |
| Start dev server | `npm run openchat -- dev` |
| Build release | `npm run openchat -- build` |
| Validate import layers | `npm run check:imports` |

`check` runs Biome, frontend unit tests, locale consistency, FSD import checks, the TypeScript/Vite asset build, Rust formatting, and locked Rust checking/tests. `check-types` runs Biome, TypeScript, Rust formatting, and locked Rust checking without tests. CI uses the same `check` task for this shared validation set.
