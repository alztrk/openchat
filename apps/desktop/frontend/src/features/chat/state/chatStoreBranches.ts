import {
  type ChatMessage,
  findLeafForMessage,
  getActiveBranchMessages,
  saveMessage,
  setActiveLeafMessageId,
} from '@features/chat/data';
import type { ChatState } from './chatStore';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

export function createBranchesActions(set: StoreSet, get: StoreGet) {
  return {
    async retryMessage(messageId: string): Promise<boolean> {
      const current = get();
      const activeConversation = current.conversations.find((c) => c.id === current.activeConversationId);
      const branchMessages = getActiveBranchMessages(
        current.messages,
        activeConversation?.activeLeafMessageId ?? null,
      );
      const isLatest = branchMessages.at(-1)?.id === messageId;
      const message = current.messages.find((m) => m.id === messageId);
      if (
        !current.activeConversationId ||
        !message ||
        message.role !== 'assistant' ||
        message.status === 'streaming' ||
        !isLatest
      ) {
        set({ errorCode: 'responseRetryUnavailable' });
        return false;
      }
      return get().sendMessage('', [], undefined, undefined, {
        excludedAssistantMessageId: messageId,
        userMessageAlreadySaved: true,
      });
    },

    async branchFromUserMessage(
      messageId: string,
      editedContent: string,
      _branchTitle: string,
    ): Promise<boolean> {
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
      set({ errorCode: null, errorMessage: null, opencodeRetryAtUnixMs: null, isCreatingMessageBranch: true });
      try {
        const newUserMessage: ChatMessage = {
          id: crypto.randomUUID().replaceAll('-', ''),
          conversationId: sourceConversationId,
          role: 'user',
          content: editedContent,
          attachments: sourceMessage.attachments,
          createdAt: Date.now(),
          outputTokens: null,
          tokensPerSecond: null,
          elapsedMicroseconds: null,
          providerId: sourceMessage.providerId,
          modelId: sourceMessage.modelId,
          citationSources: '[]',
          personalMemoryFactIds: sourceMessage.personalMemoryFactIds ?? '[]',
          reasoningSummaries: '[]',
          toolActivities: '[]',
          status: 'completed',
          failureCode: null,
          parentMessageId: sourceMessage.parentMessageId ?? null,
        };

        await saveMessage(newUserMessage);

        set((state) => ({
          messages: [...state.messages, newUserMessage],
          conversations: state.conversations.map((conversation) =>
            conversation.id === sourceConversationId
              ? { ...conversation, activeLeafMessageId: newUserMessage.id }
              : conversation,
          ),
          isCreatingMessageBranch: false,
        }));

        return await get().sendMessage(editedContent, [], undefined, undefined, {
          userMessageAlreadySaved: true,
        });
      } catch {
        set({
          errorCode: 'conversationBranchCreateFailed',
          errorMessage: null,
          opencodeRetryAtUnixMs: null,
          isCreatingMessageBranch: false,
        });
        return false;
      } finally {
        set({ isCreatingMessageBranch: false });
      }
    },

    async switchMessageBranch(targetMessageId: string): Promise<void> {
      const state = get();
      const conversationId = state.activeConversationId;
      if (!conversationId) return;

      const targetLeafId = findLeafForMessage(state.messages, targetMessageId);
      try {
        await setActiveLeafMessageId(conversationId, targetLeafId);
      } catch {
        // Best effort active leaf persistence
      }

      set((current) => ({
        conversations: current.conversations.map((c) =>
          c.id === conversationId ? { ...c, activeLeafMessageId: targetLeafId } : c,
        ),
      }));
    },

    async stopResponse(conversationId?: string | null): Promise<void> {
      const state = get();
      const targetConversationId = conversationId === undefined ? state.activeConversationId : conversationId;
      if (conversationId !== undefined && targetConversationId === null) return;
      const comparisonOperations = targetConversationId
        ? state.comparisonOperationsByConversation[targetConversationId] ?? []
        : [];
      const operation = targetConversationId
        ? state.activeOperationsByConversation[targetConversationId] ??
          (state.sendingConversationIds.length <= 1 ? state.activeOperation : undefined)
        : state.activeOperation;
      const operations = comparisonOperations.length > 0 ? comparisonOperations : operation ? [operation] : [];
      if (operations.length === 0) return;
      await Promise.all(operations.map((activeOperation) => activeOperation.cancel()));
    },

    dismissError(): void {
      set({ errorCode: null, errorMessage: null, opencodeRetryAtUnixMs: null });
    },
  };
}
