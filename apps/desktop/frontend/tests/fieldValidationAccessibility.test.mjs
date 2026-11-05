import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const customProviderSource = readSource('src/features/provider-settings/components/CustomProviderDialog.tsx');
const createProjectSource = readSource('src/features/projects/components/CreateProjectDialog.tsx');
const projectMcpSource = readSource('src/features/projects/components/ProjectMcpPanel.tsx');
const archivePassphraseSource = readSource('src/features/app-settings/components/ArchivePassphraseDialog.tsx');
const conversationArchiveSource = readSource('src/features/app-settings/components/ConversationArchiveSettingsSection.tsx');
const profileArchiveSource = readSource('src/features/app-settings/components/ProfileArchiveSettingsSection.tsx');

test('custom provider validation is described by the field that failed and focuses that field', () => {
  assert.match(customProviderSource, /error=\{nameError\}/u);
  assert.match(customProviderSource, /error=\{baseUrlError\}/u);
  assert.match(customProviderSource, /error=\{reasoningEffortsError\}/u);
  assert.match(customProviderSource, /error=\{apiKeyError\}/u);
  assert.match(customProviderSource, /aria-describedby=\{httpApprovalError \? 'custom-provider-http-approval-error'/u);
  assert.match(customProviderSource, /nameInputRef\.current\?\.focus\(\)/u);
  assert.match(customProviderSource, /baseUrlInputRef\.current\?\.focus\(\)/u);
  assert.match(customProviderSource, /reasoningEffortsInputRef\.current\?\.focus\(\)/u);
  assert.match(customProviderSource, /document\.getElementById\('custom-provider-http-approval'\)\?\.focus\(\)/u);
  assert.match(customProviderSource, /apiKeyInputRef\.current\?\.focus\(\)/u);
});

test('project creation errors are linked to the project name or folder chooser', () => {
  assert.match(createProjectSource, /error=\{fieldError === 'name' \? t\('projectNameRequired'\) : undefined\}/u);
  assert.match(createProjectSource, /aria-describedby=\{fieldError === 'folder' \? 'create-project-folder-error'/u);
  assert.match(createProjectSource, /id="create-project-folder-error"[\s\S]*role="alert"/u);
  assert.match(createProjectSource, /nameInputRef\.current\?\.focus\(\)/u);
  assert.match(createProjectSource, /folderButtonRef\.current\?\.focus\(\)/u);
});

test('project MCP field validation errors are attached to their controls and receive focus', () => {
  assert.match(projectMcpSource, /error=\{idError\}/u);
  assert.match(projectMcpSource, /error=\{programError\}/u);
  assert.match(projectMcpSource, /error=\{endpointError\}/u);
  assert.match(projectMcpSource, /error=\{environmentError\}/u);
  assert.match(projectMcpSource, /project-mcp-environment-hint project-mcp-environment-error/u);
  assert.match(projectMcpSource, /idInputRef\.current\?\.focus\(\)/u);
  assert.match(projectMcpSource, /programInputRef\.current\?\.focus\(\)/u);
  assert.match(projectMcpSource, /endpointInputRef\.current\?\.focus\(\)/u);
  assert.match(projectMcpSource, /environmentTextareaRef\.current\?\.focus\(\)/u);
});

test('archive passphrase validation is linked to and focuses the relevant password field', () => {
  assert.match(archivePassphraseSource, /error=\{passphraseError\}/u);
  assert.match(archivePassphraseSource, /error=\{confirmationError\}/u);
  assert.match(archivePassphraseSource, /errorMessageField === 'passphrase'/u);
  assert.match(archivePassphraseSource, /onInputChange\(\)/u);
  assert.match(archivePassphraseSource, /passphraseInputRef\.current\?\.focus\(\)/u);
  assert.match(archivePassphraseSource, /confirmationInputRef\.current\?\.focus\(\)/u);
});

test('archive passphrase field errors are not repeated by the settings surface alert', () => {
  assert.match(conversationArchiveSource, /isPassphraseOpen &&[\s\S]*conversationArchiveInvalidPassphraseOrFile[\s\S]*conversationArchivePassphraseInvalid/u);
  assert.match(profileArchiveSource, /passphraseTarget !== null && errorKey === 'profileArchivePassphraseInvalid'/u);
  assert.match(conversationArchiveSource, /onInputChange=\{\(\) => setErrorKey\(null\)\}/u);
  assert.match(profileArchiveSource, /onInputChange=\{\(\) => setErrorKey\(null\)\}/u);
});
