import type { ProjectToolPermissionRule, ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import {
  clearSavedProjectToolPermissionRules,
  clearSavedToolPermissionMode,
  parseProjectToolPermissionRules,
  readSavedProjectToolPermissionRules,
  readSavedToolPermissionMode,
} from '@/modules/projects/data/projectPreferences';
import { localServiceClient } from '@/platform/local-service';

let cachedToolPermissionMode: ToolPermissionMode | undefined;
let toolPermissionModeLoad: Promise<ToolPermissionMode> | undefined;
let toolPermissionModeRevision = 0;
const projectRulesCache = new Map<string, Record<string, ProjectToolPermissionRule>>();
const projectRulesLoads = new Map<string, Promise<Record<string, ProjectToolPermissionRule>>>();
const projectRulesRevisions = new Map<string, number>();

/** Loads the service-owned global policy and transfers a saved renderer value once if needed. */
export function loadToolPermissionMode(): Promise<ToolPermissionMode> {
  if (cachedToolPermissionMode !== undefined) return Promise.resolve(cachedToolPermissionMode);
  if (toolPermissionModeLoad !== undefined) return toolPermissionModeLoad;

  const revision = toolPermissionModeRevision;
  let initialMode: ToolPermissionMode | null;
  try {
    initialMode = readSavedToolPermissionMode();
    // Legacy renderer storage cannot silently grant full access after the host gained native confirmation.
    if (initialMode === 'full_access') initialMode = null;
  } catch (error) {
    return Promise.reject(error);
  }
  const loading = localServiceClient
    .call('tools.permissions.mode.get', {
      ...(initialMode === null ? {} : { initialMode }),
    })
    .then((response) => parseToolPermissionMode(response.mode))
    .then((mode) => {
      clearSavedToolPermissionMode();
      if (toolPermissionModeRevision === revision) cachedToolPermissionMode = mode;
      return cachedToolPermissionMode ?? mode;
    })
    .finally(() => {
      if (toolPermissionModeLoad === loading) toolPermissionModeLoad = undefined;
    });
  toolPermissionModeLoad = loading;
  return loading;
}

/** Saves a user-selected global policy in the local service. */
export async function saveToolPermissionMode(mode: ToolPermissionMode): Promise<ToolPermissionMode> {
  const response = await localServiceClient.call('tools.permissions.mode.set', { mode });
  const savedMode = parseToolPermissionMode(response.mode);
  clearSavedToolPermissionMode();
  toolPermissionModeRevision += 1;
  cachedToolPermissionMode = savedMode;
  return savedMode;
}

/** Loads service-owned project rules and transfers a saved renderer value once if needed. */
export function loadProjectToolPermissionRules(projectId: string): Promise<Record<string, ProjectToolPermissionRule>> {
  const cachedRules = projectRulesCache.get(projectId);
  if (cachedRules !== undefined) return Promise.resolve(cachedRules);
  const existingLoad = projectRulesLoads.get(projectId);
  if (existingLoad !== undefined) return existingLoad;

  const revision = projectRulesRevisions.get(projectId) ?? 0;
  let initialRules: Record<string, ProjectToolPermissionRule> | null;
  try {
    initialRules = readSavedProjectToolPermissionRules(projectId);
    // Legacy renderer storage cannot silently grant project tools after the host gained native confirmation.
    if (initialRules !== null) {
      const restrictedRules = Object.fromEntries(Object.entries(initialRules).filter(([, rule]) => rule !== 'allow'));
      initialRules = Object.keys(restrictedRules).length === 0 ? null : restrictedRules;
    }
  } catch (error) {
    return Promise.reject(error);
  }
  const loading = localServiceClient
    .call('project.tool_permissions.get', {
      projectId,
      ...(initialRules === null ? {} : { initialRules }),
    })
    .then((response) => parseProjectToolPermissionRules(response.rules))
    .then((rules) => {
      clearSavedProjectToolPermissionRules(projectId);
      if ((projectRulesRevisions.get(projectId) ?? 0) === revision) projectRulesCache.set(projectId, rules);
      return projectRulesCache.get(projectId) ?? rules;
    })
    .finally(() => {
      if (projectRulesLoads.get(projectId) === loading) projectRulesLoads.delete(projectId);
    });
  projectRulesLoads.set(projectId, loading);
  return loading;
}

/** Saves the complete project tool policy in the local service. */
export async function saveProjectToolPermissionRules(
  projectId: string,
  rules: Readonly<Record<string, ProjectToolPermissionRule>>,
): Promise<Record<string, ProjectToolPermissionRule>> {
  const response = await localServiceClient.call('project.tool_permissions.set', { projectId, rules });
  const savedRules = parseProjectToolPermissionRules(response.rules);
  clearSavedProjectToolPermissionRules(projectId);
  projectRulesRevisions.set(projectId, (projectRulesRevisions.get(projectId) ?? 0) + 1);
  projectRulesCache.set(projectId, savedRules);
  return savedRules;
}

/** Validates the local service's permission-mode response before caching it. */
function parseToolPermissionMode(value: unknown): ToolPermissionMode {
  if (value === 'require_approval' || value === 'approve_safe_operations' || value === 'full_access') return value;
  throw new Error('The local service returned an invalid tool permission mode.');
}
