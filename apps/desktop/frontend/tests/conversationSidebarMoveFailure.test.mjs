import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const sidebarSource = readSource('src/features/chat/components/ConversationSidebar.tsx');
const actionsSource = readSource('src/features/chat/hooks/useConversationSidebarActions.ts');

function sourceBetween(startMarker, endMarker) {
  const start = sidebarSource.indexOf(startMarker);
  const end = sidebarSource.indexOf(endMarker, start);
  assert.ok(start >= 0, `Missing source marker: ${startMarker}`);
  assert.ok(end > start, `Missing end marker after: ${startMarker}`);
  return sidebarSource.slice(start, end);
}

test('failed project and custom-folder drops preserve current folder metadata', () => {
  const projectDrop = sourceBetween('const dropConversationToProject', 'const dropConversationToFolder');
  const folderDrop = sourceBetween('const dropConversationToFolder', 'const dropConversationToChats');

  assert.match(projectDrop, /const moved = await moveConversation\(conversation, projectId\);\s*if \(!moved\) return;/u);
  assert.match(projectDrop, /if \(!moved\) return;[\s\S]*?handleMoveConversationToFolder\(conversation, null\)/u);
  assert.match(folderDrop, /const moved = await moveConversation\(conversation, null\);\s*if \(!moved\) return;/u);
  assert.match(folderDrop, /if \(!moved\) return;[\s\S]*?handleMoveConversationToFolder\(conversation, folderId\)/u);
});

test('failed project removal leaves a conversation in its current sidebar location', () => {
  const chatsDrop = sourceBetween('const dropConversationToChats', 'const dropConversationToPinned');

  assert.match(chatsDrop, /const moved = await moveConversation\(conversation, null\);\s*if \(!moved\) return;/u);
  assert.match(chatsDrop, /if \(!moved\) return;[\s\S]*?handleMoveConversationToFolder\(conversation, null\)/u);
});

test('sidebar move actions report whether the service mutation succeeded', () => {
  assert.match(
    actionsSource,
    /const moveConversation = async \(conversation: ChatConversation, projectId: string \| null\): Promise<boolean> => \{\s*if \(conversation\.projectId === projectId && !conversation\.isPinned\) return true;\s*return runMutation\(\(\) => moveConversations\(\[conversation\.id\], projectId\), 'projectMoveFailed'\);/u,
  );
});
