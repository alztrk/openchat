# Contributing to OpenChat

OpenChat's Windows desktop application uses a React frontend hosted by Tauri v2. A local Rust service handles provider connections, model catalogs, tool execution, and storage.

## Before you start

- Check open issues before beginning substantial work
- Keep contributions focused on the agreed scope
- Preserve local user data; never include credentials or private data

## Development setup

### Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Node.js | 22 | React frontend |
| npm | — | Frontend dependencies |
| Rust | 1.95 (pinned) | Local service & Tauri host |
| Visual Studio C++ build tools | — | Tauri compilation |
| WebView2 | — | Tauri runtime |

### Getting started

```bash
# Install frontend dependencies
npm ci --prefix apps/desktop/frontend

# Start dev server (with sidecar)
npm --prefix apps/desktop/frontend run openchat -- dev

# Run workspace checks
npm run openchat -- check
```

### CLI tasks

| Command | Description |
|---------|-------------|
| `npm run openchat -- dev` | Start dev server with sidecar |
| `npm run openchat -- build` | Build release |
| `npm run openchat -- check` | Full check (lint + types + Rust + tests) |
| `npm run openchat -- check-types` | Type check + lint (no tests) |
| `npm run openchat -- maintain-target` | Inspect Cargo target directory usage |

## Code quality

### Frontend (Biome)

Biome enforces correctness, React patterns, accessibility, and common errors. Project rules additionally reject:

- Tailwind arbitrary values and native `<select>` elements
- Unused imports/variables, undeclared names, explicit `any`
- Loose equality, duplicate object keys, unsafe type assertions
- Accumulating spread operations (quadratic pattern)
- `console`, `debugger`, array-index React keys
- Direct prototype method calls, global `isNaN`/`isFinite`

Run `npx biome check` from `apps/desktop/frontend` before submitting.

### TypeScript

- `noUncheckedIndexedAccess` — array and index-signature reads must handle absent values
- `noImplicitOverride` — inherited class members must mark intentional overrides

### Rust

- `#![deny(missing_docs)]` — all public items must have documentation
- `#![deny(clippy::undocumented_unsafe_blocks)]` — every `unsafe` block needs a `// SAFETY:` comment

## Commit guidelines

- Explain the problem and its user-visible effect
- Link related issues when they exist
- Describe the checks you ran
- Include screenshots for UI changes
- Update relevant documentation when behavior changes

## Documentation standards

### Rust

- Module-level `//!` comment stating purpose and boundary
- Document every `pub` item: responsibility, constraints, failure behavior, security assumptions
- `// SAFETY:` before every `unsafe` block — explain the pointer, handle, ownership, or lifetime invariant
- Comments in English

### TypeScript/JavaScript

- JSDoc for public exports: purpose, parameters, return values, side effects
- Avoid comments as substitutes for clear names or types
- Keep comments accurate; update or remove stale docs in the same change

## Cargo artifacts

Build artifacts go to the workspace `target` directory. The dev profile keeps line tables but omits dependency debug symbols. Use `cargo build --profile debugging` for full debug symbols.

```bash
# Check target directory size (20 GiB soft limit)
npm run openchat -- maintain-target

# Clean if exceeded
npm run openchat -- maintain-target --apply
```

## Validation

| Step | Command |
|------|---------|
| Frontend lint | `npx biome check` |
| Type check | `npm run openchat -- check-types` |
| Rust workspace check | `cargo check --workspace` |
| Rust formatting | `cargo fmt --all -- --check` |
| Rust tests | `cargo test --workspace` |

CI runs all of the above plus RustSec audits and npm audits on push/PR.

## Security contributions

When modifying security-sensitive code (storage, credentials, sandbox, network), review the [security review](./docs/security-review.md) for the current threat model and known boundaries.
