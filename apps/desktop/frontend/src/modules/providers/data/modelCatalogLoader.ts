import type { JsonObject, LocalServiceClient } from '@/platform/local-service';
import { isObject, numberOrNull } from '@/shared/utils/json';
import { requiresChatGptWorkspaceSelection } from './chatGptWorkspaceSelection';
import type { ChatModel, ChatModelProvider } from './chatModels';
import { parseProviderCatalog } from './modelCatalogParser';
import {
  type CompatibleProviderId,
  type CustomProviderSummary,
  loadProviderCredentialStatus,
  type ProviderCredentialSnapshot,
} from './providerCredentials';
import type { ChatRoute } from './routes';

const copilotModelCatalogTimeoutMs = 95_000;

export interface ModelCatalogBatch {
  models: ChatModel[];
  providers: ChatModelProvider[];
  hasUnavailableSources: boolean;
  hasConfiguredProvider: boolean;
  needsChatGptWorkspaceSelection: boolean;
}

export async function loadChatGptModels(client: LocalServiceClient, forceRefresh: boolean): Promise<ModelCatalogBatch> {
  let connectionResult: JsonObject;
  try {
    connectionResult = await client.call('chatgpt.connections.list');
  } catch {
    return {
      models: [],
      providers: [],
      hasUnavailableSources: true,
      hasConfiguredProvider: false,
      needsChatGptWorkspaceSelection: false,
    };
  }
  if (!Array.isArray(connectionResult.connections)) {
    return {
      models: [],
      providers: [],
      hasUnavailableSources: true,
      hasConfiguredProvider: false,
      needsChatGptWorkspaceSelection: false,
    };
  }
  const connections = connectionResult.connections.filter(isObject);
  const connection = connections.find((item) => item.authStatus === 'active' && item.isSelected === true);
  if (!connection || typeof connection.id !== 'string' || !Array.isArray(connection.workspaces)) {
    return {
      models: [],
      providers: [],
      hasUnavailableSources: false,
      hasConfiguredProvider: false,
      needsChatGptWorkspaceSelection: false,
    };
  }
  const providers: ChatModelProvider[] = [{ providerId: 'chatgpt', name: null, available: true }];
  const connectionId = connection.id;
  const workspaces = connection.workspaces.filter(isObject);
  const workspace = workspaces.find((item) => item.isSelected === true);
  if (!workspace || typeof workspace.id !== 'string') {
    return {
      models: [],
      providers,
      hasUnavailableSources: false,
      hasConfiguredProvider: true,
      needsChatGptWorkspaceSelection: requiresChatGptWorkspaceSelection(workspaces),
    };
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
    return {
      models: [],
      providers,
      hasUnavailableSources: true,
      hasConfiguredProvider: true,
      needsChatGptWorkspaceSelection: false,
    };
  }
  if (
    !Array.isArray(modelResult.models) ||
    (modelResult.freshness !== 'current' && modelResult.freshness !== 'stale')
  ) {
    return {
      models: [],
      providers,
      hasUnavailableSources: true,
      hasConfiguredProvider: true,
      needsChatGptWorkspaceSelection: false,
    };
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
      return [
        {
          ...route,
          key: `${route.providerId}:${route.connectionId}:${route.workspaceId}:${route.modelId}`,
          displayName: model.displayName,
          description: typeof model.description === 'string' ? model.description : null,
          supportsReasoning: model.reasoningLevels.length > 0,
          supportsImages: model.supportsImages === true,
          supportsTools: typeof model.supportsTools === 'boolean' ? model.supportsTools : null,
          supportsFastMode: model.supportsFastMode === true,
          reasoningLevels: model.reasoningLevels,
          contextWindow: numberOrNull(model.contextWindow),
          catalogStale,
        },
      ];
    });
  return {
    models,
    providers,
    hasUnavailableSources: false,
    hasConfiguredProvider: true,
    needsChatGptWorkspaceSelection: false,
  };
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
      return [
        {
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
        },
      ];
    });
}

export async function loadApiKeyModels(client: LocalServiceClient, forceRefresh: boolean): Promise<ModelCatalogBatch> {
  let status: ProviderCredentialSnapshot;
  try {
    status = await loadProviderCredentialStatus();
  } catch {
    return {
      models: [],
      providers: [],
      hasUnavailableSources: true,
      hasConfiguredProvider: false,
      needsChatGptWorkspaceSelection: false,
    };
  }
  const hasConfiguredProvider =
    status.chatgptApiKeys.length > 0 ||
    status.compatibleProviders.some((provider) => provider.isConfigured) ||
    status.customProviders.some((provider) => provider.isConfigured) ||
    status.openCodeConfigured;
  const tasks: Array<Promise<{ models: ChatModel[]; failed: boolean }>> = [];
  let copilotConnected = false;
  let copilotStatusFailed = false;
  try {
    const copilotStatus = await client.call('copilot.oauth.status');
    copilotConnected =
      copilotStatus.connected === true &&
      isObject(copilotStatus.connection) &&
      copilotStatus.connection.authStatus === 'active';
  } catch {
    copilotStatusFailed = true;
  }
  const kiloConfigured =
    status.compatibleProviders.find((provider) => provider.providerId === 'kilo')?.isConfigured ?? false;
  const providers: ChatModelProvider[] = [
    { providerId: 'opencode', name: null, available: true },
    { providerId: 'kilo', name: null, available: true },
    { providerId: 'copilot', name: null, available: copilotConnected },
    ...(status.chatgptApiKeys.length > 0 ? [{ providerId: 'chatgpt', name: null, available: true }] : []),
    ...status.compatibleProviders
      .filter((provider) => provider.providerId !== 'kilo')
      .map((provider) => ({
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
    tasks.push(
      loadProviderCatalog(
        client,
        {
          providerId: 'chatgpt_api',
          connectionId: connection.id,
          sourceLabel:
            status.chatgptApiKeys.length > 1 && connection.keySuffix ? `••••${connection.keySuffix}` : undefined,
        },
        forceRefresh,
      ),
    );
  }

  for (const provider of status.compatibleProviders) {
    if (!provider.isConfigured) continue;
    tasks.push(
      loadProviderCatalog(
        client,
        {
          providerId: provider.providerId,
          connectionId: provider.providerId,
        },
        forceRefresh,
      ),
    );
  }

  if (copilotConnected) {
    tasks.push(
      loadProviderCatalog(
        client,
        {
          providerId: 'copilot',
          connectionId: 'copilot',
        },
        forceRefresh,
      ),
    );
  }

  for (const provider of status.customProviders) {
    tasks.push(loadCustomProviderCatalog(client, provider, forceRefresh));
  }

  tasks.push(loadOpenCodeFreeCatalog(client, forceRefresh));
  if (!kiloConfigured) {
    tasks.push(loadKiloFreeCatalog(client, forceRefresh));
  }
  const results = await Promise.all(tasks);
  return {
    models: results.flatMap((result) => result.models),
    providers,
    hasUnavailableSources: copilotStatusFailed || results.some((result) => result.failed),
    hasConfiguredProvider: hasConfiguredProvider || copilotConnected,
    needsChatGptWorkspaceSelection: false,
  };
}

async function loadProviderCatalog(
  client: LocalServiceClient,
  route: {
    providerId: 'chatgpt_api' | 'copilot' | CompatibleProviderId;
    connectionId: string;
    sourceLabel?: string;
  },
  forceRefresh: boolean,
): Promise<{ models: ChatModel[]; failed: boolean }> {
  try {
    let response: JsonObject;
    if (route.providerId === 'copilot') {
      response = await client.call(
        'compatible.models.list',
        {
          providerId: route.providerId,
          ...(forceRefresh ? { forceRefresh: true } : {}),
        },
        copilotModelCatalogTimeoutMs,
      );
    } else {
      response =
        route.providerId === 'chatgpt_api'
          ? await client.call('chatgpt.api.models.list', {
              connectionId: route.connectionId,
              ...(forceRefresh ? { forceRefresh: true } : {}),
            })
          : await client.call('compatible.models.list', {
              providerId: route.providerId,
              ...(forceRefresh ? { forceRefresh: true } : {}),
            });
    }
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

async function loadKiloFreeCatalog(
  client: LocalServiceClient,
  forceRefresh: boolean,
): Promise<{ models: ChatModel[]; failed: boolean }> {
  try {
    const response = await client.call('compatible.models.list', {
      providerId: 'kilo',
      ...(forceRefresh ? { forceRefresh: true } : {}),
    });
    const result = parseProviderCatalog(response, { providerId: 'kilo', connectionId: 'kilo' });
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
    const response = await client.call('compatible.models.list', {
      providerId: provider.id,
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
        allowInsecureHttp: provider.allowInsecureHttp,
        reasoningEfforts: provider.reasoningEfforts,
        enableThinking: provider.enableThinking,
      },
    });
  } catch {
    return { models: [], failed: true };
  }
}
