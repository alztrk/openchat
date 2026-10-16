import { invoke } from '@tauri-apps/api/core';

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
  constructor(readonly code: string) {
    super('The MCP storage request failed.');
    this.name = 'McpStorageClientError';
  }
}

export async function loadGlobalMcpServers(): Promise<GlobalMcpLoadResult> {
  const value: unknown = await invokeMcp('load_global_mcp_servers');
  if (!isObject(value) || !Array.isArray(value.servers) || !value.servers.every(isMcpServer) ||
    !Number.isSafeInteger(value.invalidConfigCount) || typeof value.invalidConfigCount !== 'number' || value.invalidConfigCount < 0) {
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

export async function saveGlobalMcpSecrets(
  serverId: string,
  updates: readonly GlobalMcpSecretUpdate[],
): Promise<void> {
  await invokeMcp('save_global_mcp_secrets', { serverId, updates });
}

export async function probeGlobalMcpServer(serverId: string): Promise<McpServerRecord> {
  const value: unknown = await invokeMcp('probe_global_mcp_server', { serverId });
  if (!isMcpServer(value)) throw new Error('The discovered MCP configuration is invalid.');
  return value;
}

export async function getActiveGlobalMcpInstructions(): Promise<string> {
  const value: unknown = await invokeMcp('get_active_global_mcp_instructions');
  if (typeof value !== 'string') throw new Error('The active MCP instructions are invalid.');
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
  if (isObject(error) && typeof error.code === 'string') return error.code;
  if (typeof error === 'string') {
    try {
      const parsed: unknown = JSON.parse(error);
      if (isObject(parsed) && typeof parsed.code === 'string') return parsed.code;
    } catch {
      return 'mcp_storage_unavailable';
    }
  }
  return 'mcp_storage_unavailable';
}

function isMcpServer(value: unknown): value is McpServerRecord {
  return isObject(value) &&
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
    isStringArray(value.discoveredTools);
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
