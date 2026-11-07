import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

test('NVIDIA uses its hosted OpenAI-compatible endpoint and has provider labels', () => {
  const catalogPath = path.resolve(
    '../../../services/openchat-local-service/src/domain/providers/openai_compatible/mod.rs',
  );
  const catalogContent = fs.readFileSync(catalogPath, 'utf8');
  assert.match(
    catalogContent,
    /id: "nvidia",\s*name: "NVIDIA",\s*base_url: "https:\/\/integrate\.api\.nvidia\.com\/v1",\s*protocol: CustomProviderProtocol::OpenAi/,
  );

  const sectionPath = path.resolve('src/features/provider-settings/components/ApiKeyProvidersSection.tsx');
  const sectionContent = fs.readFileSync(sectionPath, 'utf8');
  assert.match(sectionContent, /nvidia:\s*'providerNameNvidia'/);
  assert.match(sectionContent, /nvidia:\s*'https:\/\/docs\.api\.nvidia\.com\/nim\/docs\/api-quickstart'/);

  const pickerPath = path.resolve('src/features/chat/components/ModelPicker.tsx');
  const pickerContent = fs.readFileSync(pickerPath, 'utf8');
  assert.match(pickerContent, /nvidia:\s*'providerNameNvidia'/);
  assert.match(pickerContent, /'nvidia'/);

  const settingsPath = path.resolve('src/features/provider-settings/components/ProviderSettingsSection.tsx');
  const settingsContent = fs.readFileSync(settingsPath, 'utf8');
  assert.match(settingsContent, /case 'nvidia':\s*return t\('providerNameNvidia'\)/);
});

test('all locales define the NVIDIA provider name', () => {
  const locales = ['de', 'en', 'es', 'fr', 'tr'];
  for (const locale of locales) {
    const localePath = path.resolve(`src/shared/lib/i18n/locales/${locale}.json`);
    const content = JSON.parse(fs.readFileSync(localePath, 'utf8'));
    assert.equal(content.providerNameNvidia, 'NVIDIA', `Missing providerNameNvidia in ${locale}`);
  }
});

test('NVIDIA provider icon uses the bundled SVG logo', () => {
  const iconPath = path.resolve('../assets/icons/providers/nvidia.svg');
  assert.ok(fs.existsSync(iconPath), 'nvidia.svg must exist');

  const icon = fs.readFileSync(iconPath, 'utf8');
  assert.match(icon, /<svg\b/);
  assert.match(icon, /<path\b/);

  const iconComponentPath = path.resolve('src/shared/components/ProviderIcon.tsx');
  const iconComponent = fs.readFileSync(iconComponentPath, 'utf8');
  assert.match(iconComponent, /import nvidiaIconUrl from '@assets\/icons\/providers\/nvidia\.svg\?no-inline'/);
  assert.match(iconComponent, /nvidia:\s*nvidiaIconUrl/);
});
