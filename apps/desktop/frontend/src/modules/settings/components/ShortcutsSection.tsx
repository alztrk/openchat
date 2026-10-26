import { type KeyboardEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, SegmentedControl } from '@/design-system';
import { useComposerPreferencesStore } from '@/modules/conversations/data/composerPreferences';
import {
  type ComposerShortcutAction,
  formatKeyboardShortcut,
  type KeyboardShortcutBinding,
  useKeyboardShortcutsStore,
} from '@/modules/settings/stores/keyboardShortcutsStore';

const actions: ComposerShortcutAction[] = ['sendMessage', 'insertNewLine', 'stopResponse'];
const actionLabels = {
  sendMessage: { title: 'keyboardSendMessage', description: 'keyboardSendMessageDescription' },
  insertNewLine: { title: 'keyboardNewLine', description: 'keyboardNewLineDescription' },
  stopResponse: { title: 'keyboardStopResponse', description: 'keyboardStopResponseDescription' },
} as const;

export function ShortcutsSection() {
  const { t } = useTranslation();
  const [recordingAction, setRecordingAction] = useState<ComposerShortcutAction | null>(null);
  const bindings = useKeyboardShortcutsStore((state) => state.bindings);
  const loadState = useKeyboardShortcutsStore((state) => state.loadState);
  const errorCode = useKeyboardShortcutsStore((state) => state.errorCode);
  const initialize = useKeyboardShortcutsStore((state) => state.initialize);
  const retry = useKeyboardShortcutsStore((state) => state.retry);
  const updateBinding = useKeyboardShortcutsStore((state) => state.updateBinding);
  const reset = useKeyboardShortcutsStore((state) => state.reset);
  const clearError = useKeyboardShortcutsStore((state) => state.clearError);
  const submissionMode = useComposerPreferencesStore((state) => state.submissionMode);
  const setSubmissionMode = useComposerPreferencesStore((state) => state.setSubmissionMode);
  const [submissionSaveFailed, setSubmissionSaveFailed] = useState(false);

  useEffect(() => initialize(), [initialize]);

  const actionTitle = (action: ComposerShortcutAction) => t(actionLabels[action].title);
  const actionDescription = (action: ComposerShortcutAction) => t(actionLabels[action].description);

  const changeSubmissionMode = (value: string) => {
    if (value !== 'enter' && value !== 'ctrlEnter') return;
    try {
      setSubmissionMode(value);
      setSubmissionSaveFailed(false);
    } catch {
      setSubmissionSaveFailed(true);
    }
  };

  const recordKey = (event: KeyboardEvent<HTMLButtonElement>, action: ComposerShortcutAction) => {
    if (recordingAction !== action) return;
    event.preventDefault();
    if (event.key === 'Escape') {
      setRecordingAction(null);
      return;
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;
    if (!/^[A-Za-z][A-Za-z0-9]{0,31}$/.test(event.code)) return;
    const binding: KeyboardShortcutBinding = {
      code: event.code,
      control: event.ctrlKey,
      alt: event.altKey,
      shift: event.shiftKey,
      meta: event.metaKey,
    };
    updateBinding(action, binding);
    setRecordingAction(null);
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      {loadState === 'ready' && errorCode ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorCode)}
        </p>
      ) : null}
      <Card className="divide-y divide-border">
        <div className="@container">
          <div className="appearance-setting-row p-4">
            <div>
              <h3 className="font-medium text-text">{t('composerSendKeyBinding')}</h3>
              <p className="mt-1 text-sm text-secondary-text">{t('composerSendKeyBindingDescription')}</p>
            </div>
            <SegmentedControl
              label={t('composerSendKeyBinding')}
              onValueChange={changeSubmissionMode}
              options={[
                { value: 'enter', label: t('composerSubmissionModeEnter') },
                { value: 'ctrlEnter', label: t('composerSubmissionModeCtrlEnter') },
              ]}
              value={submissionMode}
            />
          </div>
          {submissionSaveFailed ? (
            <p className="px-4 pb-4 text-sm text-destructive" role="alert">
              {t('composerPreferenceSaveFailed')}
            </p>
          ) : null}
        </div>
        {loadState === 'idle' || loadState === 'loading' ? (
          <p aria-live="polite" className="p-4 text-sm text-secondary-text">
            {t('modelsLoading')}
          </p>
        ) : loadState === 'error' ? (
          <div className="flex flex-wrap items-center gap-2 p-4">
            <p className="mr-auto text-sm text-destructive" role="alert">
              {t(errorCode ?? 'keyboardShortcutLoadFailed')}
            </p>
            <Button onClick={retry} size="small" variant="secondary">
              {t('retry')}
            </Button>
            <Button onClick={reset} size="small" variant="ghost">
              {t('keyboardShortcutsReset')}
            </Button>
          </div>
        ) : (
          actions.map((action) => (
            <div className="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between" key={action}>
              <div className="min-w-0">
                <h3 className="font-medium text-text">{actionTitle(action)}</h3>
                <p className="mt-1 text-sm text-secondary-text">{actionDescription(action)}</p>
              </div>
              <Button
                aria-label={`${actionTitle(action)}: ${t(recordingAction === action ? 'keyboardShortcutRecord' : 'edit')}`}
                onClick={() => {
                  clearError();
                  setRecordingAction(recordingAction === action ? null : action);
                }}
                onKeyDown={(event) => recordKey(event, action)}
                size="small"
                variant="secondary"
              >
                {recordingAction === action ? t('keyboardShortcutRecord') : formatKeyboardShortcut(bindings[action])}
              </Button>
            </div>
          ))
        )}
      </Card>
      {loadState === 'ready' ? (
        <div className="flex justify-end">
          <Button
            onClick={() => {
              setRecordingAction(null);
              reset();
            }}
            size="small"
            variant="ghost"
          >
            {t('keyboardShortcutsReset')}
          </Button>
        </div>
      ) : null}
    </section>
  );
}
