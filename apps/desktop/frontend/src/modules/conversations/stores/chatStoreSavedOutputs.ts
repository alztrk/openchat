import type { StoreApi } from 'zustand';

import { listSavedOutputMessageKeys, setSavedOutput } from '@/modules/conversations/data/chatStorage';
import type { ChatState } from './chatStore';

type SetState = StoreApi<ChatState>['setState'];
type GetState = StoreApi<ChatState>['getState'];

export function createRefreshSavedOutputMessageKeys(set: SetState): ChatState['refreshSavedOutputMessageKeys'] {
  return async () => {
    try {
      set({ savedOutputMessageKeys: await listSavedOutputMessageKeys(), savedOutputError: false });
    } catch {
      set({ savedOutputError: true });
    }
  };
}

export function createToggleSavedOutputAction(set: SetState, get: GetState): ChatState['toggleSavedOutput'] {
  return async (messageId) => {
    const current = get();
    const conversationId = current.activeConversationId;
    const message = current.messages.find((item) => item.id === messageId);
    if (
      !conversationId ||
      !message ||
      message.conversationId !== conversationId ||
      message.role !== 'assistant' ||
      message.status !== 'completed' ||
      message.content.trim().length === 0 ||
      current.pendingSavedOutputMessageIds.includes(messageId)
    )
      return false;

    const isSaved = current.savedOutputMessageKeys.some(
      (key) => key.conversationId === conversationId && key.messageId === messageId,
    );
    set((state) => ({
      pendingSavedOutputMessageIds: [...state.pendingSavedOutputMessageIds, messageId],
      savedOutputError: false,
    }));
    try {
      await setSavedOutput(conversationId, messageId, !isSaved);
      set((state) => ({
        savedOutputMessageKeys: isSaved
          ? state.savedOutputMessageKeys.filter(
              (key) => key.conversationId !== conversationId || key.messageId !== messageId,
            )
          : [...state.savedOutputMessageKeys, { conversationId, messageId }],
        savedOutputError: false,
      }));
      return true;
    } catch {
      set({ savedOutputError: true });
      return false;
    } finally {
      set((state) => ({
        pendingSavedOutputMessageIds: state.pendingSavedOutputMessageIds.filter((id) => id !== messageId),
      }));
    }
  };
}
