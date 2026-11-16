import assert from 'node:assert/strict';
import process from 'node:process';
import path from 'node:path';
import test, { after, afterEach } from 'node:test';
import { Window } from 'happy-dom';
import { createServer } from 'vite';

const harness = {
  attachmentBase64: 'iVBORw0KGgo=',
  calls: [],
  failRead: false,
  pendingCounts: [],
  setPendingAttachmentCount(count) {
    harness.pendingCounts.push(count);
  },
};

const originalHarness = Object.getOwnPropertyDescriptor(process, '__openchatComposerAttachmentHarness');
Object.defineProperty(process, '__openchatComposerAttachmentHarness', {
  configurable: true,
  value: harness,
  writable: true,
});

const vite = await createServer({
  appType: 'custom',
  configFile: path.resolve('vite.config.ts'),
  mode: 'development',
  plugins: [
    {
      name: 'openchat-test-composer-existing-attachments',
      enforce: 'pre',
      resolveId(source, importer) {
        const normalizedSource = source.replaceAll('\\', '/');
        const normalizedImporter = importer?.replaceAll('\\', '/') ?? '';
        if (
          (normalizedSource === '@features/chat/state' || normalizedSource.endsWith('/src/features/chat/state')) &&
          normalizedImporter.endsWith('/src/features/chat/components/useComposerAttachments.ts')
        ) {
          return '\0openchat-test-composer-attachment-chat-state';
        }
        if (
          (normalizedSource === '@features/chat/data' || normalizedSource.endsWith('/src/features/chat/data')) &&
          normalizedImporter.endsWith('/src/features/chat/components/useComposerAttachments.ts')
        ) {
          return '\0openchat-test-composer-attachment-chat-data';
        }
        if (
          (normalizedSource === '@shared/lib/platform/local-service' ||
            normalizedSource.endsWith('/src/shared/lib/platform/local-service')) &&
          (normalizedImporter.endsWith('/src/features/chat/data/chatStorage.ts') ||
            normalizedImporter.endsWith('/src/features/chat/data/chatAttachments.ts'))
        ) {
          return '\0openchat-test-composer-attachment-local-service';
        }
        if (
          (normalizedSource === '@shared/lib/platform/tauri' ||
            normalizedSource.endsWith('/src/shared/lib/platform/tauri')) &&
          normalizedImporter.endsWith('/src/features/chat/data/chatStorage.ts')
        ) {
          return '\0openchat-test-composer-attachment-tauri';
        }
        if (
          (normalizedSource === '@shared/lib/preferences/assistantProfiles' ||
            normalizedSource.endsWith('/src/shared/lib/preferences/assistantProfiles')) &&
          normalizedImporter.endsWith('/src/features/chat/data/chatStorage.ts')
        ) {
          return '\0openchat-test-composer-attachment-assistant-profiles';
        }
        return null;
      },
      load(id) {
        if (id === '\0openchat-test-composer-attachment-chat-state') {
          return `
            import process from 'node:process';
            export function useChatStore(selector) {
              return selector({ setPendingAttachmentCount: process.__openchatComposerAttachmentHarness.setPendingAttachmentCount });
            }
          `;
        }
        if (id === '\0openchat-test-composer-attachment-chat-data') {
          return `
            export {
              isConversationAttachmentReference,
              readMessageAttachment,
            } from '/src/features/chat/data/chatStorage.ts';
          `;
        }
        if (id === '\0openchat-test-composer-attachment-local-service') {
          return `
            import process from 'node:process';
            export class LocalServiceError extends Error {}
            export const localServiceClient = {
              call(method, parameters) {
                const harness = process.__openchatComposerAttachmentHarness;
                harness.calls.push({ method, parameters });
                if (harness.failRead) return Promise.reject(new Error('Saved attachment is unavailable.'));
                return Promise.resolve({ dataBase64: harness.attachmentBase64 });
              },
            };
          `;
        }
        if (id === '\0openchat-test-composer-attachment-tauri') {
          return 'export function invoke() { return Promise.resolve(undefined); }';
        }
        if (id === '\0openchat-test-composer-attachment-assistant-profiles') {
          return 'export function isAssistantAgentSnapshot() { return false; }';
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
  ['navigator', browserWindow.navigator],
  ['HTMLElement', browserWindow.HTMLElement],
  ['Node', browserWindow.Node],
  ['NodeFilter', browserWindow.NodeFilter],
  ['Element', browserWindow.Element],
  ['Event', browserWindow.Event],
  ['CustomEvent', browserWindow.CustomEvent],
  ['MutationObserver', browserWindow.MutationObserver],
  ['ResizeObserver', browserWindow.ResizeObserver],
  ['File', browserWindow.File],
  ['IS_REACT_ACT_ENVIRONMENT', true],
]);
const originalGlobals = new Map();
for (const [name, value] of globals) {
  originalGlobals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { configurable: true, value, writable: true });
}

const originalCreateObjectURL = Object.getOwnPropertyDescriptor(URL, 'createObjectURL');
const originalRevokeObjectURL = Object.getOwnPropertyDescriptor(URL, 'revokeObjectURL');
let nextPreviewId = 0;
Object.defineProperty(URL, 'createObjectURL', {
  configurable: true,
  value: () => `blob:composer-test-${++nextPreviewId}`,
  writable: true,
});
Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: () => undefined, writable: true });

const { act, cleanup, renderHook, waitFor } = await import('@testing-library/react');
const { useComposerAttachments } = await vite.ssrLoadModule(
  '/src/features/chat/components/useComposerAttachments.ts',
);

const savedImage = {
  conversationId: 'conversation-1',
  messageId: 'message-1',
  attachmentId: 'attachment-1',
  fileName: 'saved-image.png',
  mimeType: 'image/png',
  sizeBytes: 8,
};

async function dispatchSavedAttachment(conversationId = savedImage.conversationId) {
  await act(async () => {
    browserWindow.dispatchEvent(
      new browserWindow.CustomEvent('openchat-composer-add-attachments', {
        detail: { conversationId, attachments: [savedImage] },
      }),
    );
    await Promise.resolve();
  });
}

afterEach(() => {
  cleanup();
  harness.attachmentBase64 = 'iVBORw0KGgo=';
  harness.calls = [];
  harness.failRead = false;
  harness.pendingCounts = [];
});

after(async () => {
  cleanup();
  await browserWindow.happyDOM.abort();
  await vite.close();
  if (originalHarness) Object.defineProperty(process, '__openchatComposerAttachmentHarness', originalHarness);
  else Reflect.deleteProperty(process, '__openchatComposerAttachmentHarness');
  for (const [name, descriptor] of originalGlobals) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
  if (originalCreateObjectURL) Object.defineProperty(URL, 'createObjectURL', originalCreateObjectURL);
  else Reflect.deleteProperty(URL, 'createObjectURL');
  if (originalRevokeObjectURL) Object.defineProperty(URL, 'revokeObjectURL', originalRevokeObjectURL);
  else Reflect.deleteProperty(URL, 'revokeObjectURL');
});

test('restores a saved image from the matching conversation into the composer', async () => {
  const { result } = renderHook(() =>
    useComposerAttachments({
      selectedModel: { supportsImages: true },
      targetConversationId: savedImage.conversationId,
    }),
  );

  await dispatchSavedAttachment();
  await waitFor(() => assert.equal(result.current.pendingAttachments.length, 1));

  assert.equal(result.current.attachmentErrorKey, null);
  assert.equal(result.current.pendingAttachments[0]?.name, savedImage.fileName);
  assert.equal(result.current.pendingAttachments[0]?.kind, 'image');
  assert.equal(result.current.pendingAttachments[0]?.bytes.length, savedImage.sizeBytes);
  assert.deepEqual(harness.calls, [
    {
      method: 'storage.chat.attachments.read',
      parameters: {
        conversationId: savedImage.conversationId,
        messageId: savedImage.messageId,
        attachmentId: savedImage.attachmentId,
      },
    },
  ]);
  assert.equal(harness.pendingCounts.at(-1), 1);
});

test('rejects a saved image when the selected model does not support images', async () => {
  const { result } = renderHook(() =>
    useComposerAttachments({
      selectedModel: { supportsImages: false },
      targetConversationId: savedImage.conversationId,
    }),
  );

  await dispatchSavedAttachment();
  await waitFor(() => assert.equal(result.current.attachmentErrorKey, 'modelDoesNotSupportImages'));

  assert.equal(result.current.pendingAttachments.length, 0);
  assert.equal(harness.pendingCounts.includes(1), false);
  assert.equal(harness.calls.length, 1);
});

test('reports a missing saved attachment without changing the composer', async () => {
  harness.failRead = true;
  const { result } = renderHook(() =>
    useComposerAttachments({
      selectedModel: { supportsImages: true },
      targetConversationId: savedImage.conversationId,
    }),
  );

  await dispatchSavedAttachment();
  await waitFor(() => assert.equal(result.current.attachmentErrorKey, 'attachmentReadFailed'));

  assert.equal(result.current.pendingAttachments.length, 0);
  assert.equal(harness.pendingCounts.includes(1), false);
  assert.equal(harness.calls.length, 1);
});

test('ignores saved attachments dispatched for a different conversation', async () => {
  const { result } = renderHook(() =>
    useComposerAttachments({
      selectedModel: { supportsImages: true },
      targetConversationId: savedImage.conversationId,
    }),
  );

  await dispatchSavedAttachment('conversation-2');

  assert.equal(result.current.pendingAttachments.length, 0);
  assert.equal(harness.calls.length, 0);
});
