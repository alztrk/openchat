import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import test from 'node:test';

// Localized strings verification
test('all 5 locales define drag-and-drop context ingestion keys', () => {
  const locales = ['en', 'tr', 'de', 'es', 'fr'];
  const requiredKeys = [
    'dropFilesOverlayTitle',
    'dropFilesOverlayDescription',
    'composerDropTargetLabel',
  ];

  for (const locale of locales) {
    const filePath = resolve(`src/localization/locales/${locale}.json`);
    const content = JSON.parse(readFileSync(filePath, 'utf-8'));

    for (const key of requiredKeys) {
      assert.ok(
        typeof content[key] === 'string' && content[key].trim().length > 0,
        `Locale '${locale}' is missing required key '${key}'`,
      );
    }
  }
});

// Attachment specifications and limits mirroring chatAttachments.ts
const maximumAttachmentFileCount = 10;
const maximumAttachmentImageCount = 3;
const maximumAttachmentBytes = 14 * 1024 * 1024;
const maximumImageBytes = 10 * 1024 * 1024;
const maximumTextBytes = 1024 * 1024;

const attachmentTypes = {
  png: { kind: 'image', mimeType: 'image/png', maximumBytes: maximumImageBytes },
  jpg: { kind: 'image', mimeType: 'image/jpeg', maximumBytes: maximumImageBytes },
  jpeg: { kind: 'image', mimeType: 'image/jpeg', maximumBytes: maximumImageBytes },
  webp: { kind: 'image', mimeType: 'image/webp', maximumBytes: maximumImageBytes },
  gif: { kind: 'image', mimeType: 'image/gif', maximumBytes: maximumImageBytes },
  txt: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  log: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  md: { kind: 'text', mimeType: 'text/markdown', maximumBytes: maximumTextBytes },
  csv: { kind: 'text', mimeType: 'text/csv', maximumBytes: maximumTextBytes },
  json: { kind: 'text', mimeType: 'application/json', maximumBytes: maximumTextBytes },
  jsonl: { kind: 'text', mimeType: 'application/json', maximumBytes: maximumTextBytes },
  xml: { kind: 'text', mimeType: 'application/xml', maximumBytes: maximumTextBytes },
  html: { kind: 'text', mimeType: 'text/html', maximumBytes: maximumTextBytes },
  css: { kind: 'text', mimeType: 'text/css', maximumBytes: maximumTextBytes },
  js: { kind: 'text', mimeType: 'text/javascript', maximumBytes: maximumTextBytes },
  jsx: { kind: 'text', mimeType: 'text/javascript', maximumBytes: maximumTextBytes },
  ts: { kind: 'text', mimeType: 'text/typescript', maximumBytes: maximumTextBytes },
  tsx: { kind: 'text', mimeType: 'text/typescript', maximumBytes: maximumTextBytes },
  dart: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  rs: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  py: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  go: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  java: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  kt: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  swift: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  c: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  h: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  cpp: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  hpp: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  cs: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  sh: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  ps1: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  yaml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  yml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  toml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  ini: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  sql: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
};

function hasValidImageSignature(mimeType, bytes) {
  if (mimeType === 'image/png') {
    return (
      bytes.length >= 8 &&
      bytes[0] === 0x89 &&
      bytes[1] === 0x50 &&
      bytes[2] === 0x4e &&
      bytes[3] === 0x47 &&
      bytes[4] === 0x0d &&
      bytes[5] === 0x0a &&
      bytes[6] === 0x1a &&
      bytes[7] === 0x0a
    );
  }
  if (mimeType === 'image/jpeg') {
    return bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  }
  if (mimeType === 'image/gif') {
    return (
      bytes.length >= 6 &&
      bytes[0] === 0x47 &&
      bytes[1] === 0x49 &&
      bytes[2] === 0x46 &&
      bytes[3] === 0x38 &&
      (bytes[4] === 0x37 || bytes[4] === 0x39) &&
      bytes[5] === 0x61
    );
  }
  return (
    mimeType === 'image/webp' &&
    bytes.length >= 12 &&
    bytes[0] === 0x52 &&
    bytes[1] === 0x49 &&
    bytes[2] === 0x46 &&
    bytes[3] === 0x46 &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x45 &&
    bytes[10] === 0x42 &&
    bytes[11] === 0x50
  );
}

function validateFile(name, size, bytes) {
  const extension = name.split('.').at(-1)?.toLowerCase() ?? '';
  const type = attachmentTypes[extension];
  if (!type) return { ok: false, error: 'attachmentUnsupportedType' };
  if (size <= 0) return { ok: false, error: 'attachmentReadFailed' };
  if (size > type.maximumBytes) return { ok: false, error: 'attachmentFileTooLarge' };
  if (type.kind === 'image' && !hasValidImageSignature(type.mimeType, bytes)) {
    return { ok: false, error: 'attachmentInvalidImage' };
  }
  if (type.kind === 'text') {
    try {
      new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } catch {
      return { ok: false, error: 'attachmentMustBeUtf8' };
    }
  }
  return { ok: true, type, kind: type.kind };
}

function validateTotalAttachments(current, added) {
  const combined = [...current, ...added];
  if (
    combined.length > maximumAttachmentFileCount ||
    combined.filter((attachment) => attachment.kind === 'image').length > maximumAttachmentImageCount
  ) {
    return { ok: false, error: 'attachmentCountExceeded' };
  }
  if (combined.reduce((total, attachment) => total + attachment.sizeBytes, 0) > maximumAttachmentBytes) {
    return { ok: false, error: 'attachmentTotalTooLarge' };
  }
  return { ok: true };
}

test('supported image formats including GIF are recognized with valid signatures', () => {
  // PNG
  const pngBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00]);
  assert.equal(validateFile('test.png', pngBytes.length, pngBytes).ok, true);

  // JPG / JPEG
  const jpgBytes = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]);
  assert.equal(validateFile('photo.jpg', jpgBytes.length, jpgBytes).ok, true);
  assert.equal(validateFile('photo.jpeg', jpgBytes.length, jpgBytes).ok, true);

  // WEBP
  const webpBytes = new Uint8Array([
    0x52, 0x49, 0x46, 0x46, 0x20, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
  ]);
  assert.equal(validateFile('image.webp', webpBytes.length, webpBytes).ok, true);

  // GIF87a and GIF89a
  const gif87Bytes = new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x37, 0x61, 0x01, 0x00]);
  assert.equal(validateFile('anim.gif', gif87Bytes.length, gif87Bytes).ok, true);

  const gif89Bytes = new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00]);
  assert.equal(validateFile('animated.gif', gif89Bytes.length, gif89Bytes).ok, true);

  // Corrupted GIF signature
  const badGifBytes = new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x30, 0x61]);
  assert.deepEqual(validateFile('bad.gif', badGifBytes.length, badGifBytes), {
    ok: false,
    error: 'attachmentInvalidImage',
  });
});

test('code and text file formats are correctly ingested', () => {
  const codeFiles = [
    'script.js',
    'component.tsx',
    'main.rs',
    'app.py',
    'data.json',
    'readme.md',
    'debug.log',
    'notes.txt',
  ];

  const validUtf8 = new TextEncoder().encode('console.log("hello world");');
  for (const filename of codeFiles) {
    const res = validateFile(filename, validUtf8.length, validUtf8);
    assert.equal(res.ok, true, `Expected ${filename} to be valid`);
    assert.equal(res.kind, 'text');
  }
});

test('unsupported file extensions and invalid UTF-8 are rejected', () => {
  const binaryBytes = new Uint8Array([0x00, 0xff, 0xfe, 0xba, 0xbe]);
  assert.deepEqual(validateFile('app.exe', binaryBytes.length, binaryBytes), {
    ok: false,
    error: 'attachmentUnsupportedType',
  });

  const invalidUtf8 = new Uint8Array([0xc3, 0x28]); // invalid continuation byte
  assert.deepEqual(validateFile('corrupt.txt', invalidUtf8.length, invalidUtf8), {
    ok: false,
    error: 'attachmentMustBeUtf8',
  });
});

test('attachment size limits enforce bounds on single files and combined totals', () => {
  // Single image too large (> 10 MB)
  const hugeImageSize = 10 * 1024 * 1024 + 1;
  const dummyBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert.deepEqual(validateFile('huge.png', hugeImageSize, dummyBytes), {
    ok: false,
    error: 'attachmentFileTooLarge',
  });

  // Single text file too large (> 1 MB)
  const hugeTextSize = 1024 * 1024 + 1;
  assert.deepEqual(validateFile('huge.txt', hugeTextSize, new Uint8Array(10)), {
    ok: false,
    error: 'attachmentFileTooLarge',
  });

  // Combined count exceeds 10 files
  const tenFiles = Array.from({ length: 10 }, (_, i) => ({
    id: `file-${i}`,
    name: `doc-${i}.txt`,
    mimeType: 'text/plain',
    sizeBytes: 100,
    kind: 'text',
  }));
  assert.equal(validateTotalAttachments(tenFiles, []).ok, true);

  const eleventhFile = [
    { id: 'file-11', name: 'doc-11.txt', mimeType: 'text/plain', sizeBytes: 100, kind: 'text' },
  ];
  assert.deepEqual(validateTotalAttachments(tenFiles, eleventhFile), {
    ok: false,
    error: 'attachmentCountExceeded',
  });

  // Image count exceeds 3 images
  const threeImages = [
    { id: 'img-1', name: '1.png', mimeType: 'image/png', sizeBytes: 1000, kind: 'image' },
    { id: 'img-2', name: '2.png', mimeType: 'image/png', sizeBytes: 1000, kind: 'image' },
    { id: 'img-3', name: '3.png', mimeType: 'image/png', sizeBytes: 1000, kind: 'image' },
  ];
  assert.equal(validateTotalAttachments(threeImages, []).ok, true);

  const fourthImage = [
    { id: 'img-4', name: '4.png', mimeType: 'image/png', sizeBytes: 1000, kind: 'image' },
  ];
  assert.deepEqual(validateTotalAttachments(threeImages, fourthImage), {
    ok: false,
    error: 'attachmentCountExceeded',
  });

  // Cumulative bytes exceed 14 MB
  const largeFiles = [
    { id: 'img-1', name: '1.png', mimeType: 'image/png', sizeBytes: 8 * 1024 * 1024, kind: 'image' },
    { id: 'img-2', name: '2.png', mimeType: 'image/png', sizeBytes: 6 * 1024 * 1024, kind: 'image' },
  ];
  assert.equal(validateTotalAttachments(largeFiles, []).ok, true); // 14 MB

  const extraFile = [
    { id: 'doc-1', name: 'extra.txt', mimeType: 'text/plain', sizeBytes: 1024, kind: 'text' },
  ];
  assert.deepEqual(validateTotalAttachments(largeFiles, extraFile), {
    ok: false,
    error: 'attachmentTotalTooLarge',
  });
});
