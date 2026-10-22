import { create } from 'zustand';

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
  errorKey: ErrorKey | null;
  initialize(): void;
  retry(): void;
  saveProfile(profile: AssistantProfile): boolean;
  removeProfile(profileId: string): boolean;
  selectProfile(profileId: string | null): boolean;
  getActiveProfile(): AssistantProfile | null;
  clearError(): void;
}

export const useAssistantProfilesStore = create<AssistantProfilesState>((set, get) => ({
  profiles: [],
  activeProfileId: null,
  loadState: 'idle',
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

  saveProfile(profile) {
    if (get().loadState !== 'ready') return false;
    const current = get();
    const isNameDuplicate = current.profiles.some(
      (item) => item.id !== profile.id && item.name.toLocaleLowerCase('en-US') === profile.name.toLocaleLowerCase('en-US'),
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
    return persist({ profiles, activeProfileId: current.activeProfileId }, set);
  },

  removeProfile(profileId) {
    if (get().loadState !== 'ready') return false;
    const current = get();
    const snapshot = {
      profiles: current.profiles.filter((profile) => profile.id !== profileId),
      activeProfileId: current.activeProfileId === profileId ? null : current.activeProfileId,
    };
    return persist(snapshot, set);
  },

  selectProfile(profileId) {
    if (get().loadState !== 'ready') return false;
    const current = get();
    if (profileId !== null && !current.profiles.some((profile) => profile.id === profileId)) return false;
    return persist({ profiles: current.profiles, activeProfileId: profileId }, set);
  },

  getActiveProfile() {
    const current = get();
    return current.profiles.find((profile) => profile.id === current.activeProfileId) ?? null;
  },

  clearError() {
    set({ errorKey: null });
  },
}));

function persist(
  snapshot: AssistantProfilesSnapshot,
  set: (partial: Partial<AssistantProfilesState>) => void,
): boolean {
  try {
    const parsed = parseAssistantProfiles(snapshot);
    writeAssistantProfiles(parsed);
    set({ ...parsed, loadState: 'ready', errorKey: null });
    return true;
  } catch {
    set({ errorKey: 'assistantProfileSaveFailed' });
    return false;
  }
}
