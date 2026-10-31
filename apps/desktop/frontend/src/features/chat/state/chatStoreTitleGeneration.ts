import type { ChatModel } from '@features/provider-settings/data/chatModels';
import { formatModelRouteKey, modelRouteKey } from '@features/provider-settings/data/modelPreferences';
import { loadRouteParameters } from '@features/provider-settings/data/modelRouteParameters';
import {
  readTitleGenerationPreference,
  type TitleGenerationPreference,
} from '@features/provider-settings/data/titleGenerationPreferences';
import type { JsonObject } from '@shared/lib/platform/local-service';

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
    const routeParameters = loadRouteParameters(model);
    return {
      mode: preference.mode,
      target: {
        providerId: model.providerId,
        modelId: model.modelId,
        ...(model.connectionId ? { connectionId: model.connectionId } : {}),
        ...(model.workspaceId ? { workspaceId: model.workspaceId } : {}),
        ...(typeof routeParameters.apiKeyConnectionId === 'string'
          ? { apiKeyConnectionId: routeParameters.apiKeyConnectionId }
          : {}),
      },
    };
  } catch {
    // Title generation is optional; the frontend disables it when its saved route or credential is unavailable.
    return { mode: 'disabled' };
  }
}
