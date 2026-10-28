import { create } from 'zustand';
import { useAssistantExecutionPreferencesStore } from '@/modules/settings/data/assistantExecutionPreferences';
import {
  type AssistantProfile,
  type AssistantProfilesSnapshot,
  parseAssistantProfiles,
  readAssistantProfiles,
  writeAssistantProfiles,
} from '@/modules/settings/data/assistantProfiles';

type LoadState = 'idle' | 'loading' | 'ready' | 'error';
type ErrorKey =
  | 'assistantProfilesLoadFailed'
  | 'assistantProfileSaveFailed'
  | 'assistantProfileNameExists'
  | 'assistantProfilesLimitReached';

interface AssistantProfilesState extends AssistantProfilesSnapshot {
  loadState: LoadState;
  saveState: 'idle' | 'saving' | 'error';
  errorKey: ErrorKey | null;
  initialize(): void;
  retry(): void;
  saveProfile(profile: AssistantProfile): Promise<boolean>;
  removeProfile(profileId: string): Promise<boolean>;
  clearError(): void;
}

export const useAssistantProfilesStore = create<AssistantProfilesState>((set, get) => ({
  profiles: [],
  loadState: 'idle',
  saveState: 'idle',
  errorKey: null,

  initialize() {
    if (get().loadState === 'loading' || get().loadState === 'ready') return;
    set({ loadState: 'loading', errorKey: null });
    try {
      const snapshot = readAssistantProfiles();
      set({ ...snapshot, loadState: 'ready', errorKey: null });
    } catch {
      set({ loadState: 'error', errorKey: 'assistantProfilesLoadFailed' });
    }
  },

  retry() {
    set({ loadState: 'idle', errorKey: null });
    get().initialize();
  },

  async saveProfile(profile) {
    if (get().loadState !== 'ready' || get().saveState === 'saving') return false;
    const current = get();
    const isNameDuplicate = current.profiles.some(
      (item) =>
        item.id !== profile.id && item.name.toLocaleLowerCase('en-US') === profile.name.toLocaleLowerCase('en-US'),
    );
    if (isNameDuplicate) {
      set({ errorKey: 'assistantProfileNameExists' });
      return false;
    }
    const exists = current.profiles.some((item) => item.id === profile.id);
    if (!exists && current.profiles.length >= 20) {
      set({ errorKey: 'assistantProfilesLimitReached' });
      return false;
    }
    const profiles = exists
      ? current.profiles.map((item) => (item.id === profile.id ? profile : item))
      : [...current.profiles, profile];
    return persist({ profiles }, set);
  },

  async removeProfile(profileId) {
    if (get().loadState !== 'ready' || get().saveState === 'saving') return false;
    const current = get();
    const executionPreferences = useAssistantExecutionPreferencesStore.getState();
    if (executionPreferences.loadState === 'ready' && executionPreferences.preferences.defaultProfileId === profileId) {
      if (!(await executionPreferences.setPreference('defaultProfileId', null))) {
        set({ errorKey: 'assistantProfileSaveFailed' });
        return false;
      }
    }
    const snapshot = {
      profiles: current.profiles.filter((profile) => profile.id !== profileId),
    };
    return persist(snapshot, set);
  },

  clearError() {
    set({ errorKey: null });
  },
}));

async function persist(
  snapshot: AssistantProfilesSnapshot,
  set: (partial: Partial<AssistantProfilesState>) => void,
): Promise<boolean> {
  set({ saveState: 'saving' });
  try {
    const parsed = parseAssistantProfiles(snapshot);
    await writeAssistantProfiles(parsed);
    set({ ...parsed, loadState: 'ready', saveState: 'idle', errorKey: null });
    return true;
  } catch {
    set({ saveState: 'error', errorKey: 'assistantProfileSaveFailed' });
    return false;
  }
}
