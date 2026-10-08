# Contributing to OpenChat

Thanks for your interest in contributing. The first release focuses on Windows AI chat, model selection, conversation history, and local workspace tools, including file changes with review and revert support. See the [README](README.md) for the current project direction and [ChatGPT implementation plan](docs/chatgpt-integration-plan.md) for the agreed integration behavior and delivery order.

## Before starting work

- Check the open issues before beginning a substantial feature or technical change.
- For work that does not already have an issue, open one first to describe the problem and proposed change.
- Keep contributions focused on the agreed issue and current project scope.

## Pull requests

- Explain the problem the change solves and its user-visible effect.
- Link the related issue when one exists.
- Describe the checks you ran. If you could not run a check, say so clearly.
- Include screenshots for user interface changes when they help reviewers understand the result.
- Update project documentation when a change affects user-facing behavior or setup instructions.
- Do not include API keys, tokens, credentials, private data, or other secrets.

## Build requirements

Windows development requires Flutter, Rust with Cargo, and the Visual Studio C++ build tools used by Flutter's Windows runner. The Windows build compiles the Rust local service and packages it beside the application executable.

## Checks

- GitHub Actions runs the checks below on Windows without provider credentials or live provider requests.
- Workflow actions are pinned to commit SHAs; Dependabot checks GitHub Actions, Cargo, and Pub updates weekly.
- `flutter pub get --enforce-lockfile`
- `flutter gen-l10n`
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test`
- `cargo fmt --manifest-path native/openchat-service/Cargo.toml -- --check`
- `cargo fmt --manifest-path native/openchat-launcher/Cargo.toml -- --check`
- `cargo check --all-targets --locked --manifest-path native/openchat-service/Cargo.toml`
- `cargo test --locked --manifest-path native/openchat-service/Cargo.toml`
- `tools/build_windows_portable.ps1`
- `cargo test --locked --manifest-path native/openchat-launcher/Cargo.toml --target-dir build/launcher-test-target`
- `git diff --check`

The Windows workflow uploads the portable executable as a short-lived build artifact. It does not publish a GitHub release. Live provider-account checks require configured provider credentials and are run separately from credential-free CI.

Visual regression checks live in `test/fidelity_screenshot_test.dart`. They render the existing Flutter components in both themes with isolated test data, check narrow windows and enlarged text, and exercise the history drawer and virtual-keyboard layout. The files in `test/goldens` are local rendering baselines, not evidence of a live provider connection.

For an intentional visual change, run `flutter test --no-pub --update-goldens test/fidelity_screenshot_test.dart`, inspect the changed images, then run `flutter test --no-pub` without updating baselines. Check the composer with all applicable selectors visible: fitting the default empty state alone does not prove the controls fit a narrow window.

Open menus and provider forms at 200% text scale, visit every settings section, and check both narrow and short windows. Scope repeated navigation labels to their page so a test cannot accidentally open the global model catalog instead of model settings. Keep empty and unavailable states reachable without a connected service. Verify focus, Escape dismissal, disabled actions, and touch targets before accepting new visual baselines.
