import assert from 'node:assert/strict';
import test from 'node:test';

import { runCheck, runCheckTypes, runValidationTasks } from '../../../../scripts/tasks/check.mjs';

test('validation task runner reports synchronous command failures and still runs remaining tasks', async () => {
  const started = [];
  const messages = [];

  await assert.rejects(
    runValidationTasks(
      'check',
      [
        {
          name: 'first command',
          run: () => {
            started.push('first');
            throw new Error('spawn failed');
          },
        },
        { name: 'second command', run: () => started.push('second') },
        {
          name: 'third command',
          run: () => {
            started.push('third');
            throw 'non-error failure';
          },
        },
      ],
      (message) => messages.push(message),
    ),
    /check: 2 task\(s\) failed: first command, third command/u,
  );

  assert.deepEqual(started, ['first', 'second', 'third']);
  assert.match(messages.join(''), /first command failed: spawn failed/u);
  assert.match(messages.join(''), /second command passed/u);
  assert.match(messages.join(''), /third command failed: non-error failure/u);
});

test('check runs the complete frontend and locked Rust validation set', async () => {
  const calls = [];

  await runCheck({
    frontendDirectory: 'frontend',
    workspaceRoot: 'workspace',
    runNpmFn: (args, cwd) => calls.push({ command: 'npm', args, cwd }),
    runCommandFn: (command, args, options) =>
      calls.push({ command, args, cwd: options.cwd, testThreads: options.env?.RUST_TEST_THREADS }),
    write: () => undefined,
  });

  assert.deepEqual(calls, [
    {
      command: 'npm',
      args: ['exec', '--', 'biome', 'check', '.', '../../../scripts/tasks/check.mjs', '../../../scripts/check-imports.mjs'],
      cwd: 'frontend',
    },
    { command: 'npm', args: ['test'], cwd: 'frontend' },
    { command: 'npm', args: ['run', 'check:locales'], cwd: 'frontend' },
    { command: 'npm', args: ['run', 'check:imports'], cwd: 'frontend' },
    { command: 'npm', args: ['run', 'build'], cwd: 'frontend' },
    { command: 'cargo', args: ['fmt', '--all', '--', '--check'], cwd: 'workspace', testThreads: undefined },
    { command: 'cargo', args: ['check', '--workspace', '--locked'], cwd: 'workspace', testThreads: undefined },
    {
      command: 'cargo',
      args: [
        'test',
        '--workspace',
        '--features',
        'openchat-local-service/desktop-profile-test-support',
        '--locked',
        '--',
        '--test-threads=1',
      ],
      cwd: 'workspace',
      testThreads: '1',
    },
  ]);
});

test('check-types keeps its no-tests type and compilation scope', async () => {
  const calls = [];

  await runCheckTypes({
    frontendDirectory: 'frontend',
    workspaceRoot: 'workspace',
    runNpmFn: (args, cwd) => calls.push({ command: 'npm', args, cwd }),
    runCommandFn: (command, args, options) => calls.push({ command, args, cwd: options.cwd }),
    write: () => undefined,
  });

  assert.deepEqual(calls, [
    {
      command: 'npm',
      args: ['exec', '--', 'biome', 'check', '.', '../../../scripts/tasks/check.mjs', '../../../scripts/check-imports.mjs'],
      cwd: 'frontend',
    },
    { command: 'npm', args: ['exec', '--', 'tsc', '--noEmit'], cwd: 'frontend' },
    { command: 'cargo', args: ['fmt', '--all', '--', '--check'], cwd: 'workspace' },
    { command: 'cargo', args: ['check', '--workspace', '--locked'], cwd: 'workspace' },
  ]);
});

test('check continues all validation tasks after several command runners throw synchronously', async () => {
  const calls = [];

  await assert.rejects(
    runCheck({
      frontendDirectory: 'frontend',
      workspaceRoot: 'workspace',
      runNpmFn: (args) => {
        calls.push(args);
        if (args[0] === 'exec') throw new Error('Biome unavailable');
      },
      runCommandFn: (command, args) => {
        calls.push([command, ...args]);
        if (args[0] === 'fmt') throw new Error('rustfmt unavailable');
      },
      write: () => undefined,
    }),
    /check: 2 task\(s\) failed: Biome lint, Rust formatting/u,
  );

  assert.equal(calls.length, 8);
  assert.deepEqual(calls.slice(0, 5), [
    ['exec', '--', 'biome', 'check', '.', '../../../scripts/tasks/check.mjs', '../../../scripts/check-imports.mjs'],
    ['test'],
    ['run', 'check:locales'],
    ['run', 'check:imports'],
    ['run', 'build'],
  ]);
});
