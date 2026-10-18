import React from 'react';
import ReactDOM from 'react-dom/client';
import '../index.css';
import { StartupErrorPage } from '@/shell/StartupErrorPage';

const rootElement = document.getElementById('root');

if (!rootElement) {
  throw new Error('The OpenChat application root element is missing.');
}
const reactRoot = ReactDOM.createRoot(rootElement);

async function startApplication(): Promise<void> {
  const [{ App }, { i18n, i18nReady }] = await Promise.all([import('./App'), import('../localization/i18n')]);
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
  reactRoot.render(<StartupErrorPage error={error} onRetry={startApplication} />);
});
