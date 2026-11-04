import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import test from 'node:test';

import { groupResponses, startsNewDay } from '../src/features/chat/data/messageGrouping.ts';

test('groupResponses groups consecutive assistant responses correctly', () => {
  const messages = [
    { id: '1', role: 'user' },
    { id: '2', role: 'assistant' },
    { id: '3', role: 'assistant' },
    { id: '4', role: 'user' },
    { id: '5', role: 'assistant' },
  ];
  const groups = groupResponses(messages);
  assert.equal(groups.length, 4);
  assert.equal(groups[0].messages.length, 1);
  assert.equal(groups[1].messages.length, 2);
  assert.equal(groups[2].messages.length, 1);
  assert.equal(groups[3].messages.length, 1);
});

test('startsNewDay correctly detects same day vs new day boundaries', () => {
  const now = 1776518400000;
  assert.equal(startsNewDay(now, now), false);
  assert.equal(startsNewDay(null, now), true);
  assert.equal(startsNewDay(now, null), true);
  assert.equal(startsNewDay(null, null), false);
  // +10 minutes
  assert.equal(startsNewDay(now, now + 600_000), false);
  // +25 hours
  assert.equal(startsNewDay(now, now + 90_000_000), true);
});

test('benchmark diagnostic: production grouping handles 10,000 synthetic messages', (context) => {
  const messages = Array.from({ length: 10_000 }, (_, i) => ({
    id: `msg-${i}`,
    role: i % 3 === 0 ? 'user' : 'assistant',
  }));

  const start = performance.now();
  const groups = groupResponses(messages);
  const elapsed = performance.now() - start;

  assert.equal(groups.length, 6_667);
  context.diagnostic(
    `10,000 synthetic messages grouped by production code in ${elapsed.toFixed(3)}ms (${groups.length} groups); this does not measure React rendering or scrolling.`,
  );
});
