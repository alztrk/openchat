import { getChatSchemaVersion, LocalServiceError, localServiceClient } from '@/platform/local-service';
import { savePendingArchiveExport as saveArchiveExport } from '@/platform/tauri';

const conversationArchiveTimeoutMs = 30 * 60 * 1000;
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

export async function exportConversationArchive(
  conversationIds: string[],
  passphrase: string,
): Promise<ConversationArchiveExportResult> {
  const value = await localServiceClient.call(
    'conversation.archive.export',
    {
      conversationIds,
      passphrase,
    },
    conversationArchiveTimeoutMs,
  );
  return {
    ...parseConversationArchiveResult(value),
    exportId: requiredExportId(value),
  };
}

export async function inspectConversationArchive(
  path: string,
  passphrase: string,
): Promise<ConversationArchiveSummary> {
  const value = await localServiceClient.call(
    'conversation.archive.inspect',
    {
      path,
      passphrase,
    },
    conversationArchiveTimeoutMs,
  );
  return {
    conversationCount: requiredCount(value, 'conversationCount'),
    messageCount: requiredCount(value, 'messageCount'),
    attachmentCount: requiredCount(value, 'attachmentCount'),
    attachmentBytes: requiredCount(value, 'attachmentBytes'),
    duplicateConversationCount: requiredCount(value, 'duplicateConversationCount'),
    createdAtUnixMs: requiredCount(value, 'createdAtUnixMs'),
    formatVersion: requiredPositiveCount(value, 'formatVersion'),
  };
}

export async function restoreConversationArchive(
  path: string,
  passphrase: string,
  conflictPolicy: 'skip_existing' | 'import_as_copy',
): Promise<ConversationArchiveResult> {
  const value = await localServiceClient.call(
    'conversation.archive.restore',
    {
      path,
      passphrase,
      conflictPolicy,
    },
    conversationArchiveTimeoutMs,
  );
  return parseConversationArchiveResult(value);
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
