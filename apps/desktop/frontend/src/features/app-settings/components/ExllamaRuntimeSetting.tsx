import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input, Select } from '@shared/components';
import type { LocalEngineSettings } from '@features/app-settings/data/localEngineCatalog';

export function ExllamaRuntimeSetting({
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
    draft.exllamaMaxSeqLen !== settings.exllamaMaxSeqLen ||
    draft.exllamaPort !== settings.exllamaPort ||
    draft.exllamaCacheMode !== settings.exllamaCacheMode;

  return (
    <Card className="grid gap-4 bg-surface" padding="compact">
      <h4 className="font-semibold text-text">{t('localEngineExllamaSettings')}</h4>
      <div className="grid gap-3 sm:grid-cols-2">
        <Input
          description={t('localEngineContextSizeDescription')}
          label={t('localEngineExllamaMaxSeqLen')}
          max={2000000}
          min={128}
          onChange={(event) => {
            const parsed = event.target.value.trim() === '' ? null : Number(event.target.value);
            if (parsed === null || Number.isSafeInteger(parsed)) {
              setDraft((current) => ({ ...current, exllamaMaxSeqLen: parsed }));
            }
          }}
          type="number"
          value={draft.exllamaMaxSeqLen ?? ''}
        />
        <Input
          label={t('localEngineExllamaPort')}
          max={65535}
          min={1024}
          onChange={(event) => {
            const parsed = event.target.value.trim() === '' ? null : Number(event.target.value);
            if (parsed === null || Number.isSafeInteger(parsed)) {
              setDraft((current) => ({ ...current, exllamaPort: parsed }));
            }
          }}
          type="number"
          value={draft.exllamaPort ?? ''}
        />
        <Select
          label={t('localEngineExllamaCacheMode')}
          onChange={(event) => {
            const value = event.target.value;
            if (value === 'default' || value === 'FP16' || value === 'Q8' || value === 'Q6' || value === 'Q4') {
              setDraft((current) => ({ ...current, exllamaCacheMode: value === 'default' ? null : value }));
            }
          }}
          options={[
            { label: t('localEngineDefault'), value: 'default' },
            { label: 'FP16', value: 'FP16' },
            { label: 'Q8', value: 'Q8' },
            { label: 'Q6', value: 'Q6' },
            { label: 'Q4', value: 'Q4' },
          ]}
          value={draft.exllamaCacheMode ?? 'default'}
        />
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          disabled={active || !dirty}
          onClick={() =>
            setDraft((current) => ({
              ...current,
              exllamaMaxSeqLen: null,
              exllamaPort: null,
              exllamaCacheMode: null,
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
