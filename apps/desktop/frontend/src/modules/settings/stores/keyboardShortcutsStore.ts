import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'openchat.keyboardShortcuts';

export type ComposerShortcutAction = 'sendMessage' | 'insertNewLine' | 'stopResponse';

export interface KeyboardShortcutBinding {
  code: string;
  control: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
}

export interface KeyboardShortcutBindings {
  sendMessage: KeyboardShortcutBinding;
  insertNewLine: KeyboardShortcutBinding;
  stopResponse: KeyboardShortcutBinding;
}

type LoadState = 'idle' | 'loading' | 'ready' | 'error';
type KeyboardShortcutError = 'keyboardShortcutLoadFailed' | 'keyboardShortcutSaveFailed' | 'keyboardShortcutConflict';

interface KeyboardShortcutsState {
  bindings: KeyboardShortcutBindings;
  loadState: LoadState;
  errorCode: KeyboardShortcutError | null;
  initialize(): void;
  retry(): void;
  updateBinding(action: ComposerShortcutAction, binding: KeyboardShortcutBinding): void;
  reset(): void;
  clearError(): void;
}

export const defaultKeyboardShortcuts: KeyboardShortcutBindings = {
  sendMessage: { code: 'Enter', control: false, alt: false, shift: false, meta: false },
  insertNewLine: { code: 'Enter', control: false, alt: false, shift: true, meta: false },
  stopResponse: { code: 'Escape', control: false, alt: false, shift: false, meta: false },
};

export const useKeyboardShortcutsStore = create<KeyboardShortcutsState>((set, get) => ({
  bindings: defaultKeyboardShortcuts,
  loadState: 'idle',
  errorCode: null,

  initialize() {
    if (get().loadState === 'loading' || get().loadState === 'ready') return;
    set({ loadState: 'loading', errorCode: null });
    try {
      const encoded = unifiedStorage.getItem(storageKey);
      const bindings = encoded === null ? defaultKeyboardShortcuts : parseBindings(JSON.parse(encoded));
      set({ bindings, loadState: 'ready' });
    } catch {
      set({ loadState: 'error', errorCode: 'keyboardShortcutLoadFailed' });
    }
  },

  retry() {
    set({ loadState: 'idle' });
    get().initialize();
  },

  updateBinding(action, binding) {
    const current = get().bindings;
    if (get().loadState !== 'ready') return;
    if (
      Object.entries(current).some(
        ([otherAction, otherBinding]) => otherAction !== action && hasSameKeys(otherBinding, binding),
      )
    ) {
      set({ errorCode: 'keyboardShortcutConflict' });
      return;
    }
    const nextBindings = { ...current, [action]: binding };
    try {
      unifiedStorage.setItem(storageKey, JSON.stringify(nextBindings));
      set({ bindings: nextBindings, errorCode: null });
    } catch {
      set({ errorCode: 'keyboardShortcutSaveFailed' });
    }
  },

  reset() {
    if (get().loadState === 'loading') return;
    try {
      unifiedStorage.setItem(storageKey, JSON.stringify(defaultKeyboardShortcuts));
      set({ bindings: defaultKeyboardShortcuts, loadState: 'ready', errorCode: null });
    } catch {
      set({ errorCode: 'keyboardShortcutSaveFailed' });
    }
  },

  clearError() {
    set({ errorCode: null });
  },
}));

export function keyboardShortcutMatches(event: KeyboardEvent, binding: KeyboardShortcutBinding): boolean {
  return (
    event.code === binding.code &&
    event.ctrlKey === binding.control &&
    event.altKey === binding.alt &&
    event.shiftKey === binding.shift &&
    event.metaKey === binding.meta
  );
}

export function formatKeyboardShortcut(binding: KeyboardShortcutBinding): string {
  const keyName =
    binding.code === 'Escape'
      ? 'Esc'
      : binding.code === 'Space'
        ? 'Space'
        : binding.code.startsWith('Digit')
          ? binding.code.slice(5)
          : binding.code.startsWith('Key')
            ? binding.code.slice(3)
            : binding.code;
  return [
    ...(binding.control ? ['Ctrl'] : []),
    ...(binding.alt ? ['Alt'] : []),
    ...(binding.shift ? ['Shift'] : []),
    ...(binding.meta ? ['Meta'] : []),
    keyName,
  ].join(' + ');
}

function parseBindings(value: unknown): KeyboardShortcutBindings {
  if (!isObject(value)) throw new Error('Keyboard shortcuts are invalid.');
  const bindings = {
    sendMessage: parseBinding(value.sendMessage),
    insertNewLine: parseBinding(value.insertNewLine),
    stopResponse: parseBinding(value.stopResponse),
  };
  const actionBindings = Object.values(bindings);
  if (
    actionBindings.some((binding, index) =>
      actionBindings.slice(index + 1).some((other) => hasSameKeys(binding, other)),
    )
  ) {
    throw new Error('Keyboard shortcuts conflict.');
  }
  return bindings;
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

function hasSameKeys(left: KeyboardShortcutBinding, right: KeyboardShortcutBinding): boolean {
  return (
    left.code === right.code &&
    left.control === right.control &&
    left.alt === right.alt &&
    left.shift === right.shift &&
    left.meta === right.meta
  );
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
