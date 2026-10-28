import { flushSync } from 'react-dom';
import type { Root } from 'react-dom/client';
import { getCurrentWindow, isTauri } from '@/platform/tauri';

function requireRootElement(): HTMLElement {
  const rootElement = document.getElementById('root');
  if (!rootElement) throw new Error('The OpenChat application root element is missing.');
  return rootElement;
}

const rootElement = requireRootElement();
let reactRoot: Root | undefined;
let appStylesPromise: Promise<unknown> | undefined;

function loadAppStyles(): Promise<unknown> {
  appStylesPromise ??= import('../index.css');
  return appStylesPromise;
}

async function getReactRoot(): Promise<[typeof import('react'), Root]> {
  const [React, ReactDOM] = await Promise.all([import('react'), import('react-dom/client')]);
  reactRoot ??= ReactDOM.createRoot(rootElement);
  return [React, reactRoot];
}

function renderAndRevealWindow(render: () => void): void {
  if (!isTauri()) {
    render();
    return;
  }

  flushSync(render);
  void getCurrentWindow()
    .show()
    .then(() => window.dispatchEvent(new Event('openchat-startup-window-visible')))
    .catch((cause: unknown) => {
      globalThis.reportError(new Error('The application window could not be shown.', { cause }));
    });
}

async function startApplication(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
  if (new URLSearchParams(location.search).get('computerUseOverlay') === '1') {
    const [{ ComputerUseOverlay }, [React, root]] = await Promise.all([import('./ComputerUseOverlay'), getReactRoot()]);
    root.render(React.createElement(ComputerUseOverlay));
    return;
  }

  const appStylesPromise = loadAppStyles();
  const appImportPromise = import('./App');
  const storageModulePromise = import('@/platform/storage/unifiedStorage');
  const reactRootPromise = getReactRoot();
  const { unifiedStorage } = await storageModulePromise;
  await unifiedStorage.initialize(options);
  await appStylesPromise;
  const [{ App }, { i18n, i18nReady }, [React, root]] = await Promise.all([
    appImportPromise,
    import('../localization/i18n'),
    reactRootPromise,
  ]);
  await i18nReady;

  document.documentElement.lang = i18n.language;
  i18n.on('languageChanged', (language) => {
    document.documentElement.lang = language;
  });

  renderAndRevealWindow(() => root.render(React.createElement(React.StrictMode, null, React.createElement(App))));
}

void startApplication().catch(async (error: unknown) => {
  try {
    await loadAppStyles();
  } catch (stylesError) {
    globalThis.reportError(new Error('The application styles could not be loaded.', { cause: stylesError }));
  }
  const [{ StartupErrorPage }, [React, root]] = await Promise.all([import('@/shell/StartupErrorPage'), getReactRoot()]);
  renderAndRevealWindow(() =>
    root.render(
      React.createElement(StartupErrorPage, {
        error,
        onRetry: () => startApplication(),
        onStartWithEmptyProfile: () => startApplication({ startWithEmptyStorage: true }),
      }),
    ),
  );
});
