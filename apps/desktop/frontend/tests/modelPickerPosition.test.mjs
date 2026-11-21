import assert from 'node:assert/strict';
import test from 'node:test';

import { calculateModelPickerPosition } from '../src/features/chat/components/modelPickerPosition.ts';

test('positions the picker above when the trigger is close to the viewport bottom', () => {
  const position = calculateModelPickerPosition(
    { top: 500, right: 240, bottom: 540, left: 200 },
    { width: 520, height: 420 },
    { width: 1024, height: 768 },
  );

  assert.deepEqual(position, {
    top: 72,
    left: 200,
    width: 520,
    maxHeight: 420,
    placement: 'above',
    alignment: 'left',
  });
});

test('positions the picker below and right-aligns it when the trigger is near the viewport edge', () => {
  const position = calculateModelPickerPosition(
    { top: 20, right: 840, bottom: 60, left: 800 },
    { width: 520, height: 420 },
    { width: 1024, height: 768 },
  );

  assert.deepEqual(position, {
    top: 68,
    left: 320,
    width: 520,
    maxHeight: 420,
    placement: 'below',
    alignment: 'right',
  });
});

test('shrinks the picker to the available viewport width and height', () => {
  const position = calculateModelPickerPosition(
    { top: 120, right: 42, bottom: 160, left: 2 },
    { width: 520, height: 420 },
    { width: 360, height: 260 },
  );

  assert.deepEqual(position, {
    top: 8,
    left: 8,
    width: 344,
    maxHeight: 104,
    placement: 'above',
    alignment: 'left',
  });
});
