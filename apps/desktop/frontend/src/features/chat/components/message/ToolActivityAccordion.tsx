import type { TFunction } from 'i18next';
import {
  Ban,
  Brain,
  ChevronDown,
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

import { Button } from '@shared/components';
import {
  getConversationFileCheckpointRecoveryStatus,
  revertConversationFileCheckpointOperation,
  type ChatFileChange,
} from '@features/chat/data/chatFileChanges';
import { LocalServiceError } from '@shared/lib/platform/local-service';
import { formatJsonValue, type ToolActivity, type ToolActivityStatus } from './messageData';
import { formatToolPath, getRequestedToolPath, ToolActivityResults } from './ToolActivityResults';
import { getReadFileContent } from './ToolFileResults';
import {
  asRecord,
  getReadFileLineRange,
  getRequestedSearchQuery,
  getSearchMatchCount,
  isTerminalTool,
  stringValue,
} from './toolResultUtils';

type TranslationKey = keyof typeof import('@shared/lib/i18n/locales/en.json');

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

const fileEditStatusKeys: Record<ToolActivityStatus, TranslationKey> = {
  awaitingApproval: 'toolEditFileAwaitingApproval',
  waitingForUser: 'toolEditFileWaitingForUser',
  running: 'toolEditFileRunning',
  completed: 'toolEditFileCompleted',
  failed: 'toolEditFileFailed',
  denied: 'toolEditFileDenied',
  cancelled: 'toolEditFileCancelled',
};

interface ToolActivityAccordionProps {
  activity: ToolActivity;
  conversationId: string | null;
  isResponseInProgress: boolean;
  onCheckpointRestored?: () => void;
  onOpenFileChanges?: () => void;
  onOpenFile?(path: string, content: string): void;
  onOpenEditedFile?(change: ChatFileChange): void;
}

export function ToolActivityAccordion({
  activity,
  conversationId,
  isResponseInProgress,
  onCheckpointRestored,
  onOpenFileChanges,
  onOpenFile,
  onOpenEditedFile,
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
  const isFileEditTool = activity.name === 'edit_file' || activity.name === 'edit';
  const isFileReadTool = activity.name === 'read_file' || activity.name === 'read';
  const isFileSearchTool = activity.name === 'search_files' || activity.name === 'grep';
  const isCommandExecutionTool = activity.name === 'execute_command' || activity.name === 'bash';
  const isWebSearchTool = activity.name === 'web_search';
  const isReadUrlTool = activity.name === 'read_url_content' || activity.name === 'read_url';
  const isWebTool = isWebSearchTool || isReadUrlTool;
  const showWebStatus = isWebTool && activity.status !== 'completed';
  const webArgs = asRecord(activity.arguments);
  const webOutput = asRecord(activity.output);
  const webSearchQuery = isWebSearchTool
    ? getRequestedSearchQuery(activity)
    : null;
  const webPageUrl = isReadUrlTool
    ? stringValue(webOutput?.url) ?? stringValue(webArgs?.url) ?? activity.targetPath
    : null;
  const webSearchResultCount = isWebSearchTool && Array.isArray(webOutput?.results) ? webOutput.results.length : null;
  const readUrlLength =
    isReadUrlTool && typeof webOutput?.length === 'number' && Number.isSafeInteger(webOutput.length)
      ? webOutput.length
      : null;
  const headingLabel = isFileEditTool
    ? t(fileEditStatusKeys[activity.status])
    : isFileReadTool && activity.status === 'completed'
      ? t('toolReadFileCompleted')
      : isFileSearchTool && activity.status === 'completed'
        ? t('toolSearchFilesCompleted')
        : isCommandExecutionTool &&
            (activity.status === 'completed' || activity.status === 'failed' || activity.status === 'cancelled')
          ? t('toolExecuteCommandCompleted')
          : isWebSearchTool && activity.status === 'completed'
            ? t('toolSearchFilesCompleted')
            : isReadUrlTool && activity.status === 'completed'
              ? t('toolReadFileCompleted')
              : toolLabel;
  const requestedPath =
    isTerminalTool(activity.name) || activity.name === 'computer_use' || isWebTool
      ? null
      : getRequestedToolPath(activity);
  const requestedSearchQuery = isFileSearchTool ? getRequestedSearchQuery(activity) : null;
  const searchMatchCount = isFileSearchTool ? getSearchMatchCount(activity) : null;
  const commandArgs = asRecord(activity.arguments);
  const requestedCommand = isCommandExecutionTool
    ? stringValue(commandArgs?.command) ?? stringValue(asRecord(activity.output)?.command)
    : null;
  const readFileLineRange = isFileReadTool ? getReadFileLineRange(activity) : null;
  const editedFile = isFileEditTool && requestedPath ? findEditedFile(activity, requestedPath) : null;
  const toolFileName = requestedPath?.replaceAll('\\', '/').split('/').filter(Boolean).at(-1) ?? null;
  const readFileContent = isFileReadTool ? getReadFileContent(activity.output) : null;
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
    <details className="group openchat-tool-accordion openchat-native-disclosure rounded-card border border-border bg-surface" open={isOpen}>
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm text-text [&::-webkit-details-marker]:hidden">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-control bg-composer text-accent-icon">
          <ToolIcon aria-hidden="true" size={15} />
        </span>
        <span
          aria-live="polite"
          className={
            isFileEditTool || isFileReadTool || isFileSearchTool || isCommandExecutionTool || isWebTool
              ? 'shrink-0 font-semibold'
              : 'min-w-0 flex-1 truncate font-semibold'
          }
        >
          {headingLabel}
        </span>
        {isCommandExecutionTool && requestedCommand ? (
          <span className="min-w-0 flex-1 break-words whitespace-pre-wrap text-xs text-secondary-text" title={requestedCommand}>
            {requestedCommand}
          </span>
        ) : null}
        {isFileEditTool && toolFileName ? (
          editedFile && onOpenEditedFile ? (
            <Button
              aria-label={`${t('fileChangesView')}: ${editedFile.path}`}
              className="h-6 min-h-0 min-w-0 flex-1 justify-start truncate bg-transparent px-0 text-xs text-secondary-text hover:bg-transparent hover:text-accent"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                onOpenEditedFile(editedFile);
              }}
              size="small"
              title={editedFile.path}
              type="button"
              variant="ghost"
            >
              {toolFileName}
            </Button>
          ) : (
            <span className="min-w-0 flex-1 truncate text-xs text-secondary-text" title={requestedPath ?? undefined}>
              {toolFileName}
            </span>
          )
        ) : null}
        {isFileReadTool && toolFileName && requestedPath ? (
          onOpenFile && readFileContent !== null ? (
            <Button
              aria-label={t('workspaceFileContent', { name: requestedPath })}
              className="h-6 min-h-0 min-w-0 flex-1 justify-start truncate bg-transparent px-0 text-xs text-secondary-text hover:bg-transparent hover:text-accent"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                onOpenFile(requestedPath, readFileContent);
              }}
              size="small"
              title={requestedPath}
              type="button"
              variant="ghost"
            >
              {toolFileName}
            </Button>
          ) : (
            <span className="min-w-0 flex-1 truncate text-xs text-secondary-text" title={requestedPath}>
              {toolFileName}
            </span>
          )
        ) : null}
        {isFileSearchTool && requestedSearchQuery ? (
          <span className="min-w-0 flex-1 truncate text-xs text-secondary-text" title={requestedSearchQuery}>
            {requestedSearchQuery}
          </span>
        ) : null}
        {isFileSearchTool && searchMatchCount !== null ? (
          <span className="shrink-0 text-xs text-secondary-text">
            {t('toolSearchMatchCount', { count: searchMatchCount })}
          </span>
        ) : null}
        {isWebSearchTool && webSearchQuery ? (
          <span className="min-w-0 flex-1 truncate text-xs text-secondary-text" title={webSearchQuery}>
            {webSearchQuery}
          </span>
        ) : null}
        {isReadUrlTool && webPageUrl ? (
          <span className="min-w-0 flex-1 truncate font-code text-xs text-secondary-text" title={webPageUrl}>
            {webPageUrl}
          </span>
        ) : null}
        {isWebSearchTool && webSearchResultCount !== null ? (
          <span className="shrink-0 text-xs text-secondary-text">
            {t('toolWebSearchResultCount', { count: webSearchResultCount })}
          </span>
        ) : null}
        {isReadUrlTool && readUrlLength !== null ? (
          <span className="shrink-0 text-xs text-secondary-text">{t('toolReadUrlLength', { count: readUrlLength })}</span>
        ) : null}
        {isFileReadTool && readFileLineRange !== null ? (
          <span className="shrink-0 text-xs text-secondary-text">
            {t('toolReadLineRange', readFileLineRange)}
          </span>
        ) : null}
        {isFileEditTool ? (
          <span
            aria-label={
              editedFile && editedFile.addedLines !== null && editedFile.removedLines !== null
                ? t('fileChangesLineCounts', { added: editedFile.addedLines, removed: editedFile.removedLines })
                : t('fileChangesSomeCountsUnavailable')
            }
            className="inline-flex shrink-0 gap-1 font-mono text-xs"
            role="img"
          >
            {editedFile && editedFile.addedLines !== null && editedFile.removedLines !== null ? (
              <>
                <span className="text-success">+{editedFile.addedLines}</span>
                <span className="text-destructive">−{editedFile.removedLines}</span>
              </>
            ) : (
              <span className="text-secondary-text">{t('fileChangesSomeCountsUnavailable')}</span>
            )}
          </span>
        ) : null}
        {(!isFileEditTool && !isFileReadTool && !isFileSearchTool && !isCommandExecutionTool && !isWebTool) ||
        showWebStatus ? (
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
        ) : null}
        <ChevronDown
          aria-hidden="true"
          className="ml-auto shrink-0 text-secondary-icon transition-transform group-open:rotate-180 motion-reduce:transition-none"
          size={15}
        />
      </summary>
      <div
        className={
          isFileEditTool || isFileReadTool || isFileSearchTool || isCommandExecutionTool || isWebTool
            ? isCommandExecutionTool
              ? 'border-t border-border p-2'
              : 'border-t border-border'
            : 'grid gap-3 border-t border-border px-3 pb-3 pt-2'
        }
      >
        {requestedPath && !isFileEditTool && !isFileReadTool && !isFileSearchTool ? (
          <ToolValue label={t('toolPermissionTarget')} value={formatToolPath(requestedPath)} />
        ) : null}
        <ToolActivityResults activity={activity} conversationId={conversationId} onOpenFile={onOpenFile} />
        {activity.fileChangesError && !isFileEditTool ? (
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

function findEditedFile(activity: ToolActivity, requestedPath: string) {
  const normalizePath = (path: string) => path.replaceAll('\\', '/').toLowerCase();
  const normalizedRequestedPath = normalizePath(requestedPath);
  const exactMatch = activity.fileChanges.find((change) => normalizePath(change.path) === normalizedRequestedPath);
  if (exactMatch) return exactMatch;

  const fileName = normalizedRequestedPath.split('/').at(-1);
  if (!fileName) return null;
  const sameNameMatches = activity.fileChanges.filter((change) => normalizePath(change.path).split('/').at(-1) === fileName);
  return sameNameMatches.length === 1 ? sameNameMatches[0] : null;
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
