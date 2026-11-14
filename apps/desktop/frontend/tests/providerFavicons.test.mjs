import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const providersSource = readFileSync(
  path.resolve('src/features/provider-settings/components/ApiKeyProvidersSection.tsx'),
  'utf8',
);
const cardSource = readFileSync(
  path.resolve('src/features/provider-settings/components/ApiKeyProviderCards.tsx'),
  'utf8',
);
const tauriConfig = JSON.parse(readFileSync('../tauri/tauri.conf.json', 'utf8'));
const faviconBlock = /const providerFaviconUrls:[\s\S]*?= \{([\s\S]*?)\n\};/u.exec(providersSource)?.[1];

if (!faviconBlock) throw new Error('Provider favicon mapping was not found.');

const faviconEntries = [...faviconBlock.matchAll(/^\s{2}([a-z][a-z0-9]*): '(https:\/\/[^']+)',?$/gmu)].map(
  ([, providerId, url]) => [providerId, url],
);

test('Connections favicon URLs use only their exact HTTPS origins in the desktop image CSP', () => {
  assert.ok(faviconEntries.length > 0);
  const origins = [...new Set(faviconEntries.map(([, value]) => new URL(value).origin))].sort();
  const imageDirective = tauriConfig.app.security.csp
    .split(';')
    .map((directive) => directive.trim())
    .find((directive) => directive.startsWith('img-src '));
  assert.ok(imageDirective);
  const allowedOrigins = imageDirective
    .split(/\s+/u)
    .slice(1)
    .filter((source) => source.startsWith('https://'))
    .sort();

  assert.deepEqual(allowedOrigins, origins);
  assert.equal(faviconEntries.some(([providerId]) => providerId === 'gemini'), false);
  assert.ok(faviconEntries.some(([providerId]) => providerId === 'moonshot'));
  assert.ok(faviconEntries.some(([providerId]) => providerId === 'kluster'));
});

test('credential cards keep the bundled provider mark visible while a favicon loads or fails', () => {
  assert.match(cardSource, /if \(!faviconUrl\) return <ProviderIcon/u);
  assert.match(cardSource, /!isLoaded \? <ProviderIcon/u);
  assert.match(cardSource, /onError=\{\(\) => setLoadedUrl\(null\)\}/u);
  assert.match(cardSource, /alt=""\s+aria-hidden="true"/u);
});

test('compatible provider favicon display remains confined to Connections credential cards', () => {
  assert.equal((providersSource.match(/faviconUrl=\{providerFaviconUrl\(providerId\)\}/gu) ?? []).length, 1);
  assert.match(providersSource, /function providerFaviconUrl\(providerId: CompatibleProviderId\)/u);
});
