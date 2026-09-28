# Contributing to OpenChat

Thanks for your interest in contributing. The first release is a Windows AI chat app, beginning with ChatGPT, model selection, and conversation history. File editing is outside the first-release scope; see the [README](README.md) for the current project direction and [ChatGPT implementation plan](docs/chatgpt-integration-plan.md) for the agreed integration behavior and delivery order.

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

- `flutter analyze`
- `cargo fmt --manifest-path native/zihora-service/Cargo.toml -- --check`
- `flutter build windows --release`
- `git diff --check`
