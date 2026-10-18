import { createConversationBranchFromUserMessage } from '@/modules/conversations/data/chatStorage';
import type { ChatState } from './chatStore';
import { isSameRoute } from './chatStoreUtils';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

export function createBranchesActions(set: StoreSet, get: StoreGet) {
  return {
    async retryMessage(messageId: string): Promise<boolean> {
      const current = get();
      const messageIndex = current.messages.findIndex((message) => message.id === messageId);
      const message = messageIndex >= 0 ? current.messages[messageIndex] : null;
      const previousMessage = messageIndex > 0 ? current.messages[messageIndex - 1] : null;
      const isLatest = messageIndex === current.messages.length - 1;
      let responseVersionCount = 0;
      if (message?.role === 'assistant') {
        for (let index = messageIndex; index >= 0 && current.messages[index]?.role === 'assistant'; index -= 1) {
          responseVersionCount += 1;
        }
      }
      if (
        !current.activeConversationId ||
        !message ||
        message.role !== 'assistant' ||
        message.status === 'streaming' ||
        !isLatest ||
        previousMessage?.role !== 'user' ||
        responseVersionCount > 1
      ) {
        set({ errorCode: 'responseRetryUnavailable' });
        return false;
      }
      return get().sendMessage('', [], undefined, undefined, {
        excludedAssistantMessageId: messageId,
        userMessageAlreadySaved: true,
      });
    },

    async branchFromUserMessage(messageId: string, editedContent: string, branchTitle: string): Promise<boolean> {
      const current = get();
      const sourceConversationId = current.activeConversationId;
      const sourceMessage = current.messages.find((message) => message.id === messageId);
      if (
        !sourceConversationId ||
        !sourceMessage ||
        sourceMessage.role !== 'user' ||
        current.isSending ||
        current.isCreatingMessageBranch ||
        current.compactingConversationId === sourceConversationId ||
        (editedContent.trim().length === 0 && sourceMessage.attachments.length === 0)
      ) {
        set({ errorCode: 'conversationBranchCreateFailed' });
        return false;
      }
      set({ errorCode: null, isCreatingMessageBranch: true });
      try {
        const branch = await createConversationBranchFromUserMessage(
          sourceConversationId,
          messageId,
          crypto.randomUUID().replaceAll('-', ''),
          branchTitle,
          editedContent,
        );
        const branchModel = current.models.find((model) => isSameRoute(model, branch.conversation));
        set((state) => ({
          conversations: [
            branch.conversation,
            ...state.conversations.filter((conversation) => conversation.id !== branch.conversation.id),
          ],
          activeConversationId: branch.conversation.id,
          pendingProjectId: null,
          messages: branch.messages,
          selectedModelKey: branchModel?.key ?? state.selectedModelKey,
          selectedReasoningEffort: null,
          activeGoal: null,
          goalStatus: 'ready',
          searchTargetMessageId: null,
          searchHighlightMessageId: null,
          isLoadingMessages: false,
          isCreatingMessageBranch: false,
        }));
        return await get().sendMessage(editedContent, [], undefined, undefined, {
          userMessageAlreadySaved: true,
        });
      } catch {
        set({ errorCode: 'conversationBranchCreateFailed', isCreatingMessageBranch: false });
        return false;
      } finally {
        set({ isCreatingMessageBranch: false });
      }
    },

    async stopResponse(): Promise<void> {
      const operation = get().activeOperation;
      if (!operation) return;
      await operation.cancel();
    },

    dismissError(): void {
      set({ errorCode: null });
    },
  };
}
