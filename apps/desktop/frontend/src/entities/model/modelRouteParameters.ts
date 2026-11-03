import { type JsonObject, LocalServiceError } from '@shared/lib/platform/local-service';
import type { ChatModel } from './types';

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
  if (model.apiKeyConnectionId) {
    return { apiKeyConnectionId: model.apiKeyConnectionId };
  }
  if (model.customProvider) {
    return {
      apiKeyConnectionId: model.customProvider.id,
    };
  }
  return {};
}
