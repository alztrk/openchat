import type { ProjectToolPermissionRule } from './types';

export interface ProjectToolPermissionTarget {
  id: string;
  permissionRuleName: string;
}

const projectToolPrefix = 'project_tool__';
const projectToolIdPattern = /^[a-z0-9_-]{1,48}$/u;
const permissionFingerprintPattern = /^[a-f0-9]{64}$/u;

export function isProjectToolPermissionRuleName(name: string): boolean {
  if (!name.startsWith(projectToolPrefix)) return false;

  const suffix = name.slice(projectToolPrefix.length);
  if (projectToolIdPattern.test(suffix)) return true;

  const separator = suffix.lastIndexOf('__');
  return (
    separator > 0 &&
    projectToolIdPattern.test(suffix.slice(0, separator)) &&
    permissionFingerprintPattern.test(suffix.slice(separator + 2))
  );
}

export function normalizeProjectToolPermissionRules(
  rules: Readonly<Record<string, ProjectToolPermissionRule>>,
): Record<string, ProjectToolPermissionRule> {
  const normalized = { ...rules };
  for (const [name, rule] of Object.entries(normalized)) {
    if (
      name.startsWith(projectToolPrefix) &&
      projectToolIdPattern.test(name.slice(projectToolPrefix.length)) &&
      rule === 'allow'
    ) {
      delete normalized[name];
    }
  }
  return normalized;
}

export function legacyProjectToolPermissionRuleName(name: string): string | undefined {
  if (!name.startsWith(projectToolPrefix)) return undefined;
  const suffix = name.slice(projectToolPrefix.length);
  const separator = suffix.lastIndexOf('__');
  if (separator <= 0 || !projectToolIdPattern.test(suffix.slice(0, separator))) return undefined;
  if (!permissionFingerprintPattern.test(suffix.slice(separator + 2))) return undefined;
  return `${projectToolPrefix}${suffix.slice(0, separator)}`;
}

export function projectToolPermissionRuleValue(
  rules: Readonly<Record<string, ProjectToolPermissionRule>>,
  name: string,
): ProjectToolPermissionRule | undefined {
  const currentRule = rules[name];
  if (currentRule !== undefined) return currentRule;

  const legacyName = legacyProjectToolPermissionRuleName(name);
  const legacyRule = legacyName === undefined ? undefined : rules[legacyName];
  return legacyRule === 'allow' ? undefined : legacyRule;
}
