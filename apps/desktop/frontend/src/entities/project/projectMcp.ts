export type ProjectMcpTransport = 'stdio' | 'streamableHttp';

export interface ProjectMcpPackage {
  version: string;
  sha256: string;
  authenticodeSignerSha256?: string;
  sourceKeyFingerprint: string;
  sizeBytes: number;
}

export interface ProjectMcpServer {
  id: string;
  enabled: boolean;
  transport: ProjectMcpTransport;
  program?: string;
  arguments: string[];
  environmentVariables: string[];
  endpoint?: string;
  authEnvironmentVariable?: string;
  package?: ProjectMcpPackage;
  catalogToolNames?: string[];
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
    typeof id !== 'string' ||
    !/^[a-z0-9_]{1,24}$/.test(id) ||
    typeof enabled !== 'boolean' ||
    (transport !== 'stdio' && transport !== 'streamableHttp') ||
    !Array.isArray(args) ||
    !args.every((entry) => typeof entry === 'string') ||
    !Array.isArray(environmentVariables) ||
    !environmentVariables.every((entry) => typeof entry === 'string')
  ) {
    throw new Error('A project MCP server is invalid.');
  }
  const program = value.program;
  const endpoint = value.endpoint;
  const authEnvironmentVariable = value.authEnvironmentVariable;
  const packageReference = value.package === undefined ? undefined : parseProjectMcpPackage(value.package);
  const catalogToolNames =
    value.catalogToolNames === undefined ? undefined : parseCatalogToolNames(value.catalogToolNames);
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
    ...(packageReference === undefined ? {} : { package: packageReference }),
    ...(catalogToolNames === undefined ? {} : { catalogToolNames }),
  };
  validateProjectMcpServer(server);
  return server;
}

export function validateProjectMcpServer(server: ProjectMcpServer): void {
  if (!/^[a-z0-9_]{1,24}$/.test(server.id)) throw new Error('invalid_id');
  if (server.catalogToolNames !== undefined) parseCatalogToolNames(server.catalogToolNames);
  if (server.transport === 'stdio') {
    if (server.package === undefined) {
      if (!server.program || !isAbsoluteProgramPath(server.program)) throw new Error('program_required');
    } else if (server.program !== undefined) {
      throw new Error('invalid_server');
    }
    if (server.endpoint !== undefined || server.authEnvironmentVariable !== undefined)
      throw new Error('invalid_server');
    if (
      server.arguments.length > 64 ||
      server.arguments.some((argument) => argument.includes('\0')) ||
      new TextEncoder().encode(server.arguments.join('')).length > 32 * 1024
    ) {
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
  if (server.package !== undefined) throw new Error('invalid_server');
  if (!server.endpoint || !isHttpsEndpoint(server.endpoint)) throw new Error('endpoint_required');
  if (server.authEnvironmentVariable !== undefined && !isEnvironmentName(server.authEnvironmentVariable)) {
    throw new Error('environment_variables_invalid');
  }
}

function parseCatalogToolNames(value: unknown): string[] {
  if (
    !Array.isArray(value) ||
    value.length > 128 ||
    !value.every((name: unknown) => typeof name === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(name))
  ) {
    throw new Error('invalid_server');
  }
  const normalized = value.map((name: string) => name.toLocaleLowerCase('en-US'));
  if (new Set(normalized).size !== normalized.length) throw new Error('invalid_server');
  return [...value];
}

function parseProjectMcpPackage(value: unknown): ProjectMcpPackage {
  if (!isObject(value)) throw new Error('invalid_server');
  const packageReference: ProjectMcpPackage = {
    version: typeof value.version === 'string' ? value.version : '',
    sha256: typeof value.sha256 === 'string' ? value.sha256 : '',
    ...(value.authenticodeSignerSha256 === undefined
      ? {}
      : {
          authenticodeSignerSha256:
            typeof value.authenticodeSignerSha256 === 'string' ? value.authenticodeSignerSha256 : '',
        }),
    sourceKeyFingerprint: typeof value.sourceKeyFingerprint === 'string' ? value.sourceKeyFingerprint : '',
    sizeBytes: typeof value.sizeBytes === 'number' ? value.sizeBytes : Number.NaN,
  };
  if (
    !/^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$/.test(packageReference.version) ||
    !/^[a-f0-9]{64}$/.test(packageReference.sha256) ||
    (packageReference.authenticodeSignerSha256 !== undefined &&
      !/^[a-f0-9]{64}$/.test(packageReference.authenticodeSignerSha256)) ||
    !/^[a-f0-9]{64}$/.test(packageReference.sourceKeyFingerprint) ||
    !Number.isSafeInteger(packageReference.sizeBytes) ||
    packageReference.sizeBytes < 1 ||
    packageReference.sizeBytes > 128 * 1024 * 1024
  ) {
    throw new Error('invalid_server');
  }
  return packageReference;
}

function isAbsoluteProgramPath(value: string): boolean {
  return value.length <= 4096 && !value.includes('\0') && (value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value));
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
  return /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(name) && !reservedEnvironmentNames.has(name.toUpperCase());
}

function isHttpsEndpoint(endpoint: string): boolean {
  try {
    const parsed = new URL(endpoint);
    return (
      parsed.protocol === 'https:' &&
      parsed.hostname.length > 0 &&
      parsed.username === '' &&
      parsed.password === '' &&
      parsed.search === '' &&
      parsed.hash === ''
    );
  } catch {
    return false;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
