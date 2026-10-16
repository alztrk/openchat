import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { Brain, LoaderCircle, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Switch } from '@/shared/ui';
import {
  clearPersonalMemory,
  createPersonalMemoryFact,
  maxPersonalMemoryFactCharacters,
  maxPersonalMemoryFacts,
  readPersonalMemoryPreferences,
  writePersonalMemoryEnabled,
  writePersonalMemoryFacts,
  type PersonalMemoryFact,
  type PersonalMemoryPreferences,
} from '@/features/settings/data/personalMemory';

type LoadState = 'loading' | 'loaded' | 'failed';
type DialogMode = { type: 'add' } | { type: 'edit'; fact: PersonalMemoryFact } | null;

export function PersonalMemorySettingsSection() {
  const { t } = useTranslation();
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [preferences, setPreferences] = useState<PersonalMemoryPreferences>({ enabled: false, facts: [] });
  const [isSaving, setIsSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [clearFailed, setClearFailed] = useState(false);
  const [dialogMode, setDialogMode] = useState<DialogMode>(null);
  const [draft, setDraft] = useState('');
  const [pendingDelete, setPendingDelete] = useState<PersonalMemoryFact | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(() => {
    setLoadState('loading');
    try {
      setPreferences(readPersonalMemoryPreferences());
      setLoadState('loaded');
    } catch {
      setLoadState('failed');
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const saveFacts = (facts: PersonalMemoryFact[]): boolean => {
    setIsSaving(true);
    setSaveFailed(false);
    try {
      writePersonalMemoryFacts(facts);
      setPreferences((current) => ({ ...current, facts }));
      return true;
    } catch {
      setSaveFailed(true);
      return false;
    } finally {
      setIsSaving(false);
    }
  };

  const setEnabled = (enabled: boolean) => {
    setIsSaving(true);
    setSaveFailed(false);
    try {
      writePersonalMemoryEnabled(enabled);
      setPreferences((current) => ({ ...current, enabled }));
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const submitFact = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!dialogMode || isSaving) return;
    const normalized = draft.trim();
    if (!normalized || normalized.length > maxPersonalMemoryFactCharacters) return;
    if (dialogMode.type === 'add' && preferences.facts.length >= maxPersonalMemoryFacts) {
      setSaveFailed(true);
      return;
    }
    let next: PersonalMemoryFact[];
    try {
      next = dialogMode.type === 'add'
        ? [...preferences.facts, createPersonalMemoryFact(normalized)]
        : preferences.facts.map((fact) => fact.id === dialogMode.fact.id
          ? { ...fact, content: normalized }
          : fact);
    } catch {
      setSaveFailed(true);
      return;
    }
    if (saveFacts(next)) {
      setDialogMode(null);
      setDraft('');
    }
  };

  const deleteFact = () => {
    if (!pendingDelete || isSaving) return;
    if (saveFacts(preferences.facts.filter((fact) => fact.id !== pendingDelete.id))) {
      setPendingDelete(null);
    }
  };

  const clearAll = () => {
    setIsSaving(true);
    setClearFailed(false);
    try {
      clearPersonalMemory();
      setPreferences({ enabled: false, facts: [] });
      setLoadState('loaded');
      setConfirmClear(false);
    } catch {
      setClearFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Card className="grid gap-4 p-4 sm:p-5">
      <div className="flex items-start gap-3">
        <Brain aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={19} />
        <div className="grid gap-1">
          <h3 className="font-semibold">{t('personalMemoryTitle')}</h3>
          <p className="text-sm text-secondary-text">{t('personalMemoryDescription')}</p>
          <p className="text-sm text-secondary-text">{t('personalMemoryPrivacyNotice')}</p>
        </div>
      </div>

      {loadState === 'loading' ? (
        <p aria-live="polite" className="text-sm text-secondary-text">{t('settingsSectionLoading')}</p>
      ) : loadState === 'failed' ? (
        <div className="grid justify-items-start gap-3">
          <p className="text-sm text-destructive" role="alert">{t('personalMemoryLoadFailed')}</p>
          <div className="flex flex-wrap gap-2">
            <Button onClick={load} size="small"><RefreshCw aria-hidden="true" size={15} />{t('retry')}</Button>
            <Button onClick={() => setConfirmClear(true)} size="small" variant="destructive"><Trash2 aria-hidden="true" size={15} />{t('personalMemoryClearAction')}</Button>
          </div>
        </div>
      ) : (
        <>
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
            <div className="grid gap-1">
              <span className="font-medium">{t('personalMemoryEnabled')}</span>
              <span className="text-sm text-secondary-text">{t('personalMemoryEnabledDescription')}</span>
            </div>
            <Switch aria-label={t('personalMemoryEnabled')} checked={preferences.enabled} disabled={isSaving} onCheckedChange={setEnabled} />
          </div>

          <div className="grid gap-2 border-t border-border pt-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h4 className="font-medium">{t('personalMemoryFactsTitle')}</h4>
              <div className="flex flex-wrap items-center gap-2">
                {preferences.facts.length > 0 ? <Button disabled={isSaving} onClick={() => setConfirmClear(true)} size="small" variant="ghost"><Trash2 aria-hidden="true" size={15} />{t('personalMemoryClearAction')}</Button> : null}
                <Button disabled={isSaving || preferences.facts.length >= maxPersonalMemoryFacts} onClick={() => { setDraft(''); setDialogMode({ type: 'add' }); }} size="small"><Plus aria-hidden="true" size={15} />{t('personalMemoryAdd')}</Button>
              </div>
            </div>
            {preferences.facts.length === 0 ? (
              <p className="py-2 text-sm text-secondary-text">{t('personalMemoryEmpty')}</p>
            ) : (
              <ul className="grid divide-y divide-border">
                {preferences.facts.map((fact) => (
                  <li className="flex min-w-0 items-start gap-3 py-3" key={fact.id}>
                    <p className="min-w-0 flex-1 whitespace-pre-wrap break-words text-sm">{fact.content}</p>
                    <div className="flex shrink-0 gap-1">
                      <Button aria-label={t('personalMemoryEdit')} disabled={isSaving} onClick={() => { setDraft(fact.content); setDialogMode({ type: 'edit', fact }); }} size="icon" variant="ghost"><Pencil aria-hidden="true" size={16} /></Button>
                      <Button aria-label={t('personalMemoryDelete')} disabled={isSaving} onClick={() => setPendingDelete(fact)} size="icon" variant="ghost"><Trash2 aria-hidden="true" size={16} /></Button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
            {preferences.facts.length >= maxPersonalMemoryFacts ? <p className="text-sm text-secondary-text">{t('personalMemoryLimitReached')}</p> : null}
          </div>
        </>
      )}

      {saveFailed ? <p className="text-sm text-destructive" role="alert">{t('personalMemorySaveFailed')}</p> : null}
      {clearFailed ? <p className="text-sm text-destructive" role="alert">{t('personalMemoryClearFailed')}</p> : null}

      <Dialog onOpenChange={(open) => { if (!open) { setDialogMode(null); setDraft(''); } }} open={dialogMode !== null} title={t(dialogMode?.type === 'edit' ? 'personalMemoryEdit' : 'personalMemoryAdd')}>
        <form className="grid gap-4" onSubmit={submitFact}>
          <label className="grid gap-2 text-sm font-medium" htmlFor="personal-memory-fact">{t('personalMemoryFactLabel')}
            <textarea
              autoFocus
              className="min-h-24 w-full resize-y rounded-control border border-border bg-raised px-3 py-2 font-normal text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus"
              id="personal-memory-fact"
              maxLength={maxPersonalMemoryFactCharacters}
              onChange={(event) => setDraft(event.target.value)}
              placeholder={t('personalMemoryFactHint')}
              value={draft}
            />
          </label>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isSaving} onClick={() => setDialogMode(null)} type="button">{t('cancel')}</Button>
            <Button disabled={isSaving || !draft.trim() || draft.trim().length > maxPersonalMemoryFactCharacters} type="submit">{isSaving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}{t('save')}</Button>
          </div>
        </form>
      </Dialog>

      <Dialog onOpenChange={setConfirmClear} open={confirmClear} title={t('personalMemoryClearTitle')}>
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('personalMemoryClearConfirmation')}</p>
          {clearFailed ? <p className="text-sm text-destructive" role="alert">{t('personalMemoryClearFailed')}</p> : null}
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isSaving} onClick={() => setConfirmClear(false)}>{t('cancel')}</Button>
            <Button disabled={isSaving} onClick={clearAll} variant="destructive">{isSaving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <Trash2 aria-hidden="true" size={16} />}{t('personalMemoryClearConfirm')}</Button>
          </div>
        </div>
      </Dialog>

      <Dialog onOpenChange={(open) => { if (!open) setPendingDelete(null); }} open={pendingDelete !== null} title={t('personalMemoryDeleteTitle')}>
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('personalMemoryDeleteConfirmation')}</p>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isSaving} onClick={() => setPendingDelete(null)}>{t('cancel')}</Button>
            <Button disabled={isSaving} onClick={deleteFact} variant="destructive"><Trash2 aria-hidden="true" size={16} />{t('personalMemoryDeleteConfirm')}</Button>
          </div>
        </div>
      </Dialog>
    </Card>
  );
}
