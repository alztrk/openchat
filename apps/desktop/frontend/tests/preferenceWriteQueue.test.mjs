import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

function createTestStore(initializer) {
  let state;
  const set = (patch) => {
    state = { ...state, ...patch };
  };
  const get = () => state;
  const store = (selector = (current) => current) => selector(state);
  store.getState = () => state;
  state = initializer(set, get);
  return store;
}

const storageState = {
  values: new Map(),
  pendingWrite: Promise.resolve(),
  nextWriteGate: null,
};

const storage = {
  isInitialized: true,
  getItem: (key) => storageState.values.get(key) ?? null,
  setPersistedItem(key, value) {
    const write = storageState.pendingWrite.then(async () => {
      const gate = storageState.nextWriteGate;
      storageState.nextWriteGate = null;
      if (gate) {
        gate.started();
        await gate.wait;
      }
      storageState.values.set(key, value);
    });
    storageState.pendingWrite = write.then(
      () => undefined,
      () => undefined,
    );
    return write;
  },
  waitForWrites: () => storageState.pendingWrite,
  pauseNextWrite() {
    let signalStarted;
    let releaseWrite;
    const started = new Promise((resolve) => {
      signalStarted = resolve;
    });
    const wait = new Promise((resolve) => {
      releaseWrite = resolve;
    });
    storageState.nextWriteGate = {
      started: signalStarted,
      wait,
    };
    return { started, release: releaseWrite };
  },
};

globalThis.__openChatPreferenceTestCreate = createTestStore;
globalThis.__openChatPreferenceTestStorage = storage;

const serialQueuePath = path.resolve('src/shared/lib/async/serialTaskQueue.ts');
const serialQueueSource = fs.readFileSync(serialQueuePath, 'utf8');
const serialQueueCompiled = ts.transpileModule(serialQueueSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { createSerialTaskQueue } = await import(
  `data:text/javascript;base64,${Buffer.from(serialQueueCompiled).toString('base64')}`
);

globalThis.__openChatPreferenceTestQueue = { createSerialTaskQueue };

async function loadPreferenceStore(sourcePath) {
  const source = fs
    .readFileSync(path.resolve(sourcePath), 'utf8')
    .replace("import { create } from 'zustand';", 'const create = globalThis.__openChatPreferenceTestCreate;')
    .replace(
      "import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';",
      'const unifiedStorage = globalThis.__openChatPreferenceTestStorage;',
    )
    .replace(
      "import { createSerialTaskQueue } from '@shared/lib/async/serialTaskQueue';",
      'const { createSerialTaskQueue } = globalThis.__openChatPreferenceTestQueue;',
    );
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
}

test('a failed serial task does not block the next task', async () => {
  const enqueue = createSerialTaskQueue();
  const failure = new Error('write failed');
  const first = enqueue(() => Promise.reject(failure));
  const second = enqueue(() => Promise.resolve('second task completed'));

  await assert.rejects(first, (error) => error === failure);
  assert.equal(await second, 'second task completed');
});

test('concurrent patches merge in each whole-object preference store', async () => {
  const cases = [
    {
      name: 'design preferences',
      path: 'src/features/design-workspace/data/designPreferences.ts',
      exportName: 'useDesignPreferencesStore',
      storageKey: 'design.generation_preferences',
      first: (store) => store.updatePreferences({ gallerySort: 'oldest' }),
      second: (store) => store.updatePreferences({ favoritesOnly: true }),
      expected: { gallerySort: 'oldest', favoritesOnly: true },
    },
    {
      name: 'chat display preferences',
      path: 'src/shared/lib/preferences/chatDisplayPreferences.ts',
      exportName: 'useChatDisplayPreferencesStore',
      storageKey: 'chat.display_preferences',
      first: (store) => store.setPreference('autoFollowResponses', false),
      second: (store) => store.setPreference('wrapCode', false),
      expected: { autoFollowResponses: false, wrapCode: false },
    },
    {
      name: 'assistant execution preferences',
      path: 'src/shared/lib/preferences/assistantExecutionPreferences.ts',
      exportName: 'useAssistantExecutionPreferencesStore',
      storageKey: 'assistant.execution_preferences',
      first: (store) => store.setPreference('includeProjectSkills', false),
      second: (store) => store.setPreference('maxGoalRuntimeMinutes', 30),
      expected: { includeProjectSkills: false, maxGoalRuntimeMinutes: 30 },
    },
  ];

  for (const item of cases) {
    await storage.waitForWrites();
    storageState.values.clear();
    const gate = storage.pauseNextWrite();
    const module = await loadPreferenceStore(item.path);
    const store = module[item.exportName].getState();
    store.initialize();

    const firstWrite = item.first(store);
    await gate.started;
    const secondWrite = item.second(store);
    gate.release();

    assert.deepEqual(await Promise.all([firstWrite, secondWrite]), [true, true], item.name);
    const saved = JSON.parse(storageState.values.get(item.storageKey));
    const current = module[item.exportName].getState().preferences;
    for (const [key, value] of Object.entries(item.expected)) {
      assert.equal(saved[key], value, `${item.name} persisted ${key}`);
      assert.equal(current[key], value, `${item.name} retained ${key} in memory`);
    }
  }
});
