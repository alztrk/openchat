import {
  classifyServiceErrorCode,
  isServiceErrorCategory,
  LocalServiceError,
  localServiceClient,
  type ServiceErrorCategory,
} from '@shared/lib/platform/local-service';
import { isObject } from '@shared/lib/utils/json';

export type ProviderBalanceProviderId = 'openrouter' | 'deepseek' | 'opencode';
export type ProviderQuotaWindowLabel = 'key' | 'credits' | 'rolling' | 'weekly' | 'monthly';
export type ProviderBalanceLabel = 'key' | 'credits' | 'balance' | 'daily' | 'weekly' | 'monthly';
export type ProviderBalanceAmountField = 'total' | 'granted' | 'toppedUp' | 'limit' | 'remaining' | 'used';

export interface ProviderQuotaWindow {
  label: ProviderQuotaWindowLabel;
  windowSeconds: number | null;
  usedPercent: number;
  resetAtUnixMs: number | null;
}

export interface ProviderBalance {
  label: ProviderBalanceLabel;
  currency: string;
  total: number | null;
  granted: number | null;
  toppedUp: number | null;
  limit: number | null;
  remaining: number | null;
  used: number | null;
}

export interface ProviderBalanceSnapshot {
  providerId: ProviderBalanceProviderId;
  status: 'available' | 'unavailable';
  reasonCode: string | null;
  errorCode: string | null;
  errorCategory: ServiceErrorCategory | null;
  windows: ProviderQuotaWindow[];
  balances: ProviderBalance[];
}

export const providerBalanceAmountFields: readonly ProviderBalanceAmountField[] = [
  'total',
  'granted',
  'toppedUp',
  'limit',
  'remaining',
  'used',
];

const maximumTimestamp = 8.64e15;

/** Loads quotas and balances for providers whose API key is saved; no credentials cross this boundary. */
export async function loadProviderBalances(): Promise<ProviderBalanceSnapshot[]> {
  const response = await localServiceClient.call('providers.quota_balances.list', {}, 30_000);
  if (!Array.isArray(response.providers)) throw invalidResponse();
  const snapshots = response.providers.map(parseSnapshot);
  const ids = new Set(snapshots.map((snapshot) => snapshot.providerId));
  if (ids.size !== snapshots.length) throw invalidResponse();
  return snapshots;
}

/** Formats a provider amount in its own currency, falling back to a plain number and currency code. */
export function formatBalanceAmount(value: number, currency: string, locale: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency, maximumFractionDigits: 4 }).format(value);
  } catch {
    return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 4 }).format(value)} ${currency}`;
  }
}

function parseSnapshot(value: unknown): ProviderBalanceSnapshot {
  if (!isObject(value)) throw invalidResponse();
  const { providerId, status, reasonCode, error } = value;
  const errorCode =
    error === null ? null : isObject(error) && isNonEmptyString(error.code) ? error.code : undefined;
  if (
    (providerId !== 'openrouter' && providerId !== 'deepseek' && providerId !== 'opencode') ||
    (status !== 'available' && status !== 'unavailable') ||
    (reasonCode !== null && !isNonEmptyString(reasonCode)) ||
    errorCode === undefined ||
    !Array.isArray(value.windows) ||
    !Array.isArray(value.balances)
  ) {
    throw invalidResponse();
  }
  const windows = value.windows.map(parseWindow);
  const balances = value.balances.map(parseBalance);
  if (
    (status === 'available' && (reasonCode !== null || errorCode !== null || windows.length + balances.length === 0)) ||
    (status === 'unavailable' && reasonCode === null) ||
    (errorCode !== null && reasonCode !== errorCode) ||
    new Set(windows.map((window) => window.label)).size !== windows.length ||
    new Set(balances.map((balance) => `${balance.label}:${balance.currency}`)).size !== balances.length
  ) {
    throw invalidResponse();
  }
  const errorCategory =
    isObject(error) && isServiceErrorCategory(error.category)
      ? error.category
      : errorCode === null
        ? null
        : classifyServiceErrorCode(errorCode);
  return { providerId, status, reasonCode, errorCode, errorCategory, windows, balances };
}

function parseWindow(value: unknown): ProviderQuotaWindow {
  if (!isObject(value)) throw invalidResponse();
  const { label, usedPercent } = value;
  const windowSeconds = nullablePositiveSafeInteger(value.windowSeconds);
  const resetAtUnixMs = nullableTimestamp(value.resetAtUnixMs);
  if (
    (label !== 'key' && label !== 'credits' && label !== 'rolling' && label !== 'weekly' && label !== 'monthly') ||
    typeof usedPercent !== 'number' ||
    !Number.isFinite(usedPercent) ||
    usedPercent < 0 ||
    windowSeconds === undefined ||
    resetAtUnixMs === undefined
  ) {
    throw invalidResponse();
  }
  return { label, windowSeconds, usedPercent, resetAtUnixMs };
}

function parseBalance(value: unknown): ProviderBalance {
  if (!isObject(value)) throw invalidResponse();
  const { label, currency } = value;
  const total = nullableAmount(value.total);
  const granted = nullableAmount(value.granted);
  const toppedUp = nullableAmount(value.toppedUp);
  const limit = nullableAmount(value.limit);
  const remaining = nullableAmount(value.remaining);
  const used = nullableAmount(value.used);
  if (
    (label !== 'key' &&
      label !== 'credits' &&
      label !== 'balance' &&
      label !== 'daily' &&
      label !== 'weekly' &&
      label !== 'monthly') ||
    typeof currency !== 'string' ||
    !/^[A-Z]{3}$/.test(currency) ||
    total === undefined ||
    granted === undefined ||
    toppedUp === undefined ||
    limit === undefined ||
    remaining === undefined ||
    used === undefined ||
    [total, granted, toppedUp, limit, remaining, used].every((amount) => amount === null)
  ) {
    throw invalidResponse();
  }
  return { label, currency, total, granted, toppedUp, limit, remaining, used };
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function nullableAmount(value: unknown): number | null | undefined {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER) {
    return undefined;
  }
  return Object.is(value, -0) ? 0 : value;
}

function nullablePositiveSafeInteger(value: unknown): number | null | undefined {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 1) return undefined;
  return value;
}

function nullableTimestamp(value: unknown): number | null | undefined {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || Math.abs(value) > maximumTimestamp) {
    return undefined;
  }
  return value;
}

function invalidResponse(): LocalServiceError {
  return new LocalServiceError('invalid_provider_response', 'The provider balance response was invalid.');
}
