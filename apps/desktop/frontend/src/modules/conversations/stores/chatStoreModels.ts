import type { StoreApi } from 'zustand/vanilla';
import { readProjectDefaultModelPreference } from '@/modules/projects/data/projectPreferences';
import type { ChatModelProvider } from '@/modules/providers/data/chatModels';
import { loadApiKeyModels, loadChatGptModels, loadLocalModels } from '@/modules/providers/data/modelCatalogLoader';
import {
  type DefaultModelPreference,
  formatModelRouteKey,
  modelRouteKey,
  readLastSelectedModelPreference,
} from '@/modules/providers/data/modelPreferences';
import { refreshOAuthRateLimits } from '@/modules/providers/data/oauthRateLimits';
import type { LocalServiceClient } from '@/platform/local-service';
import type { ChatState } from './chatStore';
import { isSameRoute } from './chatStoreUtils';
import { loadModelCatalogsProgressively, type ModelCatalogProgress } from './modelCatalogProgress';
import { createModelCatalogRefreshGuard } from './modelCatalogRefreshGuard';

type ChatStoreSetState = StoreApi<ChatState>['setState'];
type ChatStoreGetState = StoreApi<ChatState>['getState'];

export function createRefreshModelsAction(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
  localServiceClient: LocalServiceClient,
): ChatState['refreshModels'] {
  const refreshGuard = createModelCatalogRefreshGuard();

  return async (forceRefresh = false) => {
    const refreshRevision = refreshGuard.begin();
    set({ modelStatus: 'loading', modelError: false, chatGptWorkspaceSelectionRequired: false });
    let lastSelectedPreference: DefaultModelPreference | null = null;
    let lastSelectedPreferenceRead = false;

    const publishCatalogs = ({
      chatGpt: chatGptCatalogResult,
      local: localCatalogResult,
      apiKey: apiKeyCatalogResult,
      allSettled: allCatalogsSettled,
    }: ModelCatalogProgress) => {
      if (!refreshGuard.isCurrent(refreshRevision)) return;
      const chatGptCatalog = chatGptCatalogResult?.status === 'fulfilled' ? chatGptCatalogResult.value : null;
      const localModels = localCatalogResult?.status === 'fulfilled' ? localCatalogResult.value : [];
      const apiKeyCatalog = apiKeyCatalogResult?.status === 'fulfilled' ? apiKeyCatalogResult.value : null;
      const models = [...(chatGptCatalog?.models ?? []), ...(apiKeyCatalog?.models ?? []), ...localModels];
      const providerMap = new Map<string, ChatModelProvider>([
        ['opencode', { providerId: 'opencode', name: null, available: true }],
        ['kilo', { providerId: 'kilo', name: null, available: true }],
        ['llama_cpp', { providerId: 'llama_cpp', name: null, available: true }],
      ]);
      for (const provider of [...(chatGptCatalog?.providers ?? []), ...(apiKeyCatalog?.providers ?? [])]) {
        const currentProvider = providerMap.get(provider.providerId);
        providerMap.set(provider.providerId, {
          providerId: provider.providerId,
          name: provider.name ?? currentProvider?.name ?? null,
          available: provider.available || currentProvider?.available === true,
        });
      }
      const current = get();
      const activeConversation = current.conversations.find(
        (conversation) => conversation.id === current.activeConversationId,
      );
      const routeModel = activeConversation
        ? models.find((model) => isSameRoute(model, activeConversation))
        : undefined;
      let projectPreference: DefaultModelPreference | null = null;
      let projectPreferenceInvalid = false;
      if (current.pendingProjectId !== null) {
        try {
          projectPreference = readProjectDefaultModelPreference(current.pendingProjectId);
        } catch {
          projectPreferenceInvalid = true;
        }
      }
      const defaultPreference = projectPreferenceInvalid ? null : (projectPreference ?? current.defaultModel);
      const defaultModel = defaultPreference
        ? models.find(
            (model) =>
              modelRouteKey(model) === formatModelRouteKey(defaultPreference) &&
              !current.hiddenModelKeys.includes(modelRouteKey(model)),
          )
        : undefined;
      if (!lastSelectedPreferenceRead) {
        try {
          lastSelectedPreference = readLastSelectedModelPreference();
        } catch (cause) {
          globalThis.reportError(new Error('The last selected model preference could not be read.', { cause }));
        }
        lastSelectedPreferenceRead = true;
      }
      const lastSelectedModelPreference = lastSelectedPreference;
      const lastSelectedModel = lastSelectedModelPreference
        ? models.find(
            (model) =>
              modelRouteKey(model) === formatModelRouteKey(lastSelectedModelPreference) &&
              !current.hiddenModelKeys.includes(modelRouteKey(model)),
          )
        : undefined;
      const preferredNewChatModel = projectPreference !== null ? defaultModel : (lastSelectedModel ?? defaultModel);
      const currentSelection = !current.activeConversationId
        ? models.find(
            (model) =>
              model.key === current.selectedModelKey && !current.hiddenModelKeys.includes(modelRouteKey(model)),
          )
        : undefined;
      const selectedModelKey =
        routeModel?.key ??
        currentSelection?.key ??
        (!current.activeConversationId && preferredNewChatModel ? preferredNewChatModel.key : null);
      const selectedModel = models.find((model) => model.key === selectedModelKey);
      const selectedReasoningEffort =
        current.activeConversationId || current.selectedModelKey === selectedModelKey
          ? current.selectedReasoningEffort
          : selectedModel?.supportsReasoning
            ? (current.modelReasoningEfforts[modelRouteKey(selectedModel)] ?? null)
            : null;
      const errorCode = projectPreferenceInvalid
        ? 'projectDefaultModelLoadFailed'
        : current.errorCode === 'modelRequired' && selectedModelKey !== null
          ? null
          : current.errorCode;
      const hasUnavailableSources =
        chatGptCatalogResult?.status === 'rejected' ||
        chatGptCatalog?.hasUnavailableSources === true ||
        localCatalogResult?.status === 'rejected' ||
        apiKeyCatalogResult?.status === 'rejected' ||
        apiKeyCatalog?.hasUnavailableSources === true;
      set({
        models,
        modelProviders: [...providerMap.values()],
        modelStatus: allCatalogsSettled ? 'ready' : 'loading',
        modelError: allCatalogsSettled && hasUnavailableSources,
        hasConfiguredProvider:
          chatGptCatalog?.hasConfiguredProvider === true || apiKeyCatalog?.hasConfiguredProvider === true,
        chatGptWorkspaceSelectionRequired: chatGptCatalog?.needsChatGptWorkspaceSelection === true,
        selectedModelKey,
        selectedReasoningEffort,
        errorCode,
      });
      if (allCatalogsSettled) void refreshOAuthRateLimits(models);
    };

    await loadModelCatalogsProgressively(
      {
        chatGpt: loadChatGptModels(localServiceClient, forceRefresh),
        local: loadLocalModels(localServiceClient),
        apiKey: (onProgress) => loadApiKeyModels(localServiceClient, forceRefresh, onProgress),
      },
      publishCatalogs,
    );
  };
}
