# Changelog

Completed work is summarized from repository history and grouped by the relevant Git commit date. These dates record repository changes; they do not indicate release dates.

## Unreleased

### Changed

- Show bundled engine-specific marks for llama.cpp, SGLang, and vLLM in provider and local engine surfaces.
- Moved local speech model installation into Voice settings and added microphone selection plus supported output routing for notification sounds. Voice capture now releases microphone resources on stop, app hide, and recorder timeout; it supports pause, audio review before local transcription, automatic speech-language detection, and draft insertion only after user confirmation.
- Refresh missing or stale models.dev pricing when loading Usage Statistics so matching ChatGPT OAuth models can show estimated request costs.
- Moved local speech model installation and its progress controls from the chat composer to a dedicated Voice settings section.
- Added microphone selection and supported output routing for notification sounds in Audio settings; voice capture now releases the microphone on stop, app hide, and recorder timeout, keeps the composer hidden during capture, supports pause and audio review before local Whisper transcription, automatically detects the spoken language, appends text only after user confirmation, and disables the microphone until the local speech model is available.
- Try official provider website favicons on Connections credential cards, retaining bundled icons when remote favicons are unavailable.
- Removed the recent unsuccessful generations section from the Design image library.
- Grouped tool activity above assistant responses with expandable per-tool details and elapsed response time.

### Added

- Added an opt-in vLLM local engine for native Linux and Windows through an existing WSL 2 distribution, with pinned Transformers snapshots, configurable launch settings, authenticated local inference, and model-preserving uninstall.
- Added managed ExLlama and SGLang local engines, with Linux support, native Windows ExLlama, WSL 2 SGLang, configurable runtime settings, update support, and Transformers model downloads from Library. The SGLang runtime includes its pinned CUDA compiler for JIT kernel builds.
- Added per-model context length overrides for local inference engines, and made installed local models available in chat so sending a message starts a stopped runtime and the selected model.
- Add native macOS DMG packaging for Intel and Apple Silicon, and include checksummed installers in desktop releases.
- Added separate preview releases and stable tagged releases, with package SHA-256 verification and published checksums.
- Added a validated SPDX 2.3 source dependency SBOM to tagged desktop releases.
- Preserve PDF attachments as encrypted original documents and send them as native inputs on vision-capable OpenAI Responses routes, with local text extraction for other providers.
- Added in-chat model comparisons: selected models start together from one saved prompt, with responses available in tabs in the same conversation.
- Added a dedicated Design workspace for editable, reusable interface designs with structured vector paths, components, variants, click and hover prototype triggers, selectable transitions, editable motion keyframes, canvas comments, cross-document text styles, revision comparison, object snapping with alignment guides, portable document archives, and SVG, PNG, PDF, and HTML handoff; Image Studio remains a separate sidebar page.
- Added prototype preview controls for destination frames, triggers, presentations, and transitions.
- Added text typography controls and reusable text and color styles, component variant and instance controls, and canvas alignment and distribution actions in Design.
- Added user-triggered local text-to-speech playback for completed assistant responses.
- Added reusable parameterized workflow templates with prompt variable expansion, durable persistence, validation, and JSON import/export.
- Added optional before-start and after-response prompts to reusable workflows; successful responses place the follow-up in the empty composer for review without sending it.
- Added grouping for selected Design layers, keeping their positions while wrapping them in a frame.
- Added Amazon Bedrock Converse and ConverseStream protocol support, AWS SigV4 authentication, regional runtime configuration, and EventStream binary decoding.
- Added Microsoft AI Foundry and Azure OpenAI model routing, deployment identifier configuration, API versioning, and credential headers.

### Fixed

- Correctly read SGLang runtime markers on WSL so managed runtimes can be uninstalled and reinstalled while keeping downloaded models.
- Abort model comparisons and remove empty comparison chats when a pane cannot load its saved history.
- Limit WebView2 browser-process recovery to one recreation per app session, even if the replacement renderer reports ready before crashing again, then show a localized native error.

## 2026-11-11

### Added

- Added message branches and in-conversation navigation between response versions.
- Added manual local inference settings for GPU layers, thread count, and context size.
- Expanded provider usage tracking for reasoning tokens, cache usage, timing data, and streamed usage payloads; reviewed live ChatGPT OAuth model, usage, credit-reset, and response endpoints.

### Changed

- Batched streamed text events across the Rust and frontend boundary and added a read-only SQLite connection pool for WAL-backed reads.
- Moved context estimation and text analysis work to Rust and added an in-memory conversation hot cache.
- Reduced streaming-related frontend work, optimized context usage updates and sidebar rendering, and replaced the diff dependency with a local parser and split/unified views.
- Added SQLite indexes for conversation and message ordering and removed an allocation from SSE line parsing.

## 2026-11-10

### Added

- Added a dedicated project management section to Settings.
- Guarded ResizeObserver-driven layout updates and filtered benign resize-loop notifications.

## 2026-11-07

### Added

- Required Linux desktop packages in the desktop release workflow alongside Windows validation and packaging.
- Completed the Windows notification integration, including the application identity, Start Menu shortcut, and WinRT activation flow.

## 2026-11-05

### Added

- Added a saved-attachment library entry point in the composer and fixed its service query.
- Added accessible model-selection guidance for empty chats, model-download progress labels, search controls, focus handling, and primary-navigation naming.
- Added advanced history search filters and persisted search presets, with user-visible handling for persistence failures.
- Added recovery flows for design-canvas data and prompt-template storage.
- Added progress and cancellation for attachment indexing, conversation archive operations, and security-scan deletion.

### Changed

- Kept sidebar folder and conversation placement stable when persistence or move operations fail; preserved invalid folder preference data for recovery.
- Restricted the Vite development CORS origin and protected managed llama.cpp inference requests with a per-process bearer key.
- Improved message and attachment validation, cleanup, archive recovery, history pagination, and stale-conversation error handling.
- Bounded worktree listing and creation, serialized concurrent preference updates, and made notification quiet-hours edits validate before saving.
- Removed nested main landmarks from feature surfaces and removed the unused AppManager copy after checking its consumers.

## 2026-11-04

### Added

- Consolidated local and CI validation through the centralized check harness and recorded the completed static review and isolated first-use walkthrough.
- Added durable attachment staging and recovery, bounded sidecar pipe writes, bounded window-state events, and renderer recovery behavior.
- Added typed safeguards for attachment persistence, message size limits, and conversation archive operations.

### Changed

- Improved desktop startup, renderer readiness, window geometry restoration, and shutdown behavior.

## 2026-11-03

### Changed

- Completed frontend feature/app layer-boundary cleanup and reduced cross-feature import warnings to zero.
- Stabilized Rust test isolation and compaction-related coverage; removed obsolete summary and import warnings.

## 2026-11-02

### Fixed

- Fixed Rust compilation issues in conversation history and provider compaction, added the expected file-edit occurrence check, and removed an invalid test-runner flag.
- Recorded passing TypeScript, frontend test, locale, Biome, and import-layer checks in repository history.

## 2026-11-01

### Changed

- Consolidated the Rust sidecar domain, providers, tools, storage, and sandbox modules into the local-service workspace; removed abandoned crate drafts and duplicate workspace configuration.
- Aligned the frontend test suite with the FSD paths and completed the CI gates and repository architecture guidance.

## 2026-10-31

### Changed

- Completed the initial frontend architecture migration: added TypeScript and Vite aliases, established the FSD directory structure and import checks, moved shared code and feature modules, and introduced shared conversation, model, and project entities.
- Moved application routing and layout responsibilities into focused app components.
