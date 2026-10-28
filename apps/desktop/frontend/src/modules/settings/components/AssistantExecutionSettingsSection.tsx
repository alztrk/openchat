import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Select, Switch } from '@/design-system';
import { providerNameForModel } from '@/modules/conversations/components/composerUtils';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { useAssistantExecutionPreferencesStore } from '@/modules/settings/data/assistantExecutionPreferences';
import { useAssistantProfilesStore } from '@/modules/settings/stores/assistantProfilesStore';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { SettingsLoadError, SettingsSaveFeedback } from './SettingsState';

const goalRuntimeChoices = [0, 30, 60, 120, 240] as const;

export function AssistantExecutionSettingsSection() {
  const { t } = useTranslation();
  const preferences = useAssistantExecutionPreferencesStore((state) => state.preferences);
  const preferenceLoadState = useAssistantExecutionPreferencesStore((state) => state.loadState);
  const preferenceSaveState = useAssistantExecutionPreferencesStore((state) => state.saveState);
  const initializePreferences = useAssistantExecutionPreferencesStore((state) => state.initialize);
  const retryPreferences = useAssistantExecutionPreferencesStore((state) => state.retry);
  const setPreference = useAssistantExecutionPreferencesStore((state) => state.setPreference);
  const resetPreferences = useAssistantExecutionPreferencesStore((state) => state.reset);
  const profiles = useAssistantProfilesStore((state) => state.profiles);
  const profilesLoadState = useAssistantProfilesStore((state) => state.loadState);
  const profilesErrorKey = useAssistantProfilesStore((state) => state.errorKey);
  const initializeProfiles = useAssistantProfilesStore((state) => state.initialize);
  const retryProfiles = useAssistantProfilesStore((state) => state.retry);
  const models = useChatStore((state) => state.models);

  useEffect(() => {
    initializePreferences();
    initializeProfiles();
  }, [initializePreferences, initializeProfiles]);

  if (preferenceLoadState === 'error') {
    return (
      <SettingsLoadError
        message={t('assistantPreferencesLoadFailed')}
        onRetry={retryPreferences}
        onReset={() => void resetPreferences()}
        resetLabel={t('assistantPreferencesReset')}
        resetLoading={preferenceSaveState === 'saving'}
      />
    );
  }

  const profileOptions = profiles.map((profile) => ({ value: profile.id, label: profile.name }));
  if (
    preferences.defaultProfileId !== null &&
    !profileOptions.some((option) => option.value === preferences.defaultProfileId)
  ) {
    profileOptions.unshift({ value: preferences.defaultProfileId, label: t('assistantProfileDefaultUnavailable') });
  }

  const modelOptions = models.map((model) => ({
    value: modelRouteKey(model),
    label: `${providerNameForModel(model, (key) => t(key))}: ${model.displayName}`,
  }));
  if (
    preferences.preferredDelegationModelRouteKey !== null &&
    !modelOptions.some((option) => option.value === preferences.preferredDelegationModelRouteKey)
  ) {
    modelOptions.unshift({
      value: preferences.preferredDelegationModelRouteKey,
      label: t('delegationModelUnavailable'),
    });
  }

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      {profilesLoadState === 'error' ? (
        <div
          className="flex flex-wrap items-center justify-between gap-3 rounded-control border border-destructive p-3"
          role="alert"
        >
          <p className="text-sm text-destructive">{t(profilesErrorKey ?? 'assistantProfilesLoadFailed')}</p>
          <Button onClick={retryProfiles} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : null}
      <Card className="divide-y divide-border">
        <SettingsPreferenceRow
          description={t('defaultAssistantProfileDescription')}
          id="setting-default-assistant-profile"
          label={t('defaultAssistantProfile')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={
                preferenceLoadState !== 'ready' || profilesLoadState !== 'ready' || preferenceSaveState === 'saving'
              }
              label={t('defaultAssistantProfile')}
              onChange={(event) => void setPreference('defaultProfileId', event.target.value || null)}
              options={[{ value: '', label: t('noDefaultAssistantProfile') }, ...profileOptions]}
              value={preferences.defaultProfileId ?? ''}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('preferredDelegationModelDescription')}
          id="setting-preferred-delegation-model"
          label={t('preferredDelegationModel')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={preferenceLoadState !== 'ready' || preferenceSaveState === 'saving'}
              label={t('preferredDelegationModel')}
              onChange={(event) => void setPreference('preferredDelegationModelRouteKey', event.target.value || null)}
              options={[{ value: '', label: t('noPreferredDelegationModel') }, ...modelOptions]}
              value={preferences.preferredDelegationModelRouteKey ?? ''}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('includeProjectSkillsDescription')}
          id="setting-include-project-skills"
          label={t('includeProjectSkills')}
        >
          {(descriptionId) => (
            <Switch
              aria-label={t('includeProjectSkills')}
              aria-describedby={descriptionId}
              checked={preferences.includeProjectSkills}
              disabled={preferenceLoadState !== 'ready' || preferenceSaveState === 'saving'}
              onCheckedChange={(value) => void setPreference('includeProjectSkills', value)}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('maxGoalRuntimeDescription')}
          id="setting-max-goal-runtime"
          label={t('maxGoalRuntime')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={preferenceLoadState !== 'ready' || preferenceSaveState === 'saving'}
              label={t('maxGoalRuntime')}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (isGoalRuntimeChoice(value)) void setPreference('maxGoalRuntimeMinutes', value);
              }}
              options={goalRuntimeChoices.map((minutes) => ({
                value: String(minutes),
                label: minutes === 0 ? t('goalRuntimeUnlimited') : t('durationMinutes', { count: minutes }),
              }))}
              value={String(preferences.maxGoalRuntimeMinutes)}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
      </Card>
      <SettingsSaveFeedback errorMessage={t('assistantPreferencesSaveFailed')} state={preferenceSaveState} />
    </section>
  );
}

function isGoalRuntimeChoice(value: number): value is (typeof goalRuntimeChoices)[number] {
  return goalRuntimeChoices.some((choice) => choice === value);
}
