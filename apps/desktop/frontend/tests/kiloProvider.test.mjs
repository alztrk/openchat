import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

test('compatible providers come from the native catalog and have UI documentation', () => {
  const catalogPath = path.resolve(
    '../../../services/openchat-local-service/src/domain/providers/openai_compatible/mod.rs',
  );
  const catalogContent = fs.readFileSync(catalogPath, 'utf8');
  assert.match(catalogContent, /id: "kilo"/);

  const sectionPath = path.resolve('src/modules/providers/components/ApiKeyProvidersSection.tsx');
  const sectionContent = fs.readFileSync(sectionPath, 'utf8');
  assert.match(sectionContent, /snapshot\.compatibleProviders\.map/);
  assert.match(sectionContent, /kilo:\s*'providerNameKilo'/);
  assert.match(sectionContent, /kilo:\s*'https:\/\/kilo\.ai\/docs'/);
});

test('all 5 locales define kilo provider names and descriptions', () => {
  const locales = ['de', 'en', 'es', 'fr', 'tr'];
  for (const locale of locales) {
    const localePath = path.resolve(`src/localization/locales/${locale}.json`);
    const content = JSON.parse(fs.readFileSync(localePath, 'utf8'));
    assert.equal(content.providerNameKilo, 'Kilo AI', `Missing providerNameKilo in ${locale}`);
    assert.ok(
      typeof content.kiloApiDescription === 'string' && content.kiloApiDescription.length > 10,
      `Missing or invalid kiloApiDescription in ${locale}`,
    );
  }
});

test('modelCatalogLoader includes kilo in providers and loads kilo free catalog when not configured', () => {
  const loaderPath = path.resolve('src/modules/providers/data/modelCatalogLoader.ts');
  const loaderContent = fs.readFileSync(loaderPath, 'utf8');
  assert.match(loaderContent, /providerId:\s*'kilo',\s*name:\s*null,\s*available:\s*true/);
  assert.match(loaderContent, /loadKiloFreeCatalog/);
});

test('kilo icon exists and is registered in ProviderIcon', () => {
  const iconPath = path.resolve('../assets/icons/providers/kilo.svg');
  assert.ok(fs.existsSync(iconPath), 'kilo.svg must exist');

  const iconCompPath = path.resolve('src/modules/providers/components/ProviderIcon.tsx');
  const iconCompContent = fs.readFileSync(iconCompPath, 'utf8');
  assert.match(iconCompContent, /kilo:\s*kiloIconUrl/);
});

test('ModelPicker and chatStoreModels configure kilo ordering, translations, and groupings', () => {
  const pickerPath = path.resolve('src/modules/conversations/components/ModelPicker.tsx');
  const pickerContent = fs.readFileSync(pickerPath, 'utf8');
  assert.match(pickerContent, /'kilo'/);
  assert.match(pickerContent, /kilo:\s*'providerNameKilo'/);
  assert.match(pickerContent, /providerId === 'openrouter' \|\| providerId === 'kilo'/);

  const storePath = path.resolve('src/modules/conversations/stores/chatStoreModels.ts');
  const storeContent = fs.readFileSync(storePath, 'utf8');
  assert.match(storeContent, /\['kilo',\s*\{\s*providerId:\s*'kilo'/);
});
