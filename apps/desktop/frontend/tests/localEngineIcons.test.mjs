import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const assetDirectory = path.resolve('src/shared/components/assets');

test('local engine icons are selected by their engine IDs', () => {
  for (const icon of ['llama-cpp.svg', 'sglang.svg', 'vllm.svg']) {
    assert.ok(fs.existsSync(path.join(assetDirectory, icon)), `${icon} must exist`);
  }

  const engineIcon = fs.readFileSync(path.resolve('src/shared/components/EngineIcon.tsx'), 'utf8');
  assert.match(engineIcon, /engineId === 'llama_cpp' \|\| engineId === 'vllm'/);
  assert.match(engineIcon, /engineId === 'sglang'/);
  assert.match(engineIcon, /engineId === 'exllama'/);
  assert.match(engineIcon, /Microchip, Zap/);
  assert.match(engineIcon, /assets\/sglang\.svg/);
  assert.match(engineIcon, /assets\/vllm\.svg/);

  const providerIcon = fs.readFileSync(path.resolve('src/shared/components/ProviderIcon.tsx'), 'utf8');
  assert.match(
    providerIcon,
    /providerId === 'llama_cpp' \|\| providerId === 'vllm' \|\| providerId === 'exllama' \|\| providerId === 'sglang'/,
  );
});

test('engine icon sources and applicable licenses are documented beside the assets', () => {
  const notices = fs.readFileSync(path.join(assetDirectory, 'THIRD_PARTY_NOTICES.md'), 'utf8');
  assert.match(notices, /Existing bundled Llama\.cpp/);
  assert.match(notices, /sgl-project\/sglang/);
  assert.match(notices, /simple-icons\/simple-icons/);
  assert.match(notices, /does not automatically apply to individual brand marks/);

  const sglangLicense = fs.readFileSync(path.join(assetDirectory, 'LICENSE-SGLANG.txt'), 'utf8');
  assert.match(sglangLicense, /Apache License/);
  assert.match(sglangLicense, /Copyright 2023-2024 SGLang Team/);
});
