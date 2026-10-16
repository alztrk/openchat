import type { JsonObject } from '@/shared/services/LocalServiceClient';
import { compatibleProviderIds, type CustomProviderSummary } from '@/features/settings/data/providerCredentials';
import { isObject, numberOrNull } from '@/features/chat/stores/chatStoreUtils';
import type { ChatModel } from './chatModels';
import type { ChatRoute } from './chatStorage';

export function parseProviderCatalog(
  response: JsonObject,
  route: {
    providerId: string;
    connectionId?: string;
    sourceLabel?: string;
    customProvider?: Omit<CustomProviderSummary, 'isConfigured' | 'keySuffix'>;
  },
): { models: ChatModel[]; failed: boolean } {
  if (
    !Array.isArray(response.models) ||
    (response.freshness !== 'current' && response.freshness !== 'stale')
  ) {
    return { models: [], failed: true };
  }
  const models = response.models.flatMap((item): ChatModel[] => {
    if (
      !isObject(item) ||
      item.isAvailable !== true ||
      typeof item.id !== 'string' ||
      typeof item.displayName !== 'string'
    ) {
      return [];
    }
    const reasoningLevels = Array.isArray(item.reasoningLevels) &&
      item.reasoningLevels.every((level) => typeof level === 'string')
      ? item.reasoningLevels
      : [];
    const connectionId = route.connectionId;
    const apiKeyConnectionId = route.providerId === 'chatgpt_api'
      ? connectionId
      : compatibleProviderIds.some((providerId) => providerId === route.providerId)
        ? route.providerId
        : route.customProvider?.id;
    const chatRoute: ChatRoute = {
      providerId: route.providerId,
      modelId: item.id,
      ...(route.providerId === 'chatgpt' && connectionId ? { connectionId } : {}),
      ...(apiKeyConnectionId ? { apiKeyConnectionId } : {}),
    };
    return [{
      ...chatRoute,
      key: `${route.providerId}:${connectionId ?? ''}:${item.id}`,
      displayName: item.displayName,
      description: typeof item.description === 'string' ? item.description : null,
      supportsReasoning: item.supportsReasoning === true,
      supportsImages: item.supportsImages === true,
      supportsTools: typeof item.supportsTools === 'boolean' ? item.supportsTools : null,
      supportsFastMode: item.supportsFastMode === true,
      reasoningLevels,
      contextWindow: numberOrNull(item.contextWindow),
      sourceLabel: route.sourceLabel,
      groupId: typeof item.groupId === 'string' ? item.groupId : undefined,
      customProvider: route.customProvider,
      catalogStale: response.freshness === 'stale',
    }];
  });
  return { models, failed: false };
}
