import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input } from '@shared/components';
import type { LocalEngineSettings } from '@features/app-settings/data/localEngineCatalog';

export function SglangRuntimeSetting({
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

  const dirty =
    draft.sglangContextLength !== settings.sglangContextLength ||
    draft.sglangPort !== settings.sglangPort ||
    draft.sglangMemFractionStatic !== settings.sglangMemFractionStatic ||
    draft.sglangMaxRunningRequests !== settings.sglangMaxRunningRequests;

  return (
    <Card className="grid gap-4 bg-surface" padding="compact">
      <h4 className="font-semibold text-text">{t('localEngineSglangSettings')}</h4>
      <div className="grid gap-3 sm:grid-cols-2">
        <NumberSetting
          description={t('localEngineContextSizeDescription')}
          label={t('localEngineSglangContextLength')}
          max="2000000"
          min="128"
          onChange={(value) => updateNumber(setDraft, 'sglangContextLength', value)}
          value={draft.sglangContextLength}
        />
        <NumberSetting
          label={t('localEngineSglangPort')}
          max="65535"
          min="1024"
          onChange={(value) => updateNumber(setDraft, 'sglangPort', value)}
          value={draft.sglangPort}
        />
        <NumberSetting
          label={t('localEngineSglangMemoryFraction')}
          max="0.99"
          min="0.1"
          onChange={(value) => updateNumber(setDraft, 'sglangMemFractionStatic', value)}
          step="0.01"
          value={draft.sglangMemFractionStatic}
        />
        <NumberSetting
          label={t('localEngineSglangMaxRunningRequests')}
          max="1024"
          min="1"
          onChange={(value) => updateNumber(setDraft, 'sglangMaxRunningRequests', value)}
          value={draft.sglangMaxRunningRequests}
        />
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          disabled={active || !dirty}
          onClick={() =>
            setDraft((current) => ({
              ...current,
              sglangContextLength: null,
              sglangPort: null,
              sglangMemFractionStatic: null,
              sglangMaxRunningRequests: null,
            }))
          }
          size="small"
          variant="ghost"
        >
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
  description,
  label,
  max,
  min,
  onChange,
  step = '1',
  value,
}: {
  description?: string;
  label: string;
  max: string;
  min: string;
  onChange(value: string): void;
  step?: string;
  value: number | null;
}) {
  return <Input description={description} label={label} max={max} min={min} onChange={(event) => onChange(event.target.value)} step={step} type="number" value={value ?? ''} />;
}

function updateNumber(
  setDraft: Dispatch<SetStateAction<LocalEngineSettings>>,
  key: 'sglangContextLength' | 'sglangPort' | 'sglangMemFractionStatic' | 'sglangMaxRunningRequests',
  value: string,
) {
  const parsed = value.trim() === '' ? null : Number(value);
  const integerSetting = key !== 'sglangMemFractionStatic';
  if (parsed === null || (Number.isFinite(parsed) && (!integerSetting || Number.isSafeInteger(parsed)))) {
    setDraft((current) => ({ ...current, [key]: parsed }));
  }
}
