# Developer scripts

Developer task logic is implemented in Node.js and exposed through npm scripts, without platform-specific shell syntax:

| Script | Purpose |
| --- | --- |
| `build.mjs` | Builds the host local-service sidecar and Tauri bundle. |
| `check.mjs` | Runs frontend and Rust formatting, build, and test checks. |
| `clean.mjs` | Removes generated build and bundle output directories. |
| `dev.mjs` | Builds the host sidecar and starts the desktop development app. |
| `maintain-target.mjs` | Reports Cargo target sizes and can clean oversized targets. |

Run these commands from `apps/desktop/frontend`:

```bash
npm run desktop:dev
npm run desktop:build
npm run check:workspace
npm run clean:workspace
npm run maintain-target -- --max-gib 20
```

Cargo target selection and sidecar naming use the Rust host target triple, so the same task commands can prepare a sidecar on Windows or Linux. Linux packaging still requires enabling a Linux Tauri bundle target, adding its build dependencies and CI job, and implementing the platform-specific sandbox support.

`npm run maintain-target -- --max-gib 20` previews target usage. Add `--apply` only when you want Cargo to clean those target directories.
