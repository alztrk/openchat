import { readNotificationPreferences } from '@/modules/notifications/data/notificationPreferences';

let sharedAudioContext: AudioContext | null = null;

export function getAudioContext(): AudioContext | null {
  if (typeof window === 'undefined') return null;
  const AudioContextClass =
    window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioContextClass) return null;

  if (!sharedAudioContext || sharedAudioContext.state === 'closed') {
    try {
      sharedAudioContext = new AudioContextClass();
    } catch {
      return null;
    }
  }
  return sharedAudioContext;
}

export async function prepareAudioContext(ctx: AudioContext): Promise<boolean> {
  if (ctx.state === 'suspended') {
    try {
      await ctx.resume();
    } catch {
      return false;
    }
  }
  return ctx.state === 'running';
}

/**
 * Synthesizes a subtle, harmonic two-tone chime for completed responses.
 */
export async function playCompletionChime(volume = 0.7): Promise<void> {
  const ctx = getAudioContext();
  if (!ctx) return;
  const ready = await prepareAudioContext(ctx);
  if (!ready) return;

  const clampedVolume = Math.max(0, Math.min(1, volume));
  if (clampedVolume <= 0) return;

  const now = ctx.currentTime;
  const masterGain = ctx.createGain();
  const peakGain = clampedVolume * 0.22;
  masterGain.gain.setValueAtTime(peakGain, now);
  masterGain.connect(ctx.destination);

  // Tone 1: E5 (659.25 Hz)
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

  // Tone 2: B5 (987.77 Hz)
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
}

/**
 * Synthesizes a gentle, soft alert for generation errors and task blockers.
 */
export async function playFailureAlert(volume = 0.7): Promise<void> {
  const ctx = getAudioContext();
  if (!ctx) return;
  const ready = await prepareAudioContext(ctx);
  if (!ready) return;

  const clampedVolume = Math.max(0, Math.min(1, volume));
  if (clampedVolume <= 0) return;

  const now = ctx.currentTime;
  const masterGain = ctx.createGain();
  const peakGain = clampedVolume * 0.2;
  masterGain.gain.setValueAtTime(peakGain, now);
  masterGain.connect(ctx.destination);

  // Tone 1: 415.30 Hz (Ab4)
  const osc1 = ctx.createOscillator();
  const gain1 = ctx.createGain();
  osc1.type = 'sine';
  osc1.frequency.setValueAtTime(415.3, now);

  gain1.gain.setValueAtTime(0.0001, now);
  gain1.gain.exponentialRampToValueAtTime(1.0, now + 0.015);
  gain1.gain.exponentialRampToValueAtTime(0.0001, now + 0.22);

  osc1.connect(gain1);
  gain1.connect(masterGain);

  osc1.start(now);
  osc1.stop(now + 0.25);

  // Tone 2: 311.13 Hz (Eb4)
  const t2 = now + 0.13;
  const osc2 = ctx.createOscillator();
  const gain2 = ctx.createGain();
  osc2.type = 'sine';
  osc2.frequency.setValueAtTime(311.13, t2);

  gain2.gain.setValueAtTime(0.0001, t2);
  gain2.gain.exponentialRampToValueAtTime(0.9, t2 + 0.015);
  gain2.gain.exponentialRampToValueAtTime(0.0001, t2 + 0.35);

  osc2.connect(gain2);
  gain2.connect(masterGain);

  osc2.start(t2);
  osc2.stop(t2 + 0.38);
}

/**
 * Plays audio feedback according to current user preferences and status.
 */
export async function playAssistantResponseAudio(status: 'completed' | 'failed'): Promise<void> {
  try {
    const preferences = readNotificationPreferences();
    if (preferences.soundMuted) return;
    if (preferences.soundVolume <= 0) return;

    if (status === 'completed') {
      if (!preferences.soundEnabled) return;
      await playCompletionChime(preferences.soundVolume);
    } else if (status === 'failed') {
      if (!preferences.failureSoundEnabled) return;
      await playFailureAlert(preferences.soundVolume);
    }
  } catch {
    // Audio feedback should fail silently without interrupting conversation flow
  }
}

/**
 * Plays a preview chime using current or custom volume.
 */
export async function playTestChime(volume?: number): Promise<void> {
  try {
    const effectiveVolume = volume ?? readNotificationPreferences().soundVolume;
    await playCompletionChime(effectiveVolume);
  } catch {
    // Preview failure handled silently
  }
}
