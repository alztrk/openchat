import { create } from 'zustand';

import {
  clearConversationHistory as clearConversationHistoryStorage,
  createConversationBranchFromUserMessage,
  getConversation,
  getMessages,
  hasCompletedAssistantResponse as hasCompletedAssistantResponseStorage,
  listModelFavorites,
  listConversations,
  listProjects,
  renameConversation as renameConversationStorage,
  setConversationRoute,
  setModelFavorite,
  type ChatModelFavorite,
  type ChatConversation,
  type ChatMessage,
  type ChatProject,
  type SavedOutputMessageKey,
} from '@/features/chat/data/chatStorage';
import type { ChatModel, ChatModelProvider } from '@/features/chat/data/chatModels';
export type { ChatModel } from '@/features/chat/data/chatModels';
import {
  LocalServiceClient,
  LocalServiceError,
  type ServiceOperation,
} from '@/shared/services/LocalServiceClient';
import { createRespondToToolPermissionAction } from './chatStoreToolPermissions';
import { createRefreshSavedOutputMessageKeys, createToggleSavedOutputAction } from './chatStoreSavedOutputs';
import { createTemporaryChatActions, createTemporarySendMessageAction } from './chatStoreTemporary';
import type { TemporaryChatSession } from './chatStoreTemporary';
import type { PendingChatAttachment } from '@/features/chat/data/chatAttachments';
import { isTerminalChatGoalStatus, parseChatGoal, type ChatGoal } from '@/features/chat/data/chatGoals';
import {
  clearHiddenModelKeys,
  clearModelContextLimits,
  defaultPreferenceForModel,
  formatModelRouteKey,
  modelRouteKey,
  readModelPreferences,
  readLastSelectedModelPreference,
  writeLastSelectedModelPreference,
  writeDefaultModelPreference,
  writeHiddenModelKeys,
  writeModelContextLimit,
  type DefaultModelPreference,
} from '@/features/settings/data/modelPreferences';
import {
  readProjectDefaultModelPreference,
} from '@/features/chat/data/projectPreferences';
import {
  favoriteSourceConnection,
  isSameRoute,
  upsertMessage,
} from './chatStoreUtils';
import { createSendMessageAction } from './chatStoreSend';
import { createRefreshModelsAction } from './chatStoreModels';

type ServiceStatus = 'idle' | 'loading' | 'ready' | 'error';
type FavoriteStatus = 'idle' | 'loading' | 'ready' | 'error';
type ChatSendMode = 'plan' | 'goal';

export interface ChatSendOptions {
  mode?: ChatSendMode;
  reasoningEffort?: string | null;
  fastMode?: boolean;
  excludedAssistantMessageId?: string;
  userMessageAlreadySaved?: boolean;
}

export interface ChatToolPermissionRequest {
  id: string;
  toolName: string;
  targetPath: string;
  arguments: Record<string, unknown>;
}

export type ChatErrorCode =
  | 'messageHistoryLoadFailed'
  | 'messageSaveFailed'
  | 'chatRequestFailed'
  | 'providerLastRequestSaveFailed'
  | 'modelRequired'
  | 'modelDoesNotSupportImages'
  | 'providerCredentialMissing'
  | 'personalMemoryLoadFailed'
  | 'attachmentSaveFailed'
  | 'userQuestionUnavailable'
  | 'searchMessageUnavailable'
  | 'projectDefaultModelLoadFailed'
  | 'projectToolRulesLoadFailed'
  | 'projectSkillsLoadFailed'
  | 'toolPermissionModeLoadFailed'
  | 'goalAlreadyActive'
  | 'goalStateUnavailable'
  | 'goalObjectiveRequired'
  | 'goalObjectiveTooLong'
  | 'conversationModelSaveFailed'
  | 'temporaryChatConnectionRequired'
  | 'temporaryChatRemoveAttachmentsFirst'
  | 'temporaryChatStopBeforeLeaving'
  | 'temporaryChatMessageLimitReached'
  | 'temporaryChatSaveFailed'
  | 'temporaryChatSaveCleanupFailed'
  | 'temporaryChatExitRequired'
  | 'conversationBranchCreateFailed'
  | 'responseRetryUnavailable'
  | 'responseRetryNotCompleted'
  | 'responseReplaceFailed'
  | 'responseRetryCleanupFailed';

export interface ChatState {
  serviceStatus: ServiceStatus;
  storageInitializationErrorCode: string | null;
  modelStatus: ServiceStatus;
  conversations: ChatConversation[];
  projects: ChatProject[];
  projectsStatus: ServiceStatus;
  activeConversationId: string | null;
  temporaryChat: TemporaryChatSession | null;
  isUpdatingConversationModel: boolean;
  compactingConversationId: string | null;
  isCreatingMessageBranch: boolean;
  pendingQuestionSendBlock: { conversationId: string; blocked: boolean } | null;
  selectedReasoningEffort: string | null;
  activeGoal: ChatGoal | null;
  goalStatus: ServiceStatus;
  isChangingGoal: boolean;
  pendingProjectId: string | null;
  messages: ChatMessage[];
  searchTargetMessageId: string | null;
  searchHighlightMessageId: string | null;
  models: ChatModel[];
  modelProviders: ChatModelProvider[];
  modelFavorites: ChatModelFavorite[];
  savedOutputMessageKeys: SavedOutputMessageKey[];
  pendingSavedOutputMessageIds: string[];
  savedOutputError: boolean;
  pendingAttachmentCount: number;
  pendingToolPermissionRequest: ChatToolPermissionRequest | null;
  isRespondingToToolPermission: boolean;
  toolPermissionError: 'toolPermissionRequestExpired' | 'toolPermissionResponseFailed' | null;
  hasConfiguredProvider: boolean;
  hasCompletedAssistantResponse: boolean | null;
  favoriteStatus: FavoriteStatus;
  hiddenModelKeys: string[];
  defaultModel: DefaultModelPreference | null;
  modelContextLimits: Record<string, number>;
  defaultModelPreferenceInvalid: boolean;
  hiddenModelPreferencesInvalid: boolean;
  modelContextLimitsInvalid: boolean;
  personalMemoryExcludedForNewConversation: boolean;
  selectedModelKey: string | null;
  isLoadingMessages: boolean;
  isSending: boolean;
  activeOperation: ServiceOperation | null;
  modelError: boolean;
  errorCode: ChatErrorCode | null;
  initialize(): Promise<void>;
  refreshConversations(): Promise<void>;
  renameConversation(conversationId: string, title: string): Promise<void>;
  refreshSavedOutputMessageKeys(): Promise<void>;
  refreshProjects(): Promise<void>;
  clearConversationHistory(): Promise<void>;
  refreshModels(forceRefresh?: boolean): Promise<void>;
  refreshModelFavorites(): Promise<void>;
  setDefaultModel(model: ChatModel): boolean;
  clearDefaultModel(): boolean;
  clearInvalidDefaultModel(): boolean;
  toggleHiddenModel(model: ChatModel): boolean;
  clearHiddenModelPreferences(): boolean;
  setModelContextLimit(routeKey: string, limit: number | null): boolean;
  setPersonalMemoryExcludedForNewConversation(excluded: boolean): void;
  clearModelContextPreferences(): boolean;
  setPendingQuestionSendBlock(conversationId: string | null, blocked: boolean): void;
  beginConversationCompaction(conversationId: string): boolean;
  finishConversationCompaction(conversationId: string): void;
  toggleFavoriteModel(model: ChatModel): Promise<boolean>;
  removeModelFavorite(favorite: ChatModelFavorite): Promise<boolean>;
  toggleSavedOutput(messageId: string): Promise<boolean>;
  respondToToolPermission(approved: boolean): Promise<void>;
  startTemporaryChat(): boolean;
  discloseTemporaryProvider(providerId: string): void;
  saveTemporaryChat(): Promise<boolean>;
  discardTemporaryChat(): void;
  setPendingAttachmentCount(count: number): void;
  selectModel(key: string | null): void;
  selectConversationModel(key: string): Promise<boolean>;
  selectReasoningEffort(effort: string | null): void;
  selectConversation(id: string, targetMessageId?: string | null): Promise<void>;
  refreshActiveGoal(conversationId: string): Promise<void>;
  pauseActiveGoal(): Promise<void>;
  stopActiveGoal(): Promise<void>;
  updateMessage(message: ChatMessage): void;
  clearSearchTarget(found: boolean): void;
  setPendingProjectId(projectId: string | null): void;
  startNewChat(projectId?: string | null): void;
  sendMessage(
    content: string,
    attachments?: readonly PendingChatAttachment[],
    resumeRunId?: string,
    resumeAssistantMessageId?: string,
    options?: ChatSendOptions,
  ): Promise<boolean>;
  retryMessage(messageId: string): Promise<boolean>;
  branchFromUserMessage(messageId: string, editedContent: string, branchTitle: string): Promise<boolean>;
  stopResponse(): Promise<void>;
  dismissError(): void;
}

export const localServiceClient = new LocalServiceClient();

let messageLoadGeneration = 0;
let goalChangeGeneration = 0;
let goalRefreshGeneration = 0;

export const useChatStore = create<ChatState>((set, get) => ({
  serviceStatus: 'idle',
  storageInitializationErrorCode: null,
  modelStatus: 'idle',
  conversations: [],
  projects: [],
  projectsStatus: 'idle',
  activeConversationId: null,
  temporaryChat: null,
  isUpdatingConversationModel: false,
  compactingConversationId: null,
  isCreatingMessageBranch: false,
  pendingQuestionSendBlock: null,
  selectedReasoningEffort: null,
  activeGoal: null,
  goalStatus: 'idle',
  isChangingGoal: false,
  pendingProjectId: null,
  messages: [],
  searchTargetMessageId: null,
  searchHighlightMessageId: null,
  models: [],
  modelProviders: [],
  modelFavorites: [],
  savedOutputMessageKeys: [],
  pendingSavedOutputMessageIds: [],
  savedOutputError: false,
  pendingAttachmentCount: 0,
  pendingToolPermissionRequest: null,
  isRespondingToToolPermission: false,
  toolPermissionError: null,
  hasConfiguredProvider: false,
  hasCompletedAssistantResponse: null,
  favoriteStatus: 'idle',
  hiddenModelKeys: [],
  defaultModel: null,
  modelContextLimits: {},
  defaultModelPreferenceInvalid: false,
  hiddenModelPreferencesInvalid: false,
  modelContextLimitsInvalid: false,
  personalMemoryExcludedForNewConversation: false,
  selectedModelKey: null,
  isLoadingMessages: false,
  isSending: false,
  activeOperation: null,
  modelError: false,
  errorCode: null,

  async initialize() {
    if (get().serviceStatus === 'loading' || get().serviceStatus === 'ready') return;
    set({ serviceStatus: 'loading', storageInitializationErrorCode: null, projectsStatus: 'loading', errorCode: null });
    try {
      await localServiceClient.initialize();
      const [conversationResult, responseResult, projectResult] = await Promise.allSettled([
        listConversations(),
        hasCompletedAssistantResponseStorage(),
        listProjects(),
      ]);
      if (conversationResult.status === 'rejected') {
        throw conversationResult.reason;
      }
      if (responseResult.status === 'rejected') {
        throw responseResult.reason;
      }
      const preferences = readModelPreferences();
      set({
        conversations: conversationResult.value,
        hasCompletedAssistantResponse: responseResult.value,
        projects: projectResult.status === 'fulfilled' ? projectResult.value : [],
        projectsStatus: projectResult.status === 'fulfilled' ? 'ready' : 'error',
        serviceStatus: 'ready',
        storageInitializationErrorCode: null,
        ...mapModelPreferences(preferences),
      });
      await get().refreshModelFavorites();
      await get().refreshSavedOutputMessageKeys();
      await get().refreshModels();
    } catch (error) {
      set({
        serviceStatus: 'error',
        storageInitializationErrorCode: error instanceof LocalServiceError ? error.code : null,
        projectsStatus: 'error',
        errorCode: 'messageHistoryLoadFailed',
      });
    }
  },

  async refreshConversations() {
    const conversations = await listConversations();
    const currentConversationId = get().activeConversationId;
    if (currentConversationId !== null && !conversations.some((conversation) => conversation.id === currentConversationId)) {
      messageLoadGeneration += 1;
      goalChangeGeneration += 1;
      goalRefreshGeneration += 1;
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
        isLoadingMessages: false,
      });
      await get().refreshSavedOutputMessageKeys();
      return;
    }
    set({ conversations });
    await get().refreshSavedOutputMessageKeys();
  },

  async renameConversation(conversationId, title) {
    await renameConversationStorage(conversationId, title);
    set((state) => ({
      conversations: state.conversations.map((conversation) => conversation.id === conversationId
        ? { ...conversation, title }
        : conversation),
    }));
  },

  refreshSavedOutputMessageKeys: createRefreshSavedOutputMessageKeys(set),

  async refreshProjects() {
    set({ projectsStatus: 'loading' });
    try {
      const projects = await listProjects();
      set({ projects, projectsStatus: 'ready' });
    } catch (error) {
      set({ projectsStatus: 'error' });
      throw error;
    }
  },

  async clearConversationHistory() {
    if (get().isSending) {
      throw new LocalServiceError('conversation_archive_busy', 'Stop the active response before clearing chat history.');
    }
    if (get().isUpdatingConversationModel) {
      throw new LocalServiceError('conversation_archive_busy', 'Wait for the current model change before clearing chat history.');
    }
    await localServiceClient.call('chat.file_changes.delete_all');
    await clearConversationHistoryStorage();
    messageLoadGeneration += 1;
    goalChangeGeneration += 1;
    goalRefreshGeneration += 1;
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
      isLoadingMessages: false,
      errorCode: null,
    });
  },

  refreshModels: createRefreshModelsAction(set, get, localServiceClient),

  setPendingQuestionSendBlock(conversationId, blocked) {
    const current = get().pendingQuestionSendBlock;
    if (conversationId === null && current === null) return;
    if (current?.conversationId === conversationId && current.blocked === blocked) return;
    set({
      pendingQuestionSendBlock: conversationId === null
        ? null
        : { conversationId, blocked },
    });
  },

  beginConversationCompaction(conversationId) {
    const current = get();
    const pendingQuestionBlocksSend = current.pendingQuestionSendBlock?.conversationId === conversationId &&
      current.pendingQuestionSendBlock.blocked;
    if (
      current.activeConversationId !== conversationId ||
      current.isSending ||
      current.isUpdatingConversationModel ||
      current.isCreatingMessageBranch ||
      current.compactingConversationId !== null ||
      pendingQuestionBlocksSend
    ) return false;
    set({ compactingConversationId: conversationId });
    return true;
  },

  finishConversationCompaction(conversationId) {
    if (get().compactingConversationId === conversationId) {
      set({ compactingConversationId: null });
    }
  },

  selectModel(key) {
    const current = get();
    const model = key === null
      ? null
      : current.models.find((item) =>
          item.key === key && !current.hiddenModelKeys.includes(modelRouteKey(item)),
        ) ?? null;
    if (key !== null && model === null) return;
    if (model !== null) {
      try {
        writeLastSelectedModelPreference(defaultPreferenceForModel(model));
      } catch (cause) {
        globalThis.reportError(new Error('The last selected model could not be saved.', { cause }));
      }
    }
    set({
      selectedModelKey: key,
      ...(current.temporaryChat ? {
        temporaryChat: {
          ...current.temporaryChat,
          route: model,
        },
      } : {}),
      selectedReasoningEffort: current.selectedModelKey === key ? current.selectedReasoningEffort : null,
      errorCode: null,
    });
  },

  async selectConversationModel(key) {
    const current = get();
    const model = current.models.find((item) =>
      item.key === key && !current.hiddenModelKeys.includes(modelRouteKey(item)),
    );
    if (!model) return false;
    const conversationId = current.activeConversationId;
    if (!conversationId) {
      get().selectModel(key);
      return get().selectedModelKey === key;
    }
    if (current.isSending || current.isUpdatingConversationModel || current.modelStatus === 'loading') {
      return false;
    }
    const conversation = current.conversations.find((item) => item.id === conversationId);
    if (!conversation) {
      set({ errorCode: 'messageHistoryLoadFailed' });
      return false;
    }
    set({ isUpdatingConversationModel: true, errorCode: null });
    try {
      await setConversationRoute(conversationId, model);
      const latest = get();
      set({
        conversations: latest.conversations.map((item) => item.id === conversationId
          ? {
              ...item,
              providerId: model.providerId,
              modelId: model.modelId,
              connectionId: model.connectionId ?? null,
              workspaceId: model.workspaceId ?? null,
              apiKeyConnectionId: model.apiKeyConnectionId ?? null,
              updatedAt: Date.now(),
            }
          : item),
        ...(latest.activeConversationId === conversationId
          ? {
              selectedModelKey: key,
              selectedReasoningEffort: current.selectedModelKey === key ? current.selectedReasoningEffort : null,
            }
          : {}),
      });
      try {
        writeLastSelectedModelPreference(defaultPreferenceForModel(model));
      } catch (cause) {
        globalThis.reportError(new Error('The last selected model could not be saved.', { cause }));
      }
      return true;
    } catch {
      set({ errorCode: 'conversationModelSaveFailed' });
      return false;
    } finally {
      set({ isUpdatingConversationModel: false });
    }
  },

  selectReasoningEffort(effort) {
    const current = get();
    const model = current.models.find((item) => item.key === current.selectedModelKey);
    if (effort !== null && (!model?.supportsReasoning || !model.reasoningLevels.includes(effort))) return;
    set({ selectedReasoningEffort: effort });
  },

  async refreshModelFavorites() {
    set({ favoriteStatus: 'loading' });
    try {
      const modelFavorites = await listModelFavorites();
      set({ modelFavorites, favoriteStatus: 'ready' });
    } catch {
      set({ favoriteStatus: 'error' });
    }
  },

  setDefaultModel(model) {
    const preference = defaultPreferenceForModel(model);
    const current = get();
    const modelKey = modelRouteKey(model);
    const hiddenModelKeys = current.hiddenModelKeys.filter((key) => key !== modelKey);
    try {
      if (hiddenModelKeys.length !== current.hiddenModelKeys.length || current.hiddenModelPreferencesInvalid) {
        writeHiddenModelKeys(hiddenModelKeys);
      }
      writeDefaultModelPreference(preference);
      set({
        defaultModel: preference,
        defaultModelPreferenceInvalid: false,
        hiddenModelKeys,
        hiddenModelPreferencesInvalid: false,
        selectedModelKey: current.activeConversationId ? current.selectedModelKey : model.key,
      });
      try {
        writeLastSelectedModelPreference(preference);
      } catch (cause) {
        globalThis.reportError(new Error('The last selected model could not be saved.', { cause }));
      }
      return true;
    } catch {
      return false;
    }
  },

  clearDefaultModel() {
    return get().clearInvalidDefaultModel();
  },

  clearInvalidDefaultModel() {
    try {
      writeDefaultModelPreference(null);
      set({ defaultModel: null, defaultModelPreferenceInvalid: false });
      return true;
    } catch {
      return false;
    }
  },

  toggleHiddenModel(model) {
    const key = modelRouteKey(model);
    const current = get();
    const isHidden = current.hiddenModelKeys.includes(key);
    const hiddenModelKeys = isHidden
      ? current.hiddenModelKeys.filter((entry) => entry !== key)
      : [...current.hiddenModelKeys, key];
    const defaultIsHidden = !isHidden && current.defaultModel !== null &&
      formatModelRouteKey(current.defaultModel) === key;
    try {
      writeHiddenModelKeys(hiddenModelKeys);
      if (defaultIsHidden) writeDefaultModelPreference(null);
      set({
        hiddenModelKeys,
        hiddenModelPreferencesInvalid: false,
        defaultModel: defaultIsHidden ? null : current.defaultModel,
        defaultModelPreferenceInvalid: defaultIsHidden ? false : current.defaultModelPreferenceInvalid,
        selectedModelKey: !isHidden && !current.activeConversationId && current.selectedModelKey === model.key
          ? null
          : current.selectedModelKey,
      });
      return true;
    } catch {
      return false;
    }
  },

  clearHiddenModelPreferences() {
    try {
      clearHiddenModelKeys();
      set({ hiddenModelKeys: [], hiddenModelPreferencesInvalid: false });
      return true;
    } catch {
      return false;
    }
  },

  setModelContextLimit(routeKey, limit) {
    try {
      writeModelContextLimit(routeKey, limit);
      set((current) => {
        const modelContextLimits = { ...current.modelContextLimits };
        if (limit === null) delete modelContextLimits[routeKey];
        else modelContextLimits[routeKey] = limit;
        return { modelContextLimits, modelContextLimitsInvalid: false };
      });
      return true;
    } catch {
      return false;
    }
  },

  setPersonalMemoryExcludedForNewConversation(excluded) {
    if (get().activeConversationId !== null || get().isSending) return;
    set({ personalMemoryExcludedForNewConversation: excluded });
  },

  clearModelContextPreferences() {
    try {
      clearModelContextLimits();
      set({ modelContextLimits: {}, modelContextLimitsInvalid: false });
      return true;
    } catch {
      return false;
    }
  },

  async toggleFavoriteModel(model) {
    const sourceConnectionId = favoriteSourceConnection(model);
    const favorite = get().modelFavorites.find((item) =>
      item.providerId === model.providerId &&
      item.modelId === model.modelId &&
      item.sourceConnectionId === sourceConnectionId,
    );
    try {
      await setModelFavorite({
        providerId: model.providerId,
        modelId: model.modelId,
        displayName: model.displayName,
        ...(sourceConnectionId ? { sourceConnectionId } : {}),
      }, !favorite);
      const modelFavorites = favorite
        ? get().modelFavorites.filter((item) => !(
            item.providerId === model.providerId &&
            item.modelId === model.modelId &&
            item.sourceConnectionId === sourceConnectionId
          ))
        : [...get().modelFavorites, {
            providerId: model.providerId,
            modelId: model.modelId,
            displayName: model.displayName,
            sourceConnectionId,
          }];
      set({ modelFavorites, favoriteStatus: 'ready' });
      return true;
    } catch {
      set({ favoriteStatus: 'error' });
      return false;
    }
  },

  async removeModelFavorite(favorite) {
    try {
      await setModelFavorite({
        providerId: favorite.providerId,
        modelId: favorite.modelId,
        displayName: favorite.displayName,
        ...(favorite.sourceConnectionId === null
          ? {}
          : { sourceConnectionId: favorite.sourceConnectionId }),
      }, false);
      set((current) => ({
        modelFavorites: current.modelFavorites.filter((item) => !(
          item.providerId === favorite.providerId &&
          item.modelId === favorite.modelId &&
          item.sourceConnectionId === favorite.sourceConnectionId
        )),
        favoriteStatus: 'ready',
      }));
      return true;
    } catch {
      set({ favoriteStatus: 'error' });
      return false;
    }
  },

  toggleSavedOutput: createToggleSavedOutputAction(set, get),
  respondToToolPermission: createRespondToToolPermissionAction(set, get, localServiceClient),

  async selectConversation(id, targetMessageId = null) {
    if (get().temporaryChat) {
      set({ errorCode: get().isSending ? 'temporaryChatStopBeforeLeaving' : 'temporaryChatExitRequired' });
      return;
    }
    const generation = ++messageLoadGeneration;
    goalChangeGeneration += 1;
    goalRefreshGeneration += 1;
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
      selectedModelKey:
        get().models.find((model) => isSameRoute(model, selectedConversation))?.key ?? null,
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

  async refreshActiveGoal(conversationId) {
    if (get().activeConversationId !== conversationId) return;
    const generation = ++goalRefreshGeneration;
    set({ goalStatus: 'loading' });
    try {
      const response = await localServiceClient.call('chat.goal.active', { conversationId });
      if (!('goal' in response)) throw new Error('The active goal response is invalid.');
      const goal = parseChatGoal(response.goal);
      if (goal && goal.conversationId !== conversationId) throw new Error('The active goal belongs to another chat.');
      if (get().activeConversationId !== conversationId || generation !== goalRefreshGeneration) return;
      set({
        activeGoal: goal && !isTerminalChatGoalStatus(goal.status) ? goal : null,
        goalStatus: 'ready',
      });
    } catch {
      if (get().activeConversationId === conversationId && generation === goalRefreshGeneration) {
        set({ activeGoal: null, goalStatus: 'error' });
      }
    }
  },

  async pauseActiveGoal() {
    const current = get();
    const goal = current.activeGoal;
    if (!goal || goal.status !== 'running' || current.isChangingGoal) return;
    const generation = ++goalChangeGeneration;
    set({ isChangingGoal: true, goalStatus: 'ready' });
    try {
      await localServiceClient.call('chat.goal.pause', {
        conversationId: goal.conversationId,
        runId: goal.runId,
      });
      if (
        generation !== goalChangeGeneration ||
        get().activeConversationId !== goal.conversationId ||
        get().activeGoal?.runId !== goal.runId
      ) return;
      if (current.activeConversationId === goal.conversationId) await current.activeOperation?.cancel();
      await get().refreshActiveGoal(goal.conversationId);
    } catch {
      if (generation === goalChangeGeneration && get().activeConversationId === goal.conversationId) {
        set({ goalStatus: 'error' });
      }
    } finally {
      if (generation === goalChangeGeneration) set({ isChangingGoal: false });
    }
  },

  async stopActiveGoal() {
    const current = get();
    const goal = current.activeGoal;
    if (!goal || current.isChangingGoal) return;
    const generation = ++goalChangeGeneration;
    set({ isChangingGoal: true, goalStatus: 'ready' });
    try {
      await localServiceClient.call('chat.goal.stop', {
        conversationId: goal.conversationId,
        runId: goal.runId,
      });
      if (
        generation !== goalChangeGeneration ||
        get().activeConversationId !== goal.conversationId ||
        get().activeGoal?.runId !== goal.runId
      ) return;
      if (current.activeConversationId === goal.conversationId) await current.activeOperation?.cancel();
      await get().refreshActiveGoal(goal.conversationId);
    } catch {
      if (generation === goalChangeGeneration && get().activeConversationId === goal.conversationId) {
        set({ goalStatus: 'error' });
      }
    } finally {
      if (generation === goalChangeGeneration) set({ isChangingGoal: false });
    }
  },

  updateMessage(message) {
    if (get().activeConversationId !== message.conversationId) return;
    set((current) => ({ messages: upsertMessage(current.messages, message) }));
  },

  clearSearchTarget(found) {
    const targetMessageId = get().searchTargetMessageId;
    if (!targetMessageId) return;
    set({
      searchTargetMessageId: null,
      searchHighlightMessageId: found ? targetMessageId : null,
      errorCode: found ? null : 'searchMessageUnavailable',
    });
  },

  setPendingProjectId(projectId) {
    const current = get();
    if (current.activeConversationId !== null || current.isSending) return;
    if (projectId !== null && !current.projects.some((project) => project.id === projectId)) return;
    set({ pendingProjectId: projectId });
  },

  startNewChat(projectId = null) {
    messageLoadGeneration++;
    goalChangeGeneration += 1;
    goalRefreshGeneration += 1;
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
    const defaultPreference = projectPreferenceInvalid
      ? null
      : projectPreference ?? lastSelectedPreference ?? current.defaultModel;
    const defaultModel = defaultPreference
      ? current.models.find((model) =>
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
      isLoadingMessages: false,
      selectedModelKey: defaultModel?.key ?? null,
      errorCode: projectPreferenceInvalid ? 'projectDefaultModelLoadFailed' : null,
    });
  },

  ...createTemporaryChatActions(set, get),

  sendMessage: (...args) => get().temporaryChat
    ? createTemporarySendMessageAction(set, get, localServiceClient)(...args)
    : createSendMessageAction(set, get, localServiceClient)(...args),

  async retryMessage(messageId) {
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

  async branchFromUserMessage(messageId, editedContent, branchTitle) {
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

  async stopResponse() {
    const operation = get().activeOperation;
    if (!operation) return;
    await operation.cancel();
  },

  dismissError() {
    set({ errorCode: null });
  },
}));

function mapModelPreferences(preferences: ReturnType<typeof readModelPreferences>) {
  return {
    defaultModel: preferences.defaultModel,
    hiddenModelKeys: preferences.hiddenModelKeys,
    modelContextLimits: preferences.contextLimits,
    defaultModelPreferenceInvalid: preferences.defaultModelInvalid,
    hiddenModelPreferencesInvalid: preferences.hiddenModelKeysInvalid,
    modelContextLimitsInvalid: preferences.contextLimitsInvalid,
  };
}

