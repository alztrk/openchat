import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/providers/data/connectionPlanState.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { mergeConnectionPlanUpdates } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('a late plan refresh cannot restore a connection removed after its snapshot was taken', () => {
  const current = [
    {
      id: 'remaining',
      authStatus: 'active',
      displayName: 'Current account',
      email: 'current@example.test',
      planType: 'old-plan',
      isSelected: true,
      workspaces: [{ id: 'workspace-a', displayName: 'Workspace A', planType: 'old-plan', isSelected: true }],
    },
  ];
  const refreshed = [
    {
      ...current[0],
      planType: 'updated-plan',
      workspaces: [{ ...current[0].workspaces[0], planType: 'updated-plan' }],
    },
    {
      id: 'removed',
      authStatus: 'active',
      displayName: 'Removed account',
      email: 'removed@example.test',
      planType: 'stale-plan',
      isSelected: false,
      workspaces: [],
    },
  ];

  const merged = mergeConnectionPlanUpdates(current, refreshed);

  assert.deepEqual(merged.map((connection) => connection.id), ['remaining']);
  assert.equal(merged[0].planType, 'updated-plan');
  assert.equal(merged[0].workspaces[0].displayName, 'Workspace A');
  assert.equal(merged[0].workspaces[0].planType, 'updated-plan');
});

test('a plan refresh cannot remove current connections or workspaces absent from its snapshot', () => {
  const current = [
    {
      id: 'account',
      authStatus: 'active',
      displayName: 'Account',
      email: 'account@example.test',
      planType: 'current-plan',
      isSelected: true,
      workspaces: [
        { id: 'workspace-a', displayName: 'Workspace A', planType: 'current-plan', isSelected: true },
        { id: 'workspace-b', displayName: 'Workspace B', planType: null, isSelected: false },
      ],
    },
  ];

  assert.deepEqual(mergeConnectionPlanUpdates(current, []), current);
  const merged = mergeConnectionPlanUpdates(current, [{ ...current[0], workspaces: [current[0].workspaces[0]] }]);
  assert.equal(merged[0].workspaces.length, 2);
  assert.equal(merged[0].workspaces[1].displayName, 'Workspace B');
});
