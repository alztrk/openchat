import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

// Storage mock for unifiedStorage
const storage = new Map();
const unifiedStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
};

const composerSubmissionModeStorageKey = 'chat.composer_submission_mode';
const defaultComposerSubmissionMode = 'enter';

function isComposerSubmissionMode(value) {
  return value === 'enter' || value === 'ctrlEnter';
}

function readComposerSubmissionMode() {
  try {
    const value = unifiedStorage.getItem(composerSubmissionModeStorageKey);
    return isComposerSubmissionMode(value) ? value : defaultComposerSubmissionMode;
  } catch {
    return defaultComposerSubmissionMode;
  }
}

function writeComposerSubmissionMode(mode) {
  unifiedStorage.setItem(composerSubmissionModeStorageKey, mode);
}

function shouldSendFromKeyboard(event, mode) {
  if (event.key !== 'Enter' || event.altKey) return false;
  if (mode === 'ctrlEnter') {
    return (event.ctrlKey || event.metaKey) && !event.shiftKey;
  }
  return !event.shiftKey && !event.ctrlKey && !event.metaKey;
}

function shouldInsertNewlineFromKeyboard(event, mode) {
  if (event.key !== 'Enter' || event.altKey) return false;
  if (mode === 'ctrlEnter') {
    return !event.ctrlKey && !event.metaKey;
  }
  return event.shiftKey && !event.ctrlKey && !event.metaKey;
}

test('composer submission mode loads default value cleanly', () => {
  storage.clear();
  assert.equal(readComposerSubmissionMode(), 'enter');
});

test('composer submission mode persists updates across sessions', () => {
  storage.clear();
  writeComposerSubmissionMode('ctrlEnter');
  assert.equal(readComposerSubmissionMode(), 'ctrlEnter');
  assert.equal(storage.get(composerSubmissionModeStorageKey), 'ctrlEnter');

  writeComposerSubmissionMode('enter');
  assert.equal(readComposerSubmissionMode(), 'enter');
  assert.equal(storage.get(composerSubmissionModeStorageKey), 'enter');
});

test('composer submission mode falls back to default on invalid values', () => {
  storage.clear();
  storage.set(composerSubmissionModeStorageKey, 'invalid_mode');
  assert.equal(readComposerSubmissionMode(), 'enter');

  storage.set(composerSubmissionModeStorageKey, '12345');
  assert.equal(readComposerSubmissionMode(), 'enter');
});

test('Option 1 (Enter): keyboard send and newline rules', () => {
  const mode = 'enter';

  // Plain Enter sends
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, mode),
    true,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false },
      mode,
    ),
    false,
  );

  // Shift+Enter inserts newline, does NOT send
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: true }, mode),
    false,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: true },
      mode,
    ),
    true,
  );

  // Ctrl+Enter does not send in Option 1
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false }, mode),
    false,
  );

  // Cmd+Enter does not send in Option 1
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: true, altKey: false, shiftKey: false }, mode),
    false,
  );

  // Non-Enter keys do not send
  assert.equal(
    shouldSendFromKeyboard({ key: 'a', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, mode),
    false,
  );
});

test('Option 2 (Ctrl/Cmd+Enter): keyboard send and newline rules', () => {
  const mode = 'ctrlEnter';

  // Plain Enter inserts newline, does NOT send
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, mode),
    false,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false },
      mode,
    ),
    true,
  );

  // Shift+Enter inserts newline, does NOT send
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: true }, mode),
    false,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: true },
      mode,
    ),
    true,
  );

  // Ctrl+Enter sends, does not insert newline
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false }, mode),
    true,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false },
      mode,
    ),
    false,
  );

  // Cmd+Enter (metaKey) sends, does not insert newline
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: false, metaKey: true, altKey: false, shiftKey: false }, mode),
    true,
  );
  assert.equal(
    shouldInsertNewlineFromKeyboard(
      { key: 'Enter', ctrlKey: false, metaKey: true, altKey: false, shiftKey: false },
      mode,
    ),
    false,
  );

  // Ctrl+Shift+Enter does not send
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: true, metaKey: false, altKey: false, shiftKey: true }, mode),
    false,
  );

  // Alt+Enter does not send
  assert.equal(
    shouldSendFromKeyboard({ key: 'Enter', ctrlKey: true, metaKey: false, altKey: true, shiftKey: false }, mode),
    false,
  );
});

test('all 5 locales define all required composer send key binding strings', () => {
  const locales = ['en', 'tr', 'de', 'es', 'fr'];
  const requiredKeys = [
    'composerSendKeyBinding',
    'composerSendKeyBindingDescription',
    'composerSubmissionModeEnter',
    'composerSubmissionModeCtrlEnter',
    'composerPreferenceSaveFailed',
    'keyboardHintCtrlEnter',
    'composerPlaceholderEnter',
    'composerPlaceholderCtrlEnter',
    'composerSendTooltipEnter',
    'composerSendTooltipCtrlEnter',
  ];

  for (const locale of locales) {
    const fileUrl = new URL(`../src/localization/locales/${locale}.json`, import.meta.url);
    const content = JSON.parse(readFileSync(fileUrl, 'utf8'));

    for (const key of requiredKeys) {
      assert.ok(
        typeof content[key] === 'string' && content[key].length > 0,
        `Missing or empty translation key "${key}" in ${locale}.json`,
      );
    }
  }
});
