import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

function compileTsModule(source) {
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return import(`data:text/javascript;base64,${Buffer.from(output).toString('base64')}`);
}

const messageDataSource = fs
  .readFileSync(path.resolve('src/modules/conversations/components/message/messageData.ts'), 'utf8')
  .replace(
    "import { type ChatFileChange, parseChatFileChange } from '@/modules/conversations/data/chatFileChanges';",
    'const parseChatFileChange = () => null;',
  );

const chatStoreUtilsSource = fs
  .readFileSync(path.resolve('src/modules/conversations/stores/chatStoreUtils.ts'), 'utf8')
  .replace("import { isObject } from '@/shared/utils/json';", '')
  .replace(
    "export { isObject, numberOrNull } from '@/shared/utils/json';",
    `export function numberOrNull(value) {
      return typeof value === 'number' && Number.isFinite(value) ? value : null;
    }
    export function isObject(value) {
      return typeof value === 'object' && value !== null && !Array.isArray(value);
    }`,
  );

const { buildAssistantTimeline, parseReasoningSummaries, parseToolActivities } =
  await compileTsModule(messageDataSource);
const { applyStreamEvent } = await compileTsModule(chatStoreUtilsSource);

test('stamps and preserves reasoning summary timeline offsets across multi-round Turkish UTF-8 streams', () => {
  const route = { conversationId: 'conv_1', providerId: 'opencode', modelId: 'kimi-k2.5' };
  const encoder = new TextEncoder();

  let message = applyStreamEvent(
    {
      name: 'chat.reasoning.delta',
      data: {
        messageId: 'msg_1',
        createdAtUnixMs: 1_000,
        content: '',
        reasoningGroups: [{ id: 'reasoning_msg_1', content: 'Önce dizin yapısını inceleyelim.', isComplete: false }],
      },
    },
    null,
    route,
  );

  const roundOneText = 'Proje yapısını inceliyorum.';
  message = applyStreamEvent(
    {
      name: 'chat.delta',
      data: {
        messageId: 'msg_1',
        createdAtUnixMs: 1_010,
        content: roundOneText,
        reasoningGroups: [
          {
            id: 'reasoning_msg_1',
            content: 'Önce dizin yapısını inceleyelim.',
            elapsedMicroseconds: 450_000,
            isComplete: true,
          },
        ],
      },
    },
    message,
    route,
  );

  const roundOneBytes = encoder.encode(roundOneText).byteLength;
  message = applyStreamEvent(
    {
      name: 'chat.tool.updated',
      data: {
        messageId: 'msg_1',
        createdAtUnixMs: 1_020,
        content: roundOneText,
        toolActivity: {
          callId: 'call_1',
          name: 'list_directory',
          arguments: { path: '.' },
          output: 'apps/\nservices/',
          assistantTextBeforeByteOffset: roundOneBytes,
          status: 'completed',
        },
      },
    },
    message,
    route,
  );

  message = applyStreamEvent(
    {
      name: 'chat.reasoning.delta',
      data: {
        messageId: 'msg_1',
        createdAtUnixMs: 1_030,
        content: roundOneText,
        reasoningGroups: [
          {
            id: 'reasoning_msg_1',
            content: 'Önce dizin yapısını inceleyelim.',
            elapsedMicroseconds: 450_000,
            isComplete: true,
          },
          {
            id: 'reasoning_msg_1_2',
            content: 'Şimdi tarama planını oluşturacağım.',
            elapsedMicroseconds: 320_000,
            isComplete: true,
          },
        ],
      },
    },
    message,
    route,
  );

  const fullText = `${roundOneText}\n\nŞimdi tarama planımı kuruyorum.`;
  message = applyStreamEvent(
    {
      name: 'chat.delta',
      data: {
        messageId: 'msg_1',
        createdAtUnixMs: 1_040,
        content: fullText,
      },
    },
    message,
    route,
  );

  assert.ok(message);
  const summaries = parseReasoningSummaries(message.reasoningSummaries);
  const activities = parseToolActivities(message.toolActivities);
  assert.equal(summaries.length, 2);
  assert.equal(summaries[0]?.assistantTextBeforeByteOffset, 0);
  assert.equal(summaries[0]?.toolActivityCountBefore, 0);
  assert.equal(summaries[1]?.assistantTextBeforeByteOffset, roundOneBytes);
  assert.equal(summaries[1]?.toolActivityCountBefore, 1);

  const segments = buildAssistantTimeline(
    message.content,
    { visibleContent: message.content, reasoningContent: '' },
    summaries,
    activities,
  );

  assert.deepEqual(
    segments.map((segment) =>
      segment.kind === 'text'
        ? { kind: 'text', content: segment.content.trim() }
        : segment.kind === 'reasoning'
          ? { kind: 'reasoning', ids: segment.summaries.map((item) => item.id) }
          : { kind: 'tools', calls: segment.activities.map((item) => item.callId) },
    ),
    [
      { kind: 'reasoning', ids: ['reasoning_msg_1'] },
      { kind: 'text', content: 'Proje yapısını inceliyorum.' },
      { kind: 'tools', calls: ['call_1'] },
      { kind: 'reasoning', ids: ['reasoning_msg_1_2'] },
      { kind: 'text', content: 'Şimdi tarama planımı kuruyorum.' },
    ],
  );
});

test('snaps mid-character UTF-8 byte offsets safely withoutreplacement corruption', () => {
  const content = 'Güvenlik özeti hazır.';
  const segments = buildAssistantTimeline(
    content,
    { visibleContent: content, reasoningContent: '' },
    [],
    [
      {
        callId: 'call_utf8',
        name: 'read_file',
        arguments: { path: 'README.md' },
        output: 'ok',
        // Offset 2 lands on the continuation byte of "ü" in UTF-8 ("G" = 1 byte, "ü" = 2 bytes).
        assistantTextBeforeByteOffset: 2,
        targetPath: 'README.md',
        fileChanges: [],
        fileChangesError: null,
        recoveryCheckpointId: null,
        status: 'completed',
      },
    ],
  );

  const recombined = segments
    .filter((segment) => segment.kind === 'text')
    .map((segment) => segment.content)
    .join('');
  assert.equal(recombined, content);
  assert.equal(recombined.includes('\uFFFD'), false);
});

test('MessageItem renders reasoning summaries across all providers without a ChatGPT-only gate', () => {
  const messageItemSource = fs.readFileSync(
    path.resolve('src/modules/conversations/components/MessageItem.tsx'),
    'utf8',
  );
  assert.equal(messageItemSource.includes("providerId === 'chatgpt'"), false);
  assert.equal(messageItemSource.includes("providerId === 'chatgpt_api'"), false);
});
