import assert from 'node:assert/strict';
import test from 'node:test';

import { ensureFrontendDependencies, ensureFrontendDependenciesInterruptibly } from '../../../../scripts/shared/rust.mjs';
import { runCommand } from '../../../../scripts/shared/runner.mjs';
import { CommandInterruptedError, runInterruptibleCommand } from '../../../../scripts/shared/session.mjs';

test('frontend dependency bootstrap validates the root workspace and does not reinstall a complete tree', () => {
  const calls = [];
  const workspaceRoot = 'C:\\workspace';

  ensureFrontendDependencies({
    workspaceRoot,
    runNpmFn: (args, cwd, options) => {
      calls.push({ args, cwd, options });
      return { status: 0 };
    },
    write: () => undefined,
  });

  assert.deepEqual(calls, [
    {
      args: ['ls', '--workspace', 'openchat-frontend', '--all', '--parseable', '--silent'],
      cwd: workspaceRoot,
      options: { captureOutput: true, allowFailure: true },
    },
  ]);
});

test('frontend dependency bootstrap runs npm ci from the workspace root when dependencies are incomplete', () => {
  const calls = [];
  const messages = [];
  const workspaceRoot = 'C:\\workspace';

  ensureFrontendDependencies({
    workspaceRoot,
    runNpmFn: (args, cwd, options) => {
      calls.push({ args, cwd, options });
      return { status: calls.length === 1 ? 1 : 0 };
    },
    write: (message) => messages.push(message),
  });

  assert.deepEqual(calls, [
    {
      args: ['ls', '--workspace', 'openchat-frontend', '--all', '--parseable', '--silent'],
      cwd: workspaceRoot,
      options: { captureOutput: true, allowFailure: true },
    },
    { args: ['ci'], cwd: workspaceRoot, options: undefined },
  ]);
  assert.match(messages.join(''), /dependencies are incomplete/u);
});

test('interruptible frontend dependency bootstrap installs from the workspace root when validation fails', async () => {
  const calls = [];
  const workspaceRoot = 'C:\\workspace';

  await ensureFrontendDependenciesInterruptibly(
    {},
    {
      workspaceRoot,
      npmCliPath: 'npm-cli.js',
      runInterruptibleCommandFn: async (command, args, options) => {
        calls.push({ command, args, options });
        return { stdout: '', status: calls.length === 1 ? 1 : 0 };
      },
      write: () => undefined,
    },
  );

  assert.deepEqual(calls, [
    {
      command: process.execPath,
      args: ['npm-cli.js', 'ls', '--workspace', 'openchat-frontend', '--all', '--parseable', '--silent'],
      options: { cwd: workspaceRoot, captureOutput: true, interruptController: {}, allowFailure: true },
    },
    {
      command: process.execPath,
      args: ['npm-cli.js', 'ci'],
      options: { cwd: workspaceRoot, interruptController: {} },
    },
  ]);
});

test('interruptible frontend dependency bootstrap skips install for a complete workspace', async () => {
  const calls = [];

  await ensureFrontendDependenciesInterruptibly(
    {},
    {
      npmCliPath: 'npm-cli.js',
      runInterruptibleCommandFn: async (...args) => {
        calls.push(args);
        return { stdout: '', status: 0 };
      },
      write: () => undefined,
    },
  );

  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0][1], [
    'npm-cli.js',
    'ls',
    '--workspace',
    'openchat-frontend',
    '--all',
    '--parseable',
    '--silent',
  ]);
});

test('interrupting dependency validation never starts an install', async () => {
  const calls = [];
  const interruption = new CommandInterruptedError('SIGINT');

  await assert.rejects(
    ensureFrontendDependenciesInterruptibly(
      {},
      {
        npmCliPath: 'npm-cli.js',
        runInterruptibleCommandFn: async (...args) => {
          calls.push(args);
          throw interruption;
        },
        write: () => undefined,
      },
    ),
    (error) => error === interruption,
  );
  assert.equal(calls.length, 1);
});

test('runCommand returns a nonzero status only when failure is explicitly allowed', () => {
  const args = ['-e', 'process.exit(7)'];

  const result = runCommand(process.execPath, args, { captureOutput: true, allowFailure: true });
  assert.equal(result.status, 7);
  assert.throws(() => runCommand(process.execPath, args, { captureOutput: true }), /exit code 7/u);
});

test('runInterruptibleCommand returns a nonzero status only when failure is explicitly allowed', async () => {
  const result = await runInterruptibleCommand(process.execPath, ['-e', 'process.exit(9)'], {
    captureOutput: true,
    allowFailure: true,
  });

  assert.equal(result.status, 9);
});
