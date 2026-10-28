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
