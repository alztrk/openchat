import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const composerSource = readSource('src/features/chat/components/ChatComposer.tsx');
const dialogSource = readSource('src/features/chat/components/AttachmentLibraryDialog.tsx');
const localePaths = ['de', 'en', 'es', 'fr', 'tr'].map((locale) => `src/shared/lib/i18n/locales/${locale}.json`);

test('the composer exposes separate local-file and saved-library attachment actions', () => {
  assert.match(composerSource, /aria-label=\{t\('attachmentChooseFile'\)\}[\s\S]*?filePickerRef\.current\?\.click\(\)/u);
  assert.match(
    composerSource,
    /aria-label=\{t\('attachmentChooseFromLibrary'\)\}[\s\S]*?onClick=\{\(\) => setIsAttachmentLibraryOpen\(true\)\}/u,
  );
  assert.match(composerSource, /<FolderOpen aria-hidden="true" size=\{18\} \/>/u);
});

test('file and library attachment controls share model, send, read, and placeholder guards', () => {
  assert.match(
    composerSource,
    /const isAttachmentSelectionDisabled =\s*isTargetPlaceholder \|\|\s*pickerModels\.length === 0 \|\|\s*isSending \|\|\s*isReadingAttachments \|\|\s*isUpdatingConversationModel;/u,
  );
  assert.equal((composerSource.match(/disabled=\{isAttachmentSelectionDisabled\}/gu) ?? []).length, 4);
});

test('the saved-library dialog adds a selected item to the composer and closes only on success', () => {
  assert.match(composerSource, /onSelect=\{\(item\) => addExistingAttachments\(\[item\]\)\}/u);
  assert.match(composerSource, /open=\{isAttachmentLibraryOpen\}/u);
  assert.match(
    dialogSource,
    /if \(await onSelect\(item\)\) onOpenChange\(false\);\s*else setSelectionFailed\(true\);/u,
  );
});

test('all supported locales provide the saved-library action name', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    assert.equal(typeof locale.attachmentChooseFromLibrary, 'string', `${localePath} is missing the library action name`);
  }
});
