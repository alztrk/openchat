import { localServiceClient } from '@/platform/local-service';
import { isObject } from '@/shared/utils/json';
import { loadCopilotOAuthStatus, loadCopilotUsage } from './copilotOAuth';

export const OAUTH_PLANS_UPDATED_EVENT = 'openchat:oauth-plans-updated';
export const OAUTH_PLAN_CHECK_INTERVAL_MS = 15 * 60 * 1000;
export const OAUTH_PLAN_FOCUS_COOLDOWN_MS = 3 * 60 * 1000;

export interface OAuthPlanRefreshSummary {
  readonly chatGptConnections: readonly unknown[];
  readonly copilotConnectionId: string | null;
  readonly copilotPlanType: string | null;
  readonly planOrAuthChanged: boolean;
}

let inFlightRefresh: Promise<OAuthPlanRefreshSummary> | null = null;
let lastCompletedCheckAtUnixMs = 0;
let cachedCopilotPlanType: string | null = null;
let cachedCopilotConnectionId: string | null = null;
let hasObservedCopilotPlan = false;

export function getCachedCopilotPlanType(): string | null {
  return cachedCopilotPlanType;
}

export function shouldRefreshOAuthPlansOnFocus(now = Date.now()): boolean {
  return now - lastCompletedCheckAtUnixMs >= OAUTH_PLAN_FOCUS_COOLDOWN_MS;
}

export function refreshOAuthPlans(): Promise<OAuthPlanRefreshSummary> {
  if (inFlightRefresh !== null) {
    return inFlightRefresh;
  }
  const task = performOAuthPlanRefresh().finally(() => {
    if (inFlightRefresh === task) {
      inFlightRefresh = null;
    }
  });
  inFlightRefresh = task;
  return task;
}

export function subscribeToOAuthPlanUpdates(listener: (summary: OAuthPlanRefreshSummary) => void): () => void {
  if (typeof window === 'undefined') {
    return () => undefined;
  }
  const handler = (event: Event) => {
    if (event instanceof CustomEvent && isRefreshSummary(event.detail)) {
      listener(event.detail);
    }
  };
  window.addEventListener(OAUTH_PLANS_UPDATED_EVENT, handler);
  return () => window.removeEventListener(OAUTH_PLANS_UPDATED_EVENT, handler);
}

async function performOAuthPlanRefresh(): Promise<OAuthPlanRefreshSummary> {
  const [chatGptResult, copilotResult] = await Promise.all([refreshChatGptPlans(), refreshCopilotPlan()]);
  lastCompletedCheckAtUnixMs = Date.now();
  const summary: OAuthPlanRefreshSummary = {
    chatGptConnections: chatGptResult.connections,
    copilotConnectionId: copilotResult.connectionId,
    copilotPlanType: copilotResult.planType,
    planOrAuthChanged: chatGptResult.changed || copilotResult.changed,
  };
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent(OAUTH_PLANS_UPDATED_EVENT, { detail: summary }));
  }
  return summary;
}

async function refreshChatGptPlans(): Promise<{ connections: readonly unknown[]; changed: boolean }> {
  try {
    const initialResponse = await localServiceClient.call('chatgpt.connections.list');
    const initialConnections = Array.isArray(initialResponse.connections) ? initialResponse.connections : [];
    const hasActiveConnection = initialConnections.some((item) => isObject(item) && item.authStatus === 'active');
    if (!hasActiveConnection) {
      return { connections: initialConnections, changed: false };
    }
    const beforeSignature = serializeConnectionPlanState(initialConnections);
    const refreshedResponse = await localServiceClient.call('chatgpt.connections.refresh_plans', {}, 30_000);
    const refreshedConnections = Array.isArray(refreshedResponse.connections)
      ? refreshedResponse.connections
      : initialConnections;
    const afterSignature = serializeConnectionPlanState(refreshedConnections);
    return {
      connections: refreshedConnections,
      changed: beforeSignature !== afterSignature,
    };
  } catch {
    return { connections: [], changed: false };
  }
}

async function refreshCopilotPlan(): Promise<{
  connectionId: string | null;
  planType: string | null;
  changed: boolean;
}> {
  try {
    const status = await loadCopilotOAuthStatus();
    if (!status.connected || status.connection === null || status.connection.authStatus !== 'active') {
      const changed = hasObservedCopilotPlan && (cachedCopilotConnectionId !== null || cachedCopilotPlanType !== null);
      cachedCopilotConnectionId = null;
      cachedCopilotPlanType = null;
      hasObservedCopilotPlan = true;
      return { connectionId: null, planType: null, changed };
    }
    const usage = await loadCopilotUsage();
    const connectionChanged = cachedCopilotConnectionId !== status.connection.id;
    const nextPlanType = usage.planType;
    const changed = hasObservedCopilotPlan && (connectionChanged || cachedCopilotPlanType !== nextPlanType);
    cachedCopilotConnectionId = status.connection.id;
    cachedCopilotPlanType = nextPlanType;
    hasObservedCopilotPlan = true;
    return { connectionId: status.connection.id, planType: nextPlanType, changed };
  } catch {
    return {
      connectionId: cachedCopilotConnectionId,
      planType: cachedCopilotPlanType,
      changed: false,
    };
  }
}

function serializeConnectionPlanState(connections: readonly unknown[]): string {
  return JSON.stringify(
    connections.filter(isObject).map((connection) => ({
      id: typeof connection.id === 'string' ? connection.id : '',
      authStatus: typeof connection.authStatus === 'string' ? connection.authStatus : '',
      planType: typeof connection.planType === 'string' ? connection.planType : null,
      workspaces: Array.isArray(connection.workspaces)
        ? connection.workspaces.filter(isObject).map((workspace) => ({
            id: typeof workspace.id === 'string' ? workspace.id : '',
            planType: typeof workspace.planType === 'string' ? workspace.planType : null,
          }))
        : [],
    })),
  );
}

function isRefreshSummary(value: unknown): value is OAuthPlanRefreshSummary {
  return (
    isObject(value) &&
    Array.isArray(value.chatGptConnections) &&
    (value.copilotConnectionId === null || typeof value.copilotConnectionId === 'string') &&
    (value.copilotPlanType === null || typeof value.copilotPlanType === 'string') &&
    typeof value.planOrAuthChanged === 'boolean'
  );
}
