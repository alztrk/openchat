# Developer scripts

Platform-specific scripts live in a directory named for their target operating system. The current Windows scripts are under `scripts/windows/`:

| Script | Purpose |
| --- | --- |
| `build.ps1` | Builds the Windows local-service sidecar and Tauri installer. |
| `check.ps1` | Runs frontend and Rust formatting, build, and test checks. |
| `clean.ps1` | Removes generated build and bundle output directories. |
| `dev.ps1` | Builds the Windows sidecar and starts the desktop development app. |
| `maintain-target.ps1` | Reports Cargo target sizes and optionally cleans oversized targets. |

Run these commands from the repository root, for example:

```powershell
.\scripts\windows\dev.ps1
.\scripts\windows\check.ps1
```

When Linux support is implemented, add Linux-specific entry points under `scripts/linux/`. Keep operating-system paths, executable names, packaging steps, and cleanup behavior in the matching platform directory. Continue using Cargo, npm, and Tauri directly for cross-platform operations; introduce shared orchestration only if the platform scripts develop duplicated behavior that needs one source of truth.

Tauri's `externalBin` requires a sidecar binary named with its target triple. Each platform build must place the service in `apps/desktop/tauri/binaries/` using the matching target-specific name before packaging.
