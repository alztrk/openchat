import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const navigationRailSource = readFileSync(path.resolve('src/widgets/navigation-rail/NavigationRail.tsx'), 'utf8');
const localeDirectory = path.resolve('src/shared/lib/i18n/locales');
const supportedLocales = ['de', 'en', 'es', 'fr', 'tr'];

test('the application navigation landmark is not named after only the chat surface', () => {
  assert.match(navigationRailSource, /<nav\s+aria-label=\{t\('primaryNavigation'\)\}/u);
  assert.doesNotMatch(navigationRailSource, /<nav\s+aria-label=\{t\('chats'\)\}/u);
});

test('every supported locale names the primary application navigation', () => {
  for (const locale of supportedLocales) {
    const translations = JSON.parse(readFileSync(path.join(localeDirectory, `${locale}.json`), 'utf8'));
    assert.equal(typeof translations.primaryNavigation, 'string', `${locale} must define primaryNavigation`);
    assert.notEqual(translations.primaryNavigation.trim(), '', `${locale} must not leave it blank`);
  }
});
