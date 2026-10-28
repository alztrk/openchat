export interface ConnectionRemovalWaitOptions {
  intervalMs: number;
  failedOperationReconciliationMs: number;
  onCheckFailure(error: unknown): void;
}

export function isRetryableConnectionRemovalFailure(error: unknown): boolean {
  return (
    error instanceof Error && error.name === 'LocalServiceError' && 'retryable' in error && error.retryable === true
  );
}

export async function waitForConnectionRemoval(
  connectionId: string,
  operationResult: Promise<Record<string, unknown>>,
  connectionExists: (id: string) => Promise<boolean>,
  options: ConnectionRemovalWaitOptions,
): Promise<void> {
  type OperationOutcome = { kind: 'completed'; response: Record<string, unknown> } | { kind: 'failed'; cause: unknown };
  const outcomePromise: Promise<OperationOutcome> = operationResult.then(
    (response) => ({ kind: 'completed', response }),
    (cause: unknown) => ({ kind: 'failed', cause }),
  );
  let checkFailureReported = false;

  while (true) {
    const outcome = await Promise.race([outcomePromise, wait(options.intervalMs).then(() => null)]);
    if (outcome?.kind === 'completed') {
      if (outcome.response.removed === true) return;
      throw new Error('The local service did not confirm ChatGPT connection removal.');
    }
    if (outcome?.kind === 'failed') {
      const removed = await reconcileFailedRemoval(
        connectionId,
        connectionExists,
        options.failedOperationReconciliationMs,
        options.intervalMs,
        (error) => {
          if (checkFailureReported) return;
          options.onCheckFailure(error);
          checkFailureReported = true;
        },
      );
      if (removed) return;
      throw outcome.cause;
    }

    try {
      if (!(await connectionExists(connectionId))) return;
    } catch (error) {
      if (!checkFailureReported) {
        options.onCheckFailure(error);
        checkFailureReported = true;
      }
    }
  }
}

async function reconcileFailedRemoval(
  connectionId: string,
  connectionExists: (id: string) => Promise<boolean>,
  reconciliationTimeoutMs: number,
  intervalMs: number,
  onCheckFailure: (error: unknown) => void,
): Promise<boolean> {
  const deadline = Date.now() + reconciliationTimeoutMs;
  while (true) {
    try {
      if (!(await connectionExists(connectionId))) return true;
    } catch (error) {
      onCheckFailure(error);
    }

    const remainingMs = deadline - Date.now();
    if (remainingMs <= 0) return false;
    await wait(Math.min(intervalMs, remainingMs));
  }
}

function wait(delayMs: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, delayMs));
}
