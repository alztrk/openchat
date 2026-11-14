import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { LocalServiceError, localServiceClient, type ServiceOperation } from '@shared/lib/platform/local-service';
import { toast } from '@shared/lib/toast';
import { parseVoiceInputModelStatus } from '@entities/voice-input/model';

import {
  baseLanguageCode,
  parseVoiceInputTranscription,
} from '@features/chat/data/voiceInput';

import {
  VoiceInputCapture,
  type VoiceInputCaptureLabels,
  type VoiceInputError,
  type VoiceInputErrorCode,
} from './VoiceInputCapture';
import type { VoiceInputPayload } from './audioEncoding';

type ModelState = 'loading' | 'missing' | 'ready' | 'error';

export interface VoiceInputControlProps {
  language: string;
  onAppendTranscript(transcript: string): void;
  onExclusiveModeChange?(active: boolean): void;
  overlayContainer?: HTMLElement | null;
  disabled?: boolean;
  className?: string;
}

const captureErrorKeys = {
  unsupported: 'voiceInputUnsupported',
  'invalid-language': 'voiceInputInvalidLanguage',
  'permission-denied': 'voiceInputPermissionDenied',
  'microphone-unavailable': 'voiceInputMicrophoneUnavailable',
  'capture-in-use': 'voiceInputCaptureInUse',
  'capture-failed': 'voiceInputCaptureFailed',
  'playback-failed': 'voiceInputPlaybackFailed',
  'empty-audio': 'voiceInputEmptyAudio',
  'encoding-failed': 'voiceInputEncodingFailed',
  'callback-failed': 'voiceInputCallbackFailed',
} as const satisfies Record<VoiceInputErrorCode, string>;

export function VoiceInputControl({
  language,
  onAppendTranscript,
  onExclusiveModeChange,
  overlayContainer,
  disabled = false,
  className,
}: VoiceInputControlProps) {
  const { t } = useTranslation();
  const [modelState, setModelState] = useState<ModelState>('loading');
  const [isCancellingTranscription, setIsCancellingTranscription] = useState(false);
  const mountedRef = useRef(true);
  const activeTranscriptionRef = useRef<ServiceOperation | null>(null);
  const cancelTranscriptionRequestedRef = useRef(false);

  const loadModelStatus = useCallback(async () => {
    setModelState('loading');
    try {
      const response = await localServiceClient.call('voice.input.model.status');
      const status = parseVoiceInputModelStatus(response);
      if (!mountedRef.current) return;
      setModelState(status.installed ? 'ready' : 'missing');
    } catch {
      if (!mountedRef.current) return;
      setModelState('error');
      toast.error(t('voiceInputModelStatusFailed'));
    }
  }, [t]);

  useEffect(() => {
    void loadModelStatus();
  }, [loadModelStatus]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      cancelTranscriptionRequestedRef.current = true;
      const transcription = activeTranscriptionRef.current;
      if (transcription) {
        void transcription.cancel().catch((cause: unknown) => {
          globalThis.reportError(new Error('The local voice transcription could not be cancelled.', { cause }));
        });
      }
    };
  }, []);

  const transcribe = useCallback(
    async (payload: VoiceInputPayload) => {
      cancelTranscriptionRequestedRef.current = false;
      const params = {
        pcm16Base64: payload.pcm16Base64,
        sampleRate: payload.sampleRate,
        language: payload.language,
      };
      let operation: ServiceOperation | null = null;
      try {
        operation = await localServiceClient.startOperation('voice.input.transcribe', params);
        if (!mountedRef.current) {
          await operation.cancel();
          return;
        }
        if (cancelTranscriptionRequestedRef.current) {
          let cancelled = false;
          try {
            cancelled = await operation.cancel();
          } catch (cause) {
            globalThis.reportError(new Error('The local voice transcription cancellation failed.', { cause }));
          }
          if (cancelled) return;
          cancelTranscriptionRequestedRef.current = false;
          setIsCancellingTranscription(false);
          toast.error(t('voiceInputTranscriptionCancelFailed'));
        }
        activeTranscriptionRef.current = operation;
        const response = await operation.result;
        if (!mountedRef.current || cancelTranscriptionRequestedRef.current) return;
        const transcript = parseVoiceInputTranscription(response);
        if (transcript.length === 0) {
          toast.info(t('voiceInputEmptyTranscript'));
          return;
        }
        onAppendTranscript(transcript);
      } catch (cause) {
        const wasCancelled =
          cancelTranscriptionRequestedRef.current ||
          (cause instanceof LocalServiceError && cause.code === 'operation_cancelled');
        if (!wasCancelled && mountedRef.current) toast.error(t('voiceInputTranscriptionFailed'));
      } finally {
        if (activeTranscriptionRef.current === operation) activeTranscriptionRef.current = null;
        if (mountedRef.current) setIsCancellingTranscription(false);
        params.pcm16Base64 = '';
        payload.pcm16Base64 = '';
        if (mountedRef.current) cancelTranscriptionRequestedRef.current = false;
      }
    },
    [onAppendTranscript, t],
  );

  const cancelTranscription = useCallback(async () => {
    if (isCancellingTranscription) return;
    cancelTranscriptionRequestedRef.current = true;
    setIsCancellingTranscription(true);
    const operation = activeTranscriptionRef.current;
    if (!operation) return;
    try {
      const cancelled = await operation.cancel();
      if (!cancelled) {
        cancelTranscriptionRequestedRef.current = false;
        if (mountedRef.current) {
          setIsCancellingTranscription(false);
          toast.error(t('voiceInputTranscriptionCancelFailed'));
        }
      }
    } catch (cause) {
      cancelTranscriptionRequestedRef.current = false;
      if (mountedRef.current) {
        setIsCancellingTranscription(false);
        toast.error(t('voiceInputTranscriptionCancelFailed'));
      }
      globalThis.reportError(new Error('The local voice transcription cancellation failed.', { cause }));
    }
  }, [isCancellingTranscription, t]);

  const handleCaptureError = useCallback(
    (error: VoiceInputError) => {
      toast.error(t(captureErrorKeys[error.code]));
    },
    [t],
  );

  const normalizedLanguage = baseLanguageCode(language);
  const labels: VoiceInputCaptureLabels = {
    startRecording:
      modelState === 'ready'
        ? t('voiceInputStartRecording')
        : modelState === 'loading'
          ? t('voiceInputModelLoading')
          : t('voiceInputModelUnavailable'),
    requestingPermission: t('voiceInputRequestingPermission'),
    recording: t('voiceInputRecording'),
    paused: t('voiceInputPaused'),
    processing: t('voiceInputProcessing'),
    cancelProcessing: t('voiceInputCancelProcessing'),
    close: t('voiceInputClose'),
    pauseRecording: t('voiceInputPauseRecording'),
    resumeRecording: t('voiceInputResumeRecording'),
    finishRecording: t('voiceInputFinishRecording'),
    playRecording: t('voiceInputPlayRecording'),
    pausePlayback: t('voiceInputPausePlayback'),
    confirmTranscription: t('voiceInputConfirmTranscription'),
    listenBeforeTranscribing: t('voiceInputListenBeforeTranscribing'),
    audioLevel: t('voiceInputAudioLevel'),
    speechDetected: t('voiceInputSpeechDetected'),
    silenceDetected: t('voiceInputSilenceDetected'),
  };

  return (
    <div className={className}>
      <VoiceInputCapture
        disabled={disabled || modelState !== 'ready' || normalizedLanguage === null}
        language={normalizedLanguage ?? 'auto'}
        labels={labels}
        onComplete={transcribe}
        onError={handleCaptureError}
        onProcessingCancel={() => void cancelTranscription()}
        onExclusiveModeChange={onExclusiveModeChange}
        overlayContainer={overlayContainer}
      />
    </div>
  );
}
