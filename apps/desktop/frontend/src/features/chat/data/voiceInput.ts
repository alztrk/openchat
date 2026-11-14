import type { JsonObject } from '@shared/lib/platform/local-service';

/** Extracts the transcript returned by the local sidecar and drops surrounding whitespace. */
export function parseVoiceInputTranscription(value: JsonObject): string {
  if (typeof value.text !== 'string') throw new Error('The local voice input transcription is invalid.');
  return value.text.trim();
}

/** Appends a non-empty transcript without submitting or rewriting the existing draft. */
export function appendVoiceTranscriptToDraft(draft: string, transcript: string): string {
  const addition = transcript.trim();
  if (addition.length === 0) return draft;
  if (draft.length === 0) return addition;
  return /\s$/u.test(draft) ? `${draft}${addition}` : `${draft}\n${addition}`;
}

/** Converts an i18n locale into the base language identifier accepted by Whisper. */
export function baseLanguageCode(language: string): string | null {
  const firstSegment = language.trim().split(/[-_]/u, 1)[0] ?? '';
  return /^[A-Za-z]{2,8}$/u.test(firstSegment) ? firstSegment.toLowerCase() : null;
}
