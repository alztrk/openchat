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
  clearModelReasoningEfforts,
  type DefaultModelPreference,
  defaultPreferenceForModel,
  formatModelRouteKey,
  modelRouteKey,
  readModelPreferences,
  writeDefaultModelPreference,
  writeHiddenModelKeys,
  writeLastSelectedModelPreference,
  writeModelContextLimit,
  writeModelReasoningEffort,
} from '@/modules/providers/data/modelPreferences';
import type { AssistantAgentSnapshot } from '@/modules/settings/data/assistantProfiles';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@/platform/local-service';
import { createBranchesActions } from './chatStoreBranches';
import { createConversationsActions } from './chatStoreConversations';
import { createGoalsActions } from './chatStoreGoals';
import { createRefreshModelsAction } from './chatStoreModels';
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
  | 'chatHistoryUnavailable'
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
  | 'assistantPreferencesLoadFailed'
  | 'assistantProfileDefaultUnavailable'
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
  pendingAgentSnapshot: AssistantAgentSnapshot | null;
  messages: ChatMessage[];
  searchTargetMessageId: string | null;
  searchHighlightMessageId: string | null;
  searchHighlightQuery: string | null;
  searchMatchIndex: number;
  searchMatchCount: number;
  models: ChatModel[];
  modelProviders: ChatModelProvider[];
  modelFavorites: ChatModelFavorite[];
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
  modelReasoningEfforts: Record<string, string>;
  defaultModelPreferenceInvalid: boolean;
  hiddenModelPreferencesInvalid: boolean;
  modelContextLimitsInvalid: boolean;
  modelReasoningEffortsInvalid: boolean;
  personalMemoryExcludedForNewConversation: boolean;
  selectedModelKey: string | null;
  isLoadingMessages: boolean;
  isSending: boolean;
  sendingConversationId: string | null;
  activeOperation: ServiceOperation | null;
  modelError: boolean;
  errorCode: ChatErrorCode | null;
  errorMessage: string | null;
  opencodeRetryAtUnixMs: number | null;
  initialize(): Promise<void>;
  refreshConversations(): Promise<void>;
  renameConversation(conversationId: string, title: string): Promise<void>;
  applyTitleUpdate(conversationId: string, title: string): void;
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
  setModelReasoningEffort(routeKey: string, effort: string | null): boolean;
  clearModelReasoningPreferences(): boolean;
  setPersonalMemoryExcludedForNewConversation(excluded: boolean): void;
  clearModelContextPreferences(): boolean;
  setPendingQuestionSendBlock(conversationId: string | null, blocked: boolean): void;
  beginConversationCompaction(conversationId: string): boolean;
  finishConversationCompaction(conversationId: string): void;
  toggleFavoriteModel(model: ChatModel): Promise<boolean>;
  removeModelFavorite(favorite: ChatModelFavorite): Promise<boolean>;
  respondToToolPermission(approved: boolean): Promise<void>;
  startTemporaryChat(): boolean;
  discloseTemporaryProvider(providerId: string): void;
  saveTemporaryChat(): Promise<boolean>;
  discardTemporaryChat(): void;
  setPendingAttachmentCount(count: number): void;
  selectModel(key: string | null): void;
  selectConversationModel(key: string, conversationId?: string): Promise<boolean>;
  selectReasoningEffort(effort: string | null): void;
  selectConversation(id: string, targetMessageId?: string | null, targetQuery?: string | null): Promise<void>;
  refreshActiveGoal(conversationId: string): Promise<void>;
  pauseActiveGoal(reason?: 'user_paused' | 'budget_exceeded'): Promise<void>;
  stopActiveGoal(): Promise<void>;
  updateMessage(message: ChatMessage): void;
  clearSearchTarget(found: boolean): void;
  setPendingProjectId(projectId: string | null): void;
  startNewChat(projectId?: string | null): void;
  startNewChatWithAgent(agent: AssistantAgentSnapshot): void;
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
  pendingAgentSnapshot: null,
  messages: [],
  searchTargetMessageId: null,
  searchHighlightMessageId: null,
  searchHighlightQuery: null,
  searchMatchIndex: 0,
  searchMatchCount: 0,
  models: [],
  modelProviders: [],
  modelFavorites: [],
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
  modelReasoningEfforts: {},
  defaultModelPreferenceInvalid: false,
  hiddenModelPreferencesInvalid: false,
  modelContextLimitsInvalid: false,
  modelReasoningEffortsInvalid: false,
  personalMemoryExcludedForNewConversation: false,
  selectedModelKey: null,
  isLoadingMessages: false,
  isSending: false,
  sendingConversationId: null,
  activeOperation: null,
  modelError: false,
  errorCode: null,
  errorMessage: null,
  opencodeRetryAtUnixMs: null,

  async initialize() {
    if (get().serviceStatus === 'loading' || get().serviceStatus === 'ready') return;
    set({
      serviceStatus: 'loading',
      storageInitializationErrorCode: null,
      projectsStatus: 'loading',
      errorCode: null,
      errorMessage: null,
      opencodeRetryAtUnixMs: null,
    });
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
      await Promise.all([get().refreshModelFavorites(), get().refreshModels()]);
    } catch (error) {
      set({
        serviceStatus: 'error',
        storageInitializationErrorCode: error instanceof LocalServiceError ? error.code : null,
        projectsStatus: 'error',
        errorCode: 'messageHistoryLoadFailed',
        errorMessage:
          error instanceof LocalServiceError
            ? (error.details ?? error.message).trim().slice(0, 1000) || null
            : error instanceof Error
              ? error.message.trim().slice(0, 1000) || null
              : typeof error === 'string'
                ? error.trim().slice(0, 1000) || null
                : null,
      });
    }
  },

  ...createConversationsActions(set, get),
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
      selectedReasoningEffort:
        current.selectedModelKey === key
          ? current.selectedReasoningEffort
          : model === null
            ? null
            : defaultReasoningEffort(model, current.modelReasoningEfforts),
      errorCode: null,
      errorMessage: null,
      opencodeRetryAtUnixMs: null,
    });
  },

  async selectConversationModel(key, targetConversationId) {
    const current = get();
    const model = current.models.find(
      (item) => item.key === key && !current.hiddenModelKeys.includes(modelRouteKey(item)),
    );
    if (!model) return false;
    const conversationId = targetConversationId ?? current.activeConversationId;
    if (!conversationId) {
      get().selectModel(key);
      return get().selectedModelKey === key;
    }
    if (current.isSending || current.isUpdatingConversationModel || current.modelStatus === 'loading') {
      return false;
    }
    const conversation = current.conversations.find((item) => item.id === conversationId);
    if (!conversation) {
      set({ errorCode: 'messageHistoryLoadFailed', opencodeRetryAtUnixMs: null });
      return false;
    }
    set({ isUpdatingConversationModel: true, errorCode: null, errorMessage: null, opencodeRetryAtUnixMs: null });
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
              selectedReasoningEffort:
                current.selectedModelKey === key
                  ? current.selectedReasoningEffort
                  : defaultReasoningEffort(model, latest.modelReasoningEfforts),
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
      set({ errorCode: 'conversationModelSaveFailed', opencodeRetryAtUnixMs: null });
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

  setModelReasoningEffort(routeKey, effort) {
    const model = get().models.find((item) => modelRouteKey(item) === routeKey);
    if (!model?.supportsReasoning || (effort !== null && !model.reasoningLevels.includes(effort))) return false;
    try {
      writeModelReasoningEffort(routeKey, effort);
      set((current) => {
        const modelReasoningEfforts = { ...current.modelReasoningEfforts };
        if (effort === null) delete modelReasoningEfforts[routeKey];
        else modelReasoningEfforts[routeKey] = effort;
        return {
          modelReasoningEfforts,
          modelReasoningEffortsInvalid: false,
          ...(current.activeConversationId === null && current.selectedModelKey === model.key
            ? { selectedReasoningEffort: effort }
            : {}),
        };
      });
      return true;
    } catch {
      return false;
    }
  },

  clearModelReasoningPreferences() {
    try {
      clearModelReasoningEfforts();
      set({ modelReasoningEfforts: {}, modelReasoningEffortsInvalid: false });
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
    modelReasoningEfforts: preferences.reasoningEfforts,
    defaultModelPreferenceInvalid: preferences.defaultModelInvalid,
    hiddenModelPreferencesInvalid: preferences.hiddenModelKeysInvalid,
    modelContextLimitsInvalid: preferences.contextLimitsInvalid,
    modelReasoningEffortsInvalid: preferences.reasoningEffortsInvalid,
  };
}

function defaultReasoningEffort(model: ChatModel, preferences: Readonly<Record<string, string>>): string | null {
  if (!model.supportsReasoning) return null;
  const effort = preferences[modelRouteKey(model)];
  return effort && model.reasoningLevels.includes(effort) ? effort : null;
}
