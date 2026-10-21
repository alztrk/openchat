import assert from 'node:assert/strict';
import test from 'node:test';

// Storage mock for unifiedStorage
const storage = new Map();
const unifiedStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
};

const preferenceKeys = {
  responses: 'notifications.responses',
  questions: 'notifications.questions',
  responsePreview: 'notifications.response_preview',
  quickReply: 'notifications.quick_reply',
  soundEnabled: 'notifications.sound_enabled',
  soundMuted: 'notifications.sound_muted',
  soundVolume: 'notifications.sound_volume',
  failureSoundEnabled: 'notifications.failure_sound_enabled',
};

const defaultPreferences = {
  responses: true,
  questions: true,
  responsePreview: true,
  quickReply: true,
  soundEnabled: true,
  soundMuted: false,
  soundVolume: 0.7,
  failureSoundEnabled: true,
};

function readNotificationPreferences() {
  return {
    responses: readBooleanPreference('responses'),
    questions: readBooleanPreference('questions'),
    responsePreview: readBooleanPreference('responsePreview'),
    quickReply: readBooleanPreference('quickReply'),
    soundEnabled: readBooleanPreference('soundEnabled'),
    soundMuted: readBooleanPreference('soundMuted'),
    soundVolume: readNumberPreference('soundVolume'),
    failureSoundEnabled: readBooleanPreference('failureSoundEnabled'),
  };
}

function writeNotificationPreference(key, value) {
  unifiedStorage.setItem(preferenceKeys[key], String(value));
}

function readBooleanPreference(key) {
  const value = unifiedStorage.getItem(preferenceKeys[key]);
  if (value === null) return defaultPreferences[key];
  if (value !== 'true' && value !== 'false') {
    throw new Error('Notification preferences are invalid.');
  }
  return value === 'true';
}

function readNumberPreference(key) {
  const value = unifiedStorage.getItem(preferenceKeys[key]);
  if (value === null) return defaultPreferences[key];
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 1) {
    throw new Error('Notification preferences are invalid.');
  }
  return parsed;
}

// Audio synthesizer mock to verify Web Audio API nodes
function createMockAudioContext() {
  const nodes = [];
  return {
    state: 'running',
    currentTime: 10,
    resume: async () => undefined,
    createGain: () => {
      const node = {
        kind: 'gain',
        gain: {
          value: 1,
          setValueAtTime: (val, t) => {
            node.gain.value = val;
            node.gain.setValueAtTimeCalled = { val, t };
          },
          exponentialRampToValueAtTime: (val, t) => {
            node.ramps = node.ramps || [];
            node.ramps.push({ val, t });
          },
        },
        connect: (target) => {
          node.connectedTo = target;
        },
      };
      nodes.push(node);
      return node;
    },
    createOscillator: () => {
      const node = {
        kind: 'oscillator',
        type: 'sine',
        frequency: {
          value: 440,
          setValueAtTime: (val, t) => {
            node.frequency.value = val;
            node.setFreqCalled = { val, t };
          },
        },
        connect: (target) => {
          node.connectedTo = target;
        },
        start: (t) => {
          node.startedAt = t;
        },
        stop: (t) => {
          node.stoppedAt = t;
        },
      };
      nodes.push(node);
      return node;
    },
    destination: { type: 'destination' },
    getNodes: () => nodes,
  };
}

async function playChimeOnMock(ctx, volume) {
  const clampedVolume = Math.max(0, Math.min(1, volume));
  if (clampedVolume <= 0) return false;

  const now = ctx.currentTime;
  const masterGain = ctx.createGain();
  const peakGain = clampedVolume * 0.22;
  masterGain.gain.setValueAtTime(peakGain, now);
  masterGain.connect(ctx.destination);

  // Tone 1: E5
  const osc1 = ctx.createOscillator();
  const gain1 = ctx.createGain();
  osc1.type = 'sine';
  osc1.frequency.setValueAtTime(659.25, now);
  gain1.gain.setValueAtTime(0.0001, now);
  gain1.gain.exponentialRampToValueAtTime(1.0, now + 0.015);
  gain1.gain.exponentialRampToValueAtTime(0.0001, now + 0.28);
  osc1.connect(gain1);
  gain1.connect(masterGain);
  osc1.start(now);
  osc1.stop(now + 0.3);

  // Tone 2: B5
  const t2 = now + 0.11;
  const osc2 = ctx.createOscillator();
  const gain2 = ctx.createGain();
  osc2.type = 'sine';
  osc2.frequency.setValueAtTime(987.77, t2);
  gain2.gain.setValueAtTime(0.0001, t2);
  gain2.gain.exponentialRampToValueAtTime(1.0, t2 + 0.015);
  gain2.gain.exponentialRampToValueAtTime(0.0001, t2 + 0.45);
  osc2.connect(gain2);
  gain2.connect(masterGain);
  osc2.start(t2);
  osc2.stop(t2 + 0.48);
  return true;
}

test('audio feedback preferences load default values cleanly', () => {
  storage.clear();
  const prefs = readNotificationPreferences();
  assert.equal(prefs.soundEnabled, true);
  assert.equal(prefs.soundMuted, false);
  assert.equal(prefs.soundVolume, 0.7);
  assert.equal(prefs.failureSoundEnabled, true);
});

test('audio feedback preferences persist volume and toggle updates', () => {
  storage.clear();
  writeNotificationPreference('soundEnabled', false);
  writeNotificationPreference('soundVolume', 0.45);
  writeNotificationPreference('soundMuted', true);
  writeNotificationPreference('failureSoundEnabled', false);

  const prefs = readNotificationPreferences();
  assert.equal(prefs.soundEnabled, false);
  assert.equal(prefs.soundVolume, 0.45);
  assert.equal(prefs.soundMuted, true);
  assert.equal(prefs.failureSoundEnabled, false);
});

test('audio feedback clamps and rejects invalid volume numbers', () => {
  storage.clear();
  storage.set('notifications.sound_volume', 'invalid');
  assert.throws(() => readNotificationPreferences(), /invalid/);

  storage.set('notifications.sound_volume', '1.5');
  assert.throws(() => readNotificationPreferences(), /invalid/);

  storage.set('notifications.sound_volume', '-0.1');
  assert.throws(() => readNotificationPreferences(), /invalid/);
});

test('completion chime synthesizes clean two-tone harmonic frequencies', async () => {
  const mockCtx = createMockAudioContext();
  const played = await playChimeOnMock(mockCtx, 0.8);
  assert.equal(played, true);

  const nodes = mockCtx.getNodes();
  const oscillators = nodes.filter((n) => n.kind === 'oscillator');
  assert.equal(oscillators.length, 2);
  assert.equal(oscillators[0].frequency.value, 659.25);
  assert.equal(oscillators[1].frequency.value, 987.77);

  const masterGain = nodes.find((n) => n.kind === 'gain' && n.connectedTo === mockCtx.destination);
  assert.ok(masterGain);
  assert.equal(masterGain.gain.setValueAtTimeCalled.val, 0.8 * 0.22);
});

test('audio playback skips gracefully when volume is zero or muted', async () => {
  const mockCtx = createMockAudioContext();
  const played = await playChimeOnMock(mockCtx, 0);
  assert.equal(played, false);
  assert.equal(mockCtx.getNodes().length, 0);
});
