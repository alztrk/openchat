import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, Select, Switch } from '@/design-system';
import { useChatDisplayPreferencesStore } from '@/modules/settings/data/chatDisplayPreferences';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { SettingsLoadError, SettingsSaveFeedback } from './SettingsState';

export function ChatBehaviorSettingsSection() {
  const { t } = useTranslation();
  const preferences = useChatDisplayPreferencesStore((state) => state.preferences);
  const loadState = useChatDisplayPreferencesStore((state) => state.loadState);
  const saveState = useChatDisplayPreferencesStore((state) => state.saveState);
  const initialize = useChatDisplayPreferencesStore((state) => state.initialize);
  const retry = useChatDisplayPreferencesStore((state) => state.retry);
  const setPreference = useChatDisplayPreferencesStore((state) => state.setPreference);
  const reset = useChatDisplayPreferencesStore((state) => state.reset);

  useEffect(() => initialize(), [initialize]);

  if (loadState === 'error') {
    return (
      <SettingsLoadError
        message={t('chatDisplayPreferencesLoadFailed')}
        onRetry={retry}
        onReset={() => void reset()}
        resetLabel={t('chatDisplayPreferencesReset')}
      />
    );
  }

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <Card className="divide-y divide-border">
        <SettingsPreferenceRow
          description={t('autoFollowResponsesDescription')}
          id="setting-auto-follow"
          label={t('autoFollowResponses')}
        >
          {(descriptionId) => (
            <Switch
              aria-label={t('autoFollowResponses')}
              aria-describedby={descriptionId}
              checked={preferences.autoFollowResponses}
              disabled={loadState !== 'ready' || saveState === 'saving'}
              onCheckedChange={(value) => void setPreference('autoFollowResponses', value)}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('wrapCodeBlocksDescription')}
          id="setting-wrap-code"
          label={t('wrapCodeBlocks')}
        >
          {(descriptionId) => (
            <Switch
              aria-label={t('wrapCodeBlocks')}
              aria-describedby={descriptionId}
              checked={preferences.wrapCode}
              disabled={loadState !== 'ready' || saveState === 'saving'}
              onCheckedChange={(value) => void setPreference('wrapCode', value)}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('defaultCopyFormatDescription')}
          id="setting-copy-format"
          label={t('defaultCopyFormat')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={saveState === 'saving'}
              label={t('defaultCopyFormat')}
              onChange={(event) => {
                const value = event.target.value;
                if (value === 'markdown' || value === 'plainText') void setPreference('copyFormat', value);
              }}
              options={[
                { value: 'markdown', label: t('copyCleanMarkdown') },
                { value: 'plainText', label: t('copyPlainText') },
              ]}
              value={preferences.copyFormat}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
      </Card>
      <SettingsSaveFeedback errorMessage={t('chatDisplayPreferencesSaveFailed')} state={saveState} />
    </section>
  );
}
