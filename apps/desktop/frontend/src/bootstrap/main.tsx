import type { Root } from 'react-dom/client';

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

async function startApplication(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
  if (new URLSearchParams(location.search).get('computerUseOverlay') === '1') {
    const [{ ComputerUseOverlay }, [React, root]] = await Promise.all([
      import('./ComputerUseOverlay'),
      getReactRoot(),
    ]);
    root.render(React.createElement(ComputerUseOverlay));
    return;
  }

  const appStylesPromise = loadAppStyles();
  const appImportPromise = import('./App');
  const storageModulePromise = import('@/platform/storage/unifiedStorage');
  const { unifiedStorage } = await storageModulePromise;
  await unifiedStorage.initialize(options);
  await appStylesPromise;
  const [{ App }, { i18n, i18nReady }, [React, root]] = await Promise.all([
    appImportPromise,
    import('../localization/i18n'),
    getReactRoot(),
  ]);
  await i18nReady;

  document.documentElement.lang = i18n.language;
  i18n.on('languageChanged', (language) => {
    document.documentElement.lang = language;
  });

  root.render(React.createElement(React.StrictMode, null, React.createElement(App)));
}

void startApplication().catch(async (error: unknown) => {
  try {
    await loadAppStyles();
  } catch (stylesError) {
    globalThis.reportError(new Error('The application styles could not be loaded.', { cause: stylesError }));
  }
  const [{ StartupErrorPage }, [React, root]] = await Promise.all([
    import('@/shell/StartupErrorPage'),
    getReactRoot(),
  ]);
  root.render(
    React.createElement(StartupErrorPage, {
      error,
      onRetry: () => startApplication(),
      onStartWithEmptyProfile: () => startApplication({ startWithEmptyStorage: true }),
    }),
  );
});
