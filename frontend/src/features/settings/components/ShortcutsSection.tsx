import { useEffect, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/shared/ui';
import {
  formatKeyboardShortcut,
  useKeyboardShortcutsStore,
  type ComposerShortcutAction,
  type KeyboardShortcutBinding,
} from '@/features/settings/stores/keyboardShortcutsStore';

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

  useEffect(() => initialize(), [initialize]);

  const actionTitle = (action: ComposerShortcutAction) => t(actionLabels[action].title);
  const actionDescription = (action: ComposerShortcutAction) => t(actionLabels[action].description);

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

  if (loadState === 'idle' || loadState === 'loading') {
    return <p aria-live="polite" className="text-sm text-secondary-text">{t('modelsLoading')}</p>;
  }

  if (loadState === 'error') {
    return (
      <section aria-labelledby="keyboard-shortcuts-heading" className="grid gap-4">
        <h2 className="text-xl font-semibold leading-7" id="keyboard-shortcuts-heading">{t('keyboardShortcuts')}</h2>
        <p className="text-sm text-destructive" role="alert">{t(errorCode ?? 'keyboardShortcutLoadFailed')}</p>
        <div className="flex flex-wrap gap-2">
          <Button onClick={retry} size="small" variant="secondary">{t('retry')}</Button>
          <Button onClick={reset} size="small" variant="ghost">{t('keyboardShortcutsReset')}</Button>
        </div>
      </section>
    );
  }

  return (
    <section aria-labelledby="keyboard-shortcuts-heading" className="grid gap-5">
      <div>
        <h2 className="text-xl font-semibold leading-7" id="keyboard-shortcuts-heading">{t('keyboardShortcuts')}</h2>
        <p className="mt-1 text-sm text-secondary-text">{t('keyboardShortcutsDescription')}</p>
      </div>
      {errorCode ? <p className="text-sm text-destructive" role="alert">{t(errorCode)}</p> : null}
      <Card className="divide-y divide-border p-0">
        {actions.map((action) => (
          <div className="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between" key={action}>
            <div className="min-w-0">
              <h3 className="font-medium text-text">{actionTitle(action)}</h3>
              <p className="mt-1 text-sm text-secondary-text">{actionDescription(action)}</p>
            </div>
            <Button
              aria-label={`${actionTitle(action)}: ${t(recordingAction === action ? 'keyboardShortcutRecord' : 'edit')}`}
              disabled={loadState !== 'ready'}
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
        ))}
      </Card>
      <div className="flex justify-end">
        <Button onClick={() => { setRecordingAction(null); reset(); }} size="small" variant="ghost">
          {t('keyboardShortcutsReset')}
        </Button>
      </div>
    </section>
  );
}
