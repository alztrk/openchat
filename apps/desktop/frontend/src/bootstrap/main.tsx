import { flushSync } from 'react-dom';
import type { Root } from 'react-dom/client';
import { getCurrentWindow, invoke, isTauri } from '@shared/lib/platform/tauri';

declare global {
  interface Window {
    __OPENCHAT_STARTUP_BENCHMARK__?: {
      mark: (milestone: string) => void;
      reportWindowVisible: () => Promise<void>;
    };
  }
}

function markStartupMilestone(milestone: string): void {
  window.__OPENCHAT_STARTUP_BENCHMARK__?.mark(milestone);
}

markStartupMilestone('bootstrap-module-evaluated');

function requireRootElement(): HTMLElement {
  const rootElement = document.getElementById('root');
  if (!rootElement) throw new Error('The OpenChat application root element is missing.');
  return rootElement;
}

const rootElement = requireRootElement();
let reactRoot: Root | undefined;
let appStylesPromise: Promise<unknown> | undefined;
let startupWindowShowPromise: Promise<void> | undefined;
let startupWindowShowFailureReported = false;
let languageChangeListenerRegistered = false;

function loadAppStyles(): Promise<unknown> {
  if (!appStylesPromise) {
    markStartupMilestone('styles-import-start');
    appStylesPromise = import('../index.css').then(() => {
      markStartupMilestone('styles-ready');
    });
  }
  return appStylesPromise;
}

function showStartupWindow(): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  startupWindowShowPromise ??= getCurrentWindow()
    .show()
    .then(async () => {
      markStartupMilestone('startup-window-show-complete');
      const reportWindowVisible = window.__OPENCHAT_STARTUP_BENCHMARK__?.reportWindowVisible;
      if (reportWindowVisible) await reportWindowVisible();
    });
  return startupWindowShowPromise;
}

async function reportRendererReady(language: string): Promise<void> {
  try {
    await invoke('report_webview_renderer_ready', { language, appUrl: window.location.href });
  } catch (cause: unknown) {
    globalThis.reportError(new Error('The desktop host could not update renderer recovery state.', { cause }));
  }
}

async function getReactRoot(): Promise<[typeof import('react'), Root]> {
  markStartupMilestone('react-modules-import-start');
  const [React, ReactDOM] = await Promise.all([import('react'), import('react-dom/client')]);
  markStartupMilestone('react-modules-ready');
  reactRoot ??= ReactDOM.createRoot(rootElement);
  markStartupMilestone('react-root-created');
  return [React, reactRoot];
}

function renderAndRevealWindow(render: () => void): void {
  if (!isTauri()) {
    render();
    return;
  }

  flushSync(render);
  markStartupMilestone('react-first-commit-complete');
  markStartupMilestone('window-show-requested');
  void showStartupWindow()
    .then(() => {
      markStartupMilestone('window-show-complete');
      return reportRendererReady(document.documentElement.lang);
    })
    .then(() => window.dispatchEvent(new Event('openchat-startup-window-visible')))
    .catch((cause: unknown) => {
      if (startupWindowShowFailureReported) return;
      globalThis.reportError(new Error('The application window could not be shown.', { cause }));
    });
}

async function startApplication(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
  markStartupMilestone('application-bootstrap-start');
  if (!isTauri()) {
    const [, { DesktopRequiredPage }, i18nModule, [React, root]] = await Promise.all([
      loadAppStyles(),
      import('./DesktopRequiredPage'),
      import('@shared/lib/i18n/i18n'),
      getReactRoot(),
    ]);
    await i18nModule.i18nReady;
    document.documentElement.lang = i18nModule.i18n.language;
    root.render(React.createElement(DesktopRequiredPage));
    return;
  }

  if (new URLSearchParams(location.search).get('computerUseOverlay') === '1') {
    const [{ ComputerUseOverlay }, [React, root]] = await Promise.all([import('./ComputerUseOverlay'), getReactRoot()]);
    root.render(React.createElement(ComputerUseOverlay));
    return;
  }

  markStartupMilestone('startup-window-show-requested');
  void showStartupWindow().catch((cause: unknown) => {
    startupWindowShowFailureReported = true;
    globalThis.reportError(new Error('The application window could not be shown.', { cause }));
  });

  const appStylesPromise = loadAppStyles();
  markStartupMilestone('storage-module-import-start');
  const storageModulePromise = import('@shared/lib/platform/storage/unifiedStorage').then((module) => {
    markStartupMilestone('storage-module-ready');
    return module;
  });
  markStartupMilestone('i18n-module-import-start');
  const i18nModulePromise = import('@shared/lib/i18n/i18n').then((module) => {
    markStartupMilestone('i18n-module-ready');
    return module;
  });
  const reactRootPromise = getReactRoot();
  markStartupMilestone('app-module-import-start');
  const appImportPromise = import('./App').then((module) => {
    markStartupMilestone('app-module-ready');
    return module;
  });
  const errorBoundaryPromise = import('@widgets/application-error').then((module) => {
    markStartupMilestone('error-boundary-module-ready');
    return module;
  });
  const { unifiedStorage } = await storageModulePromise;
  markStartupMilestone('storage-initialize-start');
  const storageInitialization = unifiedStorage.initialize(options).then(() => {
    markStartupMilestone('storage-initialize-ready');
  });
  void storageInitialization.catch(() => undefined);
  await appStylesPromise;
  const [{ App }, { ApplicationErrorBoundary }, i18nModule, [React, root]] = await Promise.all([
    appImportPromise,
    errorBoundaryPromise,
    i18nModulePromise,
    reactRootPromise,
  ]);
  await i18nModule.i18nReady;
  markStartupMilestone('i18n-ready');

  document.documentElement.lang = i18nModule.i18n.language;
  if (!languageChangeListenerRegistered) {
    i18nModule.i18n.on('languageChanged', (language) => {
      document.documentElement.lang = language;
      if (isTauri()) void reportRendererReady(language);
    });
    languageChangeListenerRegistered = true;
  }

  markStartupMilestone('react-first-render-start');
  renderAndRevealWindow(() =>
    root.render(
      React.createElement(
        React.StrictMode,
        null,
        React.createElement(ApplicationErrorBoundary, null, React.createElement(App)),
      ),
    ),
  );

  await storageInitialization;
  await i18nModule.changeSupportedLocale(i18nModule.getInitialLocale());
  window.dispatchEvent(new Event('openchat-storage-ready'));
}

void startApplication().catch(async (error: unknown) => {
  try {
    await loadAppStyles();
  } catch (stylesError) {
    globalThis.reportError(new Error('The application styles could not be loaded.', { cause: stylesError }));
  }
  const [{ StartupErrorPage }, [React, root]] = await Promise.all([import('@widgets/startup-error'), getReactRoot()]);
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
