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
import type { LocalServiceClient } from '@/platform/local-service';
import type { ChatState } from './chatStore';
import { isSameRoute } from './chatStoreUtils';

type ChatStoreSetState = StoreApi<ChatState>['setState'];
type ChatStoreGetState = StoreApi<ChatState>['getState'];

export function createRefreshModelsAction(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
  localServiceClient: LocalServiceClient,
): ChatState['refreshModels'] {
  return async (forceRefresh = false) => {
    set({ modelStatus: 'loading', modelError: false });
    const [chatGptCatalogResult, localCatalogResult, apiKeyCatalogResult] = await Promise.allSettled([
      loadChatGptModels(localServiceClient, forceRefresh),
      loadLocalModels(localServiceClient),
      loadApiKeyModels(localServiceClient, forceRefresh),
    ]);
    const chatGptCatalog = chatGptCatalogResult.status === 'fulfilled' ? chatGptCatalogResult.value : null;
    const localModels = localCatalogResult.status === 'fulfilled' ? localCatalogResult.value : [];
    const apiKeyCatalog = apiKeyCatalogResult.status === 'fulfilled' ? apiKeyCatalogResult.value : null;
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
    const routeModel = activeConversation ? models.find((model) => isSameRoute(model, activeConversation)) : undefined;
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
    let lastSelectedPreference: DefaultModelPreference | null = null;
    try {
      lastSelectedPreference = readLastSelectedModelPreference();
    } catch (cause) {
      globalThis.reportError(new Error('The last selected model preference could not be read.', { cause }));
    }
    const lastSelectedModel = lastSelectedPreference
      ? models.find(
          (model) =>
            modelRouteKey(model) === formatModelRouteKey(lastSelectedPreference) &&
            !current.hiddenModelKeys.includes(modelRouteKey(model)),
        )
      : undefined;
    const preferredNewChatModel = projectPreference !== null ? defaultModel : (lastSelectedModel ?? defaultModel);
    const currentSelection = !current.activeConversationId
      ? models.find(
          (model) => model.key === current.selectedModelKey && !current.hiddenModelKeys.includes(modelRouteKey(model)),
        )
      : undefined;
    const hasUnavailableSources =
      chatGptCatalogResult.status === 'rejected' ||
      chatGptCatalog?.hasUnavailableSources === true ||
      localCatalogResult.status === 'rejected' ||
      apiKeyCatalogResult.status === 'rejected' ||
      apiKeyCatalog?.hasUnavailableSources === true;
    set({
      models,
      modelProviders: [...providerMap.values()],
      modelStatus: 'ready',
      modelError: hasUnavailableSources,
      hasConfiguredProvider:
        chatGptCatalog?.hasConfiguredProvider === true || apiKeyCatalog?.hasConfiguredProvider === true,
      selectedModelKey:
        routeModel?.key ??
        currentSelection?.key ??
        (!current.activeConversationId && preferredNewChatModel ? preferredNewChatModel.key : null),
      errorCode: projectPreferenceInvalid ? 'projectDefaultModelLoadFailed' : current.errorCode,
    });
  };
}
