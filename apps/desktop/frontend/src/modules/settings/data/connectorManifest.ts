export interface ConnectorManifestTool {
  name: string;
  description?: string;
}

export interface ConnectorManifestConfiguration {
  name: string;
  description: string;
  required: boolean;
  secret: boolean;
}

export interface ConnectorManifest {
  version: 1;
  tools: ConnectorManifestTool[];
  permissions: string[];
  providers: string[];
  configuration: ConnectorManifestConfiguration[];
}

export interface ConnectorToolComparison {
  declaredOnly: string[];
  runtimeOnly: string[];
}

const maximumManifestBytes = 16 * 1024;
const maximumTools = 128;
const maximumPermissions = 32;
const maximumProviders = 16;
const maximumConfigurationItems = 64;
const bidiControlCodePoints = new Set([
  0x061c, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2066, 0x2067, 0x2068, 0x2069, 0x206a, 0x206b,
  0x206c, 0x206d, 0x206e, 0x206f,
]);

export function parseConnectorManifest(value: unknown): ConnectorManifest {
  if (!isRecord(value) || !hasOnlyKeys(value, ['version', 'tools', 'permissions', 'providers', 'configuration'])) {
    throw new TypeError('Connector manifest has an unsupported shape.');
  }
  if (value.version !== 1) throw new TypeError('Connector manifest version is unsupported.');

  const manifest: ConnectorManifest = {
    version: 1,
    tools: parseTools(value.tools),
    permissions: parseTextList(value.permissions, maximumPermissions, 240),
    providers: parseTextList(value.providers, maximumProviders, 120),
    configuration: parseConfiguration(value.configuration),
  };
  const encoded = JSON.stringify(manifest);
  if (new TextEncoder().encode(encoded).byteLength > maximumManifestBytes) {
    throw new TypeError('Connector manifest exceeds its size limit.');
  }
  return manifest;
}

export function compareConnectorToolNames(
  declaredNames: readonly string[] | undefined,
  runtimeNames: readonly string[],
): ConnectorToolComparison | null {
  if (declaredNames === undefined || declaredNames.length === 0) return null;

  const runtimeSet = new Set(runtimeNames);
  const declaredSet = new Set(declaredNames);
  return {
    declaredOnly: declaredNames.filter((name) => !runtimeSet.has(name)),
    runtimeOnly: runtimeNames.filter((name) => !declaredSet.has(name)),
  };
}

function parseTools(value: unknown): ConnectorManifestTool[] {
  if (!Array.isArray(value) || value.length > maximumTools) {
    throw new TypeError('Connector manifest tool list is invalid.');
  }
  const names = new Set<string>();
  return value.map((item) => {
    if (!isRecord(item) || !hasOnlyKeys(item, ['name', 'description'])) {
      throw new TypeError('Connector manifest tool is invalid.');
    }
    const name = requiredText(item.name, 128);
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(name)) {
      throw new TypeError('Connector manifest tool name is invalid.');
    }
    assertUnique(names, name);
    const description = readOptionalText(item, 'description', 500);
    return description === undefined ? { name } : { name, description };
  });
}

function parseConfiguration(value: unknown): ConnectorManifestConfiguration[] {
  if (!Array.isArray(value) || value.length > maximumConfigurationItems) {
    throw new TypeError('Connector manifest configuration list is invalid.');
  }
  const names = new Set<string>();
  return value.map((item) => {
    if (
      !isRecord(item) ||
      !hasOnlyKeys(item, ['name', 'description', 'required', 'secret']) ||
      typeof item.required !== 'boolean' ||
      typeof item.secret !== 'boolean'
    ) {
      throw new TypeError('Connector manifest configuration item is invalid.');
    }
    const name = requiredText(item.name, 128);
    assertUnique(names, name);
    return {
      name,
      description: requiredText(item.description, 500),
      required: item.required,
      secret: item.secret,
    };
  });
}

function parseTextList(value: unknown, maximumItems: number, maximumLength: number): string[] {
  if (!Array.isArray(value) || value.length > maximumItems) {
    throw new TypeError('Connector manifest declaration list is invalid.');
  }
  const values = value.map((item) => requiredText(item, maximumLength));
  const normalized = new Set<string>();
  for (const item of values) assertUnique(normalized, item);
  return values;
}

function requiredText(value: unknown, maximumLength: number): string {
  if (
    typeof value !== 'string' ||
    value.length === 0 ||
    value.length > maximumLength ||
    value.trim() !== value ||
    containsUnsafeControlCharacter(value)
  ) {
    throw new TypeError('Connector manifest text is invalid.');
  }
  return value;
}

function readOptionalText(value: Record<string, unknown>, key: string, maximumLength: number): string | undefined {
  if (!Object.hasOwn(value, key)) return undefined;
  return requiredText(value[key], maximumLength);
}

function containsUnsafeControlCharacter(value: string): boolean {
  for (const character of value) {
    const code = character.codePointAt(0);
    if (code !== undefined && (code < 32 || code === 127 || bidiControlCodePoints.has(code))) return true;
  }
  return false;
}

function assertUnique(values: Set<string>, value: string): void {
  const normalized = value.toLocaleLowerCase('en-US');
  if (values.has(normalized)) throw new TypeError('Connector manifest contains a duplicate declaration.');
  values.add(normalized);
}

function hasOnlyKeys(value: Record<string, unknown>, allowedKeys: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowedKeys.includes(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
