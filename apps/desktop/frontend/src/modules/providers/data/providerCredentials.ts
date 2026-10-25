/** @file Maps provider credential operations to Tauri and exposes only redacted status to React. */
import { i18n } from '@/localization/i18n';
import { invoke } from '@/platform/tauri';

interface NativeHttpProviderDialogCopy {
  title: string;
  intro: string;
  warning: string;
  allow: string;
  cancel: string;
}

function getNativeHttpProviderDialogCopy(): NativeHttpProviderDialogCopy {
  return {
    title: i18n.t('nativeProviderHttpTitle'),
    intro: i18n.t('nativeProviderHttpIntro'),
    warning: i18n.t('nativeProviderHttpWarning'),
    allow: i18n.t('nativeProviderHttpAllow'),
    cancel: i18n.t('nativeProviderHttpCancel'),
  };
}

/** Provider identifiers accepted by the native credential commands. */
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
  'kilo',
] as const;

/** A supported provider whose API key is stored by the native host. */
export type CompatibleProviderId = (typeof compatibleProviderIds)[number];

/** Redacted status for one saved ChatGPT API connection key. */
export interface ChatGptApiKeySummary {
  id: string;
  keySuffix: string | null;
}

/** Redacted setup status for one built-in compatible provider. */
export interface CompatibleProviderCredentialSummary {
  providerId: CompatibleProviderId;
  isConfigured: boolean;
  keySuffix: string | null;
}

/** Redacted configuration and key status for a user-defined provider. */
export interface CustomProviderSummary {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  allowInsecureHttp: boolean;
  reasoningMode: 'none' | 'reasoningEffort' | 'chatTemplateThinking';
  reasoningLevels: string[];
  isConfigured: boolean;
  keySuffix: string | null;
}

/** Credential metadata returned to settings without exposing full key values. */
export interface ProviderCredentialSnapshot {
  chatgptApiKeys: ChatGptApiKeySummary[];
  compatibleProviders: CompatibleProviderCredentialSummary[];
  customProviders: CustomProviderSummary[];
  openCodeConfigured: boolean;
  openCodeKeySuffix: string | null;
}

/**
 * Loads the redacted credential status from the native host.
 *
 * @throws When the IPC response is malformed or the native host cannot read the protected store.
 */
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
    return [
      {
        providerId: item.providerId,
        isConfigured: item.isConfigured,
        keySuffix: item.keySuffix,
      },
    ];
  });
  const customProviders = value.customProviders.flatMap((item): CustomProviderSummary[] => {
    if (
      !isObject(item) ||
      typeof item.id !== 'string' ||
      !isCustomProviderId(item.id) ||
      typeof item.name !== 'string' ||
      typeof item.baseUrl !== 'string' ||
      (item.protocol !== 'openAi' && item.protocol !== 'anthropic') ||
      typeof item.allowInsecureHttp !== 'boolean' ||
      (item.reasoningMode !== 'none' &&
        item.reasoningMode !== 'reasoningEffort' &&
        item.reasoningMode !== 'chatTemplateThinking') ||
      !Array.isArray(item.reasoningLevels) ||
      !item.reasoningLevels.every((level) => typeof level === 'string') ||
      typeof item.isConfigured !== 'boolean' ||
      !isNullableString(item.keySuffix)
    ) {
      return [];
    }
    return [
      {
        id: item.id,
        name: item.name,
        baseUrl: item.baseUrl,
        protocol: item.protocol,
        allowInsecureHttp: item.allowInsecureHttp,
        reasoningMode: item.reasoningMode,
        reasoningLevels: item.reasoningLevels,
        isConfigured: item.isConfigured,
        keySuffix: item.keySuffix,
      },
    ];
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

/** Saves a ChatGPT API key under the selected connection identifier.
 *
 * The key is sent to the native host for protected storage and is never returned to the renderer.
 *
 * @param connectionId Stable identifier for the ChatGPT API connection.
 * @param apiKey The provider key supplied by the user.
 * @throws When the native host rejects the key or cannot write protected storage.
 */
export async function saveChatGptApiKey(connectionId: string, apiKey: string): Promise<void> {
  await invoke('save_chatgpt_api_key', { connectionId, apiKey });
}

/** Saves an API key for a supported OpenAI-compatible provider.
 *
 * The key is sent to the native host for protected storage and is never returned to the renderer.
 *
 * @param providerId Provider that owns the key.
 * @param apiKey The provider key supplied by the user.
 * @throws When the native host rejects the key or cannot write protected storage.
 */
export async function saveCompatibleProviderApiKey(providerId: CompatibleProviderId, apiKey: string): Promise<void> {
  await invoke('save_compatible_provider_api_key', { providerId, apiKey });
}

/** Saves the OpenCode API key through the native protected-storage command.
 *
 * @param apiKey The provider key supplied by the user.
 * @throws When the native host rejects the key or cannot write protected storage.
 */
export async function saveOpenCodeApiKey(apiKey: string): Promise<void> {
  await invoke('save_opencode_api_key', { apiKey });
}

/** Creates or updates a custom provider and returns its redacted setup status.
 *
 * An empty key leaves an existing key unchanged. The native host validates and stores the
 * configuration and optional key; the full key is not returned to the renderer.
 *
 * @param input User-entered custom-provider settings and optional API key.
 * @throws When the native host rejects the configuration or cannot write protected storage.
 */
export async function saveCustomProvider(input: {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  allowInsecureHttp: boolean;
  reasoningMode: 'none' | 'reasoningEffort' | 'chatTemplateThinking';
  reasoningLevels: string[];
  apiKey: string;
}): Promise<CustomProviderSummary> {
  const value: unknown = await invoke('save_custom_provider', {
    input: {
      ...input,
      reasoningLevels: input.reasoningLevels.join(','),
      apiKey: input.apiKey.trim().length > 0 ? input.apiKey : null,
      dialogCopy: getNativeHttpProviderDialogCopy(),
    },
  });
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    !isCustomProviderId(value.id) ||
    typeof value.name !== 'string' ||
    typeof value.baseUrl !== 'string' ||
    (value.protocol !== 'openAi' && value.protocol !== 'anthropic') ||
    typeof value.allowInsecureHttp !== 'boolean' ||
    (value.reasoningMode !== 'none' &&
      value.reasoningMode !== 'reasoningEffort' &&
      value.reasoningMode !== 'chatTemplateThinking') ||
    !Array.isArray(value.reasoningLevels) ||
    !value.reasoningLevels.every((level) => typeof level === 'string') ||
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
    allowInsecureHttp: value.allowInsecureHttp,
    reasoningMode: value.reasoningMode,
    reasoningLevels: value.reasoningLevels,
    isConfigured: value.isConfigured,
    keySuffix: value.keySuffix,
  };
}

/** Deletes a custom provider and its associated key from native protected storage. */
export async function deleteCustomProvider(id: `custom_${string}`): Promise<void> {
  await invoke('delete_custom_provider', { id });
}

/** Deletes the API key saved for one ChatGPT API connection. */
export async function deleteChatGptApiKey(connectionId: string): Promise<void> {
  await invoke('delete_chatgpt_api_key', { connectionId });
}

/** Deletes the saved API key for a supported compatible provider. */
export async function deleteCompatibleProviderApiKey(providerId: CompatibleProviderId): Promise<void> {
  await invoke('delete_compatible_provider_api_key', { providerId });
}

/** Deletes the saved OpenCode API key from native protected storage. */
export async function deleteOpenCodeApiKey(): Promise<void> {
  await invoke('delete_opencode_api_key');
}

/** Narrows an untrusted string to the built-in provider identifier union. */
function isCompatibleProviderId(value: string): value is CompatibleProviderId {
  return compatibleProviderIds.some((providerId) => providerId === value);
}

/** Checks the identifier format used for persisted custom-provider routes. */
function isCustomProviderId(value: string): value is `custom_${string}` {
  return /^custom_[a-z0-9_]{12,57}$/.test(value);
}

/** Narrows a non-null, non-array JSON value to a string-keyed object. */
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Narrows a JSON value to a string or explicit null. */
function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}
