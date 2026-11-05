import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bootstrapSource = readFileSync(path.join(frontendRoot, 'src/bootstrap/main.tsx'), 'utf8');
const noticeSource = readFileSync(path.join(frontendRoot, 'src/bootstrap/DesktopRequiredPage.tsx'), 'utf8');
const locales = ['de', 'en', 'es', 'fr', 'tr'];

test('non-Tauri startup renders the desktop-required notice before desktop services or overlays', () => {
  const runtimeGuard = bootstrapSource.indexOf('if (!isTauri()) {');
  const desktopNotice = bootstrapSource.indexOf("import('./DesktopRequiredPage')", runtimeGuard);
  const noticeRender = bootstrapSource.indexOf('root.render(React.createElement(DesktopRequiredPage));', runtimeGuard);
  const guardReturn = bootstrapSource.indexOf('return;', noticeRender);
  const overlayBranch = bootstrapSource.indexOf("get('computerUseOverlay') === '1'");
  const storageImport = bootstrapSource.indexOf("import('@shared/lib/platform/storage/unifiedStorage')");

  assert.notEqual(runtimeGuard, -1);
  assert.ok(desktopNotice > runtimeGuard);
  assert.ok(noticeRender > desktopNotice);
  assert.ok(guardReturn > noticeRender);
  assert.ok(guardReturn < overlayBranch);
  assert.ok(guardReturn < storageImport);
  assert.ok(desktopNotice < storageImport);
  assert.ok(overlayBranch > runtimeGuard);
  assert.ok(storageImport > runtimeGuard);
  assert.match(noticeSource, /<main[^>]*aria-labelledby="desktop-required-title"/u);
  assert.match(noticeSource, /t\('desktopRequiredTitle'\)/u);
  assert.match(noticeSource, /t\('desktopRequiredDescription'\)/u);
});

test('desktop-required copy is present in every supported locale', () => {
  for (const locale of locales) {
    const dictionary = JSON.parse(
      readFileSync(path.join(frontendRoot, `src/shared/lib/i18n/locales/${locale}.json`), 'utf8'),
    );
    assert.ok(dictionary.desktopRequiredTitle, `${locale} desktopRequiredTitle`);
    assert.ok(dictionary.desktopRequiredDescription, `${locale} desktopRequiredDescription`);
  }
});
