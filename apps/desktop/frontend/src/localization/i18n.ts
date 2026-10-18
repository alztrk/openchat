import i18n, { type Resource } from 'i18next';
import ICU from 'i18next-icu';
import { initReactI18next } from 'react-i18next';

import type en from './locales/en.json';

export const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'] as const;
export type SupportedLocale = (typeof supportedLocales)[number];
export type LocalePreference = SupportedLocale | 'system';
type LocaleDictionary = typeof en;

const localeLoaders: Record<SupportedLocale, () => Promise<LocaleDictionary>> = {
  de: async () => (await import('./locales/de.json')).default,
  en: async () => (await import('./locales/en.json')).default,
  es: async () => (await import('./locales/es.json')).default,
  fr: async () => (await import('./locales/fr.json')).default,
  tr: async () => (await import('./locales/tr.json')).default,
};

export function getLocalePreference(): LocalePreference {
  try {
    const preferred = localStorage.getItem('openchat.language');
    const savedLocale = supportedLocales.find((locale) => locale === preferred);
    if (savedLocale) return savedLocale;
  } catch {
    return 'system';
  }
  return 'system';
}

function getSystemLocale(): SupportedLocale {
  const language = new Intl.Locale(navigator.language).language;
  return supportedLocales.find((locale) => locale === language) ?? 'en';
}

function getInitialLocale(): SupportedLocale {
  const preference = getLocalePreference();
  return preference === 'system' ? getSystemLocale() : preference;
}

async function initializeI18n(): Promise<void> {
  const locale = getInitialLocale();
  const resources: Resource = {
    [locale]: { translation: await localeLoaders[locale]() },
  };
  await i18n
    .use(ICU)
    .use(initReactI18next)
    .init({
      lng: locale,
      fallbackLng: 'en',
      supportedLngs: [...supportedLocales],
      resources,
      interpolation: { escapeValue: false },
      returnNull: false,
    });
}

export const i18nReady = initializeI18n();

export async function changeSupportedLocale(locale: SupportedLocale): Promise<void> {
  await i18nReady;
  if (!i18n.hasResourceBundle(locale, 'translation')) {
    i18n.addResourceBundle(locale, 'translation', await localeLoaders[locale](), true, true);
  }
  await i18n.changeLanguage(locale);
}

export async function changeLocalePreference(preference: LocalePreference): Promise<void> {
  await i18nReady;
  if (preference === 'system') {
    localStorage.removeItem('openchat.language');
    await changeSupportedLocale(getSystemLocale());
    return;
  }
  localStorage.setItem('openchat.language', preference);
  await changeSupportedLocale(preference);
}

export { i18n };
