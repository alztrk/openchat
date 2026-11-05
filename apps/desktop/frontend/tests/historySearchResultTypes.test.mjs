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
    'const unifiedStorage = { getItem: () => null, setItem: () => undefined };',
  )
  .replace(
    "import { listProjects } from './chatStorage';",
    'const listProjects = async () => globalThis.__historySearchTestState.projects;',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { searchHistory } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

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
