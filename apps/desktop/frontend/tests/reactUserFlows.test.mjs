import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test, { after, afterEach } from 'node:test';
import { Window } from 'happy-dom';
import { createInstance } from 'i18next';
import { createServer } from 'vite';

const vite = await createServer({
  appType: 'custom',
  configFile: path.resolve('vite.config.ts'),
  mode: 'development',
  server: { middlewareMode: true, hmr: false },
});

const browserWindow = new Window({ url: 'http://localhost/' });
const globals = new Map([
  ['window', browserWindow],
  ['document', browserWindow.document],
  ['navigator', browserWindow.navigator],
  ['HTMLElement', browserWindow.HTMLElement],
  ['Node', browserWindow.Node],
  ['Element', browserWindow.Element],
  ['Event', browserWindow.Event],
  ['CustomEvent', browserWindow.CustomEvent],
  ['MouseEvent', browserWindow.MouseEvent],
  ['KeyboardEvent', browserWindow.KeyboardEvent],
  ['MutationObserver', browserWindow.MutationObserver],
  ['ResizeObserver', browserWindow.ResizeObserver],
  ['getComputedStyle', browserWindow.getComputedStyle.bind(browserWindow)],
  ['requestAnimationFrame', browserWindow.requestAnimationFrame.bind(browserWindow)],
  ['cancelAnimationFrame', browserWindow.cancelAnimationFrame.bind(browserWindow)],
  ['IS_REACT_ACT_ENVIRONMENT', true],
]);
const originalGlobals = new Map();

for (const [name, value] of globals) {
  originalGlobals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { configurable: true, value, writable: true });
}

const React = await import('react');
const { I18nextProvider } = await import('react-i18next');
const { cleanup, fireEvent, render, screen } = await import('@testing-library/react');
const { ConversationEmptyState } = await vite.ssrLoadModule('/src/features/chat/components/ConversationEmptyState.tsx');
const { NavigationRail } = await vite.ssrLoadModule('/src/widgets/navigation-rail/NavigationRail.tsx');

const localeDirectory = path.resolve('src/shared/lib/i18n/locales');
const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'];
const translationsByLocale = new Map();
const i18nByLocale = new Map();

for (const locale of supportedLocales) {
  const translations = JSON.parse(readFileSync(path.join(localeDirectory, `${locale}.json`), 'utf8'));
  const i18n = createInstance();
  await i18n.init({
    fallbackLng: false,
    lng: locale,
    resources: { [locale]: { translation: translations } },
  });
  translationsByLocale.set(locale, translations);
  i18nByLocale.set(locale, i18n);
}

function localized(locale, child) {
  const i18n = i18nByLocale.get(locale);
  assert.ok(i18n, `missing i18n instance for ${locale}`);
  return React.createElement(I18nextProvider, { i18n }, child);
}

function emptyStateProps(overrides = {}) {
  return {
    hasAvailableModels: true,
    hasConfiguredProvider: true,
    isLoading: false,
    modelCatalogFailed: false,
    modelCatalogStale: false,
    chatGptWorkspaceSelectionRequired: false,
    onOpenConnections: () => undefined,
    onRetryModels: () => undefined,
    selectedModel: false,
    ...overrides,
  };
}

afterEach(cleanup);

after(async () => {
  cleanup();
  await vite.close();
  await browserWindow.happyDOM.abort();
  browserWindow.close();

  for (const [name, descriptor] of originalGlobals) {
    if (descriptor) {
      Object.defineProperty(globalThis, name, descriptor);
    } else {
      Reflect.deleteProperty(globalThis, name);
    }
  }
});

test('a model catalog with no selected model renders selection guidance, and selection renders the welcome state', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  const view = render(
    localized('en', React.createElement(ConversationEmptyState, emptyStateProps())),
  );
  assert.equal(
    screen.getByRole('heading', { level: 1 }).textContent,
    translations.emptyChatSelectModelTitle,
  );
  assert.equal(screen.getByText(translations.emptyChatSelectModelBody).textContent, translations.emptyChatSelectModelBody);

  view.rerender(
    localized('en', React.createElement(ConversationEmptyState, emptyStateProps({ selectedModel: true }))),
  );
  assert.equal(screen.getByRole('heading', { level: 1 }).textContent, translations.emptyChatWelcomeTitle);
  assert.equal(screen.getByText(translations.emptyChatWelcomeBody).textContent, translations.emptyChatWelcomeBody);
});

test('a failed empty model catalog shows recovery guidance instead of selection guidance', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);

  render(
    localized(
      'en',
      React.createElement(
        ConversationEmptyState,
        emptyStateProps({ hasAvailableModels: false, modelCatalogFailed: true }),
      ),
    ),
  );

  assert.equal(screen.getByRole('heading', { level: 1 }).textContent, translations.modelsUnavailable);
  assert.equal(screen.getByText(translations.modelCatalogUnavailable).textContent, translations.modelCatalogUnavailable);
  assert.equal(screen.queryByText(translations.emptyChatSelectModelBody), null);
});

test('the empty chat connection action invokes its callback when setup is required', () => {
  const translations = translationsByLocale.get('en');
  assert.ok(translations);
  let openConnectionsCount = 0;

  render(
    localized(
      'en',
      React.createElement(
        ConversationEmptyState,
        emptyStateProps({
          hasAvailableModels: false,
          hasConfiguredProvider: false,
          onOpenConnections: () => {
            openConnectionsCount += 1;
          },
        }),
      ),
    ),
  );

  fireEvent.click(screen.getByRole('button', { name: translations.connections }));
  assert.equal(openConnectionsCount, 1);
});

test('the primary navigation landmark and surface action render in every supported locale', () => {
  for (const locale of supportedLocales) {
    const translations = translationsByLocale.get(locale);
    assert.ok(translations, `missing translations for ${locale}`);
    const navigatedSurfaces = [];

    render(
      localized(
        locale,
        React.createElement(NavigationRail, {
          activeSurface: 'chats',
          isDark: false,
          onNavigate: (surface) => navigatedSurfaces.push(surface),
          onToggleTheme: () => undefined,
        }),
      ),
    );

    const navigation = screen.getByRole('navigation', { name: translations.primaryNavigation });
    assert.equal(navigation.tagName, 'NAV');
    assert.notEqual(translations.primaryNavigation, translations.chats);

    fireEvent.click(screen.getByRole('button', { name: translations.modelLibrary }));
    assert.deepEqual(navigatedSurfaces, ['models']);
    cleanup();
  }
});
