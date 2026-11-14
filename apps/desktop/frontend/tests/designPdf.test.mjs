import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/design-workspace/data/designPdf.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { createDesignPdf } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('Design PDF embeds a bounded JPEG page with valid byte offsets', () => {
  const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xd9]);
  const pdf = createDesignPdf(jpeg, 1200, 600);
  const encoded = new TextDecoder().decode(pdf);
  assert.equal(encoded.startsWith('%PDF-1.4'), true);
  assert.equal(encoded.endsWith('%%EOF'), true);
  assert.match(encoded, /\/MediaBox \[0 0 595 298\]/);
  assert.match(encoded, /\/Subtype \/Image \/Width 1200 \/Height 600/);
  const xrefStart = Number(encoded.match(/startxref\n(\d+)\n%%EOF/)?.[1]);
  assert.equal(encoded.slice(xrefStart, xrefStart + 4), 'xref');
  const xrefEntries = encoded.slice(xrefStart).match(/xref\n0 6\n[\s\S]*?trailer/)?.[0].split('\n').slice(3, 8);
  assert.equal(xrefEntries?.length, 5);
  for (const [index, entry] of (xrefEntries ?? []).entries()) {
    const offset = Number(entry?.slice(0, 10));
    const objectHeader = Buffer.from(pdf.slice(offset, offset + 7)).toString('ascii');
    assert.equal(objectHeader, `${index + 1} 0 obj`);
  }
});

test('Design PDF rejects incomplete JPEG data and unsupported image bounds', () => {
  assert.throws(() => createDesignPdf(new Uint8Array([0xff, 0xd8]), 100, 100));
  assert.throws(() => createDesignPdf(new Uint8Array([0xff, 0xd8, 0xff, 0xd9]), 5000, 100));
});
