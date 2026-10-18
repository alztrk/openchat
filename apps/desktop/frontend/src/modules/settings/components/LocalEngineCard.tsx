import { ChevronDown, LoaderCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Card } from '@/design-system';
import { LocalEngineIcon } from '@/modules/providers/components/EngineIcon';
import type {
  BusyAction,
  EngineVariant,
  ExternalServer,
  InstallProgress,
  LocalEngine,
  LocalModel,
} from '@/modules/settings/data/localEngineCatalog';
import {
  acceleratorLabel,
  catalogStatusKey,
  channelStatusKey,
  runtimeStatusKey,
  variantStatusKey,
} from '@/modules/settings/data/localEngineCatalog';
import { EngineModelDirectorySetting } from './EngineModelDirectorySetting';
import { LlamaRuntimeSetting } from './LlamaRuntimeSetting';
import { InstallProgressInline } from './LocalEngineInstallProgress';
import { LocalModelsSection } from './LocalModelsSection';

export function EngineCard({
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
