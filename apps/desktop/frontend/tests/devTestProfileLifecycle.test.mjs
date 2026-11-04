import assert from 'node:assert/strict';
import test from 'node:test';

import { assertDesktopTestProfileExit } from '../../../../scripts/tasks/dev.mjs';

test('accepts only zero or the observed Windows close status with a matching normal-close marker', () => {
  const runId = 'd378edab-4611-48e7-a769-1d8fd7f5e85f';

  assert.doesNotThrow(() => assertDesktopTestProfileExit(0, runId, runId));
  assert.doesNotThrow(() => assertDesktopTestProfileExit(0xffff_ffff, runId, runId));
});

test('rejects crashes and unmarked exits even when the exit status is zero', () => {
  const runId = 'd378edab-4611-48e7-a769-1d8fd7f5e85f';

  assert.throws(
    () => assertDesktopTestProfileExit(1, null, runId),
    /exited with code 1 without a confirmed normal window close/u,
  );
  assert.throws(
    () => assertDesktopTestProfileExit(0, 'stale-run-marker', runId),
    /without a confirmed normal window close/u,
  );
  assert.throws(
    () => assertDesktopTestProfileExit(7, runId, runId),
    /unexpected code 7/u,
  );
});
