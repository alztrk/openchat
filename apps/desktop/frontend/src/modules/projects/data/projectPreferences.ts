import { type DefaultModelPreference, parseDefaultModelPreference } from '@/modules/providers/data/modelPreferences';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ProjectToolPermissionRule = 'ask' | 'allow' | 'deny';
export type ToolPermissionMode = 'require_approval' | 'approve_safe_operations' | 'full_access';

const projectDefaultModelPrefix = 'models.project_default.';
const projectSkillPrefix = 'project.skills.';
const projectToolRulesPrefix = 'project.tool_rules.';
const toolPermissionModeKey = 'tools.permission_mode';
const projectToolRuleNames = new Set([
  'list_files',
  'search_files',
  'read_file',
  'write_file',
  'edit_file',
  'get_file_info',
  'execute_command',
  'send_terminal_input',
  'web_search',
  'read_url_content',
  'delegate_task',
  'run_project_task',
]);

export function readProjectDefaultModelPreference(projectId: string): DefaultModelPreference | null {
  const raw = unifiedStorage.getItem(`${projectDefaultModelPrefix}${validateProjectId(projectId)}`);
  if (raw === null) return null;
  return parseDefaultModelPreference(JSON.parse(raw));
}

export function writeProjectDefaultModelPreference(projectId: string, preference: DefaultModelPreference | null): void {
  const key = `${projectDefaultModelPrefix}${validateProjectId(projectId)}`;
  if (preference === null) {
    unifiedStorage.removeItem(key);
    return;
  }
  unifiedStorage.setItem(key, JSON.stringify(parseDefaultModelPreference(preference)));
}

export function readProjectSkillIds(projectId: string): string[] {
  const raw = unifiedStorage.getItem(`${projectSkillPrefix}${validateProjectId(projectId)}`);
  if (raw === null) return [];
  const value: unknown = JSON.parse(raw);
  if (
    !Array.isArray(value) ||
    value.length > 32 ||
    !value.every((item) => typeof item === 'string' && /^[a-z][a-z0-9-]{0,63}$/.test(item)) ||
    new Set(value).size !== value.length
  ) {
    throw new Error('The saved project Skill selection is invalid.');
  }
  return value;
}

export function writeProjectSkillIds(projectId: string, skillIds: readonly string[]): void {
  const normalized = [...new Set(skillIds)].sort();
  if (normalized.length > 32 || normalized.some((id) => !/^[a-z][a-z0-9-]{0,63}$/.test(id))) {
    throw new Error('The project Skill selection is invalid.');
  }
  const key = `${projectSkillPrefix}${validateProjectId(projectId)}`;
  if (normalized.length === 0) {
    unifiedStorage.removeItem(key);
    return;
  }
  unifiedStorage.setItem(key, JSON.stringify(normalized));
}

export function readSavedProjectToolPermissionRules(
  projectId: string,
): Record<string, ProjectToolPermissionRule> | null {
  const raw = unifiedStorage.getItem(`${projectToolRulesPrefix}${validateProjectId(projectId)}`);
  if (raw === null) return null;
  return parseProjectToolPermissionRules(JSON.parse(raw));
}

export function parseProjectToolPermissionRules(value: unknown): Record<string, ProjectToolPermissionRule> {
  if (!isObject(value) || Object.keys(value).length > 1080) {
    throw new Error('The saved project tool permission rules are invalid.');
  }
  const rules: Record<string, ProjectToolPermissionRule> = {};
  for (const [name, rule] of Object.entries(value)) {
    if (!isPermissionRule(rule)) throw new Error('The saved project tool permission rules are invalid.');
    if (name === 'git_status' || name === 'git_diff' || name === 'git_history') continue;
    if (!isProjectToolRuleName(name)) {
      throw new Error('The saved project tool permission rules are invalid.');
    }
    rules[name] = rule;
  }
  return rules;
}

export function readSavedToolPermissionMode(): ToolPermissionMode | null {
  const value = unifiedStorage.getItem(toolPermissionModeKey);
  if (value === null) return null;
  if (isToolPermissionMode(value)) return value;
  throw new Error('The saved tool permission mode is invalid.');
}

export function clearSavedProjectToolPermissionRules(projectId: string): void {
  unifiedStorage.removeItem(`${projectToolRulesPrefix}${validateProjectId(projectId)}`);
}

export function clearSavedToolPermissionMode(): void {
  unifiedStorage.removeItem(toolPermissionModeKey);
}

function validateProjectId(projectId: string): string {
  if (!/^[A-Za-z0-9_-]{1,128}$/.test(projectId)) {
    throw new Error('The project identifier is invalid.');
  }
  return projectId;
}

function isProjectToolRuleName(name: string): boolean {
  if (projectToolRuleNames.has(name)) return true;
  if (name.startsWith('run_project_task__')) {
    return /^[a-z0-9_-]{1,64}$/.test(name.slice('run_project_task__'.length));
  }
  if (name.startsWith('project_tool__')) {
    return /^[a-z0-9_-]{1,48}$/.test(name.slice('project_tool__'.length));
  }
  if (!name.startsWith('mcp__') || name.length > 64) return false;
  const suffix = name.slice('mcp__'.length);
  if (suffix.endsWith('__*')) return /^[a-z0-9_]{1,24}$/.test(suffix.slice(0, -3));
  return /^[a-z0-9_]{1,24}__[A-Za-z0-9_-]+$/.test(suffix);
}

function isPermissionRule(value: unknown): value is ProjectToolPermissionRule {
  return value === 'ask' || value === 'allow' || value === 'deny';
}

function isToolPermissionMode(value: string): value is ToolPermissionMode {
  return value === 'require_approval' || value === 'approve_safe_operations' || value === 'full_access';
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
