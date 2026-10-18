import { ChevronDown, FolderOpen, Link, LoaderCircle, Search, Square, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Dialog } from '@/design-system';
import { LocalEngineIcon } from '@/modules/providers/components/EngineIcon';
import type {
  BusyAction,
  DiscoveryResult,
  EngineCatalog,
  EngineErrorKey,
  EngineVariant,
  ExternalCandidate,
  ExternalServer,
  InstallProgress,
  LoadState,
  LocalEngine,
  LocalModel,
  Notice,
  PendingRegistration,
} from '@/modules/settings/data/localEngineCatalog';
import {
  acceleratorLabel,
  catalogStatusKey,
  channelStatusKey,
  formatBytes,
  installPhaseKey,
  modelActionErrorKey,
  parseCatalog,
  parseDiscovery,
  parseExternalCandidate,
  parseInstallProgress,
  runtimeStatusKey,
  variantStatusKey,
} from '@/modules/settings/data/localEngineCatalog';
import { useModelDirectoryStore } from '@/modules/settings/stores/modelDirectoryStore';
import {
  type JsonObject,
  LocalServiceError,
  localServiceClient,
  type ServiceEvent,
  type ServiceOperation,
} from '@/platform/local-service';
import { open } from '@/platform/tauri';
import { LlamaRuntimeSetting } from './LlamaRuntimeSetting';

export function LocalEnginesSection() {
  const { i18n, t } = useTranslation();
  const modelDirectories = useModelDirectoryStore((state) => state.directories);
  const directoryAvailability = useModelDirectoryStore((state) => state.directoryAvailability);
  const hydrateDirectoryFor = useModelDirectoryStore((state) => state.hydrateDirectoryFor);
  const setDirectoryFor = useModelDirectoryStore((state) => state.setDirectoryFor);
  const resetDirectoryFor = useModelDirectoryStore((state) => state.resetDirectoryFor);
  const [catalog, setCatalog] = useState<EngineCatalog | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [errorKey, setErrorKey] = useState<EngineErrorKey | null>(null);
  const [activeOperation, setActiveOperation] = useState<ServiceOperation | null>(null);
  const [activeAction, setActiveAction] = useState<BusyAction | null>(null);
  const [startingModelId, setStartingModelId] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<InstallProgress | null>(null);
  const [assetProgress, setAssetProgress] = useState<Record<string, InstallProgress>>({});
  const [isCancelling, setIsCancelling] = useState(false);
  const [pendingRegistration, setPendingRegistration] = useState<PendingRegistration | null>(null);
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);
  const [removeTarget, setRemoveTarget] = useState<LocalModel | null>(null);
  const [externalCandidates, setExternalCandidates] = useState<ExternalCandidate[]>([]);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [expandedEngineIds, setExpandedEngineIds] = useState<Set<string>>(() => new Set());
  const autoScanStarted = useRef(false);

  const loadCatalog = useCallback(async () => {
    setLoadState('loading');
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('local.engines.list');
      const nextCatalog = parseCatalog(response);
      for (const engine of nextCatalog.engines) hydrateDirectoryFor(engine.engineId);
      setCatalog(nextCatalog);
      setLoadState('ready');
    } catch {
      setCatalog(null);
      setLoadState('error');
      setErrorKey('localEnginesLoadFailed');
    }
  }, [hydrateDirectoryFor]);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  const runOperation = async (
    action: BusyAction,
    method: string,
    params: JsonObject,
    onEvent?: (event: ServiceEvent) => void,
  ) => {
    setActiveAction(action);
    setErrorKey(null);
    setNotice(null);
    try {
      const operation = await localServiceClient.startOperation(method, params, onEvent);
      setActiveOperation(operation);
      const result = await operation.result;
      if (action === 'install' && result.installed !== true) {
        throw new Error('The local engine service did not confirm installation.');
      }
      await loadCatalog();
    } catch (error) {
      if (!(error instanceof LocalServiceError) || error.code !== 'operation_cancelled') {
        setErrorKey(action === 'install' ? 'localEngineInstallFailed' : 'localModelActionError');
      }
      await loadCatalog();
    } finally {
      setActiveOperation(null);
      setActiveAction(null);
      setInstallProgress(null);
      setAssetProgress({});
      setIsCancelling(false);
    }
  };

  const stopModel = async (modelId: string) => {
    setActiveAction('remove');
    setErrorKey(null);
    try {
      await localServiceClient.call('local.engines.stop_model', { modelId });
      await loadCatalog();
    } catch {
      setErrorKey('localModelActionError');
    } finally {
      setActiveAction(null);
    }
  };

  const startModel = async (modelId: string) => {
    if (activeAction !== null) return;
    setStartingModelId(modelId);
    try {
      await runOperation('start', 'local.engines.start', { modelId });
    } finally {
      setStartingModelId(null);
    }
  };

  const chooseExecutable = async () => {
    if (activeAction !== null) return;
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        filters: [{ name: 'llama-server', extensions: ['exe'] }],
      });
      if (typeof selected !== 'string') return;
      setActiveAction('executable');
      setErrorKey(null);
      await localServiceClient.call('local.engines.llama_server_path.set', { path: selected });
      await loadCatalog();
    } catch {
      setErrorKey('localEngineSettingsFailed');
    } finally {
      setActiveAction(null);
    }
  };

  const clearExecutable = async () => {
    if (activeAction !== null) return;
    setActiveAction('executable');
    setErrorKey(null);
    try {
      await localServiceClient.call('local.engines.llama_server_path.set', { path: null });
      await loadCatalog();
    } catch {
      setErrorKey('localEngineSettingsFailed');
    } finally {
      setActiveAction(null);
    }
  };

  const detectExternalServers = useCallback(
    async (showEmptyNotice = true) => {
      if (activeAction !== null) return;
      setActiveAction('external');
      setErrorKey(null);
      try {
        const response = await localServiceClient.call('local.engines.external_llama_server.detect');
        if (!Array.isArray(response.servers)) throw new Error('External server response is invalid.');
        const candidates = response.servers.flatMap(parseExternalCandidate);
        const connected = new Set(
          (catalog?.externalServers ?? []).map((server) => `${server.processId}:${server.port}`),
        );
        setExternalCandidates(
          candidates.filter((candidate) => !connected.has(`${candidate.processId}:${candidate.port}`)),
        );
        if (candidates.length === 0 && showEmptyNotice) setNotice({ key: 'localEngineExternalServerNotFound' });
        await loadCatalog();
      } catch (error) {
        if (error instanceof LocalServiceError && error.code === 'local_engine_process_scan_failed') {
          setErrorKey('localEngineExternalServerScanFailed');
        } else {
          setErrorKey('localEngineExternalServerScanFailed');
        }
      } finally {
        setActiveAction(null);
      }
    },
    [activeAction, catalog?.externalServers, loadCatalog],
  );

  useEffect(() => {
    if (catalog?.hostOs !== 'windows' || autoScanStarted.current) return;
    autoScanStarted.current = true;
    void detectExternalServers(false);
  }, [catalog, detectExternalServers]);

  const connectExternalServer = async () => {
    const candidate = externalCandidates[0];
    if (!candidate || activeAction !== null) return;
    setActiveAction('external');
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('local.engines.external_llama_server.connect', {
        processId: candidate.processId,
        port: candidate.port,
      });
      const modelIds = Array.isArray(response.modelIds)
        ? response.modelIds.filter((modelId): modelId is string => typeof modelId === 'string')
        : [];
      setExternalCandidates((current) => current.slice(1));
      setNotice({
        key: 'localEngineExternalServerConnected',
        values: { port: candidate.port, count: modelIds.length },
      });
      await loadCatalog();
    } catch (error) {
      if (error instanceof LocalServiceError && error.code === 'local_engine_external_server_auth_required') {
        setErrorKey('localEngineExternalServerConnectFailed');
      } else {
        setErrorKey('localEngineExternalServerConnectFailed');
      }
    } finally {
      setActiveAction(null);
    }
  };

  const disconnectExternalServer = async (server: ExternalServer) => {
    if (activeAction !== null) return;
    setActiveAction('external');
    setErrorKey(null);
    try {
      await localServiceClient.call('local.engines.external_llama_server.disconnect', {
        processId: server.processId,
        port: server.port,
      });
      await loadCatalog();
    } catch {
      setErrorKey('localEngineExternalServerConnectFailed');
    } finally {
      setActiveAction(null);
    }
  };

  const chooseModelPath = async (engineId: string, pathKind: PendingRegistration['pathKind']) => {
    if (activeAction !== null) return;
    try {
      const selected = await open({
        directory: pathKind === 'directory',
        multiple: false,
        filters: pathKind === 'file' ? [{ name: 'GGUF model', extensions: ['gguf'] }] : undefined,
      });
      if (typeof selected === 'string') setPendingRegistration({ engineId, path: selected, pathKind });
    } catch {
      setErrorKey('localModelActionError');
    }
  };

  const registerModel = async (storageAction: 'move' | 'copy' | 'keep') => {
    const pending = pendingRegistration;
    const engine = catalog?.engines.find((item) => item.engineId === pending?.engineId);
    if (!pending || !engine || activeAction !== null) return;
    setActiveAction('register');
    setErrorKey(null);
    setNotice(null);
    try {
      const response = await localServiceClient.call('local.models.register', {
        engineId: engine.engineId,
        modelPath: pending.path,
        modelDirectory: modelDirectories[engine.engineId] ?? engine.modelDirectory,
        storageAction,
      });
      if (typeof response.id !== 'string') throw new Error('The local model response is invalid.');
      setPendingRegistration(null);
      setNotice({ key: 'localModelSaved' });
      await loadCatalog();
    } catch (error) {
      setErrorKey(modelActionErrorKey(error));
    } finally {
      setActiveAction(null);
    }
  };

  const scanModels = async (engineId: string, directoryOverride?: string) => {
    const engine = catalog?.engines.find((item) => item.engineId === engineId);
    if (!engine || activeAction !== null) return;
    setActiveAction('scan');
    setErrorKey(null);
    setNotice(null);
    try {
      const response = await localServiceClient.call('local.models.discover', {
        engineId: engine.engineId,
        modelDirectory: directoryOverride ?? modelDirectories[engine.engineId] ?? engine.modelDirectory,
      });
      const result = parseDiscovery(response);
      setDiscovery(result.models.length > 0 ? result : null);
      if (result.models.length === 0) {
        setNotice({ key: result.truncated ? 'localModelDiscoveryTruncated' : 'localModelDiscoveryEmpty' });
      }
    } catch (error) {
      setErrorKey(modelActionErrorKey(error));
    } finally {
      setActiveAction(null);
    }
  };

  const chooseModelDirectory = async (engine: LocalEngine) => {
    if (activeAction !== null) return;
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== 'string') return;
      if (!setDirectoryFor(engine.engineId, selected)) {
        setErrorKey('localModelActionError');
        return;
      }
      setErrorKey(null);
      await scanModels(engine.engineId, selected);
    } catch {
      setErrorKey('localModelActionError');
    }
  };

  const restoreEngineDefaultDirectory = async (engine: LocalEngine) => {
    if (activeAction !== null) return;
    if (!resetDirectoryFor(engine.engineId)) {
      setErrorKey('localModelActionError');
      return;
    }
    setErrorKey(null);
    await scanModels(engine.engineId, engine.modelDirectory);
  };

  const registerDiscovered = async () => {
    const found = discovery;
    if (!found || !catalog || activeAction !== null) return;
    setActiveAction('register');
    setErrorKey(null);
    let registered = 0;
    let failed = 0;
    for (const model of found.models) {
      try {
        const engine = catalog.engines.find((item) => item.engineId === model.engineId);
        if (!engine) {
          failed++;
          continue;
        }
        const response = await localServiceClient.call('local.models.register', {
          engineId: model.engineId,
          modelPath: model.path,
          modelDirectory: modelDirectories[engine.engineId] ?? engine.modelDirectory,
          storageAction: 'keep',
        });
        if (typeof response.id === 'string') registered++;
        else failed++;
      } catch {
        failed++;
      }
    }
    setDiscovery(null);
    setNotice(
      registered === found.models.length
        ? { key: 'localModelDiscoveryRegistered', values: { count: registered } }
        : { key: 'localModelDiscoveryPartial', values: { registered, total: found.models.length } },
    );
    if (failed > 0) setErrorKey('localModelActionError');
    setActiveAction(null);
    await loadCatalog();
  };

  const removeModel = async () => {
    const target = removeTarget;
    if (!target || activeAction !== null) return;
    setActiveAction('remove');
    setErrorKey(null);
    try {
      const response = await localServiceClient.call('local.models.remove', { modelId: target.id });
      if (response.removed !== true) throw new Error('The local model registration was not removed.');
      setRemoveTarget(null);
      setNotice({ key: 'localModelRemoved' });
      await loadCatalog();
    } catch (error) {
      setErrorKey(modelActionErrorKey(error));
    } finally {
      setActiveAction(null);
    }
  };

  const cancelInstall = async () => {
    if (!activeOperation || isCancelling) return;
    setIsCancelling(true);
    await activeOperation.cancel();
  };

  const cancelModelStart = async () => {
    if (!activeOperation || activeAction !== 'start' || startingModelId === null || isCancelling) return;
    setIsCancelling(true);
    try {
      await activeOperation.cancel();
    } catch {
      setErrorKey('localModelActionError');
      setIsCancelling(false);
    }
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-6">
      {loadState === 'loading' ? (
        <div aria-live="polite" className="flex justify-center py-8" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-text" size={22} />
        </div>
      ) : null}
      {loadState === 'error' ? (
        <div className="rounded-card border border-destructive/40 bg-surface p-4" role="alert">
          <p className="text-sm text-destructive">{t(errorKey ?? 'localEnginesLoadFailed')}</p>
          <Button className="mt-3" onClick={() => void loadCatalog()} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : null}
      {errorKey && loadState !== 'error' ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
      {notice ? (
        <p className="text-sm text-secondary-text" role="status">
          {t(notice.key, notice.values)}
        </p>
      ) : null}

      {loadState === 'ready' && catalog ? (
        <>
          {catalog.engines.length === 0 ? (
            <Card className="flex items-center justify-between gap-3 p-4">
              <p className="text-sm text-secondary-text">{t('localEnginesEmpty')}</p>
              <Button onClick={() => void loadCatalog()} size="small" variant="secondary">
                {t('localEnginesReload')}
              </Button>
            </Card>
          ) : null}
          {catalog.engines.map((engine) => (
            <EngineCard
              activeAction={activeAction}
              assetProgress={assetProgress}
              directory={modelDirectories[engine.engineId] ?? engine.modelDirectory}
              directoryAvailable={directoryAvailability[engine.engineId] ?? true}
              engine={engine}
              executableAvailable={catalog.executableAvailable}
              executablePath={catalog.executablePath}
              externalServers={catalog.externalServers}
              expanded={expandedEngineIds.has(engine.engineId)}
              hostOs={catalog.hostOs}
              installProgress={installProgress}
              key={engine.engineId}
              locale={i18n.language}
              models={catalog.models.filter((model) => model.engineId === engine.engineId)}
              onAddModel={() =>
                void chooseModelPath(engine.engineId, engine.engineId === 'llama_cpp' ? 'file' : 'directory')
              }
              onChooseDirectory={() => void chooseModelDirectory(engine)}
              onChooseExecutable={() => void chooseExecutable()}
              onCheckExternalServers={() => void detectExternalServers()}
              onClearExecutable={() => void clearExecutable()}
              onCancelStart={() => void cancelModelStart()}
              onDiscover={() => void scanModels(engine.engineId)}
              onDisconnectExternalServer={(server) => void disconnectExternalServer(server)}
              onInstall={(variant) =>
                void runOperation(
                  'install',
                  'local.engines.install',
                  { engineId: engine.engineId, variantId: variant.variantId },
                  (event) => {
                    const progress = parseInstallProgress(event);
                    if (!progress) return;
                    setInstallProgress(progress);
                    setAssetProgress((current) => ({ ...current, [progress.assetName]: progress }));
                  },
                )
              }
              onRemove={(model) => setRemoveTarget(model)}
              onStart={(modelId) => void startModel(modelId)}
              onStop={(modelId) => void stopModel(modelId)}
              onToggle={() =>
                setExpandedEngineIds((current) => {
                  const next = new Set(current);
                  if (next.has(engine.engineId)) next.delete(engine.engineId);
                  else next.add(engine.engineId);
                  return next;
                })
              }
              onUseDefaultDirectory={() => void restoreEngineDefaultDirectory(engine)}
              runningModelIds={catalog.runningModelIds}
              startingModelId={startingModelId}
              startingModelIsCancelling={isCancelling}
            />
          ))}

          {activeAction === 'install' && activeOperation ? (
            <InstallProgressCard
              assetProgress={assetProgress}
              cancelling={isCancelling}
              locale={i18n.language}
              onCancel={() => void cancelInstall()}
              progress={installProgress}
            />
          ) : null}
        </>
      ) : null}

      <Dialog
        onOpenChange={(openState) => {
          if (!openState && activeAction !== 'register') setPendingRegistration(null);
        }}
        open={pendingRegistration !== null}
        title={t('localModelStorageChoiceTitle')}
      >
        {pendingRegistration ? (
          <StorageChoiceDialog
            active={activeAction === 'register'}
            onCancel={() => setPendingRegistration(null)}
            onChoose={(action) => void registerModel(action)}
            path={pendingRegistration.path}
            targetDirectory={
              modelDirectories[pendingRegistration.engineId] ??
              catalog?.engines.find((engine) => engine.engineId === pendingRegistration.engineId)?.modelDirectory ??
              ''
            }
          />
        ) : null}
      </Dialog>

      <Dialog
        onOpenChange={(openState) => {
          if (!openState && activeAction !== 'register') setDiscovery(null);
        }}
        open={discovery !== null}
        title={t('localModelDiscoveryTitle')}
      >
        {discovery ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('localModelDiscoveryPrompt', { count: discovery.models.length })}
            </p>
            {discovery.truncated ? <p className="text-sm text-warning">{t('localModelDiscoveryTruncated')}</p> : null}
            <ul className="grid max-h-72 gap-2 overflow-auto rounded-control border border-border p-3">
              {discovery.models.map((model) => (
                <li className="border-b border-border py-2 last:border-b-0" key={`${model.engineId}:${model.path}`}>
                  <p className="break-words text-sm font-medium text-text">{model.displayName}</p>
                  <p className="mt-1 break-all font-mono text-xs text-secondary-text">{model.path}</p>
                </li>
              ))}
            </ul>
            <div className="flex flex-wrap justify-end gap-2">
              <Button disabled={activeAction === 'register'} onClick={() => setDiscovery(null)}>
                {t('cancel')}
              </Button>
              <Button disabled={activeAction !== null} onClick={() => void registerDiscovered()} variant="primary">
                {activeAction === 'register' ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <FolderOpen aria-hidden="true" size={16} />
                )}
                {t('localModelDiscoveryRegisterAll')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>

      <Dialog
        onOpenChange={(openState) => {
          if (!openState && activeAction !== 'remove') setRemoveTarget(null);
        }}
        open={removeTarget !== null}
        title={t('localModelRemove')}
      >
        {removeTarget ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">{t('localModelRemoveConfirmation')}</p>
            <p className="break-all font-mono text-xs text-text">{removeTarget.path}</p>
            <div className="flex justify-end gap-2">
              <Button disabled={activeAction === 'remove'} onClick={() => setRemoveTarget(null)}>
                {t('cancel')}
              </Button>
              <Button disabled={activeAction !== null} onClick={() => void removeModel()} variant="destructive">
                {activeAction === 'remove' ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <Trash2 aria-hidden="true" size={16} />
                )}
                {t('localModelRemove')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>

      <Dialog
        onOpenChange={(openState) => {
          if (!openState && activeAction !== 'external') setExternalCandidates((current) => current.slice(1));
        }}
        open={externalCandidates.length > 0}
        title={t('localEngineExternalServerFoundTitle')}
      >
        {externalCandidates[0] ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('localEngineExternalServerFoundDescription', { port: externalCandidates[0].port })}
            </p>
            <div className="flex justify-end gap-2">
              <Button
                disabled={activeAction === 'external'}
                onClick={() => setExternalCandidates((current) => current.slice(1))}
              >
                {t('localEngineExternalServerNotNow')}
              </Button>
              <Button disabled={activeAction !== null} onClick={() => void connectExternalServer()} variant="primary">
                {activeAction === 'external' ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <Link aria-hidden="true" size={16} />
                )}
                {t('localEngineExternalServerConnect')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </section>
  );
}

function EngineCard({
  activeAction,
  assetProgress,
  directory,
  directoryAvailable,
  engine,
  executableAvailable,
  executablePath,
  externalServers,
  expanded,
  hostOs,
  installProgress,
  locale,
  models,
  onAddModel,
  onCancelStart,
  onChooseDirectory,
  onChooseExecutable,
  onCheckExternalServers,
  onClearExecutable,
  onDiscover,
  onDisconnectExternalServer,
  onInstall,
  onRemove,
  onStart,
  onStop,
  onToggle,
  onUseDefaultDirectory,
  runningModelIds,
  startingModelId,
  startingModelIsCancelling,
}: {
  activeAction: BusyAction | null;
  assetProgress: Record<string, InstallProgress>;
  directory: string;
  directoryAvailable: boolean;
  engine: LocalEngine;
  executableAvailable: boolean;
  executablePath: string | null;
  externalServers: ExternalServer[];
  expanded: boolean;
  hostOs: string;
  installProgress: InstallProgress | null;
  locale: string;
  models: LocalModel[];
  onAddModel(): void;
  onCancelStart(): void;
  onChooseDirectory(): void;
  onChooseExecutable(): void;
  onCheckExternalServers(): void;
  onClearExecutable(): void;
  onDiscover(): void;
  onDisconnectExternalServer(server: ExternalServer): void;
  onInstall(variant: EngineVariant): void;
  onRemove(model: LocalModel): void;
  onStart(modelId: string): void;
  onStop(modelId: string): void;
  onToggle(): void;
  onUseDefaultDirectory(): void;
  runningModelIds: string[];
  startingModelId: string | null;
  startingModelIsCancelling: boolean;
}) {
  const { t } = useTranslation();
  const installing =
    activeAction === 'install' && installProgress?.engineId === engine.engineId && installProgress.variantId !== '';
  const installed =
    engine.variants.some((variant) => variant.installed) || (engine.engineId === 'llama_cpp' && executableAvailable);
  const runtimeLabel =
    engine.catalogStatus === 'blocked'
      ? t('localEngineUnavailable')
      : !installed
        ? t('localEngineNotInstalled')
        : t(runtimeStatusKey(engine.runtimeStatus));
  const channelKey = channelStatusKey(engine.channel);
  return (
    <Card className="overflow-hidden p-0">
      <button
        aria-controls={`local-engine-panel-${engine.engineId}`}
        aria-expanded={expanded}
        className="flex w-full flex-wrap items-start justify-between gap-3 px-5 py-4 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus"
        onClick={onToggle}
        type="button"
      >
        <div className="flex min-w-0 items-start gap-3">
          <LocalEngineIcon className="mt-1" engineId={engine.engineId} size={24} tone="accent" />
          <div className="min-w-0">
            <h3 className="font-semibold text-text">{engine.displayName}</h3>
            <p className="mt-1 font-mono text-xs text-secondary-text">
              {t('localEngineRelease', { tag: engine.releaseTag })}
            </p>
            <div className="mt-2 flex flex-wrap gap-2 text-xs text-secondary-text">
              <span className="rounded-control border border-border px-2 py-1">
                {channelKey ? t(channelKey) : engine.channel}
              </span>
              {engine.catalogStatus === 'deprecated' ? (
                <span className="rounded-control border border-border px-2 py-1">
                  {t(catalogStatusKey(engine.catalogStatus))}
                </span>
              ) : null}
            </div>
          </div>
        </div>
        <div className="grid justify-items-end gap-1">
          <span className="text-xs font-semibold text-secondary-text">{t('localEngineHealth')}</span>
          <span className="rounded-control border border-border px-2 py-1 text-xs text-secondary-text">
            {runtimeLabel}
          </span>
        </div>
        <ChevronDown
          aria-hidden="true"
          className="openchat-disclosure-chevron mt-1 shrink-0 text-secondary-icon"
          data-collapsed={!expanded}
          size={18}
        />
      </button>
      {expanded ? (
        <div
          className="openchat-disclosure-enter grid gap-4 border-t border-border p-5"
          id={`local-engine-panel-${engine.engineId}`}
        >
          {engine.engineId === 'llama_cpp' ? (
            <LlamaRuntimeSetting
              executableActive={activeAction === 'executable'}
              executableAvailable={executableAvailable}
              executablePath={executablePath}
              externalActive={activeAction === 'external'}
              externalServers={externalServers}
              onCheckExternalServers={onCheckExternalServers}
              onChooseExecutable={onChooseExecutable}
              onClearExecutable={onClearExecutable}
              onDisconnectExternalServer={onDisconnectExternalServer}
              showExternalCheck={hostOs === 'windows'}
            />
          ) : null}
          {(engine.catalogStatus === 'blocked' || engine.catalogStatus === 'deprecated') && engine.statusReason ? (
            <p className="text-sm text-secondary-text">
              {engine.statusReason === 'release_check_unavailable'
                ? t('localEngineReleaseCheckUnavailable')
                : t('localEngineBlocked')}
            </p>
          ) : null}
          {engine.releaseCheckStatus !== 'current' ? (
            <p className="text-sm text-secondary-text">{t('localEngineReleaseCheckUnavailable')}</p>
          ) : null}
          {engine.releaseCheckStatus === 'current' &&
          engine.latestUpstreamReleaseTag &&
          engine.latestUpstreamReleaseTag !== engine.releaseTag ? (
            <p className="text-sm text-warning">
              {t('localEngineNewerReleaseIncompatible', {
                latest: engine.latestUpstreamReleaseTag,
                compatible: engine.releaseTag,
              })}
            </p>
          ) : null}
          {engine.variants.length > 0 ? (
            <>
              <h4 className="text-sm font-semibold text-text">{t('localEngineVariants')}</h4>
              <ul className="grid gap-2">
                {engine.variants.map((variant) => {
                  const busy = activeAction === 'install';
                  return (
                    <li
                      className="grid gap-3 rounded-control border border-border bg-composer px-3 py-3"
                      key={variant.variantId}
                    >
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap gap-2">
                            <span className="font-mono text-xs font-semibold text-text">
                              {acceleratorLabel(variant.accelerator)}
                            </span>
                            <span className="rounded-control border border-border px-2 py-0.5 text-xs text-secondary-text">
                              {variant.os}
                            </span>
                            <span className="rounded-control border border-border px-2 py-0.5 text-xs text-secondary-text">
                              {variant.architecture}
                            </span>
                            {variant.recommended ? (
                              <span className="rounded-control border border-accent/40 bg-selected px-2 py-0.5 text-xs text-accent">
                                {t('localEngineRecommended')}
                              </span>
                            ) : null}
                          </div>
                          <p className="mt-2 text-xs text-secondary-text">{t(variantStatusKey(variant.status))}</p>
                          {variant.runtimeRequirements.length > 0 ? (
                            <p className="mt-1 text-xs text-secondary-text">
                              {t('localEngineRuntimeRequirements', {
                                requirements: variant.runtimeRequirements.join(' · '),
                              })}
                            </p>
                          ) : null}
                        </div>
                        {variant.canInstall ? (
                          <Button
                            disabled={busy || activeAction !== null}
                            onClick={() => onInstall(variant)}
                            size="small"
                            variant="secondary"
                          >
                            {busy ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}
                            {t(variant.installed ? 'localEngineUpdate' : 'localEngineInstall')}
                          </Button>
                        ) : null}
                      </div>
                      {busy &&
                      installProgress?.engineId === engine.engineId &&
                      installProgress.variantId === variant.variantId ? (
                        <InstallProgressInline
                          assetProgress={assetProgress}
                          locale={locale}
                          progress={installProgress}
                        />
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            </>
          ) : null}
          {installing ? <p className="text-xs text-secondary-text">{t('localEngineInstalling')}</p> : null}
          <EngineModelDirectorySetting
            available={directoryAvailable}
            directory={directory}
            isBusy={activeAction !== null}
            isCustom={directory !== engine.modelDirectory}
            onChoose={onChooseDirectory}
            onScan={onDiscover}
            onUseDefault={onUseDefaultDirectory}
            scanning={activeAction === 'scan'}
          />
          <LocalModelsSection
            addLabel={engine.engineId === 'llama_cpp' ? 'localModelAddFile' : 'localModelAddFolder'}
            activeAction={activeAction}
            engineId={engine.engineId}
            models={models}
            onAddModel={onAddModel}
            onCancelStart={onCancelStart}
            onRemove={onRemove}
            onStart={onStart}
            onStop={onStop}
            runningModelIds={runningModelIds}
            startingModelId={startingModelId}
            startingModelIsCancelling={startingModelIsCancelling}
          />
        </div>
      ) : null}
    </Card>
  );
}

function EngineModelDirectorySetting({
  available,
  directory,
  isBusy,
  isCustom,
  onChoose,
  onScan,
  onUseDefault,
  scanning,
}: {
  available: boolean;
  directory: string;
  isBusy: boolean;
  isCustom: boolean;
  onChoose(): void;
  onScan(): void;
  onUseDefault(): void;
  scanning: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-3 rounded-control border border-border bg-surface p-4">
      <div>
        <h4 className="font-semibold text-text">{t('localModelDirectoryTitle')}</h4>
        <p className="mt-1 text-sm text-secondary-text">{t('localModelDirectoryDescription')}</p>
      </div>
      <p className={`break-all font-mono text-xs ${available ? 'text-secondary-text' : 'text-warning'}`}>{directory}</p>
      {!available ? (
        <p className="text-sm text-warning" role="status">
          {t('localModelDirectoryUnavailable')}
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button disabled={isBusy} onClick={onChoose} size="small" variant="secondary">
          <FolderOpen aria-hidden="true" size={16} />
          {t('localModelChooseDirectory')}
        </Button>
        <Button disabled={isBusy} onClick={onScan} size="small" variant="ghost">
          {scanning ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <Search aria-hidden="true" size={16} />
          )}
          {t(scanning ? 'localModelScanningDirectory' : 'localModelScanDirectory')}
        </Button>
        {isCustom ? (
          <Button disabled={isBusy} onClick={onUseDefault} size="small" variant="ghost">
            {t('localModelUseDefaultDirectory')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function LocalModelsSection({
  addLabel,
  activeAction,
  engineId,
  models,
  onAddModel,
  onCancelStart,
  onRemove,
  onStart,
  onStop,
  runningModelIds,
  startingModelId,
  startingModelIsCancelling,
}: {
  addLabel: 'localModelAddFile' | 'localModelAddFolder';
  activeAction: BusyAction | null;
  engineId: string;
  models: LocalModel[];
  onAddModel(): void;
  onCancelStart(): void;
  onRemove(model: LocalModel): void;
  onStart(modelId: string): void;
  onStop(modelId: string): void;
  runningModelIds: string[];
  startingModelId: string | null;
  startingModelIsCancelling: boolean;
}) {
  const { t } = useTranslation();
  return (
    <section aria-labelledby={`local-models-heading-${engineId}`} className="grid gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h3 className="text-base font-semibold" id={`local-models-heading-${engineId}`}>
          {t('localModels')}
        </h3>
        <div className="flex flex-wrap gap-2">
          <Button disabled={activeAction !== null} onClick={onAddModel} size="small" variant="secondary">
            <FolderOpen aria-hidden="true" size={16} />
            {t(addLabel)}
          </Button>
        </div>
      </div>
      {models.length === 0 ? (
        <p className="rounded-card border border-border bg-surface p-5 text-sm text-secondary-text">
          {t('localModelsEmpty')}
        </p>
      ) : null}
      <ul className="grid gap-2">
        {models.map((model) => {
          const running = runningModelIds.includes(model.id);
          const busy = activeAction !== null;
          const starting = startingModelId === model.id;
          return (
            <li key={model.id}>
              <Card className="flex min-w-0 flex-wrap items-start justify-between gap-4 p-4">
                <div className="min-w-0 flex-1">
                  <p className="break-words font-medium text-text">{model.displayName}</p>
                  <p className="mt-1 break-all font-mono text-xs text-secondary-text">{model.path}</p>
                  {!model.pathExists ? <p className="mt-2 text-sm text-warning">{t('localModelPathMissing')}</p> : null}
                  {model.pathExists && !model.isAvailable ? (
                    <p className="mt-2 text-sm text-warning">{t('localModelEngineNotReady')}</p>
                  ) : null}
                </div>
                <div className="flex shrink-0 flex-wrap gap-2">
                  {model.pathExists && model.isAvailable ? (
                    starting ? (
                      <Button
                        aria-label={t('cancel')}
                        disabled={startingModelIsCancelling}
                        onClick={onCancelStart}
                        size="small"
                        variant="secondary"
                      >
                        {startingModelIsCancelling ? (
                          <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
                        ) : (
                          <X aria-hidden="true" size={14} />
                        )}
                        {t('cancel')}
                      </Button>
                    ) : (
                      <Button
                        disabled={busy}
                        onClick={() => (running ? onStop(model.id) : onStart(model.id))}
                        size="small"
                        variant="secondary"
                      >
                        {running ? <Square aria-hidden="true" size={14} /> : null}
                        {t(running ? 'localEngineStopModel' : 'localEngineStartModel')}
                      </Button>
                    )
                  ) : null}
                  <Button
                    aria-label={`${t('localModelRemove')}: ${model.displayName}`}
                    disabled={busy}
                    onClick={() => onRemove(model)}
                    size="small"
                    variant="ghost"
                  >
                    <Trash2 aria-hidden="true" size={16} />
                    {t('localModelRemove')}
                  </Button>
                </div>
              </Card>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function StorageChoiceDialog({
  active,
  onCancel,
  onChoose,
  path,
  targetDirectory,
}: {
  active: boolean;
  onCancel(): void;
  onChoose(action: 'move' | 'copy' | 'keep'): void;
  path: string;
  targetDirectory: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-4">
      <p className="break-all font-mono text-xs text-secondary-text">{path}</p>
      <p className="text-sm text-secondary-text">{t('localModelStorageChoiceTarget', { folder: targetDirectory })}</p>
      <div className="grid gap-2">
        <Button disabled={active} onClick={() => onChoose('move')} variant="secondary">
          {t('localModelMoveToFolder')}
        </Button>
        <Button disabled={active} onClick={() => onChoose('copy')} variant="secondary">
          {t('localModelCopyToFolder')}
        </Button>
        <Button disabled={active} onClick={() => onChoose('keep')} variant="ghost">
          {t('localModelKeepInPlace')}
        </Button>
      </div>
      <Button disabled={active} onClick={onCancel} variant="ghost">
        {t('cancel')}
      </Button>
    </div>
  );
}

function InstallProgressCard({
  assetProgress,
  cancelling,
  locale,
  onCancel,
  progress,
}: {
  assetProgress: Record<string, InstallProgress>;
  cancelling: boolean;
  locale: string;
  onCancel(): void;
  progress: InstallProgress | null;
}) {
  const { t } = useTranslation();
  return (
    <Card aria-live="polite" className="grid gap-3 p-4">
      <InstallProgressInline assetProgress={assetProgress} locale={locale} progress={progress} />
      <div className="flex justify-end">
        <Button disabled={cancelling} onClick={onCancel} size="small" variant="secondary">
          {cancelling ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <X aria-hidden="true" size={16} />
          )}
          {t(cancelling ? 'localEngineCancellingInstall' : 'localEngineCancelInstall')}
        </Button>
      </div>
    </Card>
  );
}

function InstallProgressInline({
  assetProgress,
  locale,
  progress,
}: {
  assetProgress: Record<string, InstallProgress>;
  locale: string;
  progress: InstallProgress | null;
}) {
  const { t } = useTranslation();
  const fraction =
    progress && progress.totalBytes > 0 ? Math.min(progress.downloadedBytes, progress.totalBytes) : undefined;
  const assets = Object.values(assetProgress);
  return (
    <div className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
        <span className="font-semibold text-text">
          {progress ? t(installPhaseKey(progress.phase)) : t('localEngineInstalling')}
        </span>
        {progress ? <span className="truncate text-secondary-text">{progress.assetName}</span> : null}
      </div>
      <progress
        aria-label={t('localEngineInstallProgress')}
        className="h-2 w-full accent-accent"
        max={progress?.totalBytes ?? 1}
        value={fraction ?? 0}
      />
      {progress ? (
        <p className="text-xs text-secondary-text">
          {formatBytes(progress.downloadedBytes, locale)} / {formatBytes(progress.totalBytes, locale)}
        </p>
      ) : null}
      {assets.length > 1 ? (
        <ul className="grid gap-1">
          {assets.map((asset) => (
            <li className="flex items-center gap-2 text-xs text-secondary-text" key={asset.assetName}>
              <span className="min-w-0 flex-1 truncate">{asset.assetName}</span>
              <progress
                className="h-1.5 w-28 accent-accent"
                max={asset.totalBytes || 1}
                value={asset.totalBytes > 0 ? Math.min(asset.downloadedBytes, asset.totalBytes) : 0}
              />
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
