import type { TFunction } from 'i18next';
import { useSyncExternalStore } from 'react';

import { type ChatGoal, parseChatGoal } from '@/modules/conversations/data/chatGoals';
import { type ChatMessage, getMessages, saveMessage } from '@/modules/conversations/data/chatStorage';
import {
  formatUnknownServiceError,
  resolveServiceFailureCode,
} from '@/modules/conversations/stores/chatStoreErrorDetails';
import { applyStreamEvent, upsertMessage } from '@/modules/conversations/stores/chatStoreUtils';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@/platform/local-service';
import { retryDelayFromProviderDetails } from './securityScanRetry';
import {
  cancelSecurityScan,
  deleteSecurityScan,
  listSecurityScans,
  type SecurityScan,
  type SecurityScanStatus,
  scheduleSecurityScanRetry,
  securityScanObjective,
} from './securityScans';

export type SecurityScanErrorKey =
  | 'securityScanRequestFailed'
  | 'securityScanRouteMissing'
  | 'securityScanNoResumableCheckpoint'
  | 'securityScanStopFailed'
  | 'securityScanDeleteFailed'
  | 'securityScanStatusInvalid'
  | 'securityScansLoadFailed'
  | 'securityScanCreateFailed';

export class SecurityScanUiError extends Error {
  readonly translationKey: SecurityScanErrorKey;

  constructor(translationKey: SecurityScanErrorKey) {
    super(translationKey);
    this.name = 'SecurityScanUiError';
    this.translationKey = translationKey;
  }
}

export function formatSecurityScanError(
  error: unknown,
  t: TFunction,
  fallbackKey: SecurityScanErrorKey = 'securityScanRequestFailed',
): string | null {
  if (error === null || error === undefined) return null;
  if (error instanceof SecurityScanUiError) {
    return t(error.translationKey);
  }
  return formatUnknownServiceError(error, t, fallbackKey);
}

export interface SecurityScanRuntimeSnapshot {
  messages: ChatMessage[];
  goal: ChatGoal | null;
  isActive: boolean;
  error: unknown;
}

const emptySnapshot: SecurityScanRuntimeSnapshot = {
  messages: [],
  goal: null,
  isActive: false,
  error: null,
};
const snapshots = new Map<string, SecurityScanRuntimeSnapshot>();
const listeners = new Map<string, Set<() => void>>();
const operations = new Map<string, ServiceOperation>();
const operationStarts = new Map<string, Promise<ServiceOperation>>();
const inFlight = new Set<string>();
const refreshingScans = new Set<string>();
const deletedScans = new Set<string>();
const retryTimers = new Map<string, number>();
const resumeAfterOperation = new Set<string>();
const initialized = new Set<string>();

const STREAM_SAVE_THROTTLE_MS = 2_500;
const streamTextEncoder = new TextEncoder();

export function useSecurityScanRuntime(scanId: string | null): SecurityScanRuntimeSnapshot {
  const subscribe = (listener: () => void) => {
    if (!scanId) return () => undefined;
    const current = listeners.get(scanId) ?? new Set<() => void>();
    current.add(listener);
    listeners.set(scanId, current);
    return () => {
      current.delete(listener);
      if (current.size === 0) listeners.delete(scanId);
    };
  };
  const getSnapshot = () => (scanId ? (snapshots.get(scanId) ?? emptySnapshot) : emptySnapshot);
  return useSyncExternalStore(subscribe, getSnapshot, () => emptySnapshot);
}

export async function initializeActiveSecurityScans(): Promise<void> {
  const scans = await listSecurityScans();
  await Promise.all(
    scans
      .filter((scan) => ['queued', 'running', 'waiting_provider', 'interrupted'].includes(scan.status))
      .map(reconcileActiveSecurityScan),
  );
}

async function reconcileActiveSecurityScan(scan: SecurityScan): Promise<void> {
  const existing = snapshots.get(scan.id);
  if (!existing) {
    await initializeSecurityScan(scan);
    const initializedSnapshot = snapshots.get(scan.id);
    if (!initializedSnapshot || initializedSnapshot.error !== null) {
      throw initializedSnapshot?.error instanceof Error
        ? initializedSnapshot.error
        : new SecurityScanUiError('securityScanRequestFailed');
    }
    return;
  }

  try {
    const hasActiveOperation = operations.has(scan.id) || inFlight.has(scan.id);
    const [messages, activeGoal] = await Promise.all([
      hasActiveOperation ? Promise.resolve(existing.messages) : getMessages(scan.conversationId),
      readActiveGoal(scan.conversationId),
    ]);
    const goal = resolveRetainedGoal(activeGoal, existing.goal, scan.conversationId, scan.status);
    update(scan.id, { messages, goal, error: null });

    if (goal?.status === 'interrupted') {
      if (hasActiveOperation) resumeAfterOperation.add(scan.id);
      else void resumeSecurityScan(scan, goal);
    } else if (goal?.status === 'paused' && goal.pauseReason === 'quota') {
      scheduleQuotaResume(scan, goal);
    } else if (scan.status === 'queued' && goal === null && !hasActiveOperation) {
      if (messages.length === 0) await saveInitialMessage(scan);
      void startSecurityScan(scan);
    }
  } catch (error) {
    update(scan.id, { error });
    throw error;
  }
}

export async function initializeSecurityScan(scan: SecurityScan): Promise<void> {
  const existing = snapshots.get(scan.id);
  if (existing) {
    if (scan.status === 'waiting_provider' && existing.goal?.pauseReason === 'quota') {
      scheduleQuotaResume(scan, existing.goal);
    }
    return;
  }
  if (initialized.has(scan.id)) return;
  initialized.add(scan.id);
  try {
    const [messages, goal] = await Promise.all([getMessages(scan.conversationId), readActiveGoal(scan.conversationId)]);
    update(scan.id, { messages, goal });
    if (scan.status === 'queued' && goal === null) {
      if (messages.length === 0) await saveInitialMessage(scan);
      void startSecurityScan(scan);
    } else if (goal?.status === 'interrupted') {
      void resumeSecurityScan(scan, goal);
    } else if (goal?.status === 'paused' && goal.pauseReason === 'quota') {
      scheduleQuotaResume(scan, goal);
    }
  } catch (error) {
    initialized.delete(scan.id);
    update(scan.id, { error });
  }
}

export async function startSecurityScan(scan: SecurityScan): Promise<void> {
  await startOperation(scan, null);
}

export async function resumeSecurityScan(scan: SecurityScan, goal?: ChatGoal | null): Promise<void> {
  try {
    const activeGoal = goal ?? (await readActiveGoal(scan.conversationId));
    if (!activeGoal && scan.status === 'queued') {
      const messages = await getMessages(scan.conversationId);
      if (messages.length === 0) await saveInitialMessage(scan);
      else update(scan.id, { messages });
      await startOperation(scan, null);
      return;
    }
    if (!activeGoal || !['paused', 'interrupted'].includes(activeGoal.status)) {
      update(scan.id, { error: new SecurityScanUiError('securityScanNoResumableCheckpoint') });
      return;
    }
    clearRetryTimer(scan.id);
    update(scan.id, { goal: activeGoal, error: null });
    await startOperation(scan, activeGoal.runId);
  } catch (error) {
    update(scan.id, { error });
  }
}

export async function stopSecurityScan(scan: SecurityScan): Promise<void> {
  clearRetryTimer(scan.id);
  resumeAfterOperation.delete(scan.id);
  try {
    const goal = snapshots.get(scan.id)?.goal ?? (await readActiveGoal(scan.conversationId));
    let operation = operations.get(scan.id);
    const pendingStart = operationStarts.get(scan.id);
    if (!operation && pendingStart) {
      try {
        operation = await pendingStart;
      } catch {
        operation = undefined;
      }
    }
    if (operation) {
      const cancellationAccepted = await operation.cancel();
      if (!cancellationAccepted) {
        throw new SecurityScanUiError('securityScanStopFailed');
      }
    } else if (goal) {
      const response = await localServiceClient.call('chat.goal.stop', {
        conversationId: scan.conversationId,
        runId: goal.runId,
      });
      const stoppedGoal = parseChatGoal(response);
      if (!stoppedGoal) throw new SecurityScanUiError('securityScanStopFailed');
      update(scan.id, { goal: stoppedGoal });
    }
    await cancelSecurityScan(scan.id);
    update(scan.id, { isActive: operations.has(scan.id) || inFlight.has(scan.id), error: null });
    await refreshMessages(scan);
  } catch (error) {
    update(scan.id, { error });
  }
}

export async function deleteSecurityScanRuntime(scan: SecurityScan): Promise<void> {
  deletedScans.add(scan.id);
  clearRetryTimer(scan.id);
  resumeAfterOperation.delete(scan.id);
  try {
    let operation = operations.get(scan.id);
    const pendingStart = operationStarts.get(scan.id);
    if (!operation && pendingStart) {
      try {
        operation = await pendingStart;
      } catch {
        operation = undefined;
      }
    }
    if (operation) {
      await operation.cancel().catch(() => false);
      try {
        await operation.result;
      } catch {
        // Cancelled or failed operation has finished unwinding before database deletion.
      }
    }
    await deleteSecurityScan(scan.id);
    operations.delete(scan.id);
    operationStarts.delete(scan.id);
    inFlight.delete(scan.id);
    refreshingScans.delete(scan.id);
    initialized.delete(scan.id);
    snapshots.delete(scan.id);
    for (const listener of listeners.get(scan.id) ?? []) listener();
  } catch (error) {
    deletedScans.delete(scan.id);
    throw error;
  }
}

export async function refreshSecurityScan(scan: SecurityScan): Promise<void> {
  if (deletedScans.has(scan.id) || operations.has(scan.id) || inFlight.has(scan.id) || refreshingScans.has(scan.id)) {
    return;
  }
  refreshingScans.add(scan.id);
  try {
    const existing = snapshots.get(scan.id);
    const [messages, activeGoal] = await Promise.all([
      getMessages(scan.conversationId),
      readActiveGoal(scan.conversationId),
    ]);
    if (deletedScans.has(scan.id) || operations.has(scan.id) || inFlight.has(scan.id)) return;
    const goal = resolveRetainedGoal(activeGoal, existing?.goal ?? null, scan.conversationId, scan.status);
    update(scan.id, {
      messages,
      goal,
      ...(scan.status !== 'failed' ? { error: null } : {}),
    });
    if (goal?.status === 'interrupted' && !operations.has(scan.id)) {
      void resumeSecurityScan(scan, goal);
    } else if (goal?.status === 'paused' && goal.pauseReason === 'quota') {
      scheduleQuotaResume(scan, goal);
    }
  } catch (error) {
    if (deletedScans.has(scan.id) || operations.has(scan.id) || inFlight.has(scan.id)) return;
    const isMappedError =
      error instanceof LocalServiceError ||
      error instanceof SecurityScanUiError ||
      resolveServiceFailureCode(error, 'unmapped_refresh_error') !== 'unmapped_refresh_error';
    update(scan.id, {
      error: isMappedError ? error : new SecurityScanUiError('securityScansLoadFailed'),
    });
  } finally {
    refreshingScans.delete(scan.id);
  }
}

async function startOperation(scan: SecurityScan, resumeRunId: string | null): Promise<void> {
  if (deletedScans.has(scan.id) || operations.has(scan.id) || inFlight.has(scan.id)) return;
  if (!scan.providerId || !scan.modelId) {
    update(scan.id, { error: new SecurityScanUiError('securityScanRouteMissing') });
    return;
  }
  inFlight.add(scan.id);
  update(scan.id, { isActive: true, error: null });
  let assistantMessage: ChatMessage | null = null;
  let pendingMessage: ChatMessage | null = null;
  let throttledMessage: ChatMessage | null = null;
  let throttleTimer: number | undefined;
  let lastSaveStartedAt = 0;
  let persistWorker: Promise<void> | null = null;
  let pendingTextBytes = 0;
  let persistenceError: unknown = null;
  let operationPromise: Promise<ServiceOperation> | null = null;

  const clearThrottleTimer = (): void => {
    if (throttleTimer !== undefined) {
      window.clearTimeout(throttleTimer);
      throttleTimer = undefined;
    }
  };
  const persistLatest = (): void => {
    if (deletedScans.has(scan.id) || !pendingMessage || persistWorker) return;
    lastSaveStartedAt = Date.now();
    persistWorker = (async () => {
      while (pendingMessage && !deletedScans.has(scan.id)) {
        const message = pendingMessage;
        pendingMessage = null;
        await saveMessage(message);
      }
    })()
      .catch((error: unknown) => {
        persistenceError = error;
      })
      .finally(() => {
        persistWorker = null;
        if (pendingMessage && !deletedScans.has(scan.id)) persistLatest();
      });
  };
  const enqueueSaveImmediate = (message: ChatMessage): void => {
    if (deletedScans.has(scan.id)) return;
    clearThrottleTimer();
    throttledMessage = null;
    pendingMessage = message;
    persistLatest();
  };
  const scheduleCheckpointSave = (message: ChatMessage): void => {
    if (deletedScans.has(scan.id)) return;
    const elapsedSinceSave = Date.now() - lastSaveStartedAt;
    if (lastSaveStartedAt === 0 || elapsedSinceSave >= STREAM_SAVE_THROTTLE_MS) {
      enqueueSaveImmediate(message);
      return;
    }
    throttledMessage = message;
    if (throttleTimer !== undefined) return;
    throttleTimer = window.setTimeout(
      () => {
        throttleTimer = undefined;
        if (throttledMessage && !deletedScans.has(scan.id)) {
          const next = throttledMessage;
          throttledMessage = null;
          pendingMessage = next;
          persistLatest();
        }
      },
      Math.max(50, STREAM_SAVE_THROTTLE_MS - elapsedSinceSave),
    );
  };
  const flushScheduledSave = (): void => {
    clearThrottleTimer();
    if (throttledMessage && !deletedScans.has(scan.id)) {
      pendingMessage = throttledMessage;
      throttledMessage = null;
      persistLatest();
    }
  };
  const latestAssistantMessage = (): ChatMessage | null => assistantMessage;

  try {
    operationPromise = localServiceClient.startOperation(
      'chat.send',
      {
        conversationId: scan.conversationId,
        ...(resumeRunId ? { resumeRunId } : { goal: true, goalObjective: securityScanObjective }),
      },
      (event) => {
        if (deletedScans.has(scan.id)) return;
        if (event.name === 'chat.goal.updated') {
          try {
            const goal = parseChatGoal(event.data);
            if (!goal || goal.conversationId !== scan.conversationId) {
              throw new SecurityScanUiError('securityScanStatusInvalid');
            }
            update(scan.id, { goal });
          } catch (error) {
            update(scan.id, { error });
          }
        }
        const message = applyStreamEvent(event, assistantMessage, {
          conversationId: scan.conversationId,
          providerId: scan.providerId ?? '',
          modelId: scan.modelId ?? '',
        });
        if (!message) return;
        assistantMessage = message;
        update(scan.id, { messages: upsertMessage(snapshots.get(scan.id)?.messages ?? [], message) });
        if (event.name === 'chat.started') {
          pendingTextBytes = 0;
          enqueueSaveImmediate(message);
          return;
        }
        if (event.name === 'chat.text.delta' && typeof event.data.delta === 'string') {
          pendingTextBytes += streamTextEncoder.encode(event.data.delta).byteLength;
        }
        if (
          pendingTextBytes >= 256 * 1024 ||
          ['chat.delta', 'chat.reasoning.delta', 'chat.tool.updated', 'chat.citations.updated'].includes(event.name)
        ) {
          pendingTextBytes = 0;
          scheduleCheckpointSave(message);
        }
      },
    );
    operationStarts.set(scan.id, operationPromise);
    const operation = await operationPromise;
    operations.set(scan.id, operation);
    const result = await operation.result;
    if (deletedScans.has(scan.id)) return;
    flushScheduledSave();
    const finalStatus = result.status === 'stopped' ? 'stopped' : 'completed';
    const completedMessage = latestAssistantMessage();
    if (completedMessage) {
      assistantMessage = {
        ...completedMessage,
        status: finalStatus,
        outputTokens: numberOrNull(result.outputTokens) ?? completedMessage.outputTokens,
        elapsedMicroseconds: numberOrNull(result.elapsedMicroseconds) ?? completedMessage.elapsedMicroseconds,
        tokensPerSecond: numberOrNull(result.tokensPerSecond) ?? completedMessage.tokensPerSecond,
      };
      update(scan.id, { messages: upsertMessage(snapshots.get(scan.id)?.messages ?? [], assistantMessage) });
      enqueueSaveImmediate(assistantMessage);
    }
    await waitForPersistence(
      () => persistWorker,
      () => pendingMessage,
    );
    if (persistenceError) throw persistenceError;
    update(scan.id, { isActive: false });
    const activeGoal = await readActiveGoal(scan.conversationId);
    const goal = resolveRetainedGoal(
      activeGoal,
      snapshots.get(scan.id)?.goal ?? null,
      scan.conversationId,
      finalStatus === 'stopped' ? 'cancelled' : 'completed',
    );
    update(scan.id, { goal });
    if (goal?.status === 'paused' && goal.pauseReason === 'quota') {
      scheduleQuotaResume(scan, goal);
    }
  } catch (error) {
    clearThrottleTimer();
    throttledMessage = null;
    if (deletedScans.has(scan.id)) return;
    const failureCode = resolveServiceFailureCode(error, 'security_scan_request_failed');
    const failedMessage = latestAssistantMessage();
    if (failedMessage) {
      assistantMessage = { ...failedMessage, status: 'failed', failureCode };
      update(scan.id, { messages: upsertMessage(snapshots.get(scan.id)?.messages ?? [], assistantMessage) });
      enqueueSaveImmediate(assistantMessage);
    }
    try {
      await waitForPersistence(
        () => persistWorker,
        () => pendingMessage,
      );
    } catch (persistenceFailure) {
      globalThis.reportError(new Error('A security scan message could not be saved.', { cause: persistenceFailure }));
    }
    update(scan.id, { isActive: false, error: failedMessage ? null : error });
    try {
      const activeGoal = await readActiveGoal(scan.conversationId);
      const goal = resolveRetainedGoal(activeGoal, snapshots.get(scan.id)?.goal ?? null, scan.conversationId, 'failed');
      update(scan.id, { goal });
      if (goal?.status === 'paused' && goal.pauseReason === 'quota') {
        scheduleQuotaResume(
          scan,
          goal,
          error instanceof LocalServiceError ? retryDelayFromProviderDetails(error.details) : null,
        );
      }
    } catch (goalError) {
      globalThis.reportError(new Error('The security scan state could not be refreshed.', { cause: goalError }));
    }
  } finally {
    clearThrottleTimer();
    if (operationPromise && operationStarts.get(scan.id) === operationPromise) {
      operationStarts.delete(scan.id);
    }
    operations.delete(scan.id);
    inFlight.delete(scan.id);
    if (!deletedScans.has(scan.id)) {
      const interruptedGoal = snapshots.get(scan.id)?.goal;
      if (resumeAfterOperation.delete(scan.id) && interruptedGoal?.status === 'interrupted') {
        void resumeSecurityScan(scan, interruptedGoal);
      }
    }
  }
}

function scheduleQuotaResume(scan: SecurityScan, goal: ChatGoal, authoritativeDelayMs: number | null = null): void {
  if (operations.has(scan.id) || inFlight.has(scan.id) || retryTimers.has(scan.id)) return;
  const scheduledRetryAt = scan.nextRetryAtUnixMs;
  const hasScheduledRetry = scheduledRetryAt !== null;
  const retryCount = Math.min(scan.retryCount + (authoritativeDelayMs !== null || !hasScheduledRetry ? 1 : 0), 100);
  const now = Date.now();
  const backoffMs = Math.min(60_000 * 2 ** Math.min(retryCount - 1, 4), 15 * 60_000);
  const retryAt =
    authoritativeDelayMs !== null
      ? now + authoritativeDelayMs
      : hasScheduledRetry
        ? Math.max(now, scheduledRetryAt)
        : now + backoffMs;
  const persistSchedule = hasScheduledRetry
    ? Promise.resolve()
    : scheduleSecurityScanRetry(scan.id, retryCount, retryAt);
  void persistSchedule
    .then(() => {
      const timer = window.setTimeout(
        () => {
          retryTimers.delete(scan.id);
          void resumeSecurityScan({ ...scan, retryCount, nextRetryAtUnixMs: null }, goal);
        },
        Math.max(0, retryAt - Date.now()),
      );
      retryTimers.set(scan.id, timer);
    })
    .catch((error: unknown) => update(scan.id, { error }));
}

async function saveInitialMessage(scan: SecurityScan): Promise<void> {
  if (!scan.providerId || !scan.modelId) throw new SecurityScanUiError('securityScanRouteMissing');
  const message: ChatMessage = {
    id: crypto.randomUUID().replaceAll('-', ''),
    conversationId: scan.conversationId,
    role: 'user',
    content: securityScanObjective,
    attachments: [],
    createdAt: Date.now(),
    outputTokens: null,
    tokensPerSecond: null,
    elapsedMicroseconds: null,
    providerId: scan.providerId,
    modelId: scan.modelId,
    citationSources: '[]',
    personalMemoryFactIds: '[]',
    reasoningSummaries: '[]',
    toolActivities: '[]',
    status: 'completed',
    failureCode: null,
  };
  await saveMessage(message);
  update(scan.id, { messages: [message] });
}

async function refreshMessages(scan: SecurityScan): Promise<void> {
  const existing = snapshots.get(scan.id);
  const messages = await getMessages(scan.conversationId);
  const activeGoal = await readActiveGoal(scan.conversationId);
  const goal = resolveRetainedGoal(activeGoal, existing?.goal ?? null, scan.conversationId, 'cancelled');
  update(scan.id, { messages, goal });
}

async function readActiveGoal(conversationId: string): Promise<ChatGoal | null> {
  const response = await localServiceClient.call('chat.goal.active', { conversationId });
  if (!('goal' in response)) throw new SecurityScanUiError('securityScanStatusInvalid');
  const goal = parseChatGoal(response.goal);
  if (goal && goal.conversationId !== conversationId) throw new SecurityScanUiError('securityScanStatusInvalid');
  return goal;
}

function update(scanId: string, update: Partial<SecurityScanRuntimeSnapshot>): void {
  const current = snapshots.get(scanId) ?? emptySnapshot;
  snapshots.set(scanId, { ...current, ...update });
  for (const listener of listeners.get(scanId) ?? []) listener();
}

function clearRetryTimer(scanId: string): void {
  const timer = retryTimers.get(scanId);
  if (timer !== undefined) window.clearTimeout(timer);
  retryTimers.delete(scanId);
}

async function waitForPersistence(
  getWorker: () => Promise<void> | null,
  getPending: () => ChatMessage | null,
): Promise<void> {
  while (getWorker() || getPending()) {
    const worker = getWorker();
    if (worker) await worker;
    else await new Promise((resolve) => window.setTimeout(resolve, 0));
  }
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function resolveRetainedGoal(
  activeGoal: ChatGoal | null,
  previousGoal: ChatGoal | null,
  conversationId: string,
  scanStatus: SecurityScanStatus,
): ChatGoal | null {
  if (activeGoal) return activeGoal;
  if (!previousGoal || previousGoal.conversationId !== conversationId) return null;
  if (scanStatus === 'completed' || scanStatus === 'cancelled' || scanStatus === 'failed') {
    return previousGoal.status === scanStatus ? previousGoal : { ...previousGoal, status: scanStatus };
  }
  return previousGoal;
}
