import {
  Activity,
  CircleAlert,
  CircleCheck,
  Clock3,
  FolderSearch,
  ListChecks,
  Plus,
  ShieldCheck,
  StopCircle,
} from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Checkbox, Select } from '@/design-system';
import { MessageList } from '@/modules/conversations/components/MessageList';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { formatFailureCodeMessage } from '@/modules/conversations/stores/chatStoreErrorDetails';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import {
  formatSecurityScanError,
  initializeSecurityScan,
  refreshSecurityScan,
  resumeSecurityScan,
  stopSecurityScan,
  useSecurityScanRuntime,
} from '../data/securityScanRuntime';
import {
  createSecurityScan,
  listSecurityScans,
  type SecurityScan,
  type SecurityScanStatus,
} from '../data/securityScans';

export function SecurityScanPage() {
  const { t } = useTranslation();
  const projects = useChatStore((state) => state.projects);
  const models = useChatStore((state) => state.models);
  const [scans, setScans] = useState<SecurityScan[]>([]);
  const [selectedScanId, setSelectedScanId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshTime, setRefreshTime] = useState(Date.now());

  const refreshScans = useCallback(async () => {
    try {
      const savedScans = await listSecurityScans();
      setScans(savedScans);
      setError(null);
      for (const scan of savedScans) void initializeSecurityScan(scan);
      setSelectedScanId((current) => current ?? savedScans[0]?.id ?? null);
    } catch (cause) {
      setError(formatSecurityScanError(cause, t, 'securityScansLoadFailed'));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void refreshScans();
    const listTimer = window.setInterval(() => void refreshScans(), 5000);
    const elapsedTimer = window.setInterval(() => setRefreshTime(Date.now()), 1000);
    return () => {
      window.clearInterval(listTimer);
      window.clearInterval(elapsedTimer);
    };
  }, [refreshScans]);

  const selectedScan = scans.find((scan) => scan.id === selectedScanId) ?? null;
  const runtime = useSecurityScanRuntime(selectedScan?.id ?? null);

  useEffect(() => {
    if (!selectedScan) return;
    void refreshSecurityScan(selectedScan);
    const timer = window.setInterval(() => void refreshSecurityScan(selectedScan), 2500);
    return () => window.clearInterval(timer);
  }, [selectedScan]);

  const capableModels = useMemo(() => models.filter((model) => model.supportsTools !== false), [models]);
  const providers = useMemo(
    () => [...new Map(capableModels.map((model) => [model.providerId, model.providerId])).values()].sort(),
    [capableModels],
  );

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden bg-background">
      <aside className="conversation-sidebar app-content-divider flex h-full shrink-0 flex-col border-y-0 border-r bg-navigation">
        <div className="flex min-h-16 items-center gap-3 border-b border-border px-4 py-3">
          <ShieldCheck aria-hidden="true" className="shrink-0 text-accent" size={20} />
          <h1 className="min-w-0 truncate text-base font-semibold text-text">{t('securityScansTitle')}</h1>
        </div>
        <div className="flex items-center justify-between px-4 py-3">
          <h2 className="text-sm font-medium text-secondary-text">{t('securityScanList')}</h2>
          <span className="tabular-nums text-xs text-secondary-text">{scans.length}</span>
        </div>
        <div className="openchat-scroll-area conversation-sidebar-scroll-area min-w-0 flex-1 px-2.5 pb-4 pt-1">
          {isLoading ? (
            <p className="px-2 py-3 text-sm text-secondary-text" role="status">
              {t('securityScansLoading')}
            </p>
          ) : scans.length === 0 ? (
            <p className="px-2 py-3 text-sm text-secondary-text">{t('securityScansEmpty')}</p>
          ) : (
            <ul className="grid gap-1">
              {scans.map((scan) => (
                <ScanListItem
                  key={scan.id}
                  active={scan.id === selectedScanId}
                  now={refreshTime}
                  onSelect={() => {
                    setShowCreateForm(false);
                    setSelectedScanId(scan.id);
                  }}
                  scan={scan}
                />
              ))}
            </ul>
          )}
        </div>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
        <header className="flex min-h-16 shrink-0 items-center justify-end gap-4 border-b border-border px-4 py-3 md:px-6">
          <Button
            onClick={() => {
              setShowCreateForm((current) => !current);
              setError(null);
            }}
            size="small"
            variant={showCreateForm ? 'secondary' : 'primary'}
          >
            <Plus aria-hidden="true" size={16} />
            {t(showCreateForm ? 'securityScanCloseCreate' : 'securityScanNew')}
          </Button>
        </header>

        {error ? (
          <div className="mx-4 mt-3 flex items-start gap-2 rounded-control border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive md:mx-6">
            <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0" size={16} />
            <p role="alert">{error}</p>
          </div>
        ) : null}

        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          {showCreateForm ? (
            <NewScanForm
              capableModels={capableModels}
              isCreating={isCreating}
              onCancel={() => setShowCreateForm(false)}
              onCreate={async (project, model, selectedRoots, publicWebResearch) => {
                setIsCreating(true);
                setError(null);
                try {
                  const scan = await createSecurityScan({
                    id: crypto.randomUUID().replaceAll('-', ''),
                    conversationId: crypto.randomUUID().replaceAll('-', ''),
                    project,
                    route: model,
                    selectedRoots,
                    publicWebResearch,
                  });
                  setScans((current) => [scan, ...current]);
                  setSelectedScanId(scan.id);
                  setShowCreateForm(false);
                  await initializeSecurityScan(scan);
                } catch (cause) {
                  setError(formatSecurityScanError(cause, t, 'securityScanCreateFailed'));
                } finally {
                  setIsCreating(false);
                }
              }}
              onError={(message) => setError(message)}
              projects={projects}
              providers={providers}
            />
          ) : selectedScan ? (
            <ScanDetail
              goal={runtime.goal}
              isActive={runtime.isActive}
              messages={runtime.messages}
              now={refreshTime}
              onResume={() => void resumeSecurityScan(selectedScan, runtime.goal)}
              onStop={() => void stopSecurityScan(selectedScan)}
              scan={selectedScan}
              runtimeError={formatSecurityScanError(runtime.error, t)}
            />
          ) : (
            <EmptySelection isLoading={isLoading} onCreate={() => setShowCreateForm(true)} />
          )}
        </div>
      </div>
    </div>
  );
}

interface ScanListItemProps {
  active: boolean;
  now: number;
  onSelect(): void;
  scan: SecurityScan;
}

const statusLabels = {
  queued: 'securityScanStatusQueued',
  running: 'securityScanStatusRunning',
  waiting_provider: 'securityScanStatusWaitingProvider',
  paused: 'securityScanStatusPaused',
  interrupted: 'securityScanStatusInterrupted',
  completed: 'securityScanStatusCompleted',
  cancelled: 'securityScanStatusCancelled',
  failed: 'securityScanStatusFailed',
} as const satisfies Record<SecurityScanStatus, `securityScanStatus${string}`>;

function ScanListItem({ active, now, onSelect, scan }: ScanListItemProps) {
  const { t } = useTranslation();
  const status = visibleScanStatus(scan, null);
  return (
    <li>
      <button
        aria-current={active ? 'true' : undefined}
        className={`w-full rounded-control border px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${active ? 'border-border bg-selected' : 'border-transparent hover:bg-hover'}`}
        onClick={onSelect}
        type="button"
      >
        <span className="flex min-w-0 items-center justify-between gap-2">
          <span className="truncate text-sm font-medium text-text">{scan.projectName}</span>
          <StatusDot status={status} />
        </span>
        <span className="mt-1.5 flex items-center justify-between gap-2 text-xs text-secondary-text">
          <span className="truncate">{t(statusLabelKey(status))}</span>
          <span className="shrink-0 tabular-nums">{formatElapsed(scan, now)}</span>
        </span>
      </button>
    </li>
  );
}

interface NewScanFormProps {
  capableModels: ChatModel[];
  isCreating: boolean;
  onCancel(): void;
  onCreate(project: ChatProject, model: ChatModel, selectedRoots: string[], publicWebResearch: boolean): Promise<void>;
  onError(message: string): void;
  projects: ChatProject[];
  providers: string[];
}

function NewScanForm({
  capableModels,
  isCreating,
  onCancel,
  onCreate,
  onError,
  projects,
  providers,
}: NewScanFormProps) {
  const { t } = useTranslation();
  const rootInputId = useId();
  const publicWebResearchInputId = useId();
  const [projectId, setProjectId] = useState(projects[0]?.id ?? '');
  const [providerId, setProviderId] = useState(providers[0] ?? '');
  const [modelKey, setModelKey] = useState('');
  const [selectedRoots, setSelectedRoots] = useState<string[]>([]);
  const [publicWebResearch, setPublicWebResearch] = useState(false);
  const project = projects.find((candidate) => candidate.id === projectId) ?? null;
  const roots = useMemo(() => projectRoots(project), [project]);
  const providerModels = useMemo(
    () => capableModels.filter((model) => model.providerId === providerId),
    [capableModels, providerId],
  );
  const model = providerModels.find((candidate) => candidate.key === modelKey) ?? providerModels[0] ?? null;

  useEffect(() => {
    if (!projects.some((candidate) => candidate.id === projectId)) {
      setProjectId(projects[0]?.id ?? '');
    }
  }, [projectId, projects]);
  useEffect(() => {
    if (!providers.includes(providerId)) setProviderId(providers[0] ?? '');
  }, [providerId, providers]);
  useEffect(() => {
    setSelectedRoots(roots);
  }, [roots]);
  useEffect(() => {
    if (!providerModels.some((candidate) => candidate.key === modelKey)) {
      setModelKey(providerModels[0]?.key ?? '');
    }
  }, [modelKey, providerModels]);

  const toggleRoot = (root: string, checked: boolean) => {
    setSelectedRoots((current) =>
      checked ? [...new Set([...current, root])] : current.filter((candidate) => candidate !== root),
    );
  };

  const submit = async () => {
    if (!project || !model || selectedRoots.length === 0) {
      onError(t('securityScanSelectionRequired'));
      return;
    }
    await onCreate(project, model, selectedRoots, publicWebResearch);
  };

  return (
    <section className="openchat-scroll-area min-h-0 flex-1 overflow-y-auto px-4 py-6 md:px-8">
      <div className="mx-auto max-w-2xl">
        <div className="mb-6 flex items-start gap-3">
          <FolderSearch aria-hidden="true" className="mt-0.5 text-accent" size={20} />
          <div>
            <h2 className="text-base font-semibold text-text">{t('securityScanNew')}</h2>
            <p className="mt-1 text-sm text-secondary-text">{t('securityScanSetupDescription')}</p>
          </div>
        </div>

        {projects.length === 0 ? (
          <div className="rounded-control border border-border px-4 py-3 text-sm text-secondary-text">
            {t('securityScanNoProjects')}
          </div>
        ) : (
          <div className="grid gap-5">
            <Select
              label={t('securityScanProject')}
              onChange={(event) => setProjectId(event.target.value)}
              options={projects.map((item) => ({ value: item.id, label: item.name }))}
              value={projectId}
            />

            <fieldset className="grid gap-2">
              <legend className="text-sm font-medium text-text">{t('securityScanFolders')}</legend>
              <p className="text-xs text-secondary-text">{t('securityScanFoldersDescription')}</p>
              <div className="grid gap-1">
                {roots.map((root, index) => {
                  const inputId = `${rootInputId}-${index}`;
                  return (
                    <label
                      className="flex min-w-0 cursor-pointer items-start gap-2 rounded-control px-2 py-2 text-sm hover:bg-hover"
                      htmlFor={inputId}
                      key={root}
                    >
                      <Checkbox
                        checked={selectedRoots.includes(root)}
                        className="mt-0.5"
                        id={inputId}
                        onChange={(event) => toggleRoot(root, event.currentTarget.checked)}
                      />
                      <span className="min-w-0 break-all font-code text-xs text-text">{root}</span>
                    </label>
                  );
                })}
              </div>
            </fieldset>

            <label
              className="flex cursor-pointer items-start gap-2 rounded-control border border-border px-3 py-3"
              htmlFor={publicWebResearchInputId}
            >
              <Checkbox
                checked={publicWebResearch}
                className="mt-0.5"
                id={publicWebResearchInputId}
                onChange={(event) => setPublicWebResearch(event.currentTarget.checked)}
              />
              <span className="grid gap-1">
                <span className="text-sm font-medium text-text">{t('securityScanPublicWebResearch')}</span>
                <span className="text-xs text-secondary-text">{t('securityScanPublicWebResearchDescription')}</span>
              </span>
            </label>

            {capableModels.length === 0 ? (
              <div className="rounded-control border border-border px-4 py-3 text-sm text-secondary-text">
                {t('securityScanNoModels')}
              </div>
            ) : (
              <div className="grid gap-4 sm:grid-cols-2">
                <Select
                  label={t('securityScanProvider')}
                  onChange={(event) => {
                    setProviderId(event.target.value);
                    setModelKey('');
                  }}
                  options={providers.map((id) => ({ value: id, label: id }))}
                  value={providerId}
                />
                <Select
                  label={t('securityScanModel')}
                  onChange={(event) => setModelKey(event.target.value)}
                  options={providerModels.map((item) => ({ value: item.key, label: item.displayName }))}
                  value={model?.key ?? ''}
                />
              </div>
            )}

            <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-4">
              <Button onClick={onCancel} variant="ghost">
                {t('cancel')}
              </Button>
              <Button
                disabled={!project || !model || selectedRoots.length === 0 || capableModels.length === 0}
                loading={isCreating}
                onClick={() => void submit()}
                variant="primary"
              >
                <ShieldCheck aria-hidden="true" size={16} />
                {t('securityScanStart')}
              </Button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

interface ScanDetailProps {
  goal: ReturnType<typeof useSecurityScanRuntime>['goal'];
  isActive: boolean;
  messages: ReturnType<typeof useSecurityScanRuntime>['messages'];
  now: number;
  onResume(): void;
  onStop(): void;
  scan: SecurityScan;
  runtimeError: string | null;
}

function ScanDetail({ goal, isActive, messages, now, onResume, onStop, scan, runtimeError }: ScanDetailProps) {
  const { t } = useTranslation();
  const status = visibleScanStatus(scan, goal);
  const pendingStepIndex = goal?.todos.findIndex((todo) => !todo.completed) ?? -1;
  const pendingStep = pendingStepIndex >= 0 ? (goal?.todos[pendingStepIndex]?.text ?? null) : null;
  const completedSteps = goal?.todos.filter((todo) => todo.completed).length ?? 0;
  const totalSteps = goal?.todos.length ?? 0;
  const canStop = isActive || !['completed', 'cancelled', 'failed'].includes(status);
  const hasFailedMessage = messages.some((message) => message.status === 'failed');
  const displayedError =
    runtimeError ?? (!hasFailedMessage && status === 'failed' ? formatFailureCodeMessage(scan.failureCode, t) : null);
  const canResume =
    status === 'paused' ||
    status === 'interrupted' ||
    status === 'waiting_provider' ||
    (status === 'queued' && (displayedError !== null || hasFailedMessage));

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-border px-4 py-3 md:px-6">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="max-w-full truncate text-base font-semibold text-text">{scan.projectName}</h2>
            <StatusBadge status={status} />
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-secondary-text">
            <span>{scan.modelId ?? t('securityScanModelUnavailable')}</span>
            <span className="inline-flex items-center gap-1 tabular-nums">
              <Clock3 aria-hidden="true" size={13} />
              {formatElapsed(scan, now)}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {canResume ? (
            <Button onClick={onResume} size="small" variant="secondary">
              {t(status === 'waiting_provider' ? 'securityScanResumeNow' : 'securityScanResume')}
            </Button>
          ) : null}
          {canStop ? (
            <Button onClick={onStop} size="small" variant="destructive">
              <StopCircle aria-hidden="true" size={15} />
              {t('securityScanStop')}
            </Button>
          ) : null}
        </div>
      </header>

      <section aria-label={t('securityScanPlan')} className="shrink-0 border-b border-border px-4 py-3 md:px-6">
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            {status === 'running' ? (
              <Activity aria-hidden="true" className="shrink-0 text-accent motion-safe:animate-pulse" size={16} />
            ) : (
              <ListChecks aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            )}
            <span className="truncate text-sm font-medium text-text">
              {pendingStep ?? (goal?.todos.length ? t('securityScanPlanComplete') : t('securityScanInspecting'))}
            </span>
          </div>
          {totalSteps > 0 ? (
            <span className="shrink-0 text-xs tabular-nums text-secondary-text">
              {completedSteps}/{totalSteps}
            </span>
          ) : null}
        </div>
        {totalSteps > 0 ? (
          <progress
            aria-label={t('securityScanPlanProgress')}
            className="mt-2 h-1.5 w-full overflow-hidden rounded-full accent-accent"
            max={totalSteps}
            value={completedSteps}
          />
        ) : null}
        {goal?.todos.length ? (
          <div className="openchat-scroll-area conversation-sidebar-scroll-area mt-2.5 max-h-36 rounded-control border border-border bg-surface p-1.5">
            <ol className="grid grid-cols-1 gap-1">
              {goal.todos.map((todo, index) => {
                const isCurrentStep = !todo.completed && index === pendingStepIndex;
                const duplicateIndex = goal.todos.slice(0, index).filter((item) => item.text === todo.text).length;
                return (
                  <li
                    className={`flex min-w-0 items-start gap-2 rounded-control px-2 py-1 text-xs ${
                      isCurrentStep ? 'bg-selected font-medium text-text' : 'text-secondary-text'
                    }`}
                    key={`${todo.text}:${duplicateIndex}`}
                  >
                    {todo.completed ? (
                      <CircleCheck aria-hidden="true" className="mt-0.5 shrink-0 text-success" size={14} />
                    ) : isCurrentStep ? (
                      <span aria-hidden="true" className="mt-1.5 size-2 shrink-0 rounded-full bg-accent" />
                    ) : (
                      <span aria-hidden="true" className="mt-1.5 size-2 shrink-0 rounded-full border border-border" />
                    )}
                    <span className={`min-w-0 flex-1 break-words leading-5 ${todo.completed ? 'line-through' : ''}`}>
                      {todo.text}
                    </span>
                  </li>
                );
              })}
            </ol>
          </div>
        ) : null}
        {goal?.progress ? <p className="mt-2 line-clamp-2 text-xs text-secondary-text">{goal.progress}</p> : null}
      </section>

      {status === 'waiting_provider' ? (
        <div className="mx-4 mt-3 flex shrink-0 items-start gap-2 rounded-control border border-border bg-background px-3 py-2 text-sm text-secondary-text md:mx-6">
          <Clock3 aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
          <p>{t('securityScanQuotaWaiting')}</p>
        </div>
      ) : null}
      {displayedError ? (
        <div className="mx-4 mt-3 flex shrink-0 items-start gap-2 rounded-control border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive md:mx-6">
          <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
          <p role="alert">{displayedError}</p>
        </div>
      ) : null}

      {messages.length > 0 ? (
        <MessageList
          assistantModelFallback={scan.modelId}
          assistantProviderFallback={scan.providerId}
          conversationIdOverride={scan.conversationId}
          isLoadingMessagesOverride={false}
          messagesOverride={messages}
        />
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center p-6">
          <div className="max-w-sm text-center">
            {status === 'running' || isActive ? (
              <Activity aria-hidden="true" className="mx-auto mb-3 text-accent motion-safe:animate-pulse" size={24} />
            ) : (
              <ShieldCheck aria-hidden="true" className="mx-auto mb-3 text-secondary-icon" size={24} />
            )}
            <p className="text-sm text-secondary-text">
              {status === 'running' || isActive ? t('securityScanInspecting') : t('securityScanNoMessages')}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

function EmptySelection({ isLoading, onCreate }: { isLoading: boolean; onCreate(): void }) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center p-6">
      <div className="max-w-sm text-center">
        <ShieldCheck aria-hidden="true" className="mx-auto mb-3 text-secondary-icon" size={25} />
        <p className="text-sm text-secondary-text">
          {isLoading ? t('securityScansLoading') : t('securityScansChooseOrStart')}
        </p>
        {!isLoading ? (
          <Button className="mt-4" onClick={onCreate} variant="secondary">
            <Plus aria-hidden="true" size={16} />
            {t('securityScanNew')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: SecurityScanStatus }) {
  const { t } = useTranslation();
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs ${statusColor(status)}`}>
      <StatusDot status={status} />
      {t(statusLabelKey(status))}
    </span>
  );
}

function StatusDot({ status }: { status: SecurityScanStatus }) {
  return (
    <span
      aria-hidden="true"
      className={`size-1.5 shrink-0 rounded-full ${
        status === 'running'
          ? 'bg-accent motion-safe:animate-pulse'
          : status === 'completed'
            ? 'bg-success'
            : status === 'failed'
              ? 'bg-destructive'
              : status === 'cancelled'
                ? 'bg-secondary-icon'
                : status === 'waiting_provider' || status === 'paused' || status === 'interrupted'
                  ? 'bg-warning'
                  : 'bg-secondary-icon'
      }`}
    />
  );
}

function visibleScanStatus(
  scan: SecurityScan,
  goal: ReturnType<typeof useSecurityScanRuntime>['goal'],
): SecurityScanStatus {
  if (goal?.status === 'running') return 'running';
  if (goal?.status === 'interrupted') return 'interrupted';
  if (goal?.status === 'paused') return goal.pauseReason === 'quota' ? 'waiting_provider' : 'paused';
  if (goal?.status === 'cancelled') return 'cancelled';
  if (goal?.status === 'completed') return 'completed';
  if (goal?.status === 'failed') return 'failed';
  return scan.status;
}

function statusLabelKey(status: SecurityScanStatus): (typeof statusLabels)[SecurityScanStatus] {
  return statusLabels[status];
}

function statusColor(status: SecurityScanStatus): string {
  if (status === 'completed') return 'border-success/30 bg-success/5 text-success';
  if (status === 'failed') return 'border-destructive/30 bg-destructive/5 text-destructive';
  if (status === 'running') return 'border-accent/30 bg-accent/5 text-accent';
  if (status === 'waiting_provider' || status === 'paused' || status === 'interrupted') {
    return 'border-warning/30 bg-warning/5 text-text';
  }
  return 'border-border bg-background text-secondary-text';
}

function projectRoots(project: ChatProject | null): string[] {
  if (!project) return [];
  return project.workspaceRoots.length > 0 ? project.workspaceRoots : [project.folderPath];
}

function formatElapsed(scan: SecurityScan, now: number): string {
  const end = scan.finishedAtUnixMs ?? now;
  const seconds = Math.max(0, Math.floor((end - scan.createdAtUnixMs) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`
    : `${minutes}:${String(remainder).padStart(2, '0')}`;
}
