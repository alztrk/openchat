import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import type { ChatRoute } from '@/modules/providers/data/routes';
import { localServiceClient } from '@/platform/local-service';
import { isObject } from '@/shared/utils/json';

export type SecurityScanStatus =
  | 'queued'
  | 'running'
  | 'waiting_provider'
  | 'paused'
  | 'interrupted'
  | 'completed'
  | 'cancelled'
  | 'failed';

export interface SecurityScan {
  id: string;
  conversationId: string;
  projectId: string;
  projectName: string;
  providerId: string | null;
  modelId: string | null;
  selectedRoots: string[];
  publicWebResearch: boolean;
  status: SecurityScanStatus;
  activeRunId: string | null;
  retryCount: number;
  nextRetryAtUnixMs: number | null;
  createdAtUnixMs: number;
  updatedAtUnixMs: number;
  finishedAtUnixMs: number | null;
  failureCode: string | null;
}

export const securityScanObjective =
  'Perform a thorough, evidence-based security audit of the selected project folders. Inspect the project first, create an ordered scan plan, complete every relevant step, adapt the plan when evidence requires it, and report verified findings directly to the user.';

export async function listSecurityScans(): Promise<SecurityScan[]> {
  const response = await localServiceClient.call('security.scans.list');
  if (!Array.isArray(response.scans) || !response.scans.every(isSecurityScan)) {
    throw new Error('The saved security scans are invalid.');
  }
  return response.scans;
}

export async function createSecurityScan(input: {
  id: string;
  conversationId: string;
  project: ChatProject;
  route: ChatRoute;
  selectedRoots: string[];
  publicWebResearch: boolean;
}): Promise<SecurityScan> {
  const response = await localServiceClient.call('security.scans.create', {
    input: {
      scanId: input.id,
      conversationId: input.conversationId,
      projectId: input.project.id,
      providerId: input.route.providerId,
      modelId: input.route.modelId,
      connectionId: input.route.connectionId ?? null,
      workspaceId: input.route.workspaceId ?? null,
      apiKeyConnectionId: input.route.apiKeyConnectionId ?? null,
      selectedRoots: input.selectedRoots,
      publicWebResearch: input.publicWebResearch,
    },
  });
  if (!isSecurityScan(response.scan)) throw new Error('The new security scan is invalid.');
  return response.scan;
}

export async function scheduleSecurityScanRetry(
  scanId: string,
  retryCount: number,
  nextRetryAtUnixMs: number,
): Promise<void> {
  const response = await localServiceClient.call('security.scans.retry.schedule', {
    input: { scanId, retryCount, nextRetryAtUnixMs },
  });
  if (response.saved !== true) throw new Error('The security scan retry could not be scheduled.');
}

export async function cancelSecurityScan(scanId: string): Promise<void> {
  const response = await localServiceClient.call('security.scans.cancel', { input: { scanId } });
  if (response.cancelled !== true) throw new Error('The security scan could not be stopped.');
}

export async function deleteSecurityScan(scanId: string): Promise<void> {
  const response = await localServiceClient.call('security.scans.delete', { input: { scanId } });
  if (response.deleted !== true) throw new Error('The security scan could not be deleted.');
}

export function isSecurityScan(value: unknown): value is SecurityScan {
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    typeof value.conversationId !== 'string' ||
    typeof value.projectId !== 'string' ||
    typeof value.projectName !== 'string' ||
    !isNullableString(value.providerId) ||
    !isNullableString(value.modelId) ||
    !Array.isArray(value.selectedRoots) ||
    !value.selectedRoots.every((root) => typeof root === 'string') ||
    typeof value.publicWebResearch !== 'boolean' ||
    !isSecurityScanStatus(value.status) ||
    !isNullableString(value.activeRunId) ||
    typeof value.retryCount !== 'number' ||
    !Number.isSafeInteger(value.retryCount) ||
    value.retryCount < 0 ||
    !isNullableSafeInteger(value.nextRetryAtUnixMs) ||
    !isSafeInteger(value.createdAtUnixMs) ||
    !isSafeInteger(value.updatedAtUnixMs) ||
    !isNullableSafeInteger(value.finishedAtUnixMs) ||
    !isNullableString(value.failureCode)
  ) {
    return false;
  }
  return true;
}

function isSecurityScanStatus(value: unknown): value is SecurityScanStatus {
  return (
    value === 'queued' ||
    value === 'running' ||
    value === 'waiting_provider' ||
    value === 'paused' ||
    value === 'interrupted' ||
    value === 'completed' ||
    value === 'cancelled' ||
    value === 'failed'
  );
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function isNullableSafeInteger(value: unknown): value is number | null {
  return value === null || isSafeInteger(value);
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
