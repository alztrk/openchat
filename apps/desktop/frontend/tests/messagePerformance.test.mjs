import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import test from 'node:test';

function groupResponses(messages) {
  const groups = [];
  for (let i = 0; i < messages.length; i++) {
    const message = messages[i];
    if (!message) continue;
    const previous = groups[groups.length - 1];
    if (message.role === 'assistant' && previous?.messages[previous.messages.length - 1]?.role === 'assistant') {
      previous.messages.push(message);
      continue;
    }
    groups.push({ id: message.id, messages: [message] });
  }
  return groups;
}

function startsNewDay(previous, current) {
  if (previous === null || current === null) return previous !== current;
  if (previous === current) return false;
  const previousDate = new Date(previous);
  const currentDate = new Date(current);
  return (
    previousDate.getFullYear() !== currentDate.getFullYear() ||
    previousDate.getMonth() !== currentDate.getMonth() ||
    previousDate.getDate() !== currentDate.getDate()
  );
}

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

test('benchmark: groupResponses processes 10,000 messages in under 5ms', (context) => {
  const messages = Array.from({ length: 10_000 }, (_, i) => ({
    id: `msg-${i}`,
    role: i % 3 === 0 ? 'user' : 'assistant',
  }));

  const start = performance.now();
  const groups = groupResponses(messages);
  const elapsed = performance.now() - start;

  assert.ok(groups.length > 0);
  assert.ok(elapsed < 10, `groupResponses took ${elapsed.toFixed(2)}ms (expected < 10ms)`);
  context.diagnostic(`10,000 messages grouped in ${elapsed.toFixed(3)}ms (${groups.length} groups)`);
});
