import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type SidebarSectionId = 'projects' | 'pinned' | 'chats' | 'archived';

const collapsedSectionsKey = 'chat.collapsed_sidebar_sections';
const supportedSectionSet: ReadonlySet<string> = new Set<SidebarSectionId>(['projects', 'pinned', 'chats', 'archived']);

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

function isSidebarSectionId(value: unknown): value is SidebarSectionId {
  return typeof value === 'string' && supportedSectionSet.has(value);
}
