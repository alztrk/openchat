import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type SidebarSectionId = 'projects' | 'folders' | 'pinned' | 'chats' | 'archived' | 'trash';
export type SidebarSortMode = 'updatedAt' | 'createdAt' | 'alphabetical' | 'manual';
export type SidebarViewMode = 'compact' | 'detailed';

export interface CustomSidebarFolder {
  id: string;
  name: string;
  conversationIds: string[];
  createdAt: number;
}

export const SIDEBAR_DEFAULT_WIDTH = 280;
export const SIDEBAR_MIN_WIDTH = 220;
export const SIDEBAR_MAX_WIDTH = 420;
export const SIDEBAR_TRASH_RETENTION_MS = 30 * 86_400_000;
export const SPLIT_VIEW_DEFAULT_RATIO = 50;
export const SPLIT_VIEW_MIN_RATIO = 30;
export const SPLIT_VIEW_MAX_RATIO = 70;
export const MAX_OPEN_CONVERSATION_TABS = 12;

let activeSidebarDraggingConversationId: string | null = null;

export function setSidebarDraggingConversationId(id: string | null): void {
  activeSidebarDraggingConversationId = id;
}

export function getSidebarDraggingConversationId(): string | null {
  return activeSidebarDraggingConversationId;
}

const collapsedSectionsKey = 'chat.collapsed_sidebar_sections';
const sidebarWidthKey = 'chat.sidebar_width';
const sidebarSortModeKey = 'chat.sidebar_sort_mode';
const sidebarViewModeKey = 'chat.sidebar_view_mode';
const sidebarManualOrderKey = 'chat.sidebar_manual_order';
const customFoldersKey = 'chat.custom_sidebar_folders';
const trashedConversationsKey = 'chat.trashed_conversations';
const openConversationTabsKey = 'chat.open_conversation_tabs';
const splitViewRatioKey = 'chat.split_view_ratio';

const supportedSectionSet: ReadonlySet<string> = new Set<SidebarSectionId>([
  'projects',
  'folders',
  'pinned',
  'chats',
  'archived',
  'trash',
]);

export interface SidebarPreferencesReadResult {
  collapsedSections: ReadonlySet<SidebarSectionId>;
  error: Error | null;
}

export function readCollapsedSidebarSections(): SidebarPreferencesReadResult {
  try {
    const encoded = unifiedStorage.getItem(collapsedSectionsKey);
    if (encoded === null) return { collapsedSections: new Set(), error: null };
    const values: unknown = JSON.parse(encoded);
    if (!Array.isArray(values)) {
      throw new Error('The stored sidebar section preferences are invalid.');
    }
    const sections = values.filter(isSidebarSectionId);
    if (sections.length !== values.length) {
      throw new Error('The stored sidebar section preferences are invalid.');
    }
    return { collapsedSections: new Set(sections), error: null };
  } catch (cause) {
    return {
      collapsedSections: new Set(),
      error: new Error('Sidebar section preferences could not be read.', { cause }),
    };
  }
}

export function writeCollapsedSidebarSections(sections: ReadonlySet<SidebarSectionId>): void {
  unifiedStorage.setItem(collapsedSectionsKey, JSON.stringify([...sections]));
}

export function clampSidebarWidth(width: number): number {
  if (!Number.isFinite(width)) return SIDEBAR_DEFAULT_WIDTH;
  return Math.round(Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, width)));
}

export function readSidebarWidth(): number {
  const encoded = unifiedStorage.getItem(sidebarWidthKey);
  if (encoded === null) return SIDEBAR_DEFAULT_WIDTH;
  const parsed = Number(encoded);
  return clampSidebarWidth(parsed);
}

export function writeSidebarWidth(width: number): void {
  unifiedStorage.setItem(sidebarWidthKey, String(clampSidebarWidth(width)));
}

export function readSidebarSortMode(): SidebarSortMode {
  const value = unifiedStorage.getItem(sidebarSortModeKey);
  if (value === 'updatedAt' || value === 'createdAt' || value === 'alphabetical' || value === 'manual') {
    return value;
  }
  return 'updatedAt';
}

export function writeSidebarSortMode(mode: SidebarSortMode): void {
  unifiedStorage.setItem(sidebarSortModeKey, mode);
}

export function readSidebarViewMode(): SidebarViewMode {
  const value = unifiedStorage.getItem(sidebarViewModeKey);
  if (value === 'compact' || value === 'detailed') {
    return value;
  }
  return 'compact';
}

export function writeSidebarViewMode(mode: SidebarViewMode): void {
  unifiedStorage.setItem(sidebarViewModeKey, mode);
}

export function readSidebarManualOrder(): string[] {
  try {
    const encoded = unifiedStorage.getItem(sidebarManualOrderKey);
    if (!encoded) return [];
    const parsed: unknown = JSON.parse(encoded);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((id): id is string => typeof id === 'string' && id.length > 0);
  } catch {
    return [];
  }
}

export function writeSidebarManualOrder(order: readonly string[]): void {
  unifiedStorage.setItem(sidebarManualOrderKey, JSON.stringify([...new Set(order)]));
}

export function readCustomSidebarFolders(): CustomSidebarFolder[] {
  try {
    const encoded = unifiedStorage.getItem(customFoldersKey);
    if (!encoded) return [];
    const parsed: unknown = JSON.parse(encoded);
    if (!Array.isArray(parsed)) return [];
    const validFolders: CustomSidebarFolder[] = [];
    for (const item of parsed) {
      if (
        typeof item === 'object' &&
        item !== null &&
        typeof (item as { id?: unknown }).id === 'string' &&
        typeof (item as { name?: unknown }).name === 'string' &&
        Array.isArray((item as { conversationIds?: unknown }).conversationIds)
      ) {
        const id = (item as { id: string }).id.trim();
        const name = (item as { name: string }).name.trim();
        const conversationIds = (item as { conversationIds: unknown[] }).conversationIds.filter(
          (cid): cid is string => typeof cid === 'string' && cid.length > 0,
        );
        const createdAt =
          typeof (item as { createdAt?: unknown }).createdAt === 'number'
            ? (item as { createdAt: number }).createdAt
            : Date.now();
        if (id && name) {
          validFolders.push({
            id,
            name,
            conversationIds: [...new Set(conversationIds)],
            createdAt,
          });
        }
      }
    }
    return validFolders;
  } catch {
    return [];
  }
}

export function writeCustomSidebarFolders(folders: readonly CustomSidebarFolder[]): void {
  unifiedStorage.setItem(customFoldersKey, JSON.stringify(folders));
}

export function readTrashedConversations(now = Date.now()): Record<string, number> {
  try {
    const encoded = unifiedStorage.getItem(trashedConversationsKey);
    if (!encoded) return {};
    const parsed: unknown = JSON.parse(encoded);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return {};
    const activeTrash: Record<string, number> = {};
    let pruned = false;
    for (const [id, deletedAt] of Object.entries(parsed)) {
      if (typeof deletedAt === 'number' && Number.isFinite(deletedAt)) {
        if (now - deletedAt <= SIDEBAR_TRASH_RETENTION_MS) {
          activeTrash[id] = deletedAt;
        } else {
          pruned = true;
        }
      }
    }
    if (pruned) {
      writeTrashedConversations(activeTrash);
    }
    return activeTrash;
  } catch {
    return {};
  }
}

export function writeTrashedConversations(trashed: Record<string, number>): void {
  unifiedStorage.setItem(trashedConversationsKey, JSON.stringify(trashed));
}

export function clampSplitViewRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) return SPLIT_VIEW_DEFAULT_RATIO;
  return Math.round(Math.min(SPLIT_VIEW_MAX_RATIO, Math.max(SPLIT_VIEW_MIN_RATIO, ratio)));
}

export function readSplitViewRatio(): number {
  const encoded = unifiedStorage.getItem(splitViewRatioKey);
  if (encoded === null) return SPLIT_VIEW_DEFAULT_RATIO;
  return clampSplitViewRatio(Number(encoded));
}

export function writeSplitViewRatio(ratio: number): void {
  unifiedStorage.setItem(splitViewRatioKey, String(clampSplitViewRatio(ratio)));
}

export function readOpenConversationTabs(): string[] {
  try {
    const encoded = unifiedStorage.getItem(openConversationTabsKey);
    if (!encoded) return [];
    const parsed: unknown = JSON.parse(encoded);
    if (!Array.isArray(parsed)) return [];
    return [...new Set(parsed.filter((id): id is string => typeof id === 'string' && id.length > 0))].slice(
      0,
      MAX_OPEN_CONVERSATION_TABS,
    );
  } catch {
    return [];
  }
}

export function writeOpenConversationTabs(tabIds: readonly string[]): void {
  const unique = [...new Set(tabIds.filter((id) => typeof id === 'string' && id.length > 0))].slice(
    0,
    MAX_OPEN_CONVERSATION_TABS,
  );
  unifiedStorage.setItem(openConversationTabsKey, JSON.stringify(unique));
}

export function resolveWorkspaceDropZone(
  clientX: number,
  bounds: { left: number; width: number },
): 'primary' | 'split' {
  if (!Number.isFinite(clientX) || !Number.isFinite(bounds.width) || bounds.width <= 0) {
    return 'primary';
  }
  const relativeX = (clientX - bounds.left) / bounds.width;
  return relativeX >= 0.52 ? 'split' : 'primary';
}

export function sortSidebarConversations<T extends { id: string; title: string; updatedAt: number; createdAt: number }>(
  conversations: readonly T[],
  sortMode: SidebarSortMode,
  manualOrder: readonly string[] = [],
): T[] {
  const copy = [...conversations];
  if (sortMode === 'createdAt') {
    return copy.sort((a, b) => b.createdAt - a.createdAt || b.updatedAt - a.updatedAt);
  }
  if (sortMode === 'alphabetical') {
    return copy.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
  }
  if (sortMode === 'manual') {
    const indexMap = new Map<string, number>();
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

function isSidebarSectionId(value: unknown): value is SidebarSectionId {
  return typeof value === 'string' && supportedSectionSet.has(value);
}
