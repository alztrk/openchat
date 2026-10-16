import { useEffect, useState } from 'react';
import { RefreshCw, RotateCcw, Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { localServiceClient } from '@/features/chat/stores/chatStore';
import { Button } from '@/shared/ui';
import {
  clearSharedInstructions,
  countSharedInstructionsCharacters,
  maximumCustomInstructionsBytes,
  maximumSharedInstructionsCharacters,
  readSharedInstructions,
  SharedInstructionsStorageError,
  writeSharedInstructions,
} from '@/features/settings/data/sharedInstructions';

type LoadState = 'loading' | 'ready' | 'error';
type SettingsErrorKey =
  | 'sharedInstructionsLoadFailed'
  | 'sharedInstructionsSaveFailed'
  | 'sharedInstructionsTooLong'
  | 'sharedInstructionsTooLongBytes';
type FeedbackKey = 'sharedInstructionsSaved' | 'sharedInstructionsRestored';

export function SharedInstructionsSection() {
  const { t } = useTranslation();
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [instructions, setInstructions] = useState('');
  const [savedInstructions, setSavedInstructions] = useState('');
  const [defaultInstructions, setDefaultInstructions] = useState<string | null>(null);
  const [defaultLoadFailed, setDefaultLoadFailed] = useState(false);
  const [errorKey, setErrorKey] = useState<SettingsErrorKey | null>(null);
  const [feedbackKey, setFeedbackKey] = useState<FeedbackKey | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoadState('loading');
    setErrorKey(null);
    setFeedbackKey(null);

    const load = async () => {
      let saved: string | null;
      try {
        saved = readSharedInstructions();
      } catch {
        if (!cancelled) {
          setLoadState('error');
          setErrorKey('sharedInstructionsLoadFailed');
        }
        return;
      }

      let defaults: string | null = null;
      let defaultsFailed = false;
      try {
        const response = await localServiceClient.call('instructions.defaults.get');
        if (typeof response.instructions === 'string' && response.instructions.trim().length > 0) {
          defaults = response.instructions;
        } else {
          defaultsFailed = true;
        }
      } catch {
        defaultsFailed = true;
      }

      if (cancelled) return;
      const initialInstructions = saved && saved.trim().length > 0 ? saved : defaults ?? '';
      setDefaultInstructions(defaults);
      setDefaultLoadFailed(defaultsFailed);
      setInstructions(initialInstructions);
      setSavedInstructions(initialInstructions);
      setLoadState('ready');
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [loadAttempt]);

  const characterCount = countSharedInstructionsCharacters(instructions);
  const tooManyCharacters = characterCount > maximumSharedInstructionsCharacters;
  const tooManyBytes = new TextEncoder().encode(instructions).byteLength > maximumCustomInstructionsBytes;
  const isDirty = instructions !== savedInstructions;

  const save = () => {
    if (loadState !== 'ready' || !isDirty || tooManyCharacters || tooManyBytes) return;
    setErrorKey(null);
    setFeedbackKey(null);
    try {
      writeSharedInstructions(instructions);
      setSavedInstructions(instructions);
      setFeedbackKey('sharedInstructionsSaved');
    } catch (error) {
      if (error instanceof SharedInstructionsStorageError) {
        setErrorKey(error.code === 'too_many_characters'
          ? 'sharedInstructionsTooLong'
          : 'sharedInstructionsTooLongBytes');
      } else {
        setErrorKey('sharedInstructionsSaveFailed');
      }
    }
  };

  const restoreDefaults = () => {
    if (!defaultInstructions) return;
    setErrorKey(null);
    setFeedbackKey(null);
    try {
      clearSharedInstructions();
      setInstructions(defaultInstructions);
      setSavedInstructions(defaultInstructions);
      setFeedbackKey('sharedInstructionsRestored');
    } catch {
      setErrorKey('sharedInstructionsSaveFailed');
    }
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-3.5">
      <p className="text-settings-description text-secondary-text" id="shared-instructions-description">
        {t('sharedInstructionsDescription')}
      </p>
      {loadState === 'loading' ? (
        <progress aria-label={t('sharedInstructionsLoading')} className="h-1 w-full accent-accent" max={100} />
      ) : loadState === 'error' ? (
        <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
          <p className="text-sm text-destructive">{t(errorKey ?? 'sharedInstructionsLoadFailed')}</p>
          <Button
            aria-label={t('retry')}
            onClick={() => setLoadAttempt((attempt) => attempt + 1)}
            size="small"
            variant="secondary"
          >
            <RefreshCw aria-hidden="true" size={16} />
            {t('retry')}
          </Button>
        </div>
      ) : (
        <div className="grid gap-4">
          {defaultLoadFailed ? (
            <p className="text-sm text-warning" role="status">{t('defaultInstructionsLoadFailed')}</p>
          ) : null}
          <div className="grid gap-2">
            <label className="font-medium" htmlFor="shared-instructions-input">{t('sharedInstructions')}</label>
            <textarea
              aria-describedby="shared-instructions-description shared-instructions-count"
              className="min-h-32 max-h-56 w-full resize-y rounded-control border border-border bg-composer px-4 py-3.5 text-base text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-60"
              id="shared-instructions-input"
              maxLength={maximumSharedInstructionsCharacters}
              onChange={(event) => {
                setInstructions(event.target.value);
                setErrorKey(null);
                setFeedbackKey(null);
              }}
              placeholder={t('sharedInstructionsHint')}
              rows={4}
              value={instructions}
            />
          </div>
          {tooManyCharacters ? (
            <p className="text-sm text-destructive" role="alert">{t('sharedInstructionsTooLong')}</p>
          ) : tooManyBytes ? (
            <p className="text-sm text-destructive" role="alert">{t('sharedInstructionsTooLongBytes')}</p>
          ) : null}
          {errorKey ? <p className="text-sm text-destructive" role="alert">{t(errorKey)}</p> : null}
          {feedbackKey ? <p aria-live="polite" className="text-sm text-success" role="status">{t(feedbackKey)}</p> : null}
          <div className="flex flex-wrap items-center justify-between gap-3 px-3">
            <span className="text-xs text-secondary-text" id="shared-instructions-count">
              {characterCount}/{maximumSharedInstructionsCharacters}
            </span>
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                disabled={!defaultInstructions || instructions === defaultInstructions}
                onClick={restoreDefaults}
                variant="ghost"
              >
                <RotateCcw aria-hidden="true" size={16} />
                {t('restoreDefaults')}
              </Button>
              <Button disabled={!isDirty || tooManyCharacters || tooManyBytes} onClick={save} variant="primary">
                <Save aria-hidden="true" size={16} />
                {t('save')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
