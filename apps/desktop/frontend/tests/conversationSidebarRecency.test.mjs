import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendRoot = path.resolve(__dirname, '..');

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

function sortSidebarConversations(conversations, sortMode, manualOrder = []) {
  const copy = [...conversations];
  if (sortMode === 'createdAt') {
    return copy.sort((a, b) => b.createdAt - a.createdAt || b.updatedAt - a.updatedAt);
  }
  if (sortMode === 'alphabetical') {
    return copy.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
  }
  if (sortMode === 'manual') {
    const indexMap = new Map();
    manualOrder.forEach((id, idx) => indexMap.set(id, idx));
    return copy.sort((a, b) => {
      const idxA = indexMap.get(a.id);
      const idxB = indexMap.get(b.id);
      if (idxA !== undefined && idxB !== undefined) return idxA - idxB;
      if (idxA !== undefined) return -1;
      if (idxB !== undefined) return 1;
      return b.updatedAt - a.updatedAt;
    });
  }
  return copy.sort((a, b) => b.updatedAt - a.updatedAt);
}

function clampSidebarWidth(width) {
  if (!Number.isFinite(width)) return 280;
  return Math.round(Math.min(420, Math.max(220, width)));
}

function clampSplitViewRatio(ratio) {
  if (!Number.isFinite(ratio)) return 0.5;
  return Number(Math.min(0.7, Math.max(0.3, ratio)).toFixed(3));
}

function resolveWorkspaceDropZone(clientX, rectLeft, rectWidth) {
  if (!Number.isFinite(clientX) || !Number.isFinite(rectLeft) || !Number.isFinite(rectWidth) || rectWidth <= 0) {
    return 'primary';
  }
  const relativeX = (clientX - rectLeft) / rectWidth;
  return relativeX >= 0.5 ? 'split' : 'primary';
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

test('sortSidebarConversations supports updatedAt, createdAt, alphabetical, and manual modes', () => {
  const items = [
    { id: '1', title: 'Zebra', createdAt: 100, updatedAt: 300 },
    { id: '2', title: 'Alpha', createdAt: 300, updatedAt: 100 },
    { id: '3', title: 'Beta', createdAt: 200, updatedAt: 200 },
  ];

  assert.deepEqual(
    sortSidebarConversations(items, 'updatedAt').map((i) => i.id),
    ['1', '3', '2'],
  );
  assert.deepEqual(
    sortSidebarConversations(items, 'createdAt').map((i) => i.id),
    ['2', '3', '1'],
  );
  assert.deepEqual(
    sortSidebarConversations(items, 'alphabetical').map((i) => i.id),
    ['2', '3', '1'],
  );
  assert.deepEqual(
    sortSidebarConversations(items, 'manual', ['3', '1', '2']).map((i) => i.id),
    ['3', '1', '2'],
  );
});

test('clampSidebarWidth bounds sidebar width between 220 and 420 with 280 default', () => {
  assert.equal(clampSidebarWidth(150), 220);
  assert.equal(clampSidebarWidth(310), 310);
  assert.equal(clampSidebarWidth(600), 420);
  assert.equal(clampSidebarWidth(Number.NaN), 280);
});

test('clampSplitViewRatio bounds split ratio between 0.3 and 0.7 and resolveWorkspaceDropZone detects left/right drop targets', () => {
  assert.equal(clampSplitViewRatio(0.1), 0.3);
  assert.equal(clampSplitViewRatio(0.55), 0.55);
  assert.equal(clampSplitViewRatio(0.95), 0.7);
  assert.equal(clampSplitViewRatio(Number.NaN), 0.5);

  assert.equal(resolveWorkspaceDropZone(300, 100, 800), 'primary');
  assert.equal(resolveWorkspaceDropZone(600, 100, 800), 'split');
  assert.equal(resolveWorkspaceDropZone(500, 100, 800), 'split');
});

test('animations.css and index.css define AI working ring spinner, left-to-right title shimmer, and virtual row classes', () => {
  const animationsCss = fs.readFileSync(
    path.join(frontendRoot, 'src', 'design-system', 'styles', 'animations.css'),
    'utf-8',
  );
  const indexCss = fs.readFileSync(path.join(frontendRoot, 'src', 'index.css'), 'utf-8');

  assert.match(animationsCss, /@keyframes openchat-ai-ring-spin/);
  assert.match(animationsCss, /@keyframes openchat-ai-ring-dash/);
  assert.match(animationsCss, /@keyframes openchat-title-shimmer-flow/);
  assert.match(animationsCss, /\.openchat-ai-ring-spinner/);
  assert.match(animationsCss, /\.openchat-ai-ring-arc/);
  assert.match(animationsCss, /\.openchat-title-shimmer/);
  assert.match(indexCss, /\.conversation-sidebar-virtual-row/);
});

test('all 5 locales define sidebar roadmap and AI working indicator keys', () => {
  const locales = ['en', 'tr', 'de', 'es', 'fr'];
  const requiredKeys = [
    'aiWorkingIndicatorLabel',
    'duplicateConversation',
    'conversationDuplicateSuffix',
    'conversationDuplicated',
    'conversationDuplicateFailed',
    'customFolders',
    'createCustomFolder',
    'customFolderNamePlaceholder',
    'renameCustomFolder',
    'deleteCustomFolder',
    'moveConversationToFolder',
    'removeFromFolder',
    'sidebarViewAndSortOptions',
    'sidebarViewModeLabel',
    'sidebarViewCompact',
    'sidebarViewDetailed',
    'sidebarSortByLabel',
    'sidebarSortUpdatedAt',
    'sidebarSortCreatedAt',
    'sidebarSortAlphabetical',
    'sidebarSortManual',
    'recentlyDeletedChats',
    'conversationMovedToTrash',
    'conversationRestoredFromTrash',
    'deleteConversationPermanently',
    'loadMoreChats',
    'resizeSidebar',
    'openInSplitView',
    'openChatTabsLabel',
    'closeChatTab',
    'splitViewToggle',
    'closeSplitView',
    'swapSplitPanes',
    'focusSplitConversation',
    'splitViewPaneLabel',
    'splitViewEmptyHint',
    'splitViewEmptyConversation',
    'splitViewReplyPlaceholder',
    'resizeSplitView',
    'dropConversationPrimaryTitle',
    'dropConversationPrimaryHint',
    'dropConversationSplitTitle',
    'dropConversationSplitHint',
    'openRightPanel',
    'closeRightPanel',
    'toggleRightPanel',
  ];

  for (const locale of locales) {
    const filePath = path.join(frontendRoot, 'src', 'localization', 'locales', `${locale}.json`);
    const content = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
    for (const key of requiredKeys) {
      assert.ok(
        typeof content[key] === 'string' && content[key].trim().length > 0,
        `Locale '${locale}' is missing required key '${key}'`,
      );
    }
  }
});
