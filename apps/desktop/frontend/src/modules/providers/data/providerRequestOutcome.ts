import { containsAsciiControlCharacters } from '@/shared/utils/textValidation';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ProviderRequestOutcome =
  | 'completed'
  | 'waitingForInput'
  | 'stopped'
  | 'authenticationFailed'
  | 'rateLimited'
  | 'networkFailed'
  | 'failed';

export interface ProviderRequestRoute {
  providerId: string;
  connectionId: string;
  workspaceId: string | null;
}

export interface ProviderRequestOutcomeRecord {
  outcome: ProviderRequestOutcome;
  occurredAtUnixMs: number;
}

export class ProviderRequestOutcomeStorageError extends Error {
  constructor(readonly reason: 'readFailed' | 'writeFailed' | 'invalidRoute') {
    super('The provider request outcome could not be accessed.');
    this.name = 'ProviderRequestOutcomeStorageError';
  }
}

const maximumTimestamp = 8_640_000_000_000_000;

export function readProviderRequestOutcomeRecord(route: ProviderRequestRoute): ProviderRequestOutcomeRecord | null {
  const key = storageKey(route);
  let raw: string | null;
  try {
    raw = unifiedStorage.getItem(key);
  } catch {
    throw new ProviderRequestOutcomeStorageError('readFailed');
  }
  if (raw === null) return null;

  try {
    const value: unknown = JSON.parse(raw);
    if (!isObject(value) || !isOutcome(value.outcome) || !isTimestamp(value.occurredAtUnixMs)) {
      throw new Error('The provider request outcome is invalid.');
    }
    return { outcome: value.outcome, occurredAtUnixMs: value.occurredAtUnixMs };
  } catch {
    throw new ProviderRequestOutcomeStorageError('readFailed');
  }
}

export function writeProviderRequestOutcomeRecord(
  route: ProviderRequestRoute,
  record: ProviderRequestOutcomeRecord,
): void {
  const key = storageKey(route);
  if (!isOutcome(record.outcome) || !isTimestamp(record.occurredAtUnixMs)) {
    throw new ProviderRequestOutcomeStorageError('writeFailed');
  }
  try {
    unifiedStorage.setItem(key, JSON.stringify(record));
  } catch {
    throw new ProviderRequestOutcomeStorageError('writeFailed');
  }
}

export function clearProviderRequestOutcomeRecord(route: ProviderRequestRoute): void {
  const key = storageKey(route);
  try {
    unifiedStorage.removeItem(key);
  } catch {
    throw new ProviderRequestOutcomeStorageError('writeFailed');
  }
}

function storageKey(route: ProviderRequestRoute): string {
  if (
    !isRoutePart(route.providerId) ||
    !isRoutePart(route.connectionId) ||
    (route.workspaceId !== null && !isRoutePart(route.workspaceId))
  ) {
    throw new ProviderRequestOutcomeStorageError('invalidRoute');
  }
  const workspace = route.workspaceId === null ? 'no-workspace' : `workspace-${encodeURIComponent(route.workspaceId)}`;
  return `openchat.providerRequestOutcome.route.${encodeURIComponent(route.providerId)}.${encodeURIComponent(route.connectionId)}.${workspace}`;
}

function isRoutePart(value: string): boolean {
  return value.length > 0 && value.length <= 256 && !containsAsciiControlCharacters(value);
}

function isOutcome(value: unknown): value is ProviderRequestOutcome {
  return (
    value === 'completed' ||
    value === 'waitingForInput' ||
    value === 'stopped' ||
    value === 'authenticationFailed' ||
    value === 'rateLimited' ||
    value === 'networkFailed' ||
    value === 'failed'
  );
}

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= maximumTimestamp;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
