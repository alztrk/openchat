import assert from 'node:assert/strict';
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
      name: 'openchat-test-project-sidebar-drop-targets',
      enforce: 'pre',
      resolveId(source, importer) {
        if (
          source === '@features/chat/components' &&
          importer?.replaceAll('\\', '/').endsWith('/src/features/projects/components/ProjectSidebarSection.tsx')
        ) {
          return '\0openchat-test-project-sidebar-chat-components';
        }
        return null;
      },
      load(id) {
        if (id !== '\0openchat-test-project-sidebar-chat-components') return null;
        return `
          export { ConversationSidebarSection } from '/src/features/chat/components/ConversationSidebarSection.tsx';
          export function ConversationSidebarRow() { return null; }
        `;
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
const { I18nextProvider, initReactI18next } = await import('react-i18next');
const { cleanup, fireEvent, render, screen } = await import('@testing-library/react');
const { ProjectSidebarSection } = await vite.ssrLoadModule(
  '/src/features/projects/components/ProjectSidebarSection.tsx',
);
const { ConversationSidebarSection } = await vite.ssrLoadModule(
  '/src/features/chat/components/ConversationSidebarSection.tsx',
);
const { MessageSquare } = await import('lucide-react');

const i18n = createInstance();
await i18n.use(initReactI18next).init({
  fallbackLng: false,
  lng: 'en',
  resources: { en: { translation: { projects: 'Projects' } } },
});

const conversationMimeType = 'application/x-openchat-conversation';

function dropDataTransfer(conversationId, types = [conversationMimeType]) {
  const hasConversationPayload = types.includes(conversationMimeType);
  return {
    dropEffect: 'none',
    getData: (type) => (type === conversationMimeType && hasConversationPayload ? conversationId : ''),
    types,
  };
}

function renderWithLocale(element) {
  return render(React.createElement(I18nextProvider, { i18n }, element));
}

function projectSectionProps(onDropConversationToProject, isBusy = false) {
  return {
    activeConversationId: null,
    collapsed: false,
    conversations: [],
    hasMore: false,
    isBusy,
    isLoadingMore: false,
    loadError: false,
    onArchive: () => undefined,
    onCreateProjectChat: () => undefined,
    onDelete: () => undefined,
    onDropConversationToProject,
    onExport: () => undefined,
    onLoadMore: () => undefined,
    onMoveToChats: () => undefined,
    onMoveToProject: () => undefined,
    onRename: () => undefined,
    onRetry: () => undefined,
    onSelectConversation: () => undefined,
    onToggleCollapsed: () => undefined,
    onTogglePinned: () => undefined,
    projects: [{ id: 'project-1', name: 'Project Alpha', workspaceRoots: [] }],
    status: 'ready',
  };
}

afterEach(() => cleanup());

after(async () => {
  await browserWindow.happyDOM.abort();
  await vite.close();
  for (const [name, descriptor] of originalGlobals) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
});

test('project drop target accepts an OpenChat conversation and routes it to that project', () => {
  const droppedConversationIds = [];
  renderWithLocale(React.createElement(ProjectSidebarSection, projectSectionProps((conversationId, projectId) => {
    droppedConversationIds.push({ conversationId, projectId });
  })));

  const projectTarget = screen.getByRole('button', { name: 'Project Alpha' }).closest('li');
  assert.ok(projectTarget);
  fireEvent.dragOver(projectTarget, { dataTransfer: dropDataTransfer('conversation-1') });
  fireEvent.drop(projectTarget, { dataTransfer: dropDataTransfer('conversation-1') });

  assert.deepEqual(droppedConversationIds, [{ conversationId: 'conversation-1', projectId: 'project-1' }]);
});

test('project drop target ignores unsupported payloads and busy state', () => {
  const droppedConversationIds = [];
  const element = (isBusy) =>
    React.createElement(ProjectSidebarSection, projectSectionProps((conversationId, projectId) => {
      droppedConversationIds.push({ conversationId, projectId });
    }, isBusy));

  renderWithLocale(element(false));
  const projectTarget = screen.getByRole('button', { name: 'Project Alpha' }).closest('li');
  assert.ok(projectTarget);
  assert.equal(fireEvent.dragOver(projectTarget, { dataTransfer: dropDataTransfer('foreign-item', ['text/plain']) }), true);
  fireEvent.drop(projectTarget, { dataTransfer: dropDataTransfer('foreign-item', ['text/plain']) });
  cleanup();

  renderWithLocale(element(true));
  const busyProjectTarget = screen.getByRole('button', { name: 'Project Alpha' }).closest('li');
  assert.ok(busyProjectTarget);
  assert.equal(fireEvent.dragOver(busyProjectTarget, { dataTransfer: dropDataTransfer('conversation-1') }), true);
  fireEvent.drop(busyProjectTarget, { dataTransfer: dropDataTransfer('conversation-1') });

  assert.deepEqual(droppedConversationIds, []);
});

test('Chats section accepts an OpenChat conversation and ignores unsupported payloads', () => {
  const droppedConversationIds = [];
  renderWithLocale(
    React.createElement(
      ConversationSidebarSection,
      {
        collapsed: false,
        icon: MessageSquare,
        onDropConversation: (conversationId) => droppedConversationIds.push(conversationId),
        onToggle: () => undefined,
        title: 'Chats',
      },
      React.createElement('li', null, 'Chat row'),
    ),
  );

  const chatsTarget = screen.getByRole('region', { name: 'Chats' });
  assert.equal(fireEvent.dragOver(chatsTarget, { dataTransfer: dropDataTransfer('conversation-2') }), false);
  fireEvent.drop(chatsTarget, { dataTransfer: dropDataTransfer('conversation-2') });
  assert.equal(fireEvent.dragOver(chatsTarget, { dataTransfer: dropDataTransfer('foreign-item', ['text/plain']) }), true);
  fireEvent.drop(chatsTarget, { dataTransfer: dropDataTransfer('foreign-item', ['text/plain']) });

  assert.deepEqual(droppedConversationIds, ['conversation-2']);
});
