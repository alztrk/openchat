import { useChatStore } from '@features/chat/state';

export function useChatStreaming() {
  const isSending = useChatStore((state) => state.isSending);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const stopResponse = useChatStore((state) => state.stopResponse);

  return { isSending, sendMessage, stopResponse };
}
