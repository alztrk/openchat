# Contributing to OpenChat

OpenChat's Windows desktop application uses a React frontend hosted by Tauri. A
local Rust service handles provider requests, model catalogs, and tool
execution. See the [README](README.md) for the current architecture and
supported workflows.

## Before starting work

- Check open issues before beginning a substantial feature or technical change.
- Keep contributions focused on the agreed issue and current project scope.
- Preserve local user data and avoid including credentials or private data.

## Source documentation

- Give each source module a short file-level overview that states its purpose and boundary. Use Rust `//!` comments, TypeScript JSDoc, and Dart `///` if Dart source is reintroduced.
- Document every class, struct, enum, trait, interface, component, method, and function with the language's documentation comment syntax. State its responsibility and important design constraints instead of repeating its name or implementation line by line.
- Describe parameters, return values, side effects, failure behavior, security assumptions, and usage constraints when they affect callers. Omit details that do not apply rather than adding empty template sections.
- Keep comments accurate as code changes. Update or remove stale documentation in the same change, and do not use comments as a substitute for clear names or types.
- Write source comments in English. Put `// SAFETY:` immediately before every Rust unsafe block and explain the pointer, handle, ownership, or lifetime invariant that makes it valid.

## Pull requests

- Explain the problem and its user-visible effect.
- Link the related issue when one exists.
- Describe the checks you ran. If a check could not run, say why.
- Include screenshots for interface changes when they help reviewers.
- Update the smallest relevant documentation when behavior or setup changes.

## Windows development requirements

- Node.js 22 and npm for the React frontend.
- Rust 1.95 and Cargo for the Tauri host and local service, pinned by `rust-toolchain.toml` and matching the Windows CI toolchain.
- Visual Studio C++ build tools and WebView2 for Tauri's Windows target.

Install frontend dependencies from the repository root with
`npm ci --prefix apps/desktop/frontend`. Start the desktop development app with
`npm --prefix apps/desktop/frontend run openchat -- dev`. Build the frontend with
`npm run build --prefix apps/desktop/frontend`. Run the required UI design lint
from the frontend directory with `npx biome check`.

Biome's recommended preset checks correctness, React hook dependencies,
accessibility, stable list keys, and common JavaScript and TypeScript errors.
Project rules also reject Tailwind arbitrary values, native `<select>`, unused
imports and variables, undeclared names, explicit `any`, loose equality,
duplicate object keys, unsafe TypeScript type assertions, `console`, and
`debugger`. They also reject accumulating spread operations that can make an
array or object reduction quadratic. Keep new frontend code clear of these
patterns, dangerous HTML sinks, array-index React keys, direct prototype-method
calls, and global `isNaN` or `isFinite` checks. Narrow values with runtime checks
or explicit types instead of type assertions. TypeScript's `noUncheckedIndexedAccess`
also requires explicit handling of array and index-signature reads that may be
absent, while `noImplicitOverride` requires inherited class members to mark
intentional overrides explicitly. Suppress a rule only when the behavior is
intentional and the source explains why.

## Cargo build artifacts

Cargo writes generated build artifacts to the workspace `target` directory.
The development profile keeps line tables for the workspace and omits debug
symbols from dependencies. Use `cargo build --profile debugging` when full
debug symbols are needed.

Use this command to report the size of Cargo-marked target directories:

```bash
npm --prefix apps/desktop/frontend run openchat -- maintain-target --max-gib 20
```

It previews by default. Add `--apply` to clean those directories when their
combined size exceeds 20 GiB, after checking that no `cargo` or `rustc` process
is running and previewing Cargo's cleanup. Any scheduled task is configured per
user and is not installed by cloning the repository; update its command if it
still points to an older script.

## Validation and packaging

The Windows CI workflow lints the frontend's required design rules, formats
and checks the unified Rust service and Tauri host, runs their Rust tests, audits
the Rust workspace lockfile with RustSec, audits shipped npm dependencies, builds the React frontend,
compiles the Tauri application, and uploads the NSIS installer as a short-lived artifact.
It does not publish a GitHub release or perform live provider-account checks.
Both Rust packages deny undocumented unsafe blocks. Add a focused `// SAFETY:`
comment immediately before each unsafe block and state the pointer, handle, or
ownership invariant that makes the operation valid. The CI Clippy checks enforce
this requirement.

Run the service and sandbox tests with:

```powershell
cargo test -p openchat-local-service --locked -- --test-threads=1
```

Use the repository's pinned lockfile when building. The service sidecar must
be built for the Tauri target and placed in `apps/desktop/tauri/binaries` using the
target-triple filename configured by Tauri before packaging. The CI workflow
shows the supported Windows build sequence.

Current automated coverage includes local Rust service tests, Windows
AppContainer boundary tests, and frontend type-check/bundle validation. There
is no Flutter test or golden-image suite; review React changes in the running
Tauri app and report any Windows checks that were not available.
