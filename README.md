# OpenChat

<p align="center">
  <img src="apps/desktop/assets/brand/variants/openchat-horizontal.svg" alt="OpenChat" width="560" />
</p>

OpenChat is a local-first, privacy-focused desktop application designed for developers and technical workflows. It brings cloud AI providers, local inference runtimes, and isolated workspace tools together into a single, high-performance interface.

---

## Capabilities

### Unified Provider Support
Connect to multiple model ecosystems through standard, secure protocols:
- **Cloud Providers:** Direct integration with ChatGPT (OAuth), Anthropic Claude, Google Gemini, Mistral, DeepSeek, Groq, Cerebras, OpenRouter, Perplexity, xAI, Together AI, SambaNova, Fireworks AI, and Cohere, alongside custom OpenAI- and Anthropic-compatible endpoints.
- **Local Inference:** Built-in runtime management for `llama.cpp` supporting CPU and NVIDIA CUDA GPU acceleration, enabling offline GGUF model execution without external services.
- **Credential Storage:** Keys and tokens are kept in the operating system credential store (Windows Credential Manager / DPAPI). Credentials are never exposed to UI renderers or written to diagnostic logs.

### Image Design Workspace
- **ChatGPT Image Generation:** Create images in the Design workspace with a ChatGPT OAuth account or an OpenAI API key. The chat `generate_image` tool uses the same generation service.
- **Image Library:** Browse images generated in Design and chat from one local library. Search and filter results, compare variants, reuse prompts, save prompt templates, and organize images with favorites, tags, and collections.
- **Generation Controls:** Configure supported model options such as dimensions, quality, background treatment, and API output format. ChatGPT OAuth image generation uses the account's normal ChatGPT quota; there is no separate image quota. OpenAI API limits apply to API-key generation requests.

### Workspace Tools and Sandboxing
Run development tasks safely through configurable permission boundaries:
- **Filesystem Operations:** Read, search, write, and patch files with side-by-side diff review and revert capability.
- **Process Isolation:**
  - **Windows AppContainer (Default):** Isolates local tool processes in approval and safe-operation modes with filesystem and network boundaries.
  - **WSL2 Container Sandbox (Opt-in):** Uses `bubblewrap` (`bwrap --unshare-all`) for local tool processes in approval and safe-operation modes.
  - **Full access:** Lets file tools use any absolute path allowed to the current account, and runs terminal commands, configured project tools, and project MCP servers used as chat tools without AppContainer or bubblewrap isolation. It does not elevate the process or bypass Windows ACLs or UAC. Global MCP catalog discovery remains sandboxed.
- **Trust Handoff Protection:** Sensitive repository paths, including `.git/hooks` and IDE configurations, require explicit confirmation (`Ask`) before modifications.
- **Live Data Loss Prevention (DLP):** Real-time secret and token masking (`[REDACTED]`) sanitizes terminal outputs, tool calls, and file reads to prevent credentials from entering model context.
- **Model Context Protocol (MCP):** Connect to local stdio and remote Streamable HTTP MCP servers with granular tool-level permissions.

### Context Management and Retrieval
- **Hybrid Search Engine:** Combines full-text keyword indexing (SQLite FTS5) with local multilingual embeddings (ONNX) for semantic codebase recall.
- **Context Compaction:** Bounded summarization and checkpointing prevent context window overflow while preserving message history.
- **Projects and Skills:** Organize workspaces with project-specific instructions, declared tasks, and reusable skills (`SKILL.md`).

### Execution Modes
- **Plan Mode (`/plan`):** Instructs models to formulate detailed technical proposals before tools can be invoked.
- **Goal Mode (`/goal`):** Autonomous multi-turn workflow execution with real-time checklists, progress tracking, and interrupt recovery.

---

## Getting Started

### Requirements
- Windows 10 or 11 (64-bit)
- Node.js 22 and npm
- Rust 1.95 (pinned by `rust-toolchain.toml`)

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/alztrk/openchat.git
   cd OpenChat
   ```

2. Compile the backend service:
   ```powershell
   cargo build -p openchat-local-service --release
   New-Item -ItemType Directory -Force apps/desktop/tauri/binaries
   Copy-Item -LiteralPath target/release/openchat-local-service.exe -Destination apps/desktop/tauri/binaries/openchat-local-service-x86_64-pc-windows-msvc.exe -Force
   ```

   The full sidecar and installer sequence is also available as `.\scripts\build.ps1`.

3. Install frontend dependencies and start development mode:
   ```bash
   cd apps/desktop/frontend
   npm install
   npm run tauri:dev
   ```

### Packaging

To build the standalone installer:
```bash
cd apps/desktop/frontend
npm run tauri:build
```

---

## Architecture Principles

- **Local-First Design:** Chat transcripts and vector stores reside in local SQLite databases under `%LOCALAPPDATA%\OpenChat`. No telemetry or personal information is transmitted to third parties.
- **Explicit Authorizations:** `Ask for approval` and `Approve safe operations` retain process isolation. `Full access` disables isolation for agent tool execution and uses the current account's normal OS permissions; it does not grant administrator privileges. The separate global MCP catalog probe remains sandboxed.
- **Technical Documentation:** Architecture, design, roadmap, and security notes are indexed in [docs/README.md](docs/README.md).

---

## License

OpenChat is open-source software licensed under the [MIT License](LICENSE).
