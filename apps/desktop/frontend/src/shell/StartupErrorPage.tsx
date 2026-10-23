import { AlertTriangle, Copy, LoaderCircle, Minus, Square, X } from 'lucide-react';
import { type MouseEvent, useEffect, useLayoutEffect, useState } from 'react';
import english from '@/localization/locales/en.json';
import { Button, Tooltip } from '@/design-system';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { getCurrentWindow, isTauri } from '@/platform/tauri';
import { BrandMark } from './BrandMark';

const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'] as const;
type StartupLocale = (typeof supportedLocales)[number];

type StartupErrorCopy = Pick<
  typeof english,
  | 'startupErrorTitle'
  | 'startupErrorDescription'
  | 'startupErrorDetails'
  | 'startupErrorRetry'
  | 'startupErrorRetrying'
  | 'startupErrorCloseApp'
  | 'startupErrorMinimize'
  | 'startupErrorMaximize'
  | 'startupErrorRestore'
  | 'startupErrorClose'
>;

const startupLocaleLoaders: Record<StartupLocale, () => Promise<StartupErrorCopy>> = {
  de: async () => (await import('@/localization/locales/de.json')).default,
  en: async () => english,
  es: async () => (await import('@/localization/locales/es.json')).default,
  fr: async () => (await import('@/localization/locales/fr.json')).default,
  tr: async () => (await import('@/localization/locales/tr.json')).default,
};

function getStartupCopy(): StartupErrorCopy {
  // English copy is available synchronously if a locale chunk cannot load during startup recovery.
  return english;
}

function readStartupErrorDetails(error: unknown): string | null {
  if (error instanceof Error) {
    const details = 'details' in error && typeof error.details === 'string' ? error.details : error.message;
    return details.trim().slice(0, 1000) || null;
  }
  return typeof error === 'string' ? error.trim().slice(0, 1000) || null : null;
}

function findLocale(value: string | null): StartupLocale | null {
  const language = value?.toLowerCase().split(/[-_]/, 1)[0];
  return supportedLocales.find((locale) => locale === language) ?? null;
}

function getStartupLocale(): StartupLocale {
  let savedLanguage: string | null = null;
  try {
    savedLanguage = unifiedStorage.getItem('openchat.language');
  } catch (cause) {
    globalThis.reportError(new Error('The saved OpenChat language could not be read.', { cause }));
  }

  const savedLocale = savedLanguage === 'system' ? null : findLocale(savedLanguage);
  const locale = savedLocale ?? findLocale(navigator.language) ?? 'en';
  return locale;
}

function getStartupTheme(): 'dark' | 'light' | 'system' {
  try {
    const preference = unifiedStorage.getItem('openchat.theme');
    if (preference === 'dark' || preference === 'light' || preference === 'system') return preference;
  } catch (cause) {
    globalThis.reportError(new Error('The saved OpenChat theme could not be read.', { cause }));
  }
  return 'system';
}

function reportWindowActionError(action: string, cause: unknown): void {
  globalThis.reportError(new Error(`OpenChat could not ${action}.`, { cause }));
}

export interface StartupErrorPageProps {
  error: unknown;
  onRetry(): Promise<void>;
}

export function StartupErrorPage({ error, onRetry }: StartupErrorPageProps) {
  const [locale] = useState(getStartupLocale);
  const [copy, setCopy] = useState(getStartupCopy);
  const desktop = isTauri();
  const [isRetrying, setIsRetrying] = useState(false);
  const [currentError, setCurrentError] = useState(error);
  const errorDetails = readStartupErrorDetails(currentError);
  const [maximized, setMaximized] = useState(false);
  const [systemIsDark, setSystemIsDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  const [themePreference] = useState(getStartupTheme);
  const isDark = themePreference === 'dark' || (themePreference === 'system' && systemIsDark);

  useEffect(() => {
    if (locale === 'en') return;
    let active = true;
    void startupLocaleLoaders[locale]()
      .then((dictionary) => {
        if (active) setCopy(dictionary);
      })
      .catch((cause: unknown) => {
        globalThis.reportError(new Error('The startup locale could not be loaded.', { cause }));
      });
    return () => {
      active = false;
    };
  }, [locale]);
  useLayoutEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
    document.documentElement.lang = locale;
  }, [isDark, locale]);

  useEffect(() => {
    if (themePreference !== 'system') return;
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const updateTheme = (event: MediaQueryListEvent) => setSystemIsDark(event.matches);
    mediaQuery.addEventListener('change', updateTheme);
    return () => mediaQuery.removeEventListener('change', updateTheme);
  }, [themePreference]);

  useEffect(() => {
    globalThis.reportError(
      currentError instanceof Error ? currentError : new Error('OpenChat startup failed.', { cause: currentError }),
    );
  }, [currentError]);

  useEffect(() => {
    if (!desktop) return;
    const currentWindow = getCurrentWindow();
    let active = true;
    let unlistenResize: (() => void) | undefined;

    void currentWindow
      .isMaximized()
      .then((value) => {
        if (active) setMaximized(value);
      })
      .catch((cause: unknown) => reportWindowActionError('read the window state', cause));

    void currentWindow
      .onResized(() => {
        void currentWindow
          .isMaximized()
          .then((value) => {
            if (active) setMaximized(value);
          })
          .catch((cause: unknown) => reportWindowActionError('read the window state', cause));
      })
      .then((unlisten) => {
        if (active) unlistenResize = unlisten;
        else unlisten();
      })
      .catch((cause: unknown) => reportWindowActionError('listen for window changes', cause));

    return () => {
      active = false;
      unlistenResize?.();
    };
  }, [desktop]);

  async function retryStartup(): Promise<void> {
    setIsRetrying(true);
    try {
      await onRetry();
    } catch (cause) {
      setCurrentError(cause);
      setIsRetrying(false);
    }
  }

  function handleTitleBarMouseDown(event: MouseEvent<HTMLElement>): void {
    if (!desktop || event.button !== 0) return;
    if (event.target instanceof Element && event.target.closest('button')) return;

    const currentWindow = getCurrentWindow();
    const action = event.detail === 2 ? currentWindow.toggleMaximize() : currentWindow.startDragging();
    void action.catch((cause: unknown) => reportWindowActionError('move the window', cause));
  }

  function runWindowAction(action: () => Promise<void>, name: string): void {
    void action().catch((cause: unknown) => reportWindowActionError(name, cause));
  }

  return (
    <div className="startup-error-page">
      {/* biome-ignore lint/a11y/noStaticElementInteractions: Tauri uses the native title-bar drag gesture to move this window. */}
      <header className="startup-error-titlebar" onMouseDown={handleTitleBarMouseDown}>
        <div aria-hidden="true" className="startup-error-titlebar__drag-region" />
        {desktop ? (
          <div className="startup-error-titlebar__controls">
            <Tooltip label={copy.startupErrorMinimize}>
              <Button
                aria-label={copy.startupErrorMinimize}
                className="size-8 rounded-control"
                onClick={() => runWindowAction(() => getCurrentWindow().minimize(), 'minimize the window')}
                size="icon"
                variant="ghost"
              >
                <Minus aria-hidden="true" size={12} />
              </Button>
            </Tooltip>
            <Tooltip label={maximized ? copy.startupErrorRestore : copy.startupErrorMaximize}>
              <Button
                aria-label={maximized ? copy.startupErrorRestore : copy.startupErrorMaximize}
                className="size-8 rounded-control"
                onClick={() => runWindowAction(() => getCurrentWindow().toggleMaximize(), 'resize the window')}
                size="icon"
                variant="ghost"
              >
                {maximized ? <Copy aria-hidden="true" size={12} /> : <Square aria-hidden="true" size={12} />}
              </Button>
            </Tooltip>
            <Tooltip label={copy.startupErrorClose}>
              <Button
                aria-label={copy.startupErrorClose}
                className="size-8 rounded-control hover:bg-destructive hover:text-white"
                onClick={() => runWindowAction(() => getCurrentWindow().close(), 'close the window')}
                size="icon"
                variant="ghost"
              >
                <X aria-hidden="true" size={12} />
              </Button>
            </Tooltip>
          </div>
        ) : null}
      </header>

      <main className="startup-error-content" role="alert">
        <section aria-labelledby="startup-error-title" className="startup-error-card">
          <div aria-hidden="true" className="startup-error-card__icon">
            <AlertTriangle size={24} />
          </div>
          <div className="startup-error-card__copy">
            <h1 className="startup-error-card__title" id="startup-error-title">
              {copy.startupErrorTitle}
            </h1>
            <p>{copy.startupErrorDescription}</p>
            {errorDetails ? (
              <p className="startup-error-card__diagnostic">
                <strong>{copy.startupErrorDetails}:</strong> {errorDetails}
              </p>
            ) : null}
          </div>

          <div className="startup-error-card__actions">
            <Button disabled={isRetrying} onClick={() => void retryStartup()} variant="primary">
              <LoaderCircle aria-hidden="true" className={isRetrying ? 'animate-spin' : ''} size={16} />
              {isRetrying ? copy.startupErrorRetrying : copy.startupErrorRetry}
            </Button>
            {desktop ? (
              <Button
                onClick={() => runWindowAction(() => getCurrentWindow().close(), 'close the window')}
                variant="secondary"
              >
                {copy.startupErrorCloseApp}
              </Button>
            ) : null}
          </div>
        </section>
      </main>

      <footer className="startup-error-footer">
        <span>OpenChat</span>
        <BrandMark size="footer" />
      </footer>
    </div>
  );
}
