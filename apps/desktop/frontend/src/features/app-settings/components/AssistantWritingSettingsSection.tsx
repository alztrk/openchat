import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, Switch } from '@shared/components';
import { useComposerPreferencesStore } from '@shared/lib/preferences/composerPreferences';
import { useChatStore } from '@features/chat/state';
import { ModelPickerField } from '@features/chat/components';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { TitleGenerationSettingsSection } from './TitleGenerationSettingsSection';

export function AssistantWritingSettingsSection() {
  const { t } = useTranslation();
  const models = useChatStore((state) => state.models);
  const ghostTextEnabled = useComposerPreferencesStore((state) => state.ghostTextEnabled);
  const loadState = useComposerPreferencesStore((state) => state.loadState);
  const setGhostTextEnabled = useComposerPreferencesStore((state) => state.setGhostTextEnabled);
  const ghostTextModelKey = useComposerPreferencesStore((state) => state.ghostTextModelKey);
  const setGhostTextModelKey = useComposerPreferencesStore((state) => state.setGhostTextModelKey);
  const [saveFailed, setSaveFailed] = useState(false);

  const selectedGhostTextModel = models.find((model) => model.key === ghostTextModelKey) ?? null;

  const changeEnabled = async (enabled: boolean) => {
    try {
      await setGhostTextEnabled(enabled);
      setSaveFailed(false);
    } catch {
      setSaveFailed(true);
    }
  };

  const changeModel = (modelKey: string) => {
    try {
      setGhostTextModelKey(modelKey);
      setSaveFailed(false);
    } catch {
      setSaveFailed(true);
    }
  };

  return (
    <div className="settings-data-page">
      <div className="settings-data-group">
        <h2 className="settings-data-group-title">{t('composerGhostTextEnabled')}</h2>
        <Card className="grid gap-4" padding="default">
          <SettingsPreferenceRow
            description={t('composerGhostTextEnabledDescription')}
            label={t('composerGhostTextEnabled')}
          >
            <Switch
              aria-label={t('composerGhostTextEnabled')}
              checked={ghostTextEnabled}
              disabled={loadState !== 'ready'}
              onCheckedChange={changeEnabled}
            />
          </SettingsPreferenceRow>
          {ghostTextEnabled ? (
            <SettingsPreferenceRow description={t('composerGhostTextModelDescription')} label={t('composerGhostTextModel')}>
              <ModelPickerField
                disabled={loadState !== 'ready'}
                emptySelectionLabel={
                  ghostTextModelKey === 'auto' ? t('composerGhostTextModelAuto') : t('modelPickerSelectModel')
                }
                emptyOptionLabel={t('composerGhostTextModelAuto')}
                onSelect={(model) => changeModel(model.key)}
                onSelectEmpty={() => changeModel('auto')}
                selectedModel={selectedGhostTextModel}
                triggerAriaLabel={t('composerGhostTextModel')}
              />
            </SettingsPreferenceRow>
          ) : null}
          {saveFailed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('composerPreferenceSaveFailed')}
            </p>
          ) : null}
        </Card>
      </div>
      <TitleGenerationSettingsSection />
    </div>
  );
}
