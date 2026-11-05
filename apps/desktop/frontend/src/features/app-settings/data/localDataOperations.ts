import {
  type ServiceOperation,
  getChatSchemaVersion,
  LocalServiceError,
  localServiceClient,
} from '@shared/lib/platform/local-service';
import { savePendingArchiveExport as saveArchiveExport } from '@shared/lib/platform/tauri';

export const CONVERSATION_ARCHIVE_TIMEOUT_MS = 30 * 60 * 1000;
const profileArchiveTimeoutMs = 3 * 60 * 60 * 1000;

export function buildArchiveFileName(archiveType: 'openchatbackup' | 'openchatprofilebackup'): string {
  return `OpenChat-${Date.now()}.${archiveType}`;
}

export async function savePendingArchiveExport(
  kind: 'conversation' | 'profile',
  exportId: string,
  fileName: string,
  title: string,
): Promise<boolean> {
  try {
    return await saveArchiveExport(kind, exportId, fileName, title);
  } catch (error) {
    if (typeof error === 'object' && error !== null && 'code' in error && 'message' in error) {
      const { code, message } = error;
      if (typeof code === 'string' && typeof message === 'string') {
        throw new LocalServiceError(code, message);
      }
    }
    throw error;
  }
}

export interface ConversationArchiveSummary {
  conversationCount: number;
  messageCount: number;
  attachmentCount: number;
  attachmentBytes: number;
  duplicateConversationCount: number;
  createdAtUnixMs: number;
  formatVersion: number;
}

export interface ConversationArchiveResult {
  conversationCount: number;
  messageCount: number;
  attachmentCount: number;
  attachmentBytes: number;
  duplicateConversationCount: number;
}

export interface ConversationArchiveExportResult extends ConversationArchiveResult {
  exportId: string;
}

export type ConversationArchiveAction = 'export' | 'inspect' | 'restore';

export type ConversationArchiveOperationProgress = {
  phase: 'reading' | 'writing' | 'verifying' | 'restoring' | 'committing';
  completedItems: number;
  totalItems: number | null;
  completedBytes: number;
  totalBytes: number | null;
};

export interface ConversationArchiveOperationStatus {
  operationInProgress: boolean;
  operationId: number | null;
  action: ConversationArchiveAction | null;
  progress: ConversationArchiveOperationProgress | null;
}

export interface RawConversationArchiveOperation {
  id: number;
  action: ConversationArchiveAction;
  result: ServiceOperation['result'];
  cancel: ServiceOperation['cancel'];
}

export type ConversationArchiveOperationHandle<T> = Omit<RawConversationArchiveOperation, 'result'> & {
  result: Promise<T>;
};

let activeConversationArchiveOperation: RawConversationArchiveOperation | null = null;
let startingConversationArchiveOperation:
  | { action: ConversationArchiveAction; promise: Promise<RawConversationArchiveOperation> }
  | null = null;

export interface ProfileArchiveResult {
  conversationCount: number;
  messageCount: number;
  attachmentCount: number;
  attachmentBytes: number;
  requiresRestart: boolean;
}

export interface ProfileArchiveExportResult extends ProfileArchiveResult {
  exportId: string;
}

export async function startConversationArchiveExport(
  conversationIds: string[],
  passphrase: string,
): Promise<ConversationArchiveOperationHandle<ConversationArchiveExportResult>> {
  const operation = await startRawConversationArchiveOperation(
    'export',
    'conversation.archive.export',
    {
      conversationIds,
      passphrase,
    },
  );
  return {
    ...operation,
    result: operation.result.then((value) => ({
      ...parseConversationArchiveResult(value),
      exportId: requiredExportId(value),
    })),
  };
}

export async function startConversationArchiveInspection(
  path: string,
  passphrase: string,
): Promise<ConversationArchiveOperationHandle<ConversationArchiveSummary>> {
  const operation = await startRawConversationArchiveOperation(
    'inspect',
    'conversation.archive.inspect',
    {
      path,
      passphrase,
    },
  );
  return {
    ...operation,
    result: operation.result.then((value) => ({
      conversationCount: requiredCount(value, 'conversationCount'),
      messageCount: requiredCount(value, 'messageCount'),
      attachmentCount: requiredCount(value, 'attachmentCount'),
      attachmentBytes: requiredCount(value, 'attachmentBytes'),
      duplicateConversationCount: requiredCount(value, 'duplicateConversationCount'),
      createdAtUnixMs: requiredCount(value, 'createdAtUnixMs'),
      formatVersion: requiredPositiveCount(value, 'formatVersion'),
    })),
  };
}

export async function startConversationArchiveRestore(
  path: string,
  passphrase: string,
  conflictPolicy: 'skip_existing' | 'import_as_copy',
): Promise<ConversationArchiveOperationHandle<ConversationArchiveResult>> {
  const operation = await startRawConversationArchiveOperation(
    'restore',
    'conversation.archive.restore',
    {
      path,
      passphrase,
      conflictPolicy,
    },
  );
  return { ...operation, result: operation.result.then(parseConversationArchiveResult) };
}

export function getActiveConversationArchiveOperation(): RawConversationArchiveOperation | null {
  return activeConversationArchiveOperation;
}

export async function getConversationArchiveOperationStatus(): Promise<ConversationArchiveOperationStatus> {
  const value = await localServiceClient.call('conversation.archive.status');
  if (
    typeof value.operationInProgress !== 'boolean' ||
    (value.operationId !== null &&
      (typeof value.operationId !== 'number' || !Number.isSafeInteger(value.operationId) || value.operationId < 0)) ||
    (value.action !== null && !isConversationArchiveAction(value.action)) ||
    (value.progress !== null && !isConversationArchiveProgress(value.progress))
  ) {
    throw invalidResponse('The archive operation status is invalid.');
  }
  if (value.operationInProgress && (value.operationId === null || value.action === null)) {
    throw invalidResponse('The active archive operation status is incomplete.');
  }
  if (
    !value.operationInProgress &&
    (value.operationId !== null || value.action !== null || value.progress !== null)
  ) {
    throw invalidResponse('The idle archive operation status contains active operation data.');
  }
  return {
    operationInProgress: value.operationInProgress,
    operationId: value.operationId,
    action: value.action,
    progress: value.progress,
  };
}

export async function cancelConversationArchiveOperation(operationId: number): Promise<boolean> {
  const result = await localServiceClient.call('system.cancel', { targetId: operationId });
  return result.cancelled === true;
}

function startRawConversationArchiveOperation(
  action: ConversationArchiveAction,
  method: string,
  params: Record<string, unknown>,
): Promise<RawConversationArchiveOperation> {
  if (activeConversationArchiveOperation) {
    return activeConversationArchiveOperation.action === action
      ? Promise.resolve(activeConversationArchiveOperation)
      : Promise.reject(archiveOperationBusy());
  }
  if (startingConversationArchiveOperation) {
    return startingConversationArchiveOperation.action === action
      ? startingConversationArchiveOperation.promise
      : Promise.reject(archiveOperationBusy());
  }

  const promise = localServiceClient.startOperation(method, params).then((operation) => {
    const active: RawConversationArchiveOperation = { ...operation, action };
    activeConversationArchiveOperation = active;
    const clearActiveOperation = () => {
      if (activeConversationArchiveOperation?.id === active.id) {
        activeConversationArchiveOperation = null;
      }
    };
    void operation.result.then(clearActiveOperation, clearActiveOperation);
    return active;
  });
  const trackedPromise = promise.finally(() => {
    if (startingConversationArchiveOperation?.promise === trackedPromise) {
      startingConversationArchiveOperation = null;
    }
  });
  startingConversationArchiveOperation = { action, promise: trackedPromise };
  return trackedPromise;
}

function archiveOperationBusy(): LocalServiceError {
  return new LocalServiceError(
    'conversation_archive_operation_busy',
    'A conversation archive operation is already in progress.',
    true,
  );
}

function isConversationArchiveAction(value: unknown): value is ConversationArchiveAction {
  return value === 'export' || value === 'inspect' || value === 'restore';
}

function isConversationArchiveProgress(value: unknown): value is ConversationArchiveOperationProgress {
  if (!isObject(value)) return false;
  return (
    (value.phase === 'reading' ||
      value.phase === 'writing' ||
      value.phase === 'verifying' ||
      value.phase === 'restoring' ||
      value.phase === 'committing') &&
    isNonNegativeSafeInteger(value.completedItems) &&
    (value.totalItems === null || isNonNegativeSafeInteger(value.totalItems)) &&
    isNonNegativeSafeInteger(value.completedBytes) &&
    (value.totalBytes === null || isNonNegativeSafeInteger(value.totalBytes))
  );
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export async function exportProfileArchive(passphrase: string): Promise<ProfileArchiveExportResult> {
  const value = await localServiceClient.call('profile.archive.export', { passphrase }, profileArchiveTimeoutMs);
  return {
    ...parseProfileArchiveResult(value),
    exportId: requiredExportId(value),
  };
}

export async function prepareProfileRestore(path: string, passphrase: string): Promise<ProfileArchiveResult> {
  const chatSchemaVersion = await getChatSchemaVersion();
  const value = await localServiceClient.call(
    'profile.archive.prepare_restore',
    {
      path,
      passphrase,
      chatSchemaVersion,
    },
    profileArchiveTimeoutMs,
  );
  return parseProfileArchiveResult(value);
}

function parseConversationArchiveResult(value: Record<string, unknown>): ConversationArchiveResult {
  return {
    conversationCount: requiredAlternativeCount(value, 'restoredConversationCount', 'conversationCount'),
    messageCount: requiredAlternativeCount(value, 'restoredMessageCount', 'messageCount'),
    attachmentCount: requiredAlternativeCount(value, 'restoredAttachmentCount', 'attachmentCount'),
    attachmentBytes: optionalCount(value, 'attachmentBytes') ?? 0,
    duplicateConversationCount: optionalCount(value, 'duplicateConversationCount') ?? 0,
  };
}

function parseProfileArchiveResult(value: Record<string, unknown>): ProfileArchiveResult {
  if (typeof value.requiresRestart !== 'boolean') {
    throw invalidResponse('The profile archive response is invalid.');
  }
  return {
    conversationCount: requiredCount(value, 'conversationCount'),
    messageCount: requiredCount(value, 'messageCount'),
    attachmentCount: requiredCount(value, 'attachmentCount'),
    attachmentBytes: requiredCount(value, 'attachmentBytes'),
    requiresRestart: value.requiresRestart,
  };
}

function requiredAlternativeCount(value: Record<string, unknown>, preferred: string, alternative: string): number {
  return requiredCount(value, preferred, optionalCount(value, alternative));
}

function requiredCount(value: Record<string, unknown>, field: string, alternative?: number): number {
  const result = optionalCount(value, field) ?? alternative;
  if (result === undefined) throw invalidResponse(`The archive field "${field}" is invalid.`);
  return result;
}

function requiredPositiveCount(value: Record<string, unknown>, field: string): number {
  const result = requiredCount(value, field);
  if (result === 0) throw invalidResponse(`The archive field "${field}" is invalid.`);
  return result;
}

function optionalCount(value: Record<string, unknown>, field: string): number | undefined {
  const raw = value[field];
  if (raw === undefined || raw === null) return undefined;
  if (typeof raw !== 'number' || !Number.isSafeInteger(raw) || raw < 0) {
    throw invalidResponse(`The archive field "${field}" is invalid.`);
  }
  return raw;
}

function requiredExportId(value: Record<string, unknown>): string {
  const exportId = value.exportId;
  if (typeof exportId !== 'string' || !/^[0-9a-f]{32}$/.test(exportId)) {
    throw invalidResponse('The archive export identifier is invalid.');
  }
  return exportId;
}

function invalidResponse(message: string): LocalServiceError {
  return new LocalServiceError('invalid_service_response', message);
}
