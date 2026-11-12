import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  expandWorkflowTemplate,
  exportWorkflowTemplatesJson,
  importWorkflowTemplatesJson,
  isWorkflowTemplateNameAvailable,
  parseWorkflowTemplate,
  parseWorkflowTemplates,
  workflowParameterNames,
  WorkflowTemplateValidationError,
} from '../src/features/model-catalog/data/workflowTemplateCore.ts';

const sampleValidTemplate = {
  id: 'a0000000-0000-4000-8000-000000000001',
  name: 'Code Review Workflow',
  description: 'Reviews code changes with standard security rules',
  startingPrompt: 'Review the following code in {{language}}:\n\n{{code}}\n\nCheck for {{criteria}}.',
  parameters: [
    { name: 'language', defaultValue: 'TypeScript', required: true },
    { name: 'code', required: true },
    { name: 'criteria', defaultValue: 'security and performance' },
  ],
  providerId: 'chatgpt_api',
  modelId: 'gpt-4o',
  systemPrompt: 'You are a senior code reviewer.',
  tools: ['code_interpreter', 'file_changes'],
  createdAt: 1700000000000,
  updatedAt: 1700000000000,
};

test('parses a valid parameterized workflow template', () => {
  const parsed = parseWorkflowTemplate(sampleValidTemplate);
  assert.equal(parsed.id, sampleValidTemplate.id);
  assert.equal(parsed.name, sampleValidTemplate.name);
  assert.equal(parsed.description, sampleValidTemplate.description);
  assert.equal(parsed.startingPrompt, sampleValidTemplate.startingPrompt);
  assert.equal(parsed.providerId, 'chatgpt_api');
  assert.equal(parsed.modelId, 'gpt-4o');
  assert.deepEqual(parsed.tools, ['code_interpreter', 'file_changes']);
  assert.equal(parsed.parameters.length, 3);
});

test('extracts parameter names from starting prompt', () => {
  const names = workflowParameterNames('Translate {{sourceText}} from {{sourceLang}} to {{targetLang}}.');
  assert.deepEqual(names, ['sourceText', 'sourceLang', 'targetLang']);
});

test('expands workflow template variables into the prompt', () => {
  const expanded = expandWorkflowTemplate('Hello {{name}}, welcome to {{service}}!', {
    name: 'Alice',
    service: 'OpenChat',
  });
  assert.equal(expanded, 'Hello Alice, welcome to OpenChat!');
});

test('handles missing variable values by replacing with empty string', () => {
  const expanded = expandWorkflowTemplate('Value: {{missing}}', {});
  assert.equal(expanded, 'Value: ');
});

test('checks workflow template name availability case-insensitively', () => {
  const templates = [parseWorkflowTemplate(sampleValidTemplate)];
  assert.equal(isWorkflowTemplateNameAvailable(templates, 'code review workflow', null), false);
  assert.equal(isWorkflowTemplateNameAvailable(templates, 'Code Review Workflow', sampleValidTemplate.id), true);
  assert.equal(isWorkflowTemplateNameAvailable(templates, 'New Unique Workflow', null), true);
});

test('rejects duplicate template IDs or names in list', () => {
  const duplicate = { ...sampleValidTemplate, id: 'a0000000-0000-4000-8000-000000000002' };
  assert.throws(
    () => parseWorkflowTemplates([sampleValidTemplate, duplicate]),
    WorkflowTemplateValidationError,
  );
});

test('rejects templates with invalid UUID, empty name, or oversized prompt', () => {
  assert.throws(
    () => parseWorkflowTemplate({ ...sampleValidTemplate, id: 'not-a-uuid' }),
    WorkflowTemplateValidationError,
  );
  assert.throws(
    () => parseWorkflowTemplate({ ...sampleValidTemplate, name: '' }),
    WorkflowTemplateValidationError,
  );
  assert.throws(
    () => parseWorkflowTemplate({ ...sampleValidTemplate, startingPrompt: '' }),
    WorkflowTemplateValidationError,
  );
});

test('exports workflow templates to JSON and imports them safely', () => {
  const original = [parseWorkflowTemplate(sampleValidTemplate)];
  const jsonText = exportWorkflowTemplatesJson(original);
  const { imported, skippedCount } = importWorkflowTemplatesJson(jsonText, []);

  assert.equal(skippedCount, 0);
  assert.equal(imported.length, 1);
  assert.equal(imported[0]?.name, sampleValidTemplate.name);
});

test('import skips duplicate templates when already existing', () => {
  const original = [parseWorkflowTemplate(sampleValidTemplate)];
  const jsonText = exportWorkflowTemplatesJson(original);
  const { imported, skippedCount } = importWorkflowTemplatesJson(jsonText, original);

  assert.equal(skippedCount, 1);
  assert.equal(imported.length, 0);
});

test('workflow template storage uses unified durable persistence', () => {
  const storageSource = readFileSync(
    new URL('../src/features/model-catalog/data/workflowTemplates.ts', import.meta.url),
    'utf8',
  );
  assert.match(storageSource, /new WorkflowTemplateReadError\('corrupt', encoded\)/u);
  assert.match(storageSource, /new WorkflowTemplateReadError\('unavailable'\)/u);
  assert.match(storageSource, /await unifiedStorage\.setPersistedItem\(storageKey/u);
  assert.match(storageSource, /await unifiedStorage\.removePersistedItem\(storageKey\)/u);
});
