# Developer tasks

All developer tasks use the single cross-platform CLI in `scripts/openchat-cli.mjs`. Run
it through the frontend npm script from `apps/desktop/frontend`:

```bash
npm run openchat -- dev
npm run openchat -- dev-test-profile
npm run openchat -- dev-test-profile --cleanup-profile
npm run openchat -- dev-test-profile --profile-root "<absolute OS temporary path>"
npm run openchat -- startup-benchmark
npm run openchat -- build
npm run openchat -- check
npm run openchat -- clean
npm run openchat -- maintain-target --max-gib 20
```

`dev` builds the debug local-service sidecar and starts Tauri. `build` builds the
locked release sidecar and Tauri bundle. `check` runs frontend lint and build plus
Rust formatting, checking, and tests. `clean` removes generated build and bundle
output directories.

`dev-test-profile` builds both the Tauri host and local-service sidecar with the
debug-only `desktop-profile-test-support` feature, then starts the app with an
isolated profile directly under the OS temporary directory. It prints the profile
path and keeps that profile after the app closes so it can be reused for a second
launch. The host accepts only `openchat-desktop-test-<UUID>` directories directly
under the canonical temporary directory. Remove that exact profile directory when
the test is complete.

For interactive checks that should leave no generated profile behind, pass
`--cleanup-profile` without `--profile-root`. The CLI then removes only the
UUID-named profile it created directly under the OS temporary directory after
the app exits, including when the build or launch fails. When interrupted with
Ctrl+C, it waits for the child process to exit before removing that profile.
The option cannot be combined with a caller-supplied profile path.

`startup-benchmark` builds a debug local-service sidecar with isolated-profile
support, starts a Tauri debug build with startup timing enabled, and measures the
process-to-React-ready interval. It requires the normal OpenChat application shell
and verifies the temporary active database has no plaintext SQLite header before
reporting a result. The command removes its UUID-named temporary profile after the
run, including when interrupted with Ctrl+C. This is a debug startup measurement,
not a release-package result.

`maintain-target` reports Cargo target sizes and previews cleanup by default. Add
`--apply` only when you want to clean those Cargo target directories.

Cargo target selection and sidecar naming use the Rust host target triple, so the
same task commands can prepare a sidecar on Windows or Linux. Linux packaging still
requires enabling a Linux Tauri bundle target, adding its build dependencies and CI
job, and implementing the platform-specific sandbox support.
