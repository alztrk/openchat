const storageKey = 'connectors.local_catalog';
const maxCatalogBytes = 64 * 1024;

export type ConnectorTransport = 'stdio' | 'streamableHttp';

export interface ConnectorCatalogEntry {
  id: string;
  name: string;
  publisher: string;
  version: string;
  sourceUrl: string;
  requestedAccess: string;
  dataDisclosure: string;
  transport: ConnectorTransport;
  program?: string;
  arguments: string[];
  environmentVariables: string[];
  endpoint?: string;
  authEnvironmentVariable?: string;
}

export type ConnectorCatalogFailure = 'invalid' | 'unavailable';

export class ConnectorCatalogError extends Error {
  constructor(readonly failure: ConnectorCatalogFailure) {
    super(`Connector catalog ${failure}.`);
    this.name = 'ConnectorCatalogError';
  }
}

export function readConnectorCatalog(): ConnectorCatalogEntry[] {
  try {
    const encoded = window.localStorage.getItem(storageKey);
    if (encoded === null) return [];
    if (new TextEncoder().encode(encoded).byteLength > maxCatalogBytes) {
      throw new ConnectorCatalogError('invalid');
    }

    const value: unknown = JSON.parse(encoded);
    if (!Array.isArray(value) || value.length > 50) {
      throw new ConnectorCatalogError('invalid');
    }

    const entries = value.map(parseConnectorEntry);
    validateConnectorCatalog(entries);
    return entries;
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    if (error instanceof SyntaxError) throw new ConnectorCatalogError('invalid');
    throw new ConnectorCatalogError('unavailable');
  }
}

export function writeConnectorCatalog(entries: ConnectorCatalogEntry[]): void {
  try {
    validateConnectorCatalog(entries);
    if (entries.length === 0) {
      window.localStorage.removeItem(storageKey);
    } else {
      window.localStorage.setItem(storageKey, JSON.stringify(entries));
    }
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    throw new ConnectorCatalogError('unavailable');
  }
}

export function parseConnectorEntry(value: unknown): ConnectorCatalogEntry {
  if (!isRecord(value)) throw new ConnectorCatalogError('invalid');

  const entry: ConnectorCatalogEntry = {
    id: requiredString(value.id, 24),
    name: requiredString(value.name, 80).trim(),
    publisher: requiredString(value.publisher, 120).trim(),
    version: requiredString(value.version, 64).trim(),
    sourceUrl: requiredHttpsUrl(value.sourceUrl),
    requestedAccess: requiredString(value.requestedAccess, 500).trim(),
    dataDisclosure: requiredString(value.dataDisclosure, 500).trim(),
    transport: parseTransport(value.transport),
    arguments: stringList(value.arguments, 64, 4096),
    environmentVariables: stringList(value.environmentVariables, 32, 128),
  };

  const program = optionalString(value.program, 4096);
  const endpoint = value.endpoint === undefined || value.endpoint === null
    ? undefined
    : requiredHttpsUrl(value.endpoint);
  const authEnvironmentVariable = optionalString(value.authEnvironmentVariable, 128);

  if (program !== undefined) entry.program = program.trim();
  if (endpoint !== undefined) entry.endpoint = endpoint;
  if (authEnvironmentVariable !== undefined) {
    entry.authEnvironmentVariable = authEnvironmentVariable;
  }

  if (
    !/^[a-z0-9_]{1,24}$/.test(entry.id) ||
    !entry.name ||
    !entry.publisher ||
    !entry.version ||
    !entry.requestedAccess ||
    !entry.dataDisclosure ||
    (entry.transport === 'stdio' && (entry.program === undefined || !isAbsoluteWindowsPath(entry.program))) ||
    (entry.transport === 'streamableHttp' && (
      entry.endpoint === undefined ||
      entry.program !== undefined ||
      entry.arguments.length > 0 ||
      entry.environmentVariables.length > 0
    )) ||
    (entry.transport === 'stdio' && (
      entry.endpoint !== undefined ||
      entry.authEnvironmentVariable !== undefined ||
      entry.arguments.some(looksLikeEmbeddedSecret)
    ))
  ) {
    throw new ConnectorCatalogError('invalid');
  }

  const environmentNames = new Set<string>();
  for (const name of [...entry.environmentVariables, ...(entry.authEnvironmentVariable ? [entry.authEnvironmentVariable] : [])]) {
    if (!isSafeEnvironmentName(name) || environmentNames.has(name.toUpperCase())) {
      throw new ConnectorCatalogError('invalid');
    }
    environmentNames.add(name.toUpperCase());
  }

  return entry;
}

function validateConnectorCatalog(entries: ConnectorCatalogEntry[]): void {
  if (entries.length > 50) throw new ConnectorCatalogError('invalid');

  const ids = new Set<string>();
  const names = new Set<string>();
  for (const candidate of entries) {
    const entry = parseConnectorEntry(candidate);
    const normalizedName = entry.name.toLocaleLowerCase('en-US');
    if (ids.has(entry.id) || names.has(normalizedName)) {
      throw new ConnectorCatalogError('invalid');
    }
    ids.add(entry.id);
    names.add(normalizedName);
  }

  const encoded = JSON.stringify(entries);
  if (new TextEncoder().encode(encoded).byteLength > maxCatalogBytes) {
    throw new ConnectorCatalogError('invalid');
  }
}

function parseTransport(value: unknown): ConnectorTransport {
  if (value === 'stdio' || value === 'streamableHttp') return value;
  throw new ConnectorCatalogError('invalid');
}

function requiredString(value: unknown, maxLength: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw new ConnectorCatalogError('invalid');
  }
  return value;
}

function optionalString(value: unknown, maxLength: number): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw new ConnectorCatalogError('invalid');
  }
  return value;
}

function requiredHttpsUrl(value: unknown): string {
  const raw = requiredString(value, 2048);
  if (raw !== raw.trim() || raw.includes('#') || raw.includes('?')) {
    throw new ConnectorCatalogError('invalid');
  }

  try {
    const url = new URL(raw);
    if (
      url.protocol !== 'https:' ||
      !url.hostname ||
      url.username.length > 0 ||
      url.password.length > 0
    ) {
      throw new ConnectorCatalogError('invalid');
    }
    return url.toString();
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    throw new ConnectorCatalogError('invalid');
  }
}

function stringList(value: unknown, maxCount: number, maxLength: number): string[] {
  if (value === undefined || value === null) return [];
  if (
    !Array.isArray(value) ||
    value.length > maxCount ||
    value.some((item) => typeof item !== 'string' || item.length > maxLength)
  ) {
    throw new ConnectorCatalogError('invalid');
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isAbsoluteWindowsPath(value: string): boolean {
  return /^(?:[A-Za-z]:[\\/]|\\\\[^\\]+\\[^\\]+)/.test(value);
}

function isSafeEnvironmentName(value: string): boolean {
  if (!/^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(value)) return false;
  return !new Set([
    'PATH', 'SYSTEMROOT', 'WINDIR', 'PATHEXT', 'COMSPEC', 'USERPROFILE',
    'APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP',
  ]).has(value.toUpperCase());
}

function looksLikeEmbeddedSecret(argument: string): boolean {
  return /(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret|authorization|bearer)\s*[:=]\s*\S+/i.test(argument);
}
