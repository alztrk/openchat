import { type JsonObject, LocalServiceError } from '@/platform/local-service';
import type { ChatModel } from './chatModels';
import { type CompatibleProviderId, compatibleProviderIds } from './providerCredentials';

export function loadRouteParameters(model: ChatModel): JsonObject {
  if (model.providerId === 'copilot') {
    return { apiKeyConnectionId: 'copilot' };
  }
  if (model.providerId === 'chatgpt_api') {
    const connectionId = model.apiKeyConnectionId;
    if (!connectionId) {
      throw new LocalServiceError('provider_credentials_missing', 'The API key route is incomplete.');
    }
    return { apiKeyConnectionId: connectionId };
  }
  if (isCompatibleProviderId(model.providerId)) {
    return { apiKeyConnectionId: model.providerId };
  }
  if (model.customProvider) {
    return {
      apiKeyConnectionId: model.customProvider.id,
    };
  }
  return {};
}

function isCompatibleProviderId(value: string): value is CompatibleProviderId {
  return compatibleProviderIds.some((providerId) => providerId === value);
}
