import type { TFunction } from 'i18next';
import {
  Ban,
  Brain,
  CircleAlert,
  CircleCheck,
  CircleX,
  FileText,
  FolderOpen,
  GitBranch,
  GitCompare,
  GitFork,
  Globe,
  History,
  Info,
  Keyboard,
  LoaderCircle,
  LockKeyhole,
  MessageCircleQuestion,
  NotebookPen,
  Save,
  Search,
  SlidersHorizontal,
  Terminal,
  Wrench,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { formatJsonValue, type ToolActivity, type ToolActivityStatus } from './messageData';
import { formatToolPath, getRequestedToolPath, ToolActivityResults } from './ToolActivityResults';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const toolNameKeys: Partial<Record<string, TranslationKey>> = {
  list_files: 'toolListFiles',
  glob: 'toolListFiles',
  list_directory: 'toolListFiles',
  search_files: 'toolSearchFiles',
  grep: 'toolSearchFiles',
  read_file: 'toolReadFile',
  read: 'toolReadFile',
  get_file_info: 'toolGetFileInfo',
  write_file: 'toolWriteFile',
  write: 'toolWriteFile',
  edit_file: 'toolEditFile',
  edit: 'toolEditFile',
  execute_command: 'toolExecuteCommand',
  bash: 'toolExecuteCommand',
  run_project_task: 'toolRunProjectTask',
  delegate_task: 'toolDelegateTask',
  list_subagent_models: 'toolListSubagentModels',
  get_subagent_result: 'toolGetSubagentResult',
  cancel_subagent: 'toolCancelSubagent',
  save_memory: 'toolSavePersonalMemory',
  send_terminal_input: 'toolSendTerminalInput',
  git_status: 'toolGitStatus',
  git_diff: 'toolGitDiff',
  git_history: 'toolGitHistory',
  web_search: 'toolWebSearch',
  read_url_content: 'toolReadUrlContent',
  read_url: 'toolReadUrlContent',
};

const statusKeys: Record<ToolActivityStatus, TranslationKey> = {
  awaitingApproval: 'toolAwaitingApproval',
  waitingForUser: 'toolWaitingForUser',
  running: 'toolRunning',
  completed: 'toolCompleted',
  failed: 'toolFailed',
  denied: 'toolDenied',
  cancelled: 'toolCancelled',
};

interface ToolActivityAccordionProps {
  activity: ToolActivity;
}

export function ToolActivityAccordion({ activity }: ToolActivityAccordionProps) {
  const { t } = useTranslation();
  const statusLabel = t(statusKeys[activity.status]);
  const toolLabel = toolName(activity.name, t);
  const requestedPath = getRequestedToolPath(activity);
  const StatusIcon = statusIcon(activity.status);
  const ToolIcon = toolIcon(activity.name);
  const isOpen =
    activity.status === 'awaitingApproval' || activity.status === 'waitingForUser' || activity.status === 'running';
  return (
    <details className="openchat-native-disclosure rounded-card border border-border bg-surface" open={isOpen}>
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm text-text [&::-webkit-details-marker]:hidden">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-control bg-composer text-accent-icon">
          <ToolIcon aria-hidden="true" size={15} />
        </span>
        <span className="min-w-0 flex-1 truncate font-semibold">{toolLabel}</span>
        <span
          className="inline-flex shrink-0 items-center gap-1 rounded-full bg-composer px-2 py-1 text-xs text-secondary-text"
          aria-live="polite"
        >
          <StatusIcon
            aria-hidden="true"
            className={activity.status === 'running' ? 'animate-spin' : undefined}
            size={13}
          />
          {statusLabel}
        </span>
      </summary>
      <div className="grid gap-3 border-t border-border px-3 pb-3 pt-2">
        {requestedPath ? (
          <ToolValue label={t('toolPermissionTarget')} value={formatToolPath(requestedPath, t)} />
        ) : null}
        <ToolActivityResults activity={activity} />
        {activity.fileChangesError ? (
          <p className="text-sm text-destructive" role="alert">
            {activity.fileChangesError}
          </p>
        ) : null}
        <details className="openchat-native-disclosure rounded-control border border-border bg-background">
          <summary className="flex cursor-pointer list-none items-center gap-2 px-2 py-2 text-xs font-medium text-secondary-text [&::-webkit-details-marker]:hidden">
            <SlidersHorizontal aria-hidden="true" size={14} />
            {t('toolTechnicalDetails')}
          </summary>
          <div className="grid gap-3 border-t border-border px-2 pb-2 pt-2">
            <ToolValue label={t('toolInput')} value={activity.arguments} />
            {activity.output !== null ? <ToolValue label={t('toolOutput')} value={activity.output} /> : null}
            {activity.fileChanges.length > 0 ? (
              <ToolValue label={t('fileChangesTitle')} value={activity.fileChanges} />
            ) : null}
          </div>
        </details>
      </div>
    </details>
  );
}

interface ToolValueProps {
  label: string;
  value: unknown;
}

function ToolValue({ label, value }: ToolValueProps) {
  return (
    <div className="min-w-0">
      <p className="mb-1 text-xs font-medium text-secondary-text">{label}</p>
      <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-control bg-background p-2 font-code text-xs text-text">
        {formatJsonValue(value)}
      </pre>
    </div>
  );
}

function statusIcon(status: ToolActivityStatus) {
  switch (status) {
    case 'awaitingApproval':
      return LockKeyhole;
    case 'waitingForUser':
      return MessageCircleQuestion;
    case 'running':
      return LoaderCircle;
    case 'completed':
      return CircleCheck;
    case 'failed':
      return CircleAlert;
    case 'denied':
      return Ban;
    case 'cancelled':
      return CircleX;
  }
}

function toolIcon(name: string) {
  switch (name) {
    case 'list_files':
    case 'glob':
    case 'list_directory':
      return FolderOpen;
    case 'search_files':
    case 'grep':
      return Search;
    case 'read_file':
    case 'read':
      return FileText;
    case 'get_file_info':
      return Info;
    case 'write_file':
    case 'write':
      return Save;
    case 'edit_file':
    case 'edit':
      return NotebookPen;
    case 'execute_command':
    case 'bash':
    case 'run_project_task':
      return Terminal;
    case 'delegate_task':
    case 'list_subagent_models':
    case 'get_subagent_result':
    case 'cancel_subagent':
      return GitFork;
    case 'save_memory':
      return Brain;
    case 'send_terminal_input':
      return Keyboard;
    case 'git_status':
      return GitBranch;
    case 'git_diff':
      return GitCompare;
    case 'git_history':
      return History;
    case 'web_search':
      return Globe;
    case 'read_url_content':
    case 'read_url':
      return Globe;
    default:
      return Wrench;
  }
}

function toolName(name: string, t: TFunction): string {
  const key = toolNameKeys[name];
  if (key) return t(key);
  if (name.startsWith('project_tool__')) {
    const configuredName = name.slice('project_tool__'.length).trim();
    return configuredName.length > 0 ? t('toolConfiguredProjectTool', { name: configuredName }) : t('unknownTool');
  }
  return name || t('unknownTool');
}
