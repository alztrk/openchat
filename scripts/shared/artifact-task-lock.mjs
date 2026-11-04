import { createHash, randomUUID } from 'node:crypto';
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { isWithinDirectory } from './fs-safety.mjs';
import { getWorkspaceRoot } from './runner.mjs';

const lockVersion = 1;
const stalePendingAgeMs = 5 * 60 * 1_000;
const lockFilePattern = /^\d+-[0-9a-f-]+\.json$/iu;

export function workspaceArtifactLockDirectory(workspaceRoot = getWorkspaceRoot(), temporaryDirectory = tmpdir()) {
  const canonicalWorkspaceRoot = realpathSync(workspaceRoot);
  const canonicalTemporaryDirectory = realpathSync(temporaryDirectory);
  const workspaceHash = createHash('sha256').update(canonicalWorkspaceRoot).digest('hex').slice(0, 24);
  const lockDirectory = path.join(canonicalTemporaryDirectory, `openchat-artifact-task-${workspaceHash}`);

  mkdirSync(lockDirectory, { recursive: true });
  const metadata = lstatSync(lockDirectory);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error('The OpenChat artifact-task lock path is not a regular directory.');
  }

  const canonicalLockDirectory = realpathSync(lockDirectory);
  if (!isWithinDirectory(canonicalTemporaryDirectory, canonicalLockDirectory)) {
    throw new Error('The OpenChat artifact-task lock path resolves outside the temporary directory.');
  }
  return canonicalLockDirectory;
}

export async function withWorkspaceArtifactLock(
  taskName,
  operation,
  {
    workspaceRoot = getWorkspaceRoot(),
    temporaryDirectory = tmpdir(),
    processId = process.pid,
    processIsAlive = isProcessAlive,
    createId = randomUUID,
  } = {},
) {
  if (typeof taskName !== 'string' || taskName.trim().length === 0) {
    throw new Error('An artifact task name is required.');
  }
  if (typeof operation !== 'function') {
    throw new Error('An artifact task operation is required.');
  }
  if (!Number.isSafeInteger(processId) || processId < 1) {
    throw new Error('The artifact task process identifier is invalid.');
  }

  const lockDirectory = workspaceArtifactLockDirectory(workspaceRoot, temporaryDirectory);
  const lockId = createId();
  const markerPath = path.join(lockDirectory, `${processId}-${lockId}.json`);
  const pendingPath = path.join(lockDirectory, `.pending-${processId}-${lockId}`);
  let operationResult;
  let operationError;
  let operationFailed = false;

  try {
    removeExpiredPendingFiles(lockDirectory);
    writeFileSync(
      pendingPath,
      JSON.stringify({ version: lockVersion, pid: processId, task: taskName, startedAt: new Date().toISOString() }),
      { flag: 'wx' },
    );
    renameSync(pendingPath, markerPath);
    assertNoOtherArtifactTask(lockDirectory, markerPath, processIsAlive);
    operationResult = await operation();
  } catch (error) {
    operationFailed = true;
    operationError = error;
  }

  let cleanupError;
  try {
    rmSync(markerPath, { force: true });
    rmSync(pendingPath, { force: true });
  } catch (error) {
    cleanupError = error;
  }

  if (operationFailed && cleanupError) {
    throw new AggregateError([operationError, cleanupError], 'The artifact task and its lock cleanup both failed.');
  }
  if (operationFailed) throw operationError;
  if (cleanupError) throw cleanupError;
  return operationResult;
}

function assertNoOtherArtifactTask(lockDirectory, ownMarkerPath, processIsAlive) {
  for (const entry of readdirSync(lockDirectory)) {
    const entryPath = path.join(lockDirectory, entry);
    if (entryPath === ownMarkerPath) continue;
    if (entry.startsWith('.pending-')) continue;
    if (!lockFilePattern.test(entry)) {
      throw new Error(`Cannot verify an artifact-task lock entry '${entry}'. Refusing to continue.`);
    }

    const metadata = lstatSync(entryPath);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      throw new Error(`Cannot verify an artifact-task lock entry '${entry}'. Refusing to continue.`);
    }

    const owner = readLockOwner(entryPath);
    if (!processIsAlive(owner.pid)) {
      rmSync(entryPath);
      continue;
    }

    throw new Error(`Artifact task '${owner.task}' is already running (process ${owner.pid}). Wait for it to finish, then retry.`);
  }
}

function readLockOwner(filePath) {
  let value;
  try {
    value = JSON.parse(readFileSync(filePath, 'utf8'));
  } catch (cause) {
    throw new Error(`Cannot read artifact-task lock entry '${path.basename(filePath)}'. Refusing to continue.`, { cause });
  }

  if (
    value === null ||
    typeof value !== 'object' ||
    value.version !== lockVersion ||
    !Number.isSafeInteger(value.pid) ||
    value.pid < 1 ||
    typeof value.task !== 'string' ||
    value.task.trim().length === 0
  ) {
    throw new Error(`Artifact-task lock entry '${path.basename(filePath)}' has invalid data. Refusing to continue.`);
  }
  return { pid: value.pid, task: value.task };
}

function removeExpiredPendingFiles(lockDirectory) {
  for (const entry of readdirSync(lockDirectory)) {
    if (!entry.startsWith('.pending-')) continue;
    const entryPath = path.join(lockDirectory, entry);
    const metadata = lstatSync(entryPath);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      throw new Error(`Cannot verify a pending artifact-task lock entry '${entry}'. Refusing to continue.`);
    }
    if (Date.now() - statSync(entryPath).mtimeMs > stalePendingAgeMs) {
      rmSync(entryPath);
    }
  }
}

function isProcessAlive(processId) {
  try {
    process.kill(processId, 0);
    return true;
  } catch (error) {
    if (error?.code === 'ESRCH') return false;
    if (error?.code === 'EPERM') return true;
    throw error;
  }
}
