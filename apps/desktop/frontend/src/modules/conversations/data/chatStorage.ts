import type { ChatRoute } from '@/modules/providers/data/routes';
import { type AssistantAgentSnapshot, isAssistantAgentSnapshot } from '@/modules/settings/data/assistantProfiles';
import { localServiceClient } from '@/platform/local-service';
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
  agent: AssistantAgentSnapshot | null;
  isPinned: boolean;
  isArchived: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface ChatProject {
  id: string;
  name: string;
  folderPath: string;
  workspaceRoots: string[];
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
  const response = await localServiceClient.call('storage.chat.conversations.list');
  if (!Array.isArray(response.conversations) || !response.conversations.every(isConversation)) {
    throw new Error('The stored conversation list is invalid.');
  }
  return response.conversations;
}

export async function listProjects(): Promise<ChatProject[]> {
  const response = await localServiceClient.call('storage.chat.projects.list');
  if (!Array.isArray(response.projects) || !response.projects.every(isProject)) {
    throw new Error('The stored project list is invalid.');
  }
  return response.projects;
}

/** Persists a project from a one-time native folder selection token. */
export async function createProject(id: string, name: string, folderSelectionId: string): Promise<void> {
  const folderPath: unknown = await invoke('consume_project_folder_selection_command', {
    selectionId: folderSelectionId,
  });
  if (typeof folderPath !== 'string' || folderPath.trim().length === 0) {
    throw new Error('The selected project folder is invalid.');
  }
  await localServiceClient.call('storage.chat.projects.create', {
    input: { id, name, folderPath },
  });
}

export async function addProjectWorkspaceRoot(projectId: string, folderSelectionId: string): Promise<void> {
  const folderPath: unknown = await invoke('consume_project_folder_selection_command', {
    selectionId: folderSelectionId,
  });
  if (typeof folderPath !== 'string' || folderPath.trim().length === 0) {
    throw new Error('The selected workspace folder is invalid.');
  }
  await localServiceClient.call('storage.chat.projects.workspace.add', {
    input: { projectId, folderPath },
  });
}

export async function removeProjectWorkspaceRoot(projectId: string, rootPath: string): Promise<void> {
  expectDeleted(
    await localServiceClient.call('storage.chat.projects.workspace.remove', {
      input: { projectId, rootPath },
    }),
  );
}

export async function renameConversation(conversationId: string, title: string): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.rename', {
      input: { conversationId, title },
    }),
  );
}

export async function setConversationRoute(conversationId: string, route: ChatRoute): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.route.set', {
      input: {
        conversationId,
        providerId: route.providerId,
        modelId: route.modelId,
        connectionId: route.connectionId ?? null,
        workspaceId: route.workspaceId ?? null,
        apiKeyConnectionId: route.apiKeyConnectionId ?? null,
      },
    }),
  );
}

export async function setConversationPinned(conversationId: string, isPinned: boolean): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.pinned.set', {
      input: { conversationId, isPinned },
    }),
  );
}

export async function moveConversations(conversationIds: string[], projectId: string | null): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.project.set', {
      input: { conversationIds, projectId },
    }),
  );
}

export async function setConversationsArchived(conversationIds: string[], isArchived: boolean): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.archived.set', {
      input: { conversationIds, isArchived },
    }),
  );
}

export async function deleteConversation(conversationId: string): Promise<void> {
  expectDeleted(
    await localServiceClient.call('storage.chat.conversations.delete', {
      input: { conversationId },
    }),
  );
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
  const response = await localServiceClient.call('storage.chat.responses.has_completed');
  if (typeof response.completed !== 'boolean') {
    throw new Error('The completed response status is invalid.');
  }
  return response.completed;
}

export async function clearConversationHistory(): Promise<void> {
  const response = await localServiceClient.call('storage.chat.conversations.clear');
  if (response.cleared !== true) {
    throw new Error('The conversation history clear result is invalid.');
  }
}

export async function listModelFavorites(): Promise<ChatModelFavorite[]> {
  const response = await localServiceClient.call('storage.chat.favorites.list');
  if (!Array.isArray(response.favorites) || !response.favorites.every(isModelFavorite)) {
    throw new Error('The stored model favorite list is invalid.');
  }
  return response.favorites;
}

export async function setModelFavorite(
  favorite: Omit<ChatModelFavorite, 'sourceConnectionId'> & {
    sourceConnectionId?: string;
  },
  isFavorite: boolean,
): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.favorites.set', {
      input: {
        providerId: favorite.providerId,
        modelId: favorite.modelId,
        displayName: favorite.displayName,
        isFavorite,
        sourceConnectionId: favorite.sourceConnectionId ?? null,
      },
    }),
  );
}

export async function getConversation(conversationId: string): Promise<ChatConversation | null> {
  const response = await localServiceClient.call('storage.chat.conversations.get', { conversationId });
  if (response.conversation === null) return null;
  if (!isConversation(response.conversation)) {
    throw new Error('The stored conversation is invalid.');
  }
  return response.conversation;
}

export async function getMessages(conversationId: string): Promise<ChatMessage[]> {
  let timeoutId: number | undefined;
  let value: unknown;
  try {
    value = await Promise.race([
      localServiceClient.call('storage.chat.messages.list', { conversationId }, 15_000),
      new Promise<never>((_, reject) => {
        timeoutId = window.setTimeout(() => reject(new Error('Loading conversation messages timed out.')), 15_000);
      }),
    ]);
  } finally {
    if (timeoutId !== undefined) window.clearTimeout(timeoutId);
  }
  if (!isObject(value) || !Array.isArray(value.messages) || !value.messages.every(isMessage)) {
    throw new Error('The stored message list is invalid.');
  }
  return value.messages.map((message) => ({
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
  const response = await localServiceClient.call('storage.chat.messages.search', {
    input: {
      query,
      limit,
      roleFilter: options?.roleFilter ?? null,
      hasCodeOnly: options?.hasCodeOnly ?? null,
    },
  });
  if (!Array.isArray(response.messages) || !response.messages.every(isStoredSearchMessage)) {
    throw new Error('The stored message search response is invalid.');
  }
  return response.messages;
}

export async function createConversation(
  id: string,
  title: string,
  route: ChatRoute,
  projectId: string | null = null,
  agent: AssistantAgentSnapshot | null = null,
): Promise<void> {
  expectSaved(
    await localServiceClient.call('storage.chat.conversations.create', {
      input: {
        id,
        title,
        providerId: route.providerId,
        modelId: route.modelId,
        connectionId: route.connectionId ?? null,
        workspaceId: route.workspaceId ?? null,
        apiKeyConnectionId: route.apiKeyConnectionId ?? null,
        projectId,
        agent,
      },
    }),
  );
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
    sourceConversation.agent,
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
    sourceConversation.agent,
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
  expectSaved(
    await localServiceClient.call('storage.chat.messages.save', {
      input: {
        ...message,
        content: encodeStoredMessageContent(message.content, metadata),
        attachments: payload,
        createdAt: message.createdAt ?? Date.now(),
      },
    }),
  );
}

export async function readMessageAttachment(
  conversationId: string,
  messageId: string,
  attachmentId: string,
): Promise<Uint8Array> {
  const response = await localServiceClient.call('storage.chat.attachments.read', {
    conversationId,
    messageId,
    attachmentId,
  });
  if (typeof response.dataBase64 !== 'string') throw new Error('The saved attachment is invalid.');
  return decodeBase64(response.dataBase64);
}

export async function listConversationAttachments(
  offset: number,
  limit: number,
  query: string,
  availability: 'all' | 'available' | 'missing',
): Promise<ConversationAttachmentPage> {
  const value: unknown = await localServiceClient.call('storage.chat.attachments.list', {
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
  const value: unknown = await localServiceClient.call('storage.chat.attachments.text_index.status');
  if (!isAttachmentTextIndexStatus(value)) throw new Error('The attachment text index status is invalid.');
  return value;
}

export async function rebuildAttachmentTextIndex(): Promise<AttachmentTextIndexStatus> {
  const value: unknown = await localServiceClient.call('storage.chat.attachments.text_index.rebuild');
  if (!isAttachmentTextIndexStatus(value)) throw new Error('The attachment text index status is invalid.');
  return value;
}

export async function clearAttachmentTextIndex(): Promise<AttachmentTextIndexStatus> {
  const value: unknown = await localServiceClient.call('storage.chat.attachments.text_index.clear');
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
    (value.agent === null || isAssistantAgentSnapshot(value.agent)) &&
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
    Array.isArray(value.workspaceRoots) &&
    value.workspaceRoots.every(isString) &&
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

function isStoredSearchMessage(value: unknown): value is StoredSearchMessageResult {
  if (!isObject(value)) return false;
  return (
    isString(value.id) &&
    isString(value.conversationId) &&
    (value.role === 'user' || value.role === 'assistant') &&
    isString(value.content) &&
    isNullableNumber(value.createdAt)
  );
}

function expectSaved(response: Record<string, unknown>): void {
  if (response.saved !== true) {
    throw new Error('The local service save result is invalid.');
  }
}

function expectDeleted(response: Record<string, unknown>): void {
  if (response.deleted !== true) {
    throw new Error('The local service delete result is invalid.');
  }
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
