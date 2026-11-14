import assert from 'node:assert/strict';
import test from 'node:test';

import { prepareTauriSidecar, runCheck, runCheckTypes, runValidationTasks } from '../../../../scripts/tasks/check.mjs';
import { rustBuildEnvironment } from '../../../../scripts/shared/rust.mjs';

test('Windows Rust builds select the standard CMake install when no executable is configured', () => {
  const environment = { ProgramFiles: 'C:\\Program Files', PATH: 'existing-path' };
  const selectedEnvironment = rustBuildEnvironment({
    environment,
    platform: 'win32',
    fileExists: (filePath) => filePath === 'C:\\Program Files\\CMake\\bin\\cmake.exe',
  });

  assert.equal(selectedEnvironment.CMAKE, 'C:\\Program Files\\CMake\\bin\\cmake.exe');
  assert.equal(selectedEnvironment.PATH, environment.PATH);
  assert.equal(environment.CMAKE, undefined);
});

test('Rust builds preserve a configured CMake executable and other platforms unchanged', () => {
  const configuredEnvironment = { CMAKE: 'custom-cmake', PATH: 'existing-path' };
  assert.equal(
    rustBuildEnvironment({ environment: configuredEnvironment, platform: 'win32', fileExists: () => true }),
    configuredEnvironment,
  );

  const linuxEnvironment = { PATH: 'existing-path' };
  assert.equal(
    rustBuildEnvironment({ environment: linuxEnvironment, platform: 'linux', fileExists: () => true }),
    linuxEnvironment,
  );
});

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
  const sidecarBuilds = [];

  prepareTauriSidecar((profile, locked) => sidecarBuilds.push({ profile, locked }));
  assert.deepEqual(sidecarBuilds, [{ profile: 'debug', locked: true }]);

  await runCheck({
    frontendDirectory: 'frontend',
    workspaceRoot: 'workspace',
    prepareTauriSidecarFn: () => calls.push({ command: 'sidecar' }),
    runNpmFn: (args, cwd) => calls.push({ command: 'npm', args, cwd }),
    runCommandFn: (command, args, options) =>
      calls.push({
        command,
        args,
        cwd: options.cwd,
        testThreads: options.env?.RUST_TEST_THREADS,
        ...(options.env?.CMAKE ? { cmake: options.env.CMAKE } : {}),
      }),
    write: () => undefined,
  });

  assert.deepEqual(calls, [
    { command: 'sidecar' },
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
    {
      command: 'cargo',
      args: ['check', '--workspace', '--locked'],
      cwd: 'workspace',
      testThreads: undefined,
      ...(rustBuildEnvironment().CMAKE ? { cmake: rustBuildEnvironment().CMAKE } : {}),
    },
    {
      command: 'cargo',
      args: [
        'test',
        '--workspace',
        '--features',
        'openchat-local-service/desktop-profile-test-support',
        '--locked',
        '--',
        '--test-threads=4',
      ],
      cwd: 'workspace',
      testThreads: '4',
      ...(rustBuildEnvironment().CMAKE ? { cmake: rustBuildEnvironment().CMAKE } : {}),
    },
  ]);
});

test('check-types keeps its no-tests type and compilation scope', async () => {
  const calls = [];

  await runCheckTypes({
    frontendDirectory: 'frontend',
    workspaceRoot: 'workspace',
    prepareTauriSidecarFn: () => calls.push({ command: 'sidecar' }),
    runNpmFn: (args, cwd) => calls.push({ command: 'npm', args, cwd }),
    runCommandFn: (command, args, options) => calls.push({ command, args, cwd: options.cwd }),
    write: () => undefined,
  });

  assert.deepEqual(calls, [
    { command: 'sidecar' },
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
      prepareTauriSidecarFn: () => undefined,
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
