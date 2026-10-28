import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { SegmentedControl, Select } from '@/design-system';
import {
  changeLocalePreference,
  getLocalePreference,
  type LocalePreference,
  type SupportedLocale,
  supportedLocales,
} from '@/localization/i18n';
import { SettingsPreferenceRow } from '@/modules/settings/components/SettingsPreferenceRow';
import { SettingsSaveFeedback, type SettingsSaveState } from '@/modules/settings/components/SettingsState';
import {
  type AppDensityPreference,
  type AppearancePreferences,
  type AppFontPreference,
  type AppScalePreference,
  writeAppDensityPreference,
  writeAppFontPreference,
  writeAppScalePreference,
  writeConversationTextSizePreference,
  writeConversationWidthPreference,
} from '@/modules/settings/data/appearancePreferences';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ThemePreference = 'system' | 'light' | 'dark';

export interface GeneralSettingsSectionProps {
  theme: ThemePreference;
  onThemeChange(theme: ThemePreference): void;
  appearance: AppearancePreferences;
  onAppearanceChange(preferences: Partial<AppearancePreferences>): void;
}

type GeneralSettingsErrorKey = 'appearancePreferenceSaveFailed' | 'themeSaveFailed' | 'languageSaveFailed';

const localeLabelKeys: Record<
  SupportedLocale,
  'germanLanguage' | 'englishLanguage' | 'spanishLanguage' | 'frenchLanguage' | 'turkishLanguage'
> = {
  de: 'germanLanguage',
  en: 'englishLanguage',
  es: 'spanishLanguage',
  fr: 'frenchLanguage',
  tr: 'turkishLanguage',
};

const fontOptions: ReadonlyArray<{ value: AppFontPreference; label: string }> = [
  { value: 'sourceSans3', label: 'Source Sans 3' },
  { value: 'manrope', label: 'Manrope' },
  { value: 'segoeUi', label: 'Segoe UI' },
  { value: 'georgia', label: 'Georgia' },
];

export function GeneralSettingsSection({
  appearance,
  onAppearanceChange,
  onThemeChange,
  theme,
}: GeneralSettingsSectionProps) {
  const { t } = useTranslation();
  const [localePreference, setLocalePreference] = useState<LocalePreference>(getLocalePreference);
  const [saveState, setSaveState] = useState<SettingsSaveState>('idle');
  const [saveError, setSaveError] = useState<GeneralSettingsErrorKey | null>(null);
  const controlsDisabled = saveState === 'saving';

  const saveAppearance = async (write: () => Promise<void>, next: Partial<AppearancePreferences>) => {
    setSaveState('saving');
    setSaveError(null);
    try {
      await write();
      onAppearanceChange(next);
      setSaveState('saved');
    } catch {
      setSaveState('error');
      setSaveError('appearancePreferenceSaveFailed');
    }
  };

  const changeTheme = async (value: string) => {
    if (value !== 'system' && value !== 'light' && value !== 'dark') return;
    setSaveState('saving');
    setSaveError(null);
    try {
      await unifiedStorage.setPersistedItem('openchat.theme', value);
      onThemeChange(value);
      setSaveState('saved');
    } catch {
      setSaveState('error');
      setSaveError('themeSaveFailed');
    }
  };

  const changeLocale = async (preference: LocalePreference) => {
    setSaveState('saving');
    setSaveError(null);
    try {
      await changeLocalePreference(preference);
      setLocalePreference(preference);
      setSaveState('saved');
    } catch {
      setSaveState('error');
      setSaveError('languageSaveFailed');
    }
  };

  const localeOptions = [
    { value: 'system', label: t('systemLanguage') },
    ...supportedLocales.map((locale) => ({ value: locale, label: t(localeLabelKeys[locale]) })),
  ];

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <div className="grid gap-0 rounded-card border border-border bg-raised">
        <SettingsPreferenceRow description={t('themeSettingDescription')} id="setting-theme" label={t('theme')}>
          {(descriptionId) => (
            <SegmentedControl
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('theme')}
              onValueChange={(value) => void changeTheme(value)}
              options={[
                { value: 'system', label: t('systemTheme') },
                { value: 'light', label: t('lightTheme') },
                { value: 'dark', label: t('darkTheme') },
              ]}
              value={theme}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('languageSettingDescription')}
          id="setting-language"
          label={t('language')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('language')}
              onChange={(event) => {
                const value = localeOptions.find((option) => option.value === event.target.value)?.value;
                if (value === 'system') void changeLocale(value);
                else {
                  const locale = supportedLocales.find((candidate) => candidate === value);
                  if (locale) void changeLocale(locale);
                }
              }}
              options={localeOptions}
              value={localePreference}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('conversationWidthDescription')}
          id="setting-conversation-width"
          label={t('conversationWidth')}
        >
          {(descriptionId) => (
            <SegmentedControl
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('conversationWidth')}
              onValueChange={(value) => {
                if (value === 'narrow' || value === 'normal' || value === 'wide') {
                  void saveAppearance(() => writeConversationWidthPreference(value), { conversationWidth: value });
                }
              }}
              options={[
                { value: 'narrow', label: t('widthNarrow') },
                { value: 'normal', label: t('widthNormal') },
                { value: 'wide', label: t('widthWide') },
              ]}
              value={appearance.conversationWidth}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('conversationTextSizeDescription')}
          id="setting-conversation-text-size"
          label={t('conversationTextSize')}
        >
          {(descriptionId) => (
            <SegmentedControl
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('conversationTextSize')}
              onValueChange={(value) => {
                if (value === 'small' || value === 'normal' || value === 'large') {
                  void saveAppearance(() => writeConversationTextSizePreference(value), {
                    conversationTextSize: value,
                  });
                }
              }}
              options={[
                { value: 'small', label: t('textSizeSmall') },
                { value: 'normal', label: t('textSizeNormal') },
                { value: 'large', label: t('textSizeLarge') },
              ]}
              value={appearance.conversationTextSize}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow description={t('appFontDescription')} id="setting-app-font" label={t('appFont')}>
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('appFont')}
              onChange={(event) => {
                const option = fontOptions.find((font) => font.value === event.target.value);
                if (option) void saveAppearance(() => writeAppFontPreference(option.value), { appFont: option.value });
              }}
              options={fontOptions}
              value={appearance.appFont}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow description={t('appScaleDescription')} id="setting-app-scale" label={t('appScale')}>
          {(descriptionId) => (
            <SegmentedControl
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('appScale')}
              onValueChange={(value) => {
                if (value === 'small' || value === 'normal' || value === 'large') {
                  const scale = value satisfies AppScalePreference;
                  void saveAppearance(() => writeAppScalePreference(scale), { appScale: scale });
                }
              }}
              options={[
                { value: 'small', label: t('appScaleSmall') },
                { value: 'normal', label: t('appScaleNormal') },
                { value: 'large', label: t('appScaleLarge') },
              ]}
              value={appearance.appScale}
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('appDensityDescription')}
          id="setting-app-density"
          label={t('appDensity')}
        >
          {(descriptionId) => (
            <SegmentedControl
              aria-describedby={descriptionId}
              disabled={controlsDisabled}
              label={t('appDensity')}
              onValueChange={(value) => {
                if (value === 'compact' || value === 'comfortable') {
                  const density = value satisfies AppDensityPreference;
                  void saveAppearance(() => writeAppDensityPreference(density), { appDensity: density });
                }
              }}
              options={[
                { value: 'comfortable', label: t('appDensityComfortable') },
                { value: 'compact', label: t('appDensityCompact') },
              ]}
              value={appearance.appDensity}
            />
          )}
        </SettingsPreferenceRow>
      </div>
      <SettingsSaveFeedback errorMessage={saveError ? t(saveError) : undefined} state={saveState} />
    </section>
  );
}
