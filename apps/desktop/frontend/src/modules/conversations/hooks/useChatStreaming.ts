import { useChatStore } from '@/modules/conversations/stores/chatStore';

export function useChatStreaming() {
  const isSending = useChatStore((state) => state.isSending);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const stopResponse = useChatStore((state) => state.stopResponse);

  return { isSending, sendMessage, stopResponse };
}
