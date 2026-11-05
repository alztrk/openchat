import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');

const labelCases = [
  {
    filePath: 'src/features/provider-settings/components/ApiKeyProvidersSection.tsx',
    label: 'providerCredentialStatusLoading',
    incorrectLabel: 'modelsLoading',
  },
  {
    filePath: 'src/features/chat/components/ConversationSidebarToolbar.tsx',
    label: 'assistantProfilesLoading',
    incorrectLabel: 'modelsLoading',
  },
  {
    filePath: 'src/features/model-catalog/components/McpEnvModal.tsx',
    label: 'mcpCredentialStatusLoading',
    incorrectLabel: 'modelsLoading',
  },
  {
    filePath: 'src/features/model-catalog/components/PromptTemplatesTab.tsx',
    label: 'promptTemplatesLoading',
    incorrectLabel: 'modelsLoading',
  },
  {
    filePath: 'src/features/projects/components/ProjectMcpCredentialDialog.tsx',
    label: 'projectMcpCredentialStatusLoading',
    incorrectLabel: 'historyLoading',
  },
  {
    filePath: 'src/features/projects/components/ProjectMemoryPanel.tsx',
    label: 'projectMemoryLoading',
    incorrectLabel: 'historyLoading',
  },
  {
    filePath: 'src/features/projects/components/ProjectInstructionsPanel.tsx',
    label: 'projectInstructionsLoading',
    incorrectLabel: 'historyLoading',
  },
  {
    filePath: 'src/features/projects/components/ProjectSkillsPanel.tsx',
    label: 'projectSkillsLoading',
    incorrectLabel: 'historyLoading',
  },
  {
    filePath: 'src/features/projects/components/ProjectToolPermissionsPanel.tsx',
    label: 'projectToolRulesSummaryLoading',
    incorrectLabel: 'historyLoading',
  },
  {
    filePath: 'src/features/app-settings/components/StatisticsSettingsSection.tsx',
    label: 'statisticsLoading',
    incorrectLabel: 'historyLoading',
  },
];

test('loading surfaces use domain-specific labels instead of model or conversation loading copy', () => {
  for (const { filePath, label, incorrectLabel } of labelCases) {
    const source = readSource(filePath);
    assert.ok(source.includes(`t('${label}')`), `${filePath} should use ${label}`);
    assert.ok(!source.includes(`t('${incorrectLabel}')`), `${filePath} should not use ${incorrectLabel}`);
  }
});

test('OAuth connection loading has a specific label while model loading remains model-specific', () => {
  const source = readSource('src/features/provider-settings/components/ProviderSettingsSection.tsx');
  assert.match(source, /request === 'loading'[\s\S]*?t\('oauthConnectionsLoading'\)/u);
  assert.match(source, /modelStatus === 'loading'[\s\S]*?t\('modelsLoading'\)/u);
});
