import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/settings/data/connectorManifest.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { compareConnectorToolNames, parseConnectorManifest } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);
const projectMcpSourcePath = path.resolve('src/modules/projects/data/projectMcp.ts');
const projectMcpSource = fs.readFileSync(projectMcpSourcePath, 'utf8');
const compiledProjectMcp = ts.transpileModule(projectMcpSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { parseProjectMcpServer } = await import(
  `data:text/javascript;base64,${Buffer.from(compiledProjectMcp).toString('base64')}`
);
const connectorCatalogSourcePath = path.resolve('src/modules/settings/data/connectorCatalog.ts');
const connectorCatalogSource = fs.readFileSync(connectorCatalogSourcePath, 'utf8').replace(/^import .*;\r?\n/gm, '');
const compiledConnectorCatalog = ts.transpileModule(connectorCatalogSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { parseConnectorEntry } = await import(
  `data:text/javascript;base64,${Buffer.from(compiledConnectorCatalog).toString('base64')}`
);

const validManifest = {
  version: 1,
  tools: [{ name: 'search_records', description: 'Search approved project records.' }],
  permissions: ['read project records'],
  providers: ['openai'],
  configuration: [{ name: 'API_TOKEN', description: 'Service access token.', required: true, secret: true }],
};

test('accepts a bounded version 1 connector manifest', () => {
  assert.deepEqual(parseConnectorManifest(validManifest), validManifest);
});

test('accepts optional tool descriptions but rejects malformed supplied descriptions', () => {
  assert.deepEqual(parseConnectorManifest({ ...validManifest, tools: [{ name: 'search_records' }] }).tools, [
    { name: 'search_records' },
  ]);
  assert.throws(() =>
    parseConnectorManifest({ ...validManifest, tools: [{ name: 'search_records', description: ' ' }] }),
  );
  assert.throws(() =>
    parseConnectorManifest({ ...validManifest, tools: [{ name: 'search_records', description: 'x'.repeat(501) }] }),
  );
});

test('compares declared tool names with runtime names without changing runtime authority', () => {
  assert.deepEqual(compareConnectorToolNames(['search_records', 'write_record'], ['search_records', 'read_record']), {
    declaredOnly: ['write_record'],
    runtimeOnly: ['read_record'],
  });
  assert.deepEqual(compareConnectorToolNames(['Search'], ['search']), {
    declaredOnly: ['Search'],
    runtimeOnly: ['search'],
  });
  assert.equal(compareConnectorToolNames(undefined, ['search_records']), null);
  assert.equal(compareConnectorToolNames([], ['search_records']), null);
});

test('rejects tool names that cannot be exposed by the current MCP runtime', () => {
  assert.throws(() => parseConnectorManifest({ ...validManifest, tools: [{ name: 'search records' }] }));
  assert.throws(() => parseConnectorManifest({ ...validManifest, tools: [{ name: 'x'.repeat(65) }] }));
});

test('project catalogs accept old servers and bounded catalog tool claims', () => {
  const server = { id: 'records', enabled: false, program: 'C:\\Tools\\records.exe' };
  assert.equal(parseProjectMcpServer(server).catalogToolNames, undefined);
  assert.deepEqual(parseProjectMcpServer({ ...server, catalogToolNames: ['search_records'] }).catalogToolNames, [
    'search_records',
  ]);
  assert.throws(() => parseProjectMcpServer({ ...server, catalogToolNames: ['search records'] }));
  assert.throws(() => parseProjectMcpServer({ ...server, catalogToolNames: ['search', 'SEARCH'] }));
  assert.throws(() =>
    parseProjectMcpServer({ ...server, catalogToolNames: Array.from({ length: 129 }, (_, index) => `tool_${index}`) }),
  );
});

test('project package references preserve legacy cleanup and validate signer pins', () => {
  const packageServer = {
    id: 'records',
    enabled: false,
    transport: 'stdio',
    arguments: [],
    environmentVariables: [],
    package: {
      version: '1.0.0',
      sha256: 'a'.repeat(64),
      authenticodeSignerSha256: 'b'.repeat(64),
      sourceKeyFingerprint: 'c'.repeat(64),
      sizeBytes: 1,
    },
  };
  assert.equal(parseProjectMcpServer(packageServer).package.authenticodeSignerSha256, 'b'.repeat(64));

  const legacyPackage = { ...packageServer.package };
  delete legacyPackage.authenticodeSignerSha256;
  assert.equal(
    parseProjectMcpServer({ ...packageServer, package: legacyPackage }).package.authenticodeSignerSha256,
    undefined,
  );
  assert.throws(() =>
    parseProjectMcpServer({
      ...packageServer,
      package: { ...packageServer.package, authenticodeSignerSha256: 'B'.repeat(64) },
    }),
  );
});

test('saved catalogs preserve legacy package entries and signed entries require a canonical signer pin', () => {
  const entry = {
    id: 'records',
    name: 'Records',
    publisher: 'Records publisher',
    version: '1.0.0',
    sourceUrl: 'https://example.invalid/catalog.json',
    requestedAccess: 'read project records',
    dataDisclosure: 'Project records are sent to the configured provider.',
    transport: 'stdio',
    arguments: [],
    environmentVariables: [],
    package: {
      url: 'https://example.invalid/connector.exe',
      sha256: 'a'.repeat(64),
      authenticodeSignerSha256: 'b'.repeat(64),
      sizeBytes: 1,
    },
    catalogTrust: {
      sourceUrl: 'https://example.invalid/catalog.json',
      publicKeyBase64: 'A'.repeat(43),
      keyFingerprint: 'c'.repeat(64),
    },
  };

  assert.equal(parseConnectorEntry(entry, true).package.authenticodeSignerSha256, 'b'.repeat(64));
  const legacyPackage = { ...entry.package };
  delete legacyPackage.authenticodeSignerSha256;
  assert.equal(parseConnectorEntry({ ...entry, package: legacyPackage }).package.authenticodeSignerSha256, undefined);
  assert.throws(() => parseConnectorEntry({ ...entry, package: legacyPackage }, true));
  assert.throws(() =>
    parseConnectorEntry(
      {
        ...entry,
        package: { ...entry.package, authenticodeSignerSha256: 'B'.repeat(64) },
      },
      true,
    ),
  );
});

test('rejects unsupported versions and unknown fields', () => {
  assert.throws(() => parseConnectorManifest({ ...validManifest, version: 2 }));
  assert.throws(() => parseConnectorManifest({ ...validManifest, publisherVerified: true }));
  assert.throws(() =>
    parseConnectorManifest({ ...validManifest, tools: [{ name: 'search_records', executable: true }] }),
  );
});

test('rejects invisible bidirectional controls in catalog declarations', () => {
  assert.throws(() =>
    parseConnectorManifest({
      ...validManifest,
      permissions: ['read project\u202erecords'],
    }),
  );
});

test('rejects duplicate and over-limit declarations', () => {
  assert.throws(() =>
    parseConnectorManifest({ ...validManifest, tools: [{ name: 'search_records' }, { name: 'SEARCH_RECORDS' }] }),
  );
  assert.throws(() =>
    parseConnectorManifest({ ...validManifest, permissions: [...validManifest.permissions, 'READ PROJECT RECORDS'] }),
  );
  assert.throws(() =>
    parseConnectorManifest({
      ...validManifest,
      tools: Array.from({ length: 129 }, (_, index) => ({ name: `tool_${index}` })),
    }),
  );
});

test('rejects malformed configuration requirements and oversized manifests', () => {
  assert.throws(() =>
    parseConnectorManifest({
      ...validManifest,
      configuration: [{ name: 'API_TOKEN', description: 'Service access token.', required: 'yes', secret: true }],
    }),
  );
  assert.throws(() =>
    parseConnectorManifest({
      ...validManifest,
      tools: Array.from({ length: 128 }, (_, index) => ({ name: `tool_${index}`, description: 'x'.repeat(500) })),
    }),
  );
});
