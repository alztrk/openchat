import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./audioEncoding.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  createVoiceInputPayload,
  encodeAudioBufferToPcm16Base64,
  encodePcm16Base64,
  isVoiceInputPayload,
  normalizeMaxRecordingDurationMs,
  normalizeVoiceLanguage,
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

function fakeAudioBuffer(channels, sampleRate) {
  return {
    length: channels[0]?.length ?? 0,
    numberOfChannels: channels.length,
    sampleRate,
    getChannelData(channel) {
      return channels[channel];
    },
  };
}

function decodePcm16(base64) {
  const bytes = Buffer.from(base64, 'base64');
  const samples = [];
  for (let offset = 0; offset < bytes.length; offset += 2) samples.push(bytes.readInt16LE(offset));
  return samples;
}

test('normalizes valid language tags and rejects malformed values', () => {
  assert.equal(normalizeVoiceLanguage(' tr_TR '), 'tr-TR');
  assert.equal(normalizeVoiceLanguage(''), null);
  assert.equal(normalizeVoiceLanguage('en US'), null);
  assert.equal(normalizeVoiceLanguage('en\nUS'), null);
});

test('clamps recording duration to the bounded capture range', () => {
  assert.equal(normalizeMaxRecordingDurationMs(), 30_000);
  assert.equal(normalizeMaxRecordingDurationMs(500), 1_000);
  assert.equal(normalizeMaxRecordingDurationMs(300_001), 120_000);
  assert.equal(normalizeMaxRecordingDurationMs(Number.NaN), 30_000);
});

test('encodes floating-point samples as little-endian PCM16 base64', () => {
  const samples = decodePcm16(encodePcm16Base64(new Float32Array([-1, -0.5, 0, 0.5, 1])));
  assert.deepEqual(samples, [-32768, -16384, 0, 16384, 32767]);
});

test('mixes channels and resamples decoded audio to 16 kHz', () => {
  const buffer = fakeAudioBuffer([new Float32Array([0, 1]), new Float32Array([0, 1])], 8_000);
  const samples = decodePcm16(encodeAudioBufferToPcm16Base64(buffer));

  assert.deepEqual(samples, [0, 16384, 32767, 32767]);
});

test('creates and validates the local sidecar payload', () => {
  const payload = createVoiceInputPayload(fakeAudioBuffer([new Float32Array([0, 0.5])], 16_000), 'en-US');

  assert.equal(payload.sampleRate, 16_000);
  assert.equal(payload.language, 'en-US');
  assert.equal(isVoiceInputPayload(payload), true);
  assert.equal(isVoiceInputPayload({ ...payload, sampleRate: 8_000 }), false);
  assert.equal(isVoiceInputPayload({ ...payload, pcm16Base64: 'not base64' }), false);
});

test('rejects empty decoded audio before creating a payload', () => {
  assert.throws(
    () => createVoiceInputPayload(fakeAudioBuffer([new Float32Array()], 16_000), 'en-US'),
    /decoded audio buffer is invalid/u,
  );
});
