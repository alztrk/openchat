import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const appSource = readFileSync(path.resolve('src/bootstrap/App.tsx'), 'utf8');

test('surface transitions focus the newly labeled main without taking focus on initial render', () => {
  assert.match(appSource, /const previousActiveSurfaceRef = useRef\(activeSurface\);/u);
  assert.match(appSource, /if \(previousActiveSurfaceRef\.current === activeSurface\) return;/u);
  assert.match(appSource, /previousActiveSurfaceRef\.current = activeSurface;/u);
  assert.match(appSource, /surfaceHeadingRef\.current\?\.focus\(\{ preventScroll: true \}\);/u);
  assert.match(appSource, /\}, \[activeSurface\]\);/u);
  assert.match(appSource, /<main[\s\S]*?aria-label=\{heading\}[\s\S]*?key=\{activeSurface\}[\s\S]*?ref=\{surfaceHeadingRef\}[\s\S]*?tabIndex=\{-1\}/u);
});

test('selecting the already active surface keeps the existing direct focus behavior', () => {
  const navigateStart = appSource.indexOf('const navigate = useCallback(');
  const navigateEnd = appSource.indexOf('\n  const startWorkspaceChat', navigateStart);
  assert.notEqual(navigateStart, -1);
  assert.notEqual(navigateEnd, -1);

  const navigateSource = appSource.slice(navigateStart, navigateEnd);
  assert.match(navigateSource, /if \(surface === activeSurface\) \{\s*surfaceHeadingRef\.current\?\.focus\(\);\s*return;\s*\}/u);
});
