export const VOICE_INPUT_SAMPLE_RATE = 16_000;
export const DEFAULT_MAX_RECORDING_DURATION_MS = 30_000;
export const MIN_MAX_RECORDING_DURATION_MS = 1_000;
export const MAX_MAX_RECORDING_DURATION_MS = 120_000;

const maximumLanguageLength = 35;
const languagePattern = /^[A-Za-z0-9]+(?:[-_][A-Za-z0-9]+)*$/u;
const base64Pattern = /^[A-Za-z0-9+/]*={0,2}$/u;

export interface AudioBufferLike {
  readonly length: number;
  readonly numberOfChannels: number;
  readonly sampleRate: number;
  getChannelData(channel: number): Float32Array;
}

export interface VoiceInputPayload {
  pcm16Base64: string;
  sampleRate: typeof VOICE_INPUT_SAMPLE_RATE;
  language: string;
}

/** Normalizes a BCP-47-like language tag before it crosses the local-service boundary. */
export function normalizeVoiceLanguage(language: string): string | null {
  const normalized = language.trim().replaceAll('_', '-');
  if (
    normalized.length === 0 ||
    normalized.length > maximumLanguageLength ||
    !languagePattern.test(normalized)
  ) {
    return null;
  }
  return normalized;
}

/** Keeps capture duration within a bounded in-memory audio budget. */
export function normalizeMaxRecordingDurationMs(durationMs?: number): number {
  if (durationMs === undefined || !Number.isFinite(durationMs)) return DEFAULT_MAX_RECORDING_DURATION_MS;
  return Math.min(MAX_MAX_RECORDING_DURATION_MS, Math.max(MIN_MAX_RECORDING_DURATION_MS, Math.trunc(durationMs)));
}

/** Converts a decoded audio buffer to little-endian, mono PCM16 at the target rate. */
export function encodeAudioBufferToPcm16Base64(
  buffer: AudioBufferLike,
  targetSampleRate = VOICE_INPUT_SAMPLE_RATE,
): string {
  validateAudioBuffer(buffer, targetSampleRate);
  const channels = Array.from({ length: buffer.numberOfChannels }, (_, index) => buffer.getChannelData(index));
  if (channels.some((channel) => channel.length < buffer.length)) {
    throw new Error('The decoded audio channel is shorter than the audio buffer.');
  }

  const targetLength = Math.max(1, Math.round((buffer.length * targetSampleRate) / buffer.sampleRate));
  const samples = new Float32Array(targetLength);
  try {
    for (let targetIndex = 0; targetIndex < targetLength; targetIndex += 1) {
      const sourcePosition = (targetIndex * buffer.sampleRate) / targetSampleRate;
      const sourceIndex = Math.min(buffer.length - 1, Math.floor(sourcePosition));
      const nextSourceIndex = Math.min(buffer.length - 1, sourceIndex + 1);
      const interpolation = sourcePosition - sourceIndex;
      let mixedSample = 0;

      for (const channel of channels) {
        const current = finiteSample(channel[sourceIndex] ?? 0);
        const next = finiteSample(channel[nextSourceIndex] ?? 0);
        mixedSample += current + (next - current) * interpolation;
      }
      samples[targetIndex] = mixedSample / channels.length;
    }

    return encodePcm16Base64(samples);
  } finally {
    samples.fill(0);
    for (const channel of channels) channel.fill(0);
  }
}

/** Encodes normalized floating-point samples as little-endian PCM16 base64. */
export function encodePcm16Base64(samples: ArrayLike<number>): string {
  if (samples.length === 0) throw new Error('The audio sample buffer is empty.');

  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  for (let index = 0; index < samples.length; index += 1) {
    const sample = Math.max(-1, Math.min(1, finiteSample(samples[index] ?? 0)));
    const pcmValue = sample < 0 ? Math.round(sample * 0x8000) : Math.round(sample * 0x7fff);
    view.setInt16(index * 2, pcmValue, true);
  }
  try {
    return bytesToBase64(bytes);
  } finally {
    bytes.fill(0);
  }
}

export function createVoiceInputPayload(buffer: AudioBufferLike, language: string): VoiceInputPayload {
  const normalizedLanguage = normalizeVoiceLanguage(language);
  if (normalizedLanguage === null) throw new Error('The voice input language is invalid.');

  const payload: VoiceInputPayload = {
    pcm16Base64: encodeAudioBufferToPcm16Base64(buffer),
    sampleRate: VOICE_INPUT_SAMPLE_RATE,
    language: normalizedLanguage,
  };
  if (!isVoiceInputPayload(payload)) throw new Error('The voice input payload is invalid.');
  return payload;
}

export function isVoiceInputPayload(value: unknown): value is VoiceInputPayload {
  if (!isRecord(value)) return false;
  return (
    typeof value.pcm16Base64 === 'string' &&
    isBase64(value.pcm16Base64) &&
    value.sampleRate === VOICE_INPUT_SAMPLE_RATE &&
    typeof value.language === 'string' &&
    normalizeVoiceLanguage(value.language) === value.language
  );
}

function validateAudioBuffer(buffer: AudioBufferLike, targetSampleRate: number): void {
  if (
    !Number.isInteger(buffer.length) ||
    buffer.length <= 0 ||
    !Number.isInteger(buffer.numberOfChannels) ||
    buffer.numberOfChannels <= 0 ||
    !Number.isFinite(buffer.sampleRate) ||
    buffer.sampleRate <= 0 ||
    !Number.isInteger(targetSampleRate) ||
    targetSampleRate <= 0
  ) {
    throw new Error('The decoded audio buffer is invalid.');
  }
}

function finiteSample(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

function bytesToBase64(bytes: Uint8Array): string {
  if (typeof btoa !== 'function') throw new Error('Base64 encoding is unavailable.');

  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

function isBase64(value: string): boolean {
  return value.length > 0 && value.length % 4 === 0 && base64Pattern.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
