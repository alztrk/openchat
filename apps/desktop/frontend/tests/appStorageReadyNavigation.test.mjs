import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const appSource = readFileSync(path.resolve('src/bootstrap/App.tsx'), 'utf8');

test('storage readiness applies the configured startup surface only before user navigation', () => {
  assert.match(appSource, /const didNavigateBeforeStorageReadyRef = useRef\(false\);/u);

  const initializeStart = appSource.indexOf('const initializePreferences = () => {');
  const initializeEnd = appSource.indexOf('\n    };', initializeStart);
  assert.notEqual(initializeStart, -1);
  assert.notEqual(initializeEnd, -1);

  const initializeSource = appSource.slice(initializeStart, initializeEnd);
  assert.match(
    initializeSource,
    /if \(!didNavigateBeforeStorageReadyRef\.current\) \{\s*setSurfaceHistory\(\{ surfaces: \[readInitialSurface\(\)\], index: 0 \}\);\s*\}/u,
  );
});

test('surface navigation and history navigation mark early user intent before updating history', () => {
  const navigateStart = appSource.indexOf('const navigateSurface = useCallback(');
  const navigateEnd = appSource.indexOf('\n  const requestTemporaryChatExit', navigateStart);
  assert.notEqual(navigateStart, -1);
  assert.notEqual(navigateEnd, -1);
  assert.match(
    appSource.slice(navigateStart, navigateEnd),
    /didNavigateBeforeStorageReadyRef\.current = true;[\s\S]*?setSurfaceHistory\(/u,
  );

  const historyStart = appSource.indexOf('const moveInHistory = (offset: number) => {');
  const historyEnd = appSource.indexOf('\n  const closeTemporaryChatExitDialog', historyStart);
  assert.notEqual(historyStart, -1);
  assert.notEqual(historyEnd, -1);
  assert.match(
    appSource.slice(historyStart, historyEnd),
    /didNavigateBeforeStorageReadyRef\.current = true;\s*setSurfaceHistory\(/u,
  );
});
