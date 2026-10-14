# Temporary conversations

**Status: the in-memory session model, attachment flow, save/discard flow, and
temporary service paths are implemented.** The temporary session model validates
IDs, transcript bounds, and in-memory attachment bytes, and clears them on
discard. The `chat.temporary.send` service method accepts bounded user/assistant
history and validated portable attachments for compatible provider routes and
ChatGPT OAuth. It
streams answers without conversation, run, tool, or usage persistence. The UI
explains provider-side retention before the first send and offers save, discard,
or continue when leaving a populated session. Tools remain unavailable.
Temporary content still goes to the selected provider. This
document defines the local-retention boundary.

## User-visible behavior

- A user can start a temporary conversation without changing the default for
  ordinary chats.
- Message text and attachment bytes remain available only in the active session.
  Attachments are held in memory, revalidated by the service, and are not copied
  to the conversation attachment directory. Images are offered only when the
  selected model supports them. Existing attachment types and per-file limits
  apply, with a 14 MiB attachment total, a 768 KiB text transcript cap, and a
  16 MiB total in-memory transcript cap.
  Content is discarded when the user discards the session or closes OpenChat.
- Before leaving a temporary conversation that contains content, offer save,
  discard, and continue. Saving converts sent messages and their attachments
  into an ordinary durable conversation in one database transaction, preserving
  the selected provider, model, and provider connection or workspace route.
  Unsent attachments are discarded when the user leaves.
- A send takes a synchronous preparation lock before reading credentials or
  showing provider-retention confirmation. A second send cannot start while
  preparation is pending, and the session cannot be left or discarded until
  preparation finishes or the active provider request is stopped.
- A temporary conversation is never listed in chat history, project/workspace
  lists, history search, archive indexes, saved searches, conversation exports,
  profile archives, durable run history, or usage history.
- Temporary sends do not update the provider connection's durable last-request
  outcome or timestamp. That metadata is still recorded for regular chats.
- Explain before sending that message text, attached files, transcript, and
  chosen model route are sent to the provider. Provider-side retention is
  controlled by that provider.

## Retention and service boundary

The temporary session must be represented separately from a durable
`conversationId`. A temporary ID must never be accepted by a persistence API as
though it were a durable conversation ID.

Flutter generates temporary IDs as `tmp_` followed by 128 bits of secure
randomness. The durable Flutter repository rejects that reserved prefix when
creating a conversation or saving a message. The normal Rust `chat.send` RPC
also rejects it before creating a durable run. These guards are defense in
depth; the temporary request must use a separate RPC method and ID field.

The Flutter session owns the transcript and attachment bytes in memory. The
service receives a bounded, validated transcript and portable attachment bytes
through a dedicated request-scoped protocol. It validates file metadata,
base64, declared size, image signatures, and UTF-8 text. Text attachments are
framed as user-provided data, and request instructions explicitly say not to
follow instructions found inside files. The service must not look up or create
a conversation row for the temporary session.

The provider request path must not write messages, run checkpoints, tool
activities, question state, compaction summaries, prompt-usage rows, automatic
titles, archive-memory indexes, or request manifests to `AppStorage`. It must
not copy attachments to the conversation attachment directory. The response
stream and cancellation remain scoped to the active in-memory session.

Provider authentication and catalog reads may continue using the existing
secure credential stores and provider model catalogs. Those stores contain
provider configuration, not temporary conversation content. Do not enable
project instructions, Skills, project index excerpts, MCP, project tasks, or
durable goals/questions implicitly for a temporary session.

## Tool behavior decision

Local and provider-hosted tools have different consequences from ordinary chat:
they can read or change files, access the network, or pause for user input.
Before enabling tools in temporary conversations, each tool path must support
in-memory run state and preserve the existing permission and cancellation
boundaries. Until that exists, temporary requests must not advertise tools or
accept tool calls. The UI must explain this limitation rather than silently
falling back to a durable run.

## Failure and exit behavior

- Provider errors and cancellation keep the current session transcript in
  memory and create no durable failed message or run. The user can continue
  with another message or save the transcript.
- A provider retry must reuse the current session transcript and must not send
  the same user message twice.
- If the user chooses save, the save operation writes the complete transcript
  and sent attachment bytes into a durable conversation or reports failure
  while leaving the temporary session intact.
- Closing or discarding clears session messages, attachment bytes, pending
  approvals, and cancellation state. It does not attempt to erase data already
  sent to a provider or copied by the operating system.
- If the local service restarts, an active temporary request is interrupted;
  neither transcript nor request state is reconstructed from disk.

## Verification criteria

For a temporary chat, exercise successful completion, provider failure,
cancellation, text and image attachments, unsupported-image models, save,
discard, unsent-attachment removal, and app/service exit. Inspect
the durable database tables, archive/search exports, usage records, and
attachment directories before and after each path. The transcript, attachments,
tool activity, and run state must not appear in any durable surface unless the
user explicitly saves the session. Verify compatible API-key routes and
ChatGPT OAuth, including account and workspace routing, image capability checks,
and provider-side retention disclosure. Also verify that repeated send
activation during credential reads or the provider-retention dialog starts one
request, and that leaving during that preparation does not discard the session.

Current persistence entry points include:

- Flutter message and attachment writes in
  `lib/features/chat/presentation/chat_screen.dart` and
  `lib/features/chat/data/chat_repository.dart`.
- Rust route, message-history, run/checkpoint, and response-stream writes in
  `native/openchat-service/src/rpc.rs`,
  `native/openchat-service/src/chatgpt.rs`, and
  `native/openchat-service/src/openai_compatible/conversation.rs`.
- Attachment files in
  `lib/features/chat/data/chat_attachment_store.dart` and
  `native/openchat-service/src/attachments.rs`.
