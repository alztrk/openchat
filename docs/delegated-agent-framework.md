# Delegated agent framework

## Purpose

This document describes the authorization and lifecycle contract for work delegated from a parent model run to a child model run. The child is a bounded worker inside the same user-owned conversation and project scope. It is not an independent user, a new trust boundary, or an authority that can grant permissions.

The current implementation is an initial framework, not yet a provider-neutral module. The design below records its verified behavior and the boundaries required when extracting provider-specific routing from the shared run orchestration.

## Security invariants

Every child run must satisfy these invariants:

1. **No authority escalation.** A child receives no tool, permission, filesystem, project, network, image-generation, or approval capability that the parent run does not already have. Its effective capabilities are the intersection of the parent run's capabilities and the child policy.
2. **Service-owned authorization.** The local service resolves the exact model route, credentials, project roots, tool definitions, and permission policy. Renderer-supplied tool names, paths, credentials, provider endpoints, or approval claims cannot expand that policy.
3. **Explicit context transfer.** A child receives its task and only the bounded additional context and project-memory snapshot passed by the parent. Parent conversation history is not copied wholesale. Context, tool output, and web content remain untrusted data.
4. **No implicit recursion.** Child definitions omit delegation and child-management tools. A child cannot create grandchildren, ask the user a question, use desktop control, change persistent memory, or control a parent goal.
5. **Same scope for approvals.** Any child tool action that requires approval goes through the existing service-owned permission broker and parent permission rules. The child cannot self-approve or persist broader rules.
6. **Bounded work.** Each child has limits for concurrency, elapsed time, provider rounds, tool calls, request and response bytes, output tokens, and emitted event sizes. Limits are enforced by the local service, not model instructions.
7. **Parent-owned visibility and control.** Child status, progress, result, and cancellation are associated with the parent conversation and durable run identifiers. Reading or cancelling a child requires the owning conversation and a run marked as a child run.
8. **Terminal outcome on failure.** Cancellation, timeout, provider errors, invalid routes, event-delivery failures, and storage failures must not be reported as success or leave a child falsely marked as running.

## Current implementation

The existing code provides these behaviors:

- `delegate_task` accepts an exact `modelKey` from `list_subagent_models`, a bounded task, optional bounded context, and a supported reasoning level. The renderer lists model options on the parent request and the RPC parser bounds them; the service then validates the selected route and credentials before starting a child. The renderer cannot supply a child endpoint or credential.
- Provider authentication material is loaded inside the local service. The child request supports the provider protocol selected for the resolved route; API keys and OAuth tokens are not sent back to the renderer.
- The child tool executor derives its tools from the parent run's tool definitions and permission rules, then removes recursive delegation, desktop control, user-question, persistent-memory, and goal-control tools. Project file access is read-only; project task execution is exposed only for approved task identifiers.
- The child receives the selected project root and a bounded project-memory snapshot, not the full parent message history. Tool results and context are treated as untrusted.
- A child is persisted as an `agent_runs` record with `runKind: subagent`, parent run/message/tool-call IDs, provider/model identity, its tool allowlist, and resource limits. Run listing, result retrieval, progress events, and cancellation are scoped to the owning conversation.
- Current service limits are two concurrent child workers per local-service process, a 180-second deadline, six provider rounds, 16 tool calls, 2,048 output tokens, a 64 KiB request limit, 32 KiB per child output, 64 KiB total child text, and 64 KiB event payloads.
- Cancellation signals and the semaphore are process-local. Durable run records survive independently; automatic resumption of an interrupted child after a service restart is not part of the current contract.

Relevant implementation paths include:

- `services/openchat-local-service/src/domain/tools/definitions.rs`
- `services/openchat-local-service/src/domain/tools/executor.rs`
- `services/openchat-local-service/src/domain/providers/chatgpt/subagents.rs`
- `services/openchat-local-service/src/transport/rpc/run_control.rs`
- `services/openchat-local-service/src/adapters/storage/user_questions/operations.rs`

Focused tests in `services/openchat-local-service/src/domain/providers/chatgpt/subagents/tests.rs` cover provider route identity, tool allowlists, request limits, cancellation, parsing, and safe handling of child output.

## Provider-neutral target boundary

Keep shared orchestration independent of any provider implementation. The target flow is:

1. The parent tool executor submits a validated delegation request to a service-owned coordinator.
2. The coordinator verifies the parent run is live and resolves the requested key from the model options attached to that parent request. Because those options originate in the renderer, the service must validate the provider, model, protocol, reasoning level, connection, credentials, and endpoint against service-owned configuration before starting the child.
3. A provider adapter resolves credentials and protocol details into an opaque child route. The coordinator never accepts arbitrary child endpoints or credentials from the renderer or model.
4. A policy builder creates an immutable child capability snapshot from the parent run. It narrows tool definitions, workspace roots, project tasks, permissions, and provider capabilities before execution.
5. The coordinator creates the durable child run record before scheduling work. The record stores identifiers, policy version, resource limits, provider/model metadata, and progress; it does not store credentials or an unbounded transcript.
6. A bounded worker executes the child request through the provider adapter and the narrowed tool executor. Provider-specific request encoding and stream parsing stay behind the adapter.
7. Progress and terminal results are written to the child record and emitted through the parent conversation event channel. The parent receives a bounded summary and the child run ID.
8. Cancellation resolves the child only through its parent conversation and durable child-run identity, signals the worker, releases resources, and records a terminal cancellation result.

The coordinator owns admission, policy snapshots, persistence, quotas, cancellation, and finalization. Provider adapters own model-route resolution, provider authentication, request encoding, stream decoding, and provider error mapping. Tool implementations continue to own operation-level validation and permission enforcement.

## Lifecycle and failure contract

Child runs use the existing durable run status vocabulary: queued/running progress, then completed, failed, cancelled, paused, or interrupted as appropriate. The coordinator must serialize or compare-and-set transitions so a late provider event cannot overwrite a terminal state. Cancellation and completion racing must produce one terminal result. A cancellation request for another conversation, a non-child run, or a terminal run must not affect any worker.

If route resolution fails, do not create a worker or send a provider request. If persistence fails before scheduling, do not start the child. If a provider call, tool, or progress write fails after creation, finalize the child as failed or interrupted and return an explicit bounded error to the parent. A timeout cancels outstanding I/O and tool work. All exit paths release semaphore permits and remove cancellation registrations.

## Validation contract for future changes

Changes to this framework must test at least:

- Exact model-key resolution and rejection of forged, stale, or unavailable routes.
- Child capabilities are a strict subset of the parent's capabilities, including tools, project roots, task IDs, provider features, and permissions.
- Recursive delegation, desktop control, memory mutation, user questions, and goal control remain unavailable to children.
- Parent-scoped read and cancel RPCs reject cross-conversation IDs and non-child runs.
- Concurrency and every request, tool, output, event, and time limit are enforced at the service boundary.
- Provider errors, malformed streams, storage failures, event failures, service shutdown, timeout, and cancellation finalize correctly and release resources.
- Secrets stay in local provider credential handling and never enter child checkpoints, events, or tool output.
- Durable child progress remains inspectable after a restart; interrupted work is surfaced as interrupted unless a future, separately specified resumable protocol is implemented.

Current automated coverage is focused on the ChatGPT subagent implementation. It does not establish a runtime guarantee for every provider adapter or packaged platform. Any provider-neutral extraction should retain the invariants above and add adapter-specific contract tests before being enabled for that route.
