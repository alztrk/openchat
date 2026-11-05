import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const sidebarSource = readSource('src/features/chat/components/ConversationSidebar.tsx');
const preferencesSource = readSource('src/shared/lib/preferences/sidebarPreferences.ts');
const localePaths = ['de', 'en', 'es', 'fr', 'tr'].map((locale) => `src/shared/lib/i18n/locales/${locale}.json`);

function sourceBetween(startMarker, endMarker) {
  const start = sidebarSource.indexOf(startMarker);
  const end = sidebarSource.indexOf(endMarker, start);
  assert.ok(start >= 0, `Missing source marker: ${startMarker}`);
  assert.ok(end > start, `Missing end marker after: ${startMarker}`);
  return sidebarSource.slice(start, end);
}

test('custom folder preference writes resolve only after durable storage succeeds', () => {
  const writerStart = preferencesSource.indexOf('export async function writeCustomSidebarFolders(');
  const writerEnd = preferencesSource.indexOf('\n}', writerStart);
  assert.ok(writerStart >= 0 && writerEnd > writerStart);
  const writer = preferencesSource.slice(writerStart, writerEnd);
  const validationIndex = writer.indexOf("if (current.kind !== 'ready')");
  const storageWriteIndex = writer.indexOf('unifiedStorage.setPersistedItem(customFoldersKey, JSON.stringify(folders))');
  assert.ok(validationIndex >= 0 && validationIndex < storageWriteIndex);

  const persist = sourceBetween('const persistCustomFolders', 'const handleMoveConversationToFolder');
  const writeIndex = persist.indexOf('await writeCustomSidebarFolders(nextFolders)');
  const refIndex = persist.indexOf('customFoldersRef.current = nextFolders');
  const stateIndex = persist.indexOf("setCustomFoldersState({ kind: 'ready', folders: nextFolders })");
  assert.ok(writeIndex >= 0 && writeIndex < refIndex && refIndex < stateIndex);
  assert.match(persist, /catch \(cause\)[\s\S]*?showFeedback\('customFolderSaveFailed'\)/u);
});

test('create and rename keep their editor open after save failure and share the pending guard', () => {
  const create = sourceBetween('const handleCreateCustomFolder', 'const handleRenameCustomFolder');
  const rename = sourceBetween('const handleRenameCustomFolder', 'const handleDeleteCustomFolder');

  assert.match(create, /if \(customFolderMutationPendingRef\.current\) return;/u);
  assert.match(create, /const saved = await persistCustomFolders/u);
  assert.match(create, /if \(saved\)[\s\S]*?setIsCreatingFolder\(false\)[\s\S]*?else[\s\S]*?newFolderInputRef\.current\?\.focus\(\)/u);
  assert.match(rename, /if \(customFolderMutationPendingRef\.current\) return;/u);
  assert.match(rename, /const saved = await persistCustomFolders/u);
  assert.match(rename, /if \(saved\)[\s\S]*?setEditingFolderId\(null\)[\s\S]*?else[\s\S]*?data-custom-folder-editor/u);
});

test('failed folder preference save attempts to restore any service-side conversation move', () => {
  const locationChange = sourceBetween('const moveConversationToSidebarLocation', 'const toggleFolderExpanded');

  assert.match(locationChange, /if \(needsServiceMove && !\(await moveConversation\(conversation, targetProjectId\)\)\) return false;/u);
  assert.match(locationChange, /const foldersSaved = await handleMoveConversationToFolder\(conversation, targetFolderId\);/u);
  assert.match(locationChange, /if \(foldersSaved\) return true;[\s\S]*?restoreConversationLocation\(conversation, targetProjectId\)/u);
  assert.match(locationChange, /customFolderMoveRollbackFailed/u);
});

test('all supported locales explain folder save, pending, and move rollback failures', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    assert.equal(typeof locale.customFolderSaveFailed, 'string', `${localePath} is missing save failure feedback`);
    assert.equal(typeof locale.customFolderSaving, 'string', `${localePath} is missing save progress feedback`);
    assert.equal(typeof locale.customFolderSaveInProgress, 'string', `${localePath} is missing pending feedback`);
    assert.equal(
      typeof locale.customFolderMoveRollbackFailed,
      'string',
      `${localePath} is missing move rollback feedback`,
    );
  }
});

test('an in-progress folder save is announced as a status', () => {
  assert.match(sidebarSource, /isCustomFolderSaving \? \([\s\S]*?role="status"[\s\S]*?t\('customFolderSaving'\)/u);
});
