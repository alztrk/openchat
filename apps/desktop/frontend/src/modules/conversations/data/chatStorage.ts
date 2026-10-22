import type { ChatRoute } from '@/modules/providers/data/routes';
import { invoke } from '@/platform/tauri';

import {
  type AttachmentWritePayload,
  type ChatAttachmentMetadata,
  decodeBase64,
  decodeStoredMessageContent,
  encodeBase64,
  encodeStoredMessageContent,
  type PendingChatAttachment,
} from './chatAttachments';

export type ChatRole = 'user' | 'assistant';
export type ChatMessageStatus = 'streaming' | 'completed' | 'failed' | 'stopped';

export interface ChatConversation {
  id: string;
  title: string;
  titleSource: string;
  providerId: string | null;
  modelId: string | null;
  connectionId: string | null;
  workspaceId: string | null;
  apiKeyConnectionId: string | null;
  projectId: string | null;
  isPinned: boolean;
  isArchived: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface ChatProject {
  id: string;
  name: string;
  folderPath: string;
  createdAt: number;
  updatedAt: number;
}

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: ChatRole;
  content: string;
  attachments: ChatAttachmentMetadata[];
  createdAt: number | null;
  outputTokens: number | null;
  tokensPerSecond: number | null;
  elapsedMicroseconds: number | null;
  providerId: string | null;
  modelId: string | null;
  citationSources: string;
  personalMemoryFactIds: string;
  reasoningSummaries: string;
  toolActivities: string;
  status: ChatMessageStatus;
  failureCode: string | null;
}

export interface ChatModelFavorite {
  providerId: string;
  modelId: string;
  displayName: string;
  sourceConnectionId: string | null;
}

export interface SavedOutputMessageKey {
  conversationId: string;
  messageId: string;
}

export interface SavedOutputSummary {
  conversationId: string;
  conversationTitle: string;
  messageId: string;
  excerpt: string;
  createdAtUnixMs: number;
  selectedVersion: number;
  versionCount: number;
}

export interface SavedOutputVersionSummary {
  versionNumber: number;
  createdAtUnixMs: number;
}

export interface SavedOutputVersion extends SavedOutputVersionSummary {
  content: string;
}

export interface SavedOutputSearchInput {
  query: string;
  providerId: string | null;
  modelId: string | null;
  projectId: string | null;
  isArchived: boolean | null;
  sortOrder: 'relevance' | 'newest' | 'oldest';
  fromUnixMs: number | null;
  throughUnixMs: number | null;
}

export interface SavedOutputSearchMatch {
  conversationId: string;
  conversationTitle: string;
  isConversationTitleMatch: boolean;
  messageId: string;
  role: ChatRole;
  excerpt: string;
  createdAtUnixMs: number;
  projectId: string | null;
}

export interface ConversationAttachmentSummary {
  conversationId: string;
  conversationTitle: string;
  projectId: string | null;
  messageId: string;
  attachmentId: string;
  fileName: string;
  mimeType: string;
  sizeBytes: number;
  createdAt: number;
  isAvailable: boolean;
}

export interface ConversationAttachmentPage {
  items: ConversationAttachmentSummary[];
  nextOffset: number | null;
}

export interface AttachmentTextIndexStatus {
  enabled: boolean;
  indexedFileCount: number;
  indexedBytes: number;
  truncated: boolean;
}

export async function listConversations(): Promise<ChatConversation[]> {
  const value: unknown = await invoke('list_conversations');
  if (!Array.isArray(value) || !value.every(isConversation)) {
    throw new Error('The stored conversation list is invalid.');
  }
  return value;
}

export async function listProjects(): Promise<ChatProject[]> {
  const value: unknown = await invoke('list_projects');
  if (!Array.isArray(value) || !value.every(isProject)) {
    throw new Error('The stored project list is invalid.');
  }
  return value;
}

/** Persists a project from a one-time native folder selection token. */
export async function createProject(id: string, name: string, folderSelectionId: string): Promise<void> {
  await invoke('create_project', { input: { id, name, folderSelectionId } });
}

export async function renameConversation(conversationId: string, title: string): Promise<void> {
  await invoke('rename_conversation', { conversationId, title });
}

export async function setConversationRoute(conversationId: string, route: ChatRoute): Promise<void> {
  await invoke('set_conversation_route', {
    input: {
      conversationId,
      providerId: route.providerId,
      modelId: route.modelId,
      connectionId: route.connectionId ?? null,
      workspaceId: route.workspaceId ?? null,
      apiKeyConnectionId: route.apiKeyConnectionId ?? null,
    },
  });
}

export async function setConversationPinned(conversationId: string, isPinned: boolean): Promise<void> {
  await invoke('set_conversation_pinned', { conversationId, isPinned });
}

export async function moveConversations(conversationIds: string[], projectId: string | null): Promise<void> {
  await invoke('move_conversations', { conversationIds, projectId });
}

export async function setConversationsArchived(conversationIds: string[], isArchived: boolean): Promise<void> {
  await invoke('set_conversations_archived', { conversationIds, isArchived });
}

export async function deleteConversation(conversationId: string): Promise<void> {
  await invoke('delete_conversation', { conversationId });
}

export async function recordChatRequestFailure(
  stage: 'conversationCreation' | 'userMessagePersistence' | 'requestExecution',
  code: string,
): Promise<void> {
  await invoke('record_chat_request_failure', { stage, code });
}

export type ExportFormat = 'md' | 'html' | 'json';

export async function saveConversationExport(
  fileName: string,
  title: string,
  content: string,
  format: ExportFormat,
): Promise<boolean> {
  try {
    const value: unknown = await invoke('save_conversation_export', { fileName, title, content, format });
    if (typeof value === 'boolean') {
      return value;
    }
  } catch {
    if (format === 'md') {
      const value: unknown = await invoke('save_conversation_markdown', { fileName, title, content });
      if (typeof value === 'boolean') {
        return value;
      }
    }
    if (typeof document !== 'undefined') {
      triggerBrowserDownload(fileName, content, format);
      return true;
    }
    throw new Error('The conversation export result is invalid.');
  }
  return false;
}

function triggerBrowserDownload(fileName: string, content: string, format: ExportFormat): void {
  const mimeTypes: Record<ExportFormat, string> = {
    md: 'text/markdown;charset=utf-8',
    html: 'text/html;charset=utf-8',
    json: 'application/json;charset=utf-8',
  };
  const blob = new Blob([content], { type: mimeTypes[format] });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  URL.revokeObjectURL(url);
}

export async function saveConversationMarkdown(fileName: string, title: string, content: string): Promise<boolean> {
  return saveConversationExport(fileName, title, content, 'md');
}

export async function hasCompletedAssistantResponse(): Promise<boolean> {
  const value: unknown = await invoke('has_completed_assistant_response');
  if (typeof value !== 'boolean') {
    throw new Error('The completed response status is invalid.');
  }
  return value;
}

export async function clearConversationHistory(): Promise<void> {
  await invoke('clear_conversation_history');
}

export async function listModelFavorites(): Promise<ChatModelFavorite[]> {
  const value: unknown = await invoke('list_model_favorites');
  if (!Array.isArray(value) || !value.every(isModelFavorite)) {
    throw new Error('The stored model favorite list is invalid.');
  }
  return value;
}

export async function setModelFavorite(
  favorite: Omit<ChatModelFavorite, 'sourceConnectionId'> & {
    sourceConnectionId?: string;
  },
  isFavorite: boolean,
): Promise<void> {
  await invoke('set_model_favorite', {
    input: {
      providerId: favorite.providerId,
      modelId: favorite.modelId,
      displayName: favorite.displayName,
      isFavorite,
      sourceConnectionId: favorite.sourceConnectionId ?? null,
    },
  });
}

export async function listSavedOutputMessageKeys(): Promise<SavedOutputMessageKey[]> {
  const value: unknown = await invoke('list_saved_output_message_keys');
  if (!Array.isArray(value) || !value.every(isSavedOutputMessageKey)) {
    throw new Error('The saved response list is invalid.');
  }
  return value;
}

export async function listSavedOutputs(offset = 0, limit = 30): Promise<SavedOutputSummary[]> {
  if (!Number.isInteger(offset) || offset < 0 || offset > 100_000 || !Number.isInteger(limit) || limit < 1 || limit > 50) {
    throw new RangeError('The saved response page is invalid.');
  }
  const value: unknown = await invoke('list_saved_outputs', { offset, limit });
  if (!Array.isArray(value) || value.length > limit || !value.every(isSavedOutputSummary)) {
    throw new Error('The saved response page is invalid.');
  }
  return value;
}

export async function listSavedOutputVersions(
  conversationId: string,
  messageId: string,
): Promise<SavedOutputVersionSummary[]> {
  const value: unknown = await invoke('list_saved_output_versions', { conversationId, messageId });
  if (!Array.isArray(value) || value.length === 0 || value.length > 1_000 || !value.every(isSavedOutputVersionSummary)) {
    throw new Error('The saved response version list is invalid.');
  }
  return value;
}

export async function getSavedOutputVersion(
  conversationId: string,
  messageId: string,
  versionNumber: number,
): Promise<SavedOutputVersion> {
  const value: unknown = await invoke('get_saved_output_version', { conversationId, messageId, versionNumber });
  if (!isSavedOutputVersion(value)) throw new Error('The saved response version is invalid.');
  return value;
}

export async function saveSavedOutputVersion(
  conversationId: string,
  messageId: string,
  content: string,
): Promise<SavedOutputVersion> {
  if (content.trim().length === 0 || new TextEncoder().encode(content).byteLength > 2 * 1024 * 1024) {
    throw new RangeError('The saved response version content is invalid.');
  }
  const value: unknown = await invoke('save_saved_output_version', { conversationId, messageId, content });
  if (!isSavedOutputVersion(value)) throw new Error('The saved response version is invalid.');
  return value;
}

export async function selectSavedOutputVersion(
  conversationId: string,
  messageId: string,
  versionNumber: number,
): Promise<void> {
  await invoke('select_saved_output_version', { conversationId, messageId, versionNumber });
}

export async function searchSavedOutputs(input: SavedOutputSearchInput): Promise<SavedOutputSearchMatch[]> {
  const value: unknown = await invoke('search_saved_outputs', { input });
  if (!Array.isArray(value) || value.length > 30 || !value.every(isSavedOutputSearchMatch)) {
    throw new Error('The saved response search results are invalid.');
  }
  return value;
}

export async function setSavedOutput(conversationId: string, messageId: string, isSaved: boolean): Promise<void> {
  await invoke('set_saved_output', { conversationId, messageId, isSaved });
}

export async function getConversation(conversationId: string): Promise<ChatConversation | null> {
  const value: unknown = await invoke('get_conversation', { conversationId });
  if (value === null) return null;
  if (!isConversation(value)) {
    throw new Error('The stored conversation is invalid.');
  }
  return value;
}

export async function getMessages(conversationId: string): Promise<ChatMessage[]> {
  const value: unknown = await invoke('get_messages', { conversationId });
  if (!Array.isArray(value) || !value.every(isMessage)) {
    throw new Error('The stored message list is invalid.');
  }
  return value.map((message) => ({
    ...message,
    ...decodeStoredMessageContent(message.content),
  }));
}

export interface StoredSearchMessageResult {
  id: string;
  conversationId: string;
  role: 'user' | 'assistant';
  content: string;
  createdAt: number | null;
}

export interface SearchFilterOptions {
  roleFilter?: 'user' | 'assistant' | null;
  hasCodeOnly?: boolean | null;
}

export {
  normalizeSearchText,
  type ParsedSearchQuery,
  parseSearchQuery,
} from './chatSearchUtils';

export async function searchStoredMessages(
  query: string,
  limit = 40,
  options?: SearchFilterOptions,
): Promise<StoredSearchMessageResult[]> {
  const value: unknown = await invoke('search_messages', {
    input: {
      query,
      limit,
      roleFilter: options?.roleFilter ?? null,
      hasCodeOnly: options?.hasCodeOnly ?? null,
    },
  });
  if (!Array.isArray(value)) return [];
  return value
    .filter((item): item is Record<string, unknown> => typeof item === 'object' && item !== null)
    .map((item) => ({
      id: String(item.id ?? ''),
      conversationId: String(item.conversationId ?? ''),
      role: item.role === 'assistant' ? 'assistant' : 'user',
      content: String(item.content ?? ''),
      createdAt: typeof item.createdAt === 'number' ? item.createdAt : null,
    }));
}

export async function createConversation(
  id: string,
  title: string,
  route: ChatRoute,
  projectId: string | null = null,
): Promise<void> {
  await invoke('create_conversation', {
    input: {
      id,
      title,
      providerId: route.providerId,
      modelId: route.modelId,
      connectionId: route.connectionId ?? null,
      workspaceId: route.workspaceId ?? null,
      apiKeyConnectionId: route.apiKeyConnectionId ?? null,
      projectId,
    },
  });
}

export async function createConversationBranchFromUserMessage(
  sourceConversationId: string,
  throughUserMessageId: string,
  branchId: string,
  branchTitle: string,
  editedUserMessage: string,
): Promise<{ conversation: ChatConversation; messages: ChatMessage[] }> {
  const sourceConversation = await getConversation(sourceConversationId);
  if (!sourceConversation) throw new Error('The source conversation was not found.');
  const sourceMessages = await getMessages(sourceConversationId);
  const targetIndex = sourceMessages.findIndex((message) => message.id === throughUserMessageId);
  if (targetIndex < 0 || sourceMessages[targetIndex]?.role !== 'user') {
    throw new Error('The branch source message was not found.');
  }
  const targetMessage = sourceMessages[targetIndex];
  if (!targetMessage || (editedUserMessage.trim().length === 0 && targetMessage.attachments.length === 0)) {
    throw new Error('A branched user message must contain text or an attachment.');
  }
  if (!sourceConversation.providerId || !sourceConversation.modelId) {
    throw new Error('The source conversation has no provider route.');
  }
  await createConversation(
    branchId,
    branchTitle,
    {
      providerId: sourceConversation.providerId,
      modelId: sourceConversation.modelId,
      ...(sourceConversation.connectionId ? { connectionId: sourceConversation.connectionId } : {}),
      ...(sourceConversation.workspaceId ? { workspaceId: sourceConversation.workspaceId } : {}),
      ...(sourceConversation.apiKeyConnectionId ? { apiKeyConnectionId: sourceConversation.apiKeyConnectionId } : {}),
    },
    sourceConversation.projectId,
  );

  const copiedMessages: ChatMessage[] = [];
  try {
    for (const [index, sourceMessage] of sourceMessages.slice(0, targetIndex + 1).entries()) {
      const copiedMessage: ChatMessage = {
        ...sourceMessage,
        id: `${branchId}_m${index}`,
        conversationId: branchId,
        content: index === targetIndex ? editedUserMessage : sourceMessage.content,
      };
      const attachments: PendingChatAttachment[] = [];
      for (const attachment of sourceMessage.attachments) {
        const bytes = await readMessageAttachment(sourceConversationId, sourceMessage.id, attachment.id);
        const fileBytes = new ArrayBuffer(bytes.byteLength);
        new Uint8Array(fileBytes).set(bytes);
        attachments.push({
          ...attachment,
          file: new File([fileBytes], attachment.name, { type: attachment.mimeType }),
          bytes,
          previewUrl: null,
        });
      }
      await saveMessage(copiedMessage, attachments);
      copiedMessages.push(copiedMessage);
    }
  } catch (error) {
    try {
      await deleteConversation(branchId);
    } catch {
      // Preserve the original storage error; the branch cleanup is best effort.
    }
    throw error;
  }

  const conversation = await getConversation(branchId);
  if (!conversation) throw new Error('The created conversation branch was not found.');
  return { conversation, messages: copiedMessages };
}

export async function duplicateConversation(
  sourceConversationId: string,
  duplicateId: string,
  duplicateTitle: string,
): Promise<ChatConversation> {
  const sourceConversation = await getConversation(sourceConversationId);
  if (!sourceConversation) throw new Error('The source conversation was not found.');
  if (!sourceConversation.providerId || !sourceConversation.modelId) {
    throw new Error('The source conversation has no provider route.');
  }
  const sourceMessages = await getMessages(sourceConversationId);
  await createConversation(
    duplicateId,
    duplicateTitle,
    {
      providerId: sourceConversation.providerId,
      modelId: sourceConversation.modelId,
      ...(sourceConversation.connectionId ? { connectionId: sourceConversation.connectionId } : {}),
      ...(sourceConversation.workspaceId ? { workspaceId: sourceConversation.workspaceId } : {}),
      ...(sourceConversation.apiKeyConnectionId ? { apiKeyConnectionId: sourceConversation.apiKeyConnectionId } : {}),
    },
    sourceConversation.projectId,
  );

  try {
    for (const [index, sourceMessage] of sourceMessages.entries()) {
      const copiedMessage: ChatMessage = {
        ...sourceMessage,
        id: `${duplicateId}_m${index}`,
        conversationId: duplicateId,
        status: sourceMessage.status === 'streaming' ? 'stopped' : sourceMessage.status,
      };
      const attachments: PendingChatAttachment[] = [];
      for (const attachment of sourceMessage.attachments) {
        const bytes = await readMessageAttachment(sourceConversationId, sourceMessage.id, attachment.id);
        const fileBytes = new ArrayBuffer(bytes.byteLength);
        new Uint8Array(fileBytes).set(bytes);
        attachments.push({
          ...attachment,
          file: new File([fileBytes], attachment.name, { type: attachment.mimeType }),
          bytes,
          previewUrl: null,
        });
      }
      await saveMessage(copiedMessage, attachments);
    }
  } catch (error) {
    try {
      await deleteConversation(duplicateId);
    } catch {
      // Preserve the original storage error; duplicate cleanup is best effort.
    }
    throw error;
  }

  const conversation = await getConversation(duplicateId);
  if (!conversation) throw new Error('The duplicated conversation was not found.');
  return conversation;
}

export async function saveMessage(
  message: ChatMessage,
  attachments: readonly PendingChatAttachment[] = [],
): Promise<void> {
  const metadata = attachments.map(({ id, name, mimeType, sizeBytes, kind }) => ({
    id,
    name,
    mimeType,
    sizeBytes,
    kind,
  }));
  const payload: AttachmentWritePayload[] = attachments.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    mimeType: attachment.mimeType,
    sizeBytes: attachment.sizeBytes,
    kind: attachment.kind,
    dataBase64: encodeBase64(attachment.bytes),
  }));
  await invoke('save_message', {
    input: {
      ...message,
      content: encodeStoredMessageContent(message.content, metadata),
      attachments: payload,
      createdAt: message.createdAt ?? Date.now(),
    },
  });
}

export async function readMessageAttachment(
  conversationId: string,
  messageId: string,
  attachmentId: string,
): Promise<Uint8Array> {
  const value: unknown = await invoke('read_message_attachment', {
    conversationId,
    messageId,
    attachmentId,
  });
  if (typeof value !== 'string') throw new Error('The saved attachment is invalid.');
  return decodeBase64(value);
}

export async function listConversationAttachments(
  offset: number,
  limit: number,
  query: string,
  availability: 'all' | 'available' | 'missing',
): Promise<ConversationAttachmentPage> {
  const value: unknown = await invoke('list_conversation_attachments', {
    offset,
    limit,
    query: query.trim() || null,
    availability,
  });
  if (
    !isObject(value) ||
    !Array.isArray(value.items) ||
    !value.items.every(isConversationAttachmentSummary) ||
    !(value.nextOffset === null || (Number.isSafeInteger(value.nextOffset) && typeof value.nextOffset === 'number'))
  ) {
    throw new Error('The conversation attachment library response is invalid.');
  }
  return { items: value.items, nextOffset: value.nextOffset };
}

export async function getAttachmentTextIndexStatus(): Promise<AttachmentTextIndexStatus> {
  const value: unknown = await invoke('get_attachment_text_index_status');
  if (!isAttachmentTextIndexStatus(value)) throw new Error('The attachment text index status is invalid.');
  return value;
}

export async function rebuildAttachmentTextIndex(): Promise<AttachmentTextIndexStatus> {
  const value: unknown = await invoke('rebuild_attachment_text_index');
  if (!isAttachmentTextIndexStatus(value)) throw new Error('The attachment text index status is invalid.');
  return value;
}

export async function clearAttachmentTextIndex(): Promise<AttachmentTextIndexStatus> {
  const value: unknown = await invoke('clear_attachment_text_index');
  if (!isAttachmentTextIndexStatus(value)) throw new Error('The attachment text index status is invalid.');
  return value;
}

function isConversation(value: unknown): value is ChatConversation {
  if (!isObject(value)) return false;
  return (
    isString(value.id) &&
    isString(value.title) &&
    isString(value.titleSource) &&
    isNullableString(value.providerId) &&
    isNullableString(value.modelId) &&
    isNullableString(value.connectionId) &&
    isNullableString(value.workspaceId) &&
    isNullableString(value.apiKeyConnectionId) &&
    isNullableString(value.projectId) &&
    typeof value.isPinned === 'boolean' &&
    typeof value.isArchived === 'boolean' &&
    typeof value.createdAt === 'number' &&
    typeof value.updatedAt === 'number'
  );
}

function isProject(value: unknown): value is ChatProject {
  if (!isObject(value)) return false;
  return (
    isString(value.id) &&
    isString(value.name) &&
    isString(value.folderPath) &&
    typeof value.createdAt === 'number' &&
    typeof value.updatedAt === 'number'
  );
}

function isMessage(value: unknown): value is Omit<ChatMessage, 'attachments'> {
  if (!isObject(value)) return false;
  return (
    isString(value.id) &&
    isString(value.conversationId) &&
    (value.role === 'user' || value.role === 'assistant') &&
    isString(value.content) &&
    isNullableNumber(value.createdAt) &&
    isNullableNumber(value.outputTokens) &&
    isNullableNumber(value.tokensPerSecond) &&
    isNullableNumber(value.elapsedMicroseconds) &&
    isNullableString(value.providerId) &&
    isNullableString(value.modelId) &&
    isString(value.citationSources) &&
    isString(value.personalMemoryFactIds) &&
    isString(value.reasoningSummaries) &&
    isString(value.toolActivities) &&
    isJsonArray(value.citationSources) &&
    isJsonArray(value.personalMemoryFactIds) &&
    isJsonArray(value.reasoningSummaries) &&
    isJsonArray(value.toolActivities) &&
    isMessageStatus(value.status) &&
    isNullableString(value.failureCode)
  );
}

function isMessageStatus(value: unknown): value is ChatMessageStatus {
  return value === 'streaming' || value === 'completed' || value === 'failed' || value === 'stopped';
}

function isModelFavorite(value: unknown): value is ChatModelFavorite {
  if (!isObject(value)) return false;
  return (
    isString(value.providerId) &&
    isString(value.modelId) &&
    isString(value.displayName) &&
    isNullableString(value.sourceConnectionId)
  );
}

function isSavedOutputMessageKey(value: unknown): value is SavedOutputMessageKey {
  if (!isObject(value)) return false;
  return isString(value.conversationId) && isString(value.messageId);
}

function isSavedOutputSummary(value: unknown): value is SavedOutputSummary {
  return (
    isObject(value) &&
    typeof value.conversationId === 'string' &&
    typeof value.conversationTitle === 'string' &&
    typeof value.messageId === 'string' &&
    typeof value.excerpt === 'string' &&
    typeof value.createdAtUnixMs === 'number' &&
    Number.isSafeInteger(value.createdAtUnixMs) &&
    typeof value.selectedVersion === 'number' &&
    Number.isInteger(value.selectedVersion) &&
    value.selectedVersion > 0 &&
    typeof value.versionCount === 'number' &&
    Number.isInteger(value.versionCount) &&
    value.versionCount > 0
  );
}

function isSavedOutputVersionSummary(value: unknown): value is SavedOutputVersionSummary {
  return (
    isObject(value) &&
    typeof value.versionNumber === 'number' &&
    Number.isInteger(value.versionNumber) &&
    value.versionNumber > 0 &&
    typeof value.createdAtUnixMs === 'number' &&
    Number.isSafeInteger(value.createdAtUnixMs)
  );
}

function isSavedOutputVersion(value: unknown): value is SavedOutputVersion {
  return isObject(value) && isSavedOutputVersionSummary(value) && typeof value.content === 'string';
}

function isSavedOutputSearchMatch(value: unknown): value is SavedOutputSearchMatch {
  return (
    isObject(value) &&
    isString(value.conversationId) &&
    value.conversationId.length > 0 &&
    isString(value.conversationTitle) &&
    value.conversationTitle.length > 0 &&
    typeof value.isConversationTitleMatch === 'boolean' &&
    isString(value.messageId) &&
    value.messageId.length > 0 &&
    (value.role === 'user' || value.role === 'assistant') &&
    isString(value.excerpt) &&
    Array.from(value.excerpt).length <= 4096 &&
    Number.isSafeInteger(value.createdAtUnixMs) &&
    typeof value.createdAtUnixMs === 'number' &&
    value.createdAtUnixMs >= 0 &&
    value.createdAtUnixMs <= 8_640_000_000_000_000 &&
    isNullableString(value.projectId)
  );
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || isString(value);
}

function isNullableNumber(value: unknown): value is number | null {
  return value === null || (typeof value === 'number' && Number.isFinite(value));
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isConversationAttachmentSummary(value: unknown): value is ConversationAttachmentSummary {
  return (
    isObject(value) &&
    isString(value.conversationId) &&
    isString(value.conversationTitle) &&
    isNullableString(value.projectId) &&
    isString(value.messageId) &&
    isString(value.attachmentId) &&
    isString(value.fileName) &&
    isString(value.mimeType) &&
    Number.isSafeInteger(value.sizeBytes) &&
    typeof value.sizeBytes === 'number' &&
    value.sizeBytes > 0 &&
    Number.isSafeInteger(value.createdAt) &&
    typeof value.createdAt === 'number' &&
    value.createdAt >= 0 &&
    typeof value.isAvailable === 'boolean'
  );
}

function isAttachmentTextIndexStatus(value: unknown): value is AttachmentTextIndexStatus {
  return (
    isObject(value) &&
    typeof value.enabled === 'boolean' &&
    Number.isSafeInteger(value.indexedFileCount) &&
    typeof value.indexedFileCount === 'number' &&
    value.indexedFileCount >= 0 &&
    Number.isSafeInteger(value.indexedBytes) &&
    typeof value.indexedBytes === 'number' &&
    value.indexedBytes >= 0 &&
    typeof value.truncated === 'boolean'
  );
}

function isJsonArray(value: string): boolean {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed);
  } catch {
    return false;
  }
}
