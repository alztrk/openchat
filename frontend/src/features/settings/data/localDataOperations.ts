import { chatSchemaVersion, LocalServiceError } from '@/shared/services/LocalServiceClient';
import { join } from '@tauri-apps/api/path';
import { localServiceClient } from '@/features/chat/stores/chatStore';

const conversationArchiveTimeoutMs = 30 * 60 * 1000;
const profileArchiveTimeoutMs = 3 * 60 * 60 * 1000;

export function buildArchiveOutputPath(
  directory: string,
  archiveType: 'openchatbackup' | 'openchatprofilebackup',
): Promise<string> {
  const filename = `OpenChat-${Math.trunc((performance.timeOrigin + performance.now()) * 1000)}.${archiveType}`;
  return join(directory, filename);
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

export interface ProfileArchiveResult {
  conversationCount: number;
  messageCount: number;
  attachmentCount: number;
  attachmentBytes: number;
  requiresRestart: boolean;
}

export async function exportConversationArchive(
  conversationIds: string[],
  path: string,
  passphrase: string,
): Promise<ConversationArchiveResult> {
  const value = await localServiceClient.call('conversation.archive.export', {
    conversationIds,
    path,
    passphrase,
  }, conversationArchiveTimeoutMs);
  return parseConversationArchiveResult(value);
}

export async function inspectConversationArchive(
  path: string,
  passphrase: string,
): Promise<ConversationArchiveSummary> {
  const value = await localServiceClient.call('conversation.archive.inspect', {
    path,
    passphrase,
  }, conversationArchiveTimeoutMs);
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
  const value = await localServiceClient.call('conversation.archive.restore', {
    path,
    passphrase,
    conflictPolicy,
  }, conversationArchiveTimeoutMs);
  return parseConversationArchiveResult(value);
}

export async function exportProfileArchive(
  path: string,
  passphrase: string,
): Promise<ProfileArchiveResult> {
  const value = await localServiceClient.call('profile.archive.export', {
    path,
    passphrase,
  }, profileArchiveTimeoutMs);
  return parseProfileArchiveResult(value);
}

export async function prepareProfileRestore(
  path: string,
  passphrase: string,
): Promise<ProfileArchiveResult> {
  const value = await localServiceClient.call('profile.archive.prepare_restore', {
    path,
    passphrase,
    chatSchemaVersion,
  }, profileArchiveTimeoutMs);
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

function invalidResponse(message: string): LocalServiceError {
  return new LocalServiceError('invalid_service_response', message);
}
