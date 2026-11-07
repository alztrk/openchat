import { runNpm, runCommand, getFrontendDirectory, getWorkspaceRoot } from '../shared/runner.mjs';
import { buildLocalService } from '../shared/rust.mjs';
import { ensureNoArguments } from '../help.mjs';

const profileTestFeature = 'openchat-local-service/desktop-profile-test-support';

export const check = {
  name: 'check',
  description: 'Runs frontend lint, tests, locale and import checks, asset build, and locked Rust checks.',
  args: [],
  async run(args) {
    ensureNoArguments('check', args);
    await runCheck();
  },
};

export const checkTypes = {
  name: 'check-types',
  description: 'Runs frontend lint and type checks plus locked Rust formatting and compilation checks (no tests).',
  args: [],
  async run(args) {
    ensureNoArguments('check-types', args);
    await runCheckTypes();
  },
};

export async function runCheck({
  runNpmFn = runNpm,
  runCommandFn = runCommand,
  frontendDirectory = getFrontendDirectory(),
  workspaceRoot = getWorkspaceRoot(),
  prepareTauriSidecarFn = prepareTauriSidecar,
  write = (message) => process.stdout.write(message),
} = {}) {
  prepareTauriSidecarFn();
  await runValidationTasks('check', [
    {
      name: 'Biome lint',
      run: () =>
        runNpmFn(
          ['exec', '--', 'biome', 'check', '.', '../../../scripts/tasks/check.mjs', '../../../scripts/check-imports.mjs'],
          frontendDirectory,
        ),
    },
    { name: 'Frontend unit tests', run: () => runNpmFn(['test'], frontendDirectory) },
    { name: 'Locale consistency', run: () => runNpmFn(['run', 'check:locales'], frontendDirectory) },
    { name: 'FSD import compliance', run: () => runNpmFn(['run', 'check:imports'], frontendDirectory) },
    { name: 'Frontend type and asset build', run: () => runNpmFn(['run', 'build'], frontendDirectory) },
    { name: 'Rust formatting', run: () => runCommandFn('cargo', ['fmt', '--all', '--', '--check'], { cwd: workspaceRoot }) },
    { name: 'Rust compilation check', run: () => runCommandFn('cargo', ['check', '--workspace', '--locked'], { cwd: workspaceRoot }) },
    {
      name: 'Rust tests',
      run: () =>
        runCommandFn(
          'cargo',
          ['test', '--workspace', '--features', profileTestFeature, '--locked', '--', '--test-threads=1'],
          { cwd: workspaceRoot, env: { ...process.env, RUST_TEST_THREADS: '1' } },
        ),
    },
  ], write);
}

export async function runCheckTypes({
  runNpmFn = runNpm,
  runCommandFn = runCommand,
  frontendDirectory = getFrontendDirectory(),
  workspaceRoot = getWorkspaceRoot(),
  prepareTauriSidecarFn = prepareTauriSidecar,
  write = (message) => process.stdout.write(message),
} = {}) {
  prepareTauriSidecarFn();
  await runValidationTasks('check-types', [
    {
      name: 'Biome lint',
      run: () =>
        runNpmFn(
          ['exec', '--', 'biome', 'check', '.', '../../../scripts/tasks/check.mjs', '../../../scripts/check-imports.mjs'],
          frontendDirectory,
        ),
    },
    { name: 'TypeScript type-check', run: () => runNpmFn(['exec', '--', 'tsc', '--noEmit'], frontendDirectory) },
    { name: 'Rust formatting', run: () => runCommandFn('cargo', ['fmt', '--all', '--', '--check'], { cwd: workspaceRoot }) },
    { name: 'Rust compilation check', run: () => runCommandFn('cargo', ['check', '--workspace', '--locked'], { cwd: workspaceRoot }) },
  ], write);
}

export function prepareTauriSidecar(buildLocalServiceFn = buildLocalService) {
  buildLocalServiceFn('debug', true);
}

export async function runValidationTasks(taskName, tasks, write = (message) => process.stdout.write(message)) {
  const results = await Promise.allSettled(tasks.map(async (task) => task.run()));
  const failedTasks = [];

  for (let index = 0; index < results.length; index += 1) {
    const result = results[index];
    const task = tasks[index];

    if (result.status === 'fulfilled') {
      write(`${task.name} passed.\n`);
      continue;
    }

    failedTasks.push(task.name);
    const reason = result.reason instanceof Error ? result.reason.message : String(result.reason);
    write(`${task.name} failed: ${reason}\n`);
  }

  if (failedTasks.length > 0) {
    throw new Error(`${taskName}: ${failedTasks.length} task(s) failed: ${failedTasks.join(', ')}.`);
  }
}
