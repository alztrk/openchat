import { FolderOpen, Link, LoaderCircle, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Dialog } from '@/design-system';
import type {
  BusyAction,
  DiscoveryResult,
  EngineCatalog,
  EngineErrorKey,
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
  modelActionErrorKey,
  parseCatalog,
  parseDiscovery,
  parseExternalCandidate,
  parseInstallProgress,
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
import { EngineCard } from './LocalEngineCard';
import { InstallProgressCard } from './LocalEngineInstallProgress';
import { StorageChoiceDialog } from './StorageChoiceDialog';

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
