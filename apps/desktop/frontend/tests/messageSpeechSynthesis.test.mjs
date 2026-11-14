import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test, { afterEach } from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(
  path.resolve('src/features/chat/components/message/speechSynthesis.ts'),
  'utf8',
);
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  disposeLocalSpeech,
  selectLocalSpeechVoice,
  startLocalSpeech,
  stopLocalSpeech,
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

const previousWindow = globalThis.window;
const previousUtterance = globalThis.SpeechSynthesisUtterance;

class FakeUtterance {
  constructor(text) {
    this.text = text;
    this.voice = null;
    this.lang = '';
    this.onstart = null;
    this.onend = null;
    this.onerror = null;
  }
}

function installSpeechHarness(voices, { throwOnSpeak = false, throwOnCancel = false } = {}) {
  const utterances = [];
  let cancelCount = 0;
  const synthesis = {
    getVoices() {
      return voices;
    },
    speak(utterance) {
      if (throwOnSpeak) throw new Error('speech start failed');
      utterances.push(utterance);
    },
    cancel() {
      cancelCount += 1;
      if (throwOnCancel) throw new Error('speech cancellation failed');
    },
  };

  globalThis.window = { speechSynthesis: synthesis };
  globalThis.SpeechSynthesisUtterance = FakeUtterance;
  return {
    synthesis,
    utterances,
    get cancelCount() {
      return cancelCount;
    },
  };
}

function callbacks(events) {
  return {
    onStart: () => events.push('start'),
    onFinish: () => events.push('finish'),
    onError: () => events.push('error'),
  };
}

afterEach(() => {
  stopLocalSpeech();
  globalThis.window = previousWindow;
  globalThis.SpeechSynthesisUtterance = previousUtterance;
});

test('selectLocalSpeechVoice ignores remote voices and prefers the requested language', () => {
  const selected = selectLocalSpeechVoice(
    [
      { lang: 'tr-TR', localService: false, default: true },
      { lang: 'en-US', localService: true, default: true },
      { lang: 'tr-TR', localService: true, default: false },
    ],
    'tr-TR',
  );

  assert.equal(selected?.lang, 'tr-TR');
  assert.equal(selected?.localService, true);
});

test('startLocalSpeech reports an unavailable API without invoking callbacks', () => {
  globalThis.window = {};
  const events = [];

  const result = startLocalSpeech('message-1', 'Hello', null, callbacks(events));

  assert.equal(result, 'unsupported');
  assert.deepEqual(events, []);
});

test('startLocalSpeech rejects a voice list without local voices', () => {
  installSpeechHarness([{ lang: 'en-US', localService: false, default: true }]);
  const events = [];

  const result = startLocalSpeech('message-1', 'Hello', 'en-US', callbacks(events));

  assert.equal(result, 'no-local-voice');
  assert.deepEqual(events, []);
});

test('startLocalSpeech uses the selected app language and stopLocalSpeech cancels the active utterance', () => {
  const harness = installSpeechHarness([
    { lang: 'en-US', localService: true, default: true },
    { lang: 'tr-TR', localService: true, default: false },
  ]);
  const events = [];

  const result = startLocalSpeech('message-1', 'Hello', 'tr-TR', callbacks(events));
  const utterance = harness.utterances[0];

  assert.equal(result, 'started');
  assert.equal(utterance?.text, 'Hello');
  assert.equal(utterance?.voice?.lang, 'tr-TR');
  assert.equal(utterance?.lang, 'tr-TR');
  utterance?.onstart?.();
  assert.deepEqual(events, ['start']);

  stopLocalSpeech('message-1');

  assert.equal(harness.cancelCount, 1);
  assert.deepEqual(events, ['start', 'finish']);
});

test('starting another message cancels the previous one and reports runtime errors', () => {
  const harness = installSpeechHarness([{ lang: 'en-US', localService: true, default: true }]);
  const firstEvents = [];
  const secondEvents = [];

  startLocalSpeech('message-1', 'First', 'en-US', callbacks(firstEvents));
  startLocalSpeech('message-2', 'Second', 'en-US', callbacks(secondEvents));

  assert.equal(harness.cancelCount, 1);
  assert.deepEqual(firstEvents, ['finish']);
  const secondUtterance = harness.utterances[1];
  secondUtterance?.onstart?.();
  secondUtterance?.onerror?.();

  assert.deepEqual(secondEvents, ['start', 'error']);
});

test('startLocalSpeech reports synchronous speech runtime failures', () => {
  installSpeechHarness([{ lang: 'en-US', localService: true, default: true }], { throwOnSpeak: true });
  const events = [];

  const result = startLocalSpeech('message-1', 'Hello', 'en-US', callbacks(events));

  assert.equal(result, 'runtime');
  assert.deepEqual(events, []);
});

test('disposeLocalSpeech cancels without notifying callbacks during cleanup', () => {
  const harness = installSpeechHarness([{ lang: 'en-US', localService: true, default: true }]);
  const events = [];

  startLocalSpeech('message-1', 'Hello', 'en-US', callbacks(events));
  disposeLocalSpeech('message-1');

  assert.equal(harness.cancelCount, 1);
  assert.deepEqual(events, []);
});
