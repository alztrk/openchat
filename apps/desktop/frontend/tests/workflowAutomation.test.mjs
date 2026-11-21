import assert from 'node:assert/strict';
import test from 'node:test';

import {
  completeWorkflowRun,
  nextWorkflowOccurrence,
  parseWorkflowAutomation,
  parseWorkflowAutomations,
  parseWorkflowRuns,
  WorkflowAutomationValidationError,
} from '../src/features/chat/data/workflowAutomationCore.ts';

const baseWorkflow = {
  id: 'a0000000-0000-4000-8000-000000000001',
  name: 'Weekly report',
  enabled: true,
  schedule: { cadence: 'weekly', weekday: 1, hour: 9, minute: 30 },
  steps: [
    { id: 'a0000000-0000-4000-8000-000000000002', name: 'Collect', prompt: 'Collect this week’s updates.' },
    { id: 'a0000000-0000-4000-8000-000000000003', name: 'Summarize', prompt: 'Write a concise summary.' },
  ],
  inputDefaults: {},
  providerId: 'openai',
  modelId: 'model-1',
  createdAt: 1_700_000_000_000,
  updatedAt: 1_700_000_000_000,
  nextRunAt: 1_700_000_000_000,
};

test('parses ordered recurring workflows and rejects duplicate IDs or names', () => {
  const workflow = parseWorkflowAutomation(baseWorkflow);
  assert.deepEqual(workflow.steps.map((step) => step.name), ['Collect', 'Summarize']);
  assert.throws(() => parseWorkflowAutomations([baseWorkflow, { ...baseWorkflow, id: 'a0000000-0000-4000-8000-000000000004' }]), WorkflowAutomationValidationError);
  assert.throws(() => parseWorkflowAutomations([baseWorkflow, { ...baseWorkflow, id: 'a0000000-0000-4000-8000-000000000004', name: 'weekly REPORT' }]), WorkflowAutomationValidationError);
});

test('calculates next daily occurrence in local time without running a due time twice', () => {
  const now = new Date(2026, 1, 3, 8, 15, 0, 0).getTime();
  const schedule = { cadence: 'daily', hour: 9, minute: 30 };
  assert.equal(nextWorkflowOccurrence(schedule, now), new Date(2026, 1, 3, 9, 30).getTime());
  assert.equal(nextWorkflowOccurrence(schedule, new Date(2026, 1, 3, 9, 30).getTime()), new Date(2026, 1, 4, 9, 30).getTime());
});

test('calculates the next weekly occurrence and skips the current day after its run time', () => {
  const mondayMorning = new Date(2026, 1, 2, 8, 0, 0, 0).getTime();
  const schedule = { cadence: 'weekly', weekday: 1, hour: 9, minute: 0 };
  assert.equal(nextWorkflowOccurrence(schedule, mondayMorning), new Date(2026, 1, 2, 9, 0).getTime());
  assert.equal(nextWorkflowOccurrence(schedule, new Date(2026, 1, 2, 9, 0).getTime()), new Date(2026, 1, 9, 9, 0).getTime());
});

test('bounds run records and preserves terminal execution details', () => {
  const run = {
    id: 'a0000000-0000-4000-8000-000000000005',
    workflowId: baseWorkflow.id,
    workflowName: baseWorkflow.name,
    status: 'running',
    scheduledAt: 1_700_000_000_000,
    startedAt: 1_700_000_000_010,
    completedSteps: 1,
    totalSteps: 2,
  };
  const complete = completeWorkflowRun(run, 'completed', 1_700_000_000_100, 2, 'conversation-1');
  assert.equal(complete.status, 'completed');
  assert.equal(complete.completedSteps, 2);
  assert.equal(parseWorkflowRuns([complete])[0]?.conversationId, 'conversation-1');
  assert.throws(() => completeWorkflowRun(run, 'completed', 1_700_000_000_100, 3), WorkflowAutomationValidationError);
});

test('rejects invalid schedules, missing ordered steps, and unsafe step sizes', () => {
  assert.throws(() => parseWorkflowAutomation({ ...baseWorkflow, schedule: { cadence: 'weekly', weekday: 7, hour: 10, minute: 0 } }), WorkflowAutomationValidationError);
  assert.throws(() => parseWorkflowAutomation({ ...baseWorkflow, steps: [] }), WorkflowAutomationValidationError);
  assert.throws(() => parseWorkflowAutomation({ ...baseWorkflow, steps: [{ ...baseWorkflow.steps[0], prompt: 'x'.repeat(32 * 1024 + 1) }] }), WorkflowAutomationValidationError);
});
