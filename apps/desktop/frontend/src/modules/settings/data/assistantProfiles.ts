import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import type { DefaultModelPreference } from '@/modules/providers/data/modelPreferences';

const storageKey = 'assistant.profiles.v1';
const maximumProfiles = 20;
const maximumNameLength = 64;
const maximumInstructionBytes = 16 * 1024;
const maximumAllowedTools = 32;

export type AssistantResponseStyle = 'concise' | 'balanced' | 'detailed';

export interface AssistantProfile {
  id: string;
  name: string;
  instructions: string;
  responseStyle: AssistantResponseStyle;
  preferredModel: DefaultModelPreference | null;
  toolAllowlist: string[] | null;
}

export interface AssistantProfilesSnapshot {
  profiles: AssistantProfile[];
  activeProfileId: string | null;
}

export function readAssistantProfiles(): AssistantProfilesSnapshot {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return { profiles: [], activeProfileId: null };
  return parseAssistantProfiles(JSON.parse(encoded));
}

export function writeAssistantProfiles(snapshot: AssistantProfilesSnapshot): void {
  const parsed = parseAssistantProfiles(snapshot);
  if (parsed.profiles.length === 0 && parsed.activeProfileId === null) {
    unifiedStorage.removeItem(storageKey);
    return;
  }
  unifiedStorage.setItem(storageKey, JSON.stringify(parsed));
}

export function parseAssistantProfiles(value: unknown): AssistantProfilesSnapshot {
  if (!isRecord(value) || !Array.isArray(value.profiles) || value.profiles.length > maximumProfiles) {
    throw new Error('Assistant profiles are invalid.');
  }
  const profiles = value.profiles.map(parseAssistantProfile);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const profile of profiles) {
    const normalizedName = profile.name.toLocaleLowerCase('en-US');
    if (ids.has(profile.id) || names.has(normalizedName)) throw new Error('Assistant profiles are duplicated.');
    ids.add(profile.id);
    names.add(normalizedName);
  }
  const activeProfileId = value.activeProfileId;
  if (activeProfileId !== null && (typeof activeProfileId !== 'string' || !ids.has(activeProfileId))) {
    throw new Error('The active assistant profile is invalid.');
  }
  return { profiles, activeProfileId };
}

export function parseAssistantProfile(value: unknown): AssistantProfile {
  if (!isRecord(value)) throw new Error('An assistant profile is invalid.');
  const { id, name, instructions, responseStyle } = value;
  if (
    typeof id !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id) ||
    typeof name !== 'string' ||
    name.trim() !== name ||
    name.length === 0 ||
    name.length > maximumNameLength ||
    typeof instructions !== 'string' ||
    new TextEncoder().encode(instructions).byteLength > maximumInstructionBytes ||
    (responseStyle !== 'concise' && responseStyle !== 'balanced' && responseStyle !== 'detailed')
  ) {
    throw new Error('An assistant profile is invalid.');
  }

  const preferredModel = value.preferredModel === null ? null : parsePreferredModel(value.preferredModel);
  const toolAllowlist = value.toolAllowlist === null ? null : parseToolAllowlist(value.toolAllowlist);
  return { id, name, instructions, responseStyle, preferredModel, toolAllowlist };
}

function parsePreferredModel(value: unknown): DefaultModelPreference {
  if (!isRecord(value)) throw new Error('The assistant profile model is invalid.');
  const { providerId, modelId, displayName, connectionId, workspaceId, apiKeyConnectionId } = value;
  if (
    typeof providerId !== 'string' ||
    !/^[a-z][a-z0-9_-]{0,63}$/.test(providerId) ||
    typeof modelId !== 'string' ||
    modelId.trim() !== modelId ||
    modelId.length === 0 ||
    modelId.length > 256 ||
    typeof displayName !== 'string' ||
    displayName.trim() !== displayName ||
    displayName.length === 0 ||
    displayName.length > 256 ||
    !isOptionalIdentifier(connectionId) ||
    !isOptionalIdentifier(workspaceId) ||
    !isOptionalIdentifier(apiKeyConnectionId)
  ) {
    throw new Error('The assistant profile model is invalid.');
  }
  return { providerId, modelId, displayName, connectionId, workspaceId, apiKeyConnectionId };
}

function parseToolAllowlist(value: unknown): string[] {
  if (
    !isStringArray(value) ||
    value.length > maximumAllowedTools ||
    !value.every(isValidProfileToolName) ||
    new Set(value).size !== value.length
  ) {
    throw new Error('The assistant profile tool list is invalid.');
  }
  return value;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}

function isValidProfileToolName(name: string): boolean {
  if (/^[a-z][a-z0-9_-]{0,63}$/.test(name)) return true;
  return name.length <= 64 && /^mcp__[a-z0-9_]{1,24}__[A-Za-z0-9_-]+$/.test(name);
}

function isOptionalIdentifier(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && /^[a-zA-Z0-9_-]{1,128}$/.test(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
