import {
  type ChatConversation,
  type ChatMessage,
  clearConversationHistory as clearConversationHistoryStorage,
  clearConversationUnread as clearConversationUnreadStorage,
  getConversation,
  getMessagePage,
  getMessages,
  listConversations,
  listProjects,
  renameConversation as renameConversationStorage,
} from '@features/chat/data';
import { readProjectDefaultModelPreference } from '@entities/project/projectPreferences';
import {
  type DefaultModelPreference,
  formatModelRouteKey,
  modelRouteKey,
  readLastSelectedModelPreference,
} from '@entities/model/modelPreferences';
import { readAssistantExecutionPreferences } from '@shared/lib/preferences/assistantExecutionPreferences';
import {
  type AssistantAgentSnapshot,
  readAssistantProfiles,
  snapshotAssistantProfile,
} from '@shared/lib/preferences/assistantProfiles';
import { LocalServiceError, localServiceClient } from '@shared/lib/platform/local-service';
import type { ChatState } from './chatStore';
import { isSameRoute, upsertMessage } from './chatStoreUtils';
import { isAppWindowVisibleAndFocused } from '@shared/lib/platform/windowFocus';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

let messageLoadGeneration = 0;

function mergeById<T extends { id: string }>(first: readonly T[], second: readonly T[]): T[] {
  const merged = [...first];
  const ids = new Set(first.map((item) => item.id));
  for (const item of second) {
    if (ids.has(item.id)) continue;
    ids.add(item.id);
    merged.push(item);
  }
  return merged;
}

export function createConversationsActions(set: StoreSet, get: StoreGet) {
  const beginNewChat = (
    projectId: string | null,
    agent: AssistantAgentSnapshot | null,
    preserveSelectedModel: boolean,
  ): void => {
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
    let selectedAgent = agent;
    let assistantProfileError: 'assistantPreferencesLoadFailed' | 'assistantProfileDefaultUnavailable' | null = null;
    if (selectedAgent === null) {
      try {
        const defaultProfileId = readAssistantExecutionPreferences().defaultProfileId;
        if (defaultProfileId !== null) {
          const profile = readAssistantProfiles().profiles.find((item) => item.id === defaultProfileId);
          if (profile) selectedAgent = snapshotAssistantProfile(profile);
          else assistantProfileError = 'assistantProfileDefaultUnavailable';
        }
      } catch {
        assistantProfileError = 'assistantPreferencesLoadFailed';
      }
    }
    const preservedModel = preserveSelectedModel
      ? current.models.find(
          (model) => model.key === current.selectedModelKey && !current.hiddenModelKeys.includes(modelRouteKey(model)),
        )
      : undefined;
    const defaultPreference = projectPreferenceInvalid
      ? null
      : (projectPreference ?? lastSelectedPreference ?? current.defaultModel);
    const defaultModel = defaultPreference
      ? current.models.find(
          (model) =>
            modelRouteKey(model) === formatModelRouteKey(defaultPreference) &&
            !current.hiddenModelKeys.includes(modelRouteKey(model)),
        )
      : undefined;
    const selectedModel = preservedModel ?? defaultModel ?? null;
    const savedReasoningEffort = selectedModel?.supportsReasoning
      ? current.modelReasoningEfforts[modelRouteKey(selectedModel)]
      : undefined;
    set({
      activeConversationId: null,
      temporaryChat: null,
      selectedReasoningEffort:
        savedReasoningEffort && selectedModel?.reasoningLevels.includes(savedReasoningEffort)
          ? savedReasoningEffort
          : null,
      activeGoal: null,
      interruptedRun: null,
      goalStatus: 'ready',
      isChangingGoal: false,
      pendingProjectId: projectId,
      pendingAgentSnapshot: selectedAgent,
      messages: [],
      olderMessagesCursor: null,
      isLoadingOlderMessages: false,
      olderMessagesLoadError: false,
      personalMemoryExcludedForNewConversation: false,
      searchTargetMessageId: null,
      searchHighlightMessageId: null,
      searchHighlightQuery: null,
      isLoadingMessages: false,
      selectedModelKey: selectedModel?.key ?? null,
      errorCode: projectPreferenceInvalid ? 'projectDefaultModelLoadFailed' : assistantProfileError,
      errorMessage: null,
      opencodeRetryAtUnixMs: null,
    });
  };

  return {
    async markConversationRead(conversationId: string, expectedMessageId: string): Promise<void> {
      const unreadMessageId = await clearConversationUnreadStorage(conversationId, expectedMessageId);
      set((state) => ({
        conversations: state.conversations.map((conversation) =>
          conversation.id === conversationId ? { ...conversation, unreadMessageId } : conversation,
        ),
      }));
    },

    async markActiveConversationReadIfVisible(): Promise<void> {
      if (!isAppWindowVisibleAndFocused()) return;
      const current = get();
      if (!current.activeConversationId || current.isLoadingMessages) return;
      const conversation = current.conversations.find((item) => item.id === current.activeConversationId);
      if (conversation?.unreadMessageId) {
        await get().markConversationRead(conversation.id, conversation.unreadMessageId);
      }
    },

    async refreshConversations(): Promise<void> {
      const page = await listConversations();
      const current = get();
      const pageIds = new Set(page.items.map((conversation) => conversation.id));
      let activeConversation: ChatConversation | null | undefined;
      if (current.activeConversationId && !pageIds.has(current.activeConversationId)) {
        activeConversation = await getConversation(current.activeConversationId);
      }
      const olderConversations = page.hasMore
        ? current.conversations.filter(
            (conversation) => !pageIds.has(conversation.id) && conversation.id !== current.activeConversationId,
          )
        : [];
      const conversations = mergeById(
        page.items,
        [...olderConversations, ...(activeConversation ? [activeConversation] : [])],
      );
      const activeConversationWasDeleted =
        current.activeConversationId !== null && activeConversation === null;
      set({
        conversations,
        hasMoreConversations: page.hasMore,
        conversationListNextOffset: page.nextOffset,
        conversationListLoadError: false,
      });
      if (activeConversationWasDeleted) {
        messageLoadGeneration += 1;
        set({
          activeConversationId: null,
          selectedReasoningEffort: null,
          activeGoal: null,
          interruptedRun: null,
          goalStatus: 'ready',
          isChangingGoal: false,
          messages: [],
          olderMessagesCursor: null,
          isLoadingOlderMessages: false,
          olderMessagesLoadError: false,
          searchTargetMessageId: null,
          searchHighlightMessageId: null,
          searchHighlightQuery: null,
          isLoadingMessages: false,
        });
      }
    },

    async loadMoreConversations(): Promise<void> {
      const current = get();
      if (current.isLoadingMoreConversations || current.conversationListNextOffset === null) return;
      set({ isLoadingMoreConversations: true, conversationListLoadError: false });
      try {
        const page = await listConversations(current.conversationListNextOffset);
        set((state) => ({
          conversations: mergeById(state.conversations, page.items),
          hasMoreConversations: page.hasMore,
          conversationListNextOffset: page.nextOffset,
          isLoadingMoreConversations: false,
          conversationListLoadError: false,
        }));
      } catch (error) {
        set({ isLoadingMoreConversations: false, conversationListLoadError: true });
        globalThis.reportError(new Error('The next conversation page could not be loaded.', { cause: error }));
      }
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
        const page = await listProjects();
        const currentProjects = get().projects;
        const pageIds = new Set(page.items.map((project) => project.id));
        const olderProjects = page.hasMore
          ? currentProjects.filter((project) => !pageIds.has(project.id))
          : [];
        set({
          projects: mergeById(page.items, olderProjects),
          projectsStatus: 'ready',
          hasMoreProjects: page.hasMore,
          projectListNextOffset: page.nextOffset,
          isLoadingMoreProjects: false,
          projectListLoadError: false,
        });
      } catch (error) {
        set({ projectsStatus: 'error' });
        throw error;
      }
    },

    async loadMoreProjects(): Promise<void> {
      const current = get();
      if (current.isLoadingMoreProjects || current.projectListNextOffset === null) return;
      set({ isLoadingMoreProjects: true, projectListLoadError: false });
      try {
        const page = await listProjects(current.projectListNextOffset);
        set((state) => ({
          projects: mergeById(state.projects, page.items),
          hasMoreProjects: page.hasMore,
          projectListNextOffset: page.nextOffset,
          isLoadingMoreProjects: false,
          projectListLoadError: false,
        }));
      } catch (error) {
        set({ isLoadingMoreProjects: false, projectListLoadError: true });
        globalThis.reportError(new Error('The next project page could not be loaded.', { cause: error }));
      }
    },

    async clearConversationHistory(): Promise<boolean> {
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
      const attachmentCleanupPending = await clearConversationHistoryStorage();
      messageLoadGeneration += 1;
      set({
        conversations: [],
        hasMoreConversations: false,
        conversationListNextOffset: null,
        isLoadingMoreConversations: false,
        conversationListLoadError: false,
        activeConversationId: null,
        pendingToolPermissionRequest: null,
        pendingToolPermissionRequests: [],
        respondingToolPermissionIds: [],
        sendingConversationId: null,
        sendingConversationIds: [],
        activeOperation: null,
        activeOperationsByConversation: {},
        isSending: false,
        isUpdatingConversationModel: false,
        selectedReasoningEffort: null,
        activeGoal: null,
        interruptedRun: null,
        goalStatus: 'ready',
        isChangingGoal: false,
        pendingProjectId: null,
        pendingAgentSnapshot: null,
        messages: [],
        olderMessagesCursor: null,
        isLoadingOlderMessages: false,
        olderMessagesLoadError: false,
        personalMemoryExcludedForNewConversation: false,
        searchTargetMessageId: null,
        searchHighlightMessageId: null,
        searchHighlightQuery: null,
        isLoadingMessages: false,
        errorCode: null,
        errorMessage: null,
        opencodeRetryAtUnixMs: null,
      });
      return attachmentCleanupPending;
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

    setProviderCompaction(conversationId: string, isActive: boolean): void {
      set((current) => {
        const alreadyActive = current.providerCompactionConversationIds.includes(conversationId);
        if (alreadyActive === isActive) return current;
        return {
          providerCompactionConversationIds: isActive
            ? [...current.providerCompactionConversationIds, conversationId]
            : current.providerCompactionConversationIds.filter((id) => id !== conversationId),
        };
      });
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
        interruptedRun: null,
        goalStatus: 'loading',
        isChangingGoal: false,
        pendingProjectId: null,
        pendingAgentSnapshot: null,
        messages: [],
        olderMessagesCursor: null,
        isLoadingOlderMessages: false,
        olderMessagesLoadError: false,
        isLoadingMessages: true,
        errorCode: null,
        errorMessage: null,
        opencodeRetryAtUnixMs: null,
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
              errorMessage: null,
              opencodeRetryAtUnixMs: null,
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
            errorMessage: null,
            opencodeRetryAtUnixMs: null,
          });
          return;
        }
      }

      const selectedConversation = conversation;
      if (!selectedConversation) return;
      const selectedModel = get().models.find((model) => isSameRoute(model, selectedConversation));
      const savedReasoningEffort = selectedModel?.supportsReasoning
        ? selectedConversation.reasoningEffort ?? get().modelReasoningEfforts[modelRouteKey(selectedModel)]
        : undefined;
      set({
        selectedModelKey: selectedModel?.key ?? null,
        selectedReasoningEffort:
          savedReasoningEffort && selectedModel?.reasoningLevels.includes(savedReasoningEffort)
            ? savedReasoningEffort
            : null,
      });
      try {
        const history = targetMessageId === null
          ? await getMessagePage(id)
          : { messages: await getMessages(id), hasMore: false, nextBeforeMessageId: null };
        if (generation !== messageLoadGeneration) return;
        const targetWasFound = targetMessageId === null || history.messages.some((message) => message.id === targetMessageId);
        set({
          messages: history.messages,
          olderMessagesCursor:
            targetMessageId === null && history.hasMore ? history.nextBeforeMessageId : null,
          isLoadingOlderMessages: false,
          olderMessagesLoadError: false,
          isLoadingMessages: false,
          searchTargetMessageId: targetWasFound ? targetMessageId : null,
          errorCode: targetWasFound ? null : 'searchMessageUnavailable',
          errorMessage: null,
          opencodeRetryAtUnixMs: null,
        });
        if (isAppWindowVisibleAndFocused()) {
          void get().markActiveConversationReadIfVisible().catch((cause) => {
            globalThis.reportError(new Error('The active conversation unread state could not be cleared.', { cause }));
          });
        }
        void get().refreshActiveGoal(id);
        void get().refreshInterruptedRun(id);
      } catch {
        if (generation !== messageLoadGeneration) return;
        set({
          isLoadingMessages: false,
          goalStatus: 'ready',
          searchTargetMessageId: null,
          errorCode: targetMessageId ? 'searchMessageUnavailable' : 'messageHistoryLoadFailed',
          errorMessage: null,
          opencodeRetryAtUnixMs: null,
        });
      }
    },

    async loadOlderMessages(): Promise<void> {
      const initial = get();
      const conversationId = initial.activeConversationId;
      const beforeMessageId = initial.olderMessagesCursor;
      if (!conversationId || !beforeMessageId || initial.isLoadingOlderMessages) return;

      set({ isLoadingOlderMessages: true, olderMessagesLoadError: false });
      try {
        const page = await getMessagePage(conversationId, beforeMessageId);
        const current = get();
        if (
          current.activeConversationId !== conversationId ||
          current.olderMessagesCursor !== beforeMessageId
        ) {
          return;
        }
        set((state) => {
          const existingIds = new Set(state.messages.map((message) => message.id));
          const olderMessages = page.messages.filter((message) => !existingIds.has(message.id));
          return {
            messages: [...olderMessages, ...state.messages],
            olderMessagesCursor: page.hasMore ? page.nextBeforeMessageId : null,
            isLoadingOlderMessages: false,
            olderMessagesLoadError: false,
          };
        });
      } catch (cause) {
        const current = get();
        if (
          current.activeConversationId !== conversationId ||
          current.olderMessagesCursor !== beforeMessageId
        ) {
          return;
        }
        set({ isLoadingOlderMessages: false, olderMessagesLoadError: true });
        globalThis.reportError(new Error('Earlier conversation messages could not be loaded.', { cause }));
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
        errorMessage: null,
        opencodeRetryAtUnixMs: null,
      });
    },

    setPendingProjectId(projectId: string | null): void {
      const current = get();
      if (current.activeConversationId !== null || current.isSending) return;
      if (projectId !== null && !current.projects.some((project) => project.id === projectId)) return;
      set({ pendingProjectId: projectId });
    },

    startNewChat(projectId: string | null = null): void {
      beginNewChat(projectId, null, false);
    },

    startNewChatWithAgent(agent: AssistantAgentSnapshot): void {
      beginNewChat(null, agent, true);
    },
  };
}
