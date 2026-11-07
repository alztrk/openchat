# Developer tasks

All developer tasks use the single cross-platform CLI in `scripts/run.mjs`. The
normal entry point is the frontend npm script from `apps/desktop/frontend`:

```bash
npm run openchat -- dev
npm run openchat -- dev-test-profile
npm run openchat -- dev-test-profile --cleanup-profile
npm run openchat -- dev-test-profile --profile-root "<absolute OS temporary path>"
npm run openchat -- startup-benchmark
npm run openchat -- startup-benchmark --production-frontend
npm run openchat -- perf-benchmark
npm run openchat -- build
npm run openchat -- check
npm run openchat -- clean
npm run openchat -- clean --apply
npm run openchat -- maintain-target --max-gib 20
```

The npm workspace is rooted at the repository root. Its `package-lock.json` is
the single dependency lockfile; run `npm ci` from the repository root. The
startup dependency check validates the full frontend workspace and, if needed,
runs `npm ci` from that root.

`dev` builds the debug local-service sidecar and starts Tauri. The development
task launches the Tauri CLI directly through Node, and Tauri launches Vite's JS
entry point directly. On Windows, run interruptible interactive tasks directly
through Node when you need Ctrl+C cleanup to stay under the CLI's control:

```powershell
node ../../../scripts/run.mjs dev-test-profile --cleanup-profile
```

The direct entry point resolves the npm CLI bundled beside `node.exe` for the
workspace dependency check. If npm is installed separately, use the normal npm
entry point above. Avoiding the npm script shell keeps it from intercepting
Ctrl+C; the Tauri and Vite processes also avoid nested `.cmd` shims.
`build` builds the locked release sidecar and Tauri bundle. `check` runs Biome, frontend unit tests,
locale and FSD import checks, the TypeScript/Vite asset build, Rust formatting,
locked workspace checking, and locked Rust tests. It reports all task failures
after attempting every check. `clean` previews generated build and bundle output
directories by default. Pass `--apply` to remove the listed directories.

`dev-test-profile` builds both the Tauri host and local-service sidecar with the
debug-only `desktop-profile-test-support` feature, then starts the app with an
isolated profile directly under the OS temporary directory. It prints the profile
path and keeps that profile after the app closes so it can be reused for a second
launch. The host accepts only `openchat-desktop-test-<UUID>` directories directly
under the canonical temporary directory. Remove that exact profile directory when
the test is complete.

For interactive checks that should leave no generated profile behind, pass
`--cleanup-profile` without `--profile-root`. The CLI removes only the
UUID-named profile it created directly under the OS temporary directory after
the app exits, including when the build or launch fails. On Ctrl+C on Windows,
it gives Tauri up to 10 seconds to stop. If the CLI remains open, the runner
requests forced tree termination and waits up to 3 seconds for its exit. It then
allows profile-using Windows processes 2 seconds to close. If any still
reference the unique profile path, the runner terminates only the matching
process trees and checks for up to 30 seconds that they have exited. This check
also runs if the Tauri CLI exits first, because its app processes can remain
open after the CLI stops. Cleanup retries transient Windows profile-lock errors
for up to about 16 seconds. If shutdown cannot be confirmed or profile removal
still fails, the command fails and keeps the remaining profile data. The
retained path is included in the error. The option cannot be combined with a
caller-supplied profile path.

The test-profile command treats an app exit as a normal window close only when
the debug-only host records a user-requested Tauri exit after the local service
stops successfully. That run-specific marker is required for either exit code
`0` or the observed Windows wrapper code `4294967295`; crashes, startup errors,
service shutdown failures, missing markers, and other exit codes remain errors.

`startup-benchmark` builds a debug local-service sidecar with isolated-profile
support and measures process-to-React-ready time while the Tauri app uses Vite's
development server. Add `--production-frontend` to build and measure the Vite
production bundle in the debug Tauri host and sidecar without creating an
installer. Both modes require the normal OpenChat application shell and verify
the temporary active database has no plaintext SQLite header before reporting a
result. They remove the UUID-named temporary profile after the run, including a
confirmed Ctrl+C shutdown. If child-tree shutdown cannot be confirmed, the task
preserves and reports the profile path. Both are debug-host measurements, not
release-package results. The three-minute watchdog includes compilation and
launch; reported startup durations begin in the app process and exclude
compilation.

`perf-benchmark` runs curated ignored Rust benchmarks for filesystem tools,
request-context token estimation, archived-memory search, RPC stream payloads,
and reusable HTTP client construction. It invokes each named local benchmark
separately with the locked `openchat-local-service` package. It does not run the
ignored live web-search benchmark or accept arbitrary Cargo arguments. The
benchmarks use synthetic or in-memory data and clean their temporary fixtures
when they complete.

`maintain-target` reports Cargo target sizes and previews cleanup by default. Add
`--apply` only when you want to clean those Cargo target directories.

Build, check, dev, and cleanup tasks share a workspace artifact lock. A second
task that could change or remove Cargo/Tauri outputs exits with the active task
name and process ID. `clean` and `maintain-target --apply` also check for an
external Cargo or rustc process before deleting artifacts.

Cargo target selection and sidecar naming use the Rust host target triple, so the
same task commands can prepare a sidecar on Windows or Linux. On Linux, `build`
selects the Linux Tauri configuration and produces Debian and AppImage bundles.
The CI Linux job installs the native build dependencies, runs the validation
harness, builds both bundles, and uploads them as workflow artifacts.
