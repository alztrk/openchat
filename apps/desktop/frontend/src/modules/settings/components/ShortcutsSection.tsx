import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';
import type { ComposerSubmissionMode } from '@/modules/conversations/data/composerPreferences';
import {
  composerShortcutPreset,
  defaultKeyboardShortcuts,
  formatKeyboardShortcut,
  type KeyboardShortcutAction,
  type KeyboardShortcutBinding,
  keyboardShortcutActions,
  sameKeyboardShortcut,
  useKeyboardShortcutsStore,
} from '@/modules/settings/stores/keyboardShortcutsStore';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { SettingsLoadError, SettingsSaveFeedback } from './SettingsState';

const actionLabels = {
  sendMessage: { group: 'composer', title: 'keyboardSendMessage', description: 'keyboardSendMessageDescription' },
  insertNewLine: { group: 'composer', title: 'keyboardNewLine', description: 'keyboardNewLineDescription' },
  stopResponse: { group: 'composer', title: 'keyboardStopResponse', description: 'keyboardStopResponseDescription' },
  newChat: { group: 'navigation', title: 'keyboardNewChat', description: 'keyboardNewChatDescription' },
  openSearch: { group: 'navigation', title: 'keyboardOpenSearch', description: 'keyboardOpenSearchDescription' },
  openModelPicker: {
    group: 'navigation',
    title: 'keyboardOpenModelPicker',
    description: 'keyboardOpenModelPickerDescription',
  },
  openCommandPalette: {
    group: 'navigation',
    title: 'keyboardOpenCommandPalette',
    description: 'keyboardOpenCommandPaletteDescription',
  },
  toggleRightPanel: {
    group: 'navigation',
    title: 'keyboardToggleRightPanel',
    description: 'keyboardToggleRightPanelDescription',
  },
  previousConversation: {
    group: 'navigation',
    title: 'keyboardPreviousConversation',
    description: 'keyboardPreviousConversationDescription',
  },
  nextConversation: {
    group: 'navigation',
    title: 'keyboardNextConversation',
    description: 'keyboardNextConversationDescription',
  },
} as const;
const composerActions = keyboardShortcutActions.filter((action) => actionLabels[action].group === 'composer');
const navigationActions = keyboardShortcutActions.filter((action) => actionLabels[action].group === 'navigation');

type PendingShortcutResolution =
  | {
      kind: 'binding';
      action: KeyboardShortcutAction;
      binding: KeyboardShortcutBinding;
      conflictingActions: KeyboardShortcutAction[];
    }
  | { kind: 'preset'; mode: ComposerSubmissionMode; conflictingActions: KeyboardShortcutAction[] };

export function ShortcutsSection() {
  const { t } = useTranslation();
  const [recordingAction, setRecordingAction] = useState<KeyboardShortcutAction | null>(null);
  const [pendingResolution, setPendingResolution] = useState<PendingShortcutResolution | null>(null);
  const [modifierRequired, setModifierRequired] = useState(false);
  const bindings = useKeyboardShortcutsStore((state) => state.bindings);
  const loadState = useKeyboardShortcutsStore((state) => state.loadState);
  const saveState = useKeyboardShortcutsStore((state) => state.saveState);
  const errorCode = useKeyboardShortcutsStore((state) => state.errorCode);
  const initialize = useKeyboardShortcutsStore((state) => state.initialize);
  const retry = useKeyboardShortcutsStore((state) => state.retry);
  const updateBinding = useKeyboardShortcutsStore((state) => state.updateBinding);
  const resetBinding = useKeyboardShortcutsStore((state) => state.resetBinding);
  const applyComposerPreset = useKeyboardShortcutsStore((state) => state.applyComposerPreset);
  const reset = useKeyboardShortcutsStore((state) => state.reset);
  const clearError = useKeyboardShortcutsStore((state) => state.clearError);
  const preset = composerShortcutPreset(bindings);

  useEffect(() => initialize(), [initialize]);

  useEffect(() => {
    if (recordingAction === null) return;

    const captureShortcut = async (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (
        event.repeat ||
        ['Alt', 'Control', 'Meta', 'Shift', 'OS'].includes(event.key) ||
        [
          'AltLeft',
          'AltRight',
          'ControlLeft',
          'ControlRight',
          'MetaLeft',
          'MetaRight',
          'ShiftLeft',
          'ShiftRight',
        ].includes(event.code)
      ) {
        return;
      }
      if (!/^[A-Za-z][A-Za-z0-9]{0,31}$/.test(event.code)) return;

      const binding: KeyboardShortcutBinding = {
        code: event.code,
        control: event.ctrlKey,
        alt: event.altKey,
        shift: event.shiftKey,
        meta: event.metaKey,
      };
      if (!binding.control && !binding.alt && !binding.meta && !composerActions.includes(recordingAction)) {
        setModifierRequired(true);
        return;
      }
      setModifierRequired(false);
      const result = await updateBinding(recordingAction, binding);
      if (result.status === 'conflict') {
        setPendingResolution({
          kind: 'binding',
          action: recordingAction,
          binding,
          conflictingActions: result.conflictingActions,
        });
      }
      if (result.status === 'requiresModifier') {
        setModifierRequired(true);
        return;
      }
      if (result.status !== 'notReady') setRecordingAction(null);
    };

    const handleKeyDown = (event: KeyboardEvent) => void captureShortcut(event);
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [recordingAction, updateBinding]);

  const actionTitle = (action: KeyboardShortcutAction) => t(actionLabels[action].title);
  const actionDescription = (action: KeyboardShortcutAction) => t(actionLabels[action].description);

  const requestBinding = async (action: KeyboardShortcutAction, binding: KeyboardShortcutBinding | null) => {
    clearError();
    setPendingResolution(null);
    const result = await updateBinding(action, binding);
    if (result.status === 'conflict' && binding !== null) {
      setPendingResolution({ kind: 'binding', action, binding, conflictingActions: result.conflictingActions });
    }
  };

  const requestPreset = async (mode: ComposerSubmissionMode) => {
    clearError();
    setPendingResolution(null);
    const result = await applyComposerPreset(mode);
    if (result.status === 'conflict') {
      setPendingResolution({ kind: 'preset', mode, conflictingActions: result.conflictingActions });
    }
  };

  const resolvePending = async () => {
    if (!pendingResolution) return;
    const result =
      pendingResolution.kind === 'binding'
        ? await updateBinding(pendingResolution.action, pendingResolution.binding, true)
        : await applyComposerPreset(pendingResolution.mode, true);
    if (result.status === 'saved') {
      setPendingResolution(null);
      return;
    }
    if (result.status === 'conflict') {
      setPendingResolution({ ...pendingResolution, conflictingActions: result.conflictingActions });
    }
  };

  const renderAction = (action: KeyboardShortcutAction) => {
    const binding = bindings[action];
    const isRecording = recordingAction === action;
    const pendingForAction = pendingResolution?.kind === 'binding' && pendingResolution.action === action;

    return (
      <div key={action}>
        <SettingsPreferenceRow description={actionDescription(action)} label={actionTitle(action)}>
          {(descriptionId) => (
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                aria-describedby={descriptionId}
                aria-label={`${actionTitle(action)}: ${t(isRecording ? 'keyboardShortcutRecord' : 'edit')}`}
                aria-pressed={isRecording}
                onClick={() => {
                  clearError();
                  setModifierRequired(false);
                  setPendingResolution(null);
                  setRecordingAction((current) => (current === action ? null : action));
                }}
                disabled={saveState === 'saving'}
                size="small"
                variant={isRecording ? 'elevated' : 'secondary'}
              >
                {isRecording
                  ? t('keyboardShortcutRecord')
                  : binding === null
                    ? t('keyboardShortcutUnassigned')
                    : formatKeyboardShortcut(binding)}
              </Button>
              {isRecording ? (
                <Button
                  aria-describedby={descriptionId}
                  onClick={() => setRecordingAction(null)}
                  size="small"
                  variant="ghost"
                >
                  {t('keyboardShortcutCancelRecord')}
                </Button>
              ) : (
                <>
                  <Button
                    aria-describedby={descriptionId}
                    disabled={binding === null}
                    onClick={() => void requestBinding(action, null)}
                    size="small"
                    variant="ghost"
                  >
                    {t('keyboardShortcutClear')}
                  </Button>
                  <Button
                    aria-describedby={descriptionId}
                    disabled={sameKeyboardShortcut(binding, defaultKeyboardShortcuts[action])}
                    onClick={() => {
                      clearError();
                      void resetBinding(action).then((result) => {
                        if (result.status === 'conflict' && defaultKeyboardShortcuts[action] !== null) {
                          setPendingResolution({
                            kind: 'binding',
                            action,
                            binding: defaultKeyboardShortcuts[action],
                            conflictingActions: result.conflictingActions,
                          });
                        }
                      });
                    }}
                    size="small"
                    variant="ghost"
                  >
                    {t('keyboardShortcutResetAction')}
                  </Button>
                </>
              )}
            </div>
          )}
        </SettingsPreferenceRow>
        {isRecording ? (
          <p aria-live="polite" className="sr-only" role="status">
            {t('keyboardShortcutRecordHelp')}
          </p>
        ) : null}
        {isRecording && modifierRequired ? (
          <p className="px-4 pb-2 text-sm text-destructive" role="alert">
            {t('keyboardShortcutModifierRequired')}
          </p>
        ) : null}
        {pendingForAction && pendingResolution.kind === 'binding' ? (
          <ShortcutConflictResolution
            actionNames={pendingResolution.conflictingActions.map(actionTitle)}
            onCancel={() => setPendingResolution(null)}
            onResolve={resolvePending}
          />
        ) : null}
      </div>
    );
  };

  if (loadState === 'idle' || loadState === 'loading') {
    return (
      <p aria-live="polite" className="py-4 text-sm text-secondary-text">
        {t('modelsLoading')}
      </p>
    );
  }

  if (loadState === 'error') {
    return (
      <SettingsLoadError
        message={t(errorCode ?? 'keyboardShortcutLoadFailed')}
        onRetry={retry}
        onReset={() => void reset()}
        resetLabel={t('keyboardShortcutsReset')}
        resetLoading={saveState === 'saving'}
      />
    );
  }

  const allBindingsAreDefault = keyboardShortcutActions.every((action) =>
    sameKeyboardShortcut(bindings[action], defaultKeyboardShortcuts[action]),
  );

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <SettingsSaveFeedback errorMessage={errorCode ? t(errorCode) : undefined} state={saveState} />
      <Card className="divide-y divide-border">
        <SettingsPreferenceRow description={t('keyboardComposerPresetDescription')} label={t('composerSendKeyBinding')}>
          {(descriptionId) => (
            <fieldset
              aria-describedby={descriptionId}
              className="flex min-w-0 flex-wrap justify-end gap-2 border-0 p-0"
            >
              <legend className="sr-only">{t('composerSendKeyBinding')}</legend>
              <Button
                aria-pressed={preset === 'enter'}
                disabled={saveState === 'saving'}
                onClick={() => void requestPreset('enter')}
                size="small"
                variant={preset === 'enter' ? 'elevated' : 'secondary'}
              >
                {t('composerSubmissionModeEnter')}
              </Button>
              <Button
                aria-pressed={preset === 'ctrlEnter'}
                disabled={saveState === 'saving'}
                onClick={() => void requestPreset('ctrlEnter')}
                size="small"
                variant={preset === 'ctrlEnter' ? 'elevated' : 'secondary'}
              >
                {t('composerSubmissionModeCtrlEnter')}
              </Button>
              {preset === 'custom' ? (
                <span aria-live="polite" className="self-center text-sm text-secondary-text" role="status">
                  {t('keyboardShortcutCustomPreset')}
                </span>
              ) : null}
            </fieldset>
          )}
        </SettingsPreferenceRow>
        {pendingResolution?.kind === 'preset' ? (
          <ShortcutConflictResolution
            actionNames={pendingResolution.conflictingActions.map(actionTitle)}
            onCancel={() => setPendingResolution(null)}
            onResolve={resolvePending}
            preset
          />
        ) : null}
        <PreferenceGroupTitle>{t('keyboardShortcutComposerGroup')}</PreferenceGroupTitle>
        {composerActions.map(renderAction)}
      </Card>

      <Card className="divide-y divide-border">
        <PreferenceGroupTitle>{t('keyboardShortcutNavigationGroup')}</PreferenceGroupTitle>
        {navigationActions.map(renderAction)}
      </Card>

      <div className="flex flex-wrap items-center justify-end gap-2">
        <Button
          disabled={allBindingsAreDefault || saveState === 'saving'}
          onClick={() => {
            setRecordingAction(null);
            setPendingResolution(null);
            void reset();
          }}
          size="small"
          variant="ghost"
        >
          {t('keyboardShortcutsReset')}
        </Button>
      </div>
    </section>
  );
}

function PreferenceGroupTitle({ children }: { children: string }) {
  return <h2 className="px-4 pt-4 font-semibold text-text">{children}</h2>;
}

function ShortcutConflictResolution({
  actionNames,
  onCancel,
  onResolve,
  preset = false,
}: {
  actionNames: string[];
  onCancel(): void;
  onResolve(): void;
  preset?: boolean;
}) {
  const { t } = useTranslation();
  const actionList = actionNames.join(', ');

  return (
    <div className="grid gap-2 border-t border-border bg-selected/30 px-4 py-3">
      <p className="text-sm text-text" role="alert">
        {t('keyboardShortcutConflictDetails', { actions: actionList })}
      </p>
      <div className="flex flex-wrap justify-end gap-2">
        <Button onClick={onCancel} size="small" variant="ghost">
          {t('cancel')}
        </Button>
        <Button onClick={onResolve} size="small" variant="secondary">
          {t(preset ? 'keyboardShortcutPresetReplaceConflicts' : 'keyboardShortcutReplaceConflicts')}
        </Button>
      </div>
    </div>
  );
}
