# Developer tasks

All developer tasks use the single cross-platform CLI in `scripts/openchat-cli.mjs`. Run
it through the frontend npm script from `apps/desktop/frontend`:

```bash
npm run openchat -- dev
npm run openchat -- build
npm run openchat -- check
npm run openchat -- clean
npm run openchat -- maintain-target --max-gib 20
```

`dev` builds the debug local-service sidecar and starts Tauri. `build` builds the
locked release sidecar and Tauri bundle. `check` runs frontend lint and build plus
Rust formatting, checking, and tests. `clean` removes generated build and bundle
output directories.

`maintain-target` reports Cargo target sizes and previews cleanup by default. Add
`--apply` only when you want to clean those Cargo target directories.

Cargo target selection and sidecar naming use the Rust host target triple, so the
same task commands can prepare a sidecar on Windows or Linux. Linux packaging still
requires enabling a Linux Tauri bundle target, adding its build dependencies and CI
job, and implementing the platform-specific sandbox support.
