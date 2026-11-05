import assert from 'node:assert/strict';
import process from 'node:process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test, { after, afterEach } from 'node:test';
import { Window } from 'happy-dom';
import { createInstance } from 'i18next';
import { createServer } from 'vite';

const vite = await createServer({
  appType: 'custom',
  configFile: path.resolve('vite.config.ts'),
  mode: 'development',
  plugins: [
    {
      name: 'openchat-test-attachment-library-data',
      enforce: 'pre',
      resolveId(source, importer) {
        const normalizedSource = source.replaceAll('\\', '/');
        if (
          (normalizedSource === '@features/chat/data' || normalizedSource.endsWith('/src/features/chat/data')) &&
          importer?.replaceAll('\\', '/').endsWith('/src/features/chat/components/AttachmentLibraryDialog.tsx')
        ) {
          return '\0openchat-test-attachment-library-data';
        }
        if (
          normalizedSource.endsWith('/src/features/app-settings/data/appPreferences') &&
          importer?.replaceAll('\\', '/').endsWith('/src/features/app-settings/components/AppWindowSettingsSection.tsx')
        ) {
          return '\0openchat-test-app-preferences';
        }
        const normalizedImporter = importer?.replaceAll('\\', '/') ?? '';
        if (
          normalizedSource.endsWith('/src/features/chat/hooks/useConversationSidebarActions') &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-sidebar-actions';
        }
        if (
          normalizedSource.endsWith('/src/features/chat/state') &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-chat-store';
        }
        if (
          normalizedSource.endsWith('/src/shared/lib/preferences/sidebarPreferences') &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-sidebar-preferences';
        }
        if (
          (normalizedSource === '@shared/lib/platform/storage/unifiedStorage' ||
            normalizedSource.endsWith('/src/shared/lib/platform/storage/unifiedStorage')) &&
          normalizedImporter.endsWith('/src/shared/lib/preferences/sidebarPreferences.ts')
        ) {
          return '\0openchat-test-sidebar-preference-storage';
        }
        if (
          normalizedSource.endsWith('/src/shared/lib/preferences/assistantProfilesStore') &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-assistant-profiles';
        }
        if (
          normalizedSource === './ConversationSidebarRow' &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-sidebar-row';
        }
        if (
          normalizedSource === './ConversationSidebarToolbar' &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-sidebar-toolbar';
        }
        if (
          normalizedSource === './ConversationSidebarEditDialog' &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-sidebar-edit-dialog';
        }
        if (
          normalizedSource.endsWith('/src/features/projects/components') &&
          normalizedImporter.endsWith('/src/features/chat/components/ConversationSidebar.tsx')
        ) {
          return '\0openchat-test-project-sidebar';
        }
        if (
          normalizedSource.endsWith('/src/features/model-catalog/stores/mcpStore') &&
          normalizedImporter.endsWith('/src/features/model-catalog/components/McpCatalogTab.tsx')
        ) {
          return '\0openchat-test-mcp-store';
        }
        if (
          (normalizedSource === '@features/chat/data' || normalizedSource.endsWith('/src/features/chat/data')) &&
          normalizedImporter.includes('TitleBarSearch.tsx')
        ) {
          return '\0openchat-test-titlebar-search-data';
        }
        if (
          (normalizedSource === '@features/chat/state' || normalizedSource.endsWith('/src/features/chat/state')) &&
          normalizedImporter.includes('TitleBarSearch.tsx')
        ) {
          return '\0openchat-test-titlebar-search-store';
        }
        if (
          (normalizedSource === '@features/chat/components/HistorySearchDialog' ||
            normalizedSource.endsWith('/src/features/chat/components/HistorySearchDialog')) &&
          normalizedImporter.includes('TitleBarSearch.tsx')
        ) {
          return '\0openchat-test-titlebar-search-history-dialog';
        }
        if (
          (normalizedSource === '@shared/lib/platform/storage/unifiedStorage' ||
            normalizedSource.endsWith('/src/shared/lib/platform/storage/unifiedStorage')) &&
          normalizedImporter.includes('TitleBarSearch.tsx')
        ) {
          return '\0openchat-test-titlebar-search-storage';
        }
        if (
          normalizedSource === './recentSearches' &&
          normalizedImporter.includes('TitleBarSearch.tsx')
        ) {
          return '\0openchat-test-titlebar-search-recents';
        }
        return null;
      },
      load(id) {
        if (id === '\0openchat-test-attachment-library-data') {
          return `
            import process from 'node:process';
            export function listConversationAttachments(...args) {
              return process.__openchatAttachmentLibraryHarness.list(...args);
            }
          `;
        }
        if (id === '\0openchat-test-app-preferences') {
          return `
            import process from 'node:process';
            export function isAppSurface(value) {
              return ['chats', 'design', 'models', 'security', 'settings'].includes(value);
            }
            export function readCloseWindowBehavior() { return 'exit'; }
            export function readLaunchOnStartup() { return false; }
            export function readStartupSurfacePreference() {
              return process.__openchatAppPreferencesHarness.startupSurface;
            }
            export function writeCloseWindowBehavior() { return Promise.resolve(); }
            export function writeLaunchOnStartup() { return Promise.resolve(); }
            export function writeStartupSurfacePreference(value) {
              return process.__openchatAppPreferencesHarness.writeStartupSurface(value);
            }
          `;
        }
        if (id === '\0openchat-test-sidebar-actions') {
          return `
            import process from 'node:process';
            export function useConversationSidebarActions() {
              return process.__openchatSidebarHarness.actions;
            }
          `;
        }
        if (id === '\0openchat-test-chat-store') {
          return `
            import process from 'node:process';
            export function useChatStore(selector) {
              return selector(process.__openchatSidebarHarness.chatState);
            }
            useChatStore.getState = () => process.__openchatSidebarHarness.chatState;
          `;
        }
        if (id === '\0openchat-test-sidebar-preferences') {
          return `
            import process from 'node:process';
            export const SIDEBAR_DEFAULT_WIDTH = 280;
            export const SIDEBAR_MAX_WIDTH = 420;
            export const SIDEBAR_MIN_WIDTH = 152;
            export function clampSidebarWidth(width) { return Math.round(Math.min(420, Math.max(152, width))); }
            export function readCollapsedSidebarSections() { return { collapsedSections: new Set(), error: null }; }
            export function readCustomSidebarFolders() {
              return process.__openchatSidebarHarness.customFoldersReadResult;
            }
            export function readLegacyHiddenConversationIds() { return new Set(); }
            export function readSidebarManualOrder() { return []; }
            export function readSidebarSortMode() { return 'updatedAt'; }
            export function readSidebarViewMode() { return 'compact'; }
            export function readSidebarWidth() { return 280; }
            export function readSkipConversationDeleteConfirmation() { return false; }
            export function setSidebarDraggingConversationId() {}
            export function sortSidebarConversations(conversations) { return [...conversations]; }
            export function writeCollapsedSidebarSections() {}
            export function writeCustomSidebarFolders(folders) {
              if (process.__openchatSidebarHarness.customFoldersReadResult.kind !== 'ready') {
                throw new Error('Custom sidebar folder data is not writable.');
              }
              return process.__openchatSidebarHarness.writeCustomFolders(folders);
            }
            export function writeSidebarManualOrder() {}
            export function writeSidebarSortMode() {}
            export function writeSidebarViewMode() {}
            export function writeSidebarWidth() {}
            export function writeSkipConversationDeleteConfirmation() { return Promise.resolve(); }
          `;
        }
        if (id === '\0openchat-test-sidebar-preference-storage') {
          return `
            import process from 'node:process';
            export const unifiedStorage = {
              getItem(key) {
                if (key !== 'chat.custom_sidebar_folders') throw new Error('Unexpected preference key.');
                if (process.__openchatSidebarHarness.preferenceReadFailure) {
                  throw process.__openchatSidebarHarness.preferenceReadFailure;
                }
                return process.__openchatSidebarHarness.preferenceValue;
              },
              setPersistedItem(key, value) {
                if (key !== 'chat.custom_sidebar_folders') throw new Error('Unexpected preference key.');
                process.__openchatSidebarHarness.preferenceWriteCalls.push(value);
                process.__openchatSidebarHarness.preferenceValue = value;
                return Promise.resolve();
              },
              setItem() {},
            };
          `;
        }
        if (id === '\0openchat-test-assistant-profiles') {
          return `
            export function useAssistantProfilesStore(selector) {
              return selector({ profiles: [], loadState: 'ready', initialize: () => () => undefined, retry: () => undefined });
            }
          `;
        }
        if (id === '\0openchat-test-sidebar-row') {
          return `
            import React from 'react';
            export function ConversationSidebarRow({ conversation }) {
              return React.createElement('li', null, conversation.title);
            }
          `;
        }
        if (id === '\0openchat-test-sidebar-toolbar') {
          return 'export function ConversationSidebarToolbar() { return null; }';
        }
        if (id === '\0openchat-test-sidebar-edit-dialog') {
          return 'export function ConversationSidebarEditDialog() { return null; }';
        }
        if (id === '\0openchat-test-project-sidebar') {
          return `
            export function ProjectSidebarSection() { return null; }
            export function CreateProjectDialog() { return null; }
          `;
        }
        if (id === '\0openchat-test-mcp-store') {
          return `
            import process from 'node:process';
            export function useMcpStore(selector) {
              return selector(process.__openchatMcpHarness.state);
            }
          `;
        }
        if (id === '\0openchat-test-titlebar-search-data') {
          return `
            export function listConversationAttachments() { return Promise.resolve({ items: [], nextOffset: null }); }
            export function normalizeSearchText(value) { return value.toLowerCase(); }
            export function parseSearchQuery(value) { return { cleanQuery: value.trim() }; }
            export function searchStoredMessages() {
              return Promise.resolve([{
                id: 'message-river',
                conversationId: 'conversation-river',
                role: 'user',
                content: 'A river route',
                createdAt: null,
              }]);
            }
          `;
        }
        if (id === '\0openchat-test-titlebar-search-store') {
          return `
            const chatState = { conversations: [], projects: [] };
            export function useChatStore(selector) {
              return selector(chatState);
            }
          `;
        }
        if (id === '\0openchat-test-titlebar-search-history-dialog') {
          return 'export function HistorySearchDialog() { return null; }';
        }
        if (id === '\0openchat-test-titlebar-search-storage') {
          return 'export const unifiedStorage = { isInitialized: true };';
        }
        if (id === '\0openchat-test-titlebar-search-recents') {
          return `
            import process from 'node:process';
            const harness = () => process.__openchatRecentSearchHarness;
            export const recentSearchLimits = { maximumLength: 256 };
            export function readRecentSearches() { return [...harness().terms]; }
            export async function writeRecentSearches(terms) {
              if (harness().failWrites) throw new Error('test-only storage write failure');
              harness().terms = [...terms];
            }
            export function addRecentSearch(terms, term) {
              const value = term.trim();
              return value ? [value, ...terms.filter((item) => item.toLowerCase() !== value.toLowerCase())].slice(0, 5) : terms;
            }
            export function removeRecentSearch(terms, term) { return terms.filter((item) => item !== term); }
          `;
        }
        return null;
      },
    },
  ],
  server: { middlewareMode: true, hmr: false },
});

const browserWindow = new Window({ url: 'http://localhost/' });
const globals = new Map([
  ['window', browserWindow],
  ['document', browserWindow.document],
  ['DocumentFragment', browserWindow.DocumentFragment],
  ['navigator', browserWindow.navigator],
  ['HTMLElement', browserWindow.HTMLElement],
  ['HTMLFormElement', browserWindow.HTMLFormElement],
  ['Node', browserWindow.Node],
  ['NodeFilter', browserWindow.NodeFilter],
  ['Element', browserWindow.Element],
  ['Event', browserWindow.Event],
  ['CustomEvent', browserWindow.CustomEvent],
  ['MouseEvent', browserWindow.MouseEvent],
  ['KeyboardEvent', browserWindow.KeyboardEvent],
  ['PointerEvent', browserWindow.PointerEvent],
  ['MutationObserver', browserWindow.MutationObserver],
  ['ResizeObserver', browserWindow.ResizeObserver],
  ['getComputedStyle', browserWindow.getComputedStyle.bind(browserWindow)],
  ['requestAnimationFrame', browserWindow.requestAnimationFrame.bind(browserWindow)],
  ['cancelAnimationFrame', browserWindow.cancelAnimationFrame.bind(browserWindow)],
  ['IS_REACT_ACT_ENVIRONMENT', true],
]);
const originalGlobals = new Map();

for (const [name, value] of globals) {
  originalGlobals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { configurable: true, value, writable: true });
}

const React = await import('react');
const { I18nextProvider } = await import('react-i18next');
const { cleanup, fireEvent, render, screen, waitFor, within } = await import('@testing-library/react');
const { ConversationEmptyState } = await vite.ssrLoadModule('/src/features/chat/components/ConversationEmptyState.tsx');
const { AttachmentLibraryDialog } = await vite.ssrLoadModule('/src/features/chat/components/AttachmentLibraryDialog.tsx');
const { NavigationRail } = await vite.ssrLoadModule('/src/widgets/navigation-rail/NavigationRail.tsx');
const { AppWindowSettingsSection } = await vite.ssrLoadModule(
  '/src/features/app-settings/components/AppWindowSettingsSection.tsx',
);
const { ConversationSidebar } = await vite.ssrLoadModule('/src/features/chat/components/ConversationSidebar.tsx');
const sidebarPreferencesModule = await vite.ssrLoadModule('/src/shared/lib/preferences/sidebarPreferences.ts');
const { McpCatalogTab } = await vite.ssrLoadModule('/src/features/model-catalog/components/McpCatalogTab.tsx');
const { TitleBarSearch } = await vite.ssrLoadModule('/src/widgets/title-bar/TitleBarSearch.tsx');

const localeDirectory = path.resolve('src/shared/lib/i18n/locales');
const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'];
const translationsByLocale = new Map();
const i18nByLocale = new Map();
const originalAttachmentLibraryHarness = Object.getOwnPropertyDescriptor(process, '__openchatAttachmentLibraryHarness');
const originalAppPreferencesHarness = Object.getOwnPropertyDescriptor(process, '__openchatAppPreferencesHarness');
const originalSidebarHarness = Object.getOwnPropertyDescriptor(process, '__openchatSidebarHarness');
const originalMcpHarness = Object.getOwnPropertyDescriptor(process, '__openchatMcpHarness');
const originalRecentSearchHarness = Object.getOwnPropertyDescriptor(process, '__openchatRecentSearchHarness');
const attachmentLibraryHarness = {
  list: async () => ({ items: [], nextOffset: null }),
};
const appPreferencesHarness = {
  startupSurface: 'last',
  writeStartupSurface: async (value) => {
    appPreferencesHarness.startupSurface = value;
  },
};
const sidebarHarness = {
  actions: {},
  chatState: {},
  customFoldersReadResult: { kind: 'ready', folders: [] },
  moveCalls: [],
  writeCustomFoldersCalls: [],
  preferenceValue: null,
  preferenceReadFailure: null,
  preferenceWriteCalls: [],
  writeCustomFolders: async (folders) => {
    sidebarHarness.writeCustomFoldersCalls.push(folders);
    sidebarHarness.customFoldersReadResult = { kind: 'ready', folders };
  },
};
const mcpHarness = {
  state: {},
};
const recentSearchHarness = {
  terms: ['river', 'ocean'],
  failWrites: false,
};

Object.defineProperty(process, '__openchatAttachmentLibraryHarness', {
  configurable: true,
  value: attachmentLibraryHarness,
  writable: true,
});
Object.defineProperty(process, '__openchatAppPreferencesHarness', {
  configurable: true,
  value: appPreferencesHarness,
  writable: true,
});
Object.defineProperty(process, '__openchatSidebarHarness', {
  configurable: true,
  value: sidebarHarness,
  writable: true,
});
Object.defineProperty(process, '__openchatMcpHarness', {
  configurable: true,
  value: mcpHarness,
  writable: true,
});
Object.defineProperty(process, '__openchatRecentSearchHarness', {
  configurable: true,
  value: recentSearchHarness,
  writable: true,
});

for (const locale of supportedLocales) {
  const translations = JSON.parse(readFileSync(path.join(localeDirectory, `${locale}.json`), 'utf8'));
  const i18n = createInstance();
  await i18n.init({
    fallbackLng: false,
    lng: locale,
    resources: { [locale]: { translation: translations } },
  });
  translationsByLocale.set(locale, translations);
  i18nByLocale.set(locale, i18n);
}

function localized(locale, child) {
  const i18n = i18nByLocale.get(locale);
  assert.ok(i18n, `missing i18n instance for ${locale}`);
  return React.createElement(I18nextProvider, { i18n }, child);
}

function emptyStateProps(overrides = {}) {
  return {
    hasAvailableModels: true,
    hasConfiguredProvider: true,
    isLoading: false,
    modelCatalogFailed: false,
    modelCatalogStale: false,
    chatGptWorkspaceSelectionRequired: false,
    onOpenConnections: () => undefined,
    onRetryModels: () => undefined,
    selectedModel: false,
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
  attachmentLibraryHarness.list = async () => ({ items: [], nextOffset: null });
  appPreferencesHarness.startupSurface = 'last';
  appPreferencesHarness.writeStartupSurface = async (value) => {
    appPreferencesHarness.startupSurface = value;
  };
  sidebarHarness.actions = {};
  sidebarHarness.chatState = {};
  sidebarHarness.customFoldersReadResult = { kind: 'ready', folders: [] };
  sidebarHarness.moveCalls = [];
  sidebarHarness.writeCustomFoldersCalls = [];
  sidebarHarness.preferenceValue = null;
  sidebarHarness.preferenceReadFailure = null;
  sidebarHarness.preferenceWriteCalls = [];
  sidebarHarness.writeCustomFolders = async (folders) => {
    sidebarHarness.writeCustomFoldersCalls.push(folders);
    sidebarHarness.customFoldersReadResult = { kind: 'ready', folders };
  };
  mcpHarness.state = {};
  recentSearchHarness.terms = ['river', 'ocean'];
  recentSearchHarness.failWrites = false;
});

after(async () => {
  cleanup();
  await vite.close();
  await browserWindow.happyDOM.abort();
  browserWindow.close();

  for (const [name, descriptor] of originalGlobals) {
    if (descriptor) {
      Object.defineProperty(globalThis, name, descriptor);
    } else {
      Reflect.deleteProperty(globalThis, name);
    }
  }

  if (originalAttachmentLibraryHarness) {
    Object.defineProperty(process, '__openchatAttachmentLibraryHarness', originalAttachmentLibraryHarness);
  } else {
    Reflect.deleteProperty(process, '__openchatAttachmentLibraryHarness');
  }
  if (originalAppPreferencesHarness) {
    Object.defineProperty(process, '__openchatAppPreferencesHarness', originalAppPreferencesHarness);
  } else {
    Reflect.deleteProperty(process, '__openchatAppPreferencesHarness');
  }
  if (originalSidebarHarness) {
    Object.defineProperty(process, '__openchatSidebarHarness', originalSidebarHarness);
  } else {
    Reflect.deleteProperty(process, '__openchatSidebarHarness');
  }
  if (originalMcpHarness) {
    Object.defineProperty(process, '__openchatMcpHarness', originalMcpHarness);
  } else {
    Reflect.deleteProperty(process, '__openchatMcpHarness');
  }
  if (originalRecentSearchHarness) {
    Object.defineProperty(process, '__openchatRecentSearchHarness', originalRecentSearchHarness);
  } else {
    Reflect.deleteProperty(process, '__openchatRecentSearchHarness');
  }
});

test('a model catalog with no selected model renders selection guidance, and selection renders the welcome state', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  const view = render(
    localized('en', React.createElement(ConversationEmptyState, emptyStateProps())),
  );
  assert.equal(
    screen.getByRole('heading', { level: 1 }).textContent,
    translations.emptyChatSelectModelTitle,
  );
  assert.equal(screen.getByText(translations.emptyChatSelectModelBody).textContent, translations.emptyChatSelectModelBody);

  view.rerender(
    localized('en', React.createElement(ConversationEmptyState, emptyStateProps({ selectedModel: true }))),
  );
  assert.equal(screen.getByRole('heading', { level: 1 }).textContent, translations.emptyChatWelcomeTitle);
  assert.equal(screen.getByText(translations.emptyChatWelcomeBody).textContent, translations.emptyChatWelcomeBody);
});

test('a failed empty model catalog shows recovery guidance instead of selection guidance', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  render(
    localized(
      'en',
      React.createElement(
        ConversationEmptyState,
        emptyStateProps({ hasAvailableModels: false, modelCatalogFailed: true }),
      ),
    ),
  );

  assert.equal(screen.getByRole('heading', { level: 1 }).textContent, translations.modelsUnavailable);
  assert.equal(screen.getByText(translations.modelCatalogUnavailable).textContent, translations.modelCatalogUnavailable);
  assert.equal(screen.queryByText(translations.emptyChatSelectModelBody), null);
});

test('the empty chat connection action invokes its callback when setup is required', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  let openConnectionsCount = 0;

  render(
    localized(
      'en',
      React.createElement(
        ConversationEmptyState,
        emptyStateProps({
          hasAvailableModels: false,
          hasConfiguredProvider: false,
          onOpenConnections: () => {
            openConnectionsCount += 1;
          },
        }),
      ),
    ),
  );

  fireEvent.click(screen.getByRole('button', { name: translations.connections }));
  assert.equal(openConnectionsCount, 1);
});

test('the attachment library loads available files and closes after a successful selection', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  const item = {
    attachmentId: 'attachment-1',
    conversationId: 'conversation-1',
    conversationTitle: 'A saved conversation',
    createdAt: 1,
    fileName: 'notes.txt',
    generationId: null,
    isAvailable: true,
    isFavorite: false,
    messageId: 'message-1',
    mimeType: 'text/plain',
    model: null,
    projectId: null,
    prompt: null,
    providerRoute: null,
    sizeBytes: 12,
    tags: [],
  };
  const listCalls = [];
  const selectedItems = [];
  const openChanges = [];
  attachmentLibraryHarness.list = async (...args) => {
    listCalls.push(args);
    return { items: [item], nextOffset: null };
  };

  render(
    localized(
      'en',
      React.createElement(AttachmentLibraryDialog, {
        disabled: false,
        onOpenChange: (open) => openChanges.push(open),
        onSelect: async (selected) => {
          selectedItems.push(selected);
          return true;
        },
        open: true,
      }),
    ),
  );

  assert.ok(await screen.findByRole('dialog', { name: translations.attachmentLibraryTitle }));
  assert.ok(await screen.findByText(item.fileName));
  assert.deepEqual(listCalls, [[0, 30, '', 'available']]);

  fireEvent.click(screen.getByRole('button', { name: translations.attachmentLibraryUseFile }));
  await waitFor(() => {
    assert.deepEqual(selectedItems, [item]);
    assert.deepEqual(openChanges, [false]);
  });
});

test('the attachment library keeps the dialog open and reports a failed selection', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  attachmentLibraryHarness.list = async () => ({
    items: [
      {
        attachmentId: 'attachment-2',
        conversationId: 'conversation-2',
        conversationTitle: 'Another conversation',
        createdAt: 2,
        fileName: 'report.pdf',
        generationId: null,
        isAvailable: true,
        isFavorite: false,
        messageId: 'message-2',
        mimeType: 'application/pdf',
        model: null,
        projectId: null,
        prompt: null,
        providerRoute: null,
        sizeBytes: 42,
        tags: [],
      },
    ],
    nextOffset: null,
  });
  const openChanges = [];

  render(
    localized(
      'en',
      React.createElement(AttachmentLibraryDialog, {
        disabled: false,
        onOpenChange: (open) => openChanges.push(open),
        onSelect: async () => false,
        open: true,
      }),
    ),
  );

  await screen.findByText('report.pdf');
  fireEvent.click(screen.getByRole('button', { name: translations.attachmentLibraryUseFile }));

  assert.equal((await screen.findByRole('alert')).textContent, translations.attachmentLibraryAddFailed);
  assert.deepEqual(openChanges, []);
  assert.ok(screen.getByRole('dialog', { name: translations.attachmentLibraryTitle }));
});

test('the attachment library retry action reloads after a failed request', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  let attempts = 0;
  attachmentLibraryHarness.list = async () => {
    attempts += 1;
    if (attempts === 1) throw new Error('test-only unavailable attachment library');
    return {
      items: [
        {
          attachmentId: 'attachment-3',
          conversationId: 'conversation-3',
          conversationTitle: 'Recovered conversation',
          createdAt: 3,
          fileName: 'recovered.txt',
          generationId: null,
          isAvailable: true,
          isFavorite: false,
          messageId: 'message-3',
          mimeType: 'text/plain',
          model: null,
          projectId: null,
          prompt: null,
          providerRoute: null,
          sizeBytes: 24,
          tags: [],
        },
      ],
      nextOffset: null,
    };
  };

  render(
    localized(
      'en',
      React.createElement(AttachmentLibraryDialog, {
        disabled: false,
        onOpenChange: () => undefined,
        onSelect: async () => true,
        open: true,
      }),
    ),
  );

  assert.ok(await screen.findByText(translations.attachmentLibraryLoadFailed));
  fireEvent.click(screen.getByRole('button', { name: translations.retry }));

  assert.ok(await screen.findByText('recovered.txt'));
  assert.equal(attempts, 2);
});

test('the startup surface selector reflects the persisted choice after saving', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  render(localized('en', React.createElement(AppWindowSettingsSection)));
  const startupSurface = screen.getByRole('combobox', { name: translations.startupSurface });
  assert.equal(startupSurface.textContent, translations.startupSurfaceLast);

  fireEvent.keyDown(startupSurface, { key: 'ArrowDown' });
  fireEvent.click(await screen.findByRole('option', { name: translations.securityScansTitle }));

  await waitFor(() => {
    assert.equal(appPreferencesHarness.startupSurface, 'security');
    assert.equal(startupSurface.textContent, translations.securityScansTitle);
  });
  assert.equal(screen.getByRole('status').textContent, translations.settingsSaved);
});

test('a failed startup surface save keeps the previously selected value', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  appPreferencesHarness.writeStartupSurface = async () => {
    throw new Error('test-only preference write failure');
  };

  render(localized('en', React.createElement(AppWindowSettingsSection)));
  const startupSurface = screen.getByRole('combobox', { name: translations.startupSurface });

  fireEvent.keyDown(startupSurface, { key: 'ArrowDown' });
  fireEvent.click(await screen.findByRole('option', { name: translations.securityScansTitle }));

  assert.equal((await screen.findByRole('alert')).textContent, translations.settingsSaveFailed);
  assert.equal(startupSurface.textContent, translations.startupSurfaceLast);
  assert.equal(appPreferencesHarness.startupSurface, 'last');
});

test('a failed sidebar service move leaves the custom folder membership unchanged', async () => {
  const conversation = {
    id: 'conversation-project-1',
    title: 'Existing project conversation',
    projectId: 'project-1',
    isArchived: false,
    isPinned: false,
    createdAt: 1,
    updatedAt: 1,
  };
  const folder = {
    id: 'custom-folder-1',
    name: 'Saved for later',
    conversationIds: [],
    createdAt: 1,
  };
  sidebarHarness.customFoldersReadResult = { kind: 'ready', folders: [folder] };
  sidebarHarness.chatState = {
    activeConversationId: null,
    compactingConversationId: null,
    conversations: [conversation],
    errorMessage: null,
    initialize: async () => undefined,
    projects: [{ id: 'project-1', name: 'Existing project' }],
    projectsStatus: 'ready',
    refreshProjects: async () => undefined,
    selectConversation: async () => undefined,
    sendingConversationId: null,
    sendingConversationIds: [],
    serviceStatus: 'ready',
    startNewChat: async () => undefined,
    startNewChatWithAgent: async () => undefined,
    startTemporaryChat: async () => undefined,
    temporaryChat: null,
  };
  sidebarHarness.actions = {
    archiveConversation: async () => undefined,
    createProject: async () => null,
    deleteConversation: async () => false,
    duplicateConversation: async () => null,
    exportConversation: async () => undefined,
    feedback: null,
    isBusy: false,
    moveConversation: async (movedConversation, projectId) => {
      sidebarHarness.moveCalls.push({ conversationId: movedConversation.id, projectId });
      return false;
    },
    saveTitle: async () => undefined,
    showFeedback: () => undefined,
    togglePinned: async () => false,
  };

  render(
    localized(
      'en',
      React.createElement(ConversationSidebar, {
        onRequestTemporaryExit: () => undefined,
      }),
    ),
  );

  const folderTarget = screen.getByText(folder.name).closest('li');
  assert.ok(folderTarget);
  fireEvent.drop(folderTarget, {
    dataTransfer: {
      dropEffect: 'none',
      getData: (type) => (type === 'application/x-openchat-conversation' ? conversation.id : ''),
      types: ['application/x-openchat-conversation'],
    },
  });

  await waitFor(() => {
    assert.deepEqual(sidebarHarness.moveCalls, [{ conversationId: conversation.id, projectId: null }]);
  });
  assert.deepEqual(sidebarHarness.writeCustomFoldersCalls, []);
  assert.deepEqual(sidebarHarness.customFoldersReadResult.folders, [folder]);
});

test('custom folder preference decoding preserves invalid data and blocks writes', async () => {
  const validFolder = {
    id: 'folder-one',
    name: 'Folder one',
    conversationIds: ['conversation-one'],
    createdAt: 1,
  };
  const invalidValues = [
    '{not valid json',
    JSON.stringify({ folders: [validFolder] }),
    JSON.stringify([validFolder, { id: '', name: 'Invalid', conversationIds: [], createdAt: 2 }]),
  ];

  for (const rawValue of invalidValues) {
    sidebarHarness.preferenceValue = rawValue;
    const result = sidebarPreferencesModule.readCustomSidebarFolders();

    assert.equal(result.kind, 'invalid');
    assert.equal(result.rawValue, rawValue);
    await assert.rejects(
      sidebarPreferencesModule.writeCustomSidebarFolders([validFolder]),
      /was not overwritten/u,
    );
    assert.equal(sidebarHarness.preferenceValue, rawValue);
    assert.deepEqual(sidebarHarness.preferenceWriteCalls, []);
  }

  sidebarHarness.preferenceValue = JSON.stringify([validFolder]);
  assert.deepEqual(sidebarPreferencesModule.readCustomSidebarFolders(), {
    kind: 'ready',
    folders: [validFolder],
  });
  await sidebarPreferencesModule.writeCustomSidebarFolders([]);
  assert.equal(sidebarHarness.preferenceValue, '[]');
  assert.deepEqual(sidebarHarness.preferenceWriteCalls, ['[]']);

  sidebarHarness.preferenceValue = JSON.stringify([
    { id: 'legacy-folder', name: 'Legacy folder', conversationIds: [] },
  ]);
  const legacyResult = sidebarPreferencesModule.readCustomSidebarFolders();
  assert.equal(legacyResult.kind, 'ready');
  assert.equal(typeof legacyResult.folders[0]?.createdAt, 'number');
});

test('custom folder read failures are reported and block preference writes', async () => {
  sidebarHarness.preferenceReadFailure = new Error('test-only read failure');

  const result = sidebarPreferencesModule.readCustomSidebarFolders();

  assert.equal(result.kind, 'unavailable');
  await assert.rejects(
    sidebarPreferencesModule.writeCustomSidebarFolders([]),
    /was not overwritten/u,
  );
  assert.deepEqual(sidebarHarness.preferenceWriteCalls, []);
});

test('invalid custom folder data stays visible, can be exported, and blocks moves', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  const rawValue = '[{"id":"folder-one","name":"Saved data","conversationIds":[],"createdAt":1},null]';
  const folder = {
    id: 'folder-one',
    name: 'Saved data',
    conversationIds: [],
    createdAt: 1,
  };
  const conversation = {
    id: 'conversation-project-1',
    title: 'Existing project conversation',
    projectId: 'project-1',
    isArchived: false,
    isPinned: false,
    createdAt: 1,
    updatedAt: 1,
  };
  sidebarHarness.customFoldersReadResult = {
    kind: 'invalid',
    folders: [folder],
    rawValue,
    error: new Error('test-only invalid preference'),
  };
  sidebarHarness.chatState = {
    activeConversationId: null,
    compactingConversationId: null,
    conversations: [conversation],
    errorMessage: null,
    initialize: async () => undefined,
    projects: [],
    projectsStatus: 'ready',
    refreshProjects: async () => undefined,
    selectConversation: async () => undefined,
    sendingConversationId: null,
    sendingConversationIds: [],
    serviceStatus: 'ready',
    startNewChat: async () => undefined,
    startNewChatWithAgent: async () => undefined,
    startTemporaryChat: async () => undefined,
    temporaryChat: null,
  };
  const feedbackKeys = [];
  sidebarHarness.actions = {
    archiveConversation: async () => undefined,
    createProject: async () => null,
    deleteConversation: async () => false,
    duplicateConversation: async () => null,
    exportConversation: async () => undefined,
    feedback: null,
    isBusy: false,
    moveConversation: async (movedConversation, projectId) => {
      sidebarHarness.moveCalls.push({ conversationId: movedConversation.id, projectId });
      return true;
    },
    saveTitle: async () => undefined,
    showFeedback: (key) => feedbackKeys.push(key),
    togglePinned: async () => false,
  };

  const previousCreateObjectURL = Object.getOwnPropertyDescriptor(URL, 'createObjectURL');
  const previousRevokeObjectURL = Object.getOwnPropertyDescriptor(URL, 'revokeObjectURL');
  const exportedBlobs = [];
  Object.defineProperty(URL, 'createObjectURL', {
    configurable: true,
    value: (blob) => {
      exportedBlobs.push(blob);
      return 'blob:openchat-recovery-test';
    },
    writable: true,
  });
  Object.defineProperty(URL, 'revokeObjectURL', {
    configurable: true,
    value: () => undefined,
    writable: true,
  });

  try {
    render(
      localized(
        'en',
        React.createElement(ConversationSidebar, {
          onRequestTemporaryExit: () => undefined,
        }),
      ),
    );

    assert.equal(screen.getByRole('alert').textContent, translations.customFolderDataInvalid);
    assert.equal(screen.getByRole('button', { name: translations.createCustomFolder }).disabled, true);
    assert.equal(screen.getByRole('button', { name: translations.renameCustomFolder }).disabled, true);
    assert.equal(screen.getByRole('button', { name: translations.deleteCustomFolder }).disabled, true);

    fireEvent.click(screen.getByRole('button', { name: translations.customFolderDataExport }));
    assert.equal(await exportedBlobs[0]?.text(), rawValue);
    assert.equal(screen.getByRole('status').textContent, translations.customFolderDataExported);

    const chatsSection = screen.getByRole('region', { name: translations.chats });
    fireEvent.drop(chatsSection, {
      dataTransfer: {
        dropEffect: 'none',
        getData: (type) => (type === 'application/x-openchat-conversation' ? conversation.id : ''),
        types: ['application/x-openchat-conversation'],
      },
    });

    await waitFor(() => assert.deepEqual(feedbackKeys, ['customFolderDataInvalid']));
    assert.deepEqual(sidebarHarness.moveCalls, []);
    assert.deepEqual(sidebarHarness.writeCustomFoldersCalls, []);
  } finally {
    if (previousCreateObjectURL) Object.defineProperty(URL, 'createObjectURL', previousCreateObjectURL);
    else Reflect.deleteProperty(URL, 'createObjectURL');
    if (previousRevokeObjectURL) Object.defineProperty(URL, 'revokeObjectURL', previousRevokeObjectURL);
    else Reflect.deleteProperty(URL, 'revokeObjectURL');
  }
});

test('the MCP catalog renders localized category labels and filters matching servers', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  const categoryCases = [
    ['developer', 'mcpCategoryDeveloper', 'Developer toolkit'],
    ['database', 'mcpCategoryDatabase', 'Database connector'],
    ['webSearch', 'mcpCategoryWebSearch', 'Web search'],
    ['devOpsCloud', 'mcpCategoryDevOpsCloud', 'Cloud operations'],
    ['productivity', 'mcpCategoryProductivity', 'Productivity tools'],
    ['memoryReasoning', 'mcpCategoryMemoryReasoning', 'Memory research'],
    ['finance', 'mcpCategoryFinance', 'Finance data'],
  ];
  const servers = categoryCases.map(([category, , name], index) => ({
    id: `mcp-test-${index + 1}`,
    name,
    category,
    description: `${name} description`,
    command: 'node',
    args: [],
    requiredEnvKeys: [],
    optionalEnvKeys: [],
    installed: false,
    enabled: false,
    installDir: '',
    discoveredTools: [],
  }));
  const state = {
    servers,
    loadState: 'ready',
    invalidConfigCount: 0,
    busyServerIds: [],
    errorCode: null,
    errorDetails: null,
    initialize: async () => undefined,
    install: async () => false,
    setEnabled: async () => false,
    remove: async () => false,
    loadSecretStatus: async () => [],
    saveSecrets: async () => false,
    probe: async () => false,
  };
  mcpHarness.state = state;

  render(localized('en', React.createElement(McpCatalogTab)));

  const categoryNavigation = screen.getByRole('navigation', { name: translations.mcpCategoryFilter });
  for (const [selectedCategory, translationKey, expectedServerName] of categoryCases) {
    const filter = within(categoryNavigation).getByRole('button', { name: translations[translationKey] });
    fireEvent.click(filter);
    assert.equal(filter.getAttribute('aria-pressed'), 'true', `${selectedCategory} filter is selected`);

    for (const server of servers) {
      assert.equal(
        screen.queryByRole('heading', { name: server.name }) !== null,
        server.name === expectedServerName,
        `${selectedCategory} filter visibility for ${server.name}`,
      );
    }
    assert.ok(screen.getByText(translations[translationKey], { selector: 'span' }));
  }
});

test('title bar search points its active descendant at a rendered result and closes on Escape', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  let selectedConversation = null;

  render(
    localized(
      'en',
      React.createElement(TitleBarSearch, {
        onSelectConversation: (...args) => {
          selectedConversation = args;
        },
      }),
    ),
  );

  const input = screen.getByRole('combobox', { name: translations.searchChats });
  fireEvent.change(input, { target: { value: 'river' } });
  assert.equal(input.value, 'river');

  await waitFor(() => {
    const listbox = screen.getByRole('listbox', { name: translations.searchChats });
    assert.equal(input.getAttribute('aria-expanded'), 'true');
    assert.equal(input.getAttribute('aria-controls'), listbox.id);
  });

  await waitFor(() => assert.ok(input.getAttribute('aria-activedescendant')));
  const initialActiveId = input.getAttribute('aria-activedescendant');
  assert.ok(initialActiveId);
  const initialOption = document.getElementById(initialActiveId);
  assert.ok(initialOption);
  assert.equal(initialOption.getAttribute('role'), 'option');
  assert.equal(initialOption.getAttribute('aria-selected'), 'true');

  fireEvent.keyDown(input, { key: 'ArrowDown' });
  const nextActiveId = input.getAttribute('aria-activedescendant');
  assert.ok(nextActiveId);
  assert.notEqual(nextActiveId, initialActiveId);
  const nextOption = document.getElementById(nextActiveId);
  assert.ok(nextOption);
  assert.equal(nextOption.getAttribute('role'), 'option');
  assert.equal(nextOption.getAttribute('aria-selected'), 'true');

  fireEvent.click(nextOption);
  assert.deepEqual(selectedConversation, ['conversation-river', 'message-river', 'river']);

  fireEvent.focus(input);
  await waitFor(() => screen.getByRole('listbox', { name: translations.searchChats }));
  fireEvent.keyDown(input, { key: 'Escape' });
  assert.equal(input.getAttribute('aria-expanded'), 'false');
  assert.equal(input.hasAttribute('aria-controls'), false);
  assert.equal(screen.queryByRole('listbox', { name: translations.searchChats }), null);
});

test('recent-search options follow arrow keys and Enter opens the selected term', async () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  render(
    localized(
      'en',
      React.createElement(TitleBarSearch, { onSelectConversation: () => undefined }),
    ),
  );

  const input = screen.getByRole('combobox', { name: translations.searchChats });
  fireEvent.focus(input);
  const recentListbox = await screen.findByRole('listbox', { name: translations.recentSearches });
  const options = within(recentListbox).getAllByRole('option');
  assert.deepEqual(options.map((option) => option.textContent), ['river', 'ocean']);
  assert.equal(input.getAttribute('aria-controls'), recentListbox.id);

  fireEvent.keyDown(input, { key: 'ArrowDown' });
  assert.equal(input.getAttribute('aria-activedescendant'), options[0].id);
  assert.equal(options[0].getAttribute('aria-selected'), 'true');

  fireEvent.keyDown(input, { key: 'ArrowDown' });
  assert.equal(input.getAttribute('aria-activedescendant'), options[1].id);
  assert.equal(options[0].getAttribute('aria-selected'), 'false');
  assert.equal(options[1].getAttribute('aria-selected'), 'true');

  fireEvent.keyDown(input, { key: 'Enter' });
  assert.equal(input.value, 'ocean');
  const searchListbox = await screen.findByRole('listbox', { name: translations.searchChats });
  assert.equal(input.getAttribute('aria-controls'), searchListbox.id);
});

test('recent-search delete actions have term-specific names and update only after persistence succeeds', async () => {
  const translations = translationsByLocale.get('en');
  const i18n = i18nByLocale.get('en');
  assert.ok(translations);
  assert.ok(i18n);

  render(
    localized(
      'en',
      React.createElement(TitleBarSearch, { onSelectConversation: () => undefined }),
    ),
  );

  const input = screen.getByRole('combobox', { name: translations.searchChats });
  fireEvent.focus(input);
  const listbox = await screen.findByRole('listbox', { name: translations.recentSearches });
  assert.equal(within(listbox).queryByRole('button'), null);

  const deleteButton = screen.getByRole('button', {
    name: i18n.t('deleteRecentSearch', { term: 'river' }),
  });
  fireEvent.click(deleteButton);

  await waitFor(() => {
    const updatedListbox = screen.getByRole('listbox', { name: translations.recentSearches });
    assert.deepEqual(within(updatedListbox).getAllByRole('option').map((option) => option.textContent), ['ocean']);
  });
  assert.deepEqual(recentSearchHarness.terms, ['ocean']);
});

test('failed recent-search deletion keeps the saved and visible option and announces the error', async () => {
  const translations = translationsByLocale.get('en');
  const i18n = i18nByLocale.get('en');
  assert.ok(translations);
  assert.ok(i18n);
  recentSearchHarness.failWrites = true;

  render(
    localized(
      'en',
      React.createElement(TitleBarSearch, { onSelectConversation: () => undefined }),
    ),
  );

  const input = screen.getByRole('combobox', { name: translations.searchChats });
  fireEvent.focus(input);
  const listbox = await screen.findByRole('listbox', { name: translations.recentSearches });
  fireEvent.click(
    screen.getByRole('button', {
      name: i18n.t('deleteRecentSearch', { term: 'river' }),
    }),
  );

  assert.equal((await screen.findByRole('alert')).textContent, translations.recentSearchesDeleteFailed);
  assert.deepEqual(recentSearchHarness.terms, ['river', 'ocean']);
  assert.deepEqual(within(listbox).getAllByRole('option').map((option) => option.textContent), ['river', 'ocean']);
});

test('the primary navigation landmark and surface action render in every supported locale', () => {
  for (const locale of supportedLocales) {
    const translations = translationsByLocale.get(locale);
    assert.ok(translations, `missing translations for ${locale}`);
    const navigatedSurfaces = [];

    render(
      localized(
        locale,
        React.createElement(NavigationRail, {
          activeSurface: 'chats',
          isDark: false,
          onNavigate: (surface) => navigatedSurfaces.push(surface),
          onToggleTheme: () => undefined,
        }),
      ),
    );

    const navigation = screen.getByRole('navigation', { name: translations.primaryNavigation });
    assert.equal(navigation.tagName, 'NAV');
    assert.notEqual(translations.primaryNavigation, translations.chats);

    fireEvent.click(screen.getByRole('button', { name: translations.modelLibrary }));
    assert.deepEqual(navigatedSurfaces, ['models']);
    cleanup();
  }
});
