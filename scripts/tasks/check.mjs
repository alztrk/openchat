import { runNpm, runCommand, getFrontendDirectory, getWorkspaceRoot } from '../shared/runner.mjs';
import { ensureNoArguments } from '../help.mjs';

function runNpmAsync(args, cwd = getFrontendDirectory()) {
  return Promise.resolve(runNpm(args, cwd));
}

export const check = {
  name: 'check',
  description: 'Runs frontend lint, TypeScript check, Rust formatting, checking, and tests.',
  args: [],
  async run() {
    ensureNoArguments('check', []);
    const results = await Promise.allSettled([
      runNpmAsync(['exec', '--', 'biome', 'check']),
      runNpmAsync(['exec', '--', 'tsc', '--noEmit']),
      runCommand('cargo', ['fmt', '--all', '--', '--check'], { cwd: getWorkspaceRoot() }),
      runCommand('cargo', ['check', '--workspace'], { cwd: getWorkspaceRoot() }),
      runCommand('cargo', ['test', '--workspace', '--features', 'openchat-local-service/desktop-profile-test-support', '--', '--test-threads=0'], {
        cwd: getWorkspaceRoot(),
      }),
    ]);

    const tasks = ['Biome lint', 'TypeScript type-check', 'Rust formatting', 'Rust compilation check', 'Rust tests'];
    let failed = false;

    for (let i = 0; i < results.length; i += 1) {
      if (results[i].status === 'rejected') {
        failed = true;
        process.stderr.write(tasks[i] + ' failed.\n');
      }
    }

    if (failed) {
      throw new Error('check: one or more tasks failed.');
    }
  },
};

export const checkTypes = {
  name: 'check-types',
  description: 'Runs frontend lint, TypeScript check, Rust formatting, and compilation check (no tests).',
  args: [],
  async run() {
    ensureNoArguments('check-types', []);
    const results = await Promise.allSettled([
      runNpmAsync(['exec', '--', 'biome', 'check']),
      runNpmAsync(['exec', '--', 'tsc', '--noEmit']),
      runCommand('cargo', ['fmt', '--all', '--', '--check'], { cwd: getWorkspaceRoot() }),
      runCommand('cargo', ['check', '--workspace'], { cwd: getWorkspaceRoot() }),
    ]);

    const tasks = ['Biome lint', 'TypeScript type-check', 'Rust formatting', 'Rust compilation check'];
    let failed = false;

    for (let i = 0; i < results.length; i += 1) {
      if (results[i].status === 'rejected') {
        failed = true;
        process.stderr.write(tasks[i] + ' failed.\n');
      }
    }

    if (failed) {
      throw new Error('check-types: one or more tasks failed.');
    }
  },
};
