import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, Select, Switch } from '@shared/components';
import { providerNameForModel } from '@entities/model/composerUtils';
import { useComposerPreferencesStore } from '@shared/lib/preferences/composerPreferences';
import { useChatStore } from '@features/chat/state';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { TitleGenerationSettingsSection } from './TitleGenerationSettingsSection';

export function AssistantWritingSettingsSection() {
  const { t } = useTranslation();
  const models = useChatStore((state) => state.models);
  const ghostTextEnabled = useComposerPreferencesStore((state) => state.ghostTextEnabled);
  const setGhostTextEnabled = useComposerPreferencesStore((state) => state.setGhostTextEnabled);
  const ghostTextModelKey = useComposerPreferencesStore((state) => state.ghostTextModelKey);
  const setGhostTextModelKey = useComposerPreferencesStore((state) => state.setGhostTextModelKey);
  const [saveFailed, setSaveFailed] = useState(false);

  const modelOptions = useMemo(() => {
    const options = [{ value: 'auto', label: t('composerGhostTextModelAuto') }];
    for (const model of models) {
      options.push({
        value: model.key,
        label: `${providerNameForModel(model, (key) => t(key))}: ${model.displayName}`,
      });
    }
    return options;
  }, [models, t]);

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
              onCheckedChange={changeEnabled}
            />
          </SettingsPreferenceRow>
          {ghostTextEnabled ? (
            <SettingsPreferenceRow description={t('composerGhostTextModelDescription')} label={t('composerGhostTextModel')}>
              <Select
                label={t('composerGhostTextModel')}
                onChange={(event) => changeModel(event.target.value)}
                options={modelOptions}
                value={ghostTextModelKey}
                visuallyHiddenLabel
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
