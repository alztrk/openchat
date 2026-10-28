# Instructions for AI Engineering Agents

Welcome to OpenChat. This repository contains the OpenChat desktop application, comprising a Rust local sidecar service, a Tauri v2 desktop host, and a React/TypeScript frontend.

As an AI engineering agent working in this repository, you **MUST** strictly adhere to the following build, workflow, and architecture rules.

---

## 1. Mandatory Target Directory and Cargo Rules

### Strict Single Target Directory Policy: `target/debug`
* **Agents MUST use only the single standard default target directory: `target/debug`.**
* **NEVER** create custom target directories inside the repository (e.g. `cargo test --target-dir target/<task-name>` or `CARGO_TARGET_DIR=target/<anything>` is **STRICTLY PROHIBITED**).
* Creating ad-hoc subdirectories inside `target/` causes massive disk bloat (over 100 GiB of duplicate intermediate C/C++ libraries, `.obj` files, and Windows `.pdb` debug symbols) and breaks incremental compilation caching.
* When invoking `cargo`, do not pass `--target-dir` unless pointing to an ephemeral directory under the operating system temporary folder (`%TEMP%` on Windows, `/tmp` on Linux) that is immediately deleted upon task completion.

### Standard Build and Dev Entry Points
All developer and agent tasks must use the centralized CLI via npm scripts in `apps/desktop/frontend`:

```bash
# Check formatting, linter, Rust checking, and tests:
npm run openchat -- check

# Start the desktop application in development mode:
npm run openchat -- dev

# Run desktop tests with an isolated ephemeral profile (auto-cleaned):
npm run openchat -- dev-test-profile --cleanup-profile

# Inspect Cargo target directory disk usage:
npm run openchat -- maintain-target

# Clean Cargo target directories when usage exceeds the limit:
npm run openchat -- maintain-target --apply
```

---

## 2. Target Directory Disk Budget and Hygiene

* The target directory has a strict soft ceiling of **20 GiB**.
* Before ending tasks involving heavy builds, verify that no temporary artifacts or orphaned target directories were left behind.
* If target disk usage exceeds 20 GiB, run:
  ```bash
  npm run openchat -- maintain-target --apply
  ```
* Never commit or stage anything inside `target/`. The `.gitignore` must always cover all build outputs.

---

## 3. Engineering and Safety Standards

### Architecture and Boundaries
* **Local Sidecar Service (`services/openchat-local-service`):**
  * Implements storage (SQLCipher AES-256), tool execution sandboxing (Windows AppContainer / Linux Bubblewrap), SSRF protections, and LLM provider adapters.
  * Secrets and credentials must use zeroization (`zeroize`) and Windows DPAPI / Credential Manager.
  * Subagents must never be given recursive execution tools (`delegate_task`, `computer_use`, `ask_user`, memory mutations, or goal tools).
* **Tauri Host (`apps/desktop/tauri`):**
  * Tauri v2 desktop shell. Permissions are strictly scoped in `capabilities/default.json`.
  * Never grant unrestricted filesystem (`fs:*`) or shell (`shell:*`) capabilities to the frontend window.
  * Native security confirmation dialogs must use host-owned strings (`NativeDialogCatalog`) to prevent renderer spoofing.
* **Frontend (`apps/desktop/frontend`):**
  * React 19, Tailwind CSS, Vite.
  * Never use `dangerouslySetInnerHTML`.
  * Markdown rendering must use token-based React nodes (`MessageMarkdown.tsx`) with strict URL validation (`isSafeHttpUrl`).
  * Never store credentials or unencrypted chat history in WebView `localStorage`.

### Read-Only Security Reviews
* When conducting security reviews or audits, operate in **read-only mode**. Never modify code, commit, or alter branch state during an audit unless explicitly instructed to fix a specific finding.
* Output structured audit reports to designated report directories or artifacts.
