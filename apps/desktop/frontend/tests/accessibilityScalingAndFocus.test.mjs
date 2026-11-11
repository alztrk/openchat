import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendRoot = path.resolve(__dirname, '..');

function readFrontendFile(...segments) {
  return fs.readFileSync(path.join(frontendRoot, ...segments), 'utf8');
}

test('TODO-076 (#156): keyboard focus, high contrast, forced-colors, and reduced-motion rules are enforced', () => {
  const indexCss = readFrontendFile('src', 'index.css');
  const tokensCss = readFrontendFile('src', 'shared', 'components', 'styles', 'tokens.css');
  const animationsCss = readFrontendFile('src', 'shared', 'components', 'styles', 'animations.css');

  // Visible focus rules for buttons, tabs, menu items, options, tree items, links, summaries, and segmented labels
  assert.match(indexCss, /\[role='tab'\]:focus-visible/);
  assert.match(indexCss, /\[role='menuitem'\]:focus-visible/);
  assert.match(indexCss, /\[role='option'\]:focus-visible/);
  assert.match(indexCss, /\[role='treeitem'\]:focus-visible/);
  assert.match(indexCss, /label:has\(> input:focus-visible\) \.openchat-segment-label/);

  // Form controls and switches/sliders receive explicit focus borders or focus rings
  assert.match(indexCss, /textarea:focus-visible[\s\S]*?border-color:\s*rgb\(var\(--focus-ring\)\)\s*!important/u);
  assert.match(indexCss, /\[role='switch'\]:focus-visible[\s\S]*?outline:\s*2px solid rgb\(var\(--focus-ring\)\)\s*!important/u);

  // High contrast and forced-colors mode restore 2px solid focus-ring outlines
  assert.match(tokensCss, /@media \(prefers-contrast: more\), \(forced-colors: active\)/);
  assert.match(
    indexCss,
    /@media \(prefers-contrast: more\), \(forced-colors: active\)\s*\{\s*:focus-visible,\s*label:has\(> input:focus-visible\) \.openchat-segment-label\s*\{\s*outline:\s*2px solid rgb\(var\(--focus-ring\)\)\s*!important;/u,
  );

  // Reduced motion disables animations and sidebar transitions
  assert.match(animationsCss, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(animationsCss, /\.conversation-sidebar-shell,\s*\.conversation-sidebar\s*\{\s*transition:\s*none;/u);
});

test('TODO-077 (#157): 125%-200% text/app scaling, narrow layouts, and long translations are supported across all locales', () => {
  const appearancePrefs = readFrontendFile('src', 'features', 'app-settings', 'data', 'appearancePreferences.ts');
  const generalSettings = readFrontendFile(
    'src',
    'features',
    'app-settings',
    'components',
    'GeneralSettingsSection.tsx',
  );
  const radioGroup = readFrontendFile('src', 'shared', 'components', 'RadioGroup.tsx');
  const indexCss = readFrontendFile('src', 'index.css');

  // 125%-200% text scaling values (1.25, 1.5, 1.75, 2) and app scale values (125%, 150%, 175%, 200%)
  for (const scaleValue of ["'1.25'", "'1.5'", "'1.75'", "'2'"]) {
    assert.ok(appearancePrefs.includes(scaleValue), `appearancePreferences.ts should map textScale ${scaleValue}`);
  }
  for (const appScaleValue of ["'125%'", "'150%'", "'175%'", "'200%'"]) {
    assert.ok(appearancePrefs.includes(appScaleValue), `appearancePreferences.ts should map appScale ${appScaleValue}`);
  }
  assert.match(appearancePrefs, /export function isLargeConversationTextSize/);

  // GeneralSettingsSection exposes 125%-200% options
  for (const key of ['textSizeLarge', 'textSize150', 'textSize175', 'textSize200', 'appScale125', 'appScale150', 'appScale175', 'appScale200']) {
    assert.ok(generalSettings.includes(`t('${key}')`), `GeneralSettingsSection.tsx should render ${key}`);
  }

  // Segmented control labels wrap long strings instead of clipping in narrow layouts
  assert.match(radioGroup, /openchat-segment-label[^"]*wrap-anywhere/);
  assert.match(indexCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s*minmax\(220px,\s*min\(100%,\s*300px\)\)/);

  // All 5 locales define the scaling keys with non-empty localized labels
  for (const locale of ['de', 'en', 'es', 'fr', 'tr']) {
    const data = JSON.parse(readFrontendFile('src', 'shared', 'lib', 'i18n', 'locales', `${locale}.json`));
    for (const key of [
      'textSizeSmall',
      'textSizeNormal',
      'textSizeLarge',
      'textSize150',
      'textSize175',
      'textSize200',
      'appScaleSmall',
      'appScaleNormal',
      'appScaleLarge',
      'appScale125',
      'appScale150',
      'appScale175',
      'appScale200',
    ]) {
      assert.equal(typeof data[key], 'string', `${locale}.json missing ${key}`);
      assert.ok(data[key].trim().length > 0, `${locale}.json has empty ${key}`);
    }
  }
});

test('TODO-078 (#158): Windows UI Automation live regions and landmarks expose role="status" and accessible names', () => {
  const appTsx = readFrontendFile('src', 'bootstrap', 'App.tsx');

  // Every Suspense loading fallback with aria-live="polite" in App.tsx exposes role="status" for UIA LiveRegion announcements
  const politeParagraphs = [...appTsx.matchAll(/<p\b[^>]*aria-live="polite"[^>]*>/gu)].map((match) => match[0]);
  assert.ok(politeParagraphs.length >= 6, 'Expected at least 6 polite live region paragraphs in App.tsx');
  for (const tag of politeParagraphs) {
    assert.match(tag, /\brole="status"/, `Polite live-region paragraph is missing role="status": ${tag}`);
  }

  // Large text scaling helper is wired to primary and panel ConversationPanes
  assert.equal(
    [...appTsx.matchAll(/isLargeText=\{isLargeConversationTextSize\(appearance\.conversationTextSize\)\}/gu)].length,
    2,
  );
});
