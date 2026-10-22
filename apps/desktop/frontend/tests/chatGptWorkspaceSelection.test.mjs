import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/modules/providers/data/chatGptWorkspaceSelection.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { requiresChatGptWorkspaceSelection } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('requires an explicit choice when workspaces exist but none is selected', () => {
  assert.equal(
    requiresChatGptWorkspaceSelection([
      { id: 'workspace-1', isSelected: false },
      { id: 'workspace-2', isSelected: false },
    ]),
    true,
  );
});

test('does not require a choice when one valid workspace is already selected', () => {
  assert.equal(
    requiresChatGptWorkspaceSelection([
      { id: 'workspace-1', isSelected: false },
      { id: 'workspace-2', isSelected: true },
    ]),
    false,
  );
});

test('does not report a workspace choice when the list has no usable workspaces', () => {
  assert.equal(requiresChatGptWorkspaceSelection([]), false);
  assert.equal(requiresChatGptWorkspaceSelection([null, {}, { id: '  ', isSelected: false }]), false);
});

test('all supported locales explain the missing ChatGPT workspace selection', () => {
  const localesDirectory = path.resolve('src/localization/locales');
  for (const locale of ['de', 'en', 'es', 'fr', 'tr']) {
    const translations = JSON.parse(fs.readFileSync(path.join(localesDirectory, `${locale}.json`), 'utf8'));
    assert.equal(typeof translations.chatGptWorkspaceSelectionTitle, 'string', `${locale} title`);
    assert.equal(typeof translations.chatGptWorkspaceSelectionRequired, 'string', `${locale} body`);
  }
});
