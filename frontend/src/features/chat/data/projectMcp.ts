export type ProjectMcpTransport = 'stdio' | 'streamableHttp';

export interface ProjectMcpServer {
  id: string;
  enabled: boolean;
  transport: ProjectMcpTransport;
  program?: string;
  arguments: string[];
  environmentVariables: string[];
  endpoint?: string;
  authEnvironmentVariable?: string;
}

export interface ProjectMcpCatalog {
  version: 1;
  servers: ProjectMcpServer[];
}

export function parseProjectMcpCatalog(value: unknown): ProjectMcpCatalog {
  if (!isObject(value) || value.version !== 1 || !Array.isArray(value.servers) || value.servers.length > 8) {
    throw new Error('The project MCP catalog is invalid.');
  }
  const servers = value.servers.map(parseProjectMcpServer);
  if (new Set(servers.map((server) => server.id)).size !== servers.length) {
    throw new Error('The project MCP catalog contains duplicate server IDs.');
  }
  return { version: 1, servers };
}

export function parseProjectMcpServer(value: unknown): ProjectMcpServer {
  if (!isObject(value)) throw new Error('A project MCP server is invalid.');
  const id = value.id;
  const enabled = value.enabled;
  const transport = value.transport ?? 'stdio';
  const args = value.arguments ?? [];
  const environmentVariables = value.environmentVariables ?? [];
  if (
    typeof id !== 'string' || !/^[a-z0-9_]{1,24}$/.test(id) ||
    typeof enabled !== 'boolean' ||
    (transport !== 'stdio' && transport !== 'streamableHttp') ||
    !Array.isArray(args) || !args.every((entry) => typeof entry === 'string') ||
    !Array.isArray(environmentVariables) || !environmentVariables.every((entry) => typeof entry === 'string')
  ) {
    throw new Error('A project MCP server is invalid.');
  }
  const program = value.program;
  const endpoint = value.endpoint;
  const authEnvironmentVariable = value.authEnvironmentVariable;
  if (
    (program !== undefined && typeof program !== 'string') ||
    (endpoint !== undefined && typeof endpoint !== 'string') ||
    (authEnvironmentVariable !== undefined && typeof authEnvironmentVariable !== 'string')
  ) {
    throw new Error('A project MCP server is invalid.');
  }
  const server: ProjectMcpServer = {
    id,
    enabled,
    transport,
    arguments: args,
    environmentVariables,
    ...(program === undefined ? {} : { program }),
    ...(endpoint === undefined ? {} : { endpoint }),
    ...(authEnvironmentVariable === undefined ? {} : { authEnvironmentVariable }),
  };
  validateProjectMcpServer(server);
  return server;
}

export function validateProjectMcpServer(server: ProjectMcpServer): void {
  if (!/^[a-z0-9_]{1,24}$/.test(server.id)) throw new Error('invalid_id');
  if (server.transport === 'stdio') {
    if (!server.program || !isAbsoluteProgramPath(server.program)) throw new Error('program_required');
    if (server.endpoint !== undefined || server.authEnvironmentVariable !== undefined) throw new Error('invalid_server');
    if (server.arguments.length > 64 || server.arguments.some((argument) => argument.includes('\0')) ||
      new TextEncoder().encode(server.arguments.join('')).length > 32 * 1024) {
      throw new Error('invalid_server');
    }
    if (!validEnvironmentNames(server.environmentVariables) || server.authEnvironmentVariable !== undefined) {
      throw new Error('environment_variables_invalid');
    }
    return;
  }
  if (server.program !== undefined || server.arguments.length > 0 || server.environmentVariables.length > 0) {
    throw new Error('invalid_server');
  }
  if (!server.endpoint || !isHttpsEndpoint(server.endpoint)) throw new Error('endpoint_required');
  if (server.authEnvironmentVariable !== undefined && !isEnvironmentName(server.authEnvironmentVariable)) {
    throw new Error('environment_variables_invalid');
  }
}

function isAbsoluteProgramPath(value: string): boolean {
  return value.length <= 4096 && !value.includes('\0') &&
    (value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value));
}

const reservedEnvironmentNames = new Set([
  'PATH',
  'SYSTEMROOT',
  'WINDIR',
  'PATHEXT',
  'COMSPEC',
  'USERPROFILE',
  'APPDATA',
  'LOCALAPPDATA',
  'TEMP',
  'TMP',
]);

function validEnvironmentNames(names: readonly string[]): boolean {
  return names.length <= 32 && names.every(isEnvironmentName) && new Set(names).size === names.length;
}

function isEnvironmentName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(name) &&
    !reservedEnvironmentNames.has(name.toUpperCase());
}

function isHttpsEndpoint(endpoint: string): boolean {
  try {
    const parsed = new URL(endpoint);
    return parsed.protocol === 'https:' && parsed.hostname.length > 0 && parsed.username === '' &&
      parsed.password === '' && parsed.search === '' && parsed.hash === '';
  } catch {
    return false;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
