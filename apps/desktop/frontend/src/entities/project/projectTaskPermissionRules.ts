import type { ProjectToolPermissionRule } from './types';

export interface ProjectTaskPermissionTarget {
  id: string;
  permissionRuleName: string;
}

const taskPermissionPrefix = 'run_project_task__';
const taskIdPattern = /^[a-z0-9_-]{1,64}$/u;
const permissionFingerprintPattern = /^[a-f0-9]{64}$/u;

export function projectTaskPermissionId(name: string): string | undefined {
  if (!name.startsWith(taskPermissionPrefix)) return undefined;
  const suffix = name.slice(taskPermissionPrefix.length);
  if (taskIdPattern.test(suffix)) return suffix;

  const separator = suffix.lastIndexOf('__');
  if (separator <= 0 || !taskIdPattern.test(suffix.slice(0, separator))) return undefined;
  if (!permissionFingerprintPattern.test(suffix.slice(separator + 2))) return undefined;
  return suffix.slice(0, separator);
}

export function isProjectTaskPermissionRuleName(name: string): boolean {
  return projectTaskPermissionId(name) !== undefined;
}

export function isBoundProjectTaskPermissionRuleName(name: string): boolean {
  const taskId = projectTaskPermissionId(name);
  return taskId !== undefined && name !== `${taskPermissionPrefix}${taskId}`;
}

export function legacyProjectTaskPermissionRuleName(name: string): string | undefined {
  if (!isBoundProjectTaskPermissionRuleName(name)) return undefined;
  const taskId = projectTaskPermissionId(name);
  return taskId === undefined ? undefined : `${taskPermissionPrefix}${taskId}`;
}

export function normalizeProjectTaskPermissionRules(
  rules: Readonly<Record<string, ProjectToolPermissionRule>>,
): Record<string, ProjectToolPermissionRule> {
  const normalized = { ...rules };
  for (const [name, rule] of Object.entries(normalized)) {
    if (
      rule === 'allow' &&
      isProjectTaskPermissionRuleName(name) &&
      !isBoundProjectTaskPermissionRuleName(name)
    ) {
      delete normalized[name];
    }
  }
  return normalized;
}

export function projectTaskPermissionRuleValue(
  rules: Readonly<Record<string, ProjectToolPermissionRule>>,
  name: string,
): ProjectToolPermissionRule | undefined {
  const currentRule = rules[name];
  if (currentRule !== undefined) return currentRule;

  const legacyName = legacyProjectTaskPermissionRuleName(name);
  const legacyRule = legacyName === undefined ? undefined : rules[legacyName];
  return legacyRule === 'allow' ? undefined : legacyRule;
}
