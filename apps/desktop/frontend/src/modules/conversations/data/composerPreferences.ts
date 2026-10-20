import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ComposerSubmissionMode = 'enter' | 'ctrlEnter';

export const composerSubmissionModeStorageKey = 'chat.composer_submission_mode';
export const defaultComposerSubmissionMode: ComposerSubmissionMode = 'enter';

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
}

export const useComposerPreferencesStore = create<ComposerPreferencesState>((set) => ({
  submissionMode: readComposerSubmissionMode(),
  setSubmissionMode(mode) {
    writeComposerSubmissionMode(mode);
    set({ submissionMode: mode });
  },
}));
