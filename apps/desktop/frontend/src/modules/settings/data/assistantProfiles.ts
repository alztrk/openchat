import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import type { ReadyAssistantAgentTemplateId } from './readyAssistantAgents';

const storageKey = 'assistant.profiles.v1';
const maximumProfiles = 20;
const maximumNameLength = 64;
const maximumInstructionBytes = 16 * 1024;
const maximumAllowedTools = 32;

export type AssistantResponseStyle = 'concise' | 'balanced' | 'detailed';
export type AssistantProfileTemplateId = ReadyAssistantAgentTemplateId | 'coding' | 'editor';

export interface AssistantAgentSnapshot {
  id: string;
  name: string;
  instructions: string;
  responseStyle: AssistantResponseStyle;
  toolAllowlist: string[] | null;
}

export interface AssistantProfile extends AssistantAgentSnapshot {
  templateId?: AssistantProfileTemplateId;
}

export interface AssistantProfilesSnapshot {
  profiles: AssistantProfile[];
}

export function readAssistantProfiles(): AssistantProfilesSnapshot {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return { profiles: [] };
  return parseAssistantProfiles(JSON.parse(encoded));
}

export function writeAssistantProfiles(snapshot: AssistantProfilesSnapshot): Promise<void> {
  const parsed = parseAssistantProfiles(snapshot);
  if (parsed.profiles.length === 0) {
    return unifiedStorage.removePersistedItem(storageKey);
  }
  return unifiedStorage.setPersistedItem(storageKey, JSON.stringify(parsed));
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
  return { profiles };
}

export function parseAssistantProfile(value: unknown): AssistantProfile {
  if (!isAssistantAgentSnapshot(value)) throw new Error('An assistant profile is invalid.');
  const profile: AssistantProfile = {
    id: value.id,
    name: value.name,
    instructions: value.instructions,
    responseStyle: value.responseStyle,
    toolAllowlist: value.toolAllowlist === null ? null : [...value.toolAllowlist],
  };
  if ('templateId' in value) {
    if (
      value.templateId !== 'researcher' &&
      value.templateId !== 'debugger' &&
      value.templateId !== 'visual-designer' &&
      value.templateId !== 'security-reviewer' &&
      value.templateId !== 'coding' &&
      value.templateId !== 'editor'
    ) {
      throw new Error('An assistant profile template is invalid.');
    }
    profile.templateId = value.templateId;
  }
  return profile;
}

export function snapshotAssistantProfile(profile: AssistantProfile): AssistantAgentSnapshot {
  return {
    id: profile.id,
    name: profile.name,
    instructions: profile.instructions,
    responseStyle: profile.responseStyle,
    toolAllowlist: profile.toolAllowlist === null ? null : [...profile.toolAllowlist],
  };
}

export function isAssistantAgentSnapshot(value: unknown): value is AssistantAgentSnapshot {
  if (!isRecord(value)) return false;
  const { id, name, instructions, responseStyle, toolAllowlist } = value;
  if (
    typeof id !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id) ||
    typeof name !== 'string' ||
    name.trim() !== name ||
    name.length === 0 ||
    name.length > maximumNameLength ||
    typeof instructions !== 'string' ||
    new TextEncoder().encode(instructions).byteLength > maximumInstructionBytes ||
    (responseStyle !== 'concise' && responseStyle !== 'balanced' && responseStyle !== 'detailed') ||
    !(toolAllowlist === null || isToolAllowlist(toolAllowlist))
  ) {
    return false;
  }
  return true;
}

function isToolAllowlist(value: unknown): value is string[] {
  if (
    !isStringArray(value) ||
    value.length > maximumAllowedTools ||
    !value.every(isValidProfileToolName) ||
    new Set(value).size !== value.length
  ) {
    return false;
  }
  return true;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}

function isValidProfileToolName(name: string): boolean {
  if (/^[a-z][a-z0-9_-]{0,63}$/.test(name)) return true;
  return name.length <= 64 && /^mcp__[a-z0-9_]{1,24}__[A-Za-z0-9_-]+$/.test(name);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
