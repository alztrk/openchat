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
  Terminal,
  Undo2,
  Wrench,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import {
  getConversationFileCheckpointRecoveryStatus,
  revertConversationFileCheckpointOperation,
} from '@/modules/conversations/data/chatFileChanges';
import { LocalServiceError } from '@/platform/local-service';
import { formatJsonValue, type ToolActivity, type ToolActivityStatus } from './messageData';
import { formatToolPath, getRequestedToolPath, ToolActivityResults } from './ToolActivityResults';
import { isTerminalTool } from './toolResultUtils';

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
  conversationId: string | null;
  isResponseInProgress: boolean;
  onCheckpointRestored?: () => void;
  onOpenFileChanges?: () => void;
  onOpenFile?(path: string, content: string): void;
}

export function ToolActivityAccordion({
  activity,
  conversationId,
  isResponseInProgress,
  onCheckpointRestored,
  onOpenFileChanges,
  onOpenFile,
}: ToolActivityAccordionProps) {
  const { t } = useTranslation();
  const recoveryCheckpointId = activity.recoveryCheckpointId;
  const canCheckRecovery =
    conversationId !== null &&
    recoveryCheckpointId !== null &&
    activity.fileChanges.length > 0 &&
    !isResponseInProgress &&
    (activity.status === 'failed' || activity.status === 'cancelled');
  const [recoveryStatus, setRecoveryStatus] = useState<
    'idle' | 'checking' | 'available' | 'alreadyRestored' | 'conflict' | 'checkFailed' | 'failed' | 'partial'
  >('idle');
  const [isRestoring, setIsRestoring] = useState(false);
  const [expandedRecovery, setExpandedRecovery] = useState<{
    conversationId: string;
    checkpointId: string;
  } | null>(null);
  const recoveryPathsExpanded =
    expandedRecovery !== null &&
    expandedRecovery.conversationId === conversationId &&
    expandedRecovery.checkpointId === recoveryCheckpointId;
  const statusLabel = t(statusKeys[activity.status]);
  const toolLabel = toolName(activity.name, t);
  const requestedPath =
    isTerminalTool(activity.name) || activity.name === 'computer_use' ? null : getRequestedToolPath(activity);
  const StatusIcon = statusIcon(activity.status);
  const ToolIcon = toolIcon(activity.name);
  const isOpen =
    activity.status === 'awaitingApproval' || activity.status === 'waitingForUser' || activity.status === 'running';

  useEffect(() => {
    if (!canCheckRecovery || conversationId === null || recoveryCheckpointId === null) {
      setRecoveryStatus('idle');
      return;
    }
    let current = true;
    setRecoveryStatus('checking');
    void getConversationFileCheckpointRecoveryStatus(conversationId, recoveryCheckpointId)
      .then((state) => {
        if (current) setRecoveryStatus(state);
      })
      .catch(() => {
        if (current) setRecoveryStatus('checkFailed');
      });
    return () => {
      current = false;
    };
  }, [canCheckRecovery, conversationId, recoveryCheckpointId]);

  const restoreFailedOperation = async () => {
    if (!conversationId || !recoveryCheckpointId || isRestoring) return;
    setIsRestoring(true);
    try {
      await revertConversationFileCheckpointOperation(conversationId, recoveryCheckpointId);
      setRecoveryStatus('alreadyRestored');
      onCheckpointRestored?.();
    } catch (error) {
      if (error instanceof LocalServiceError && error.code === 'file_checkpoint_conflict') {
        setRecoveryStatus('conflict');
      } else if (error instanceof LocalServiceError && error.code === 'file_checkpoint_already_restored') {
        setRecoveryStatus('alreadyRestored');
      } else if (error instanceof LocalServiceError && error.code === 'file_checkpoint_rollback_failed') {
        setRecoveryStatus('partial');
      } else {
        setRecoveryStatus('failed');
      }
    } finally {
      setIsRestoring(false);
    }
  };

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
        {requestedPath ? <ToolValue label={t('toolPermissionTarget')} value={formatToolPath(requestedPath)} /> : null}
        <ToolActivityResults activity={activity} onOpenFile={onOpenFile} />
        {activity.fileChangesError ? (
          <p className="text-sm text-destructive" role="alert">
            {t(
              activity.fileChangesError === 'recovery_unavailable'
                ? 'fileChangesRecoveryUnavailable'
                : 'fileChangesTrackingFailed',
            )}
          </p>
        ) : null}
        {canCheckRecovery ? (
          <section className="grid gap-2 rounded-control border border-border bg-background p-3">
            <p className="text-sm text-text">
              {t('fileChangesRecoveryPrompt', { count: activity.fileChanges.length })}
            </p>
            <ul aria-label={t('fileChangesRecoveryFiles')} className="grid gap-1 text-xs text-secondary-text">
              {activity.fileChanges.slice(0, 3).map((change) => (
                <li className="truncate" key={change.id} title={change.path}>
                  {change.path}
                </li>
              ))}
            </ul>
            {activity.fileChanges.length > 3 ? (
              <details
                className="rounded-control border border-border px-2 py-1"
                onToggle={(event) => {
                  if (!conversationId || !recoveryCheckpointId || !event.currentTarget.open) {
                    setExpandedRecovery(null);
                    return;
                  }
                  setExpandedRecovery({ conversationId, checkpointId: recoveryCheckpointId });
                }}
                open={recoveryPathsExpanded}
              >
                <summary className="cursor-pointer text-xs text-secondary-text">
                  {t('fileChangesRecoveryMore', { count: activity.fileChanges.length - 3 })}
                </summary>
                {recoveryPathsExpanded ? (
                  <ul
                    aria-label={t('fileChangesRecoveryFiles')}
                    className="mt-2 grid max-h-40 gap-1 overflow-y-auto text-xs text-secondary-text"
                  >
                    {activity.fileChanges.slice(3).map((change) => (
                      <li className="truncate" key={change.id} title={change.path}>
                        {change.path}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </details>
            ) : null}
            {recoveryStatus === 'checking' ? (
              <p className="text-xs text-secondary-text" role="status">
                {t('fileChangesRecoveryChecking')}
              </p>
            ) : null}
            {recoveryStatus === 'available' && (activity.fileChanges.length <= 3 || recoveryPathsExpanded) ? (
              <Button
                className="w-fit"
                disabled={isRestoring}
                onClick={() => void restoreFailedOperation()}
                size="small"
                variant="secondary"
              >
                <Undo2 aria-hidden="true" size={14} />
                <span>{t(isRestoring ? 'fileChangesRecoveryRunning' : 'fileChangesRecoveryAction')}</span>
              </Button>
            ) : null}
            {recoveryStatus === 'alreadyRestored' ? (
              <p className="text-xs text-success" role="status">
                {t('fileChangesRecoveryDone')}
              </p>
            ) : null}
            {recoveryStatus === 'conflict' ? (
              <p className="text-xs text-destructive" role="alert">
                {t('fileChangesRecoveryConflict')}
              </p>
            ) : null}
            {recoveryStatus === 'failed' ? (
              <div className="grid justify-items-start gap-2" role="alert">
                <p className="text-xs text-destructive">{t('fileChangesRecoveryFailed')}</p>
                {onOpenFileChanges ? (
                  <Button onClick={onOpenFileChanges} size="small" variant="secondary">
                    {t('fileChangesOpenButton')}
                  </Button>
                ) : null}
              </div>
            ) : null}
            {recoveryStatus === 'partial' ? (
              <div className="grid justify-items-start gap-2" role="alert">
                <p className="text-xs text-destructive">{t('fileChangesRecoveryPartial')}</p>
                {onOpenFileChanges ? (
                  <Button onClick={onOpenFileChanges} size="small" variant="secondary">
                    {t('fileChangesOpenButton')}
                  </Button>
                ) : null}
              </div>
            ) : null}
            {recoveryStatus === 'checkFailed' ? (
              <div className="grid justify-items-start gap-2" role="alert">
                <p className="text-xs text-destructive">{t('fileChangesRecoveryCheckFailed')}</p>
                <Button
                  onClick={() => {
                    if (!conversationId || !recoveryCheckpointId) return;
                    setRecoveryStatus('checking');
                    void getConversationFileCheckpointRecoveryStatus(conversationId, recoveryCheckpointId)
                      .then(setRecoveryStatus)
                      .catch(() => setRecoveryStatus('checkFailed'));
                  }}
                  size="small"
                  variant="secondary"
                >
                  {t('retry')}
                </Button>
              </div>
            ) : null}
          </section>
        ) : null}
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
