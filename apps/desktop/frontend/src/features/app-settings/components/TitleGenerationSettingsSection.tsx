import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useChatStore } from '@features/chat/state/chatStore';
import {
  findLightweightModel,
  TitleGenerationSettings,
} from '@features/provider-settings/components/ProviderSettingsSection';
import { formatModelRouteKey } from '@features/provider-settings/data/modelPreferences';
import {
  readTitleGenerationPreference,
  type TitleGenerationPreference,
  titlePreferenceForModel,
  writeTitleGenerationPreference,
} from '@features/provider-settings/data/titleGenerationPreferences';

export function TitleGenerationSettingsSection() {
  const { t } = useTranslation();
  const models = useChatStore((state) => state.models);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const [preference, setPreference] = useState<TitleGenerationPreference>({ mode: 'conversation', model: null });
  const [preferenceLoadFailed, setPreferenceLoadFailed] = useState(false);
  const [error, setError] = useState<'titlePreferenceSaveFailed' | null>(null);
  const [saving, setSaving] = useState(false);

  const loadPreference = useCallback(() => {
    try {
      setPreference(readTitleGenerationPreference());
      setPreferenceLoadFailed(false);
    } catch {
      setPreferenceLoadFailed(true);
    }
  }, []);

  useEffect(() => loadPreference(), [loadPreference]);

  const savePreference = (nextPreference: TitleGenerationPreference) => {
    setSaving(true);
    try {
      writeTitleGenerationPreference(nextPreference);
      setPreference(nextPreference);
      setPreferenceLoadFailed(false);
      setError(null);
    } catch {
      setError('titlePreferenceSaveFailed');
    } finally {
      setSaving(false);
    }
  };

  useEffect(() => {
    if (preference.mode !== 'lightweight') return;
    const lightweightModel = findLightweightModel(models);
    const nextModel = lightweightModel ? titlePreferenceForModel(lightweightModel) : null;
    const currentRoute = preference.model ? formatModelRouteKey(preference.model) : null;
    const nextRoute = nextModel ? formatModelRouteKey(nextModel) : null;
    if (currentRoute === nextRoute) return;
    const nextPreference: TitleGenerationPreference = { mode: 'lightweight', model: nextModel };
    try {
      writeTitleGenerationPreference(nextPreference);
      setPreference(nextPreference);
      setPreferenceLoadFailed(false);
      setError(null);
    } catch {
      setError('titlePreferenceSaveFailed');
    }
  }, [models, preference.mode, preference.model]);

  return (
    <section aria-labelledby="title-generation-heading" className="settings-data-group">
      <div>
        <h2 className="settings-data-group-title" id="title-generation-heading">
          {t('titleGenerationTarget')}
        </h2>
        <p className="mt-1 text-sm text-secondary-text">{t('titleGenerationTargetDescription')}</p>
      </div>
      <TitleGenerationSettings
        error={error}
        modelError={modelError}
        modelStatus={modelStatus}
        models={models}
        onChange={savePreference}
        onPreferenceRetry={loadPreference}
        onRetry={() => void refreshModels(true)}
        preference={preference}
        preferenceLoadFailed={preferenceLoadFailed}
        saving={saving}
      />
    </section>
  );
}
