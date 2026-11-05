import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const appSource = readSource('src/bootstrap/App.tsx');
const settingsSource = readSource('src/features/app-settings/components/SettingsPage.tsx');
const designPageSource = readSource('src/features/design-workspace/components/DesignPage.tsx');
const designCanvasSource = readSource('src/features/design-workspace/components/DesignCanvas.tsx');

test('the active surface keeps one main landmark and names its inner content sections', () => {
  assert.equal([...appSource.matchAll(/<main\b/gu)].length, 1);
  assert.equal([...appSource.matchAll(/<\/main>/gu)].length, 1);

  assert.doesNotMatch(settingsSource, /<main\b|<\/main>/u);
  assert.match(settingsSource, /<section\s+aria-labelledby="settings-page-heading"/u);

  assert.doesNotMatch(designPageSource, /<main\b|<\/main>/u);
  assert.match(designPageSource, /<section\s+aria-labelledby="design-library-heading"/u);
  assert.match(designPageSource, /id="design-library-heading"/u);

  assert.doesNotMatch(designCanvasSource, /<main\b|<\/main>/u);
  assert.match(designCanvasSource, /<section aria-labelledby="design-canvas-title"/u);
  assert.match(designCanvasSource, /id="design-canvas-title"/u);
});
