import {
  LocalServiceError,
  type JsonObject,
} from '@/shared/services/LocalServiceClient';
import {
  compatibleProviderIds,
  getProviderApiKey,
  type CompatibleProviderId,
} from '@/features/settings/data/providerCredentials';
import type { ChatModel } from './chatModels';

export async function loadRouteCredentials(model: ChatModel): Promise<JsonObject> {
  if (model.providerId === 'chatgpt_api') {
    const connectionId = model.apiKeyConnectionId;
    if (!connectionId) {
      throw new LocalServiceError('provider_credentials_missing', 'The API key route is incomplete.');
    }
    const apiKey = await getProviderApiKey({ providerId: 'chatgpt_api', connectionId });
    if (!apiKey) {
      throw new LocalServiceError('provider_credentials_missing', 'The API key is not available.');
    }
    return { apiKey, apiKeyConnectionId: connectionId };
  }
  if (isCompatibleProviderId(model.providerId)) {
    const providerId = model.providerId;
    const apiKey = await getProviderApiKey({ providerId });
    if (!apiKey) {
      throw new LocalServiceError('provider_credentials_missing', 'The API key is not available.');
    }
    return { apiKey, apiKeyConnectionId: providerId };
  }
  if (model.customProvider) {
    const apiKey = await getProviderApiKey({ providerId: model.customProvider.id });
    return {
      ...(apiKey ? { apiKey } : {}),
      apiKeyConnectionId: model.customProvider.id,
      customProvider: {
        id: model.customProvider.id,
        name: model.customProvider.name,
        baseUrl: model.customProvider.baseUrl,
        protocol: model.customProvider.protocol,
      },
    };
  }
  if (model.providerId === 'opencode') {
    const apiKey = await getProviderApiKey({ providerId: 'opencode' });
    return apiKey ? { apiKey } : {};
  }
  return {};
}

function isCompatibleProviderId(value: string): value is CompatibleProviderId {
  return compatibleProviderIds.some((providerId) => providerId === value);
}
