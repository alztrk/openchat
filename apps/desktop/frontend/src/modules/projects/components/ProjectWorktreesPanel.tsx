import { FolderOpen, GitCompare, RefreshCw, Terminal, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import {
  loadProjectToolPermissionRules,
  loadToolPermissionMode,
} from '@/modules/projects/data/projectPermissionSettings';
import type { ProjectToolPermissionRule, ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@/platform/local-service';

interface ProjectWorktreesPanelProps {
  project: ChatProject;
  onClose(): void;
  onUseWorktree(path: string, branch: string): Promise<void>;
}

interface Worktree {
  id: string;
  branch: string;
  path: string;
  changedFileCount: number;
}

interface ProjectTask {
  id: string;
  command: string;
  timeoutSeconds: number;
}

interface ReviewResult {
  staged: string;
  unstaged: string;
  changedFileCount: number;
}

type DetailDialog =
  | { kind: 'remove'; worktree: Worktree }
  | { kind: 'task'; worktree: Worktree; task: ProjectTask }
  | { kind: 'review'; worktree: Worktree; result: ReviewResult }
  | { kind: 'result'; task: ProjectTask; status: string; output: string; truncated: boolean };
type WorktreeErrorKey =
  | 'projectWorktreeLoadFailed'
  | 'projectWorktreeCreateFailed'
  | 'projectWorktreeOperationFailed'
  | 'projectWorktreeNotRepository'
  | 'projectWorktreeCheckFailed'
  | 'projectWorktreeTaskEmpty'
  | 'projectWorktreeTaskLoadFailed'
  | 'projectWorktreeTaskRunFailed'
  | 'projectWorktreeTaskDenied'
  | 'projectWorktreeTaskCancelled'
  | 'toolPermissionSettingsLoadFailed';

export function ProjectWorktreesPanel({ project, onClose, onUseWorktree }: ProjectWorktreesPanelProps) {
  const { t, i18n } = useTranslation();
  const mounted = useRef(false);
  const operationRef = useRef<ServiceOperation | null>(null);
  const [worktrees, setWorktrees] = useState<Worktree[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [isUsing, setIsUsing] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [checkingId, setCheckingId] = useState<string | null>(null);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [taskWorktreeId, setTaskWorktreeId] = useState<string | null>(null);
  const [tasks, setTasks] = useState<ProjectTask[]>([]);
  const [permissionMode, setPermissionMode] = useState<ToolPermissionMode | null>(null);
  const [permissionRules, setPermissionRules] = useState<Record<string, ProjectToolPermissionRule>>({});
  const [permissionsFailed, setPermissionsFailed] = useState(false);
  const [detail, setDetail] = useState<DetailDialog | null>(null);
  const [errorKey, setErrorKey] = useState<WorktreeErrorKey | null>(null);

  const refreshWorktrees = useCallback(async () => {
    setIsLoading(true);
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('project.worktrees.list', {
        projectId: project.id,
      });
      if (!Array.isArray(response.worktrees) || typeof response.truncated !== 'boolean') {
        throw new Error('The worktree list is invalid.');
      }
      const list = response.worktrees.map(parseWorktree);
      if (mounted.current) {
        setWorktrees(list);
        setTruncated(response.truncated);
        setIsLoading(false);
      }
    } catch (error) {
      if (mounted.current) {
        setErrorKey(worktreeErrorKey(error, 'projectWorktreeLoadFailed'));
        setIsLoading(false);
      }
    }
  }, [project.id]);

  useEffect(() => {
    mounted.current = true;
    void refreshWorktrees();
    void Promise.all([loadToolPermissionMode(), loadProjectToolPermissionRules(project.id)])
      .then(([mode, rules]) => {
        if (!mounted.current) return;
        setPermissionMode(mode);
        setPermissionRules(rules);
      })
      .catch(() => {
        if (mounted.current) setPermissionsFailed(true);
      });
    return () => {
      mounted.current = false;
      const operation = operationRef.current;
      operationRef.current = null;
      if (operation) {
        void operation.cancel().catch(() => {
          window.dispatchEvent(new CustomEvent('openchat-worktree-cancel-failed'));
        });
      }
    };
  }, [project.id, refreshWorktrees]);

  const createWorktree = async () => {
    if (isCreating || isLoading || removingId !== null || runningId !== null) return;
    setIsCreating(true);
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('project.worktrees.create', { projectId: project.id }, 50_000);
      const worktree = parseWorktree(response);
      await refreshWorktrees();
      await activateWorktree(worktree);
    } catch (error) {
      if (mounted.current) setErrorKey(worktreeErrorKey(error, 'projectWorktreeCreateFailed'));
    } finally {
      if (mounted.current) setIsCreating(false);
    }
  };

  const activateWorktree = async (worktree: Worktree) => {
    if (isUsing || removingId !== null || runningId !== null) return;
    setIsUsing(true);
    setErrorKey(null);
    try {
      await onUseWorktree(worktree.path, worktree.branch);
      onClose();
    } catch {
      if (mounted.current) setErrorKey('projectWorktreeOperationFailed');
    } finally {
      if (mounted.current) setIsUsing(false);
    }
  };

  const reviewWorktree = async (worktree: Worktree) => {
    if (checkingId || runningId !== null || removingId !== null) return;
    setCheckingId(worktree.id);
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('project.worktrees.review', {
        projectId: project.id,
        worktreeId: worktree.id,
      });
      if (!isObject(response.diff) || !isObject(response.status) || !Array.isArray(response.status.files)) {
        throw new Error('The worktree review response is invalid.');
      }
      if (typeof response.diff.staged !== 'string' || typeof response.diff.unstaged !== 'string') {
        throw new Error('The worktree diff response is invalid.');
      }
      setDetail({
        kind: 'review',
        worktree,
        result: {
          staged: response.diff.staged,
          unstaged: response.diff.unstaged,
          changedFileCount: response.status.files.length,
        },
      });
    } catch (error) {
      if (mounted.current) setErrorKey(worktreeErrorKey(error, 'projectWorktreeCheckFailed'));
    } finally {
      if (mounted.current) setCheckingId(null);
    }
  };

  const loadTasks = async (worktree: Worktree) => {
    if (taskWorktreeId === worktree.id || runningId !== null || removingId !== null) return;
    setTaskWorktreeId(worktree.id);
    setTasks([]);
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('project.worktrees.tasks', {
        projectId: project.id,
        worktreeId: worktree.id,
      });
      if (!Array.isArray(response.tasks)) throw new Error('The named task list is invalid.');
      const taskList = response.tasks.map(parseProjectTask);
      if (mounted.current) {
        setTasks(taskList);
        if (taskList.length === 0) setErrorKey('projectWorktreeTaskEmpty');
      }
    } catch (error) {
      if (mounted.current) setErrorKey(worktreeErrorKey(error, 'projectWorktreeTaskLoadFailed'));
      if (mounted.current) setTaskWorktreeId(null);
    }
  };

  const confirmTask = (worktree: Worktree, task: ProjectTask) => {
    if (permissionsFailed || permissionMode === null) {
      setErrorKey('toolPermissionSettingsLoadFailed');
      return;
    }
    const rule = permissionRules[`run_project_task__${task.id}`] ?? permissionRules.run_project_task;
    if (rule === 'deny') {
      setErrorKey('projectWorktreeTaskDenied');
      return;
    }
    setDetail({ kind: 'task', worktree, task });
  };

  /** Starts a task only after the service approval request matches the command shown to the user. */
  const runTask = async (worktree: Worktree, task: ProjectTask) => {
    if (!permissionMode || permissionsFailed || runningId !== null) return;
    setDetail(null);
    setRunningId(worktree.id);
    setIsCancelling(false);
    setErrorKey(null);
    let activeOperation: ServiceOperation | null = null;
    let approvalResponseFailed = false;
    let approvalPayloadMismatch = false;
    let permissionEventHandled = false;

    /** Cancels the pending task if its approval response cannot reach the service. */
    const stopAfterApprovalFailure = () => {
      approvalResponseFailed = true;
      if (mounted.current) setErrorKey('projectWorktreeOperationFailed');
      const operation = activeOperation ?? operationRef.current;
      if (operation) {
        void operation.cancel().catch((cause: unknown) => {
          globalThis.reportError(
            new Error('A project task could not be cancelled after an approval failure.', { cause }),
          );
        });
      }
    };

    try {
      const operation = await localServiceClient.startOperation(
        'project.worktrees.run_task',
        {
          projectId: project.id,
          worktreeId: worktree.id,
          taskId: task.id,
          expectedCommand: task.command,
          expectedTimeoutSeconds: task.timeoutSeconds,
        },
        (event) => {
          if (event.name !== 'chat.tool.permission.requested' || permissionEventHandled) return;
          permissionEventHandled = true;

          const approvalRequestId = event.data.approvalRequestId;
          const argumentsValue = event.data.arguments;
          const matchesConfirmedTask =
            event.data.toolName === 'run_project_task' &&
            event.data.targetPath === worktree.path &&
            isObject(argumentsValue) &&
            argumentsValue.taskId === task.id &&
            argumentsValue.command === task.command &&
            argumentsValue.timeoutSeconds === task.timeoutSeconds;

          if (typeof approvalRequestId !== 'string' || approvalRequestId.length === 0) {
            stopAfterApprovalFailure();
            return;
          }

          approvalPayloadMismatch = !matchesConfirmedTask;
          void localServiceClient
            .respondToPermission(approvalRequestId, matchesConfirmedTask, i18n.resolvedLanguage ?? i18n.language)
            .catch((cause: unknown) => {
              globalThis.reportError(new Error('The project task approval could not be delivered.', { cause }));
              stopAfterApprovalFailure();
            });
        },
      );
      activeOperation = operation;
      operationRef.current = operation;
      if (approvalResponseFailed) await operation.cancel();
      if (!mounted.current) {
        await operation.cancel();
        return;
      }
      const result = await operation.result;
      if (
        typeof result.status !== 'string' ||
        typeof result.output !== 'string' ||
        typeof result.truncated !== 'boolean'
      ) {
        throw new Error('The named task result is invalid.');
      }
      const exitCode = result.exitCode;
      if (exitCode !== undefined && exitCode !== null && typeof exitCode !== 'number') {
        throw new Error('The named task exit code is invalid.');
      }
      const status =
        result.status === 'completed'
          ? typeof exitCode === 'number'
            ? t('projectWorktreeTaskExitCode', { code: exitCode })
            : t('projectWorktreeTaskExitCodeUnavailable')
          : result.status === 'timed_out'
            ? t('projectWorktreeTaskTimedOut')
            : result.status === 'cancelled'
              ? t('projectWorktreeTaskCancelled')
              : t('projectWorktreeTaskRunFailed');
      if (mounted.current)
        setDetail({ kind: 'result', task, status, output: result.output, truncated: result.truncated });
    } catch (error) {
      if (mounted.current) {
        setErrorKey(approvalPayloadMismatch ? 'projectWorktreeTaskLoadFailed' : taskErrorKey(error, isCancelling));
      }
    } finally {
      operationRef.current = null;
      if (mounted.current) {
        setRunningId(null);
        setIsCancelling(false);
      }
    }
  };

  const cancelTask = async () => {
    const operation = operationRef.current;
    if (!operation || isCancelling) return;
    setIsCancelling(true);
    try {
      const cancelled = await operation.cancel();
      if (!cancelled && mounted.current) setErrorKey('projectWorktreeOperationFailed');
    } catch {
      if (mounted.current) setErrorKey('projectWorktreeOperationFailed');
    }
  };

  const removeWorktree = async (worktree: Worktree) => {
    setDetail(null);
    setRemovingId(worktree.id);
    setErrorKey(null);
    try {
      await localServiceClient.call(
        'project.worktrees.remove',
        { projectId: project.id, worktreeId: worktree.id },
        50_000,
      );
      await refreshWorktrees();
    } catch (error) {
      if (mounted.current) setErrorKey(worktreeErrorKey(error));
    } finally {
      if (mounted.current) setRemovingId(null);
    }
  };

  const busy = isCreating || isUsing || removingId !== null || runningId !== null;
  const detailTitle =
    detail?.kind === 'remove'
      ? t('projectWorktreeRemoveTitle')
      : detail?.kind === 'task'
        ? t('projectWorktreeTaskConfirmationTitle')
        : detail?.kind === 'review'
          ? t('projectWorktreeReviewTitle')
          : detail?.kind === 'result'
            ? t('projectWorktreeTaskResultTitle', { task: detail.task.id })
            : '';

  return (
    <div className="grid gap-3">
      <p className="text-sm text-secondary-text">{t('projectWorktreesDescription')}</p>
      {errorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
      {isLoading ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('projectWorktreesLoading')}
        </p>
      ) : (
        <div className="openchat-scroll-area grid max-h-worktree-list gap-3 overflow-y-auto pr-1">
          {worktrees.length === 0 ? (
            <p className="py-6 text-center text-sm text-secondary-text">{t('projectWorktreesEmpty')}</p>
          ) : null}
          {worktrees.map((worktree) => (
            <article className="grid gap-2 rounded-card border border-border bg-surface p-3" key={worktree.id}>
              <h3 className="text-sm font-medium text-text">
                {t('projectWorktreeBranch', { branch: worktree.branch })}
              </h3>
              <p className="break-all text-xs text-secondary-text">
                {t('projectWorktreePath', { path: worktree.path })}
              </p>
              <p className="text-xs text-secondary-text">
                {worktree.changedFileCount === 0
                  ? t('projectWorktreeStatusClean')
                  : t('projectWorktreeStatusChanges', { count: worktree.changedFileCount })}
              </p>
              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={busy || checkingId !== null}
                  onClick={() => void reviewWorktree(worktree)}
                  size="small"
                >
                  {checkingId === worktree.id ? (
                    <RefreshCw aria-hidden="true" className="animate-spin" size={14} />
                  ) : (
                    <GitCompare aria-hidden="true" size={14} />
                  )}
                  {t('projectWorktreeReview')}
                </Button>
                <Button
                  disabled={permissionsFailed || busy}
                  onClick={() => void (runningId === worktree.id ? cancelTask() : loadTasks(worktree))}
                  size="small"
                >
                  <Terminal aria-hidden="true" size={14} />
                  {runningId === worktree.id
                    ? isCancelling
                      ? t('projectWorktreeTaskStopping')
                      : t('projectWorktreeTaskStop')
                    : t('projectWorktreeRunCheck')}
                </Button>
                <Button disabled={busy} onClick={() => void activateWorktree(worktree)} size="small">
                  <FolderOpen aria-hidden="true" size={14} />
                  {t('projectWorktreeUse')}
                </Button>
                <Button
                  disabled={busy}
                  onClick={() => setDetail({ kind: 'remove', worktree })}
                  size="small"
                  variant="destructive"
                >
                  <Trash2 aria-hidden="true" size={14} />
                  {t('projectWorktreeRemove')}
                </Button>
              </div>
              {taskWorktreeId === worktree.id && tasks.length > 0 ? (
                <div className="grid gap-2 border-t border-border pt-2">
                  <h4 className="text-xs font-semibold text-secondary-text">{t('projectWorktreeTaskPickerTitle')}</h4>
                  {tasks.map((task) => (
                    <Button
                      className="justify-start"
                      disabled={busy}
                      key={task.id}
                      onClick={() => confirmTask(worktree, task)}
                      size="small"
                      variant="secondary"
                    >
                      <span className="min-w-0 truncate">{task.id}</span>
                      <span className="min-w-0 truncate font-mono text-xs">{task.command}</span>
                    </Button>
                  ))}
                </div>
              ) : null}
            </article>
          ))}
        </div>
      )}
      {truncated ? <p className="text-xs text-secondary-text">{t('projectWorktreeListTruncated')}</p> : null}
      <div className="flex flex-wrap justify-between gap-2">
        <Button
          disabled={busy || isLoading || worktrees.length >= 20}
          onClick={() => void createWorktree()}
          size="small"
          variant="secondary"
        >
          {isCreating ? t('projectWorktreesLoading') : t('projectWorktreeCreate')}
        </Button>
        <div className="flex gap-2">
          <Button
            disabled={isCreating || isUsing || removingId !== null}
            onClick={() => void refreshWorktrees()}
            size="small"
          >
            {t('retry')}
          </Button>
          <Button disabled={busy} onClick={onClose}>
            {t('close')}
          </Button>
        </div>
      </div>
      <Dialog
        className="max-h-dialog-viewport overflow-y-auto"
        onOpenChange={(open) => {
          if (!open) setDetail(null);
        }}
        open={detail !== null}
        title={detailTitle}
      >
        {detail?.kind === 'remove' ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('projectWorktreeRemoveDescription', { branch: detail.worktree.branch })}
            </p>
            <div className="flex justify-end gap-2">
              <Button onClick={() => setDetail(null)}>{t('cancel')}</Button>
              <Button onClick={() => void removeWorktree(detail.worktree)} variant="destructive">
                {t('projectWorktreeRemove')}
              </Button>
            </div>
          </div>
        ) : null}
        {detail?.kind === 'task' ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('projectWorktreeBranch', { branch: detail.worktree.branch })}
            </p>
            <div className="grid gap-2">
              <span className="text-sm font-medium text-text">{t('projectWorktreeTaskCommand')}</span>
              <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all rounded-control border border-border bg-surface p-3 font-mono text-xs text-text">
                {detail.task.command}
              </pre>
            </div>
            <p className="text-sm text-secondary-text">
              {t('projectWorktreeTaskTimeout', { seconds: detail.task.timeoutSeconds })}
            </p>
            <div className="flex justify-end gap-2">
              <Button onClick={() => setDetail(null)}>{t('cancel')}</Button>
              <Button onClick={() => void runTask(detail.worktree, detail.task)} variant="primary">
                {t('projectWorktreeTaskRun')}
              </Button>
            </div>
          </div>
        ) : null}
        {detail?.kind === 'review' ? <WorktreeReview result={detail.result} worktree={detail.worktree} /> : null}
        {detail?.kind === 'result' ? <WorktreeTaskResult detail={detail} onClose={() => setDetail(null)} /> : null}
      </Dialog>
    </div>
  );
}

function WorktreeReview({ worktree, result }: { worktree: Worktree; result: ReviewResult }) {
  const { t } = useTranslation();
  if (result.changedFileCount === 0 && result.staged.length === 0 && result.unstaged.length === 0) {
    return <p className="text-sm text-secondary-text">{t('projectWorktreeNoChanges')}</p>;
  }
  return (
    <div className="openchat-scroll-area grid max-h-worktree-details gap-3 overflow-auto">
      <p className="text-sm text-secondary-text">{t('projectWorktreeBranch', { branch: worktree.branch })}</p>
      {result.staged ? <DiffBlock label={t('projectWorktreeStagedDiff')} value={result.staged} /> : null}
      {result.unstaged ? <DiffBlock label={t('projectWorktreeUnstagedDiff')} value={result.unstaged} /> : null}
      {result.changedFileCount > 0 && !result.staged && !result.unstaged ? (
        <p className="text-sm text-secondary-text">
          {t('projectWorktreeStatusChanges', { count: result.changedFileCount })}
        </p>
      ) : null}
    </div>
  );
}

function DiffBlock({ label, value }: { label: string; value: string }) {
  return (
    <section className="grid min-w-0 gap-2">
      <h3 className="text-sm font-medium text-text">{label}</h3>
      <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-all rounded-control border border-border bg-surface p-3 font-mono text-xs text-text">
        {value}
      </pre>
    </section>
  );
}

function WorktreeTaskResult({
  detail,
  onClose,
}: {
  detail: Extract<DetailDialog, { kind: 'result' }>;
  onClose(): void;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-3">
      <p aria-live="polite" className="text-sm text-text">
        {detail.status}
      </p>
      {detail.truncated ? (
        <p className="text-xs text-secondary-text">{t('projectWorktreeTaskOutputTruncated')}</p>
      ) : null}
      <h3 className="text-sm font-medium text-text">{t('projectWorktreeTaskOutput')}</h3>
      <pre className="openchat-scroll-area max-h-worktree-output overflow-auto whitespace-pre-wrap break-all rounded-control border border-border bg-surface p-3 font-mono text-xs text-text">
        {detail.output || t('projectWorktreeTaskNoOutput')}
      </pre>
      <div className="flex justify-end">
        <Button onClick={onClose}>{t('close')}</Button>
      </div>
    </div>
  );
}

function parseWorktree(value: unknown): Worktree {
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    !/^[a-f0-9]{32}$/.test(value.id) ||
    typeof value.branch !== 'string' ||
    value.branch.length === 0 ||
    typeof value.path !== 'string' ||
    value.path.length === 0
  ) {
    throw new Error('The worktree response is invalid.');
  }
  const status = value.status;
  const files = isObject(status) && Array.isArray(status.files) ? status.files : [];
  return { id: value.id, branch: value.branch, path: value.path, changedFileCount: files.length };
}

function parseProjectTask(value: unknown): ProjectTask {
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    !/^[a-z0-9_-]{1,64}$/.test(value.id) ||
    typeof value.command !== 'string' ||
    value.command.trim().length === 0 ||
    value.command.length > 4096 ||
    typeof value.timeoutSeconds !== 'number' ||
    !Number.isInteger(value.timeoutSeconds) ||
    value.timeoutSeconds < 5 ||
    value.timeoutSeconds > 600
  ) {
    throw new Error('A named project task is invalid.');
  }
  return { id: value.id, command: value.command, timeoutSeconds: value.timeoutSeconds };
}

function worktreeErrorKey(
  error: unknown,
  fallback: WorktreeErrorKey = 'projectWorktreeOperationFailed',
): WorktreeErrorKey {
  if (!(error instanceof LocalServiceError)) return fallback;
  return error.code === 'project_not_git_repository' ? 'projectWorktreeNotRepository' : fallback;
}

function taskErrorKey(error: unknown, cancelling: boolean): WorktreeErrorKey {
  if (!(error instanceof LocalServiceError)) return 'projectWorktreeTaskRunFailed';
  if (error.code === 'operation_cancelled' && cancelling) return 'projectWorktreeTaskCancelled';
  if (error.code === 'project_task_changed') return 'projectWorktreeTaskLoadFailed';
  if (error.code === 'project_task_execution_failed') return 'projectWorktreeTaskRunFailed';
  if (error.code === 'permission_denied') return 'projectWorktreeTaskDenied';
  return 'projectWorktreeOperationFailed';
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
