import { useMemo } from 'react';

import { useChatStore } from '@features/chat/state';
import type { ChatConversation } from '@features/chat/data';

export function useChatSession() {
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const messages = useChatStore((state) => state.messages);
  const isLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const errorCode = useChatStore((state) => state.errorCode);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const initialize = useChatStore((state) => state.initialize);

  const conversationById = useMemo(() => {
    const map = new Map<string, ChatConversation>();
    for (const conversation of conversations) {
      map.set(conversation.id, conversation);
    }
    return map;
  }, [conversations]);

  const activeConversation = conversationById.get(activeConversationId ?? '') ?? null;

  return {
    activeConversation,
    activeConversationId,
    conversations,
    errorCode,
    initialize,
    isLoadingMessages,
    messages,
    serviceStatus,
  };
}
