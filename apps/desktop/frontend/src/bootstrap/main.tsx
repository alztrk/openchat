import React from 'react';
import ReactDOM from 'react-dom/client';
import '../index.css';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { StartupErrorPage } from '@/shell/StartupErrorPage';

const rootElement = document.getElementById('root');

const startupMessages: Readonly<Record<string, string>> = {
  de: 'OpenChat wird gestartet…',
  en: 'Starting OpenChat…',
  es: 'Iniciando OpenChat…',
  fr: 'OpenChat démarre…',
  tr: 'OpenChat başlatılıyor…',
};

if (!rootElement) {
  throw new Error('The OpenChat application root element is missing.');
}
const reactRoot = ReactDOM.createRoot(rootElement);

function StartupLoadingPage() {
  const language = navigator.language.toLowerCase().split(/[-_]/, 1)[0] ?? 'en';
  const message = startupMessages[language] ?? startupMessages.en ?? 'Starting OpenChat…';

  return (
    <main aria-live="polite" className="startup-loading-page" role="status">
      <span className="startup-loading-page__brand">OpenChat</span>
      <span className="startup-loading-page__message">{message}</span>
      <span aria-hidden="true" className="startup-loading-page__indicator" />
    </main>
  );
}

async function startApplication(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
  if (new URLSearchParams(location.search).get('computerUseOverlay') === '1') {
    const { ComputerUseOverlay } = await import('./ComputerUseOverlay');
    reactRoot.render(<ComputerUseOverlay />);
    return;
  }

  reactRoot.render(<StartupLoadingPage />);

  const appImportPromise = import('./App');
  await unifiedStorage.initialize(options);
  const [{ App }, { i18n, i18nReady }] = await Promise.all([appImportPromise, import('../localization/i18n')]);
  await i18nReady;

  document.documentElement.lang = i18n.language;
  i18n.on('languageChanged', (language) => {
    document.documentElement.lang = language;
  });

  reactRoot.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void startApplication().catch((error: unknown) => {
  reactRoot.render(
    <StartupErrorPage
      error={error}
      onRetry={() => startApplication()}
      onStartWithEmptyProfile={() => startApplication({ startWithEmptyStorage: true })}
    />,
  );
});
