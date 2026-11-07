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

Import rules are validated automatically (`npm run check:imports`). Lower layers cannot import upper layers. Cross-feature imports use the owning feature's declared public entrypoint; imports into another feature's implementation files are reported as warnings. An unreadable source file or an unavailable source tree fails the check.

### Local Service

The Rust sidecar owns all provider connections, tool execution, storage, and security. It runs as a stdio process spawned by the Tauri host. Key responsibilities:

- **Provider adapters** — ChatGPT (OAuth), OpenAI (API key), Anthropic, Gemini, and 15+ compatible providers
- **Tool execution** — File I/O, terminal, web search, and MCP. Windows local stdio MCP runs in AppContainer; Linux terminal and local stdio MCP processes use Bubblewrap in approval and safe-operation modes. Linux MCP sandboxes have no network; host system files are read-only and writes are limited to the project directory and sandbox `/tmp`.
- **Storage** — SQLCipher (AES-256) encrypted SQLite database, brokered through an AppContainer child on Windows and stored in the private XDG data directory on Linux
- **Credentials** — Windows Credential Manager or Linux Secret Service for the profile key; provider credentials are held in the encrypted database vault

The Tauri host also integrates native desktop notifications on Windows and Linux. Linux startup-at-login uses the user's XDG autostart directory. Linux desktop computer-use automation is not currently available.

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
| **Process isolation** | Windows AppContainer / Linux bubblewrap for isolated terminal processes; Linux requires Bubblewrap and enabled unprivileged user namespaces |
| **WebView2 GPU process** | WebView2's default GPU, renderer, and AudioService sandbox settings remain enabled |
| **Storage encryption** | SQLCipher AES-256; broker-owned in the Windows AppContainer private folder, or stored in the Linux user's XDG data directory with mode 0700 |
| **Secrets** | Profile key in Windows Credential Manager or Linux Secret Service; vault values encrypted by SQLCipher and zeroized on read |
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
