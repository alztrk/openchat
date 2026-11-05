import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const providerSource = readFileSync(path.resolve('src/features/provider-settings/components/ApiKeyProviderCards.tsx'), 'utf8');
const projectMcpSource = readFileSync(path.resolve('src/features/projects/components/ProjectMcpPanel.tsx'), 'utf8');

test('provider API key credential input submits through the form without duplicate click handling', () => {
  const credentialInput = providerSource.slice(providerSource.indexOf('export function CredentialInput('));

  assert.match(credentialInput, /<form[\s\S]*?onSubmit=\{\(event\) => \{[\s\S]*?event\.preventDefault\(\)[\s\S]*?onSave\(\)/u);
  assert.match(credentialInput, /<Input[\s\S]*?type=\{isVisible \? 'text' : 'password'\}/u);
  assert.match(credentialInput, /type="submit"/u);
  assert.doesNotMatch(credentialInput, /onClick=\{onSave\}/u);
});

test('project MCP editor uses native form submission and keeps cancel from submitting', () => {
  const editor = projectMcpSource.slice(projectMcpSource.indexOf('function ProjectMcpServerEditor('));

  assert.match(editor, /<form[\s\S]*?onSubmit=\{\(event\) => \{[\s\S]*?event\.preventDefault\(\)[\s\S]*?submit\(\)/u);
  assert.match(editor, /<Button onClick=\{onCancel\} type="button"/u);
  assert.match(editor, /<Button type="submit" variant="primary">/u);
  assert.doesNotMatch(editor, /<Button onClick=\{submit\}/u);
});
