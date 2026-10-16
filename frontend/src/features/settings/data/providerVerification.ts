export type ProviderVerificationOutcome =
  | 'verified'
  | 'emptyCatalog'
  | 'staleCatalog'
  | 'authenticationFailed'
  | 'rateLimited'
  | 'networkFailed'
  | 'failed';

export interface ProviderVerificationRoute {
  providerId: string;
  connectionId: string;
  workspaceId: string | null;
}

export interface ProviderVerificationRecord {
  outcome: ProviderVerificationOutcome;
  checkedAtUnixMs: number;
}

const maximumTimestamp = 8_640_000_000_000_000;

export class ProviderVerificationStorageError extends Error {
  constructor(readonly reason: 'readFailed' | 'writeFailed' | 'invalidRoute') {
    super('The provider verification record could not be accessed.');
    this.name = 'ProviderVerificationStorageError';
  }
}

export function readProviderVerificationRecord(
  route: ProviderVerificationRoute,
): ProviderVerificationRecord | null {
  const key = storageKey(route);
  let raw: string | null;
  try {
    raw = window.localStorage.getItem(key);
  } catch {
    throw new ProviderVerificationStorageError('readFailed');
  }
  if (raw === null) return null;

  try {
    const value: unknown = JSON.parse(raw);
    if (!isObject(value) || !isOutcome(value.outcome) || !isTimestamp(value.checkedAtUnixMs)) {
      throw new Error('The provider verification record is invalid.');
    }
    return { outcome: value.outcome, checkedAtUnixMs: value.checkedAtUnixMs };
  } catch {
    throw new ProviderVerificationStorageError('readFailed');
  }
}

export function writeProviderVerificationRecord(
  route: ProviderVerificationRoute,
  record: ProviderVerificationRecord,
): void {
  const key = storageKey(route);
  if (!isOutcome(record.outcome) || !isTimestamp(record.checkedAtUnixMs)) {
    throw new ProviderVerificationStorageError('writeFailed');
  }
  try {
    window.localStorage.setItem(key, JSON.stringify(record));
  } catch {
    throw new ProviderVerificationStorageError('writeFailed');
  }
}

export function clearProviderVerificationRecord(route: ProviderVerificationRoute): void {
  const key = storageKey(route);
  try {
    window.localStorage.removeItem(key);
  } catch {
    throw new ProviderVerificationStorageError('writeFailed');
  }
}

function storageKey(route: ProviderVerificationRoute): string {
  if (!isRoutePart(route.providerId) || !isRoutePart(route.connectionId) ||
    (route.workspaceId !== null && !isRoutePart(route.workspaceId))) {
    throw new ProviderVerificationStorageError('invalidRoute');
  }
  const workspace = route.workspaceId === null
    ? 'no-workspace'
    : `workspace-${encodeURIComponent(route.workspaceId)}`;
  return `openchat.providerVerification.route.${encodeURIComponent(route.providerId)}.${encodeURIComponent(route.connectionId)}.${workspace}`;
}

function isRoutePart(value: string): boolean {
  return value.length > 0 && value.length <= 256 && !/[\u0000-\u001f\u007f]/u.test(value);
}

function isOutcome(value: unknown): value is ProviderVerificationOutcome {
  return value === 'verified' || value === 'emptyCatalog' || value === 'staleCatalog' ||
    value === 'authenticationFailed' || value === 'rateLimited' || value === 'networkFailed' || value === 'failed';
}

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= maximumTimestamp;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
