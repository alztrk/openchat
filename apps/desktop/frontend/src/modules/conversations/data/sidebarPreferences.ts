import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { isObject } from '@/shared/utils/json';

export type SidebarSectionId = 'projects' | 'folders' | 'pinned' | 'chats' | 'archived';
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
export const WORKSPACE_PANEL_DEFAULT_RATIO = 50;
export const WORKSPACE_PANEL_MIN_RATIO = 30;
export const WORKSPACE_PANEL_MAX_RATIO = 70;

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
const legacyTrashedConversationsKey = 'chat.trashed_conversations';
const openConversationTabsKey = 'chat.open_conversation_tabs';
// Keep the stored key so the former split-view width preference carries over to the panel layout.
const workspacePanelRatioKey = 'chat.split_view_ratio';

const supportedSectionSet: ReadonlySet<string> = new Set<SidebarSectionId>([
  'projects',
  'folders',
  'pinned',
  'chats',
  'archived',
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
    const legacyTrashSections = values.filter((section) => section === 'trash');
    if (sections.length + legacyTrashSections.length !== values.length) {
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
      if (!isObject(item)) continue;
      const { id, name, conversationIds, createdAt } = item;
      if (typeof id !== 'string' || typeof name !== 'string' || !Array.isArray(conversationIds)) continue;
      const trimmedId = id.trim();
      const trimmedName = name.trim();
      const validConversationIds = conversationIds.filter(
        (conversationId): conversationId is string => typeof conversationId === 'string' && conversationId.length > 0,
      );
      const folderCreatedAt = typeof createdAt === 'number' ? createdAt : Date.now();
      if (trimmedId && trimmedName) {
        validFolders.push({
          id: trimmedId,
          name: trimmedName,
          conversationIds: [...new Set(validConversationIds)],
          createdAt: folderCreatedAt,
        });
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

export function readLegacyHiddenConversationIds(): ReadonlySet<string> {
  try {
    const encoded = unifiedStorage.getItem(legacyTrashedConversationsKey);
    if (!encoded) return new Set();
    const parsed: unknown = JSON.parse(encoded);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return new Set();
    return new Set(
      Object.entries(parsed)
        .filter(([, deletedAt]) => typeof deletedAt === 'number' && Number.isFinite(deletedAt))
        .map(([id]) => id),
    );
  } catch {
    return new Set();
  }
}

export function clampWorkspacePanelRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) return WORKSPACE_PANEL_DEFAULT_RATIO;
  return Math.round(Math.min(WORKSPACE_PANEL_MAX_RATIO, Math.max(WORKSPACE_PANEL_MIN_RATIO, ratio)));
}

export function readWorkspacePanelRatio(): number {
  const encoded = unifiedStorage.getItem(workspacePanelRatioKey);
  if (encoded === null) return WORKSPACE_PANEL_DEFAULT_RATIO;
  return clampWorkspacePanelRatio(Number(encoded));
}

export function writeWorkspacePanelRatio(ratio: number): void {
  unifiedStorage.setItem(workspacePanelRatioKey, String(clampWorkspacePanelRatio(ratio)));
}

export function readOpenConversationTabs(): string[] {
  try {
    const encoded = unifiedStorage.getItem(openConversationTabsKey);
    if (!encoded) return [];
    const parsed: unknown = JSON.parse(encoded);
    if (!Array.isArray(parsed)) return [];
    return [...new Set(parsed.filter((id): id is string => typeof id === 'string' && id.length > 0))];
  } catch {
    return [];
  }
}

export function writeOpenConversationTabs(tabIds: readonly string[]): void {
  const unique = [...new Set(tabIds.filter((id) => typeof id === 'string' && id.length > 0))];
  unifiedStorage.setItem(openConversationTabsKey, JSON.stringify(unique));
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
    manualOrder.forEach((id, idx) => {
      indexMap.set(id, idx);
    });
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
