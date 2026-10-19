import assert from 'node:assert/strict';
import test from 'node:test';

function groupConversationsByRecency(conversations, now = Date.now()) {
  const nowDate = new Date(now);
  const todayStart = new Date(nowDate.getFullYear(), nowDate.getMonth(), nowDate.getDate()).getTime();
  const yesterdayStart = todayStart - 86_400_000;
  const sevenDaysAgoStart = todayStart - 6 * 86_400_000;
  const thirtyDaysAgoStart = todayStart - 29 * 86_400_000;

  const today = [];
  const yesterday = [];
  const previous7Days = [];
  const previous30Days = [];
  const older = [];

  const sorted = [...conversations].sort((a, b) => b.updatedAt - a.updatedAt);

  for (const item of sorted) {
    if (item.updatedAt >= todayStart) {
      today.push(item);
    } else if (item.updatedAt >= yesterdayStart) {
      yesterday.push(item);
    } else if (item.updatedAt >= sevenDaysAgoStart) {
      previous7Days.push(item);
    } else if (item.updatedAt >= thirtyDaysAgoStart) {
      previous30Days.push(item);
    } else {
      older.push(item);
    }
  }

  const buckets = [];
  if (today.length > 0) buckets.push({ id: 'today', labelKey: 'today', conversations: today });
  if (yesterday.length > 0) buckets.push({ id: 'yesterday', labelKey: 'yesterday', conversations: yesterday });
  if (previous7Days.length > 0) {
    buckets.push({ id: 'previous7Days', labelKey: 'previous7Days', conversations: previous7Days });
  }
  if (previous30Days.length > 0) {
    buckets.push({ id: 'previous30Days', labelKey: 'previous30Days', conversations: previous30Days });
  }
  if (older.length > 0) buckets.push({ id: 'older', labelKey: 'older', conversations: older });

  return buckets;
}

test('groupConversationsByRecency groups chats into today, yesterday, previous 7 days, previous 30 days and older', () => {
  const fixedNow = new Date('2026-10-20T12:00:00Z').getTime();
  const todayChat = { id: 'c1', title: 'Today Chat', updatedAt: fixedNow - 3600_000 };
  const yesterdayChat = { id: 'c2', title: 'Yesterday Chat', updatedAt: fixedNow - 90_000_000 };
  const threeDaysAgoChat = { id: 'c3', title: '3 Days Ago Chat', updatedAt: fixedNow - 3 * 86_400_000 };
  const twoWeeksAgoChat = { id: 'c4', title: '2 Weeks Ago Chat', updatedAt: fixedNow - 14 * 86_400_000 };
  const twoMonthsAgoChat = { id: 'c5', title: '2 Months Ago Chat', updatedAt: fixedNow - 60 * 86_400_000 };

  const buckets = groupConversationsByRecency(
    [twoMonthsAgoChat, todayChat, twoWeeksAgoChat, yesterdayChat, threeDaysAgoChat],
    fixedNow,
  );

  assert.equal(buckets.length, 5);
  assert.equal(buckets[0].id, 'today');
  assert.equal(buckets[0].conversations.length, 1);
  assert.equal(buckets[0].conversations[0].id, 'c1');

  assert.equal(buckets[1].id, 'yesterday');
  assert.equal(buckets[1].conversations.length, 1);
  assert.equal(buckets[1].conversations[0].id, 'c2');

  assert.equal(buckets[2].id, 'previous7Days');
  assert.equal(buckets[2].conversations.length, 1);
  assert.equal(buckets[2].conversations[0].id, 'c3');

  assert.equal(buckets[3].id, 'previous30Days');
  assert.equal(buckets[3].conversations.length, 1);
  assert.equal(buckets[3].conversations[0].id, 'c4');

  assert.equal(buckets[4].id, 'older');
  assert.equal(buckets[4].conversations.length, 1);
  assert.equal(buckets[4].conversations[0].id, 'c5');
});

test('groupConversationsByRecency omits empty buckets', () => {
  const fixedNow = new Date('2026-10-20T12:00:00Z').getTime();
  const todayChat = { id: 'c1', title: 'Today Chat', updatedAt: fixedNow - 1000 };

  const buckets = groupConversationsByRecency([todayChat], fixedNow);
  assert.equal(buckets.length, 1);
  assert.equal(buckets[0].id, 'today');
});
