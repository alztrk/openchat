import { invoke } from '@tauri-apps/api/core';

export const compatibleProviderIds = [
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
] as const;

export type CompatibleProviderId = (typeof compatibleProviderIds)[number];

export interface ChatGptApiKeySummary {
  id: string;
  keySuffix: string | null;
}

export interface CompatibleProviderCredentialSummary {
  providerId: CompatibleProviderId;
  isConfigured: boolean;
  keySuffix: string | null;
}

export interface CustomProviderSummary {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  isConfigured: boolean;
  keySuffix: string | null;
}

export interface ProviderCredentialSnapshot {
  chatgptApiKeys: ChatGptApiKeySummary[];
  compatibleProviders: CompatibleProviderCredentialSummary[];
  customProviders: CustomProviderSummary[];
  openCodeConfigured: boolean;
  openCodeKeySuffix: string | null;
}

export type ProviderCredentialRoute =
  | { providerId: 'chatgpt_api'; connectionId: string }
  | { providerId: 'opencode' }
  | { providerId: CompatibleProviderId }
  | { providerId: `custom_${string}` };

export async function loadProviderCredentialStatus(): Promise<ProviderCredentialSnapshot> {
  const value: unknown = await invoke('load_provider_credential_status');
  if (
    !isObject(value) ||
    !Array.isArray(value.chatgptApiKeys) ||
    !Array.isArray(value.compatibleProviders) ||
    !Array.isArray(value.customProviders)
  ) {
    throw new Error('The provider credential status is invalid.');
  }

  const chatgptApiKeys = value.chatgptApiKeys.flatMap((item): ChatGptApiKeySummary[] => {
    if (!isObject(item) || typeof item.id !== 'string' || !isNullableString(item.keySuffix)) return [];
    return [{ id: item.id, keySuffix: item.keySuffix }];
  });
  const compatibleProviders = value.compatibleProviders.flatMap((item): CompatibleProviderCredentialSummary[] => {
    if (
      !isObject(item) ||
      typeof item.providerId !== 'string' ||
      !isCompatibleProviderId(item.providerId) ||
      typeof item.isConfigured !== 'boolean' ||
      !isNullableString(item.keySuffix)
    ) {
      return [];
    }
    return [{
      providerId: item.providerId,
      isConfigured: item.isConfigured,
      keySuffix: item.keySuffix,
    }];
  });
  const customProviders = value.customProviders.flatMap((item): CustomProviderSummary[] => {
    if (
      !isObject(item) ||
      typeof item.id !== 'string' ||
      !isCustomProviderId(item.id) ||
      typeof item.name !== 'string' ||
      typeof item.baseUrl !== 'string' ||
      (item.protocol !== 'openAi' && item.protocol !== 'anthropic') ||
      typeof item.isConfigured !== 'boolean' ||
      !isNullableString(item.keySuffix)
    ) {
      return [];
    }
    return [{
      id: item.id,
      name: item.name,
      baseUrl: item.baseUrl,
      protocol: item.protocol,
      isConfigured: item.isConfigured,
      keySuffix: item.keySuffix,
    }];
  });
  if (typeof value.openCodeConfigured !== 'boolean' || !isNullableString(value.openCodeKeySuffix)) {
    throw new Error('The provider credential status is invalid.');
  }
  if (compatibleProviders.length !== compatibleProviderIds.length) {
    throw new Error('The provider credential catalog is incomplete.');
  }
  if (customProviders.length !== value.customProviders.length) {
    throw new Error('The custom provider catalog is invalid.');
  }
  return {
    chatgptApiKeys,
    compatibleProviders,
    customProviders,
    openCodeConfigured: value.openCodeConfigured,
    openCodeKeySuffix: value.openCodeKeySuffix,
  };
}

export async function getProviderApiKey(route: ProviderCredentialRoute): Promise<string | null> {
  const connectionId = route.providerId === 'chatgpt_api' ? route.connectionId : null;
  const value: unknown = await invoke('get_provider_api_key', {
    providerId: route.providerId,
    connectionId,
  });
  if (value === null) return null;
  if (typeof value !== 'string') throw new Error('The provider credential response is invalid.');
  return value;
}

export async function saveChatGptApiKey(connectionId: string, apiKey: string): Promise<void> {
  await invoke('save_chatgpt_api_key', { connectionId, apiKey });
}

export async function saveCompatibleProviderApiKey(
  providerId: CompatibleProviderId,
  apiKey: string,
): Promise<void> {
  await invoke('save_compatible_provider_api_key', { providerId, apiKey });
}

export async function saveOpenCodeApiKey(apiKey: string): Promise<void> {
  await invoke('save_opencode_api_key', { apiKey });
}

export async function saveCustomProvider(input: {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  apiKey: string;
}): Promise<CustomProviderSummary> {
  const value: unknown = await invoke('save_custom_provider', {
    ...input,
    apiKey: input.apiKey.trim().length > 0 ? input.apiKey : null,
  });
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    !isCustomProviderId(value.id) ||
    typeof value.name !== 'string' ||
    typeof value.baseUrl !== 'string' ||
    (value.protocol !== 'openAi' && value.protocol !== 'anthropic') ||
    typeof value.isConfigured !== 'boolean' ||
    !isNullableString(value.keySuffix)
  ) {
    throw new Error('The saved custom provider is invalid.');
  }
  return {
    id: value.id,
    name: value.name,
    baseUrl: value.baseUrl,
    protocol: value.protocol,
    isConfigured: value.isConfigured,
    keySuffix: value.keySuffix,
  };
}

export async function deleteCustomProvider(id: `custom_${string}`): Promise<void> {
  await invoke('delete_custom_provider', { id });
}

export async function deleteChatGptApiKey(connectionId: string): Promise<void> {
  await invoke('delete_chatgpt_api_key', { connectionId });
}

export async function deleteCompatibleProviderApiKey(providerId: CompatibleProviderId): Promise<void> {
  await invoke('delete_compatible_provider_api_key', { providerId });
}

export async function deleteOpenCodeApiKey(): Promise<void> {
  await invoke('delete_opencode_api_key');
}

function isCompatibleProviderId(value: string): value is CompatibleProviderId {
  return compatibleProviderIds.some((providerId) => providerId === value);
}

function isCustomProviderId(value: string): value is `custom_${string}` {
  return /^custom_[a-z0-9_]{12,57}$/.test(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}
