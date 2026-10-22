import {
  type ChatMessage,
  clearConversationHistory as clearConversationHistoryStorage,
  getConversation,
  getMessages,
  listConversations,
  listProjects,
  renameConversation as renameConversationStorage,
} from '@/modules/conversations/data/chatStorage';
import { readProjectDefaultModelPreference } from '@/modules/projects/data/projectPreferences';
import { useAssistantProfilesStore } from '@/modules/settings/stores/assistantProfilesStore';
import {
  type DefaultModelPreference,
  formatModelRouteKey,
  modelRouteKey,
  readLastSelectedModelPreference,
} from '@/modules/providers/data/modelPreferences';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';
import type { ChatState } from './chatStore';
import { isSameRoute, upsertMessage } from './chatStoreUtils';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

let messageLoadGeneration = 0;

export function createConversationsActions(set: StoreSet, get: StoreGet) {
  return {
    async refreshConversations(): Promise<void> {
      const conversations = await listConversations();
      const currentConversationId = get().activeConversationId;
      if (
        currentConversationId !== null &&
        !conversations.some((conversation) => conversation.id === currentConversationId)
      ) {
        messageLoadGeneration += 1;
        set({
          conversations,
          activeConversationId: null,
          selectedReasoningEffort: null,
          activeGoal: null,
          goalStatus: 'ready',
          isChangingGoal: false,
          messages: [],
          searchTargetMessageId: null,
          searchHighlightMessageId: null,
          searchHighlightQuery: null,
          isLoadingMessages: false,
        });
        await get().refreshSavedOutputMessageKeys();
        return;
      }
      set({ conversations });
      await get().refreshSavedOutputMessageKeys();
    },

    async renameConversation(conversationId: string, title: string): Promise<void> {
      await renameConversationStorage(conversationId, title);
      set((state) => ({
        conversations: state.conversations.map((conversation) =>
          conversation.id === conversationId ? { ...conversation, title, titleSource: 'manual' } : conversation,
        ),
      }));
    },

    applyTitleUpdate(conversationId: string, title: string): void {
      set((state) => {
        const conversation = state.conversations.find((c) => c.id === conversationId);
        if (!conversation || conversation.titleSource === 'manual') {
          return state;
        }
        return {
          conversations: state.conversations.map((c) =>
            c.id === conversationId ? { ...c, title, titleSource: 'automatic' } : c,
          ),
        };
      });
    },

    async refreshProjects(): Promise<void> {
      set({ projectsStatus: 'loading' });
      try {
        const projects = await listProjects();
        set({ projects, projectsStatus: 'ready' });
      } catch (error) {
        set({ projectsStatus: 'error' });
        throw error;
      }
    },

    async clearConversationHistory(): Promise<void> {
      if (get().isSending) {
        throw new LocalServiceError(
          'conversation_archive_busy',
          'Stop the active response before clearing chat history.',
        );
      }
      if (get().isUpdatingConversationModel) {
        throw new LocalServiceError(
          'conversation_archive_busy',
          'Wait for the current model change before clearing chat history.',
        );
      }
      await localServiceClient.call('chat.file_changes.delete_all');
      await clearConversationHistoryStorage();
      messageLoadGeneration += 1;
      set({
        conversations: [],
        activeConversationId: null,
        isUpdatingConversationModel: false,
        selectedReasoningEffort: null,
        activeGoal: null,
        goalStatus: 'ready',
        isChangingGoal: false,
        pendingProjectId: null,
        messages: [],
        personalMemoryExcludedForNewConversation: false,
        searchTargetMessageId: null,
        searchHighlightMessageId: null,
        searchHighlightQuery: null,
        isLoadingMessages: false,
        errorCode: null,
      });
    },

    setPendingQuestionSendBlock(conversationId: string | null, blocked: boolean): void {
      const current = get().pendingQuestionSendBlock;
      if (conversationId === null && current === null) return;
      if (current?.conversationId === conversationId && current.blocked === blocked) return;
      set({
        pendingQuestionSendBlock: conversationId === null ? null : { conversationId, blocked },
      });
    },

    beginConversationCompaction(conversationId: string): boolean {
      const current = get();
      const pendingQuestionBlocksSend =
        current.pendingQuestionSendBlock?.conversationId === conversationId && current.pendingQuestionSendBlock.blocked;
      if (
        current.activeConversationId !== conversationId ||
        current.isSending ||
        current.isUpdatingConversationModel ||
        current.isCreatingMessageBranch ||
        current.compactingConversationId !== null ||
        pendingQuestionBlocksSend
      )
        return false;
      set({ compactingConversationId: conversationId });
      return true;
    },

    finishConversationCompaction(conversationId: string): void {
      if (get().compactingConversationId === conversationId) {
        set({ compactingConversationId: null });
      }
    },

    async selectConversation(
      id: string,
      targetMessageId: string | null = null,
      targetQuery: string | null = null,
    ): Promise<void> {
      if (get().temporaryChat) {
        set({ errorCode: get().isSending ? 'temporaryChatStopBeforeLeaving' : 'temporaryChatExitRequired' });
        return;
      }
      const generation = ++messageLoadGeneration;
      set({
        activeConversationId: id,
        selectedReasoningEffort: null,
        activeGoal: null,
        goalStatus: 'loading',
        isChangingGoal: false,
        pendingProjectId: null,
        messages: [],
        isLoadingMessages: true,
        errorCode: null,
        searchTargetMessageId: targetMessageId,
        searchHighlightMessageId: null,
        searchHighlightQuery: targetQuery,
        selectedModelKey: null,
      });

      let conversation = get().conversations.find((item) => item.id === id);
      if (!conversation) {
        try {
          const restoredConversation = await getConversation(id);
          if (generation !== messageLoadGeneration) return;
          if (!restoredConversation) {
            set({
              isLoadingMessages: false,
              goalStatus: 'ready',
              searchTargetMessageId: null,
              errorCode: targetMessageId ? 'searchMessageUnavailable' : 'messageHistoryLoadFailed',
            });
            return;
          }
          conversation = restoredConversation;
          set((current) => ({
            conversations: [restoredConversation, ...current.conversations.filter((item) => item.id !== id)],
          }));
        } catch {
          if (generation !== messageLoadGeneration) return;
          set({
            isLoadingMessages: false,
            goalStatus: 'ready',
            searchTargetMessageId: null,
            errorCode: targetMessageId ? 'searchMessageUnavailable' : 'messageHistoryLoadFailed',
          });
          return;
        }
      }

      const selectedConversation = conversation;
      if (!selectedConversation) return;
      set({
        selectedModelKey: get().models.find((model) => isSameRoute(model, selectedConversation))?.key ?? null,
      });
      try {
        const messages = await getMessages(id);
        if (generation !== messageLoadGeneration) return;
        const targetWasFound = targetMessageId === null || messages.some((message) => message.id === targetMessageId);
        set({
          messages,
          isLoadingMessages: false,
          searchTargetMessageId: targetWasFound ? targetMessageId : null,
          errorCode: targetWasFound ? null : 'searchMessageUnavailable',
        });
        void get().refreshActiveGoal(id);
      } catch {
        if (generation !== messageLoadGeneration) return;
        set({
          isLoadingMessages: false,
          goalStatus: 'ready',
          searchTargetMessageId: null,
          errorCode: targetMessageId ? 'searchMessageUnavailable' : 'messageHistoryLoadFailed',
        });
      }
    },

    updateMessage(message: ChatMessage): void {
      if (get().activeConversationId !== message.conversationId) return;
      set((current) => ({ messages: upsertMessage(current.messages, message) }));
    },

    clearSearchTarget(found: boolean): void {
      const targetMessageId = get().searchTargetMessageId;
      if (!targetMessageId) return;
      set({
        searchTargetMessageId: null,
        searchHighlightMessageId: found ? targetMessageId : null,
        searchHighlightQuery: found ? get().searchHighlightQuery : null,
        errorCode: found ? null : 'searchMessageUnavailable',
      });
    },

    setPendingProjectId(projectId: string | null): void {
      const current = get();
      if (current.activeConversationId !== null || current.isSending) return;
      if (projectId !== null && !current.projects.some((project) => project.id === projectId)) return;
      set({ pendingProjectId: projectId });
    },

    startNewChat(projectId: string | null = null): void {
      messageLoadGeneration++;
      const current = get();
      if (current.temporaryChat) {
        set({ errorCode: current.isSending ? 'temporaryChatStopBeforeLeaving' : 'temporaryChatExitRequired' });
        return;
      }
      let projectPreference: DefaultModelPreference | null = null;
      let projectPreferenceInvalid = false;
      if (projectId !== null) {
        try {
          projectPreference = readProjectDefaultModelPreference(projectId);
        } catch {
          projectPreferenceInvalid = true;
        }
      }
      let lastSelectedPreference: DefaultModelPreference | null = null;
      try {
        lastSelectedPreference = readLastSelectedModelPreference();
      } catch (cause) {
        globalThis.reportError(new Error('The last selected model preference could not be read.', { cause }));
      }
      const profilesStore = useAssistantProfilesStore.getState();
      if (profilesStore.loadState !== 'ready') profilesStore.initialize();
      const profilePreference = useAssistantProfilesStore.getState().getActiveProfile()?.preferredModel ?? null;
      const defaultPreference = projectPreferenceInvalid
        ? null
        : (projectPreference ?? profilePreference ?? lastSelectedPreference ?? current.defaultModel);
      const defaultModel = defaultPreference
        ? current.models.find(
            (model) =>
              modelRouteKey(model) === formatModelRouteKey(defaultPreference) &&
              !current.hiddenModelKeys.includes(modelRouteKey(model)),
          )
        : undefined;
      set({
        activeConversationId: null,
        temporaryChat: null,
        selectedReasoningEffort: null,
        activeGoal: null,
        goalStatus: 'ready',
        isChangingGoal: false,
        pendingProjectId: projectId,
        messages: [],
        personalMemoryExcludedForNewConversation: false,
        searchTargetMessageId: null,
        searchHighlightMessageId: null,
        searchHighlightQuery: null,
        isLoadingMessages: false,
        selectedModelKey: defaultModel?.key ?? null,
        errorCode: projectPreferenceInvalid ? 'projectDefaultModelLoadFailed' : null,
      });
    },
  };
}
