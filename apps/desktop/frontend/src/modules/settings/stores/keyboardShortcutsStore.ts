import { create } from 'zustand';

import {
  type ComposerSubmissionMode,
  readComposerSubmissionMode,
} from '@/modules/conversations/data/composerPreferences';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'openchat.keyboardShortcuts';

export const keyboardShortcutActions = [
  'sendMessage',
  'insertNewLine',
  'stopResponse',
  'newChat',
  'openSearch',
  'openModelPicker',
  'openCommandPalette',
  'toggleRightPanel',
  'previousConversation',
  'nextConversation',
] as const;

export type KeyboardShortcutAction = (typeof keyboardShortcutActions)[number];
export type ComposerShortcutAction = Extract<KeyboardShortcutAction, 'sendMessage' | 'insertNewLine' | 'stopResponse'>;

export interface KeyboardShortcutBinding {
  code: string;
  control: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
}

export type KeyboardShortcutBindings = Record<KeyboardShortcutAction, KeyboardShortcutBinding | null>;
export type ComposerShortcutPreset = ComposerSubmissionMode | 'custom';
type LoadState = 'idle' | 'loading' | 'ready' | 'error';
type SaveState = 'idle' | 'saving' | 'saved' | 'error';
type KeyboardShortcutError = 'keyboardShortcutLoadFailed' | 'keyboardShortcutSaveFailed';

export type KeyboardShortcutChangeResult =
  | { status: 'saved' }
  | { status: 'conflict'; conflictingActions: KeyboardShortcutAction[] }
  | { status: 'requiresModifier' }
  | { status: 'notReady' }
  | { status: 'saveFailed' };

interface KeyboardShortcutsState {
  bindings: KeyboardShortcutBindings;
  loadState: LoadState;
  saveState: SaveState;
  errorCode: KeyboardShortcutError | null;
  initialize(): void;
  retry(): void;
  updateBinding(
    action: KeyboardShortcutAction,
    binding: KeyboardShortcutBinding | null,
    replaceConflicts?: boolean,
  ): Promise<KeyboardShortcutChangeResult>;
  resetBinding(action: KeyboardShortcutAction, replaceConflicts?: boolean): Promise<KeyboardShortcutChangeResult>;
  applyComposerPreset(mode: ComposerSubmissionMode, replaceConflicts?: boolean): Promise<KeyboardShortcutChangeResult>;
  reset(): Promise<void>;
  clearError(): void;
}

export const defaultKeyboardShortcuts: KeyboardShortcutBindings = {
  sendMessage: { code: 'Enter', control: false, alt: false, shift: false, meta: false },
  insertNewLine: { code: 'Enter', control: false, alt: false, shift: true, meta: false },
  stopResponse: { code: 'Escape', control: false, alt: false, shift: false, meta: false },
  newChat: { code: 'KeyN', control: true, alt: false, shift: false, meta: false },
  openSearch: { code: 'KeyF', control: true, alt: false, shift: false, meta: false },
  openModelPicker: { code: 'KeyM', control: true, alt: false, shift: true, meta: false },
  openCommandPalette: { code: 'KeyK', control: true, alt: false, shift: false, meta: false },
  toggleRightPanel: { code: 'KeyP', control: true, alt: true, shift: false, meta: false },
  previousConversation: { code: 'ArrowUp', control: false, alt: true, shift: false, meta: false },
  nextConversation: { code: 'ArrowDown', control: false, alt: true, shift: false, meta: false },
};

const ctrlEnterComposerShortcuts = {
  sendMessage: { code: 'Enter', control: true, alt: false, shift: false, meta: false },
  insertNewLine: { code: 'Enter', control: false, alt: false, shift: false, meta: false },
} satisfies Pick<KeyboardShortcutBindings, 'sendMessage' | 'insertNewLine'>;

export const useKeyboardShortcutsStore = create<KeyboardShortcutsState>((set, get) => {
  const saveBindings = async (
    bindings: KeyboardShortcutBindings,
    loadState?: LoadState,
  ): Promise<KeyboardShortcutChangeResult> => {
    set({ saveState: 'saving' });
    try {
      await unifiedStorage.setPersistedItem(storageKey, JSON.stringify({ version: 2, bindings }));
      set({ bindings, errorCode: null, saveState: 'saved', ...(loadState ? { loadState } : {}) });
      return { status: 'saved' };
    } catch {
      set({ errorCode: 'keyboardShortcutSaveFailed', saveState: 'error' });
      return { status: 'saveFailed' };
    }
  };

  return {
    bindings: defaultKeyboardShortcuts,
    loadState: 'idle',
    saveState: 'idle',
    errorCode: null,

    initialize() {
      if (get().loadState === 'loading' || get().loadState === 'ready') return;
      set({ loadState: 'loading', errorCode: null });
      try {
        const encoded = unifiedStorage.getItem(storageKey);
        const legacyMode = readComposerSubmissionMode();
        const bindings =
          encoded === null
            ? keyboardShortcutsForSubmissionMode(legacyMode)
            : parseStoredBindings(JSON.parse(encoded), legacyMode);
        set({ bindings, loadState: 'ready' });
      } catch {
        set({ loadState: 'error', errorCode: 'keyboardShortcutLoadFailed' });
      }
    },

    retry() {
      set({ loadState: 'idle' });
      get().initialize();
    },

    async updateBinding(action, binding, replaceConflicts = false) {
      const current = get();
      if (current.loadState !== 'ready' || current.saveState === 'saving') return { status: 'notReady' };
      if (binding !== null && keyboardShortcutRequiresModifier(action) && !hasSystemModifier(binding)) {
        return { status: 'requiresModifier' };
      }

      const next: KeyboardShortcutBindings = { ...current.bindings, [action]: binding };
      const conflicts = conflictingActions(next, [action]);
      if (conflicts.length > 0 && !replaceConflicts) return { status: 'conflict', conflictingActions: conflicts };
      for (const conflict of conflicts) next[conflict] = null;
      return saveBindings(next);
    },

    async resetBinding(action, replaceConflicts = false) {
      return get().updateBinding(action, defaultKeyboardShortcuts[action], replaceConflicts);
    },

    async applyComposerPreset(mode, replaceConflicts = false) {
      const current = get();
      if (current.loadState !== 'ready' || current.saveState === 'saving') return { status: 'notReady' };

      const composerBindings = keyboardShortcutsForSubmissionMode(mode);
      const next: KeyboardShortcutBindings = { ...current.bindings, ...composerBindings };
      const conflicts = conflictingActions(next, ['sendMessage', 'insertNewLine']);
      if (conflicts.length > 0 && !replaceConflicts) return { status: 'conflict', conflictingActions: conflicts };
      for (const conflict of conflicts) next[conflict] = null;
      return saveBindings(next);
    },

    async reset() {
      if (get().loadState === 'loading' || get().saveState === 'saving') return;
      await saveBindings(defaultKeyboardShortcuts, 'ready');
    },

    clearError() {
      set({ errorCode: null });
    },
  };
});

export function keyboardShortcutRequiresModifier(action: KeyboardShortcutAction): boolean {
  return action !== 'sendMessage' && action !== 'insertNewLine' && action !== 'stopResponse';
}

function hasSystemModifier(binding: KeyboardShortcutBinding): boolean {
  return binding.control || binding.alt || binding.meta;
}

export function keyboardShortcutMatches(
  event: Pick<KeyboardEvent, 'altKey' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
  binding: KeyboardShortcutBinding | null,
): boolean {
  return (
    binding !== null &&
    event.code === binding.code &&
    event.ctrlKey === binding.control &&
    event.altKey === binding.alt &&
    event.shiftKey === binding.shift &&
    event.metaKey === binding.meta
  );
}

export function formatKeyboardShortcut(binding: KeyboardShortcutBinding): string {
  const platform = typeof navigator === 'undefined' ? '' : navigator.platform;
  const metaLabel = /mac/i.test(platform)
    ? '⌘'
    : /win/i.test(platform)
      ? 'Win'
      : /linux/i.test(platform)
        ? 'Super'
        : 'Meta';
  const keyName = keyNameForCode(binding.code);
  return [
    ...(binding.control ? ['Ctrl'] : []),
    ...(binding.alt ? ['Alt'] : []),
    ...(binding.shift ? ['Shift'] : []),
    ...(binding.meta ? [metaLabel] : []),
    keyName,
  ].join(' + ');
}

export function sameKeyboardShortcut(
  left: KeyboardShortcutBinding | null,
  right: KeyboardShortcutBinding | null,
): boolean {
  if (left === null || right === null) return left === right;
  return (
    left.code === right.code &&
    left.control === right.control &&
    left.alt === right.alt &&
    left.shift === right.shift &&
    left.meta === right.meta
  );
}

export function composerShortcutPreset(bindings: KeyboardShortcutBindings): ComposerShortcutPreset {
  if (
    sameKeyboardShortcut(bindings.sendMessage, defaultKeyboardShortcuts.sendMessage) &&
    sameKeyboardShortcut(bindings.insertNewLine, defaultKeyboardShortcuts.insertNewLine)
  ) {
    return 'enter';
  }
  if (
    sameKeyboardShortcut(bindings.sendMessage, ctrlEnterComposerShortcuts.sendMessage) &&
    sameKeyboardShortcut(bindings.insertNewLine, ctrlEnterComposerShortcuts.insertNewLine)
  ) {
    return 'ctrlEnter';
  }
  return 'custom';
}

function keyboardShortcutsForSubmissionMode(mode: ComposerSubmissionMode): KeyboardShortcutBindings {
  return mode === 'ctrlEnter'
    ? { ...defaultKeyboardShortcuts, ...ctrlEnterComposerShortcuts }
    : defaultKeyboardShortcuts;
}

function conflictingActions(
  bindings: KeyboardShortcutBindings,
  candidateActions: readonly KeyboardShortcutAction[],
): KeyboardShortcutAction[] {
  const conflicts = new Set<KeyboardShortcutAction>();
  for (const action of candidateActions) {
    const binding = bindings[action];
    if (binding === null) continue;
    for (const otherAction of keyboardShortcutActions) {
      const otherBinding = bindings[otherAction];
      if (otherAction !== action && otherBinding !== null && sameKeyboardShortcut(binding, otherBinding)) {
        conflicts.add(otherAction);
      }
    }
  }
  return [...conflicts];
}

function parseStoredBindings(value: unknown, legacyMode: ComposerSubmissionMode): KeyboardShortcutBindings {
  if (isObject(value) && value.version === 2) return parseBindings(value.bindings);

  const bindings = parseBindings(value);
  const hasDefaultComposerBindings =
    sameKeyboardShortcut(bindings.sendMessage, defaultKeyboardShortcuts.sendMessage) &&
    sameKeyboardShortcut(bindings.insertNewLine, defaultKeyboardShortcuts.insertNewLine);
  return legacyMode === 'ctrlEnter' && hasDefaultComposerBindings
    ? { ...bindings, ...ctrlEnterComposerShortcuts }
    : bindings;
}

function parseBindings(value: unknown): KeyboardShortcutBindings {
  if (!isObject(value)) throw new Error('Keyboard shortcuts are invalid.');
  const bindings: KeyboardShortcutBindings = {
    sendMessage: parseOptionalBinding(value.sendMessage, defaultKeyboardShortcuts.sendMessage),
    insertNewLine: parseOptionalBinding(value.insertNewLine, defaultKeyboardShortcuts.insertNewLine),
    stopResponse: parseOptionalBinding(value.stopResponse, defaultKeyboardShortcuts.stopResponse),
    newChat: parseOptionalBinding(value.newChat, defaultKeyboardShortcuts.newChat),
    openSearch: parseOptionalBinding(value.openSearch, defaultKeyboardShortcuts.openSearch),
    openModelPicker: parseOptionalBinding(value.openModelPicker, defaultKeyboardShortcuts.openModelPicker),
    openCommandPalette: parseOptionalBinding(value.openCommandPalette, defaultKeyboardShortcuts.openCommandPalette),
    toggleRightPanel: parseOptionalBinding(value.toggleRightPanel, defaultKeyboardShortcuts.toggleRightPanel),
    previousConversation: parseOptionalBinding(
      value.previousConversation,
      defaultKeyboardShortcuts.previousConversation,
    ),
    nextConversation: parseOptionalBinding(value.nextConversation, defaultKeyboardShortcuts.nextConversation),
  };
  for (const action of keyboardShortcutActions) {
    if (bindings[action] !== null && keyboardShortcutRequiresModifier(action) && !hasSystemModifier(bindings[action])) {
      bindings[action] = null;
    }
  }
  if (conflictingActions(bindings, keyboardShortcutActions).length > 0) {
    throw new Error('Keyboard shortcuts conflict.');
  }
  return bindings;
}

function parseOptionalBinding(
  value: unknown,
  fallback: KeyboardShortcutBinding | null,
): KeyboardShortcutBinding | null {
  if (value === undefined) return fallback;
  if (value === null) return null;
  return parseBinding(value);
}

function parseBinding(value: unknown): KeyboardShortcutBinding {
  if (
    !isObject(value) ||
    typeof value.code !== 'string' ||
    !/^[A-Za-z][A-Za-z0-9]{0,31}$/.test(value.code) ||
    typeof value.control !== 'boolean' ||
    typeof value.alt !== 'boolean' ||
    typeof value.shift !== 'boolean' ||
    typeof value.meta !== 'boolean'
  ) {
    throw new Error('A keyboard shortcut binding is invalid.');
  }
  return {
    code: value.code,
    control: value.control,
    alt: value.alt,
    shift: value.shift,
    meta: value.meta,
  };
}

function keyNameForCode(code: string): string {
  const namedKeys: Record<string, string> = {
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
    ArrowUp: 'Up',
    Backspace: 'Backspace',
    Delete: 'Delete',
    End: 'End',
    Enter: 'Enter',
    Escape: 'Esc',
    Home: 'Home',
    PageDown: 'Page Down',
    PageUp: 'Page Up',
    Space: 'Space',
    Tab: 'Tab',
  };
  if (namedKeys[code]) return namedKeys[code];
  if (code.startsWith('Digit')) return code.slice(5);
  if (code.startsWith('Key')) return code.slice(3).toUpperCase();
  if (code.startsWith('Numpad')) return `Num ${code.slice(6)}`;
  return code;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
