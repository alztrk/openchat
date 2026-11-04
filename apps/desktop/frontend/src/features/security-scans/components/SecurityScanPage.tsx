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
  Trash2,
} from 'lucide-react';
import {
  createContext,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

// Time constants
const MS_PER_SECOND = 1_000;
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3_600;
const REFRESH_INTERVAL_MS = 5_000;
const INITIAL_CHECK_DELAY_MS = 2_500;
const SIDEBAR_DEFAULT_WIDTH = 280;
const SIDEBAR_MIN_WIDTH = 180;
const SIDEBAR_MAX_WIDTH = 420;
const SIDEBAR_WIDTH_STORAGE_KEY = 'security_scans.sidebar_width';

function clampSecurityScanSidebarWidth(width: number): number {
  return Math.round(Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, width)));
}

function readSecurityScanSidebarWidth(): number {
  const storedWidth = unifiedStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY);
  if (storedWidth === null) return SIDEBAR_DEFAULT_WIDTH;
  const width = Number(storedWidth);
  return Number.isFinite(width) ? clampSecurityScanSidebarWidth(width) : SIDEBAR_DEFAULT_WIDTH;
}

import { Button, Checkbox, Dialog, Select, TitleTooltip } from '@shared/components';
import { MessageList } from '@features/chat/components';
import type { ChatProject } from '@entities/project';
import { useChatStore } from '@features/chat/state';
import { formatFailureCodeMessage } from '@features/chat/state';
import { ProviderIcon } from '@shared/components';
import type { ChatModel } from '@entities/model';
import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';
import {
  deleteSecurityScanRuntime,
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
  securityScanObjective,
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
  const [deleteTarget, setDeleteTarget] = useState<SecurityScan | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(readSecurityScanSidebarWidth);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const isRefreshingScansRef = useRef(false);

  const refreshScans = useCallback(async () => {
    if (isRefreshingScansRef.current) return;
    isRefreshingScansRef.current = true;
    try {
      const savedScans = await listSecurityScans();
      setScans(savedScans);
      setError(null);
      for (const scan of savedScans) void initializeSecurityScan(scan);
      setSelectedScanId((current) =>
        current && savedScans.some((scan) => scan.id === current) ? current : (savedScans[0]?.id ?? null),
      );
    } catch (cause) {
      setError(formatSecurityScanError(cause, t, 'securityScansLoadFailed'));
    } finally {
      isRefreshingScansRef.current = false;
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void refreshScans();
    const listTimer = window.setInterval(() => void refreshScans(), REFRESH_INTERVAL_MS);
    return () => {
      window.clearInterval(listTimer);
    };
  }, [refreshScans]);

  const selectedScan = scans.find((scan) => scan.id === selectedScanId) ?? null;
  const activeScanId = selectedScan?.id ?? null;
  const selectedScanRef = useRef<SecurityScan | null>(selectedScan);
  selectedScanRef.current = selectedScan;
  const runtime = useSecurityScanRuntime(activeScanId);
  const selectedStatus = selectedScan ? visibleScanStatus(selectedScan, runtime.goal) : null;

  useEffect(() => {
    const current = selectedScanRef.current;
    if (!activeScanId || !current || current.id !== activeScanId || runtime.isActive) return;
    void refreshSecurityScan(current);
    const timer = window.setInterval(() => {
      const latest = selectedScanRef.current;
      if (latest && latest.id === activeScanId) void refreshSecurityScan(latest);
    }, INITIAL_CHECK_DELAY_MS);
    return () => window.clearInterval(timer);
  }, [activeScanId, runtime.isActive]);

  const confirmDeleteScan = async () => {
    if (!deleteTarget || isDeleting) return;
    const target = deleteTarget;
    setIsDeleting(true);
    setError(null);
    try {
      await deleteSecurityScanRuntime(target);
      setScans((current) => {
        const remaining = current.filter((item) => item.id !== target.id);
        setSelectedScanId((selected) => (selected === target.id ? (remaining[0]?.id ?? null) : selected));
        return remaining;
      });
      setDeleteTarget(null);
    } catch (cause) {
      setError(formatSecurityScanError(cause, t, 'securityScanDeleteFailed'));
    } finally {
      setIsDeleting(false);
    }
  };

  const handleSidebarResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    setIsResizingSidebar(true);

    const handlePointerMove = (moveEvent: PointerEvent) => {
      setSidebarWidth(clampSecurityScanSidebarWidth(startWidth + moveEvent.clientX - startX));
    };

    const handlePointerEnd = (endEvent: PointerEvent) => {
      const finalWidth = clampSecurityScanSidebarWidth(startWidth + endEvent.clientX - startX);
      setSidebarWidth(finalWidth);
      setIsResizingSidebar(false);
      unifiedStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(finalWidth));
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerEnd);
      window.removeEventListener('pointercancel', handlePointerEnd);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerEnd);
    window.addEventListener('pointercancel', handlePointerEnd);
  };

  const handleSidebarResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.preventDefault();
      const width = clampSecurityScanSidebarWidth(sidebarWidth + (event.key === 'ArrowRight' ? 12 : -12));
      setSidebarWidth(width);
      unifiedStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(width));
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      const width = event.key === 'Home' ? SIDEBAR_MIN_WIDTH : SIDEBAR_MAX_WIDTH;
      setSidebarWidth(width);
      unifiedStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(width));
    }
  };

  const capableModels = useMemo(() => models.filter((model) => model.supportsTools !== false), [models]);
  const providers = useMemo(
    () => [...new Map(capableModels.map((model) => [model.providerId, model.providerId])).values()].sort(),
    [capableModels],
  );

  return (
    <SecurityScanClockProvider active={scans.some((scan) => scan.finishedAtUnixMs === null)}>
      <div className="security-scan-page-container flex min-h-0 min-w-0 flex-1">
      <div className="security-scan-page flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden bg-background">
      <aside
        className={`security-scan-sidebar conversation-sidebar app-content-divider relative flex h-full shrink-0 flex-col border-y-0 border-r bg-navigation ${isResizingSidebar ? 'select-none' : ''}`}
        style={{ width: `${sidebarWidth}px` }}
      >
        <div className="flex min-h-16 items-center gap-2 border-b border-border px-3 py-3">
          <ShieldCheck aria-hidden="true" className="shrink-0 text-accent" size={20} />
          <h1 className="min-w-0 flex-1 truncate text-base font-semibold text-text">{t('securityScansTitle')}</h1>
          <Button
            aria-label={t(showCreateForm ? 'securityScanCloseCreate' : 'securityScanNew')}
            className="shrink-0 px-2"
            onClick={() => {
              setShowCreateForm((current) => !current);
              setError(null);
            }}
            size="small"
            title={t(showCreateForm ? 'securityScanCloseCreate' : 'securityScanNew')}
            variant="secondary"
          >
            <Plus aria-hidden="true" size={16} />
            <span className="security-scan-new-label">{t(showCreateForm ? 'securityScanCloseCreate' : 'securityScanNew')}</span>
          </Button>
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
                  modelLabel={resolveScanModelLabel(scan, models, t('securityScanModelUnavailable'))}
                  onDelete={() => setDeleteTarget(scan)}
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
        {/* biome-ignore lint/a11y/useSemanticElements: Custom interactive resize separator for the security scan sidebar. */}
        <TitleTooltip title={t('resizeSidebar')}><div
          aria-label={t('resizeSidebar')}
          aria-orientation="vertical"
          aria-valuemax={SIDEBAR_MAX_WIDTH}
          aria-valuemin={SIDEBAR_MIN_WIDTH}
          aria-valuenow={sidebarWidth}
          className="security-scan-sidebar-resizer group/resizer absolute inset-y-0 right-0 z-20 w-1.5 cursor-col-resize select-none focus-visible:outline-none"
          onDoubleClick={() => {
            setSidebarWidth(SIDEBAR_DEFAULT_WIDTH);
            unifiedStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(SIDEBAR_DEFAULT_WIDTH));
          }}
          onKeyDown={handleSidebarResizeKeyDown}
          onPointerDown={handleSidebarResizePointerDown}
          role="separator"
          tabIndex={0}

        >
          <span
            className={`pointer-events-none absolute inset-y-0 right-0 w-0.5 transition-colors ${isResizingSidebar ? 'bg-focus' : 'bg-transparent group-hover/resizer:bg-border group-focus-visible/resizer:bg-focus'}`}
          />
        </div></TitleTooltip>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
        <header className="flex min-h-16 shrink-0 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-border px-4 py-3 md:px-6">
          {!showCreateForm && selectedScan ? (
            <div className="flex min-w-0 flex-1 flex-wrap items-center justify-between gap-x-4 gap-y-2">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <h2 className="max-w-full truncate text-base font-semibold text-text">{selectedScan.projectName}</h2>
                </div>
                <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-secondary-text">
                  <span className="inline-flex min-w-0 items-center gap-1.5">
                    {selectedScan.providerId ? <ProviderIcon providerId={selectedScan.providerId} size={13} /> : null}
                    <span className="truncate">
                      {resolveScanModelLabel(selectedScan, models, t('securityScanModelUnavailable'))}
                    </span>
                  </span>
                  <span className="inline-flex items-center gap-1 tabular-nums">
                    <Clock3 aria-hidden="true" size={13} />
                    <SecurityScanElapsedTime scan={selectedScan} />
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {selectedStatus === 'paused' || selectedStatus === 'interrupted' || selectedStatus === 'waiting_provider' ? (
                  <Button onClick={() => void resumeSecurityScan(selectedScan, runtime.goal)} size="small" variant="secondary">
                    {t(selectedStatus === 'waiting_provider' ? 'securityScanResumeNow' : 'securityScanResume')}
                  </Button>
                ) : null}
                {runtime.isActive || (selectedStatus !== null && !['completed', 'cancelled', 'failed'].includes(selectedStatus)) ? (
                  <Button onClick={() => void stopSecurityScan(selectedScan)} size="small" variant="destructive">
                    <StopCircle aria-hidden="true" size={15} />
                    {t('securityScanStop')}
                  </Button>
                ) : null}
                <Button onClick={() => setDeleteTarget(selectedScan)} size="small" variant="ghost">
                  <Trash2 aria-hidden="true" size={15} />
                  {t('securityScanDelete')}
                </Button>
              </div>
            </div>
          ) : null}
        </header>

        {error ? (
          <div className="mx-auto mt-3 w-full max-w-conversation px-4 md:px-6">
            <div className="flex items-start gap-2 rounded-control border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
              <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0" size={16} />
              <p role="alert">{error}</p>
            </div>
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
              onStartNewScan={() => {
                setShowCreateForm(true);
                setError(null);
              }}
              scan={selectedScan}
              runtimeError={formatSecurityScanError(runtime.error, t)}
            />
          ) : (
            <EmptySelection isLoading={isLoading} onCreate={() => setShowCreateForm(true)} />
          )}
        </div>
      </div>

      <Dialog
        onOpenChange={(open) => {
          if (!open && !isDeleting) setDeleteTarget(null);
        }}
        open={deleteTarget !== null}
        title={t('securityScanDeleteConfirmTitle')}
      >
        {deleteTarget ? (
          <div className="grid gap-5">
            <p className="text-sm text-secondary-text">
              {t('securityScanDeleteConfirmBody', { project: deleteTarget.projectName })}
            </p>
            <div className="flex justify-end gap-2 border-t border-border pt-4">
              <Button disabled={isDeleting} onClick={() => setDeleteTarget(null)} variant="ghost">
                {t('cancel')}
              </Button>
              <Button
                disabled={isDeleting}
                loading={isDeleting}
                onClick={() => void confirmDeleteScan()}
                variant="destructive"
              >
                {t('securityScanDeletePermanently')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
      </div>
      </div>
    </SecurityScanClockProvider>
  );
}

const SecurityScanClockContext = createContext<number | null>(null);

function SecurityScanClockProvider({ active, children }: { active: boolean; children: ReactNode }) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!active) return;
    const update = () => setNow(Date.now());
    update();
    const timer = window.setInterval(update, MS_PER_SECOND);
    return () => window.clearInterval(timer);
  }, [active]);

  return <SecurityScanClockContext.Provider value={now}>{children}</SecurityScanClockContext.Provider>;
}

function SecurityScanElapsedTime({ scan }: { scan: SecurityScan }) {
  if (scan.finishedAtUnixMs !== null) return formatElapsed(scan, scan.finishedAtUnixMs);
  return <LiveSecurityScanElapsedTime scan={scan} />;
}

function LiveSecurityScanElapsedTime({ scan }: { scan: SecurityScan }) {
  const now = useContext(SecurityScanClockContext);
  if (now === null) throw new Error('Live security scan elapsed time requires the clock provider.');
  return formatElapsed(scan, now);
}

interface ScanListItemProps {
  active: boolean;
  modelLabel: string;
  onDelete(): void;
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

function ScanListItem({ active, modelLabel, onDelete, onSelect, scan }: ScanListItemProps) {
  const { i18n, t } = useTranslation();
  const status = visibleScanStatus(scan, null);
  const createdAtLabel = formatScanCreatedAt(scan, i18n.resolvedLanguage);
  const createdAtTimeLabel = formatScanCreatedTime(scan, i18n.resolvedLanguage);
  return (
    <li className="group relative">
      <button
        aria-current={active ? 'true' : undefined}
        className={`w-full rounded-control border px-2.5 py-1.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${active ? 'border-border bg-selected' : 'border-transparent hover:bg-hover'}`}
        onClick={onSelect}
        type="button"
      >
        <span className="flex min-w-0 items-center gap-1.5 pr-7">
          <span className="min-w-0 flex-1 truncate text-sm font-medium text-text">{scan.projectName}</span>
          <TitleTooltip title={createdAtLabel}><time className="security-scan-card-created-at shrink-0 text-xs tabular-nums text-secondary-text" dateTime={new Date(scan.createdAtUnixMs).toISOString()} >
            {createdAtLabel}
          </time></TitleTooltip>
          <TitleTooltip title={createdAtLabel}><time className="security-scan-card-created-time hidden shrink-0 text-xs tabular-nums text-secondary-text" dateTime={new Date(scan.createdAtUnixMs).toISOString()} >
            {createdAtTimeLabel}
          </time></TitleTooltip>
          <TitleTooltip title={t(statusLabelKey(status))}><span aria-label={t(statusLabelKey(status))} className="inline-flex shrink-0" role="img" >
            <StatusDot status={status} />
          </span></TitleTooltip>
        </span>
        <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-secondary-text">
          {scan.providerId ? <ProviderIcon providerId={scan.providerId} size={13} /> : null}
          <span className="min-w-0 flex-1 truncate">{modelLabel}</span>
          <span className="shrink-0 tabular-nums"><SecurityScanElapsedTime scan={scan} /></span>
        </span>
      </button>
      <Button
        aria-label={t('securityScanDeleteAria', { project: scan.projectName })}
        className="absolute right-1.5 top-1.5 size-7 opacity-0 transition-opacity focus-visible:opacity-100 group-focus-within:opacity-100 group-hover:opacity-100"
        onClick={onDelete}
        size="icon"
        variant="ghost"
      >
        <Trash2 aria-hidden="true" size={14} />
      </Button>
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
  onStartNewScan(): void;
  scan: SecurityScan;
  runtimeError: string | null;
}

function ScanDetail({
  goal,
  isActive,
  messages,
  onStartNewScan,
  scan,
  runtimeError,
}: ScanDetailProps) {
  const { t } = useTranslation();
  const status = visibleScanStatus(scan, goal);
  const pendingStepIndex = goal?.todos.findIndex((todo) => !todo.completed) ?? -1;
  const pendingStep = pendingStepIndex >= 0 ? (goal?.todos[pendingStepIndex]?.text ?? null) : null;
  const completedSteps = goal?.todos.filter((todo) => todo.completed).length ?? 0;
  const totalSteps = goal?.todos.length ?? 0;
  const visibleMessages = messages.filter(
    (message) => !(message.role === 'user' && message.content === securityScanObjective),
  );
  const hasFailedAssistantMessage = visibleMessages.some(
    (message) => message.role === 'assistant' && message.status === 'failed',
  );
  const hasAssistantReport = visibleMessages.some(
    (message) => message.role === 'assistant' && message.content.trim().length > 0,
  );
  const planStatusLabel =
    status === 'running'
      ? (pendingStep ?? t('securityScanInspecting'))
      : status === 'completed'
        ? t('securityScanPlanComplete')
        : t(statusLabelKey(status));
  const displayedError = runtimeError ?? (!hasFailedAssistantMessage && status === 'failed' ? formatFailureCodeMessage(scan.failureCode, t) : null);
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      {status !== 'failed' || totalSteps > 0 ? (
        <section aria-label={t('securityScanPlan')} className="shrink-0 border-b border-border px-4 py-3 md:px-6">
        <div className="mx-auto w-full max-w-conversation">
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
            <div className="flex min-w-0 items-center gap-2">
            {status === 'running' ? (
                <Activity aria-hidden="true" className="shrink-0 text-accent" size={16} />
              ) : status === 'completed' ? (
                <CircleCheck aria-hidden="true" className="shrink-0 text-success" size={16} />
            ) : (
                <ListChecks aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
              )}
              <div className="min-w-0">
                <p className="text-xs text-secondary-text">{t('securityScanPlan')}</p>
                {status !== 'failed' ? (
                  <p className="truncate text-sm font-medium text-text">{planStatusLabel}</p>
                ) : null}
              </div>
            </div>
            {status === 'failed' && hasFailedAssistantMessage ? (
              <Button onClick={onStartNewScan} size="small" variant="secondary">
                <Plus aria-hidden="true" size={15} />
                {t('securityScanNew')}
              </Button>
            ) : null}
          </div>
          {totalSteps > 0 ? (
            <div className="mt-2 flex items-center gap-3">
              <progress
                aria-label={t('securityScanPlanProgress')}
                className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full accent-accent"
                max={totalSteps}
                value={completedSteps}
              />
              <span className="shrink-0 text-xs tabular-nums text-secondary-text">
                {completedSteps}/{totalSteps}
              </span>
            </div>
          ) : null}
          {goal?.todos.length ? (
            <details className="mt-2.5 rounded-control border border-border">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-3 py-2 text-xs text-secondary-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus">
                <span>{t('securityScanViewPlan')}</span>
                <span className="shrink-0 tabular-nums">
                  {completedSteps}/{totalSteps}
                </span>
              </summary>
              <ol className="openchat-scroll-area conversation-sidebar-scroll-area grid max-h-36 grid-cols-1 gap-1 overflow-y-auto border-t border-border p-1.5">
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
            </details>
          ) : null}
        </div>
        </section>
      ) : null}

      {status === 'waiting_provider' ? (
        <div className="mx-auto mt-3 w-full max-w-conversation px-4 md:px-6">
          <div className="flex items-start gap-2 rounded-control border border-border bg-background px-3 py-2 text-sm text-secondary-text">
            <Clock3 aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
            <p>{t('securityScanQuotaWaiting')}</p>
          </div>
        </div>
      ) : null}
      {displayedError && status !== 'failed' ? (
        <div className="mx-auto mt-3 w-full max-w-conversation px-4 md:px-6">
          <div className="flex items-start gap-2 rounded-control border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
            <p role="alert">{displayedError}</p>
          </div>
        </div>
      ) : null}

      {status === 'completed' && hasAssistantReport ? (
        <div className="mx-auto mt-3 flex w-full max-w-conversation items-start gap-2 px-4 md:px-6">
          <CircleCheck aria-hidden="true" className="mt-0.5 shrink-0 text-success" size={16} />
          <div className="min-w-0">
            <h3 className="text-sm font-medium text-text">{t('securityScanResults')}</h3>
            {totalSteps > 0 ? (
              <p className="mt-0.5 text-xs text-secondary-text">
                {t('securityScanPlanComplete')} · <span className="tabular-nums">{completedSteps}/{totalSteps}</span>
              </p>
            ) : null}
          </div>
        </div>
      ) : null}

      {visibleMessages.length > 0 || isActive ? (
        <MessageList
          assistantModelFallback={scan.modelId}
          assistantProviderFallback={scan.providerId}
          conversationIdOverride={scan.conversationId}
          isLoadingMessagesOverride={false}
          messagesOverride={visibleMessages}
          showAssistantLoading={isActive && visibleMessages.at(-1)?.role !== 'assistant'}
        />
      ) : status === 'failed' && !hasFailedAssistantMessage ? (
        <div className="openchat-scroll-area flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-4 md:p-6">
          <section
            className="w-full max-w-assistant-message rounded-control border border-destructive/40 bg-destructive/5 p-4"
            role="alert"
          >
            <div className="flex items-start gap-3">
              <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0 text-destructive" size={18} />
              <div className="min-w-0">
                <p className="text-sm text-text">{displayedError ?? t('securityScanRequestFailed')}</p>
              </div>
            </div>
            <Button className="mt-4" onClick={onStartNewScan} size="small" variant="secondary">
              <Plus aria-hidden="true" size={15} />
              {t('securityScanNew')}
            </Button>
          </section>
        </div>
      ) : status === 'completed' ? (
        <div className="flex min-h-0 flex-1 items-center justify-center p-6">
          <div className="max-w-sm text-center">
            <CircleCheck aria-hidden="true" className="mx-auto mb-3 text-success" size={24} />
            <p className="text-sm text-secondary-text">{t('securityScanNoReport')}</p>
          </div>
        </div>
      ) : status !== 'failed' ? (
        <div className="flex min-h-0 flex-1 items-center justify-center p-6">
          <div className="max-w-sm text-center">
            {status === 'running' ? (
              <Activity aria-hidden="true" className="mx-auto mb-3 text-accent" size={24} />
            ) : (
              <ShieldCheck aria-hidden="true" className="mx-auto mb-3 text-secondary-icon" size={24} />
            )}
            <p className="text-sm text-secondary-text">
              {status === 'running' ? t('securityScanInspecting') : t(statusLabelKey(status))}
            </p>
          </div>
        </div>
      ) : null}
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

function StatusDot({ status }: { status: SecurityScanStatus }) {
  return (
    <span
      aria-hidden="true"
      className={`size-1.5 shrink-0 rounded-full ${
        status === 'running'
          ? 'bg-accent'
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

function projectRoots(project: ChatProject | null): string[] {
  if (!project) return [];
  return project.workspaceRoots.length > 0 ? project.workspaceRoots : [project.folderPath];
}

function formatElapsed(scan: SecurityScan, now: number): string {
  const end = scan.finishedAtUnixMs ?? now;
  const seconds = Math.max(0, Math.floor((end - scan.createdAtUnixMs) / MS_PER_SECOND));
  const hours = Math.floor(seconds / SECONDS_PER_HOUR);
  const minutes = Math.floor((seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`
    : `${minutes}:${String(remainder).padStart(2, '0')}`;
}

function formatScanCreatedAt(scan: SecurityScan, locale: string | undefined): string {
  return new Intl.DateTimeFormat(locale, {
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    month: 'short',
  }).format(scan.createdAtUnixMs);
}

function formatScanCreatedTime(scan: SecurityScan, locale: string | undefined): string {
  return new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(scan.createdAtUnixMs);
}

function resolveScanModelLabel(scan: SecurityScan, models: readonly ChatModel[], fallback: string): string {
  if (!scan.modelId) return fallback;
  const providerMatch = models.find((model) => model.providerId === scan.providerId && model.modelId === scan.modelId);
  if (providerMatch) return providerMatch.displayName;
  const modelMatch = models.find((model) => model.modelId === scan.modelId);
  return modelMatch?.displayName ?? scan.modelId;
}
