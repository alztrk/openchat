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
- **Credential Storage:** On Windows, provider API keys, MCP secrets, and OAuth token pairs are DPAPI-protected in the SQLCipher database. Windows Credential Manager stores the database key and can contain legacy credential entries that OpenChat migrates into the protected vault. API keys entered in provider settings pass through the React renderer and Tauri IPC to the native host; credential-status commands return configuration state and redacted suffixes, not saved full keys.

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

2. Install frontend dependencies and start development mode:
   ```bash
   cd apps/desktop/frontend
   npm ci
   npm run openchat -- dev
   ```

3. Run workspace validation:
   ```bash
   cd apps/desktop/frontend
   npm run openchat -- check
   ```

### Packaging

To build the standalone installer:
```bash
cd apps/desktop/frontend
npm run openchat -- build
```

---

## Architecture Principles

- **Local-First Design:** Chat transcripts and vector stores reside in local databases. On Windows, the active database is SQLCipher-encrypted in the storage broker's private AppContainer data folder, with the profile key held in the current user's Windows Credential Manager; packaged broker and restore/export validation is still pending. Cloud-provider requests can include conversation content, attachments, and opt-in project notes or excerpts from prior conversations in the attached project. Design-canvas requests include your design brief and, when revising or reflowing a frame, its title, dimensions, style tokens, and SVG. Security scans send the selected model route the project content they read from the selected roots; queries go to the configured web-search service, and pages read during the scan can also enter the model context. Web searches send their query to the search services used by OpenChat, and page-reading requests contact the selected public URLs. Review the selected provider and search service privacy terms before sending sensitive content. OpenChat does not operate a centralized account or sync service. Conversation deletion and history clearing affect the active database; verified migration backups are retained without automatic expiry and can still contain older copies of deleted content. Those backups do not have an in-app restore flow, and historical filesystem snapshots may retain plaintext blocks from before encryption.
- **Chat-Scoped MCP Data Flow:** Project-configured MCP tool arguments go to the configured local or remote server, and returned tool results are included in subsequent requests to the selected model provider as conversation context.
- **Explicit Authorizations:** `Ask for approval` and `Approve safe operations` retain process isolation. `Full access` disables isolation for agent tool execution and uses the current account's normal OS permissions; it does not grant administrator privileges. The separate global MCP catalog probe remains sandboxed.
- **Technical Documentation:** Architecture, design, roadmap, and security notes are indexed in [docs/README.md](docs/README.md).

---

## License

OpenChat is open-source software licensed under the [MIT License](LICENSE).
