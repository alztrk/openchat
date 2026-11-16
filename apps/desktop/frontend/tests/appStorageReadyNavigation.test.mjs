import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { after, test } from 'node:test';
import { createServer } from 'vite';

const appSource = readFileSync(path.resolve('src/bootstrap/App.tsx'), 'utf8');
const vite = await createServer({
  configFile: path.resolve('vite.config.ts'),
  appType: 'custom',
  server: { middlewareMode: true },
});
const { createSurfaceHistoryState, surfaceHistoryReducer } = await vite.ssrLoadModule('/src/app/surfaceHistory.ts');
after(() => vite.close());

test('storage readiness applies the configured startup surface only before user navigation', () => {
  assert.match(appSource, /useReducer\(\s*surfaceHistoryReducer,\s*readInitialSurface\(\),\s*createSurfaceHistoryState/u);

  const initializeStart = appSource.indexOf('const initializePreferences = () => {');
  const initializeEnd = appSource.indexOf('\n    };', initializeStart);
  assert.notEqual(initializeStart, -1);
  assert.notEqual(initializeEnd, -1);

  const initializeSource = appSource.slice(initializeStart, initializeEnd);
  assert.match(initializeSource, /dispatchSurfaceHistory\(\{ type: 'storage-ready', surface: readInitialSurface\(\) \}\)/u);
});

test('surface navigation and history navigation mark early user intent before updating history', () => {
  const navigateStart = appSource.indexOf('const navigateSurface = useCallback(');
  const navigateEnd = appSource.indexOf('\n  const requestTemporaryChatExit', navigateStart);
  assert.notEqual(navigateStart, -1);
  assert.notEqual(navigateEnd, -1);
  assert.match(
    appSource.slice(navigateStart, navigateEnd),
    /dispatchSurfaceHistory\(\{ type: 'navigate', surface \}\)/u,
  );

  const exitStart = appSource.indexOf('const requestTemporaryChatExit = useCallback(');
  const exitEnd = appSource.indexOf('\n  const openStatisticsConversation', exitStart);
  assert.notEqual(exitStart, -1);
  assert.notEqual(exitEnd, -1);
  assert.match(
    appSource.slice(exitStart, exitEnd),
    /dispatchSurfaceHistory\(\{ type: 'navigation-intent' \}\);[\s\S]*?const current = useChatStore\.getState\(\);/u,
  );

  const historyStart = appSource.indexOf('const moveInHistory = (offset: number) => {');
  const historyEnd = appSource.indexOf('\n  const closeTemporaryChatExitDialog', historyStart);
  assert.notEqual(historyStart, -1);
  assert.notEqual(historyEnd, -1);
  assert.match(appSource.slice(historyStart, historyEnd), /type: 'move-in-history', offset/u);
});

test('storage readiness applies the saved startup surface when no navigation has started', () => {
  const initial = createSurfaceHistoryState('chats');
  const ready = surfaceHistoryReducer(initial, { type: 'storage-ready', surface: 'security' });

  assert.deepEqual(ready.surfaces, ['security']);
  assert.equal(ready.index, 0);
  assert.equal(ready.didNavigateBeforeStorageReady, false);
});

test('storage readiness preserves pending user navigation and back-forward history', () => {
  let state = createSurfaceHistoryState('chats');
  state = surfaceHistoryReducer(state, { type: 'navigation-intent' });
  state = surfaceHistoryReducer(state, { type: 'storage-ready', surface: 'security' });

  assert.deepEqual(state.surfaces, ['chats']);
  assert.equal(state.index, 0);

  state = surfaceHistoryReducer(state, { type: 'navigate', surface: 'models' });
  state = surfaceHistoryReducer(state, { type: 'navigate', surface: 'workflows' });
  state = surfaceHistoryReducer(state, { type: 'move-in-history', offset: -1 });
  state = surfaceHistoryReducer(state, { type: 'storage-ready', surface: 'security' });

  assert.deepEqual(state.surfaces, ['chats', 'models', 'workflows']);
  assert.equal(state.index, 1);
  assert.equal(state.didNavigateBeforeStorageReady, true);

  state = surfaceHistoryReducer(state, { type: 'move-in-history', offset: 1 });
  assert.equal(state.surfaces[state.index], 'workflows');
});
