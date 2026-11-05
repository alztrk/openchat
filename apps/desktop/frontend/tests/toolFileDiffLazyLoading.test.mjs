import assert from 'node:assert/strict';
import path from 'node:path';
import test, { after } from 'node:test';
import { Window } from 'happy-dom';
import { createServer } from 'vite';

let diffViewerImportCount = 0;
let releaseDiffViewer;
const diffViewerGate = new Promise((resolve) => {
  releaseDiffViewer = resolve;
});
const diffViewerId = '\0tool-file-diff-test:viewer';
const stubModules = new Map([
  [
    'lucide-react',
    `export const Check = () => null;
     export const File = () => null;
     export const Folder = () => null;
     export const FolderOpen = () => null;
     export const LoaderCircle = () => null;`,
  ],
  ['react-i18next', "export function useTranslation() { return { t: (key) => key }; }"],
  ['@shared/components', 'export const Button = () => null; export const TitleTooltip = () => null;'],
  [
    '@features/chat/data/chatFileChanges',
    'export async function readConversationFileChangeDiff() { return null; }',
  ],
  ['./ReadCodeLines', 'export const ReadCodeLines = () => null;'],
  [
    './FileSyntaxHighlight',
    `export function detectFileLanguage() { return 'text'; }
     export function highlightFileLines() { return []; }
     export function useHighlightingState() {}`,
  ],
  [
    './ToolSharedComponents',
    `export const PathHeading = () => null;
     export const PreviewBlock = () => null;
     export const ResultFootnote = () => null;
     export const ToolResultNotice = () => null;`,
  ],
  [
    './toolResultUtils',
    `export const asRecord = () => null;
     export const formatBytes = () => '';
     export const formatToolPath = () => '';
     export const getRequestedSearchQuery = () => null;
     export const getRequestedToolPath = () => null;
     export const integerValue = () => null;
     export const isNonNegativeInteger = () => false;
     export const pathName = () => '';
     export const stringValue = () => null;`,
  ],
]);

const vite = await createServer({
  appType: 'custom',
  configFile: false,
  plugins: [
    {
      name: 'tool-file-diff-test-modules',
      enforce: 'pre',
      resolveId(source, importer) {
        if (
          source === '../ChatFileChangesDiffViewer' &&
          importer?.replaceAll('\\', '/').endsWith('/src/features/chat/components/message/ToolFileResults.tsx')
        ) {
          return diffViewerId;
        }
        return stubModules.has(source) ? `\0tool-file-diff-test:${source}` : null;
      },
      async load(id) {
        if (id === diffViewerId) {
          diffViewerImportCount += 1;
          await diffViewerGate;
          return `import { createElement } from 'react';
            export function DiffContent({ diff }) {
              return createElement('div', { 'data-testid': 'file-diff-content' }, diff?.path ?? '');
            }`;
        }
        const dependency = [...stubModules.entries()].find(([specifier]) => id === `\0tool-file-diff-test:${specifier}`);
        return dependency?.[1] ?? null;
      },
    },
  ],
  optimizeDeps: { noDiscovery: true, include: [] },
  root: path.resolve('.'),
  server: { middlewareMode: true, hmr: false, ws: false },
  ssr: { noExternal: ['react-i18next'] },
});

const browserWindow = new Window({ url: 'http://localhost/' });
const browserGlobals = new Map([
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

for (const [name, value] of browserGlobals) {
  originalGlobals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { configurable: true, value, writable: true });
}

const React = await import('react');
const { cleanup, render, screen, waitFor } = await import('@testing-library/react');
const { EditResult } = await vite.ssrLoadModule('/src/features/chat/components/message/ToolFileResults.tsx');

after(async () => {
  releaseDiffViewer();
  cleanup();
  for (const [name, descriptor] of originalGlobals) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
  await browserWindow.happyDOM.close();
  await vite.close();
});

test('tool edit results defer the diff viewer and show an accessible loading state', async () => {
  assert.equal(diffViewerImportCount, 0);

  render(
    React.createElement(EditResult, {
      activity: { arguments: {}, fileChanges: [] },
      conversationId: null,
    }),
  );

  await waitFor(() => assert.equal(diffViewerImportCount, 1));
  assert.equal(screen.getByRole('status').textContent, 'fileChangesDiffLoading');

  releaseDiffViewer();
  await waitFor(() => assert.ok(screen.getByTestId('file-diff-content')));
  assert.equal(screen.queryByRole('status'), null);
});
