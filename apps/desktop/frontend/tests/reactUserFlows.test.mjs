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
              return process.__openchatSidebarHarness.customFolders;
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
              return process.__openchatSidebarHarness.writeCustomFolders(folders);
            }
            export function writeSidebarManualOrder() {}
            export function writeSidebarSortMode() {}
            export function writeSidebarViewMode() {}
            export function writeSidebarWidth() {}
            export function writeSkipConversationDeleteConfirmation() { return Promise.resolve(); }
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
const { McpCatalogTab } = await vite.ssrLoadModule('/src/features/model-catalog/components/McpCatalogTab.tsx');

const localeDirectory = path.resolve('src/shared/lib/i18n/locales');
const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'];
const translationsByLocale = new Map();
const i18nByLocale = new Map();
const originalAttachmentLibraryHarness = Object.getOwnPropertyDescriptor(process, '__openchatAttachmentLibraryHarness');
const originalAppPreferencesHarness = Object.getOwnPropertyDescriptor(process, '__openchatAppPreferencesHarness');
const originalSidebarHarness = Object.getOwnPropertyDescriptor(process, '__openchatSidebarHarness');
const originalMcpHarness = Object.getOwnPropertyDescriptor(process, '__openchatMcpHarness');
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
  customFolders: [],
  moveCalls: [],
  writeCustomFoldersCalls: [],
  writeCustomFolders: async (folders) => {
    sidebarHarness.writeCustomFoldersCalls.push(folders);
    sidebarHarness.customFolders = folders;
  },
};
const mcpHarness = {
  state: {},
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
  sidebarHarness.customFolders = [];
  sidebarHarness.moveCalls = [];
  sidebarHarness.writeCustomFoldersCalls = [];
  sidebarHarness.writeCustomFolders = async (folders) => {
    sidebarHarness.writeCustomFoldersCalls.push(folders);
    sidebarHarness.customFolders = folders;
  };
  mcpHarness.state = {};
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
  sidebarHarness.customFolders = [folder];
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
  assert.deepEqual(sidebarHarness.customFolders, [folder]);
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
