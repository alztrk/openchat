import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { containsAsciiControlCharacters } from '@/shared/utils/textValidation';

import type { ChatModel } from './chatModels';

const defaultModelKey = 'models.default_model';
const lastSelectedModelKey = 'models.last_selected_model';
const hiddenModelKeysKey = 'models.hidden_keys';
const modelContextLimitsKey = 'models.context_limits';
const apiKeyProviderIds = new Set([
  'chatgpt_api',
  'gemini',
  'groq',
  'cerebras',
  'openrouter',
  'mistral',
  'deepseek',
  'xai',
  'perplexity',
  'together',
  'sambanova',
  'anthropic',
  'fireworks',
  'deepinfra',
  'dashscope',
  'cohere',
]);

export interface DefaultModelPreference {
  providerId: string;
  modelId: string;
  displayName: string;
  connectionId: string | null;
  workspaceId: string | null;
  apiKeyConnectionId: string | null;
}

export interface ModelPreferencesSnapshot {
  defaultModel: DefaultModelPreference | null;
  hiddenModelKeys: string[];
  contextLimits: Record<string, number>;
  defaultModelInvalid: boolean;
  hiddenModelKeysInvalid: boolean;
  contextLimitsInvalid: boolean;
}

export function readModelPreferences(): ModelPreferencesSnapshot {
  let defaultModel: DefaultModelPreference | null = null;
  let hiddenModelKeys: string[] = [];
  let contextLimits: Record<string, number> = {};
  let defaultModelInvalid = false;
  let hiddenModelKeysInvalid = false;
  let contextLimitsInvalid = false;

  try {
    const raw = unifiedStorage.getItem(defaultModelKey);
    defaultModel = raw === null ? null : parseDefaultModelPreference(JSON.parse(raw));
  } catch {
    defaultModelInvalid = true;
  }

  try {
    const raw = unifiedStorage.getItem(hiddenModelKeysKey);
    hiddenModelKeys = raw === null ? [] : parseHiddenModelKeys(JSON.parse(raw));
  } catch {
    hiddenModelKeysInvalid = true;
  }

  try {
    const raw = unifiedStorage.getItem(modelContextLimitsKey);
    contextLimits = raw === null ? {} : parseContextLimits(JSON.parse(raw));
  } catch {
    contextLimitsInvalid = true;
  }

  return {
    defaultModel,
    hiddenModelKeys,
    contextLimits,
    defaultModelInvalid,
    hiddenModelKeysInvalid,
    contextLimitsInvalid,
  };
}

export function writeDefaultModelPreference(preference: DefaultModelPreference | null): void {
  if (preference === null) {
    unifiedStorage.removeItem(defaultModelKey);
    return;
  }
  unifiedStorage.setItem(defaultModelKey, JSON.stringify(parseDefaultModelPreference(preference)));
}

export function readLastSelectedModelPreference(): DefaultModelPreference | null {
  const raw = unifiedStorage.getItem(lastSelectedModelKey);
  return raw === null ? null : parseDefaultModelPreference(JSON.parse(raw));
}

export function writeLastSelectedModelPreference(preference: DefaultModelPreference): void {
  unifiedStorage.setItem(lastSelectedModelKey, JSON.stringify(parseDefaultModelPreference(preference)));
}

export function writeHiddenModelKeys(keys: readonly string[]): void {
  const normalized = [...new Set(keys)];
  if (
    normalized.length > 10_000 ||
    normalized.some((key) => !key || key.trim() !== key || key.length > 1024 || key.includes('\0'))
  ) {
    throw new Error('The hidden model preferences are invalid.');
  }
  if (normalized.length === 0) {
    unifiedStorage.removeItem(hiddenModelKeysKey);
    return;
  }
  unifiedStorage.setItem(hiddenModelKeysKey, JSON.stringify(normalized));
}

export function clearHiddenModelKeys(): void {
  unifiedStorage.removeItem(hiddenModelKeysKey);
}

export function writeModelContextLimit(routeKey: string, limit: number | null): void {
  if (!routeKey || routeKey.trim() !== routeKey || routeKey.length > 1024 || routeKey.includes('\0')) {
    throw new Error('The model route is invalid.');
  }
  if (limit !== null && (!Number.isSafeInteger(limit) || limit < 2048 || limit > 2_000_000)) {
    throw new Error('The model context limit is invalid.');
  }
  const limits = parseContextLimits(JSON.parse(unifiedStorage.getItem(modelContextLimitsKey) ?? '{}'));
  if (limit === null) delete limits[routeKey];
  else limits[routeKey] = limit;
  if (Object.keys(limits).length === 0) unifiedStorage.removeItem(modelContextLimitsKey);
  else unifiedStorage.setItem(modelContextLimitsKey, JSON.stringify(limits));
}

export function clearModelContextLimits(): void {
  unifiedStorage.removeItem(modelContextLimitsKey);
}

export function modelRouteKey(model: ChatModel): string {
  const providerId = model.providerId;
  const apiKeyRoute = apiKeyProviderIds.has(providerId) || /^custom_[a-z0-9_]{12,64}$/.test(providerId);
  const connectionId =
    providerId === 'chatgpt'
      ? model.connectionId
      : providerId === 'chatgpt_api'
        ? (model.apiKeyConnectionId ?? model.connectionId)
        : apiKeyRoute
          ? (model.apiKeyConnectionId ?? model.connectionId ?? providerId)
          : undefined;
  const workspaceId = providerId === 'chatgpt' ? model.workspaceId : undefined;
  return `${providerId}:${connectionId ?? ''}:${workspaceId ?? ''}:${model.modelId}`;
}

export function defaultPreferenceForModel(model: ChatModel): DefaultModelPreference {
  return {
    providerId: model.providerId,
    modelId: model.modelId,
    displayName: model.displayName,
    connectionId: model.connectionId ?? null,
    workspaceId: model.workspaceId ?? null,
    apiKeyConnectionId: model.apiKeyConnectionId ?? null,
  };
}

export function formatModelRouteKey(preference: DefaultModelPreference): string {
  const providerId = preference.providerId;
  const connectionId =
    providerId === 'chatgpt'
      ? preference.connectionId
      : providerId === 'chatgpt_api'
        ? (preference.apiKeyConnectionId ?? preference.connectionId)
        : apiKeyProviderIds.has(providerId) || /^custom_[a-z0-9_]{12,64}$/.test(providerId)
          ? (preference.apiKeyConnectionId ?? preference.connectionId ?? providerId)
          : undefined;
  const workspaceId = providerId === 'chatgpt' ? preference.workspaceId : undefined;
  return `${providerId}:${connectionId ?? ''}:${workspaceId ?? ''}:${preference.modelId}`;
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

function parseHiddenModelKeys(value: unknown): string[] {
  if (
    !Array.isArray(value) ||
    value.length > 10_000 ||
    !value.every((key) => typeof key === 'string' && key.length > 0 && key.trim() === key && key.length <= 1024) ||
    new Set(value).size !== value.length
  ) {
    throw new Error('The saved hidden model keys are invalid.');
  }
  return value;
}

function parseContextLimits(value: unknown): Record<string, number> {
  if (!isObject(value) || Object.keys(value).length > 10_000) {
    throw new Error('The saved model context limits are invalid.');
  }
  const limits: Record<string, number> = {};
  for (const [key, limit] of Object.entries(value)) {
    if (!key || key.length > 1024 || !Number.isSafeInteger(limit) || typeof limit !== 'number' || limit < 2048) {
      throw new Error('The saved model context limits are invalid.');
    }
    limits[key] = limit;
  }
  return limits;
}

function optionalString(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value === 'string' && value.length <= 128 && !containsAsciiControlCharacters(value)) return value;
  throw new Error('The saved default model is invalid.');
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
