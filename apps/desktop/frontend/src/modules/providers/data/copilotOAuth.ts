import { LocalServiceError, localServiceClient } from '@/platform/local-service';
import { containsAsciiControlCharacters } from '@/shared/utils/textValidation';

export interface CopilotOAuthConnection {
  id: string;
  login: string;
  name: string | null;
  authStatus: 'active' | 'reauth_required';
}

export interface CopilotOAuthStatus {
  connected: boolean;
  connections: CopilotOAuthConnection[];
  connection: CopilotOAuthConnection | null;
  selectedConnectionId: string | null;
}

export type CopilotQuotaKind = 'chat' | 'completions' | 'premiumInteractions' | 'other';

export interface CopilotQuotaSnapshot {
  id: string;
  kind: CopilotQuotaKind;
  entitlement: number | null;
  remaining: number | null;
  percentRemaining: number | null;
  resetAtUnixMs: number | null;
  unlimited: boolean | null;
  hasQuota: boolean | null;
  tokenBasedBilling: boolean | null;
}

export interface CopilotUsageSnapshot {
  planType: string | null;
  accessType: string | null;
  quotas: CopilotQuotaSnapshot[];
}

export interface CopilotDeviceFlow {
  flowId: string;
  userCode: string;
  verificationUri: string;
  expiresAtUnixMs: number;
  intervalSeconds: number;
}

export type CopilotDevicePoll =
  | { status: 'pending'; intervalSeconds: number; retryAfterSeconds: number }
  | { status: 'connected'; connection: CopilotOAuthConnection };

export async function loadCopilotOAuthStatus(): Promise<CopilotOAuthStatus> {
  const response = await localServiceClient.call('copilot.oauth.status');
  if (
    typeof response.connected !== 'boolean' ||
    !Array.isArray(response.connections) ||
    (response.selectedConnectionId !== null && typeof response.selectedConnectionId !== 'string')
  ) {
    throw invalidResponse();
  }
  const connections = response.connections.map((connection) => {
    if (!isRecord(connection)) throw invalidResponse();
    return parseConnection(connection);
  });
  const ids = new Set(connections.map((connection) => connection.id));
  if (ids.size !== connections.length) throw invalidResponse();
  const selectedConnectionId = response.selectedConnectionId;
  const connection =
    selectedConnectionId === null ? null : connections.find((item) => item.id === selectedConnectionId);
  if (response.connected !== (connection !== null) || (selectedConnectionId !== null && connection === undefined)) {
    throw invalidResponse();
  }
  return {
    connected: connection !== null,
    connections,
    connection: connection ?? null,
    selectedConnectionId,
  };
}

export async function startCopilotOAuth(): Promise<CopilotDeviceFlow> {
  const response = await localServiceClient.call('copilot.oauth.start', {}, 35_000);
  if (
    typeof response.flowId !== 'string' ||
    !/^[a-f\d]{32}$/i.test(response.flowId) ||
    typeof response.userCode !== 'string' ||
    response.userCode.length === 0 ||
    typeof response.verificationUri !== 'string' ||
    !isGitHubDeviceUri(response.verificationUri) ||
    typeof response.expiresAtUnixMs !== 'number' ||
    !Number.isSafeInteger(response.expiresAtUnixMs) ||
    typeof response.intervalSeconds !== 'number' ||
    !Number.isSafeInteger(response.intervalSeconds) ||
    response.intervalSeconds < 1
  ) {
    throw invalidResponse();
  }
  return {
    flowId: response.flowId,
    userCode: response.userCode,
    verificationUri: response.verificationUri,
    expiresAtUnixMs: response.expiresAtUnixMs,
    intervalSeconds: response.intervalSeconds,
  };
}

export async function pollCopilotOAuth(flowId: string): Promise<CopilotDevicePoll> {
  const response = await localServiceClient.call('copilot.oauth.poll', { flowId }, 35_000);
  if (response.status === 'pending') {
    if (
      typeof response.intervalSeconds !== 'number' ||
      !Number.isSafeInteger(response.intervalSeconds) ||
      response.intervalSeconds < 1 ||
      typeof response.retryAfterSeconds !== 'number' ||
      !Number.isSafeInteger(response.retryAfterSeconds) ||
      response.retryAfterSeconds < 1
    ) {
      throw invalidResponse();
    }
    return {
      status: 'pending',
      intervalSeconds: response.intervalSeconds,
      retryAfterSeconds: response.retryAfterSeconds,
    };
  }
  if (response.status === 'connected' && isRecord(response.connection)) {
    return { status: 'connected', connection: parseConnection(response.connection) };
  }
  throw invalidResponse();
}

export async function cancelCopilotOAuth(flowId: string): Promise<boolean> {
  const response = await localServiceClient.call('copilot.oauth.cancel', { flowId });
  if (typeof response.cancelled !== 'boolean') throw invalidResponse();
  return response.cancelled;
}

export async function selectCopilotOAuthConnection(connectionId: string): Promise<void> {
  const response = await localServiceClient.call('copilot.oauth.select', { connectionId });
  if (response.selectedConnectionId !== connectionId) throw invalidResponse();
}

export async function disconnectCopilotOAuth(connectionId: string): Promise<void> {
  const response = await localServiceClient.call('copilot.oauth.disconnect', { connectionId });
  if (response.removed !== true) throw invalidResponse();
}

export async function loadCopilotUsage(): Promise<CopilotUsageSnapshot> {
  const response = await localServiceClient.call('copilot.usage.get', {}, 60_000);
  if (
    !isRecord(response) ||
    !isNullableLabel(response.planType) ||
    !isNullableLabel(response.accessType) ||
    !Array.isArray(response.quotas)
  ) {
    throw invalidUsageResponse();
  }
  return {
    planType: response.planType,
    accessType: response.accessType,
    quotas: response.quotas.map(parseQuotaSnapshot),
  };
}

function parseConnection(value: Record<string, unknown>): CopilotOAuthConnection {
  if (
    typeof value.id !== 'string' ||
    !/^\d{1,20}$/.test(value.id) ||
    typeof value.login !== 'string' ||
    value.login.length === 0 ||
    (value.name !== null && typeof value.name !== 'string') ||
    (value.authStatus !== 'active' && value.authStatus !== 'reauth_required')
  ) {
    throw invalidResponse();
  }
  return {
    id: value.id,
    login: value.login,
    name: value.name,
    authStatus: value.authStatus,
  };
}

function isGitHubDeviceUri(value: string): boolean {
  try {
    const uri = new URL(value);
    return uri.protocol === 'https:' && uri.hostname === 'github.com' && uri.pathname === '/login/device';
  } catch {
    return false;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseQuotaSnapshot(value: unknown): CopilotQuotaSnapshot {
  if (!isRecord(value)) throw invalidUsageResponse();
  const kind = value.kind;
  if (
    typeof value.id !== 'string' ||
    !/^[a-zA-Z\d_-]{1,80}$/.test(value.id) ||
    (kind !== 'chat' && kind !== 'completions' && kind !== 'premiumInteractions' && kind !== 'other')
  ) {
    throw invalidUsageResponse();
  }
  const entitlement = nullableNumber(value.entitlement);
  const remaining = nullableNumber(value.remaining);
  const percentRemaining = nullableNumber(value.percentRemaining);
  const resetAtUnixMs = nullableNonnegativeSafeInteger(value.resetAtUnixMs);
  const unlimited = nullableBoolean(value.unlimited);
  const hasQuota = nullableBoolean(value.hasQuota);
  const tokenBasedBilling = nullableBoolean(value.tokenBasedBilling);
  if (
    entitlement === undefined ||
    remaining === undefined ||
    percentRemaining === undefined ||
    resetAtUnixMs === undefined ||
    unlimited === undefined ||
    hasQuota === undefined ||
    tokenBasedBilling === undefined ||
    (entitlement !== null && entitlement < -1) ||
    (remaining !== null && remaining < 0) ||
    (percentRemaining !== null && (percentRemaining < 0 || percentRemaining > 100))
  ) {
    throw invalidUsageResponse();
  }
  return {
    id: value.id,
    kind,
    entitlement,
    remaining,
    percentRemaining,
    resetAtUnixMs,
    unlimited,
    hasQuota,
    tokenBasedBilling,
  };
}

function isNullableLabel(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && value.length <= 128 && !containsAsciiControlCharacters(value));
}

function nullableNumber(value: unknown): number | null | undefined {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER) {
    return undefined;
  }
  return value;
}

function nullableBoolean(value: unknown): boolean | null | undefined {
  return value === null || typeof value === 'boolean' ? value : undefined;
}

function nullableNonnegativeSafeInteger(value: unknown): number | null | undefined {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    return undefined;
  }
  return value;
}

function invalidResponse(): LocalServiceError {
  return new LocalServiceError('invalid_oauth_response', 'GitHub returned an unsupported connection response.');
}

function invalidUsageResponse(): LocalServiceError {
  return new LocalServiceError('invalid_copilot_usage_response', 'GitHub returned an unsupported usage response.');
}
