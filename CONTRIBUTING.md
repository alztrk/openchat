# Contributing to OpenChat

OpenChat's Windows desktop application uses a React frontend hosted by Tauri. A
local Rust service handles provider requests, model catalogs, and tool
execution. See the [README](README.md) for the current architecture and
supported workflows.

## Before starting work

- Check open issues before beginning a substantial feature or technical change.
- Keep contributions focused on the agreed issue and current project scope.
- Preserve local user data and avoid including credentials or private data.

## Pull requests

- Explain the problem and its user-visible effect.
- Link the related issue when one exists.
- Describe the checks you ran. If a check could not run, say why.
- Include screenshots for interface changes when they help reviewers.
- Update the smallest relevant documentation when behavior or setup changes.

## Windows development requirements

- Node.js 22 and npm for the React frontend.
- Rust and Cargo for the Tauri host and local service.
- Visual Studio C++ build tools and WebView2 for Tauri's Windows target.

Install frontend dependencies from the repository root with
`npm ci --prefix frontend`. Start the desktop development app with
`npm run tauri:dev --prefix frontend`. Build the frontend with
`npm run build --prefix frontend`.

## Validation and packaging

The Windows CI workflow formats and checks both Rust crates, runs the local
service tests, builds the React frontend, compiles the Tauri application, and
uploads the NSIS installer as a short-lived artifact. It does not publish a
GitHub release or perform live provider-account checks.

Use the repository's pinned lockfiles when building. The service sidecar must
be built for the Tauri target and placed in `src-tauri/binaries` using the
target-triple filename configured by Tauri before packaging. The CI workflow
shows the supported Windows build sequence.

Current automated coverage includes local Rust service tests and frontend
type-check/bundle validation. There is no Flutter test or golden-image suite;
review React changes in the running Tauri app and report any Windows checks
that were not available.
