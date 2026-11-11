import { FolderOpen, Info, Link, LoaderCircle, RotateCcw, Save, Search, Sliders } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input, Switch } from '@shared/components';
import {
  DEFAULT_LOCAL_ENGINE_SETTINGS,
  type ExternalServer,
  type LocalEngineSettings,
} from '@features/app-settings/data/localEngineCatalog';

interface LlamaRuntimeSettingProps {
  executableActive: boolean;
  executableAvailable: boolean;
  executablePath: string | null;
  externalActive: boolean;
  externalServers: ExternalServer[];
  settings: LocalEngineSettings;
  settingsActive: boolean;
  showExternalCheck: boolean;
  onCheckExternalServers(): void;
  onChooseExecutable(): void;
  onClearExecutable(): void;
  onDisconnectExternalServer(server: ExternalServer): void;
  onSaveSettings(updated: LocalEngineSettings): Promise<void>;
}

const CONTEXT_PRESETS = [2048, 4096, 8192, 16384, 32768, 65536] as const;

function areSettingsEqual(a: LocalEngineSettings, b: LocalEngineSettings): boolean {
  return (
    a.llamaServerExecutablePath === b.llamaServerExecutablePath &&
    a.gpuLayers === b.gpuLayers &&
    a.threads === b.threads &&
    a.threadsBatch === b.threadsBatch &&
    a.contextSize === b.contextSize &&
    a.batchSize === b.batchSize &&
    a.ubatchSize === b.ubatchSize &&
    a.flashAttention === b.flashAttention &&
    a.mlock === b.mlock &&
    a.noMmap === b.noMmap &&
    a.parallel === b.parallel &&
    a.customArguments === b.customArguments
  );
}

export function LlamaRuntimeSetting({
  executableActive,
  executableAvailable,
  executablePath,
  externalActive,
  externalServers,
  settings,
  settingsActive,
  showExternalCheck,
  onCheckExternalServers,
  onChooseExecutable,
  onClearExecutable,
  onDisconnectExternalServer,
  onSaveSettings,
}: LlamaRuntimeSettingProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<LocalEngineSettings>(settings);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const isDirty = !areSettingsEqual(draft, settings);

  const handleNumberInput = (key: keyof LocalEngineSettings, raw: string) => {
    const trimmed = raw.trim();
    if (trimmed === '') {
      setDraft((prev) => ({ ...prev, [key]: null }));
      return;
    }
    const parsed = Number.parseInt(trimmed, 10);
    if (!Number.isNaN(parsed)) {
      setDraft((prev) => ({ ...prev, [key]: parsed }));
    }
  };

  const handleToggle = (key: 'flashAttention' | 'mlock' | 'noMmap', checked: boolean) => {
    setDraft((prev) => ({ ...prev, [key]: checked ? true : null }));
  };

  const handleResetDefaults = () => {
    setDraft({
      ...DEFAULT_LOCAL_ENGINE_SETTINGS,
      llamaServerExecutablePath: settings.llamaServerExecutablePath,
    });
  };

  return (
    <Card className="grid gap-4 bg-surface" padding="compact">
      {/* Executable Selection */}
      <section className="grid gap-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h4 className="font-semibold text-text">{t('localEngineExecutable')}</h4>
          <div className="flex flex-wrap gap-1.5">
            <Button
              disabled={executableActive}
              onClick={onChooseExecutable}
              size="small"
              variant={!executableAvailable ? 'primary' : 'secondary'}
            >
              {executableActive ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <FolderOpen aria-hidden="true" size={16} />
              )}
              {t('localEngineExecutableChoose')}
            </Button>
            {executablePath ? (
              <Button disabled={executableActive} onClick={onClearExecutable} size="small" variant="ghost">
                {t('localEngineExecutableClear')}
              </Button>
            ) : null}
          </div>
        </div>
        <p className={`break-all font-mono text-xs ${executableAvailable ? 'text-secondary-text' : 'text-warning'}`}>
          {executablePath ?? t('localEngineExecutableNotConfigured')}
        </p>
        {!executableAvailable && executablePath ? (
          <p className="text-sm text-warning" role="status">
            {t('localEngineExecutableMissing')}
          </p>
        ) : null}
      </section>

      {/* External Server Scan / Connections */}
      {showExternalCheck || externalServers.length > 0 ? (
        <section className="grid gap-2 rounded-control border border-border bg-raised p-2.5">
          {showExternalCheck ? (
            <div className="flex justify-end">
              <Button disabled={externalActive} onClick={onCheckExternalServers} size="small" variant="secondary">
                {externalActive ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <Search aria-hidden="true" size={16} />
                )}
                {t('localEngineExternalServerCheck')}
              </Button>
            </div>
          ) : null}
          {externalServers.length > 0 ? (
            <ul className="grid gap-2">
              {externalServers.map((server) => (
                <li
                  className="flex flex-wrap items-center justify-between gap-3 rounded-control border border-border bg-surface px-3 py-2"
                  key={`${server.processId}:${server.port}`}
                >
                  <p className="flex min-w-0 items-center gap-2 text-sm text-secondary-text">
                    <Link aria-hidden="true" className="shrink-0 text-accent-icon" size={15} />
                    <span>
                      {t('localEngineExternalServerConnected', { port: server.port, count: server.modelIds.length })}
                    </span>
                  </p>
                  <Button
                    disabled={externalActive}
                    onClick={() => onDisconnectExternalServer(server)}
                    size="small"
                    variant="ghost"
                  >
                    {t('localEngineExternalServerDisconnect')}
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}
        </section>
      ) : null}

      {/* Local Engine Runtime & Hardware Configuration */}
      <section className="grid gap-4 rounded-control border border-border bg-raised p-3 sm:p-4">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border pb-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Sliders aria-hidden="true" className="shrink-0 text-accent" size={18} />
              <h4 className="font-semibold text-text">{t('localEngineRuntimeSettings')}</h4>
            </div>
            <p className="mt-1 text-xs text-secondary-text">{t('localEngineRuntimeSettingsDescription')}</p>
          </div>
          <p className="text-xs text-secondary-text">{t('localEngineSettingsRestartNote')}</p>
        </div>

        {/* GPU Offloading (-ngl) */}
        <div className="grid gap-1.5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-gpu-layers">
              {t('localEngineGpuLayers')}
            </label>
            <div className="flex flex-wrap gap-1">
              <Button
                onClick={() => setDraft((prev) => ({ ...prev, gpuLayers: 0 }))}
                size="small"
                variant={draft.gpuLayers === 0 ? 'secondary' : 'ghost'}
              >
                {t('localEngineGpuLayersCpuOnly')}
              </Button>
              <Button
                onClick={() => setDraft((prev) => ({ ...prev, gpuLayers: 33 }))}
                size="small"
                variant={draft.gpuLayers === 33 ? 'secondary' : 'ghost'}
              >
                {t('localEngineGpuLayersHybrid')}
              </Button>
              <Button
                onClick={() => setDraft((prev) => ({ ...prev, gpuLayers: 99 }))}
                size="small"
                variant={draft.gpuLayers === 99 ? 'secondary' : 'ghost'}
              >
                {t('localEngineGpuLayersAllGpu')}
              </Button>
              <Button
                onClick={() => setDraft((prev) => ({ ...prev, gpuLayers: null }))}
                size="small"
                variant={draft.gpuLayers === null ? 'secondary' : 'ghost'}
              >
                {t('localEngineDefaultAuto')}
              </Button>
            </div>
          </div>
          <Input
            id="local-engine-gpu-layers"
            max={999}
            min={0}
            onChange={(e) => handleNumberInput('gpuLayers', e.target.value)}
            placeholder={t('localEngineDefaultAuto')}
            type="number"
            value={draft.gpuLayers ?? ''}
          />
          <p className="text-xs text-secondary-text">{t('localEngineGpuLayersDescription')}</p>
        </div>

        {/* Context Window Size (-c) */}
        <div className="grid gap-1.5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-context-size">
              {t('localEngineContextSize')}
            </label>
            <div className="flex flex-wrap gap-1">
              {CONTEXT_PRESETS.map((preset) => (
                <Button
                  key={preset}
                  onClick={() => setDraft((prev) => ({ ...prev, contextSize: preset }))}
                  size="small"
                  variant={draft.contextSize === preset ? 'secondary' : 'ghost'}
                >
                  {preset >= 1024 ? `${preset / 1024}K` : preset}
                </Button>
              ))}
              <Button
                onClick={() => setDraft((prev) => ({ ...prev, contextSize: null }))}
                size="small"
                variant={draft.contextSize === null ? 'secondary' : 'ghost'}
              >
                {t('localEngineDefaultAuto')}
              </Button>
            </div>
          </div>
          <Input
            id="local-engine-context-size"
            max={2000000}
            min={512}
            onChange={(e) => handleNumberInput('contextSize', e.target.value)}
            placeholder={t('localEngineDefaultAuto')}
            step={512}
            type="number"
            value={draft.contextSize ?? ''}
          />
          <p className="text-xs text-secondary-text">{t('localEngineContextSizeDescription')}</p>
        </div>

        {/* CPU Threads: Generation (-t) & Prompt Batch (-tb) */}
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-threads">
              {t('localEngineThreads')}
            </label>
            <Input
              id="local-engine-threads"
              max={256}
              min={1}
              onChange={(e) => handleNumberInput('threads', e.target.value)}
              placeholder={t('localEngineDefaultAuto')}
              type="number"
              value={draft.threads ?? ''}
            />
            <p className="text-xs text-secondary-text">{t('localEngineThreadsDescription')}</p>
          </div>
          <div className="grid gap-1.5">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-threads-batch">
              {t('localEngineThreadsBatch')}
            </label>
            <Input
              id="local-engine-threads-batch"
              max={256}
              min={1}
              onChange={(e) => handleNumberInput('threadsBatch', e.target.value)}
              placeholder={t('localEngineDefaultAuto')}
              type="number"
              value={draft.threadsBatch ?? ''}
            />
            <p className="text-xs text-secondary-text">{t('localEngineThreadsBatchDescription')}</p>
          </div>
        </div>

        {/* Batch Size (-b) & Micro-Batch Size (-ub) */}
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-batch-size">
              {t('localEngineBatchSize')}
            </label>
            <Input
              id="local-engine-batch-size"
              max={32768}
              min={1}
              onChange={(e) => handleNumberInput('batchSize', e.target.value)}
              placeholder={t('localEngineDefaultAuto')}
              type="number"
              value={draft.batchSize ?? ''}
            />
            <p className="text-xs text-secondary-text">{t('localEngineBatchSizeDescription')}</p>
          </div>
          <div className="grid gap-1.5">
            <label className="text-xs font-medium text-text" htmlFor="local-engine-ubatch-size">
              {t('localEngineUbatchSize')}
            </label>
            <Input
              id="local-engine-ubatch-size"
              max={32768}
              min={1}
              onChange={(e) => handleNumberInput('ubatchSize', e.target.value)}
              placeholder={t('localEngineDefaultAuto')}
              type="number"
              value={draft.ubatchSize ?? ''}
            />
            <p className="text-xs text-secondary-text">{t('localEngineUbatchSizeDescription')}</p>
          </div>
        </div>

        {/* Memory & Compute Flags (Flash Attention, mlock, no-mmap) */}
        <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-3">
          <div className="flex flex-col justify-between gap-2 rounded-control border border-border bg-surface p-3">
            <div>
              <p className="text-xs font-medium text-text">{t('localEngineFlashAttention')}</p>
              <p className="mt-1 text-xs text-secondary-text">{t('localEngineFlashAttentionDescription')}</p>
            </div>
            <div className="flex items-center justify-end pt-1">
              <Switch
                aria-label={t('localEngineFlashAttention')}
                checked={draft.flashAttention === true}
                onCheckedChange={(checked) => handleToggle('flashAttention', checked)}
              />
            </div>
          </div>
          <div className="flex flex-col justify-between gap-2 rounded-control border border-border bg-surface p-3">
            <div>
              <p className="text-xs font-medium text-text">{t('localEngineMlock')}</p>
              <p className="mt-1 text-xs text-secondary-text">{t('localEngineMlockDescription')}</p>
            </div>
            <div className="flex items-center justify-end pt-1">
              <Switch
                aria-label={t('localEngineMlock')}
                checked={draft.mlock === true}
                onCheckedChange={(checked) => handleToggle('mlock', checked)}
              />
            </div>
          </div>
          <div className="flex flex-col justify-between gap-2 rounded-control border border-border bg-surface p-3">
            <div>
              <p className="text-xs font-medium text-text">{t('localEngineNoMmap')}</p>
              <p className="mt-1 text-xs text-secondary-text">{t('localEngineNoMmapDescription')}</p>
            </div>
            <div className="flex items-center justify-end pt-1">
              <Switch
                aria-label={t('localEngineNoMmap')}
                checked={draft.noMmap === true}
                onCheckedChange={(checked) => handleToggle('noMmap', checked)}
              />
            </div>
          </div>
        </div>

        {/* Parallel Slots (-np) */}
        <div className="grid gap-1.5">
          <label className="text-xs font-medium text-text" htmlFor="local-engine-parallel">
            {t('localEngineParallel')}
          </label>
          <Input
            id="local-engine-parallel"
            max={32}
            min={1}
            onChange={(e) => handleNumberInput('parallel', e.target.value)}
            placeholder={t('localEngineDefaultAuto')}
            type="number"
            value={draft.parallel ?? ''}
          />
          <p className="text-xs text-secondary-text">{t('localEngineParallelDescription')}</p>
        </div>

        {/* Custom CLI Arguments */}
        <div className="grid gap-1.5">
          <label className="text-xs font-medium text-text" htmlFor="local-engine-custom-arguments">
            {t('localEngineCustomArguments')}
          </label>
          <Input
            id="local-engine-custom-arguments"
            onChange={(e) => {
              const val = e.target.value;
              setDraft((prev) => ({ ...prev, customArguments: val.trim() === '' ? null : val }));
            }}
            placeholder={t('localEngineCustomArgumentsPlaceholder')}
            type="text"
            value={draft.customArguments ?? ''}
          />
          <p className="text-xs text-secondary-text">{t('localEngineCustomArgumentsDescription')}</p>
        </div>

        {/* Action Buttons */}
        <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border pt-3">
          <Button disabled={settingsActive} onClick={handleResetDefaults} size="small" variant="ghost">
            <RotateCcw aria-hidden="true" size={15} />
            {t('localEngineResetDefaults')}
          </Button>
          <Button
            disabled={settingsActive || !isDirty}
            onClick={() => void onSaveSettings(draft)}
            size="small"
            variant="primary"
          >
            {settingsActive ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
            ) : (
              <Save aria-hidden="true" size={15} />
            )}
            {settingsActive ? t('localEngineSavingSettings') : t('localEngineSaveSettings')}
          </Button>
        </div>
      </section>

      {/* Third-party Servers (Ollama, LM Studio) guidance */}
      <section className="flex items-start gap-2.5 rounded-control border border-border bg-raised p-2.5">
        <Info aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={16} />
        <p className="text-xs leading-relaxed text-secondary-text">{t('localEngineThirdPartyServersNote')}</p>
      </section>
    </Card>
  );
}
