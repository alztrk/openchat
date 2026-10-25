import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { SegmentedControl, Select } from '@/design-system';
import {
  changeLocalePreference,
  getLocalePreference,
  type LocalePreference,
  type SupportedLocale,
  supportedLocales,
} from '@/localization/i18n';
import { useComposerPreferencesStore } from '@/modules/conversations/data/composerPreferences';
import {
  type AppearancePreferences,
  type AppFontPreference,
  writeAppFontPreference,
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
  const [saveError, setSaveError] = useState<'language' | 'theme' | 'appearance' | 'composer' | null>(null);
  const submissionMode = useComposerPreferencesStore((state) => state.submissionMode);
  const setSubmissionMode = useComposerPreferencesStore((state) => state.setSubmissionMode);

  const changeLocale = async (preference: LocalePreference) => {
    try {
      await changeLocalePreference(preference);
      setLocalePreference(preference);
      setSaveError(null);
    } catch {
      setSaveError('language');
    }
  };

  const changeTheme = (value: string) => {
    if (value !== 'system' && value !== 'light' && value !== 'dark') return;
    try {
      unifiedStorage.setItem('openchat.theme', value);
      onThemeChange(value);
      setSaveError(null);
    } catch {
      setSaveError('theme');
    }
  };

  const changeConversationWidth = (value: string) => {
    if (value !== 'narrow' && value !== 'normal' && value !== 'wide') return;
    try {
      writeConversationWidthPreference(value);
      onAppearanceChange({ conversationWidth: value });
      setSaveError(null);
    } catch {
      setSaveError('appearance');
    }
  };

  const changeConversationTextSize = (value: string) => {
    if (value !== 'small' && value !== 'normal' && value !== 'large') return;
    try {
      writeConversationTextSizePreference(value);
      onAppearanceChange({ conversationTextSize: value });
      setSaveError(null);
    } catch {
      setSaveError('appearance');
    }
  };

  const changeAppFont = (value: string) => {
    const option = fontOptions.find((font) => font.value === value);
    if (!option) return;
    try {
      writeAppFontPreference(option.value);
      onAppearanceChange({ appFont: option.value });
      setSaveError(null);
    } catch {
      setSaveError('appearance');
    }
  };

  const changeSubmissionMode = (value: string) => {
    if (value !== 'enter' && value !== 'ctrlEnter') return;
    try {
      setSubmissionMode(value);
      setSaveError(null);
    } catch {
      setSaveError('composer');
    }
  };

  const localeOptions = [
    { value: 'system', label: t('systemLanguage') },
    ...supportedLocales.map((locale) => ({ value: locale, label: t(localeLabelKeys[locale]) })),
  ];
  const errorKey =
    saveError === 'language'
      ? 'languageSaveFailed'
      : saveError === 'theme'
        ? 'themeSaveFailed'
        : saveError === 'composer'
          ? 'composerPreferenceSaveFailed'
          : 'appearancePreferenceSaveFailed';

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <SettingRow description={t('themeSettingDescription')} label={t('theme')}>
        <SegmentedControl
          label={t('theme')}
          onValueChange={changeTheme}
          options={[
            { value: 'system', label: t('systemTheme') },
            { value: 'light', label: t('lightTheme') },
            { value: 'dark', label: t('darkTheme') },
          ]}
          value={theme}
        />
      </SettingRow>
      <SettingRow description={t('languageSettingDescription')} label={t('language')}>
        <Select
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
      </SettingRow>
      <SettingRow description={t('conversationWidthDescription')} label={t('conversationWidth')}>
        <SegmentedControl
          label={t('conversationWidth')}
          onValueChange={changeConversationWidth}
          options={[
            { value: 'narrow', label: t('widthNarrow') },
            { value: 'normal', label: t('widthNormal') },
            { value: 'wide', label: t('widthWide') },
          ]}
          value={appearance.conversationWidth}
        />
      </SettingRow>
      <SettingRow description={t('conversationTextSizeDescription')} label={t('conversationTextSize')}>
        <SegmentedControl
          label={t('conversationTextSize')}
          onValueChange={changeConversationTextSize}
          options={[
            { value: 'small', label: t('textSizeSmall') },
            { value: 'normal', label: t('textSizeNormal') },
            { value: 'large', label: t('textSizeLarge') },
          ]}
          value={appearance.conversationTextSize}
        />
      </SettingRow>
      <SettingRow description={t('appFontDescription')} label={t('appFont')}>
        <Select
          label={t('appFont')}
          onChange={(event) => changeAppFont(event.target.value)}
          options={fontOptions}
          value={appearance.appFont}
          visuallyHiddenLabel
        />
      </SettingRow>
      <SettingRow description={t('composerSendKeyBindingDescription')} label={t('composerSendKeyBinding')}>
        <SegmentedControl
          label={t('composerSendKeyBinding')}
          onValueChange={changeSubmissionMode}
          options={[
            { value: 'enter', label: t('composerSubmissionModeEnter') },
            { value: 'ctrlEnter', label: t('composerSubmissionModeCtrlEnter') },
          ]}
          value={submissionMode}
        />
      </SettingRow>
      {saveError ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
    </section>
  );
}

function SettingRow({ children, description, label }: { children: ReactNode; description: string; label: string }) {
  return (
    <div className="@container">
      <div className="appearance-setting-row">
        <div>
          <h3 className="font-medium">{label}</h3>
          <p className="mt-1 text-sm text-secondary-text">{description}</p>
        </div>
        <div className="min-w-0">{children}</div>
      </div>
    </div>
  );
}
