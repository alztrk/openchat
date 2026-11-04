export interface CancellableSecurityScanOperation {
  cancel(): Promise<boolean>;
  result: Promise<unknown>;
}

export interface SecurityScanCancellationWaitResult {
  settled: boolean;
  cancellationAccepted: boolean | null;
}

type WaitOutcome = { kind: 'settled' } | { kind: 'cancelled'; accepted: boolean } | { kind: 'timeout' };

export async function cancelSecurityScanOperationAndWait(
  operation: CancellableSecurityScanOperation,
  timeoutMs: number,
): Promise<SecurityScanCancellationWaitResult> {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 0) {
    throw new RangeError('The operation cancellation timeout must be a non-negative safe integer.');
  }

  const deadline = Date.now() + timeoutMs;
  const settled = operation.result.then<WaitOutcome, WaitOutcome>(
    () => ({ kind: 'settled' }),
    () => ({ kind: 'settled' }),
  );
  const cancellation = Promise.resolve()
    .then(() => operation.cancel())
    .then<WaitOutcome, WaitOutcome>(
      (accepted) => ({ kind: 'cancelled', accepted }),
      () => ({ kind: 'cancelled', accepted: false }),
    );

  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<WaitOutcome>((resolve) => {
    timer = setTimeout(() => resolve({ kind: 'timeout' }), timeoutMs);
  });

  try {
    const firstOutcome = await Promise.race([settled, cancellation, timeout]);
    if (firstOutcome.kind === 'settled') {
      return { settled: true, cancellationAccepted: null };
    }
    if (firstOutcome.kind === 'timeout') {
      return { settled: false, cancellationAccepted: null };
    }

    const remainingMs = deadline - Date.now();
    if (remainingMs <= 0) {
      return { settled: false, cancellationAccepted: firstOutcome.accepted };
    }

    const settlementOutcome = await Promise.race([settled, timeout]);
    return {
      settled: settlementOutcome.kind === 'settled',
      cancellationAccepted: firstOutcome.accepted,
    };
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
