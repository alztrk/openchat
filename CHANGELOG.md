# Changelog

Completed work is summarized from repository history and grouped by the relevant Git commit date. These dates record repository changes; they do not indicate release dates.

## Unreleased

### Changed

- Moved local speech model installation into Voice settings and added microphone selection plus supported output routing for notification sounds. Voice capture now releases microphone resources on stop, app hide, and recorder timeout; it supports pause, audio review before local transcription, automatic speech-language detection, and draft insertion only after user confirmation.
- Try official provider website favicons on Connections credential cards, retaining bundled icons when remote favicons are unavailable.
- Removed the recent unsuccessful generations section from the Design image library.
- Grouped tool activity above assistant responses with expandable per-tool details and elapsed response time.

### Added

- Added separate preview releases and stable tagged releases, with package SHA-256 verification and published checksums.
- Added a validated SPDX 2.3 source dependency SBOM to tagged desktop releases.
- Added side-by-side comparisons for two or three selected models, independent saved conversations, response streams, follow-up messages, and comparison metrics.
- Added an editable Design workspace with structured documents, reusable components and styles, prototype interactions and transitions, revision comparison, alignment guides, portable archives, and SVG, PNG, PDF, and HTML export.
- Added prototype preview controls for destination frames, triggers, presentations, and transitions.
- Added Design text typography controls, reusable text and color styles, component variants and instances, plus canvas alignment and distribution actions.
- Added reusable parameterized workflow templates with prompt variable expansion, durable persistence, validation, and JSON import/export.
- Added optional before-start and after-response prompts to reusable workflows; successful responses place the follow-up in the empty composer for review without sending it.
- Added grouping for selected Design layers, keeping their positions while wrapping them in a frame.
- Added Amazon Bedrock Converse and ConverseStream protocol support, AWS SigV4 authentication, regional runtime configuration, and EventStream binary decoding.
- Added user-triggered local text-to-speech playback for completed assistant responses.
- Added Microsoft AI Foundry and Azure OpenAI model routing, deployment identifier configuration, API versioning, and credential headers.

### Fixed

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
