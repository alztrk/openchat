import { CheckCircle2, CircleAlert, CircleDashed, Link, LoaderCircle, RefreshCw, Server, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  extractPortFromBaseUrl,
  LM_STUDIO_CUSTOM_PROVIDER_ID,
  type LocalServerHealthStatus,
  type LocalServerKind,
  type LocalServerModelEntry,
  OLLAMA_CUSTOM_PROVIDER_ID,
  parseLocalServerModelsResult,
  parseLocalServersDetection,
} from '@features/app-settings/data/localEngineCatalog';
import { useChatStore } from '@features/chat/state';
import {
  type CustomProviderSummary,
  deleteCustomProvider,
  loadProviderCredentialStatus,
  saveCustomProvider,
} from '@features/provider-settings/data';
import { Button, Input } from '@shared/components';
import { localServiceClient } from '@shared/lib/platform/local-service';
import { toast } from '@shared/lib/toast';

interface ServerCardState {
  portInput: string;
  portError: boolean;
  detectedPorts: number[];
  healthStatus: LocalServerHealthStatus;
  availableModels: LocalServerModelEntry[];
  selectedModelIds: string[];
  savedProvider: CustomProviderSummary | null;
  probing: boolean;
  saving: boolean;
  removing: boolean;
}

const SERVER_DEFINITIONS: ReadonlyArray<{
  kind: LocalServerKind;
  providerId: typeof OLLAMA_CUSTOM_PROVIDER_ID | typeof LM_STUDIO_CUSTOM_PROVIDER_ID;
  displayName: string;
  defaultPort: number;
}> = [
  {
    kind: 'ollama',
    providerId: OLLAMA_CUSTOM_PROVIDER_ID,
    displayName: 'Ollama',
    defaultPort: 11434,
  },
  {
    kind: 'lm_studio',
    providerId: LM_STUDIO_CUSTOM_PROVIDER_ID,
    displayName: 'LM Studio',
    defaultPort: 1234,
  },
];

function createInitialState(defaultPort: number): ServerCardState {
  return {
    portInput: String(defaultPort),
    portError: false,
    detectedPorts: [],
    healthStatus: 'unreachable',
    availableModels: [],
    selectedModelIds: [],
    savedProvider: null,
    probing: false,
    saving: false,
    removing: false,
  };
}

function parsePortNumber(raw: string): number | null {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > 65535) return null;
  return parsed;
}

export function LocalServerConnectionsSection() {
  const { t } = useTranslation();
  const refreshModels = useChatStore((state) => state.refreshModels);
  const isMountedRef = useRef(true);
  const [detectingAll, setDetectingAll] = useState(true);
  const [servers, setServers] = useState<Record<LocalServerKind, ServerCardState>>(() => ({
    ollama: createInitialState(11434),
    lm_studio: createInitialState(1234),
  }));

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const detectServersAndLoadStatus = useCallback(async (customPorts?: Partial<Record<LocalServerKind, number>>) => {
    setDetectingAll(true);
    try {
      const snapshot = await loadProviderCredentialStatus();
      const savedOllama =
        snapshot.customProviders.find((provider) => provider.id === OLLAMA_CUSTOM_PROVIDER_ID) ?? null;
      const savedLmStudio =
        snapshot.customProviders.find((provider) => provider.id === LM_STUDIO_CUSTOM_PROVIDER_ID) ?? null;

      const ollamaPort =
        customPorts?.ollama ?? (savedOllama ? extractPortFromBaseUrl(savedOllama.baseUrl, 11434) : undefined);
      const lmStudioPort =
        customPorts?.lm_studio ?? (savedLmStudio ? extractPortFromBaseUrl(savedLmStudio.baseUrl, 1234) : undefined);

      const response = await localServiceClient.call('local.engines.local_servers.detect', {
        ...(ollamaPort !== undefined ? { ollamaPort } : {}),
        ...(lmStudioPort !== undefined ? { lmStudioPort } : {}),
      });
      const detections = parseLocalServersDetection(response);
      if (!isMountedRef.current) return;

      setServers((prev) => {
        const next = { ...prev };
        for (const def of SERVER_DEFINITIONS) {
          const detection = detections.find((item) => item.serverKind === def.kind);
          const saved = def.kind === 'ollama' ? savedOllama : savedLmStudio;
          const resolvedPort =
            detection?.activePort ??
            (saved ? extractPortFromBaseUrl(saved.baseUrl, def.defaultPort) : def.defaultPort);
          const fetchedModels = detection?.models ?? [];
          const existingSelection =
            saved && saved.selectedModels.length > 0
              ? saved.selectedModels
              : prev[def.kind].selectedModelIds.length > 0
                ? prev[def.kind].selectedModelIds
                : fetchedModels.map((model) => model.id);

          next[def.kind] = {
            ...prev[def.kind],
            portInput: String(resolvedPort),
            portError: false,
            detectedPorts: detection?.detectedPorts ?? [],
            healthStatus: detection?.healthStatus ?? 'unreachable',
            availableModels: fetchedModels,
            selectedModelIds: existingSelection,
            savedProvider: saved,
          };
        }
        return next;
      });
    } catch {
      if (isMountedRef.current) {
        toast.error(t('localServerDetectFailed'));
      }
    } finally {
      if (isMountedRef.current) {
        setDetectingAll(false);
      }
    }
  }, [t]);

  useEffect(() => {
    void detectServersAndLoadStatus();
  }, [detectServersAndLoadStatus]);

  const handlePortChange = (kind: LocalServerKind, value: string) => {
    setServers((prev) => ({
      ...prev,
      [kind]: {
        ...prev[kind],
        portInput: value,
        portError: value.trim().length > 0 && parsePortNumber(value) === null,
      },
    }));
  };

  const probeServerModels = async (kind: LocalServerKind, overridePort?: number) => {
    const state = servers[kind];
    const port = overridePort ?? parsePortNumber(state.portInput);
    if (port === null) {
      setServers((prev) => ({
        ...prev,
        [kind]: { ...prev[kind], portError: true },
      }));
      return;
    }

    setServers((prev) => ({
      ...prev,
      [kind]: {
        ...prev[kind],
        portInput: String(port),
        portError: false,
        probing: true,
      },
    }));

    try {
      const response = await localServiceClient.call('local.engines.local_servers.models', {
        serverKind: kind,
        port,
      });
      const result = parseLocalServerModelsResult(response);
      if (!isMountedRef.current) return;

      setServers((prev) => {
        const current = prev[kind];
        const availableIds = new Set(result.models.map((item) => item.id));
        const preservedSelection = current.selectedModelIds.filter((id) => availableIds.has(id));
        const nextSelection =
          preservedSelection.length > 0 ? preservedSelection : result.models.map((item) => item.id);

        return {
          ...prev,
          [kind]: {
            ...current,
            probing: false,
            healthStatus: result.healthStatus,
            availableModels: result.models,
            selectedModelIds: nextSelection,
          },
        };
      });
    } catch {
      if (!isMountedRef.current) return;
      setServers((prev) => ({
        ...prev,
        [kind]: { ...prev[kind], probing: false, healthStatus: 'unreachable' },
      }));
      toast.error(t('localServerFetchModelsFailed'));
    }
  };

  const toggleModelSelection = (kind: LocalServerKind, modelId: string) => {
    setServers((prev) => {
      const current = prev[kind];
      const exists = current.selectedModelIds.includes(modelId);
      const selectedModelIds = exists
        ? current.selectedModelIds.filter((id) => id !== modelId)
        : [...current.selectedModelIds, modelId];
      return {
        ...prev,
        [kind]: {
          ...current,
          selectedModelIds,
        },
      };
    });
  };

  const setAllModelsSelection = (kind: LocalServerKind, selectAll: boolean) => {
    setServers((prev) => {
      const current = prev[kind];
      return {
        ...prev,
        [kind]: {
          ...current,
          selectedModelIds: selectAll ? current.availableModels.map((model) => model.id) : [],
        },
      };
    });
  };

  const handleSaveConnection = async (kind: LocalServerKind) => {
    const def = SERVER_DEFINITIONS.find((item) => item.kind === kind);
    if (!def) return;
    const state = servers[kind];
    const port = parsePortNumber(state.portInput);
    if (port === null) {
      setServers((prev) => ({
        ...prev,
        [kind]: { ...prev[kind], portError: true },
      }));
      return;
    }
    if (state.availableModels.length > 0 && state.selectedModelIds.length === 0) {
      toast.error(t('localServerSelectModelsRequired'));
      return;
    }

    setServers((prev) => ({
      ...prev,
      [kind]: { ...prev[kind], saving: true },
    }));

    try {
      const saved = await saveCustomProvider({
        id: def.providerId,
        name: def.displayName,
        baseUrl: `http://127.0.0.1:${port}/v1`,
        protocol: 'openAi',
        allowInsecureHttp: true,
        reasoningEfforts: state.savedProvider?.reasoningEfforts ?? [],
        enableThinking: state.savedProvider?.enableThinking ?? false,
        selectedModels: state.selectedModelIds,
        apiKey: '',
      });
      if (!isMountedRef.current) return;
      setServers((prev) => ({
        ...prev,
        [kind]: {
          ...prev[kind],
          saving: false,
          savedProvider: saved,
          selectedModelIds: saved.selectedModels,
        },
      }));
      toast.success(
        t('localServerConnectionSaved', {
          name: def.displayName,
          count: saved.selectedModels.length,
        }),
      );
      await refreshModels(true);
    } catch {
      if (!isMountedRef.current) return;
      setServers((prev) => ({
        ...prev,
        [kind]: { ...prev[kind], saving: false },
      }));
      toast.error(t('localServerConnectionSaveFailed', { name: def.displayName }));
    }
  };

  const handleRemoveConnection = async (kind: LocalServerKind) => {
    const def = SERVER_DEFINITIONS.find((item) => item.kind === kind);
    if (!def) return;
    setServers((prev) => ({
      ...prev,
      [kind]: { ...prev[kind], removing: true },
    }));

    try {
      await deleteCustomProvider(def.providerId);
      if (!isMountedRef.current) return;
      setServers((prev) => ({
        ...prev,
        [kind]: {
          ...prev[kind],
          removing: false,
          savedProvider: null,
        },
      }));
      toast.success(t('localServerConnectionRemoved', { name: def.displayName }));
      await refreshModels(true);
    } catch {
      if (!isMountedRef.current) return;
      setServers((prev) => ({
        ...prev,
        [kind]: { ...prev[kind], removing: false },
      }));
      toast.error(t('localServerConnectionRemoveFailed', { name: def.displayName }));
    }
  };

  const renderHealthBadge = (status: LocalServerHealthStatus) => {
    switch (status) {
      case 'healthy':
        return (
          <span className="inline-flex items-center gap-1.5 rounded-pill border border-success/30 bg-success/10 px-2.5 py-0.5 text-xs font-medium text-success">
            <CheckCircle2 aria-hidden="true" size={13} />
            {t('localServerHealthHealthy')}
          </span>
        );
      case 'unhealthy':
        return (
          <span className="inline-flex items-center gap-1.5 rounded-pill border border-warning/30 bg-warning/10 px-2.5 py-0.5 text-xs font-medium text-warning">
            <CircleAlert aria-hidden="true" size={13} />
            {t('localServerHealthUnhealthy')}
          </span>
        );
      case 'unreachable':
        return (
          <span className="inline-flex items-center gap-1.5 rounded-pill border border-border bg-surface px-2.5 py-0.5 text-xs font-medium text-secondary-text">
            <CircleDashed aria-hidden="true" size={13} />
            {t('localServerHealthUnreachable')}
          </span>
        );
    }
  };

  return (
    <section
      aria-labelledby="local-server-connections-heading"
      className="grid gap-3 rounded-control border border-border bg-raised p-3 sm:p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border pb-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Server aria-hidden="true" className="shrink-0 text-accent" size={18} />
            <h4 className="font-semibold text-text" id="local-server-connections-heading">
              {t('localServerConnectionsHeading')}
            </h4>
          </div>
          <p className="mt-1 text-xs text-secondary-text">{t('localServerConnectionsDescription')}</p>
        </div>
        <Button
          disabled={detectingAll}
          onClick={() => void detectServersAndLoadStatus()}
          size="small"
          variant="secondary"
        >
          {detectingAll ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
          ) : (
            <RefreshCw aria-hidden="true" size={15} />
          )}
          {t('localServerDetectPortsButton')}
        </Button>
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        {SERVER_DEFINITIONS.map((def) => {
          const state = servers[def.kind];
          const busy = detectingAll || state.probing || state.saving || state.removing;
          const allModelsMap = new Map<string, LocalServerModelEntry>();
          for (const model of state.availableModels) {
            allModelsMap.set(model.id, model);
          }
          for (const savedId of state.savedProvider?.selectedModels ?? []) {
            if (!allModelsMap.has(savedId)) {
              allModelsMap.set(savedId, { id: savedId, displayName: savedId });
            }
          }
          const combinedModels = Array.from(allModelsMap.values());
          const portInputId = `local-server-port-${def.kind}`;

          return (
            <div
              className="grid content-start gap-3 rounded-control border border-border bg-surface p-3"
              key={def.kind}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <h5 className="font-semibold text-sm text-text">{def.displayName}</h5>
                  {state.savedProvider ? (
                    <span className="inline-flex items-center gap-1 rounded-pill bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent">
                      <Link aria-hidden="true" size={12} />
                      {t('localServerConnectedBadge')}
                    </span>
                  ) : null}
                </div>
                {renderHealthBadge(state.healthStatus)}
              </div>

              <div className="grid gap-1.5">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <label className="text-xs font-medium text-text" htmlFor={portInputId}>
                    {t('localServerPortLabel')}
                  </label>
                  <span className="font-mono text-xs text-secondary-text">
                    {`http://127.0.0.1:${parsePortNumber(state.portInput) ?? def.defaultPort}/v1`}
                  </span>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <div className="w-32">
                    <Input
                      aria-invalid={state.portError}
                      id={portInputId}
                      max={65535}
                      min={1}
                      onChange={(event) => handlePortChange(def.kind, event.target.value)}
                      placeholder={String(def.defaultPort)}
                      type="number"
                      value={state.portInput}
                    />
                  </div>
                  <Button
                    disabled={busy}
                    onClick={() => void probeServerModels(def.kind)}
                    size="small"
                    variant="secondary"
                  >
                    {state.probing ? (
                      <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                    ) : (
                      <RefreshCw aria-hidden="true" size={15} />
                    )}
                    {t('localServerCheckAndFetchModels')}
                  </Button>
                </div>
                {state.portError ? (
                  <p className="text-xs text-danger" role="alert">
                    {t('localServerPortInvalid')}
                  </p>
                ) : null}
                {state.detectedPorts.length > 0 ? (
                  <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
                    <span className="text-xs text-secondary-text">{t('localServerDetectedPortsLabel')}:</span>
                    {state.detectedPorts.map((detectedPort) => (
                      <Button
                        disabled={busy}
                        key={detectedPort}
                        onClick={() => void probeServerModels(def.kind, detectedPort)}
                        size="small"
                        variant={String(detectedPort) === state.portInput.trim() ? 'secondary' : 'ghost'}
                      >
                        {String(detectedPort)}
                      </Button>
                    ))}
                  </div>
                ) : null}
              </div>

              <div className="grid gap-2 border-t border-border pt-2.5">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-xs font-medium text-text">
                    {t('localServerModelsHeading', {
                      selected: state.selectedModelIds.length,
                      total: combinedModels.length,
                    })}
                  </span>
                  {combinedModels.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      <Button
                        disabled={busy || state.selectedModelIds.length === combinedModels.length}
                        onClick={() => setAllModelsSelection(def.kind, true)}
                        size="small"
                        variant="ghost"
                      >
                        {t('localServerSelectAllModels')}
                      </Button>
                      <Button
                        disabled={busy || state.selectedModelIds.length === 0}
                        onClick={() => setAllModelsSelection(def.kind, false)}
                        size="small"
                        variant="ghost"
                      >
                        {t('localServerClearSelectedModels')}
                      </Button>
                    </div>
                  ) : null}
                </div>

                {combinedModels.length > 0 ? (
                  <ul className="grid max-h-44 gap-1 overflow-y-auto rounded-control border border-border bg-raised p-2">
                    {combinedModels.map((model) => {
                      const checked = state.selectedModelIds.includes(model.id);
                      const checkboxId = `local-server-model-${def.kind}-${model.id}`;
                      return (
                        <li key={model.id}>
                          <label
                            className="flex cursor-pointer items-center gap-2 rounded-control px-2 py-1 text-xs text-text hover:bg-surface"
                            htmlFor={checkboxId}
                          >
                            <input
                              checked={checked}
                              className="h-3.5 w-3.5 rounded border-border accent-accent"
                              disabled={busy}
                              id={checkboxId}
                              onChange={() => toggleModelSelection(def.kind, model.id)}
                              type="checkbox"
                            />
                            <span className="truncate font-mono">{model.displayName}</span>
                          </label>
                        </li>
                      );
                    })}
                  </ul>
                ) : (
                  <p className="rounded-control border border-border bg-raised px-2.5 py-2 text-xs text-secondary-text">
                    {t('localServerNoModelsDetected')}
                  </p>
                )}
              </div>

              <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border pt-2.5">
                {state.savedProvider ? (
                  <Button
                    disabled={busy}
                    onClick={() => void handleRemoveConnection(def.kind)}
                    size="small"
                    variant="ghost"
                  >
                    {state.removing ? (
                      <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                    ) : (
                      <Trash2 aria-hidden="true" size={15} />
                    )}
                    {t('localServerDisconnectButton')}
                  </Button>
                ) : (
                  <span />
                )}
                <Button
                  disabled={busy || (combinedModels.length > 0 && state.selectedModelIds.length === 0)}
                  onClick={() => void handleSaveConnection(def.kind)}
                  size="small"
                  variant="primary"
                >
                  {state.saving ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                  ) : (
                    <Link aria-hidden="true" size={15} />
                  )}
                  {state.savedProvider ? t('localServerUpdateCatalogButton') : t('localServerConnectAndAddModelsButton')}
                </Button>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
