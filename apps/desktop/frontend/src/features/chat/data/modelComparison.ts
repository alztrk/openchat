import type { ChatMessage } from '@entities/conversation';
import type { ChatModel } from '@entities/model';

export const maximumComparisonModels = 3;

export function comparisonResponseModelLabel(
  message: Pick<ChatMessage, 'providerId' | 'modelId'>,
  models: readonly Pick<ChatModel, 'providerId' | 'modelId' | 'displayName'>[],
  fallback: string | null = null,
): string | null {
  const model = models.find(
    (candidate) => candidate.providerId === message.providerId && candidate.modelId === message.modelId,
  );
  return model?.displayName.trim() || message.modelId?.trim() || fallback?.trim() || null;
}

export function hasDistinctModelComparisonResponses(
  messages: readonly Pick<ChatMessage, 'role' | 'providerId' | 'modelId'>[],
): boolean {
  if (messages.length < 2 || messages.some((message) => message.role !== 'assistant')) return false;
  const modelRoutes = new Set(messages.map((message) => `${message.providerId ?? ''}\u0000${message.modelId ?? ''}`));
  return modelRoutes.size > 1;
}

export type ComparisonHistoryLoadState = 'loading' | 'ready' | 'failed';

export function getComparisonHistoryLoadState(
  conversationIds: readonly string[],
  readyConversationIds: ReadonlySet<string>,
  failedConversationIds: ReadonlySet<string>,
): ComparisonHistoryLoadState {
  if (conversationIds.length === 0) return 'loading';
  if (conversationIds.some((conversationId) => failedConversationIds.has(conversationId))) return 'failed';
  return conversationIds.every((conversationId) => readyConversationIds.has(conversationId)) ? 'ready' : 'loading';
}

export function toggleComparisonModelSelection(selectedKeys: readonly string[], modelKey: string): string[] {
  if (selectedKeys.includes(modelKey)) {
    return selectedKeys.filter((key) => key !== modelKey);
  }
  if (selectedKeys.length >= maximumComparisonModels) return [...selectedKeys];
  return [...selectedKeys, modelKey];
}

export function selectComparisonModelFromPicker(selectedKeys: readonly string[], modelKey: string): string[] {
  const nextKeys = toggleComparisonModelSelection(selectedKeys, modelKey);
  return nextKeys.length === 0 ? [...selectedKeys] : nextKeys;
}

export function hasValidComparisonModelSelection(selectedKeys: readonly string[]): boolean {
  return (
    selectedKeys.length >= 2 &&
    selectedKeys.length <= maximumComparisonModels &&
    new Set(selectedKeys).size === selectedKeys.length
  );
}

export function canContinueComparisonConversation(
  phase: 'selecting' | 'loading' | 'sending' | 'active' | 'failed',
  conversationId: string,
  completedConversationIds: ReadonlySet<string>,
): boolean {
  return phase === 'active' && completedConversationIds.has(conversationId);
}
