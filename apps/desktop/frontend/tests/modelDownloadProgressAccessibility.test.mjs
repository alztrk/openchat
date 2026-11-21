import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import ts from 'typescript';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const componentSource = readSource('src/features/model-catalog/components/ModelCatalogTab.tsx');
const localePaths = ['de', 'en', 'es', 'fr', 'tr'].map((locale) => `src/shared/lib/i18n/locales/${locale}.json`);

const catalogDataSource = readSource('src/features/model-catalog/data/modelCatalogData.ts').replace(
  /import\s*\{\s*type JsonObject,\s*LocalServiceError\s*\}\s*from '@shared\/lib\/platform\/local-service';/u,
  `export class LocalServiceError extends Error {
    constructor(code, message) {
      super(message);
      this.code = code;
    }
  }`,
);
const compiledCatalogData = ts.transpileModule(catalogDataSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { downloadErrorMessageKey, isModelCatalogEngineId, modelCatalogEngineIds, parseHubDetails } = await import(
  `data:text/javascript;base64,${Buffer.from(compiledCatalogData).toString('base64')}`
);

test('model download progress has a progress-specific accessible name and localized value text', () => {
  const statusStart = componentSource.indexOf('function ModelDownloadStatus(');
  assert.ok(statusStart >= 0);
  const statusSource = componentSource.slice(statusStart);

  assert.equal((statusSource.match(/aria-label=\{t\('modelDownloadProgressLabel'\)\}/gu) ?? []).length, 2);
  assert.match(statusSource, /aria-valuetext=\{[\s\S]*?t\('modelDownloadProgressValue'/u);
  assert.match(statusSource, /downloaded: formatBytes\(progress\.downloadedBytes, locale\)/u);
  assert.match(statusSource, /total: formatBytes\(progress\.totalBytes, locale\)/u);
  assert.match(statusSource, /progress && progress\.totalBytes > 0[\s\S]*?: undefined/u);
  assert.match(statusSource, /value=\{fraction \?\? undefined\}/u);
  assert.doesNotMatch(statusSource, /aria-label=\{t\('modelDownloadOptionsLabel'\)\}/u);
});

test('the model catalog exposes compatible engines and preserves the Transformers selection', () => {
  assert.deepEqual(modelCatalogEngineIds, ['llama_cpp', 'vllm', 'exllama', 'sglang']);
  for (const engineId of modelCatalogEngineIds) {
    assert.equal(isModelCatalogEngineId(engineId), true);
  }
  assert.equal(isModelCatalogEngineId('unknown'), false);

  const selectorStart = componentSource.indexOf("aria-label={t('modelDownloadEngine')}");
  const optionsStart = componentSource.indexOf('options={[', selectorStart);
  const selectedValue = componentSource.indexOf('value={engineId}', optionsStart);
  assert.ok(selectorStart >= 0 && optionsStart > selectorStart && selectedValue > optionsStart);
  const optionsSource = componentSource.slice(optionsStart, selectedValue);
  for (const [engineId, labelKey] of [
    ['llama_cpp', 'modelEngineLlamaCpp'],
    ['vllm', 'modelEngineVllm'],
    ['exllama', 'modelEngineExllama'],
    ['sglang', 'modelEngineSglang'],
  ]) {
    assert.ok(optionsSource.includes(`value: '${engineId}'`), `${engineId} is missing from the engine selector`);
    assert.ok(optionsSource.includes(`t('${labelKey}')`), `${engineId} is missing its localized label`);
    assert.ok(optionsSource.includes(`engineId="${engineId}"`), `${engineId} is missing its engine icon`);
  }
  assert.match(componentSource, /const format = engineId === 'llama_cpp' \? 'gguf' : 'transformers'/u);
});

test('all supported locales describe both model download progress and its byte range', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    assert.equal(typeof locale.modelDownloadProgressLabel, 'string', `${localePath} is missing the progress label`);
    assert.match(locale.modelDownloadProgressValue, /\{downloaded\}/u, `${localePath} is missing downloaded bytes`);
    assert.match(locale.modelDownloadProgressValue, /\{total\}/u, `${localePath} is missing total bytes`);
    assert.equal(
      typeof locale.modelDownloadChecksumMismatch,
      'string',
      `${localePath} is missing the SHA-256 mismatch error label`,
    );
    assert.equal(
      typeof locale.modelDownloadIncomplete,
      'string',
      `${localePath} is missing the incomplete download error label`,
    );
  }
});

test('model catalog data validates SHA-256 digests and maps integrity errors', () => {
  const validSha256 = '0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef';
  const details = parseHubDetails({
    repoId: 'openchat/test-gguf',
    revision: '0123456789abcdef0123456789abcdef01234567',
    gated: false,
    private: false,
    license: 'apache-2.0',
    readmeStatus: 'missing',
    readme: null,
    files: [
      {
        path: 'model-Q4_K_M.gguf',
        kind: 'model',
        sizeBytes: 4096,
        sha256: validSha256,
      },
    ],
    downloadGroups: [
      {
        id: 'gguf-1',
        displayName: 'model-Q4_K_M.gguf',
        totalBytes: 4096,
        files: [
          {
            path: 'model-Q4_K_M.gguf',
            kind: 'model',
            sizeBytes: 4096,
            sha256: validSha256,
          },
        ],
      },
      {
        id: 'gguf-missing-sha',
        displayName: 'unverified.gguf',
        totalBytes: 4096,
        files: [
          {
            path: 'unverified.gguf',
            kind: 'model',
            sizeBytes: 4096,
            sha256: null,
          },
        ],
      },
    ],
  });

  assert.equal(details.files[0].sha256, validSha256.toLowerCase());
  assert.equal(details.downloadGroups[0].canDownload, true);
  assert.equal(details.downloadGroups[1].canDownload, false);
  assert.equal(downloadErrorMessageKey('hugging_face_checksum_mismatch'), 'modelDownloadChecksumMismatch');
  assert.equal(downloadErrorMessageKey('hugging_face_download_incomplete'), 'modelDownloadIncomplete');
});
