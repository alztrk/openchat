import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';

const storageKey = 'connectors.local_catalog';
const sourcesStorageKey = 'connectors.pinned_sources';
const maxCatalogBytes = 64 * 1024;
const maxCatalogSources = 10;

export type ConnectorTransport = 'stdio' | 'streamableHttp';

export interface ConnectorExecutablePackage {
  url: string;
  sha256: string;
  sizeBytes: number;
}

export interface ConnectorCatalogTrust {
  sourceUrl: string;
  publicKeyBase64: string;
  keyFingerprint: string;
}

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
  package?: ConnectorExecutablePackage;
  catalogTrust?: ConnectorCatalogTrust;
}

export interface ConnectorCatalogSource {
  id: string;
  name: string;
  url: string;
  publicKeyBase64: string;
  lastRevision: number | null;
  lastPayloadHash: string | null;
}

export interface VerifiedConnectorCatalog {
  sourceUrl: string;
  revision: number;
  keyFingerprint: string;
  payloadHash: string;
  entries: ConnectorCatalogEntry[];
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
    const encoded = unifiedStorage.getItem(storageKey);
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
      unifiedStorage.removeItem(storageKey);
    } else {
      unifiedStorage.setItem(storageKey, JSON.stringify(entries));
    }
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    throw new ConnectorCatalogError('unavailable');
  }
}

export function readConnectorSources(): ConnectorCatalogSource[] {
  try {
    const encoded = unifiedStorage.getItem(sourcesStorageKey);
    if (encoded === null) return [];
    if (new TextEncoder().encode(encoded).byteLength > maxCatalogBytes) {
      throw new ConnectorCatalogError('invalid');
    }
    const value: unknown = JSON.parse(encoded);
    if (!Array.isArray(value) || value.length > maxCatalogSources) {
      throw new ConnectorCatalogError('invalid');
    }
    const sources = value.map(parseConnectorCatalogSource);
    const ids = new Set<string>();
    const urls = new Set<string>();
    for (const source of sources) {
      if (ids.has(source.id) || urls.has(source.url)) throw new ConnectorCatalogError('invalid');
      ids.add(source.id);
      urls.add(source.url);
    }
    return sources;
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    if (error instanceof SyntaxError) throw new ConnectorCatalogError('invalid');
    throw new ConnectorCatalogError('unavailable');
  }
}

export function writeConnectorSources(sources: ConnectorCatalogSource[]): void {
  try {
    if (sources.length > maxCatalogSources) throw new ConnectorCatalogError('invalid');
    const parsed = sources.map(parseConnectorCatalogSource);
    const ids = new Set<string>();
    const urls = new Set<string>();
    for (const source of parsed) {
      if (ids.has(source.id) || urls.has(source.url)) throw new ConnectorCatalogError('invalid');
      ids.add(source.id);
      urls.add(source.url);
    }
    const encoded = JSON.stringify(parsed);
    if (new TextEncoder().encode(encoded).byteLength > maxCatalogBytes) {
      throw new ConnectorCatalogError('invalid');
    }
    if (parsed.length === 0) unifiedStorage.removeItem(sourcesStorageKey);
    else unifiedStorage.setItem(sourcesStorageKey, encoded);
  } catch (error) {
    if (error instanceof ConnectorCatalogError) throw error;
    throw new ConnectorCatalogError('unavailable');
  }
}

export async function fetchSignedConnectorCatalog(
  source: ConnectorCatalogSource,
): Promise<VerifiedConnectorCatalog> {
  const response = await localServiceClient.call('connector.catalog.fetch_signed', {
    sourceUrl: source.url,
    publicKeyBase64: source.publicKeyBase64,
  }, 15_000);
  if (
    !isRecord(response) ||
    typeof response.sourceUrl !== 'string' ||
    !Number.isSafeInteger(response.revision) ||
    typeof response.revision !== 'number' ||
    response.revision < 1 ||
    typeof response.keyFingerprint !== 'string' ||
    !/^[a-f0-9]{64}$/.test(response.keyFingerprint) ||
    typeof response.payloadHash !== 'string' ||
    !/^[a-f0-9]{64}$/.test(response.payloadHash) ||
    !Array.isArray(response.entries) ||
    response.entries.length > 50
  ) {
    throw new LocalServiceError('invalid_service_response', 'The signed connector catalog is invalid.');
  }
  const sourceUrl = requiredHttpsUrl(response.sourceUrl);
  const entries = response.entries.map((value) => {
    if (isRecord(value) && value.package !== undefined) {
      return parseConnectorEntry({
        ...value,
        catalogTrust: {
          sourceUrl: source.url,
          publicKeyBase64: source.publicKeyBase64,
          keyFingerprint: response.keyFingerprint,
        },
      });
    }
    return parseConnectorEntry(value);
  });
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const entry of entries) {
    const normalizedName = entry.name.toLocaleLowerCase('en-US');
    if (ids.has(entry.id) || names.has(normalizedName)) {
      throw new ConnectorCatalogError('invalid');
    }
    ids.add(entry.id);
    names.add(normalizedName);
  }
  return {
    sourceUrl,
    revision: response.revision,
    keyFingerprint: response.keyFingerprint,
    payloadHash: response.payloadHash,
    entries,
  };
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
  const endpoint =
    value.endpoint === undefined || value.endpoint === null ? undefined : requiredHttpsUrl(value.endpoint);
  const authEnvironmentVariable = optionalString(value.authEnvironmentVariable, 128);
  const packageMetadata =
    value.package === undefined || value.package === null ? undefined : parseExecutablePackage(value.package);
  const catalogTrust =
    value.catalogTrust === undefined || value.catalogTrust === null
      ? undefined
      : parseCatalogTrust(value.catalogTrust);

  if (program !== undefined) entry.program = program.trim();
  if (endpoint !== undefined) entry.endpoint = endpoint;
  if (authEnvironmentVariable !== undefined) {
    entry.authEnvironmentVariable = authEnvironmentVariable;
  }
  if (packageMetadata !== undefined) entry.package = packageMetadata;
  if (catalogTrust !== undefined) entry.catalogTrust = catalogTrust;

  if (
    !/^[a-z0-9_]{1,24}$/.test(entry.id) ||
    !entry.name ||
    !entry.publisher ||
    !entry.version ||
    !entry.requestedAccess ||
    !entry.dataDisclosure ||
    (entry.transport === 'stdio' &&
      ((entry.package === undefined &&
        (entry.program === undefined || !isAbsoluteWindowsPath(entry.program))) ||
        (entry.package !== undefined &&
          (entry.program !== undefined || entry.catalogTrust === undefined)))) ||
    (entry.transport === 'streamableHttp' &&
      (entry.endpoint === undefined ||
        entry.program !== undefined ||
        entry.package !== undefined ||
        entry.catalogTrust !== undefined ||
        entry.arguments.length > 0 ||
        entry.environmentVariables.length > 0)) ||
    (entry.transport === 'stdio' &&
      (entry.endpoint !== undefined ||
        entry.authEnvironmentVariable !== undefined ||
        entry.arguments.some(looksLikeEmbeddedSecret)))
  ) {
    throw new ConnectorCatalogError('invalid');
  }
  if (entry.package === undefined && entry.catalogTrust !== undefined) {
    throw new ConnectorCatalogError('invalid');
  }

  const environmentNames = new Set<string>();
  for (const name of [
    ...entry.environmentVariables,
    ...(entry.authEnvironmentVariable ? [entry.authEnvironmentVariable] : []),
  ]) {
    if (!isSafeEnvironmentName(name) || environmentNames.has(name.toUpperCase())) {
      throw new ConnectorCatalogError('invalid');
    }
    environmentNames.add(name.toUpperCase());
  }

  return entry;
}

function parseExecutablePackage(value: unknown): ConnectorExecutablePackage {
  if (!isRecord(value)) throw new ConnectorCatalogError('invalid');
  const packageMetadata: ConnectorExecutablePackage = {
    url: requiredHttpsUrl(value.url),
    sha256: requiredString(value.sha256, 64),
    sizeBytes: typeof value.sizeBytes === 'number' ? value.sizeBytes : Number.NaN,
  };
  if (
    !/^[a-f0-9]{64}$/.test(packageMetadata.sha256) ||
    !Number.isSafeInteger(packageMetadata.sizeBytes) ||
    packageMetadata.sizeBytes < 1 ||
    packageMetadata.sizeBytes > 128 * 1024 * 1024
  ) {
    throw new ConnectorCatalogError('invalid');
  }
  return packageMetadata;
}

function parseCatalogTrust(value: unknown): ConnectorCatalogTrust {
  if (!isRecord(value)) throw new ConnectorCatalogError('invalid');
  const trust: ConnectorCatalogTrust = {
    sourceUrl: requiredHttpsUrl(value.sourceUrl),
    publicKeyBase64: requiredString(value.publicKeyBase64, 43),
    keyFingerprint: requiredString(value.keyFingerprint, 64),
  };
  if (!/^[A-Za-z0-9_-]{43}$/.test(trust.publicKeyBase64) || !/^[a-f0-9]{64}$/.test(trust.keyFingerprint)) {
    throw new ConnectorCatalogError('invalid');
  }
  return trust;
}

export function parseConnectorCatalogSource(value: unknown): ConnectorCatalogSource {
  if (!isRecord(value)) throw new ConnectorCatalogError('invalid');
  const lastRevision = value.lastRevision ?? null;
  const lastPayloadHash = value.lastPayloadHash ?? null;
  const source: ConnectorCatalogSource = {
    id: requiredString(value.id, 48),
    name: requiredString(value.name, 80).trim(),
    url: requiredHttpsUrl(value.url),
    publicKeyBase64: requiredString(value.publicKeyBase64, 43),
    lastRevision: null,
    lastPayloadHash: null,
  };
  if (
    !(lastRevision === null || (typeof lastRevision === 'number' && Number.isSafeInteger(lastRevision) && lastRevision > 0)) ||
    !(lastPayloadHash === null || (typeof lastPayloadHash === 'string' && /^[a-f0-9]{64}$/.test(lastPayloadHash))) ||
    ((lastRevision === null) !== (lastPayloadHash === null))
  ) {
    throw new ConnectorCatalogError('invalid');
  }
  source.lastRevision = lastRevision;
  source.lastPayloadHash = lastPayloadHash;
  if (
    !/^[a-z0-9_-]{1,48}$/.test(source.id) ||
    !source.name ||
    !/^[A-Za-z0-9_-]{43}$/.test(source.publicKeyBase64)
  ) {
    throw new ConnectorCatalogError('invalid');
  }
  return source;
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
    if (url.protocol !== 'https:' || !url.hostname || url.username.length > 0 || url.password.length > 0) {
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
  ]).has(value.toUpperCase());
}

function looksLikeEmbeddedSecret(argument: string): boolean {
  return /(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret|authorization|bearer)\s*[:=]\s*\S+/i.test(
    argument,
  );
}
