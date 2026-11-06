import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/chat/data/historySearch.ts');
const source = fs
  .readFileSync(sourcePath, 'utf8')
  .replace("import { i18n } from '@shared/lib/i18n/i18n';", "const i18n = { resolvedLanguage: 'en', language: 'en' };")
  .replace(
    "import { type JsonObject, LocalServiceError, localServiceClient } from '@shared/lib/platform/local-service';",
    `class LocalServiceError extends Error {
      constructor(_code, message) { super(message); }
    }
    const localServiceClient = {
      call: (...args) => globalThis.__historySearchTestState.call(...args),
    };`,
  )
  .replace(
    "import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';",
    `const unifiedStorage = {
      getItem: () => globalThis.__historySearchTestState.savedSearches ?? null,
      setPersistedItem: (...args) => globalThis.__historySearchTestState.persist(...args),
    };`,
  )
  .replace(
    "import { listAllProjects } from './chatStorage';",
    'const listAllProjects = async () => globalThis.__historySearchTestState.projects;',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  deleteHistorySearch,
  historySearchDateRange,
  loadSavedHistorySearches,
  SavedHistorySearchError,
  saveHistorySearch,
  searchHistory,
} = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

const project = {
  id: 'project-1',
  name: 'OpenChat tools',
  folderPath: 'D:/projects/OpenChat',
  updatedAt: 1_800_000_000_000,
};

function createFilters(resultType) {
  return {
    providerId: null,
    modelId: null,
    projectId: null,
    isArchived: null,
    resultType,
    sortOrder: 'relevance',
  };
}

function createSavedSearch(name = 'OpenChat', query = 'openchat') {
  return {
    name,
    query,
    filters: {
      providerId: null,
      modelId: null,
      projectId: null,
      isArchived: null,
      resultType: 'all',
      sortOrder: 'relevance',
    },
    fromUnixMs: null,
    throughUnixMs: null,
  };
}

test('conversation-only history search does not include local project matches', async () => {
  const calls = [];
  globalThis.__historySearchTestState = {
    projects: [project],
    call: async (...args) => {
      calls.push(args);
      return { results: [] };
    },
  };

  const results = await searchHistory('openchat', createFilters('conversations'), null, null);

  assert.deepEqual(results, []);
  assert.equal(calls.length, 1);
  assert.equal(calls[0][0], 'chat.history.search');
  assert.equal(calls[0][1].resultType, 'conversations');
  assert.deepEqual(Object.keys(calls[0][1]).sort(), ['query', 'resultType', 'sortOrder']);
});

test('history search forwards configured filters and date bounds', async () => {
  let request;
  globalThis.__historySearchTestState = {
    projects: [],
    call: async (_method, params) => {
      request = params;
      return { results: [] };
    },
  };

  await searchHistory(
    'openchat',
    {
      providerId: 'provider-1',
      modelId: 'model-1',
      projectId: 'project-1',
      isArchived: false,
      resultType: 'messages',
      sortOrder: 'oldest',
    },
    1_800_000_000_000,
    1_800_000_086_400_000,
  );

  assert.deepEqual(request, {
    query: 'openchat',
    fromUnixMs: 1_800_000_000_000,
    throughUnixMs: 1_800_000_086_400_000,
    providerId: 'provider-1',
    modelId: 'model-1',
    projectId: 'project-1',
    isArchived: false,
    resultType: 'messages',
    sortOrder: 'oldest',
  });
});

test('project-only history search still returns matching local projects', async () => {
  globalThis.__historySearchTestState = {
    projects: [project],
    call: async () => {
      throw new Error('Project-only search must not call the native history service.');
    },
  };

  const results = await searchHistory('openchat', createFilters('projects'), null, null);

  assert.deepEqual(results.map((result) => result.kind), ['project']);
  assert.deepEqual(results.map((result) => result.projectId), ['project-1']);
});

test('history search date range includes the selected local end date', () => {
  const range = historySearchDateRange('2026-03-08', '2026-03-08');

  assert.notEqual(range, null);
  if (!range) return;
  const start = new Date(range.fromUnixMs);
  const expectedEnd = new Date(start);
  expectedEnd.setDate(expectedEnd.getDate() + 1);

  assert.equal(start.getFullYear(), 2026);
  assert.equal(start.getMonth(), 2);
  assert.equal(start.getDate(), 8);
  assert.equal(start.getHours(), 0);
  assert.equal(range.throughUnixMs, expectedEnd.getTime());
});

test('history search date ranges clamp pre-epoch local dates to the service minimum', () => {
  const range = historySearchDateRange('1969-12-30', '1970-01-02');

  assert.notEqual(range, null);
  if (!range) return;
  assert.equal(range.fromUnixMs, 0);
  assert.ok(range.throughUnixMs > 0);
});

test('history search rejects reversed and impossible local date ranges', () => {
  assert.equal(historySearchDateRange('2026-03-09', '2026-03-08'), null);
  assert.equal(historySearchDateRange('2026-02-30', '2026-03-01'), null);
});

test('saved history search waits for durable storage before reporting success', async () => {
  let resolvePersistence;
  let savedSearches = null;
  let writeStarted = false;
  const persistence = new Promise((resolve) => {
    resolvePersistence = resolve;
  });
  globalThis.__historySearchTestState = {
    savedSearches,
    persist: async (_key, value) => {
      writeStarted = true;
      await persistence;
      savedSearches = value;
      globalThis.__historySearchTestState.savedSearches = value;
    },
  };

  const pendingSave = saveHistorySearch(createSavedSearch());
  let saveCompleted = false;
  const saveCompletion = pendingSave.then((searches) => {
    saveCompleted = true;
    return searches;
  });
  await Promise.resolve();

  assert.equal(writeStarted, true);
  assert.equal(saveCompleted, false);
  assert.equal(savedSearches, null);

  resolvePersistence();
  assert.deepEqual(await saveCompletion, [createSavedSearch()]);
  assert.deepEqual(JSON.parse(savedSearches), [createSavedSearch()]);
});

test('failed saved history search writes retain the previous value and report typed save or delete errors', async () => {
  const existing = createSavedSearch('Existing', 'existing query');
  const existingValue = JSON.stringify([existing]);
  globalThis.__historySearchTestState = {
    savedSearches: existingValue,
    persist: async () => {
      throw new Error('storage unavailable');
    },
  };

  await assert.rejects(saveHistorySearch(createSavedSearch()), (error) => {
    assert.ok(error instanceof SavedHistorySearchError);
    assert.equal(error.reason, 'saveFailed');
    return true;
  });
  assert.deepEqual(loadSavedHistorySearches(), [existing]);

  await assert.rejects(deleteHistorySearch('Existing'), (error) => {
    assert.ok(error instanceof SavedHistorySearchError);
    assert.equal(error.reason, 'deleteFailed');
    return true;
  });
  assert.deepEqual(loadSavedHistorySearches(), [existing]);
});
