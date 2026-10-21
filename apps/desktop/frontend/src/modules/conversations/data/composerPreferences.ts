import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ComposerSubmissionMode = 'enter' | 'ctrlEnter';

export const composerSubmissionModeStorageKey = 'chat.composer_submission_mode';
export const composerGhostTextEnabledStorageKey = 'chat.composer_ghost_text_enabled';
export const composerGhostTextModelStorageKey = 'chat.composer_ghost_text_model';

export const defaultComposerSubmissionMode: ComposerSubmissionMode = 'enter';
export const defaultComposerGhostTextEnabled = true;
export const defaultComposerGhostTextModel = 'auto';

export function isComposerSubmissionMode(value: unknown): value is ComposerSubmissionMode {
  return value === 'enter' || value === 'ctrlEnter';
}

export function readComposerSubmissionMode(): ComposerSubmissionMode {
  try {
    const value = unifiedStorage.getItem(composerSubmissionModeStorageKey);
    return isComposerSubmissionMode(value) ? value : defaultComposerSubmissionMode;
  } catch {
    return defaultComposerSubmissionMode;
  }
}

export function writeComposerSubmissionMode(mode: ComposerSubmissionMode): void {
  unifiedStorage.setItem(composerSubmissionModeStorageKey, mode);
}

export function readComposerGhostTextEnabled(): boolean {
  try {
    const value = unifiedStorage.getItem(composerGhostTextEnabledStorageKey);
    if (value === null) return defaultComposerGhostTextEnabled;
    return value === 'true';
  } catch {
    return defaultComposerGhostTextEnabled;
  }
}

export function writeComposerGhostTextEnabled(enabled: boolean): void {
  unifiedStorage.setItem(composerGhostTextEnabledStorageKey, String(enabled));
}

export function readComposerGhostTextModel(): string {
  try {
    const value = unifiedStorage.getItem(composerGhostTextModelStorageKey);
    return typeof value === 'string' && value.trim().length > 0 ? value : defaultComposerGhostTextModel;
  } catch {
    return defaultComposerGhostTextModel;
  }
}

export function writeComposerGhostTextModel(modelKey: string): void {
  unifiedStorage.setItem(composerGhostTextModelStorageKey, modelKey);
}

export interface ComposerKeyModifiers {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function shouldSendFromKeyboard(event: ComposerKeyModifiers, mode: ComposerSubmissionMode): boolean {
  if (event.key !== 'Enter' || event.altKey) return false;
  if (mode === 'ctrlEnter') {
    return (event.ctrlKey || event.metaKey) && !event.shiftKey;
  }
  return !event.shiftKey && !event.ctrlKey && !event.metaKey;
}

export function shouldInsertNewlineFromKeyboard(event: ComposerKeyModifiers, mode: ComposerSubmissionMode): boolean {
  if (event.key !== 'Enter' || event.altKey) return false;
  if (mode === 'ctrlEnter') {
    return !event.ctrlKey && !event.metaKey;
  }
  return event.shiftKey && !event.ctrlKey && !event.metaKey;
}

export interface ComposerPreferencesState {
  submissionMode: ComposerSubmissionMode;
  setSubmissionMode(mode: ComposerSubmissionMode): void;
  ghostTextEnabled: boolean;
  setGhostTextEnabled(enabled: boolean): void;
  ghostTextModelKey: string;
  setGhostTextModelKey(modelKey: string): void;
}

export const useComposerPreferencesStore = create<ComposerPreferencesState>((set) => ({
  submissionMode: readComposerSubmissionMode(),
  ghostTextEnabled: readComposerGhostTextEnabled(),
  ghostTextModelKey: readComposerGhostTextModel(),
  setSubmissionMode(mode) {
    writeComposerSubmissionMode(mode);
    set({ submissionMode: mode });
  },
  setGhostTextEnabled(enabled) {
    writeComposerGhostTextEnabled(enabled);
    set({ ghostTextEnabled: enabled });
  },
  setGhostTextModelKey(modelKey) {
    writeComposerGhostTextModel(modelKey);
    set({ ghostTextModelKey: modelKey });
  },
}));
