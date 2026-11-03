import { containsAsciiControlCharacters } from '@shared/lib/utils/textValidation';

export interface DefaultModelPreference {
  providerId: string;
  modelId: string;
  displayName: string;
  connectionId: string | null;
  workspaceId: string | null;
  apiKeyConnectionId: string | null;
}

export function parseDefaultModelPreference(value: unknown): DefaultModelPreference {
  if (
    !isObject(value) ||
    !isNonEmptyString(value.providerId) ||
    value.providerId.length > 128 ||
    !isNonEmptyString(value.modelId) ||
    value.modelId.length > 256 ||
    containsAsciiControlCharacters(value.providerId) ||
    containsAsciiControlCharacters(value.modelId)
  ) {
    throw new Error('The saved default model is invalid.');
  }
  const displayName =
    value.displayName === undefined ? value.modelId : isNonEmptyString(value.displayName) ? value.displayName : null;
  if (displayName === null || displayName.length > 256 || containsAsciiControlCharacters(displayName)) {
    throw new Error('The saved default model is invalid.');
  }
  return {
    providerId: value.providerId,
    modelId: value.modelId,
    displayName,
    connectionId: optionalString(value.connectionId),
    workspaceId: optionalString(value.workspaceId),
    apiKeyConnectionId: optionalString(value.apiKeyConnectionId),
  };
}

function optionalString(value: unknown): string | null {
  return value === undefined || value === null ? null : isNonEmptyString(value) ? value : null;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
