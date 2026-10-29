import type { TFunction } from 'i18next';
import { LocalServiceError } from '@/platform/local-service';

const knownServiceErrorKeyMap: Record<string, string> = {
  'chat history could not be updated.': 'chatHistoryUnavailable',
  'chat history could not be read.': 'chatHistoryUnavailable',
  'chat context usage could not be measured.': 'chatContextUsageUnavailable',
  'local conversation storage is unavailable.': 'storageUnavailable',
  'database operation queue wait timed out': 'databaseQueueTimeout',
  'database operation queue is full': 'databaseQueueFull',
  'the model provider returned a response openchat could not read.': 'invalidProviderResponse',
  'the selected model is no longer available. choose another model.': 'modelUnavailable',
  'the requested conversation was not found.': 'conversationNotFound',
  'the requested message was not found.': 'messageNotFound',
  'the assistant response could not be saved.': 'messageSaveFailed',
  'the conversation could not be opened.': 'conversationUnavailable',
};

export function formatServiceErrorMessage(errorMessage: string | null | undefined, t: TFunction): string | null {
  if (!errorMessage) return null;
  const trimmed = errorMessage.trim();
  if (trimmed.length === 0) return null;

  const lookupKey = knownServiceErrorKeyMap[trimmed.toLowerCase()];
  if (lookupKey) {
    return t(lookupKey, { defaultValue: lookupKey });
  }

  for (const [pattern, key] of Object.entries(knownServiceErrorKeyMap)) {
    if (trimmed.toLowerCase().includes(pattern)) {
      return t(key, { defaultValue: key });
    }
  }

  return trimmed;
}

export function isRedundantHistoryError(errorMessage: string | null | undefined): boolean {
  if (!errorMessage) return true;
  const lower = errorMessage.trim().toLowerCase();
  return (
    lower.includes('chat history could not be updated') ||
    lower.includes('chat history could not be read') ||
    lower.includes('storage_unavailable')
  );
}

export function getOpenCodeRetryAtUnixMs(error: unknown, providerId: string): number | null {
  if (
    providerId !== 'opencode' ||
    !(error instanceof LocalServiceError) ||
    (error.code !== 'rate_limited' && error.code !== 'opencode_free_usage_limit')
  ) {
    return null;
  }

  const match = /(?:^|[;\s])retry-after-seconds=(\d+)(?=$|[;\s])/u.exec(error.details ?? '');
  if (!match) return null;

  const retryAfterSeconds = Number(match[1]);
  if (!Number.isSafeInteger(retryAfterSeconds) || retryAfterSeconds < 0) return null;

  const retryAtUnixMs = Date.now() + retryAfterSeconds * 1000;
  if (!Number.isSafeInteger(retryAtUnixMs) || retryAtUnixMs > 8_640_000_000_000_000) return null;
  return retryAtUnixMs;
}
