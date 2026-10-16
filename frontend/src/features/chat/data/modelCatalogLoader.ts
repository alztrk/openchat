import {
  LocalServiceClient,
  type JsonObject,
} from '@/shared/services/LocalServiceClient';
import {
  getProviderApiKey,
  loadProviderCredentialStatus,
  type CompatibleProviderId,
  type CustomProviderSummary,
  type ProviderCredentialSnapshot,
} from '@/features/settings/data/providerCredentials';
import { parseProviderCatalog } from './modelCatalogParser';
import type { ChatModel, ChatModelProvider } from './chatModels';
import type { ChatRoute } from './chatStorage';
import { isObject, numberOrNull } from '@/features/chat/stores/chatStoreUtils';

export interface ModelCatalogBatch {
  models: ChatModel[];
  providers: ChatModelProvider[];
  hasUnavailableSources: boolean;
  hasConfiguredProvider: boolean;
}

export async function loadChatGptModels(
  client: LocalServiceClient,
  forceRefresh: boolean,
): Promise<ModelCatalogBatch> {
  let connectionResult: JsonObject;
  try {
    connectionResult = await client.call('chatgpt.connections.list');
  } catch {
    return { models: [], providers: [], hasUnavailableSources: true, hasConfiguredProvider: false };
  }
  if (!Array.isArray(connectionResult.connections)) {
    return { models: [], providers: [], hasUnavailableSources: true, hasConfiguredProvider: false };
  }
  const connections = connectionResult.connections.filter(isObject);
  const connection = connections.find(
    (item) => item.authStatus === 'active' && item.isSelected === true,
  );
  if (!connection || typeof connection.id !== 'string' || !Array.isArray(connection.workspaces)) {
    return { models: [], providers: [], hasUnavailableSources: false, hasConfiguredProvider: false };
  }
  const providers: ChatModelProvider[] = [{ providerId: 'chatgpt', name: null, available: true }];
  const connectionId = connection.id;
  const workspace = connection.workspaces
    .filter(isObject)
    .find((item) => item.isSelected === true);
  if (!workspace || typeof workspace.id !== 'string') {
    return { models: [], providers, hasUnavailableSources: false, hasConfiguredProvider: true };
  }
  const workspaceId = workspace.id;

  let modelResult: JsonObject;
  try {
    modelResult = await client.call('chatgpt.models.list', {
      connectionId,
      workspaceId,
      ...(forceRefresh ? { forceRefresh: true } : {}),
    });
  } catch {
    return { models: [], providers, hasUnavailableSources: true, hasConfiguredProvider: true };
  }
  if (
    !Array.isArray(modelResult.models) ||
    (modelResult.freshness !== 'current' && modelResult.freshness !== 'stale')
  ) {
    return { models: [], providers, hasUnavailableSources: true, hasConfiguredProvider: true };
  }

  const catalogStale = modelResult.freshness === 'stale';
  const models = modelResult.models
    .filter(isObject)
    .filter((model) => model.isAvailable === true)
    .flatMap((model): ChatModel[] => {
      if (
        typeof model.id !== 'string' ||
        typeof model.displayName !== 'string' ||
        !Array.isArray(model.reasoningLevels) ||
        !model.reasoningLevels.every((level) => typeof level === 'string')
      ) {
        return [];
      }
      const route: ChatRoute = {
        providerId: 'chatgpt',
        modelId: model.id,
        connectionId,
        workspaceId,
      };
      return [{
        ...route,
        key: `${route.providerId}:${route.connectionId}:${route.workspaceId}:${route.modelId}`,
        displayName: model.displayName,
        description: typeof model.description === 'string' ? model.description : null,
        supportsReasoning: model.supportsReasoning === true,
        supportsImages: model.supportsImages === true,
        supportsTools: typeof model.supportsTools === 'boolean' ? model.supportsTools : null,
        supportsFastMode: model.supportsFastMode === true,
        reasoningLevels: model.reasoningLevels,
        contextWindow: numberOrNull(model.contextWindow),
        catalogStale,
      }];
    });
  return { models, providers, hasUnavailableSources: false, hasConfiguredProvider: true };
}

export async function loadLocalModels(client: LocalServiceClient): Promise<ChatModel[]> {
  const response = await client.call('local.engines.chat_models.list', {
    engineId: 'llama_cpp',
  });
  if (!Array.isArray(response.models)) {
    throw new Error('The local model catalog is invalid.');
  }
  return response.models
    .filter(isObject)
    .filter((model) => model.isAvailable === true)
    .flatMap((model): ChatModel[] => {
      if (typeof model.id !== 'string' || typeof model.displayName !== 'string') return [];
      const route: ChatRoute = { providerId: 'llama_cpp', modelId: model.id };
      return [{
        ...route,
        key: `${route.providerId}:${route.modelId}`,
        displayName: model.displayName,
        description: typeof model.description === 'string' ? model.description : null,
        supportsReasoning: model.supportsReasoning === true,
        supportsImages: model.supportsImages === true,
        supportsTools: typeof model.supportsTools === 'boolean' ? model.supportsTools : null,
        supportsFastMode: false,
        reasoningLevels: [],
        contextWindow: numberOrNull(model.contextWindow),
        catalogStale: false,
      }];
    });
}

export async function loadApiKeyModels(
  client: LocalServiceClient,
  forceRefresh: boolean,
): Promise<ModelCatalogBatch> {
  let status: ProviderCredentialSnapshot;
  try {
    status = await loadProviderCredentialStatus();
  } catch {
    return {
      models: [],
      providers: [],
      hasUnavailableSources: true,
      hasConfiguredProvider: false,
    };
  }
  const hasConfiguredProvider =
    status.chatgptApiKeys.length > 0 ||
    status.compatibleProviders.some((provider) => provider.isConfigured) ||
    status.customProviders.some((provider) => provider.isConfigured) ||
    status.openCodeConfigured;
  const tasks: Array<Promise<{ models: ChatModel[]; failed: boolean }>> = [];
  const providers: ChatModelProvider[] = [
    { providerId: 'opencode', name: null, available: true },
    ...(status.chatgptApiKeys.length > 0
      ? [{ providerId: 'chatgpt', name: null, available: true }]
      : []),
    ...status.compatibleProviders.map((provider) => ({
      providerId: provider.providerId,
      name: null,
      available: provider.isConfigured,
    })),
    ...status.customProviders
      .filter((provider) => provider.isConfigured)
      .map((provider) => ({
        providerId: provider.id,
        name: provider.name,
        available: true,
      })),
  ];

  for (const connection of status.chatgptApiKeys) {
    tasks.push(loadProviderCatalog(client, {
      providerId: 'chatgpt_api',
      connectionId: connection.id,
      sourceLabel: status.chatgptApiKeys.length > 1 && connection.keySuffix
        ? `••••${connection.keySuffix}`
        : undefined,
    }, forceRefresh));
  }

  for (const provider of status.compatibleProviders) {
    if (!provider.isConfigured) continue;
    tasks.push(loadProviderCatalog(client, {
      providerId: provider.providerId,
      connectionId: provider.providerId,
    }, forceRefresh));
  }

  for (const provider of status.customProviders) {
    tasks.push(loadCustomProviderCatalog(client, provider, forceRefresh));
  }

  tasks.push(loadOpenCodeFreeCatalog(client, forceRefresh));
  const results = await Promise.all(tasks);
  return {
    models: results.flatMap((result) => result.models),
    providers,
    hasUnavailableSources: results.some((result) => result.failed),
    hasConfiguredProvider,
  };
}

async function loadProviderCatalog(
  client: LocalServiceClient,
  route: {
    providerId: 'chatgpt_api' | CompatibleProviderId;
    connectionId: string;
    sourceLabel?: string;
  },
  forceRefresh: boolean,
): Promise<{ models: ChatModel[]; failed: boolean }> {
  try {
    const apiKey = await getProviderApiKey(route.providerId === 'chatgpt_api'
      ? { providerId: 'chatgpt_api', connectionId: route.connectionId }
      : { providerId: route.providerId });
    if (!apiKey) return { models: [], failed: true };
    const response = route.providerId === 'chatgpt_api'
      ? await client.call('chatgpt.api.models.list', { apiKey, ...(forceRefresh ? { forceRefresh: true } : {}) })
      : await client.call('compatible.models.list', {
          providerId: route.providerId,
          apiKey,
          ...(forceRefresh ? { forceRefresh: true } : {}),
        });
    return parseProviderCatalog(response, route);
  } catch {
    return { models: [], failed: true };
  }
}

async function loadOpenCodeFreeCatalog(
  client: LocalServiceClient,
  forceRefresh: boolean,
): Promise<{ models: ChatModel[]; failed: boolean }> {
  try {
    const response = await client.call('opencode.models.list', forceRefresh ? { forceRefresh: true } : {});
    const result = parseProviderCatalog(response, { providerId: 'opencode' });
    return {
      ...result,
      models: result.models.filter((model) => model.groupId === 'free'),
    };
  } catch {
    return { models: [], failed: true };
  }
}

async function loadCustomProviderCatalog(
  client: LocalServiceClient,
  provider: CustomProviderSummary,
  forceRefresh: boolean,
): Promise<{ models: ChatModel[]; failed: boolean }> {
  try {
    const apiKey = await getProviderApiKey({ providerId: provider.id });
    const response = await client.call('compatible.models.list', {
      providerId: provider.id,
      baseUrl: provider.baseUrl,
      protocol: provider.protocol,
      ...(apiKey ? { apiKey } : {}),
      ...(forceRefresh ? { forceRefresh: true } : {}),
    });
    return parseProviderCatalog(response, {
      providerId: provider.id,
      connectionId: provider.id,
      sourceLabel: provider.name,
      customProvider: {
        id: provider.id,
        name: provider.name,
        baseUrl: provider.baseUrl,
        protocol: provider.protocol,
      },
    });
  } catch {
    return { models: [], failed: true };
  }
}
