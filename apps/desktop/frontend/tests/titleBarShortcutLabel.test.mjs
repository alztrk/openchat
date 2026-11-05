import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const titleBarSource = readFileSync(path.resolve('src/widgets/title-bar/TitleBar.tsx'), 'utf8');

test('the command palette button displays the current binding and localized unassigned state', () => {
  assert.match(titleBarSource, /useKeyboardShortcutsStore\(\(state\) => state\.bindings\.openCommandPalette\)/u);
  assert.match(titleBarSource, /formatKeyboardShortcut\(commandPaletteBinding\)/u);
  assert.match(titleBarSource, /t\('keyboardShortcutUnassigned'\)/u);
  assert.match(titleBarSource, /<kbd[\s\S]*?\{commandPaletteShortcutLabel\}[\s\S]*?<\/kbd>/u);
  assert.doesNotMatch(titleBarSource, /Ctrl K/u);
});
