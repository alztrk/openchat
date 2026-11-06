import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const modulePath = path.resolve('src/entities/project/projectToolPermissionRules.ts');
const moduleSource = readFileSync(modulePath, 'utf8');
const compiledModule = ts.transpileModule(moduleSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  isProjectToolPermissionRuleName,
  legacyProjectToolPermissionRuleName,
  normalizeProjectToolPermissionRules,
  projectToolPermissionRuleValue,
} = await import(
  `data:text/javascript;base64,${Buffer.from(compiledModule).toString('base64')}`
);

test('project tool permission names accept legacy IDs and exact lowercase SHA-256 bindings', () => {
  assert.equal(isProjectToolPermissionRuleName('project_tool__lint'), true);
  assert.equal(isProjectToolPermissionRuleName(`project_tool__lint__${'a'.repeat(64)}`), true);
  assert.equal(isProjectToolPermissionRuleName(`project_tool__lint__${'A'.repeat(64)}`), false);
  assert.equal(isProjectToolPermissionRuleName(`project_tool__lint__${'a'.repeat(63)}`), false);
  assert.equal(isProjectToolPermissionRuleName(`project_tool__Bad__${'a'.repeat(64)}`), false);
});

test('unbound allows are discarded while legacy denials remain bound to the tool ID', () => {
  const currentName = `project_tool__lint__${'b'.repeat(64)}`;
  const normalized = normalizeProjectToolPermissionRules(
    {
      project_tool__lint: 'deny',
      project_tool__removed: 'allow',
      project_tool__stale: 'deny',
      [`project_tool__lint__${'a'.repeat(64)}`]: 'allow',
    },
  );

  assert.deepEqual(normalized, {
    project_tool__lint: 'deny',
    project_tool__stale: 'deny',
    [`project_tool__lint__${'a'.repeat(64)}`]: 'allow',
  });
  assert.equal(projectToolPermissionRuleValue(normalized, currentName), 'deny');
});

test('a current fingerprint rule takes precedence over a legacy denial', () => {
  const currentName = `project_tool__lint__${'c'.repeat(64)}`;
  const normalized = normalizeProjectToolPermissionRules(
    { project_tool__lint: 'deny', [currentName]: 'allow' },
  );

  assert.deepEqual(normalized, { project_tool__lint: 'deny', [currentName]: 'allow' });
  assert.equal(projectToolPermissionRuleValue(normalized, currentName), 'allow');
  assert.equal(legacyProjectToolPermissionRuleName(currentName), 'project_tool__lint');
});
