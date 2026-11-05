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

test('service moves succeed before folder metadata changes and failed metadata saves restore location', () => {
  const moveToLocation = sourceBetween('const moveConversationToSidebarLocation', 'const toggleFolderExpanded');

  assert.match(moveToLocation, /if \(needsServiceMove && !\(await moveConversation\(conversation, targetProjectId\)\)\) return false;/u);
  assert.match(moveToLocation, /const foldersSaved = await handleMoveConversationToFolder\(conversation, targetFolderId\);/u);
  assert.match(moveToLocation, /if \(foldersSaved\) return true;[\s\S]*?restoreConversationLocation\(conversation, targetProjectId\)/u);
  assert.match(moveToLocation, /showFeedback\('customFolderMoveRollbackFailed'\)/u);
});

test('all drop destinations use the guarded location transaction', () => {
  const projectDrop = sourceBetween('const dropConversationToProject', 'const dropConversationToFolder');
  const folderDrop = sourceBetween('const dropConversationToFolder', 'const dropConversationToChats');
  const chatsDrop = sourceBetween('const dropConversationToChats', 'const dropConversationToPinned');

  assert.match(projectDrop, /await moveConversationToSidebarLocation\(conversation, projectId, null\)/u);
  assert.match(folderDrop, /await moveConversationToSidebarLocation\(conversation, null, folderId\)/u);
  assert.match(chatsDrop, /await moveConversationToSidebarLocation\(conversation, null, null\)/u);
});

test('sidebar move actions report whether the service mutation succeeded', () => {
  assert.match(
    actionsSource,
    /const moveConversation = async \(conversation: ChatConversation, projectId: string \| null\): Promise<boolean> => \{\s*if \(conversation\.projectId === projectId && !conversation\.isPinned\) return true;\s*return runMutation\(\(\) => moveConversations\(\[conversation\.id\], projectId\), 'projectMoveFailed'\);/u,
  );
});
