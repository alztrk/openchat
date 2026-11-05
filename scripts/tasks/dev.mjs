import { randomUUID } from 'node:crypto';
import { existsSync, lstatSync, readFileSync, unlinkSync } from 'node:fs';
import path from 'node:path';
import { runCommand } from '../shared/runner.mjs';
import {
  buildLocalService,
  buildLocalServiceInterruptibly,
  ensureFrontendDependencies,
  ensureFrontendDependenciesInterruptibly,
} from '../shared/rust.mjs';
import { runInterruptibleCommand } from '../shared/session.mjs';
import { reserveTemporaryTestProfileRoot, temporaryTestProfilePath } from '../shared/profile.mjs';
import { removeTemporaryTestProfile } from '../shared/profile-clean.mjs';
import { getFrontendDirectory, getTauriDirectory } from '../shared/runner.mjs';
import { ensureNoArguments } from '../help.mjs';

const tauriCliPath = path.join(getFrontendDirectory(), 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const tauriShutdownGraceMs = process.platform === 'win32' ? 10_000 : undefined;

export function assertDesktopTestProfileExit(status, markerContents, runId) {
  if (markerContents !== runId) {
    const outcome = status === null ? 'was terminated by a signal' : `exited with code ${status}`;
    throw new Error(`The desktop test ${outcome} without a confirmed normal window close.`);
  }
  if (status !== 0 && status !== 0xffff_ffff) {
    throw new Error(`The desktop test confirmed a normal window close but exited with unexpected code ${status}.`);
  }
}

function readNormalCloseMarker(markerPath) {
  if (!existsSync(markerPath)) return null;
  const metadata = lstatSync(markerPath);
  if (!metadata.isFile() || metadata.isSymbolicLink()) return null;
  return readFileSync(markerPath, 'utf8');
}

function removeNormalCloseMarker(markerPath) {
  if (!existsSync(markerPath)) return;
  const metadata = lstatSync(markerPath);
  if (metadata.isFile() && !metadata.isSymbolicLink()) unlinkSync(markerPath);
}

export const dev = {
  name: 'dev',
  description: 'Builds the debug local-service sidecar and starts Tauri dev server.',
  args: [],
  run() {
    ensureNoArguments('dev', []);
    buildLocalService('debug', false);
    ensureFrontendDependencies();
    runCommand(process.execPath, [tauriCliPath, 'dev', '--config', 'tauri.dev.conf.json'], { cwd: getTauriDirectory() });
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
        if (!value || !path.isAbsolute(value)) {
          throw new Error('--profile-root requires an absolute path.');
        }
        explicitProfileRoot = path.normalize(value);
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
      getRetainedProfilePath: () => profileRoot,
      run: async (interruptController) => {
        profileRoot ??= reserveTemporaryTestProfileRoot();
        await buildLocalServiceInterruptibly('debug', false, ['desktop-profile-test-support'], interruptController);
        interruptController.throwIfInterrupted();
        await ensureFrontendDependenciesInterruptibly(interruptController);
        interruptController.throwIfInterrupted();
        process.stdout.write('Isolated desktop test profile: ' + profileRoot + '\n');

        const runId = randomUUID();
        const markerPath = path.join(profileRoot, `normal-close-${runId}.marker`);
        let result;
        let commandError;
        let markerContents;
        try {
          result = await runInterruptibleCommand(process.execPath, [
            tauriCliPath, 'dev', '--config', 'tauri.dev.conf.json', '--features', 'desktop-profile-test-support',
            '--', '--', '--desktop-test-profile-root', profileRoot,
            '--desktop-test-run-id', runId,
          ], {
            cwd: getTauriDirectory(),
            interruptController,
            allowFailure: true,
            shutdownGraceMs: tauriShutdownGraceMs,
            shutdownMarker: profileRoot,
          });
        } catch (error) {
          commandError = error;
        } finally {
          markerContents = readNormalCloseMarker(markerPath);
          removeNormalCloseMarker(markerPath);
        }

        if (commandError) throw commandError;
        if (!result) throw new Error('The desktop command exited without a result.');
        assertDesktopTestProfileExit(result.status, markerContents, runId);
      },
      cleanup: async () => {
        if (cleanupProfileOnExit && profileRoot) await removeTemporaryTestProfile(profileRoot);
      },
    });

    if (interruptedSignal) process.exitCode = interruptedSignal === 'SIGINT' ? 130 : 143;
  },
};

export const startupBenchmark = {
  name: 'startup-benchmark',
  description: 'Measures debug startup time to Tauri window show completion and React-ready.',
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
    const { runTemporaryProfileSession } = await import('../shared/session.mjs');

    let profileRoot;
    const { interruptedSignal } = await runTemporaryProfileSession({
      getRetainedProfilePath: () => profileRoot,
      run: async (interruptController) => {
        profileRoot = temporaryTestProfilePath();
        const resultPath = path.join(profileRoot, 'result.json');
        await buildLocalServiceInterruptibly('debug', false, ['desktop-profile-test-support'], interruptController);
        interruptController.throwIfInterrupted();
        await ensureFrontendDependenciesInterruptibly(interruptController);
        interruptController.throwIfInterrupted();

        await runInterruptibleCommand(process.execPath, [
          tauriCliPath, 'dev', '--config', 'tauri.dev.conf.json', '--features', 'startup-benchmark',
          '--', '--', '--startup-benchmark-root', profileRoot,
        ], {
          cwd: getTauriDirectory(),
          interruptController,
          shutdownGraceMs: tauriShutdownGraceMs,
          shutdownMarker: profileRoot,
        });
        interruptController.throwIfInterrupted();

        const result = JSON.parse(fs.readFileSync(resultPath, 'utf8'));
        const processToWindowShowCompletedMs = result?.processToWindowVisibleMs;
        const elapsedMs = result?.processToReactReadyMs;
        const performanceSnapshot = result?.performanceSnapshot;
        const renderedRootClass = performanceSnapshot?.renderedRootClass;
        const startupMilestones = performanceSnapshot?.startupMilestones;
        if (
          typeof processToWindowShowCompletedMs !== 'number' ||
          !Number.isFinite(processToWindowShowCompletedMs) ||
          processToWindowShowCompletedMs <= 0 ||
          typeof elapsedMs !== 'number' ||
          !Number.isFinite(elapsedMs) ||
          elapsedMs <= 0 ||
          processToWindowShowCompletedMs > elapsedMs ||
          typeof renderedRootClass !== 'string' ||
          !renderedRootClass.split(/\s+/u).includes('app-shell') ||
          renderedRootClass.includes('startup-error-page') ||
          !Array.isArray(startupMilestones) ||
          !startupMilestones.some((milestone) => milestone?.name === 'react-first-commit-complete')
        ) {
          throw new Error('The desktop did not reach the OpenChat application shell with startup milestones.');
        }

        const databasePath = path.join(profileRoot, 'storage', 'db', 'openchat.sqlite3');
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

        process.stdout.write(
          `Debug startup to Tauri window show() completion: ${processToWindowShowCompletedMs.toFixed(1)} ms; ` +
            `React ready: ${elapsedMs.toFixed(1)} ms; app shell and encrypted database verified.\n`,
        );
        process.stdout.write('Renderer startup milestones (navigation-relative):\n');
        let previousMilestoneTimeMs = 0;
        for (const milestone of startupMilestones) {
          if (typeof milestone?.name !== 'string' || typeof milestone?.timeMs !== 'number') continue;
          const deltaMs = milestone.timeMs - previousMilestoneTimeMs;
          process.stdout.write(
            `  ${milestone.name}: ${milestone.timeMs.toFixed(1)} ms (+${deltaMs.toFixed(1)} ms)\n`,
          );
          previousMilestoneTimeMs = milestone.timeMs;
        }
        const navigation = performanceSnapshot?.navigation;
        if (navigation && typeof navigation === 'object') {
          process.stdout.write(
            `Navigation: DOM interactive ${navigation.domInteractiveMs?.toFixed(1) ?? 'unavailable'} ms, ` +
              `DOMContentLoaded ${navigation.domContentLoadedMs?.toFixed(1) ?? 'unavailable'} ms, ` +
              `load ${navigation.loadEventEndMs?.toFixed(1) ?? 'unavailable'} ms.\n`,
          );
        }
        const resources = performanceSnapshot?.resources;
        if (Array.isArray(resources)) {
          const latestResources = resources.slice(0, 20);
          process.stdout.write(`Latest renderer resources (top ${latestResources.length} of ${resources.length} by response end):\n`);
          for (const resource of latestResources) {
            if (typeof resource?.path !== 'string' || typeof resource?.responseEndMs !== 'number') continue;
            process.stdout.write(
              `  ${resource.responseEndMs.toFixed(1)} ms ${resource.path} ` +
                `(${resource.durationMs.toFixed(1)} ms, ` +
                `${resource.encodedBytes} encoded/${resource.decodedBytes} decoded/${resource.transferBytes} transferred bytes)\n`,
            );
          }
        }
      },
      cleanup: async () => {
        if (profileRoot) await removeTemporaryTestProfile(profileRoot);
      },
    });

    if (interruptedSignal) process.exitCode = interruptedSignal === 'SIGINT' ? 130 : 143;
  },
};
