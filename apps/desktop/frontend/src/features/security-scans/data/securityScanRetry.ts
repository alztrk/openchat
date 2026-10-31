const MAX_PROVIDER_RETRY_AFTER_SECONDS = 24 * 60 * 60;

export function retryDelayFromProviderDetails(details: string | null): number | null {
  if (!details) return null;

  const entries = [...details.matchAll(/(?:^|;\s*)retry-after-seconds=(\d+)(?=;|$)/g)];
  if (entries.length !== 1) return null;

  const seconds = Number(entries[0]?.[1]);
  if (!Number.isSafeInteger(seconds) || seconds < 1 || seconds > MAX_PROVIDER_RETRY_AFTER_SECONDS) {
    return null;
  }

  return seconds * 1000;
}
