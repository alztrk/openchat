import { isObject } from '@shared/lib/utils/json';

/** ChatGPT credit availability for a workspace; `balance` is null when the provider omits it. */
export interface UsageCredits {
  hasCredits: boolean;
  unlimited: boolean;
  balance: number | null;
}

type UsageCreditsDisplay =
  | { kind: 'hidden' }
  | { kind: 'unlimited' }
  | { kind: 'unavailable' }
  | { kind: 'balance'; balance: number };

/** Validates the credits payload and throws when a present value is malformed. */
export function parseUsageCredits(value: unknown): UsageCredits | null {
  if (value === null || value === undefined) return null;
  if (
    !isObject(value) ||
    typeof value.hasCredits !== 'boolean' ||
    typeof value.unlimited !== 'boolean' ||
    !isNullableNonnegativeNumber(value.balance)
  ) {
    throw new Error('The ChatGPT usage credits were invalid.');
  }
  return {
    hasCredits: value.hasCredits,
    unlimited: value.unlimited,
    balance: value.balance ?? null,
  };
}

/** Hides empty credits and otherwise reports unlimited, unknown, or the remaining balance. */
export function usageCreditsDisplay(credits: UsageCredits): UsageCreditsDisplay {
  if (credits.unlimited) return { kind: 'unlimited' };
  if (!credits.hasCredits) return { kind: 'hidden' };
  return credits.balance === null ? { kind: 'unavailable' } : { kind: 'balance', balance: credits.balance };
}

function isNullableNonnegativeNumber(value: unknown): value is number | null | undefined {
  return value === null || value === undefined || (typeof value === 'number' && Number.isFinite(value) && value >= 0);
}
