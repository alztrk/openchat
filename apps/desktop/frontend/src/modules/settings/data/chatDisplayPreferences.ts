import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'chat.display_preferences';

export interface ChatDisplayPreferences {
  autoFollowResponses: boolean;
  wrapCode: boolean;
  copyFormat: 'markdown' | 'plainText';
}

type LoadState = 'idle' | 'ready' | 'error';
type SaveState = 'idle' | 'saving' | 'saved' | 'error';

interface ChatDisplayPreferencesState {
  preferences: ChatDisplayPreferences;
  loadState: LoadState;
  saveState: SaveState;
  initialize(): void;
  retry(): void;
  setPreference<K extends keyof ChatDisplayPreferences>(key: K, value: ChatDisplayPreferences[K]): Promise<boolean>;
  reset(): Promise<boolean>;
}

export const defaultChatDisplayPreferences: ChatDisplayPreferences = {
  autoFollowResponses: true,
  wrapCode: true,
  copyFormat: 'markdown',
};

export const useChatDisplayPreferencesStore = create<ChatDisplayPreferencesState>((set, get) => ({
  preferences: defaultChatDisplayPreferences,
  loadState: 'idle',
  saveState: 'idle',

  initialize() {
    if (get().loadState === 'ready') return;
    try {
      set({ preferences: readPreferences(), loadState: 'ready', saveState: 'idle' });
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
      await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(defaultChatDisplayPreferences));
      set({ preferences: defaultChatDisplayPreferences, loadState: 'ready', saveState: 'saved' });
      return true;
    } catch {
      set({ saveState: 'error' });
      return false;
    }
  },
}));

function readPreferences(): ChatDisplayPreferences {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return defaultChatDisplayPreferences;
  const value: unknown = JSON.parse(encoded);
  if (!isObject(value)) throw new Error('Saved chat display preferences are invalid.');

  const autoFollowResponses = value.autoFollowResponses ?? defaultChatDisplayPreferences.autoFollowResponses;
  const wrapCode = value.wrapCode ?? defaultChatDisplayPreferences.wrapCode;
  const copyFormat = value.copyFormat ?? defaultChatDisplayPreferences.copyFormat;
  if (
    typeof autoFollowResponses !== 'boolean' ||
    typeof wrapCode !== 'boolean' ||
    (copyFormat !== 'markdown' && copyFormat !== 'plainText')
  ) {
    throw new Error('Saved chat display preferences are invalid.');
  }
  return { autoFollowResponses, wrapCode, copyFormat };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
