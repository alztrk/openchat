import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'assistant.execution_preferences';
const goalRuntimeOptions = [0, 30, 60, 120, 240] as const;

export interface AssistantExecutionPreferences {
  defaultProfileId: string | null;
  preferredDelegationModelRouteKey: string | null;
  includeProjectSkills: boolean;
  maxGoalRuntimeMinutes: (typeof goalRuntimeOptions)[number];
}

type LoadState = 'idle' | 'ready' | 'error';
type SaveState = 'idle' | 'saving' | 'saved' | 'error';

interface AssistantExecutionPreferencesState {
  preferences: AssistantExecutionPreferences;
  loadState: LoadState;
  saveState: SaveState;
  initialize(): void;
  retry(): void;
  setPreference<K extends keyof AssistantExecutionPreferences>(
    key: K,
    value: AssistantExecutionPreferences[K],
  ): Promise<boolean>;
  reset(): Promise<boolean>;
}

export const defaultAssistantExecutionPreferences: AssistantExecutionPreferences = {
  defaultProfileId: null,
  preferredDelegationModelRouteKey: null,
  includeProjectSkills: true,
  maxGoalRuntimeMinutes: 0,
};

export const useAssistantExecutionPreferencesStore = create<AssistantExecutionPreferencesState>((set, get) => ({
  preferences: defaultAssistantExecutionPreferences,
  loadState: 'idle',
  saveState: 'idle',

  initialize() {
    if (get().loadState === 'ready') return;
    try {
      set({ preferences: readAssistantExecutionPreferences(), loadState: 'ready', saveState: 'idle' });
    } catch {
      set({ loadState: 'error', saveState: 'idle' });
    }
  },

  retry() {
    set({ loadState: 'idle' });
    get().initialize();
  },

  async setPreference(key, value) {
    if (get().loadState !== 'ready') return false;
    const next = { ...get().preferences, [key]: value };
    set({ saveState: 'saving' });
    try {
      validatePreferences(next);
      await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(next));
      set({ preferences: next, saveState: 'saved' });
      return true;
    } catch {
      set({ saveState: 'error' });
      return false;
    }
  },

  async reset() {
    set({ saveState: 'saving' });
    try {
      await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(defaultAssistantExecutionPreferences));
      set({
        preferences: defaultAssistantExecutionPreferences,
        loadState: 'ready',
        saveState: 'saved',
      });
      return true;
    } catch {
      set({ saveState: 'error' });
      return false;
    }
  },
}));

export function readAssistantExecutionPreferences(): AssistantExecutionPreferences {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return defaultAssistantExecutionPreferences;
  const value: unknown = JSON.parse(encoded);
  if (!isObject(value)) throw new Error('Saved assistant execution preferences are invalid.');
  const defaultProfileId = value.defaultProfileId === undefined ? null : value.defaultProfileId;
  const preferredDelegationModelRouteKey =
    value.preferredDelegationModelRouteKey === undefined ? null : value.preferredDelegationModelRouteKey;
  const includeProjectSkills =
    value.includeProjectSkills === undefined
      ? defaultAssistantExecutionPreferences.includeProjectSkills
      : value.includeProjectSkills;
  const maxGoalRuntimeMinutes =
    value.maxGoalRuntimeMinutes === undefined
      ? defaultAssistantExecutionPreferences.maxGoalRuntimeMinutes
      : value.maxGoalRuntimeMinutes;
  if (
    !(defaultProfileId === null || typeof defaultProfileId === 'string') ||
    !(preferredDelegationModelRouteKey === null || typeof preferredDelegationModelRouteKey === 'string') ||
    typeof includeProjectSkills !== 'boolean' ||
    !isGoalRuntimeOption(maxGoalRuntimeMinutes)
  ) {
    throw new Error('Saved assistant execution preferences are invalid.');
  }
  const preferences = {
    defaultProfileId,
    preferredDelegationModelRouteKey,
    includeProjectSkills,
    maxGoalRuntimeMinutes,
  };
  validatePreferences(preferences);
  return preferences;
}

function validatePreferences(value: AssistantExecutionPreferences): void {
  if (
    !(value.defaultProfileId === null || isUuid(value.defaultProfileId)) ||
    !isBoundedRouteKey(value.preferredDelegationModelRouteKey) ||
    typeof value.includeProjectSkills !== 'boolean' ||
    !isGoalRuntimeOption(value.maxGoalRuntimeMinutes)
  ) {
    throw new Error('Saved assistant execution preferences are invalid.');
  }
}

function isGoalRuntimeOption(value: unknown): value is (typeof goalRuntimeOptions)[number] {
  return goalRuntimeOptions.some((option) => option === value);
}

function isUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function isBoundedRouteKey(value: string | null): boolean {
  return value === null || (value.length > 0 && value.length <= 1024 && !containsControlCharacters(value));
}

function containsControlCharacters(value: string): boolean {
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code < 0x20 || code === 0x7f) return true;
  }
  return false;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
