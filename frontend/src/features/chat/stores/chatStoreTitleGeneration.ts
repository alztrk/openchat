import type { JsonObject } from '@/shared/services/LocalServiceClient';
import { loadRouteCredentials } from '@/features/chat/data/modelRouteCredentials';
import type { ChatModel } from '@/features/chat/data/chatModels';
import { formatModelRouteKey, modelRouteKey } from '@/features/settings/data/modelPreferences';
import { readTitleGenerationPreference, type TitleGenerationPreference } from '@/features/settings/data/titleGenerationPreferences';

export async function loadTitleGenerationRequest(models: readonly ChatModel[]): Promise<JsonObject> {
  let preference: TitleGenerationPreference;
  try {
    preference = readTitleGenerationPreference();
  } catch {
    return { mode: 'disabled' };
  }
  if (preference.mode === 'conversation') return { mode: 'conversation' };
  if (!preference.model) return { mode: 'disabled' };

  const routeKey = formatModelRouteKey(preference.model);
  const model = models.find((candidate) => modelRouteKey(candidate) === routeKey);
  if (!model) return { mode: 'disabled' };

  try {
    const credentials = await loadRouteCredentials(model);
    return {
      mode: preference.mode,
      target: {
        providerId: model.providerId,
        modelId: model.modelId,
        ...(model.connectionId ? { connectionId: model.connectionId } : {}),
        ...(model.workspaceId ? { workspaceId: model.workspaceId } : {}),
        ...(model.apiKeyConnectionId ? { apiKeyConnectionId: model.apiKeyConnectionId } : {}),
        ...(typeof credentials.apiKey === 'string' ? { apiKey: credentials.apiKey } : {}),
        ...(typeof credentials.customProvider === 'object' && credentials.customProvider !== null
          ? { customProvider: credentials.customProvider }
          : {}),
      },
    };
  } catch {
    // Title generation is optional; the frontend disables it when its saved route or credential is unavailable.
    return { mode: 'disabled' };
  }
}
