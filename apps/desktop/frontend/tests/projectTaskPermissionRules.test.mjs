import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const modulePath = path.resolve('src/entities/project/projectTaskPermissionRules.ts');
const moduleSource = readFileSync(modulePath, 'utf8');
const compiledModule = ts.transpileModule(moduleSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  isBoundProjectTaskPermissionRuleName,
  isProjectTaskPermissionRuleName,
  legacyProjectTaskPermissionRuleName,
  normalizeProjectTaskPermissionRules,
  projectTaskPermissionId,
  projectTaskPermissionRuleValue,
} = await import(`data:text/javascript;base64,${Buffer.from(compiledModule).toString('base64')}`);

test('project task rules accept legacy IDs and exact lowercase SHA-256 bindings', () => {
  assert.equal(isProjectTaskPermissionRuleName('run_project_task__verify'), true);
  assert.equal(isProjectTaskPermissionRuleName(`run_project_task__verify__${'a'.repeat(64)}`), true);
  assert.equal(isBoundProjectTaskPermissionRuleName('run_project_task__verify'), false);
  assert.equal(isBoundProjectTaskPermissionRuleName(`run_project_task__verify__${'a'.repeat(64)}`), true);
  assert.equal(isProjectTaskPermissionRuleName(`run_project_task__verify__${'A'.repeat(64)}`), false);
  assert.equal(isProjectTaskPermissionRuleName(`run_project_task__verify__${'a'.repeat(63)}`), false);
  assert.equal(isProjectTaskPermissionRuleName('run_project_task__Verify'), false);
});

test('legacy task allows are discarded while legacy ask and deny remain applicable', () => {
  const currentName = `run_project_task__verify__${'b'.repeat(64)}`;
  const normalized = normalizeProjectTaskPermissionRules({
    run_project_task__verify: 'allow',
    run_project_task__removed: 'deny',
    [`run_project_task__verify__${'a'.repeat(64)}`]: 'allow',
  });

  assert.deepEqual(normalized, {
    run_project_task__removed: 'deny',
    [`run_project_task__verify__${'a'.repeat(64)}`]: 'allow',
  });
  assert.equal(projectTaskPermissionRuleValue(normalized, currentName), undefined);
  assert.equal(legacyProjectTaskPermissionRuleName(currentName), 'run_project_task__verify');
  assert.equal(projectTaskPermissionId(currentName), 'verify');

  assert.equal(
    projectTaskPermissionRuleValue({ run_project_task__verify: 'deny' }, currentName),
    'deny',
  );
  assert.equal(
    projectTaskPermissionRuleValue({ run_project_task__verify: 'ask' }, currentName),
    'ask',
  );
});

test('a current task rule takes precedence over a legacy denial', () => {
  const currentName = `run_project_task__verify__${'c'.repeat(64)}`;
  const rules = normalizeProjectTaskPermissionRules({
    run_project_task__verify: 'deny',
    [currentName]: 'allow',
  });

  assert.equal(projectTaskPermissionRuleValue(rules, currentName), 'allow');
});
