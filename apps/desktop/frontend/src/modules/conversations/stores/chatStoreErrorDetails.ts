import { LocalServiceError } from '@/platform/local-service';

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
