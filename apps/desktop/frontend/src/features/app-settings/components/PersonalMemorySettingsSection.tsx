import { Brain, LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Switch } from '@shared/components';
import { SettingsSectionSkeleton } from './SettingsSectionSkeleton';
import {
  clearPersonalMemory,
  deletePersonalMemoryFact,
  maxPersonalMemoryFacts,
  type PersonalMemoryFact,
  type PersonalMemoryPreferences,
  readPersonalMemoryPreferences,
  writePersonalMemoryEnabled,
} from '@shared/lib/preferences/personalMemory';

type LoadState = 'loading' | 'loaded' | 'failed';

export function PersonalMemorySettingsSection() {
  const { t } = useTranslation();
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [preferences, setPreferences] = useState<PersonalMemoryPreferences>({ enabled: false, facts: [] });
  const [isSaving, setIsSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [clearFailed, setClearFailed] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<PersonalMemoryFact | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(async () => {
    setLoadState('loading');
    try {
      setPreferences(await readPersonalMemoryPreferences());
      setLoadState('loaded');
    } catch {
      setLoadState('failed');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const setEnabled = async (enabled: boolean) => {
    setIsSaving(true);
    setSaveFailed(false);
    try {
      await writePersonalMemoryEnabled(enabled);
      setPreferences((current) => ({ ...current, enabled }));
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const deleteFact = async () => {
    if (!pendingDelete || isSaving) return;
    setIsSaving(true);
    setSaveFailed(false);
    try {
      await deletePersonalMemoryFact(pendingDelete.id);
      setPreferences((current) => ({
        ...current,
        facts: current.facts.filter((fact) => fact.id !== pendingDelete.id),
      }));
      setPendingDelete(null);
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const clearAll = async () => {
    setIsSaving(true);
    setClearFailed(false);
    try {
      await clearPersonalMemory();
      setPreferences({ enabled: false, facts: [] });
      setLoadState('loaded');
      setConfirmClear(false);
    } catch {
      setClearFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  if (loadState === 'loading') return <SettingsSectionSkeleton label={t('settingsSectionLoading')} />;

  return (
    <div className="grid gap-3">
      <Card className="flex flex-wrap items-center justify-between gap-4 rounded-control border border-border bg-raised p-4">
        <div className="grid min-w-0 gap-1">
          <span className="font-medium">{t('personalMemoryEnabled')}</span>
          <span className="max-w-3xl text-sm leading-relaxed text-secondary-text">
            {t('personalMemoryEnabledDescription')}
          </span>
        </div>
        {loadState === 'loaded' ? (
          <Switch
            aria-label={t('personalMemoryEnabled')}
            checked={preferences.enabled}
            disabled={isSaving}
            onCheckedChange={setEnabled}
          />
        ) : null}
      </Card>

      <Card className="overflow-hidden rounded-control border border-border bg-raised">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-surface/50 px-4 py-3">
          <h4 className="font-medium">{t('personalMemoryFactsTitle')}</h4>
          {loadState === 'loaded' ? (
            <div className="flex items-center gap-3">
              <span className="text-xs tabular-nums text-secondary-text">
                {preferences.facts.length} / {maxPersonalMemoryFacts}
              </span>
              {preferences.facts.length > 0 ? (
                <Button disabled={isSaving} onClick={() => setConfirmClear(true)} size="small" variant="ghost">
                  <Trash2 aria-hidden="true" size={15} />
                  {t('personalMemoryClearAction')}
                </Button>
              ) : null}
            </div>
          ) : null}
        </div>

        {loadState === 'failed' ? (
          <div className="grid justify-items-start gap-3 p-4">
            <p className="text-sm text-destructive" role="alert">
              {t('personalMemoryLoadFailed')}
            </p>
            <div className="flex flex-wrap gap-2">
              <Button onClick={load} size="small">
                <RefreshCw aria-hidden="true" size={15} />
                {t('retry')}
              </Button>
              <Button onClick={() => setConfirmClear(true)} size="small" variant="destructive">
                <Trash2 aria-hidden="true" size={15} />
                {t('personalMemoryClearAction')}
              </Button>
            </div>
          </div>
        ) : preferences.facts.length === 0 ? (
          <div className="grid justify-items-center gap-2 px-4 py-8 text-center">
            <span className="grid size-10 place-items-center rounded-full bg-surface text-secondary-icon">
              <Brain aria-hidden="true" size={19} />
            </span>
            <p className="max-w-md text-sm text-secondary-text">{t('personalMemoryEmpty')}</p>
          </div>
        ) : (
          <ul className="divide-y divide-border px-4">
            {preferences.facts.map((fact) => (
              <li className="flex min-w-0 items-start gap-3 py-3" key={fact.id}>
                <p className="min-w-0 flex-1 whitespace-pre-wrap break-words text-sm leading-relaxed">{fact.content}</p>
                <Button
                  aria-label={t('personalMemoryDelete')}
                  disabled={isSaving}
                  onClick={() => setPendingDelete(fact)}
                  size="icon"
                  variant="ghost"
                >
                  <Trash2 aria-hidden="true" size={16} />
                </Button>
              </li>
            ))}
          </ul>
        )}
        {loadState === 'loaded' && preferences.facts.length >= maxPersonalMemoryFacts ? (
          <p className="border-t border-border px-4 py-3 text-sm text-secondary-text">
            {t('personalMemoryLimitReached')}
          </p>
        ) : null}
      </Card>

      {saveFailed ? (
        <p className="text-sm text-destructive" role="alert">
          {t('personalMemorySaveFailed')}
        </p>
      ) : null}
      {clearFailed ? (
        <p className="text-sm text-destructive" role="alert">
          {t('personalMemoryClearFailed')}
        </p>
      ) : null}

      <Dialog onOpenChange={setConfirmClear} open={confirmClear} title={t('personalMemoryClearTitle')}>
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('personalMemoryClearConfirmation')}</p>
          {clearFailed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('personalMemoryClearFailed')}
            </p>
          ) : null}
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isSaving} onClick={() => setConfirmClear(false)}>
              {t('cancel')}
            </Button>
            <Button disabled={isSaving} onClick={clearAll} variant="destructive">
              {isSaving ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <Trash2 aria-hidden="true" size={16} />
              )}
              {t('personalMemoryClearConfirm')}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
        open={pendingDelete !== null}
        title={t('personalMemoryDeleteTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('personalMemoryDeleteConfirmation')}</p>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isSaving} onClick={() => setPendingDelete(null)}>
              {t('cancel')}
            </Button>
            <Button disabled={isSaving} onClick={deleteFact} variant="destructive">
              <Trash2 aria-hidden="true" size={16} />
              {t('personalMemoryDeleteConfirm')}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
