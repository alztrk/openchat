import { useChatStore } from '@/modules/conversations/stores/chatStore';

export function useChatSession() {
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const activeConversation = useChatStore((state) =>
    state.conversations.find((conversation) => conversation.id === state.activeConversationId),
  );
  const messages = useChatStore((state) => state.messages);
  const isLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const errorCode = useChatStore((state) => state.errorCode);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const initialize = useChatStore((state) => state.initialize);

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
