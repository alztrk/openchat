export type SpeechPlaybackStartResult =
  | 'started'
  | 'empty'
  | 'unsupported'
  | 'no-local-voice'
  | 'runtime';

export interface SpeechVoiceLike {
  lang: string;
  localService: boolean;
  default: boolean;
}

export interface SpeechPlaybackCallbacks {
  onStart(): void;
  onFinish(): void;
  onError(): void;
}

interface ActiveSpeech {
  messageId: string;
  synthesis: SpeechSynthesis;
  utterance: SpeechSynthesisUtterance;
  callbacks: SpeechPlaybackCallbacks;
}

let activeSpeech: ActiveSpeech | null = null;

export function selectLocalSpeechVoice<T extends SpeechVoiceLike>(
  voices: readonly T[],
  preferredLanguage: string | null = null,
): T | null {
  const localVoices = voices.filter((voice) => voice.localService);
  if (localVoices.length === 0) return null;

  const normalizedLanguage = normalizeLanguage(preferredLanguage);
  if (normalizedLanguage !== null) {
    const exactMatch = localVoices.find((voice) => normalizeLanguage(voice.lang) === normalizedLanguage);
    if (exactMatch) return exactMatch;

    const languagePrefix = normalizedLanguage.split('-')[0];
    const languageMatch = localVoices.find((voice) => normalizeLanguage(voice.lang)?.split('-')[0] === languagePrefix);
    if (languageMatch) return languageMatch;
  }

  return localVoices.find((voice) => voice.default) ?? localVoices[0] ?? null;
}

export function startLocalSpeech(
  messageId: string,
  text: string,
  preferredLanguage: string | null,
  callbacks: SpeechPlaybackCallbacks,
): SpeechPlaybackStartResult {
  const normalizedText = text.trim();
  if (!normalizedText) return 'empty';

  const synthesis = getSpeechSynthesis();
  if (!synthesis) return 'unsupported';

  const voice = selectLocalSpeechVoice(synthesis.getVoices(), preferredLanguage);
  if (!voice) return 'no-local-voice';

  stopLocalSpeech();

  let utterance: SpeechSynthesisUtterance;
  try {
    utterance = new SpeechSynthesisUtterance(normalizedText);
    utterance.voice = voice;
    utterance.lang = voice.lang;
  } catch {
    return 'runtime';
  }

  const speech: ActiveSpeech = {
    messageId,
    synthesis,
    utterance,
    callbacks,
  };
  utterance.onstart = () => {
    if (activeSpeech?.utterance === utterance) callbacks.onStart();
  };
  utterance.onend = () => finishSpeech(utterance, false);
  utterance.onerror = () => finishSpeech(utterance, true);
  activeSpeech = speech;

  try {
    synthesis.speak(utterance);
  } catch {
    if (activeSpeech?.utterance === utterance) activeSpeech = null;
    return 'runtime';
  }

  return 'started';
}

export function stopLocalSpeech(messageId?: string): void {
  clearActiveSpeech(messageId, true);
}

export function disposeLocalSpeech(messageId?: string): void {
  clearActiveSpeech(messageId, false);
}

function clearActiveSpeech(messageId: string | undefined, notify: boolean): void {
  const speech = activeSpeech;
  if (!speech || (messageId !== undefined && speech.messageId !== messageId)) return;

  activeSpeech = null;
  try {
    speech.synthesis.cancel();
  } catch {
    if (notify) speech.callbacks.onError();
    return;
  }
  if (notify) speech.callbacks.onFinish();
}

function finishSpeech(utterance: SpeechSynthesisUtterance, hasError: boolean): void {
  const speech = activeSpeech;
  if (!speech || speech.utterance !== utterance) return;

  activeSpeech = null;
  if (hasError) {
    speech.callbacks.onError();
  } else {
    speech.callbacks.onFinish();
  }
}

function getSpeechSynthesis(): SpeechSynthesis | null {
  if (typeof window === 'undefined' || typeof window.speechSynthesis === 'undefined') return null;
  return window.speechSynthesis;
}

function normalizeLanguage(language: string | null): string | null {
  const normalized = language?.trim().toLowerCase().replaceAll('_', '-');
  return normalized ? normalized : null;
}
