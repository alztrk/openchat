import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  withWorkspaceArtifactLock,
  workspaceArtifactLockDirectory,
} from '../../../../scripts/shared/artifact-task-lock.mjs';
import { pruneStaleArtifacts } from '../../../../scripts/shared/target-utils.mjs';
import { cleanGeneratedOutputs, parseCleanOptions } from '../../../../scripts/tasks/clean.mjs';
import { runMaintainTarget } from '../../../../scripts/tasks/maintain.mjs';

const bytesPerGiB = 1024n ** 3n;

test('artifact task lock prevents overlapping workspace build and cleanup tasks', async () => {
  const fixture = createFixture();
  let releaseFirstTask;
  let firstTaskStarted;
  const firstStarted = new Promise((resolve) => {
    firstTaskStarted = resolve;
  });
  const firstTask = withWorkspaceArtifactLock(
    'build',
    () =>
      new Promise((resolve) => {
        releaseFirstTask = resolve;
        firstTaskStarted();
      }),
    lockOptions(fixture, 4101, (processId) => processId === 4101),
  );

  try {
    await firstStarted;
    try {
      await assert.rejects(
        withWorkspaceArtifactLock('clean', () => undefined, lockOptions(fixture, 4102, (processId) => processId === 4101)),
        /Artifact task 'build' is already running \(process 4101\)/u,
      );
    } finally {
      releaseFirstTask();
      await firstTask;
    }

    const lockDirectory = workspaceArtifactLockDirectory(fixture.workspaceRoot, fixture.temporaryDirectory);
    assert.deepEqual(readdirSync(lockDirectory), []);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('artifact task lock releases its marker when the task fails', async () => {
  const fixture = createFixture();
  try {
    await assert.rejects(
      withWorkspaceArtifactLock('check', () => Promise.reject(new Error('check failed')), lockOptions(fixture, 4201)),
      /check failed/u,
    );
    const result = await withWorkspaceArtifactLock('clean', () => 'released', lockOptions(fixture, 4201));
    assert.equal(result, 'released');
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('artifact task lock removes markers for processes that are no longer running', async () => {
  const fixture = createFixture();
  try {
    const lockDirectory = workspaceArtifactLockDirectory(fixture.workspaceRoot, fixture.temporaryDirectory);
    writeFileSync(
      path.join(lockDirectory, '4301-11111111-1111-4111-8111-111111111111.json'),
      JSON.stringify({ version: 1, pid: 4301, task: 'build', startedAt: '2026-11-04T00:00:00.000Z' }),
    );

    const result = await withWorkspaceArtifactLock('clean', () => 'acquired', {
      ...lockOptions(fixture, 4302),
      processIsAlive: () => false,
    });

    assert.equal(result, 'acquired');
    assert.deepEqual(readdirSync(lockDirectory), []);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('clean previews generated directories without removing them', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  mkdirSync(target);
  const output = [];

  try {
    const result = cleanGeneratedOutputs({
      workspaceRoot: fixture.workspaceRoot,
      apply: false,
      write: (message) => output.push(message),
    });

    assert.deepEqual(result.targets, ['target']);
    assert.deepEqual(result.removed, []);
    assert.equal(statSync(target).isDirectory(), true);
    assert.match(output.join(''), /Would remove target\./u);
    assert.match(output.join(''), /Preview only/u);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('clean refuses an active build before removing any generated directory', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  mkdirSync(target);
  writeFileSync(path.join(target, 'keep.bin'), 'keep');

  try {
    assert.throws(
      () =>
        cleanGeneratedOutputs({
          workspaceRoot: fixture.workspaceRoot,
          apply: true,
          assertNoActiveBuild: () => {
            throw new Error('build active');
          },
          write: () => undefined,
        }),
      /build active/u,
    );
    assert.equal(statSync(path.join(target, 'keep.bin')).isFile(), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('clean removes only the listed output directories after the active-build guard passes', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  const frontendDist = path.join(fixture.workspaceRoot, 'apps', 'desktop', 'frontend', 'dist');
  const keepFile = path.join(fixture.workspaceRoot, 'keep.txt');
  mkdirSync(target);
  mkdirSync(frontendDist, { recursive: true });
  writeFileSync(keepFile, 'keep');
  let activeBuildChecks = 0;

  try {
    const result = cleanGeneratedOutputs({
      workspaceRoot: fixture.workspaceRoot,
      apply: true,
      assertNoActiveBuild: () => {
        activeBuildChecks += 1;
      },
      write: () => undefined,
    });

    assert.deepEqual(result.removed, ['target', 'apps/desktop/frontend/dist']);
    assert.equal(existsSync(target), false);
    assert.equal(existsSync(frontendDist), false);
    assert.equal(existsSync(keepFile), true);
    assert.equal(activeBuildChecks, 3);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('clean accepts preview by default and requires explicit apply for deletion', () => {
  const output = [];
  assert.deepEqual(parseCleanOptions([], (message) => output.push(message)), { apply: false });
  assert.deepEqual(parseCleanOptions(['--apply'], (message) => output.push(message)), { apply: true });
  assert.throws(() => parseCleanOptions(['--unknown'], () => undefined), /Use only --apply/u);
});

test('clean preflights every path before deleting any generated directory', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  const invalidTarget = path.join(fixture.workspaceRoot, 'services', 'openchat-local-service', 'target');
  mkdirSync(target);
  mkdirSync(path.dirname(invalidTarget), { recursive: true });
  writeFileSync(invalidTarget, 'preserve');

  try {
    assert.throws(
      () => cleanGeneratedOutputs({ workspaceRoot: fixture.workspaceRoot, apply: true, write: () => undefined }),
      /Expected a generated directory at services\/openchat-local-service\/target/u,
    );
    assert.equal(statSync(target).isDirectory(), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('clean refuses a generated output directory that resolves outside the workspace', () => {
  const fixture = createFixture();
  const externalApps = path.join(fixture.root, 'external-apps');
  const externalDist = path.join(externalApps, 'desktop', 'frontend', 'dist');
  mkdirSync(externalDist, { recursive: true });
  symlinkSync(externalApps, path.join(fixture.workspaceRoot, 'apps'), process.platform === 'win32' ? 'junction' : 'dir');

  try {
    assert.throws(
      () => cleanGeneratedOutputs({ workspaceRoot: fixture.workspaceRoot, apply: true, write: () => undefined }),
      /path outside the workspace/u,
    );
    assert.equal(statSync(externalDist).isDirectory(), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('maintain-target does not inspect or prune artifacts when usage is below the limit', async () => {
  const fixture = createFixture();
  writeFileSync(path.join(fixture.workspaceRoot, 'Cargo.toml'), '[workspace]\n');
  const target = { path: path.join(fixture.workspaceRoot, 'target'), bytes: 10n };
  let pruneCalls = 0;
  let lockCalls = 0;
  const output = [];

  try {
    await runMaintainTarget(['--apply', '--max-gib', '1'], {
      workspaceRoot: fixture.workspaceRoot,
      discoverTargetsFn: () => [target],
      pruneStaleArtifactsFn: () => {
        pruneCalls += 1;
        return { prunedBytes: 0n, prunedCount: 0, prunedPaths: [] };
      },
      withArtifactTaskLock: async (_taskName, operation) => {
        lockCalls += 1;
        return operation();
      },
      activeBuildIsRunningFn: () => false,
      write: (message) => output.push(message),
    });

    assert.equal(pruneCalls, 0);
    assert.equal(lockCalls, 0);
    assert.match(output.join(''), /no cleanup is needed/u);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('maintain-target checks for an active build before stale-artifact pruning', async () => {
  const fixture = createFixture();
  writeFileSync(path.join(fixture.workspaceRoot, 'Cargo.toml'), '[workspace]\n');
  const target = { path: path.join(fixture.workspaceRoot, 'target'), bytes: bytesPerGiB + 1n };
  let pruneCalls = 0;

  try {
    await assert.rejects(
      runMaintainTarget(['--apply', '--max-gib', '1'], {
        workspaceRoot: fixture.workspaceRoot,
        discoverTargetsFn: () => [target],
        pruneStaleArtifactsFn: () => {
          pruneCalls += 1;
          return { prunedBytes: 1n, prunedCount: 1, prunedPaths: ['stale'] };
        },
        withArtifactTaskLock: async (_taskName, operation) => operation(),
        activeBuildIsRunningFn: () => true,
        write: () => undefined,
      }),
      /build is running; target cleanup was not started/u,
    );
    assert.equal(pruneCalls, 0);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('maintain-target previews full target cleanup without applying it', async () => {
  const fixture = createFixture();
  writeFileSync(path.join(fixture.workspaceRoot, 'Cargo.toml'), '[workspace]\n');
  const target = { path: path.join(fixture.workspaceRoot, 'target'), bytes: bytesPerGiB + 1n };
  const output = [];
  let cargoCalls = 0;

  try {
    await runMaintainTarget(['--max-gib', '1'], {
      workspaceRoot: fixture.workspaceRoot,
      discoverTargetsFn: () => [target],
      pruneStaleArtifactsFn: () => ({ prunedBytes: 0n, prunedCount: 0, prunedPaths: [] }),
      runCommandFn: () => {
        cargoCalls += 1;
      },
      write: (message) => output.push(message),
    });

    assert.equal(cargoCalls, 0);
    assert.match(output.join(''), /Would clean Cargo target directory target/u);
    assert.match(output.join(''), /Preview only/u);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('maintain-target dry-runs every target before cleaning any target', async () => {
  const fixture = createFixture();
  writeFileSync(path.join(fixture.workspaceRoot, 'Cargo.toml'), '[workspace]\n');
  const targets = [
    { path: path.join(fixture.workspaceRoot, 'target'), bytes: bytesPerGiB / 2n + 1n },
    { path: path.join(fixture.workspaceRoot, 'services', 'openchat-local-service', 'target'), bytes: bytesPerGiB / 2n + 1n },
  ];
  const calls = [];
  const output = [];

  try {
    await runMaintainTarget(['--apply', '--max-gib', '1'], {
      workspaceRoot: fixture.workspaceRoot,
      discoverTargetsFn: () => targets,
      pruneStaleArtifactsFn: () => ({ prunedBytes: 0n, prunedCount: 0, prunedPaths: [] }),
      withArtifactTaskLock: async (_taskName, operation) => operation(),
      activeBuildIsRunningFn: () => false,
      runCommandFn: (_command, args) => calls.push(args.includes('--dry-run')),
      write: (message) => output.push(message),
    });

    assert.deepEqual(calls, [true, true, false, false]);
    assert.match(output.join(''), /Cargo target cleanup completed/u);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('maintain-target previews stale artifacts and verifies the same candidates before pruning', async () => {
  const fixture = createFixture();
  writeFileSync(path.join(fixture.workspaceRoot, 'Cargo.toml'), '[workspace]\n');
  const target = { path: path.join(fixture.workspaceRoot, 'target'), bytes: bytesPerGiB + 5n };
  const stalePath = path.join(target.path, 'debug', 'deps', 'old.rlib');
  let currentBytes = target.bytes;
  let applyPaths;
  const output = [];

  try {
    await runMaintainTarget(['--apply', '--max-gib', '1'], {
      workspaceRoot: fixture.workspaceRoot,
      discoverTargetsFn: () => [{ ...target, bytes: currentBytes }],
      pruneStaleArtifactsFn: (_targetPath, apply, _beforeRemove, expectedPaths) => {
        if (apply) {
          applyPaths = expectedPaths;
          currentBytes = bytesPerGiB - 1n;
        }
        return { prunedBytes: 10n, prunedCount: 1, prunedPaths: [stalePath] };
      },
      withArtifactTaskLock: async (_taskName, operation) => operation(),
      activeBuildIsRunningFn: () => false,
      write: (message) => output.push(message),
    });

    assert.deepEqual(applyPaths, [stalePath]);
    assert.match(output.join(''), /Will prune stale Cargo artifact target\\debug\\deps\\old\.rlib/u);
    assert.match(output.join(''), /Target usage after stale-artifact cleanup/u);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('stale-artifact pruning leaves unrecognized Cargo target root directories intact', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  const deps = path.join(target, 'debug', 'deps');
  const releaseOutput = path.join(target, 'release', 'keep.bin');
  const hostOutput = path.join(target, 'x86_64-pc-windows-msvc', 'keep.bin');
  const oldArtifact = path.join(deps, 'sample-1111111111111111.rlib');
  const newArtifact = path.join(deps, 'sample-2222222222222222.rlib');
  mkdirSync(deps, { recursive: true });
  mkdirSync(path.dirname(releaseOutput), { recursive: true });
  mkdirSync(path.dirname(hostOutput), { recursive: true });
  writeFileSync(path.join(target, '.rustc_info.json'), '{}');
  writeFileSync(releaseOutput, 'release');
  writeFileSync(hostOutput, 'host');
  writeFileSync(oldArtifact, 'old');
  writeFileSync(newArtifact, 'new');
  const oldTime = new Date('2020-01-01T00:00:00.000Z');
  const newTime = new Date('2021-01-01T00:00:00.000Z');
  utimesSync(oldArtifact, oldTime, oldTime);
  utimesSync(newArtifact, newTime, newTime);

  try {
    const preview = pruneStaleArtifacts(target, false);
    assert.deepEqual(preview.prunedPaths, [oldArtifact]);
    assert.equal(statSync(releaseOutput).isFile(), true);
    assert.equal(statSync(hostOutput).isFile(), true);

    const applied = pruneStaleArtifacts(target, true, () => undefined, preview.prunedPaths);
    assert.deepEqual(applied.prunedPaths, [oldArtifact]);
    assert.equal(existsSync(oldArtifact), false);
    assert.equal(statSync(newArtifact).isFile(), true);
    assert.equal(statSync(releaseOutput).isFile(), true);
    assert.equal(statSync(hostOutput).isFile(), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('stale-artifact pruning refuses a dependency directory that resolves outside the target', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  const externalDebug = path.join(fixture.root, 'external-target', 'debug');
  mkdirSync(path.join(target), { recursive: true });
  mkdirSync(path.join(externalDebug, 'deps'), { recursive: true });
  symlinkSync(externalDebug, path.join(target, 'debug'), process.platform === 'win32' ? 'junction' : 'dir');

  try {
    assert.throws(() => pruneStaleArtifacts(target), /symbolic-link directory/u);
    assert.equal(statSync(path.join(externalDebug, 'deps')).isDirectory(), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('stale-artifact pruning fails before deletion when candidates change after preview', () => {
  const fixture = createFixture();
  const target = path.join(fixture.workspaceRoot, 'target');
  const deps = path.join(target, 'debug', 'deps');
  const olderArtifact = path.join(deps, 'sample-1111111111111111.rlib');
  const newerArtifact = path.join(deps, 'sample-2222222222222222.rlib');
  const middleArtifact = path.join(deps, 'sample-3333333333333333.rlib');
  mkdirSync(deps, { recursive: true });
  writeFileSync(olderArtifact, 'old');
  writeFileSync(newerArtifact, 'new');
  const oldTime = new Date('2020-01-01T00:00:00.000Z');
  const newTime = new Date('2021-01-01T00:00:00.000Z');
  utimesSync(olderArtifact, oldTime, oldTime);
  utimesSync(newerArtifact, newTime, newTime);

  try {
    const preview = pruneStaleArtifacts(target, false);
    writeFileSync(middleArtifact, 'middle');
    utimesSync(middleArtifact, oldTime, oldTime);

    assert.throws(
      () => pruneStaleArtifacts(target, true, () => undefined, preview.prunedPaths),
      /changed after preview/u,
    );
    assert.equal(existsSync(olderArtifact), true);
    assert.equal(existsSync(middleArtifact), true);
    assert.equal(existsSync(newerArtifact), true);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

function createFixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'openchat-artifact-tasks-'));
  const workspaceRoot = path.join(root, 'workspace');
  const temporaryDirectory = path.join(root, 'system-temp');
  mkdirSync(workspaceRoot);
  mkdirSync(temporaryDirectory);
  return { root, workspaceRoot, temporaryDirectory };
}

function lockOptions(fixture, processId, processIsAlive = (ownerPid) => ownerPid === processId) {
  return {
    workspaceRoot: fixture.workspaceRoot,
    temporaryDirectory: fixture.temporaryDirectory,
    processId,
    processIsAlive,
  };
}
