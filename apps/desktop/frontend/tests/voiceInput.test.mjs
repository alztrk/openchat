import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/chat/data/voiceInput.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const modelSource = fs.readFileSync(path.resolve('src/entities/voice-input/model.ts'), 'utf8');
const compiledModels = ts.transpileModule(modelSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  appendVoiceTranscriptToDraft,
  baseLanguageCode,
  parseVoiceInputTranscription,
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
const { parseVoiceInputInstallProgress, parseVoiceInputModelStatus } = await import(
  `data:text/javascript;base64,${Buffer.from(compiledModels).toString('base64')}`
);

test('voice input model status and progress parsers accept the local RPC contract', () => {
  assert.deepEqual(parseVoiceInputModelStatus({ installed: true, modelBytes: 140, vadBytes: 2 }), {
    installed: true,
    modelBytes: 140,
    vadBytes: 2,
  });
  assert.deepEqual(
    parseVoiceInputInstallProgress({
      name: 'voice.input.model.install.progress',
      data: {
        fileName: 'speech.bin',
        totalDownloadedBytes: 128,
        totalBytes: 256,
        fileIndex: 1,
        fileCount: 2,
      },
    }),
    {
      fileName: 'speech.bin',
      totalDownloadedBytes: 128,
      totalBytes: 256,
      fileIndex: 1,
      fileCount: 2,
    },
  );
  assert.equal(
    parseVoiceInputInstallProgress({ name: 'other.event', data: {} }),
    null,
  );
});

test('voice input parsers reject malformed responses and trim transcripts', () => {
  assert.throws(() => parseVoiceInputModelStatus({ installed: true, modelBytes: -1, vadBytes: 2 }));
  assert.equal(parseVoiceInputTranscription({ text: '  local words  ' }), 'local words');
  assert.throws(() => parseVoiceInputTranscription({ text: 42 }));
});

test('voice transcript appending preserves the draft and ignores empty speech', () => {
  assert.equal(appendVoiceTranscriptToDraft('', '  hello  '), 'hello');
  assert.equal(appendVoiceTranscriptToDraft('existing', 'hello'), 'existing\nhello');
  assert.equal(appendVoiceTranscriptToDraft('existing\n', 'hello'), 'existing\nhello');
  assert.equal(appendVoiceTranscriptToDraft('existing', '   '), 'existing');
});

test('voice input uses the base locale language', () => {
  assert.equal(baseLanguageCode('tr-TR'), 'tr');
  assert.equal(baseLanguageCode('en_US'), 'en');
  assert.equal(baseLanguageCode(''), null);
  assert.equal(baseLanguageCode('123'), null);
});
