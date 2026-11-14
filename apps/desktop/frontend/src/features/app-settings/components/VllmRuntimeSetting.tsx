import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Input, Select, Switch } from '@shared/components';
import type { LocalEngineSettings } from '@features/app-settings/data/localEngineCatalog';

export function VllmRuntimeSetting({
  active,
  onSave,
  settings,
}: {
  active: boolean;
  onSave(updated: LocalEngineSettings): Promise<void>;
  settings: LocalEngineSettings;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(settings);

  useEffect(() => setDraft(settings), [settings]);

  const updateNumber = (key: 'vllmMaxModelLen' | 'vllmPort' | 'vllmGpuMemoryUtilization' | 'vllmTensorParallelSize' | 'vllmMaxNumSeqs', value: string) => {
    const parsed = value.trim() === '' ? null : Number(value);
    if (parsed === null || Number.isFinite(parsed)) setDraft((current) => ({ ...current, [key]: parsed }));
  };
  const dirty =
    draft.vllmMaxModelLen !== settings.vllmMaxModelLen ||
    draft.vllmPort !== settings.vllmPort ||
    draft.vllmGpuMemoryUtilization !== settings.vllmGpuMemoryUtilization ||
    draft.vllmTensorParallelSize !== settings.vllmTensorParallelSize ||
    draft.vllmMaxNumSeqs !== settings.vllmMaxNumSeqs ||
    draft.vllmDtype !== settings.vllmDtype ||
    draft.vllmEnforceEager !== settings.vllmEnforceEager ||
    draft.vllmCustomArguments !== settings.vllmCustomArguments;

  return (
    <Card className="grid gap-4 bg-surface" padding="compact">
      <h4 className="font-semibold text-text">{t('localEngineVllmSettings')}</h4>
      <div className="grid gap-3 sm:grid-cols-2">
        <NumberSetting label={t('localEngineVllmMaxModelLen')} value={draft.vllmMaxModelLen} onChange={(value) => updateNumber('vllmMaxModelLen', value)} />
        <NumberSetting label={t('localEngineVllmPort')} value={draft.vllmPort} onChange={(value) => updateNumber('vllmPort', value)} min="1024" max="65535" />
        <NumberSetting label={t('localEngineVllmGpuMemory')} value={draft.vllmGpuMemoryUtilization} onChange={(value) => updateNumber('vllmGpuMemoryUtilization', value)} step="0.01" min="0.1" max="0.99" />
        <NumberSetting label={t('localEngineVllmTensorParallel')} value={draft.vllmTensorParallelSize} onChange={(value) => updateNumber('vllmTensorParallelSize', value)} min="1" max="16" />
        <NumberSetting label={t('localEngineVllmMaxSequences')} value={draft.vllmMaxNumSeqs} onChange={(value) => updateNumber('vllmMaxNumSeqs', value)} min="1" max="1024" />
        <Select
          label={t('localEngineVllmDtype')}
          onChange={(event) => {
            const value = event.target.value;
            if (value === 'default' || value === 'auto' || value === 'float16' || value === 'bfloat16') {
              setDraft((current) => ({ ...current, vllmDtype: value === 'default' ? null : value }));
            }
          }}
          options={[
            { label: t('localEngineDefault'), value: 'default' },
            { label: 'auto', value: 'auto' },
            { label: 'float16', value: 'float16' },
            { label: 'bfloat16', value: 'bfloat16' },
          ]}
          value={draft.vllmDtype ?? 'default'}
        />
        <div className="flex min-h-10 items-center justify-between gap-3 text-sm text-secondary-text">
          <span>{t('localEngineVllmEnforceEager')}</span>
          <Switch aria-label={t('localEngineVllmEnforceEager')} checked={draft.vllmEnforceEager === true} onCheckedChange={(checked) => setDraft((current) => ({ ...current, vllmEnforceEager: checked ? true : null }))} />
        </div>
        <Input
          label={t('localEngineVllmCustomArguments')}
          onChange={(event) => setDraft((current) => ({ ...current, vllmCustomArguments: event.target.value || null }))}
          type="text"
          value={draft.vllmCustomArguments ?? ''}
        />
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button disabled={active || !dirty} onClick={() => setDraft((current) => ({ ...current, vllmMaxModelLen: null, vllmPort: null, vllmGpuMemoryUtilization: null, vllmTensorParallelSize: null, vllmMaxNumSeqs: null, vllmDtype: null, vllmEnforceEager: null, vllmCustomArguments: null }))} size="small" variant="ghost">
          {t('localEngineResetDefaults')}
        </Button>
        <Button disabled={active || !dirty} loading={active} onClick={() => void onSave(draft)} size="small" variant="primary">
          {t('save')}
        </Button>
      </div>
    </Card>
  );
}

function NumberSetting({
  label,
  max,
  min,
  onChange,
  step = '1',
  value,
}: {
  label: string;
  max?: string;
  min?: string;
  onChange(value: string): void;
  step?: string;
  value: number | null;
}) {
  return (
    <Input label={label} max={max} min={min} onChange={(event) => onChange(event.target.value)} step={step} type="number" value={value ?? ''} />
  );
}
