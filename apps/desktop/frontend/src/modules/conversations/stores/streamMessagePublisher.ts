import type { ChatMessage } from '@/modules/conversations/data/chatStorage';

const streamUiUpdateIntervalMs = 50;

/**
 * Coalesces append-only text deltas before publishing them to the UI.
 *
 * The current message remains owned by the stream handler and is never altered here. Deferred
 * updates publish the newest complete snapshot at most once per interval. State events publish
 * immediately and supersede any queued snapshot, so the UI does not render stale message metadata.
 * Call `flush` before completing or stopping a stream, and `dispose` when its lifecycle ends.
 *
 * @param publish Delivers a complete stream-message snapshot to the active UI store.
 */
export function createStreamMessagePublisher(publish: (message: ChatMessage) => void) {
  let pendingMessage: ChatMessage | null = null;
  let timer: number | null = null;

  const clearTimer = () => {
    if (timer === null) return;
    globalThis.clearTimeout(timer);
    timer = null;
  };

  /** Publishes the latest queued text snapshot before a stream reaches a terminal state. */
  const flush = () => {
    clearTimer();
    const message = pendingMessage;
    pendingMessage = null;
    if (message) publish(message);
  };

  /** Drops a queued snapshot when a terminal UI update replaces it. */
  const discard = () => {
    clearTimer();
    pendingMessage = null;
  };

  return {
    /**
     * Publishes state changes immediately and coalesces text-only changes on a 50 ms cadence.
     *
     * @param message Latest full assistant-message snapshot.
     * @param defer Whether the update is an append-only text delta.
     */
    enqueue(message: ChatMessage, defer: boolean) {
      if (!defer) {
        discard();
        publish(message);
        return;
      }

      pendingMessage = message;
      if (timer === null) timer = globalThis.setTimeout(flush, streamUiUpdateIntervalMs);
    },
    flush,
    discard,
    /** Releases the scheduled callback and any queued message when the stream ends. */
    dispose: discard,
  };
}
