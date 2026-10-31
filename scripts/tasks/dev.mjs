import { runNpm } from '../shared/runner.mjs';
import { buildLocalService } from '../shared/rust.mjs';
import { ensureFrontendDependencies } from '../shared/rust.mjs';
import { buildLocalServiceInterruptibly, ensureFrontendDependenciesInterruptibly } from '../shared/rust.mjs';
import { runInterruptibleCommand } from '../shared/session.mjs';
import { reserveTemporaryTestProfileRoot } from '../shared/profile.mjs';
import { removeTemporaryTestProfile } from '../shared/profile-clean.mjs';
import { getFrontendDirectory } from '../shared/runner.mjs';
import { ensureNoArguments } from '../help.mjs';

export const dev = {
  name: 'dev',
  description: 'Builds the debug local-service sidecar and starts Tauri dev server.',
  args: [],
  run() {
    ensureNoArguments('dev', []);
    buildLocalService('debug', false);
    ensureFrontendDependencies();
    runNpm(['run', 'tauri:dev']);
  },
};

export const devTestProfile = {
  name: 'dev-test-profile',
  description:
    'Builds the Tauri host and local-service sidecar with the debug-only ' +
    'desktop-profile-test-support feature, then starts the app with an ' +
    'isolated profile under the OS temporary directory.',
  args: [
    { name: '--help', short: '-h', description: 'Show this help message' },
    { name: '--cleanup-profile', description: 'Remove the automatically created profile after the app exits' },
    { name: '--profile-root', value: '<path>', description: 'Use a custom profile directory (must be an absolute path)' },
  ],
  async run(args) {
    if (args.length === 1 && (args[0] === '--help' || args[0] === '-h')) {
      process.stdout.write(
        'Usage: npm run openchat -- dev-test-profile [--cleanup-profile] [--profile-root <absolute-temp-path>]\n' +
          'Creates a UUID-named OS temporary profile when --profile-root is omitted.\n' +
          '--cleanup-profile removes only an automatically created profile after the app exits.\n',
      );
      return;
    }

    const pathMod = await import('node:path');
    let cleanupProfileOnExit = false;
    let explicitProfileRoot;

    for (let index = 0; index < args.length; index += 1) {
      const argument = args[index];
      if (argument === '--cleanup-profile' && !cleanupProfileOnExit) {
        cleanupProfileOnExit = true;
        continue;
      }
      if (argument === '--profile-root' && explicitProfileRoot === undefined) {
        const value = args[index + 1];
        if (!value || !pathMod.isAbsolute(value)) {
          throw new Error('--profile-root requires an absolute path.');
        }
        explicitProfileRoot = pathMod.normalize(value);
        index += 1;
        continue;
      }
      throw new Error('Use --cleanup-profile and/or --profile-root with an absolute path; cleanup requires an automatically created profile.');
    }

    if (cleanupProfileOnExit && explicitProfileRoot !== undefined) {
      throw new Error('--cleanup-profile cannot be used with --profile-root.');
    }

    let profileRoot = explicitProfileRoot;
    const { runTemporaryProfileSession } = await import('../shared/session.mjs');

    const { interruptedSignal } = await runTemporaryProfileSession({
      run: async (interruptController) => {
        profileRoot ??= reserveTemporaryTestProfileRoot();
        await buildLocalServiceInterruptibly('debug', false, ['desktop-profile-test-support'], interruptController);
        interruptController.throwIfInterrupted();
        await ensureFrontendDependenciesInterruptibly(interruptController);
        interruptController.throwIfInterrupted();
        process.stdout.write('Isolated desktop test profile: ' + profileRoot + '\n');

        const npmCliPath = process.env.npm_execpath;
        if (!npmCliPath) {
          throw new Error('Run this task through its npm script so it can invoke npm without a shell.');
        }

        await runInterruptibleCommand(process.execPath, [
          npmCliPath, 'run', 'tauri:dev', '--', '--features', 'desktop-profile-test-support',
          '--', '--', '--desktop-test-profile-root', profileRoot,
        ], { cwd: getFrontendDirectory(), interruptController });
      },
      cleanup: () => {
        if (cleanupProfileOnExit && profileRoot) removeTemporaryTestProfile(profileRoot);
      },
    });

    if (interruptedSignal) process.exitCode = interruptedSignal === 'SIGINT' ? 130 : 143;
  },
};

export const startupBenchmark = {
  name: 'startup-benchmark',
  description: 'Measures debug startup time from process start to React-ready.',
  args: [
    { name: '--help', short: '-h', description: 'Show this help message' },
  ],
  async run(args) {
    if (args.length === 1 && (args[0] === '--help' || args[0] === '-h')) {
      process.stdout.write('Usage: npm run openchat -- startup-benchmark\n');
      return;
    }
    ensureNoArguments('startup-benchmark', args);

    const fs = await import('node:fs');
    const pathMod = await import('node:path');
    const { runTemporaryProfileSession } = await import('../shared/session.mjs');

    let profileRoot;
    const { interruptedSignal } = await runTemporaryProfileSession({
      run: async (interruptController) => {
        profileRoot = reserveTemporaryTestProfileRoot();
        const resultPath = pathMod.join(profileRoot, 'result.json');
        await buildLocalServiceInterruptibly('debug', false, ['desktop-profile-test-support'], interruptController);
        interruptController.throwIfInterrupted();
        await ensureFrontendDependenciesInterruptibly(interruptController);
        interruptController.throwIfInterrupted();

        const npmCliPath = process.env.npm_execpath;
        if (!npmCliPath) {
          throw new Error('Run this task through its npm script so it can invoke npm without a shell.');
        }

        await runInterruptibleCommand(process.execPath, [
          npmCliPath, 'run', 'tauri:dev', '--', '--features', 'startup-benchmark',
          '--', '--', '--startup-benchmark-root', profileRoot,
        ], { cwd: getFrontendDirectory(), interruptController });
        interruptController.throwIfInterrupted();

        const result = JSON.parse(fs.readFileSync(resultPath, 'utf8'));
        const elapsedMs = result?.processToReactReadyMs;
        const renderedRootClass = result?.performanceSnapshot?.renderedRootClass;
        if (
          typeof elapsedMs !== 'number' ||
          !Number.isFinite(elapsedMs) ||
          elapsedMs <= 0 ||
          typeof renderedRootClass !== 'string' ||
          !renderedRootClass.split(/\s+/u).includes('app-shell') ||
          renderedRootClass.includes('startup-error-page')
        ) {
          throw new Error('The desktop did not reach the OpenChat application shell.');
        }

        const databasePath = pathMod.join(profileRoot, 'storage', 'db', 'openchat.sqlite3');
        if (!fs.existsSync(databasePath)) {
          throw new Error('The benchmark profile did not create the active database.');
        }
        const databaseHeader = fs.readFileSync(databasePath).subarray(0, 16);
        if (
          databaseHeader.length !== 16 ||
          databaseHeader.equals(Buffer.from('SQLite format 3\0', 'ascii'))
        ) {
          throw new Error('The benchmark profile database is missing or plaintext SQLite.');
        }

        process.stdout.write(`Debug startup to React ready: ${elapsedMs.toFixed(1)} ms; app shell and encrypted database verified.\n`);
      },
      cleanup: () => {
        if (profileRoot) removeTemporaryTestProfile(profileRoot);
      },
    });

    if (interruptedSignal) process.exitCode = interruptedSignal === 'SIGINT' ? 130 : 143;
  },
};
