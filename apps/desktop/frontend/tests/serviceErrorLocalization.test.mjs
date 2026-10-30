import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const errorDetailsSource = fs
  .readFileSync(path.resolve('src/modules/conversations/stores/chatStoreErrorDetails.ts'), 'utf8')
  .replace(
    "import { LocalServiceError } from '@/platform/local-service';",
    `export class LocalServiceError extends Error {
      constructor(code, message, retryable = false, details = null) {
        super(message);
        this.name = 'LocalServiceError';
        this.code = code;
        this.retryable = retryable;
        this.details = details;
      }
    }`,
  );

const compiled = ts.transpileModule(errorDetailsSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;

const { LocalServiceError, formatServiceErrorMessage, formatUnknownServiceError, resolveServiceFailureCode } =
  await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

const trLocale = JSON.parse(fs.readFileSync(path.resolve('src/localization/locales/tr.json'), 'utf8'));
const translateTr = (key, options) => trLocale[key] ?? options?.defaultValue ?? key;

test('maps unlisted provider tool errors to a dedicated failure code and Turkish copy', () => {
  const unlistedToolError = new LocalServiceError(
    'invalid_provider_response',
    'The provider requested a tool that was not included in the request.',
    false,
  );

  assert.equal(resolveServiceFailureCode(unlistedToolError, 'security_scan_request_failed'), 'provider_unlisted_tool');
  assert.equal(
    formatUnknownServiceError(unlistedToolError, translateTr, 'securityScanRequestFailed'),
    trLocale.providerRequestedUnlistedTool,
  );
  assert.equal(
    formatServiceErrorMessage('The provider requested a tool that was not included in the request.', translateTr),
    trLocale.providerRequestedUnlistedTool,
  );
});

test('never leaks unmapped English LocalServiceError messages when formatting scan errors', () => {
  const unknownBackendError = new LocalServiceError(
    'unmapped_internal_failure',
    'Unexpected raw English backend failure text.',
    false,
  );

  assert.equal(
    formatUnknownServiceError(unknownBackendError, translateTr, 'securityScanRequestFailed'),
    trLocale.securityScanRequestFailed,
  );
});

test('all supported locales include security scan and unavailable tool error translations', () => {
  const requiredKeys = [
    'providerRequestedUnlistedTool',
    'toolErrorNotAvailable',
    'toolErrorPlanModeDisabled',
    'securityScanRequestFailed',
    'securityScanRouteMissing',
    'securityScanNoResumableCheckpoint',
    'securityScanStopFailed',
    'securityScanStatusInvalid',
  ];

  for (const localeName of ['de', 'en', 'es', 'fr', 'tr']) {
    const locale = JSON.parse(fs.readFileSync(path.resolve(`src/localization/locales/${localeName}.json`), 'utf8'));
    for (const key of requiredKeys) {
      assert.equal(typeof locale[key], 'string', `${localeName}.json must define ${key}`);
      assert.ok(locale[key].trim().length > 0, `${localeName}.json ${key} must not be empty`);
    }
  }
});
