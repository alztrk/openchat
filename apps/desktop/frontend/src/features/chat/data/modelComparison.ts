export const maximumComparisonModels = 3;

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

export function isComparisonSelectionKeyboardShortcut(shiftKey: boolean, key: string): boolean {
  return shiftKey && (key === 'Enter' || key === ' ' || key === 'Spacebar');
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
