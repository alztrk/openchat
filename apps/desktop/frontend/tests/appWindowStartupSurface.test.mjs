import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const settingsSource = readSource('src/features/app-settings/components/AppWindowSettingsSection.tsx');
const preferencesSource = readSource('src/features/app-settings/data/appPreferences.ts');

test('startup settings accept every application surface through the shared domain guard', () => {
  assert.match(settingsSource, /if \(value !== 'last' && !isAppSurface\(value\)\) return;/u);
  assert.match(preferencesSource, /export function isAppSurface\(value: string \| null\): value is AppSurface/u);
  assert.match(
    preferencesSource,
    /value === 'chats'.*value === 'design'.*value === 'models'.*value === 'security'.*value === 'settings'/u,
  );
  assert.match(settingsSource, /\{ value: 'security', label: t\('securityScansTitle'\) \}/u);
});

test('startup surface selection persists before updating its visible value', () => {
  assert.match(
    settingsSource,
    /await writeStartupSurfacePreference\(value\);\s*setStartupSurface\(value\);\s*setAppSaveState\('saved'\);/u,
  );
  assert.match(settingsSource, /catch \{\s*setAppSaveState\('error'\);\s*\}/u);
});
