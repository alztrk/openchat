import type { JsonObject, ServiceEvent } from '@shared/lib/platform/local-service';

export interface VoiceInputModelStatus {
  installed: boolean;
  modelBytes: number;
  vadBytes: number;
}

export interface VoiceInputInstallProgress {
  fileName: string;
  totalDownloadedBytes: number;
  totalBytes: number;
  fileIndex: number;
  fileCount: number;
}

/** Validates the local speech model status before the microphone can be enabled. */
export function parseVoiceInputModelStatus(value: JsonObject): VoiceInputModelStatus {
  if (
    typeof value.installed !== 'boolean' ||
    !isNonNegativeSafeInteger(value.modelBytes) ||
    !isNonNegativeSafeInteger(value.vadBytes)
  ) {
    throw new Error('The local voice input model status is invalid.');
  }

  return {
    installed: value.installed,
    modelBytes: value.modelBytes,
    vadBytes: value.vadBytes,
  };
}

/** Accepts only progress events belonging to the local speech model installation. */
export function parseVoiceInputInstallProgress(event: ServiceEvent): VoiceInputInstallProgress | null {
  if (
    event.name !== 'voice.input.model.install.progress' ||
    typeof event.data.fileName !== 'string' ||
    event.data.fileName.trim().length === 0 ||
    !isNonNegativeSafeInteger(event.data.totalDownloadedBytes) ||
    !isNonNegativeSafeInteger(event.data.totalBytes) ||
    !isNonNegativeSafeInteger(event.data.fileIndex) ||
    !isNonNegativeSafeInteger(event.data.fileCount)
  ) {
    return null;
  }

  return {
    fileName: event.data.fileName,
    totalDownloadedBytes: event.data.totalDownloadedBytes,
    totalBytes: event.data.totalBytes,
    fileIndex: event.data.fileIndex,
    fileCount: event.data.fileCount,
  };
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
