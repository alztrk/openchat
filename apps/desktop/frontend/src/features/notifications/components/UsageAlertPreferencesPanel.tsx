import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input, Select } from '@shared/components';
import {
  defaultUsageAlertPreferences,
  readUsageAlertPreferences,
  type UsageAlertPreferences,
  writeUsageAlertPreferences,
} from '@shared/lib/usage/usageAlertPreferences';

type LoadState = 'loading' | 'loaded' | 'failed';

export function UsageAlertPreferencesPanel() {
  const { t } = useTranslation();
  const [preferences, setPreferences] = useState<UsageAlertPreferences>(defaultUsageAlertPreferences);
  const [costLimitDraft, setCostLimitDraft] = useState('');
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isSaving, setIsSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [costLimitInvalid, setCostLimitInvalid] = useState(false);

  const load = useCallback(() => {
    setLoadState('loading');
    try {
      const next = readUsageAlertPreferences();
      setPreferences(next);
      setCostLimitDraft(next.monthlyEstimatedCostLimitUsd?.toString() ?? '');
      setLoadState('loaded');
    } catch {
      setLoadState('failed');
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const save = async (next: UsageAlertPreferences, allowLoadFailure = false) => {
    if (isSaving || (loadState !== 'loaded' && !allowLoadFailure)) return;
    setIsSaving(true);
    setSaveFailed(false);
    try {
      await writeUsageAlertPreferences(next);
      setPreferences(next);
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const saveCostLimit = async () => {
    const value = costLimitDraft.trim();
    if (value.length > 0 && (!/^\d+(\.\d{1,2})?$/.test(value) || Number(value) <= 0 || Number(value) > 100_000)) {
      setCostLimitInvalid(true);
      return;
    }
    setCostLimitInvalid(false);
    await save({ ...preferences, monthlyEstimatedCostLimitUsd: value.length === 0 ? null : Number(value) });
  };

  return (
    <Card className="grid gap-4" padding="default">
      <header className="grid gap-1">
        <h2 className="font-semibold">{t('usageAlertSettings')}</h2>
        <p className="text-sm text-secondary-text">{t('usageAlertSettingsDescription')}</p>
      </header>

      {loadState === 'failed' ? (
        <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
          <p className="text-sm text-destructive">{t('usageAlertPreferencesLoadFailed')}</p>
          <div className="flex flex-wrap gap-2">
            <Button onClick={load} size="small" variant="ghost">
              {t('retry')}
            </Button>
            <Button
              onClick={() => {
                setPreferences(defaultUsageAlertPreferences);
                setCostLimitDraft('');
                setLoadState('loaded');
                void save(defaultUsageAlertPreferences, true);
              }}
              size="small"
              variant="secondary"
            >
              {t('usageAlertPreferencesReset')}
            </Button>
          </div>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <AlertThreshold
            description={t('quotaWarningThresholdDescription')}
            disabled={loadState !== 'loaded' || isSaving}
            label={t('quotaWarningThreshold')}
            onChange={(value) => void save({ ...preferences, quotaWarningAtPercent: value })}
            value={preferences.quotaWarningAtPercent}
          />
          <AlertThreshold
            description={t('monthlyCostWarningThresholdDescription')}
            disabled={loadState !== 'loaded' || isSaving}
            label={t('monthlyCostWarningThreshold')}
            onChange={(value) => void save({ ...preferences, monthlyCostWarningAtPercent: value })}
            value={preferences.monthlyCostWarningAtPercent}
          />
          <div className="grid content-start gap-2">
            <div className="grid gap-1">
              <h3 className="text-sm font-medium">{t('monthlyEstimatedCostLimit')}</h3>
              <p className="text-sm text-secondary-text">{t('monthlyEstimatedCostLimitDescription')}</p>
            </div>
            <div className="flex flex-wrap items-end gap-2">
              <Input
                className="max-w-40"
                disabled={loadState !== 'loaded' || isSaving}
                error={costLimitInvalid ? t('usageCostAlertInvalid') : undefined}
                label={t('monthlyEstimatedCostLimit')}
                max={100_000}
                min={0.01}
                onChange={(event) => {
                  setCostLimitDraft(event.target.value);
                  setCostLimitInvalid(false);
                }}
                placeholder={t('usageCostAlertDisabled')}
                step={0.01}
                type="number"
                value={costLimitDraft}
              />
              <Button
                disabled={loadState !== 'loaded' || isSaving}
                loading={isSaving}
                onClick={() => void saveCostLimit()}
                size="small"
                variant="secondary"
              >
                {t('save')}
              </Button>
            </div>
          </div>
        </div>
      )}

      {saveFailed ? (
        <p className="text-sm text-destructive" role="alert">
          {t('usageAlertPreferencesSaveFailed')}
        </p>
      ) : null}
    </Card>
  );
}

function AlertThreshold({
  description,
  disabled,
  label,
  onChange,
  value,
}: {
  description: string;
  disabled: boolean;
  label: string;
  onChange(value: number): void;
  value: number;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid content-start gap-2">
      <div className="grid gap-1">
        <h3 className="text-sm font-medium">{label}</h3>
        <p className="text-sm text-secondary-text">{description}</p>
      </div>
      <Select
        disabled={disabled}
        label={label}
        onChange={(event) => {
          const next = Number(event.target.value);
          if ([50, 75, 80, 90, 95].includes(next)) onChange(next);
        }}
        options={[50, 75, 80, 90, 95].map((percent) => ({
          value: String(percent),
          label: t('usageUsedPercent', { percent }),
        }))}
        value={String(value)}
        visuallyHiddenLabel
      />
    </div>
  );
}
