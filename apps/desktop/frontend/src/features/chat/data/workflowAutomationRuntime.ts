import {
  completeWorkflowRun,
  nextWorkflowOccurrence,
  type WorkflowAutomation,
  type WorkflowRun,
} from './workflowAutomationCore';
import { readWorkflowAutomations, readWorkflowRuns, writeWorkflowAutomations, writeWorkflowRuns } from './workflowAutomations';

export interface WorkflowExecutionResult {
  conversationId: string;
  completedSteps: number;
}

export type WorkflowExecutor = (
  workflow: WorkflowAutomation,
  onStepCompleted: (completedSteps: number, conversationId: string) => Promise<void>,
) => Promise<WorkflowExecutionResult>;

const inFlightWorkflowIds = new Set<string>();
let scheduleCheck: Promise<number> | null = null;

export async function runWorkflowAutomation(
  workflowId: string,
  scheduledAt: number | null,
  execute: WorkflowExecutor,
): Promise<WorkflowRun | null> {
  if (inFlightWorkflowIds.has(workflowId)) return null;
  inFlightWorkflowIds.add(workflowId);
  try {
    const workflows = readWorkflowAutomations();
    const workflow = workflows.find((item) => item.id === workflowId);
    if (!workflow) return null;
    const runs = readWorkflowRuns();
    if (runs.some((run) => run.workflowId === workflowId && run.status === 'running')) return null;

    const now = Date.now();
    const run: WorkflowRun = {
      id: crypto.randomUUID(),
      workflowId,
      workflowName: workflow.name,
      status: 'running',
      scheduledAt,
      startedAt: now,
      completedSteps: 0,
      totalSteps: workflow.steps.length,
    };
    await writeWorkflowRuns([...runs, run]);

    if (scheduledAt !== null) {
      const nextRunAt = nextWorkflowOccurrence(workflow.schedule, now);
      await writeWorkflowAutomations(
        workflows.map((item) => (item.id === workflowId ? { ...item, nextRunAt, updatedAt: now } : item)),
      );
    }

    try {
      const result = await execute(workflow, async (completedSteps, conversationId) => {
        const currentRuns = readWorkflowRuns();
        await writeWorkflowRuns(
          currentRuns.map((item) =>
            item.id === run.id
              ? { ...item, completedSteps, conversationId }
              : item,
          ),
        );
      });
      const currentRuns = readWorkflowRuns();
      const latestRun = currentRuns.find((item) => item.id === run.id) ?? run;
      const completed = completeWorkflowRun(latestRun, 'completed', Date.now(), result.completedSteps, result.conversationId);
      await writeWorkflowRuns(currentRuns.map((item) => (item.id === run.id ? completed : item)));
      return completed;
    } catch {
      const currentRuns = readWorkflowRuns();
      const latestRun = currentRuns.find((item) => item.id === run.id) ?? run;
      const failed = completeWorkflowRun(latestRun, 'failed', Date.now(), latestRun.completedSteps, latestRun.conversationId, 'A workflow step did not complete.');
      await writeWorkflowRuns(currentRuns.map((item) => (item.id === run.id ? failed : item)));
      return failed;
    }
  } finally {
    inFlightWorkflowIds.delete(workflowId);
  }
}

export function runDueWorkflowAutomations(execute: WorkflowExecutor, now = Date.now()): Promise<number> {
  if (scheduleCheck) return scheduleCheck;
  scheduleCheck = runDueWorkflows(execute, now).finally(() => {
    scheduleCheck = null;
  });
  return scheduleCheck;
}

export async function retryWorkflowRun(runId: string, execute: WorkflowExecutor): Promise<WorkflowRun | null> {
  const run = readWorkflowRuns().find((item) => item.id === runId);
  if (!run || (run.status !== 'failed' && run.status !== 'interrupted')) return null;
  return runWorkflowAutomation(run.workflowId, null, execute);
}

export async function markInterruptedWorkflowRuns(): Promise<number> {
  const runs = readWorkflowRuns();
  const now = Date.now();
  let interruptedCount = 0;
  const updated = runs.map((run) => {
    if (run.status !== 'running') return run;
    interruptedCount += 1;
    return completeWorkflowRun(run, 'interrupted', now, run.completedSteps, run.conversationId, 'OpenChat closed while this run was active.');
  });
  if (interruptedCount > 0) await writeWorkflowRuns(updated);
  return interruptedCount;
}

async function runDueWorkflows(execute: WorkflowExecutor, now: number): Promise<number> {
  let started = 0;
  const due = readWorkflowAutomations()
    .filter((workflow) => workflow.enabled && workflow.nextRunAt <= now)
    .sort((left, right) => left.nextRunAt - right.nextRunAt);
  for (const workflow of due) {
    const run = await runWorkflowAutomation(workflow.id, workflow.nextRunAt, execute);
    if (run !== null) started += 1;
  }
  return started;
}
