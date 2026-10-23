import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Select } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import {
  loadProjectToolPermissionRules,
  loadToolPermissionMode,
  saveProjectToolPermissionRules,
} from '@/modules/projects/data/projectPermissionSettings';
import type { ProjectToolPermissionRule, ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import { localServiceClient } from '@/platform/local-service';

const standardTools = [
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
  'list_subagent_models',
  'get_subagent_result',
  'cancel_subagent',
  'save_memory',
  'run_project_task',
] as const;

interface ProjectToolPermissionsPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectToolPermissionsPanel({ project, onClose }: ProjectToolPermissionsPanelProps) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<Record<string, ProjectToolPermissionRule>>({});
  const [toolIds, setToolIds] = useState<string[]>([]);
  const [taskIds, setTaskIds] = useState<string[]>([]);
  const [permissionMode, setPermissionMode] = useState<ToolPermissionMode>('require_approval');
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);

  useEffect(() => {
    let current = true;
    void loadProjectToolSettings(project.id)
      .then((loaded) => {
        if (!current) return;
        setRules(loaded.rules);
        setToolIds(loaded.toolIds);
        setTaskIds(loaded.taskIds);
        setPermissionMode(loaded.permissionMode);
      })
      .catch(() => {
        if (current) setLoadFailed(true);
      })
      .finally(() => {
        if (current) setIsLoading(false);
      });
    return () => {
      current = false;
    };
  }, [project.id]);

  const reload = async () => {
    setIsLoading(true);
    setLoadFailed(false);
    try {
      const loaded = await loadProjectToolSettings(project.id);
      setRules(loaded.rules);
      setToolIds(loaded.toolIds);
      setTaskIds(loaded.taskIds);
      setPermissionMode(loaded.permissionMode);
    } catch {
      setLoadFailed(true);
    } finally {
      setIsLoading(false);
    }
  };

  const ruleNames = useMemo(
    () =>
      [
        ...standardTools,
        ...taskIds.map((id) => `run_project_task__${id}`),
        ...toolIds.map((id) => `project_tool__${id}`),
        ...Object.keys(rules).filter((name) => name.startsWith('mcp__')),
      ].sort((left, right) => left.localeCompare(right)),
    [rules, taskIds, toolIds],
  );

  const updateRule = (name: string, value: string) => {
    setRules((current) => {
      const next = { ...current };
      if (value === 'inherit') delete next[name];
      else if (isPermissionRule(value)) next[name] = value;
      return next;
    });
    setSaveFailed(false);
  };

  const save = async () => {
    if (isSaving || isLoading || loadFailed) return;
    setIsSaving(true);
    setSaveFailed(false);
    try {
      await saveProjectToolPermissionRules(project.id, rules);
      onClose();
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const summary = Object.values(rules).reduce(
    (counts, rule) => {
      if (rule === 'allow') counts.allowed += 1;
      else if (rule === 'ask') counts.approval += 1;
      else counts.denied += 1;
      return counts;
    },
    { allowed: 0, approval: 0, denied: 0 },
  );

  return (
    <div className="grid gap-3">
      {isLoading ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('historyLoading')}
        </p>
      ) : loadFailed ? (
        <div className="grid gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t('projectToolRulesLoadFailed')}
          </p>
          <Button className="justify-self-start" disabled={isLoading} onClick={() => void reload()} size="small">
            {t('retry')}
          </Button>
        </div>
      ) : (
        <div className="openchat-scroll-area grid max-h-project-permission-list gap-3 overflow-y-auto pr-1">
          <p className="text-sm text-secondary-text">{t('projectToolRulesDescription')}</p>
          <p aria-live="polite" className="text-xs text-secondary-text">
            {Object.keys(rules).length === 0
              ? t('projectToolRulesSummaryInherit')
              : t('projectToolRulesSummary', summary)}
            <span className="mt-1 block">
              {t('toolPermissions')}: {t(permissionModeTranslationKey(permissionMode))}
            </span>
          </p>
          {ruleNames.map((name) => (
            <Select
              key={name}
              label={toolLabel(name, t)}
              onChange={(event) => updateRule(name, event.target.value)}
              options={[
                { value: 'inherit', label: t('projectToolRuleInherit') },
                { value: 'ask', label: t('projectToolRuleAsk') },
                { value: 'allow', label: t('projectToolRuleAllow') },
                { value: 'deny', label: t('projectToolRuleDeny') },
              ]}
              value={rules[name] ?? 'inherit'}
            />
          ))}
        </div>
      )}
      {saveFailed ? (
        <p className="text-sm text-destructive" role="alert">
          {t('projectToolRulesSaveFailed')}
        </p>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button disabled={isSaving} onClick={onClose}>
          {t('cancel')}
        </Button>
        <Button disabled={isLoading || isSaving || loadFailed} onClick={() => void save()} variant="primary">
          {isSaving ? t('saving') : t('save')}
        </Button>
      </div>
    </div>
  );
}

async function loadProjectToolSettings(projectId: string): Promise<{
  rules: Record<string, ProjectToolPermissionRule>;
  taskIds: string[];
  toolIds: string[];
  permissionMode: ToolPermissionMode;
}> {
  const [taskResponse, toolResponse, rules, permissionMode] = await Promise.all([
    localServiceClient.call('project.tasks.list', { projectId }),
    localServiceClient.call('project.tools.list', { projectId }),
    loadProjectToolPermissionRules(projectId),
    loadToolPermissionMode(),
  ]);
  return {
    rules,
    taskIds: parseIds(taskResponse.tasks, /^[a-z0-9_-]{1,64}$/, 'The project task list was invalid.'),
    toolIds: parseIds(toolResponse.tools, /^[a-z0-9_-]{1,48}$/, 'The configured project tool list was invalid.'),
    permissionMode,
  };
}

function parseIds(value: unknown, pattern: RegExp, error: string): string[] {
  if (!Array.isArray(value) || value.length > 32) throw new Error(error);
  const ids = value.map((entry) => {
    if (!isObject(entry) || typeof entry.id !== 'string' || !pattern.test(entry.id)) throw new Error(error);
    return entry.id;
  });
  if (new Set(ids).size !== ids.length) throw new Error(error);
  return ids;
}

function toolLabel(
  name: string,
  t: (key: keyof typeof import('@/localization/locales/en.json'), options?: Record<string, unknown>) => string,
): string {
  const labelKey: Partial<Record<string, keyof typeof import('@/localization/locales/en.json')>> = {
    list_files: 'toolListFiles',
    search_files: 'toolSearchFiles',
    read_file: 'toolReadFile',
    write_file: 'toolWriteFile',
    edit_file: 'toolEditFile',
    get_file_info: 'toolGetFileInfo',
    execute_command: 'toolExecuteCommand',
    send_terminal_input: 'toolSendTerminalInput',
    web_search: 'toolWebSearch',
    read_url_content: 'toolReadUrlContent',
    delegate_task: 'toolDelegateTask',
    list_subagent_models: 'toolListSubagentModels',
    get_subagent_result: 'toolGetSubagentResult',
    cancel_subagent: 'toolCancelSubagent',
    run_project_task: 'toolRunProjectTask',
  };
  const taskPrefix = 'run_project_task__';
  const projectToolPrefix = 'project_tool__';
  if (name.startsWith(taskPrefix)) return `${t('toolRunProjectTask')}: ${name.slice(taskPrefix.length)}`;
  if (name.startsWith(projectToolPrefix))
    return t('toolConfiguredProjectTool', { name: name.slice(projectToolPrefix.length) });
  const key = labelKey[name];
  return key ? t(key) : t('toolConfiguredProjectTool', { name });
}

function permissionModeTranslationKey(mode: ToolPermissionMode): keyof typeof import('@/localization/locales/en.json') {
  switch (mode) {
    case 'require_approval':
      return 'toolPermissionRequireApproval';
    case 'approve_safe_operations':
      return 'toolPermissionApproveSafeOperations';
    case 'full_access':
      return 'toolPermissionFullAccess';
  }
}

function isPermissionRule(value: string): value is ProjectToolPermissionRule {
  return value === 'ask' || value === 'allow' || value === 'deny';
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
