import { invoke } from '@shared/lib/platform/tauri';
import {
  classifyServiceErrorCode,
  localServiceErrorDetails,
  type ServiceErrorCategory,
} from '@shared/lib/platform/local-service';

import type { McpServerRecord } from './mcpCatalog';

export interface GlobalMcpSecretStatus {
  environmentName: string;
  isSaved: boolean;
}

export interface GlobalMcpSecretUpdate {
  environmentName: string;
  value: string;
}

export interface GlobalMcpLoadResult {
  servers: McpServerRecord[];
  invalidConfigCount: number;
}

export class McpStorageClientError extends Error {
  readonly category: ServiceErrorCategory;

  constructor(readonly code: string) {
    super('The local MCP storage request failed.');
    this.name = 'McpStorageClientError';
    this.category = classifyServiceErrorCode(code);
  }
}

export function mcpStorageErrorDetails(error: unknown): string | null {
  return error instanceof McpStorageClientError ? localServiceErrorDetails(error) : null;
}

export async function loadGlobalMcpServers(): Promise<GlobalMcpLoadResult> {
  const value: unknown = await invokeMcp('load_global_mcp_servers');
  if (
    !isObject(value) ||
    !Array.isArray(value.servers) ||
    !value.servers.every(isMcpServer) ||
    !Number.isSafeInteger(value.invalidConfigCount) ||
    typeof value.invalidConfigCount !== 'number' ||
    value.invalidConfigCount < 0
  ) {
    throw new Error('The stored MCP configuration list is invalid.');
  }
  return {
    servers: value.servers,
    invalidConfigCount: value.invalidConfigCount,
  };
}

export async function saveGlobalMcpServer(server: McpServerRecord): Promise<McpServerRecord> {
  const value: unknown = await invokeMcp('save_global_mcp_server', { server });
  if (!isMcpServer(value)) throw new Error('The saved MCP configuration is invalid.');
  return value;
}

export async function setGlobalMcpEnabled(serverId: string, enabled: boolean): Promise<McpServerRecord> {
  const value: unknown = await invokeMcp('set_global_mcp_enabled', { serverId, enabled });
  if (!isMcpServer(value)) throw new Error('The updated MCP configuration is invalid.');
  return value;
}

export async function removeGlobalMcpServer(serverId: string): Promise<void> {
  await invokeMcp('remove_global_mcp_server', { serverId });
}

export async function getGlobalMcpSecretStatus(
  serverId: string,
  environmentNames: readonly string[],
): Promise<GlobalMcpSecretStatus[]> {
  const value: unknown = await invokeMcp('get_global_mcp_secret_status', { serverId, environmentNames });
  if (!Array.isArray(value) || !value.every(isSecretStatus)) {
    throw new Error('The MCP secret status is invalid.');
  }
  return value;
}

export async function saveGlobalMcpSecrets(serverId: string, updates: readonly GlobalMcpSecretUpdate[]): Promise<void> {
  await invokeMcp('save_global_mcp_secrets', { serverId, updates });
}

export async function probeGlobalMcpServer(serverId: string): Promise<McpServerRecord> {
  const value: unknown = await invokeMcp('probe_global_mcp_server', { serverId });
  if (!isMcpServer(value)) throw new Error('The discovered MCP configuration is invalid.');
  return value;
}

async function invokeMcp<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new McpStorageClientError(readErrorCode(error));
  }
}

function readErrorCode(error: unknown): string {
  const fallback = 'mcp_storage_unavailable';
  if (isObject(error)) {
    return isStableErrorCode(error.code) ? error.code : fallback;
  }
  if (error instanceof Error) return readErrorCode(error.message);
  if (typeof error === 'string') {
    try {
      const parsed: unknown = JSON.parse(error);
      if (isObject(parsed) && isStableErrorCode(parsed.code)) return parsed.code;
    } catch {
      return fallback;
    }
  }
  return fallback;
}

function isStableErrorCode(value: unknown): value is string {
  return typeof value === 'string' && /^[a-z][a-z0-9_]{0,95}$/u.test(value);
}

function isMcpServer(value: unknown): value is McpServerRecord {
  return (
    isObject(value) &&
    isString(value.id) &&
    isString(value.name) &&
    isString(value.category) &&
    isString(value.description) &&
    isString(value.command) &&
    isStringArray(value.args) &&
    isStringArray(value.requiredEnvKeys) &&
    isStringArray(value.optionalEnvKeys) &&
    typeof value.installed === 'boolean' &&
    typeof value.enabled === 'boolean' &&
    isString(value.installDir) &&
    isStringArray(value.discoveredTools)
  );
}

function isSecretStatus(value: unknown): value is GlobalMcpSecretStatus {
  return isObject(value) && isString(value.environmentName) && typeof value.isSaved === 'boolean';
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isString);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}
