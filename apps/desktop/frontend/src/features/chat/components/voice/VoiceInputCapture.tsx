import { Check, LoaderCircle, Mic, Pause, Play, Square, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { Button } from '@shared/components';
import { readPreferredAudioInputDeviceId } from '@shared/lib/preferences/audioDevicePreferences';
import { cn } from '@shared/lib/utils/cn';

import {
  createVoiceInputPayload,
  normalizeMaxRecordingDurationMs,
  normalizeVoiceLanguage,
  type VoiceInputPayload,
} from './audioEncoding';

export type VoiceInputCaptureState = 'idle' | 'requesting-permission' | 'recording' | 'paused' | 'reviewing' | 'processing';

export type VoiceInputErrorCode =
  | 'unsupported'
  | 'invalid-language'
  | 'permission-denied'
  | 'microphone-unavailable'
  | 'capture-in-use'
  | 'capture-failed'
  | 'playback-failed'
  | 'empty-audio'
  | 'encoding-failed'
  | 'callback-failed';

export interface VoiceInputError {
  code: VoiceInputErrorCode;
  cause?: unknown;
}

export interface VoiceInputCaptureLabels {
  startRecording: string;
  requestingPermission: string;
  recording: string;
  paused: string;
  processing: string;
  cancelProcessing: string;
  close: string;
  pauseRecording: string;
  resumeRecording: string;
  finishRecording: string;
  playRecording: string;
  pausePlayback: string;
  confirmTranscription: string;
  listenBeforeTranscribing: string;
  audioLevel: string;
  speechDetected: string;
  silenceDetected: string;
}

export interface VoiceInputCaptureProps {
  language: string;
  labels: VoiceInputCaptureLabels;
  onComplete(payload: VoiceInputPayload): void | Promise<void>;
  onError?(error: VoiceInputError): void;
  onProcessingCancel?(): void;
  onExclusiveModeChange?(active: boolean): void;
  overlayContainer?: HTMLElement | null;
  disabled?: boolean;
  maxDurationMs?: number;
  className?: string;
}

interface CaptureSession {
  id: number;
  ownerToken: symbol;
  language: string;
  stream: MediaStream;
  recorder: MediaRecorder;
  audioContext: AudioContext | null;
  source: MediaStreamAudioSourceNode;
  analyser: AnalyserNode;
  analyserData: Uint8Array<ArrayBuffer> | null;
  chunks: Blob[];
  reviewBlob: Blob | null;
  frameId: number | null;
  timerId: number | null;
  stopWatchdogId: number | null;
  stopRequested: boolean;
  cancelled: boolean;
  finished: boolean;
  resourcesDisposed: boolean;
}

interface CaptureStartAttempt {
  ownerToken: symbol;
  cancelled: boolean;
}

const audioLevelSpeechThreshold = 0.08;
const audioLevelSmoothing = 0.25;
const captureMimeTypes = ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus'];
const maximumBlobBytes = 16 * 1024 * 1024;
const stopEventTimeoutMs = 2_000;
let activeCaptureOwner: symbol | null = null;

declare global {
  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}

export function VoiceInputCapture({
  language,
  labels,
  onComplete,
  onError,
  onProcessingCancel,
  onExclusiveModeChange,
  overlayContainer,
  disabled = false,
  maxDurationMs,
  className,
}: VoiceInputCaptureProps) {
  const [captureState, setCaptureState] = useState<VoiceInputCaptureState>('idle');
  const [audioLevel, setAudioLevel] = useState(0);
  const [speechDetected, setSpeechDetected] = useState(false);
  const [reviewUrl, setReviewUrl] = useState<string | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [hasListenedToRecording, setHasListenedToRecording] = useState(false);
  const audioElementRef = useRef<HTMLAudioElement | null>(null);
  const playbackFailureReportedRef = useRef(false);
  const idleButtonRef = useRef<HTMLButtonElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const mountedRef = useRef(true);
  const sessionRef = useRef<CaptureSession | null>(null);
  const startAttemptRef = useRef<CaptureStartAttempt | null>(null);
  const nextSessionIdRef = useRef(0);
  const audioLevelRef = useRef(0);
  const onCompleteRef = useRef(onComplete);
  const onErrorRef = useRef(onError);

  useEffect(() => {
    onCompleteRef.current = onComplete;
    onErrorRef.current = onError;
  }, [onComplete, onError]);

  useEffect(() => {
    onExclusiveModeChange?.(captureState !== 'idle');
  }, [captureState, onExclusiveModeChange]);

  const wasActiveRef = useRef(false);
  useEffect(() => {
    const isActive = captureState !== 'idle';
    if (isActive && !wasActiveRef.current) {
      closeButtonRef.current?.focus();
    } else if (!isActive && wasActiveRef.current) {
      window.requestAnimationFrame(() => idleButtonRef.current?.focus());
    }
    wasActiveRef.current = isActive;
  }, [captureState]);

  const resetVisibleState = useCallback(() => {
    if (!mountedRef.current) return;
    setCaptureState('idle');
    setAudioLevel(0);
    setSpeechDetected(false);
    setReviewUrl(null);
    setIsPlaying(false);
    setHasListenedToRecording(false);
  }, []);

  const reportError = useCallback((error: VoiceInputError) => {
    if (mountedRef.current) onErrorRef.current?.(error);
  }, []);

  const reportPlaybackFailure = useCallback(
    (cause?: unknown) => {
      if (playbackFailureReportedRef.current) return;
      playbackFailureReportedRef.current = true;
      reportError({ code: 'playback-failed', ...(cause === undefined ? {} : { cause }) });
    },
    [reportError],
  );

  useEffect(() => {
    if (!reviewUrl) return;
    const audioElement = new Audio();
    audioElement.preload = 'none';
    audioElement.src = reviewUrl;
    const handlePlaybackStateChange = () => setIsPlaying(!audioElement.paused);
    const handlePlaybackEnd = () => {
      setIsPlaying(false);
      setHasListenedToRecording(true);
    };
    const handlePlaybackError = () => reportPlaybackFailure(audioElement.error);
    playbackFailureReportedRef.current = false;
    audioElement.addEventListener('play', handlePlaybackStateChange);
    audioElement.addEventListener('pause', handlePlaybackStateChange);
    audioElement.addEventListener('ended', handlePlaybackEnd);
    audioElement.addEventListener('error', handlePlaybackError);
    audioElementRef.current = audioElement;
    return () => {
      audioElement.pause();
      audioElement.removeEventListener('play', handlePlaybackStateChange);
      audioElement.removeEventListener('pause', handlePlaybackStateChange);
      audioElement.removeEventListener('ended', handlePlaybackEnd);
      audioElement.removeEventListener('error', handlePlaybackError);
      audioElement.removeAttribute('src');
      audioElement.load();
      if (audioElementRef.current === audioElement) audioElementRef.current = null;
      URL.revokeObjectURL(reviewUrl);
    };
  }, [reportPlaybackFailure, reviewUrl]);

  const clearCurrentSession = useCallback(
    (session: CaptureSession, reset: () => void) => {
      if (sessionRef.current?.id !== session.id) return;
      sessionRef.current = null;
      reset();
    },
    [],
  );

  const finalizeSession = useCallback(
    (session: CaptureSession): void => {
      if (session.finished) return;
      session.finished = true;

      if (session.cancelled) {
        disposeSession(session);
        clearCurrentSession(session, resetVisibleState);
        return;
      }

      try {
        if (session.chunks.length === 0) throw new VoiceInputFailure('empty-audio');
        const blob = new Blob(session.chunks, { type: session.recorder.mimeType || 'audio/webm' });
        if (blob.size === 0) throw new VoiceInputFailure('empty-audio');
        if (blob.size > maximumBlobBytes) throw new VoiceInputFailure('capture-failed');
        session.reviewBlob = blob;
        disposeSession(session);
        if (!mountedRef.current || sessionRef.current?.id !== session.id) return;
        setAudioLevel(0);
        setSpeechDetected(false);
        setHasListenedToRecording(false);
        setReviewUrl(URL.createObjectURL(blob));
        setCaptureState('reviewing');
      } catch (cause) {
        disposeSession(session);
        clearCurrentSession(session, resetVisibleState);
        if (cause instanceof VoiceInputFailure) {
          reportError({ code: cause.code, cause: cause.cause });
        } else {
          reportError({ code: 'encoding-failed', cause });
        }
      }
    },
    [clearCurrentSession, reportError, resetVisibleState],
  );

  const failSession = useCallback(
    (session: CaptureSession, code: VoiceInputErrorCode, cause?: unknown) => {
      if (session.finished) return;
      session.finished = true;
      session.cancelled = true;
      disposeSession(session);
      clearCurrentSession(session, resetVisibleState);
      reportError({ code, cause });
    },
    [clearCurrentSession, reportError, resetVisibleState],
  );

  const updateAudioLevel = useCallback((session: CaptureSession) => {
    if (!mountedRef.current || sessionRef.current?.id !== session.id || session.finished) return;
    const data = session.analyserData;
    if (!data) return;

    if (session.recorder.state !== 'recording') return;
    session.analyser.getByteTimeDomainData(data);
    let squaredTotal = 0;
    for (const value of data) {
      const normalized = (value - 128) / 128;
      squaredTotal += normalized * normalized;
    }
    const rawLevel = Math.min(1, Math.sqrt(squaredTotal / data.length) * 4);
    const previousLevel = audioLevelRef.current;
    const nextLevel = previousLevel + (rawLevel - previousLevel) * audioLevelSmoothing;
    audioLevelRef.current = nextLevel;
    setAudioLevel(nextLevel);
    setSpeechDetected(nextLevel >= audioLevelSpeechThreshold);
    session.frameId = window.requestAnimationFrame(() => updateAudioLevel(session));
  }, []);

  const requestStop = useCallback(
    (session: CaptureSession, cancelled: boolean) => {
      if (session.finished || session.stopRequested) return;
      session.stopRequested = true;
      session.cancelled = cancelled;
      if (session.timerId !== null) {
        window.clearTimeout(session.timerId);
        session.timerId = null;
      }

      if (session.recorder.state === 'inactive') {
        void finalizeSession(session);
        return;
      }

      session.stopWatchdogId = window.setTimeout(() => {
        session.stopWatchdogId = null;
        if (session.cancelled) {
          session.finished = true;
          disposeSession(session);
          clearCurrentSession(session, resetVisibleState);
          return;
        }
        failSession(session, 'capture-failed', new Error('The recorder did not emit a stop event.'));
      }, stopEventTimeoutMs);

      try {
        session.recorder.stop();
      } catch (cause) {
        if (cancelled) {
          session.finished = true;
          disposeSession(session);
          clearCurrentSession(session, resetVisibleState);
        } else {
          failSession(session, 'capture-failed', cause);
        }
      }
    },
    [clearCurrentSession, failSession, finalizeSession, resetVisibleState],
  );

  const startRecording = useCallback(async () => {
    if (
      !mountedRef.current ||
      disabled ||
      captureState !== 'idle' ||
      sessionRef.current !== null ||
      startAttemptRef.current !== null
    ) return;
    const normalizedLanguage = normalizeVoiceLanguage(language);
    if (normalizedLanguage === null) {
      reportError({ code: 'invalid-language' });
      return;
    }
    if (typeof navigator.mediaDevices?.getUserMedia !== 'function' || typeof MediaRecorder === 'undefined') {
      reportError({ code: 'unsupported' });
      return;
    }

    if (activeCaptureOwner !== null) {
      reportError({ code: 'capture-in-use' });
      return;
    }

    const ownerToken = Symbol('voice-input-capture');
    const attempt: CaptureStartAttempt = { ownerToken, cancelled: false };
    activeCaptureOwner = ownerToken;
    startAttemptRef.current = attempt;
    const isAttemptActive = () =>
      mountedRef.current &&
      !attempt.cancelled &&
      startAttemptRef.current === attempt &&
      activeCaptureOwner === ownerToken;

    setCaptureState('requesting-permission');
    let stream: MediaStream | undefined;
    let audioContext: AudioContext | undefined;
    let source: MediaStreamAudioSourceNode | undefined;
    let analyser: AnalyserNode | undefined;
    let recorder: MediaRecorder | undefined;
    let session: CaptureSession | undefined;
    try {
      const preferredDeviceId = readPreferredAudioInputDeviceId();
      stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
          ...(preferredDeviceId ? { deviceId: { exact: preferredDeviceId } } : {}),
        },
      });
      if (!isAttemptActive()) {
        stopTracks(stream);
        releaseCaptureOwner(ownerToken);
        if (startAttemptRef.current === attempt) startAttemptRef.current = null;
        resetVisibleState();
        return;
      }

      audioContext = createAudioContext();
      await audioContext.resume();
      if (!isAttemptActive()) {
        stopTracks(stream);
        closeAudioContext(audioContext);
        releaseCaptureOwner(ownerToken);
        if (startAttemptRef.current === attempt) startAttemptRef.current = null;
        resetVisibleState();
        return;
      }

      source = audioContext.createMediaStreamSource(stream);
      analyser = audioContext.createAnalyser();
      analyser.fftSize = 1024;
      source.connect(analyser);
      recorder = createMediaRecorder(stream);
      const activeSession: CaptureSession = {
        id: nextSessionIdRef.current + 1,
        ownerToken,
        language: normalizedLanguage,
        stream,
        recorder,
        audioContext,
        source,
        analyser,
        analyserData: new Uint8Array(new ArrayBuffer(analyser.fftSize)),
        chunks: [],
        reviewBlob: null,
        frameId: null,
        timerId: null,
        stopWatchdogId: null,
        stopRequested: false,
        cancelled: false,
        finished: false,
        resourcesDisposed: false,
      };
      session = activeSession;
      nextSessionIdRef.current = activeSession.id;
      sessionRef.current = activeSession;
      startAttemptRef.current = null;
      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) activeSession.chunks.push(event.data);
      };
      recorder.onerror = () => failSession(activeSession, 'capture-failed');
      recorder.onstop = () => {
        if (activeSession.stopWatchdogId !== null) {
          window.clearTimeout(activeSession.stopWatchdogId);
          activeSession.stopWatchdogId = null;
        }
        stopTracks(activeSession.stream);
        releaseCaptureOwner(activeSession.ownerToken);
        void finalizeSession(activeSession);
      };

      recorder.start(250);
      setCaptureState('recording');
      audioLevelRef.current = 0;
      activeSession.frameId = window.requestAnimationFrame(() => updateAudioLevel(activeSession));
      activeSession.timerId = window.setTimeout(
        () => requestStop(activeSession, false),
        normalizeMaxRecordingDurationMs(maxDurationMs),
      );
    } catch (cause) {
      if (session) {
        session.finished = true;
        disposeSession(session);
        if (sessionRef.current?.id === session.id) sessionRef.current = null;
      } else {
        if (recorder?.state === 'recording') {
          try {
            recorder.stop();
          } catch (cleanupCause) {
            reportCleanupFailure(cleanupCause);
          }
        }
        if (source) {
          try {
            source.disconnect();
          } catch (cleanupCause) {
            reportCleanupFailure(cleanupCause);
          }
        }
        if (analyser) {
          try {
            analyser.disconnect();
          } catch (cleanupCause) {
            reportCleanupFailure(cleanupCause);
          }
        }
        if (stream) stopTracks(stream);
        if (audioContext) closeAudioContext(audioContext);
        releaseCaptureOwner(ownerToken);
      }
      if (startAttemptRef.current === attempt) startAttemptRef.current = null;
      if (mountedRef.current) {
        setCaptureState('idle');
        if (!attempt.cancelled) reportError({ ...mapCaptureError(cause), cause });
      }
    }
  }, [
    captureState,
    disabled,
    failSession,
    finalizeSession,
    language,
    maxDurationMs,
    reportError,
    requestStop,
    resetVisibleState,
    updateAudioLevel,
  ]);

  const stopRecording = useCallback(() => {
    const session = sessionRef.current;
    if (session) requestStop(session, false);
  }, [requestStop]);

  const pauseRecording = useCallback(() => {
    const session = sessionRef.current;
    if (!session || captureState !== 'recording') return;
    try {
      session.recorder.pause();
      if (session.frameId !== null) {
        window.cancelAnimationFrame(session.frameId);
        session.frameId = null;
      }
      setCaptureState('paused');
    } catch (cause) {
      failSession(session, 'capture-failed', cause);
    }
  }, [captureState, failSession]);

  const resumeRecording = useCallback(() => {
    const session = sessionRef.current;
    if (!session || captureState !== 'paused') return;
    try {
      session.recorder.resume();
      setCaptureState('recording');
      session.frameId = window.requestAnimationFrame(() => updateAudioLevel(session));
    } catch (cause) {
      failSession(session, 'capture-failed', cause);
    }
  }, [captureState, failSession, updateAudioLevel]);

  const cancelPermissionRequest = useCallback(() => {
    const attempt = startAttemptRef.current;
    if (!attempt) return;
    attempt.cancelled = true;
    startAttemptRef.current = null;
    releaseCaptureOwner(attempt.ownerToken);
    resetVisibleState();
  }, [resetVisibleState]);

  const closeCapture = useCallback(() => {
    audioElementRef.current?.pause();
    setIsPlaying(false);
    if (captureState === 'requesting-permission') {
      cancelPermissionRequest();
      return;
    }

    const session = sessionRef.current;
    if (session && !session.finished) {
      session.cancelled = true;
      if (!session.stopRequested) requestStop(session, true);
      return;
    }

    if (session) {
      session.cancelled = true;
      session.reviewBlob = null;
      disposeSession(session);
      clearCurrentSession(session, resetVisibleState);
    } else {
      resetVisibleState();
    }

    if (!session && captureState === 'processing') onProcessingCancel?.();
  }, [cancelPermissionRequest, captureState, clearCurrentSession, onProcessingCancel, requestStop, resetVisibleState]);

  const confirmTranscription = useCallback(async () => {
    const session = sessionRef.current;
    const blob = session?.reviewBlob;
    if (!session || !blob || captureState !== 'reviewing') return;

    setCaptureState('processing');
    audioElementRef.current?.pause();
    setReviewUrl(null);
    let audioContext: AudioContext | null = null;
    let encodedAudio: ArrayBuffer | null = null;
    try {
      encodedAudio = await blob.arrayBuffer();
      if (!mountedRef.current || session.cancelled || sessionRef.current?.id !== session.id) return;
      audioContext = createAudioContext();
      const decoded = await audioContext.decodeAudioData(encodedAudio);
      if (encodedAudio.byteLength > 0) new Uint8Array(encodedAudio).fill(0);
      encodedAudio = null;

      if (!mountedRef.current || session.cancelled || sessionRef.current?.id !== session.id) return;

      const payload = createVoiceInputPayload(decoded, session.language);
      session.reviewBlob = null;
      disposeSession(session);
      sessionRef.current = null;
      setAudioLevel(0);
      setSpeechDetected(false);
      try {
        await onCompleteRef.current(payload);
      } catch (cause) {
        reportError({ code: 'callback-failed', cause });
      } finally {
        resetVisibleState();
      }
    } catch (cause) {
      if (!session.cancelled) {
        session.reviewBlob = null;
        disposeSession(session);
        clearCurrentSession(session, resetVisibleState);
        if (cause instanceof VoiceInputFailure) {
          reportError({ code: cause.code, cause: cause.cause });
        } else {
          reportError({ code: 'encoding-failed', cause });
        }
      }
    } finally {
      if (encodedAudio && encodedAudio.byteLength > 0) new Uint8Array(encodedAudio).fill(0);
      if (audioContext) closeAudioContext(audioContext);
    }
  }, [captureState, clearCurrentSession, reportError, resetVisibleState]);

  const togglePlayback = useCallback(async () => {
    const audioElement = audioElementRef.current;
    if (!audioElement) return;
    if (!audioElement.paused) {
      audioElement.pause();
      return;
    }
    playbackFailureReportedRef.current = false;
    try {
      await audioElement.play();
    } catch (cause) {
      reportPlaybackFailure(cause);
    }
  }, [reportPlaybackFailure]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const attempt = startAttemptRef.current;
      if (attempt) {
        attempt.cancelled = true;
        startAttemptRef.current = null;
        releaseCaptureOwner(attempt.ownerToken);
      }
      const session = sessionRef.current;
      if (!session) return;
      session.finished = true;
      session.cancelled = true;
      disposeSession(session);
      sessionRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cancelWhenHidden = () => {
      if (!document.hidden) return;
      const attempt = startAttemptRef.current;
      if (attempt) {
        attempt.cancelled = true;
        startAttemptRef.current = null;
        releaseCaptureOwner(attempt.ownerToken);
        resetVisibleState();
      }
      const session = sessionRef.current;
      if (session) {
        if (session.finished) closeCapture();
        else if (session.stopRequested) session.cancelled = true;
        else requestStop(session, true);
      } else if (captureState === 'processing') {
        closeCapture();
      }
    };
    document.addEventListener('visibilitychange', cancelWhenHidden);
    return () => document.removeEventListener('visibilitychange', cancelWhenHidden);
  }, [captureState, closeCapture, requestStop, resetVisibleState]);

  const statusLabel =
    captureState === 'requesting-permission'
      ? labels.requestingPermission
      : captureState === 'recording'
        ? labels.recording
        : captureState === 'paused'
          ? labels.paused
          : labels.processing;

  const overlay = (
    <div
      aria-busy={captureState === 'requesting-permission' || captureState === 'processing'}
      className="voice-input-exclusive-overlay"
      data-speech-active={captureState === 'recording' && speechDetected}
      data-voice-input-state={captureState}
    >
      <Button
        aria-label={labels.close}
        ref={closeButtonRef}
        onClick={closeCapture}
        size="icon"
        title={labels.close}
        type="button"
        variant="ghost"
      >
        <X aria-hidden="true" size={18} />
      </Button>

      <div className="voice-input-exclusive-status">
        {captureState === 'reviewing' && reviewUrl ? (
          <>
            <Button
              aria-label={isPlaying ? labels.pausePlayback : labels.playRecording}
              onClick={() => void togglePlayback()}
              size="small"
              title={isPlaying ? labels.pausePlayback : labels.playRecording}
              type="button"
              variant="secondary"
            >
              {isPlaying ? <Pause aria-hidden="true" size={16} /> : <Play aria-hidden="true" size={16} />}
              <span>{isPlaying ? labels.pausePlayback : labels.playRecording}</span>
            </Button>
            {!hasListenedToRecording ? (
              <span aria-live="polite" className="text-xs text-secondary-text" role="status">
                {labels.listenBeforeTranscribing}
              </span>
            ) : null}
          </>
        ) : (
          <>
            <span aria-live="polite" aria-atomic="true" role="status">{statusLabel}</span>
            {captureState === 'recording' ? (
              <>
                <progress aria-label={labels.audioLevel} className="h-1.5 w-20 accent-accent" max={1} value={audioLevel} />
                <span>{speechDetected ? labels.speechDetected : labels.silenceDetected}</span>
              </>
            ) : null}
            {captureState === 'processing' ? <LoaderCircle aria-hidden="true" className="animate-spin" size={18} /> : null}
          </>
        )}
      </div>

      <div className="voice-input-exclusive-actions">
        {captureState === 'recording' ? (
          <>
            <Button
              aria-label={labels.pauseRecording}
              onClick={pauseRecording}
              size="small"
              title={labels.pauseRecording}
              type="button"
              variant="secondary"
            >
              <Pause aria-hidden="true" size={16} />
              <span>{labels.pauseRecording}</span>
            </Button>
            <Button
              aria-label={labels.finishRecording}
              onClick={stopRecording}
              size="small"
              title={labels.finishRecording}
              type="button"
              variant="secondary"
            >
              <Square aria-hidden="true" fill="currentColor" size={14} />
              <span>{labels.finishRecording}</span>
            </Button>
          </>
        ) : captureState === 'paused' ? (
          <>
            <Button
              aria-label={labels.resumeRecording}
              onClick={resumeRecording}
              size="small"
              title={labels.resumeRecording}
              type="button"
              variant="secondary"
            >
              <Play aria-hidden="true" size={16} />
              <span>{labels.resumeRecording}</span>
            </Button>
            <Button
              aria-label={labels.finishRecording}
              onClick={stopRecording}
              size="small"
              title={labels.finishRecording}
              type="button"
              variant="secondary"
            >
              <Square aria-hidden="true" fill="currentColor" size={14} />
              <span>{labels.finishRecording}</span>
            </Button>
          </>
        ) : captureState === 'reviewing' ? (
          <Button
            aria-label={labels.confirmTranscription}
            onClick={() => void confirmTranscription()}
            disabled={!hasListenedToRecording}
            size="small"
            title={hasListenedToRecording ? labels.confirmTranscription : labels.listenBeforeTranscribing}
            type="button"
            variant="primary"
          >
            <Check aria-hidden="true" size={16} />
            <span>{labels.confirmTranscription}</span>
          </Button>
        ) : captureState === 'processing' && onProcessingCancel ? (
          <Button
            aria-label={labels.cancelProcessing}
            onClick={closeCapture}
            size="small"
            title={labels.cancelProcessing}
            type="button"
            variant="secondary"
          >
            <X aria-hidden="true" size={16} />
            <span>{labels.cancelProcessing}</span>
          </Button>
        ) : null}
      </div>
    </div>
  );

  return (
    captureState === 'idle' ? (
      <div className={cn('flex items-center gap-2', className)} data-voice-input-state="idle">
        <Button
          aria-label={labels.startRecording}
          disabled={disabled}
          onClick={() => void startRecording()}
          ref={idleButtonRef}
          size="icon"
          title={labels.startRecording}
          type="button"
          variant="ghost"
        >
          <Mic aria-hidden="true" size={18} />
        </Button>
      </div>
    ) : overlayContainer ? (
      createPortal(overlay, overlayContainer)
    ) : (
      overlay
    )
  );
}

function createAudioContext(): AudioContext {
  const AudioContextConstructor = window.AudioContext ?? window.webkitAudioContext;
  if (!AudioContextConstructor) throw new VoiceInputFailure('unsupported');
  return new AudioContextConstructor();
}

function createMediaRecorder(stream: MediaStream): MediaRecorder {
  const supportedMimeType = captureMimeTypes.find((mimeType) => MediaRecorder.isTypeSupported(mimeType));
  return supportedMimeType ? new MediaRecorder(stream, { mimeType: supportedMimeType }) : new MediaRecorder(stream);
}

function disposeSession(session: CaptureSession): void {
  if (session.resourcesDisposed) return;
  session.resourcesDisposed = true;
  if (session.frameId !== null) {
    window.cancelAnimationFrame(session.frameId);
    session.frameId = null;
  }
  if (session.timerId !== null) {
    window.clearTimeout(session.timerId);
    session.timerId = null;
  }
  if (session.stopWatchdogId !== null) {
    window.clearTimeout(session.stopWatchdogId);
    session.stopWatchdogId = null;
  }
  session.recorder.ondataavailable = null;
  session.recorder.onerror = null;
  session.recorder.onstop = null;
  if (session.recorder.state !== 'inactive') {
    try {
      session.recorder.stop();
    } catch (cause) {
      reportCleanupFailure(cause);
    }
  }
  try {
    session.source.disconnect();
  } catch (cause) {
    reportCleanupFailure(cause);
  }
  try {
    session.analyser.disconnect();
  } catch (cause) {
    reportCleanupFailure(cause);
  }
  stopTracks(session.stream);
  releaseCaptureOwner(session.ownerToken);
  closeAudioContext(session.audioContext);
  session.audioContext = null;
  session.analyserData = null;
  session.chunks.length = 0;
}

function releaseCaptureOwner(ownerToken: symbol): void {
  if (activeCaptureOwner === ownerToken) activeCaptureOwner = null;
}

function stopTracks(stream: MediaStream): void {
  for (const track of stream.getTracks()) {
    try {
      track.stop();
    } catch (cause) {
      reportCleanupFailure(cause);
    }
  }
}

function closeAudioContext(audioContext: AudioContext | null): void {
  if (!audioContext) return;
  if (audioContext.state === 'closed') return;
  try {
    void audioContext.close().catch((cause: unknown) => {
      reportCleanupFailure(cause);
    });
  } catch (cause) {
    reportCleanupFailure(cause);
  }
}

function reportCleanupFailure(cause: unknown): void {
  if (typeof globalThis.reportError === 'function') {
    globalThis.reportError(new Error('Voice input resource cleanup failed.', { cause }));
  }
}

function mapCaptureError(error: unknown): VoiceInputError {
  if (error instanceof VoiceInputFailure) return { code: error.code, cause: error.cause };
  if (error instanceof DOMException) {
    if (error.name === 'NotAllowedError' || error.name === 'SecurityError') {
      return { code: 'permission-denied' };
    }
    if (error.name === 'NotFoundError' || error.name === 'NotReadableError' || error.name === 'OverconstrainedError') {
      return { code: 'microphone-unavailable' };
    }
  }
  return { code: 'capture-failed' };
}

class VoiceInputFailure extends Error {
  constructor(
    readonly code: Extract<VoiceInputErrorCode, 'unsupported' | 'empty-audio' | 'capture-failed'>,
    override readonly cause?: unknown,
  ) {
    super(`Voice input failed: ${code}`, { cause });
    this.name = 'VoiceInputFailure';
  }
}
