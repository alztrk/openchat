import { useEffect, useLayoutEffect, useState, type MouseEvent } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { AlertTriangle, Copy, LoaderCircle, Minus, Square, X } from 'lucide-react';

import { BrandMark } from './BrandMark';
import { Button, Tooltip } from '@/shared/ui';

const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'] as const;
type StartupLocale = (typeof supportedLocales)[number];

interface StartupErrorCopy {
  title: string;
  description: string;
  retry: string;
  retrying: string;
  closeApp: string;
  minimize: string;
  maximize: string;
  restore: string;
  close: string;
  technicalDetails: string;
}

const startupErrorCopy = {
  de: {
    title: 'OpenChat konnte nicht gestartet werden',
    description: 'Die Startoberfläche oder eine Sprachdatei konnte nicht geladen werden. Versuche es erneut. Falls das Problem bleibt, schließe OpenChat und starte es später noch einmal.',
    retry: 'Erneut versuchen',
    retrying: 'Wird erneut versucht …',
    closeApp: 'OpenChat schließen',
    minimize: 'Minimieren',
    maximize: 'Maximieren',
    restore: 'Wiederherstellen',
    close: 'Schließen',
    technicalDetails: 'Technische Details',
  },
  en: {
    title: 'OpenChat could not start',
    description: 'The startup interface or a language file could not be loaded. Try again. If the problem continues, close OpenChat and start it again later.',
    retry: 'Try again',
    retrying: 'Trying again…',
    closeApp: 'Close OpenChat',
    minimize: 'Minimize',
    maximize: 'Maximize',
    restore: 'Restore',
    close: 'Close',
    technicalDetails: 'Technical details',
  },
  es: {
    title: 'No se pudo iniciar OpenChat',
    description: 'No se pudo cargar la interfaz de inicio o un archivo de idioma. Inténtalo de nuevo. Si el problema continúa, cierra OpenChat y vuelve a abrirlo más tarde.',
    retry: 'Reintentar',
    retrying: 'Reintentando…',
    closeApp: 'Cerrar OpenChat',
    minimize: 'Minimizar',
    maximize: 'Maximizar',
    restore: 'Restaurar',
    close: 'Cerrar',
    technicalDetails: 'Detalles técnicos',
  },
  fr: {
    title: 'OpenChat n’a pas pu démarrer',
    description: 'L’interface de démarrage ou un fichier de langue n’a pas pu être chargé. Réessayez. Si le problème persiste, fermez OpenChat et relancez-le plus tard.',
    retry: 'Réessayer',
    retrying: 'Nouvelle tentative…',
    closeApp: 'Fermer OpenChat',
    minimize: 'Réduire',
    maximize: 'Agrandir',
    restore: 'Restaurer',
    close: 'Fermer',
    technicalDetails: 'Détails techniques',
  },
  tr: {
    title: 'OpenChat başlatılamadı',
    description: 'Başlangıç arayüzü veya bir dil dosyası yüklenemedi. Yeniden deneyin. Sorun sürerse OpenChat’i kapatıp daha sonra tekrar açın.',
    retry: 'Yeniden dene',
    retrying: 'Yeniden deneniyor…',
    closeApp: 'OpenChat’i kapat',
    minimize: 'Küçült',
    maximize: 'Büyüt',
    restore: 'Eski boyuta döndür',
    close: 'Kapat',
    technicalDetails: 'Teknik ayrıntılar',
  },
} satisfies Record<StartupLocale, StartupErrorCopy>;

function findLocale(value: string | null): StartupLocale | null {
  const language = value?.toLowerCase().split(/[-_]/, 1)[0];
  return supportedLocales.find((locale) => locale === language) ?? null;
}

function getStartupLocale(): StartupLocale {
  let savedLanguage: string | null = null;
  try {
    savedLanguage = localStorage.getItem('openchat.language');
  } catch (cause) {
    globalThis.reportError(new Error('The saved OpenChat language could not be read.', { cause }));
  }

  const savedLocale = savedLanguage === 'system' ? null : findLocale(savedLanguage);
  const locale = savedLocale ?? findLocale(navigator.language) ?? 'en';
  return locale;
}

function getStartupTheme(): 'dark' | 'light' | 'system' {
  try {
    const preference = localStorage.getItem('openchat.theme');
    if (preference === 'dark' || preference === 'light' || preference === 'system') return preference;
  } catch (cause) {
    globalThis.reportError(new Error('The saved OpenChat theme could not be read.', { cause }));
  }
  return 'system';
}

function getErrorMessage(error: unknown): string | null {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === 'string' && error.trim()) return error;
  return null;
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
  const copy = startupErrorCopy[locale];
  const desktop = isTauri();
  const [isRetrying, setIsRetrying] = useState(false);
  const [currentError, setCurrentError] = useState(error);
  const [maximized, setMaximized] = useState(false);
  const [systemIsDark, setSystemIsDark] = useState(
    () => window.matchMedia('(prefers-color-scheme: dark)').matches,
  );
  const [themePreference] = useState(getStartupTheme);
  const isDark = themePreference === 'dark' || (themePreference === 'system' && systemIsDark);
  const technicalDetails = getErrorMessage(currentError);

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

    void currentWindow.isMaximized().then((value) => {
      if (active) setMaximized(value);
    }).catch((cause: unknown) => reportWindowActionError('read the window state', cause));

    void currentWindow.onResized(() => {
      void currentWindow.isMaximized().then((value) => {
        if (active) setMaximized(value);
      }).catch((cause: unknown) => reportWindowActionError('read the window state', cause));
    }).then((unlisten) => {
      if (active) unlistenResize = unlisten;
      else unlisten();
    }).catch((cause: unknown) => reportWindowActionError('listen for window changes', cause));

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
      <header className="startup-error-titlebar" onMouseDown={handleTitleBarMouseDown}>
        <div aria-hidden="true" className="startup-error-titlebar__drag-region" />
        {desktop ? (
          <div className="startup-error-titlebar__controls">
            <Tooltip label={copy.minimize}>
              <Button
                aria-label={copy.minimize}
                className="size-8 rounded-control"
                onClick={() => runWindowAction(() => getCurrentWindow().minimize(), 'minimize the window')}
                size="icon"
                variant="ghost"
              >
                <Minus aria-hidden="true" size={12} />
              </Button>
            </Tooltip>
            <Tooltip label={maximized ? copy.restore : copy.maximize}>
              <Button
                aria-label={maximized ? copy.restore : copy.maximize}
                className="size-8 rounded-control"
                onClick={() => runWindowAction(() => getCurrentWindow().toggleMaximize(), 'resize the window')}
                size="icon"
                variant="ghost"
              >
                {maximized ? <Copy aria-hidden="true" size={12} /> : <Square aria-hidden="true" size={12} />}
              </Button>
            </Tooltip>
            <Tooltip label={copy.close}>
              <Button
                aria-label={copy.close}
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
            <h1 className="startup-error-card__title" id="startup-error-title">{copy.title}</h1>
            <p>{copy.description}</p>
          </div>

          {technicalDetails ? (
            <details className="startup-error-details">
              <summary>{copy.technicalDetails}</summary>
              <pre>{technicalDetails}</pre>
            </details>
          ) : null}

          <div className="startup-error-card__actions">
            <Button disabled={isRetrying} onClick={() => void retryStartup()} variant="primary">
              <LoaderCircle aria-hidden="true" className={isRetrying ? 'animate-spin' : ''} size={16} />
              {isRetrying ? copy.retrying : copy.retry}
            </Button>
            {desktop ? (
              <Button
                onClick={() => runWindowAction(() => getCurrentWindow().close(), 'close the window')}
                variant="secondary"
              >
                {copy.closeApp}
              </Button>
            ) : null}
          </div>
        </section>
      </main>

      <footer className="startup-error-footer" aria-label="OpenChat">
        <span>OpenChat</span>
        <BrandMark size="footer" />
      </footer>
    </div>
  );
}
