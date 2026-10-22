import { create } from 'zustand';

import {
  type ChatConversation,
  type ChatMessage,
  type ChatModelFavorite,
  type ChatProject,
  hasCompletedAssistantResponse as hasCompletedAssistantResponseStorage,
  listConversations,
  listModelFavorites,
  listProjects,
  type SavedOutputMessageKey,
  setConversationRoute,
  setModelFavorite,
} from '@/modules/conversations/data/chatStorage';
import type { ChatModel, ChatModelProvider } from '@/modules/providers/data/chatModels';

export type { ChatModel } from '@/modules/providers/data/chatModels';

import type { PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';
import type { ChatGoal } from '@/modules/conversations/data/chatGoals';
import {
  clearHiddenModelKeys,
  clearModelContextLimits,
  type DefaultModelPreference,
  defaultPreferenceForModel,
  formatModelRouteKey,
  modelRouteKey,
  readModelPreferences,
  writeDefaultModelPreference,
  writeHiddenModelKeys,
  writeLastSelectedModelPreference,
  writeModelContextLimit,
} from '@/modules/providers/data/modelPreferences';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@/platform/local-service';
import { createBranchesActions } from './chatStoreBranches';
import { createConversationsActions } from './chatStoreConversations';
import { createGoalsActions } from './chatStoreGoals';
import { createRefreshModelsAction } from './chatStoreModels';
import { createRefreshSavedOutputMessageKeys, createToggleSavedOutputAction } from './chatStoreSavedOutputs';
import { createSendMessageAction } from './chatStoreSend';
import type { TemporaryChatSession } from './chatStoreTemporary';
import { createTemporaryChatActions } from './chatStoreTemporary';
import { createRespondToToolPermissionAction } from './chatStoreToolPermissions';
import { favoriteSourceConnection } from './chatStoreUtils';

type ServiceStatus = 'idle' | 'loading' | 'ready' | 'error';
type FavoriteStatus = 'idle' | 'loading' | 'ready' | 'error';
type ChatSendMode = 'plan' | 'goal';

export interface ChatSendOptions {
  mode?: ChatSendMode;
  reasoningEffort?: string | null;
  fastMode?: boolean;
  excludedAssistantMessageId?: string;
  userMessageAlreadySaved?: boolean;
  conversationId?: string | null;
  onUserMessageCommitted?: (conversationId: string) => void;
}

export interface ChatToolPermissionRequest {
  id: string;
  conversationId: string;
  toolName: string;
  targetPath: string;
  arguments: Record<string, unknown>;
}

export type ChatErrorCode =
  | 'messageHistoryLoadFailed'
  | 'messageSaveFailed'
  | 'conversationCreateFailed'
  | 'chatRequestFailed'
  | 'providerLastRequestSaveFailed'
  | 'modelRequired'
  | 'modelDoesNotSupportImages'
  | 'providerCredentialMissing'
  | 'providerConnectionUnavailable'
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
  searchHighlightQuery: string | null;
  searchMatchIndex: number;
  searchMatchCount: number;
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
  chatGptWorkspaceSelectionRequired: boolean;
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
  sendingConversationId: string | null;
  activeOperation: ServiceOperation | null;
  modelError: boolean;
  errorCode: ChatErrorCode | null;
  initialize(): Promise<void>;
  refreshConversations(): Promise<void>;
  renameConversation(conversationId: string, title: string): Promise<void>;
  applyTitleUpdate(conversationId: string, title: string): void;
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
  selectConversation(id: string, targetMessageId?: string | null, targetQuery?: string | null): Promise<void>;
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
  setSearchMatchInfo(index: number, count: number): void;
  nextSearchMatch(): void;
  prevSearchMatch(): void;
  clearSearchHighlight(): void;
}

let titleUpdateUnsubscribe: (() => void) | null = null;

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
  searchHighlightQuery: null,
  searchMatchIndex: 0,
  searchMatchCount: 0,
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
  chatGptWorkspaceSelectionRequired: false,
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
  sendingConversationId: null,
  activeOperation: null,
  modelError: false,
  errorCode: null,

  async initialize() {
    if (get().serviceStatus === 'loading' || get().serviceStatus === 'ready') return;
    set({ serviceStatus: 'loading', storageInitializationErrorCode: null, projectsStatus: 'loading', errorCode: null });
    try {
      const storagePromise = Promise.allSettled([
        listConversations(),
        hasCompletedAssistantResponseStorage(),
        listProjects(),
      ]);

      await localServiceClient.initialize();
      if (!titleUpdateUnsubscribe) {
        titleUpdateUnsubscribe = localServiceClient.subscribe((event) => {
          if (event.name === 'chat.title_updated') {
            const data = event.data;
            if (typeof data.conversationId === 'string' && typeof data.title === 'string') {
              get().applyTitleUpdate(data.conversationId, data.title);
            }
          }
        });
      }
      const [conversationResult, responseResult, projectResult] = await storagePromise;
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
      await Promise.all([get().refreshModelFavorites(), get().refreshSavedOutputMessageKeys(), get().refreshModels()]);
    } catch (error) {
      set({
        serviceStatus: 'error',
        storageInitializationErrorCode: error instanceof LocalServiceError ? error.code : null,
        projectsStatus: 'error',
        errorCode: 'messageHistoryLoadFailed',
      });
    }
  },

  ...createConversationsActions(set, get),
  refreshSavedOutputMessageKeys: createRefreshSavedOutputMessageKeys(set),
  refreshModels: createRefreshModelsAction(set, get, localServiceClient),

  selectModel(key) {
    const current = get();
    const model =
      key === null
        ? null
        : (current.models.find((item) => item.key === key && !current.hiddenModelKeys.includes(modelRouteKey(item))) ??
          null);
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
      ...(current.temporaryChat
        ? {
            temporaryChat: {
              ...current.temporaryChat,
              route: model,
            },
          }
        : {}),
      selectedReasoningEffort: current.selectedModelKey === key ? current.selectedReasoningEffort : null,
      errorCode: null,
    });
  },

  async selectConversationModel(key) {
    const current = get();
    const model = current.models.find(
      (item) => item.key === key && !current.hiddenModelKeys.includes(modelRouteKey(item)),
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
        conversations: latest.conversations.map((item) =>
          item.id === conversationId
            ? {
                ...item,
                providerId: model.providerId,
                modelId: model.modelId,
                connectionId: model.connectionId ?? null,
                workspaceId: model.workspaceId ?? null,
                apiKeyConnectionId: model.apiKeyConnectionId ?? null,
                updatedAt: Date.now(),
              }
            : item,
        ),
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
    const defaultIsHidden =
      !isHidden && current.defaultModel !== null && formatModelRouteKey(current.defaultModel) === key;
    try {
      writeHiddenModelKeys(hiddenModelKeys);
      if (defaultIsHidden) writeDefaultModelPreference(null);
      set({
        hiddenModelKeys,
        hiddenModelPreferencesInvalid: false,
        defaultModel: defaultIsHidden ? null : current.defaultModel,
        defaultModelPreferenceInvalid: defaultIsHidden ? false : current.defaultModelPreferenceInvalid,
        selectedModelKey:
          !isHidden && !current.activeConversationId && current.selectedModelKey === model.key
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
    const favorite = get().modelFavorites.find(
      (item) =>
        item.providerId === model.providerId &&
        item.modelId === model.modelId &&
        item.sourceConnectionId === sourceConnectionId,
    );
    try {
      await setModelFavorite(
        {
          providerId: model.providerId,
          modelId: model.modelId,
          displayName: model.displayName,
          ...(sourceConnectionId ? { sourceConnectionId } : {}),
        },
        !favorite,
      );
      const modelFavorites = favorite
        ? get().modelFavorites.filter(
            (item) =>
              !(
                item.providerId === model.providerId &&
                item.modelId === model.modelId &&
                item.sourceConnectionId === sourceConnectionId
              ),
          )
        : [
            ...get().modelFavorites,
            {
              providerId: model.providerId,
              modelId: model.modelId,
              displayName: model.displayName,
              sourceConnectionId,
            },
          ];
      set({ modelFavorites, favoriteStatus: 'ready' });
      return true;
    } catch {
      set({ favoriteStatus: 'error' });
      return false;
    }
  },

  async removeModelFavorite(favorite) {
    try {
      await setModelFavorite(
        {
          providerId: favorite.providerId,
          modelId: favorite.modelId,
          displayName: favorite.displayName,
          ...(favorite.sourceConnectionId === null ? {} : { sourceConnectionId: favorite.sourceConnectionId }),
        },
        false,
      );
      set((current) => ({
        modelFavorites: current.modelFavorites.filter(
          (item) =>
            !(
              item.providerId === favorite.providerId &&
              item.modelId === favorite.modelId &&
              item.sourceConnectionId === favorite.sourceConnectionId
            ),
        ),
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
  ...createTemporaryChatActions(set, get),
  ...createGoalsActions(set, get),
  sendMessage: createSendMessageAction(set, get, localServiceClient),
  ...createBranchesActions(set, get),

  setSearchMatchInfo(index: number, count: number): void {
    set({ searchMatchIndex: index, searchMatchCount: count });
  },
  nextSearchMatch(): void {
    const { searchMatchIndex, searchMatchCount } = get();
    if (searchMatchCount > 1) {
      set({ searchMatchIndex: (searchMatchIndex + 1) % searchMatchCount });
    }
  },
  prevSearchMatch(): void {
    const { searchMatchIndex, searchMatchCount } = get();
    if (searchMatchCount > 1) {
      set({ searchMatchIndex: (searchMatchIndex - 1 + searchMatchCount) % searchMatchCount });
    }
  },
  clearSearchHighlight(): void {
    set({
      searchHighlightQuery: null,
      searchHighlightMessageId: null,
      searchMatchIndex: 0,
      searchMatchCount: 0,
    });
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
