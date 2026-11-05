import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const globalShortcutsSource = readFileSync(path.resolve('src/bootstrap/useAppKeyboardShortcuts.ts'), 'utf8');
const shortcutRecorderSource = readFileSync(
  path.resolve('src/features/app-settings/components/ShortcutsSection.tsx'),
  'utf8',
);

test('shortcut recording intercepts keydown before the global shortcut listener', () => {
  assert.match(shortcutRecorderSource, /window\.addEventListener\('keydown', handleKeyDown, true\)/u);
  assert.match(shortcutRecorderSource, /event\.preventDefault\(\);\s*event\.stopPropagation\(\);/u);
  assert.match(globalShortcutsSource, /window\.addEventListener\('keydown', handleGlobalShortcut\);/u);
  assert.match(globalShortcutsSource, /window\.removeEventListener\('keydown', handleGlobalShortcut\);/u);
  assert.doesNotMatch(globalShortcutsSource, /window\.addEventListener\('keydown', handleGlobalShortcut, true\)/u);
  assert.doesNotMatch(globalShortcutsSource, /window\.removeEventListener\('keydown', handleGlobalShortcut, true\)/u);
});

test('unprevented keydown events continue to reach normal global shortcut dispatch', () => {
  assert.match(globalShortcutsSource, /if \(event\.defaultPrevented \|\| event\.repeat\) return;/u);
  assert.match(globalShortcutsSource, /keyboardShortcutMatches\(event, binding\)/u);
  assert.match(globalShortcutsSource, /event\.preventDefault\(\);\s*switch \(action\)/u);
});
