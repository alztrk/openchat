import type { ChatConversation } from '@/modules/conversations/data/chatStorage';

export type RecencyBucketKey = 'today' | 'yesterday' | 'previous7Days' | 'previous30Days' | 'older';

export interface RecencyBucket {
  id: RecencyBucketKey;
  labelKey: RecencyBucketKey;
  conversations: ChatConversation[];
}

export function groupConversationsByRecency(
  conversations: readonly ChatConversation[],
  now = Date.now(),
): RecencyBucket[] {
  const nowDate = new Date(now);
  const todayStart = new Date(nowDate.getFullYear(), nowDate.getMonth(), nowDate.getDate()).getTime();
  const yesterdayStart = todayStart - 86_400_000;
  const sevenDaysAgoStart = todayStart - 6 * 86_400_000;
  const thirtyDaysAgoStart = todayStart - 29 * 86_400_000;

  const today: ChatConversation[] = [];
  const yesterday: ChatConversation[] = [];
  const previous7Days: ChatConversation[] = [];
  const previous30Days: ChatConversation[] = [];
  const older: ChatConversation[] = [];

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

  const buckets: RecencyBucket[] = [];
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

export function getFocusableElements(root: HTMLElement): HTMLElement[] {
  const selector =
    'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';
  return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
    (element) => element.getClientRects().length > 0,
  );
}

export function normalizeProjectPath(path: string): string {
  return path.trim().replaceAll('/', '\\').replace(/\\+$/, '').toLowerCase();
}
