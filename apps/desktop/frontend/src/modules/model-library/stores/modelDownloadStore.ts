import { create } from 'zustand';

import { type JsonObject, LocalServiceError, localServiceClient } from '@/platform/local-service';

export type ModelDownloadStatus =
  | 'idle'
  | 'starting'
  | 'downloading'
  | 'cancelling'
  | 'completed'
  | 'cancelled'
  | 'failed';

export interface ModelDownloadProgress {
  fileName: string;
  downloadedBytes: number;
  totalBytes: number;
  fileIndex: number;
  fileCount: number;
}

export interface ModelDownloadResult {
  repoId: string;
  revision: string;
  engineId: string;
  downloadedFiles: number;
  totalBytes: number;
  componentPath: string | null;
}

export interface ModelDownloadRequest {
  repoId: string;
  revision: string;
  format: 'gguf';
  groupId: string;
  componentPath?: string;
  modelDirectory?: string;
}

interface ModelDownloadState {
  status: ModelDownloadStatus;
  activeRepoId: string | null;
  progress: ModelDownloadProgress | null;
  progressUnavailable: boolean;
  errorCode: string | null;
  result: ModelDownloadResult | null;
  start(request: ModelDownloadRequest): Promise<void>;
  cancel(): Promise<void>;
}

let operationGeneration = 0;
let cancelOperation: (() => Promise<boolean>) | null = null;
let cancelRequested = false;

export const useModelDownloadStore = create<ModelDownloadState>((set, get) => ({
  status: 'idle',
  activeRepoId: null,
  progress: null,
  progressUnavailable: false,
  errorCode: null,
  result: null,
  start: async (request) => {
    if (isActive(get().status)) return;

    const generation = ++operationGeneration;
    cancelOperation = null;
    cancelRequested = false;
    set({
      status: 'starting',
      activeRepoId: request.repoId,
      progress: null,
      progressUnavailable: false,
      errorCode: null,
      result: null,
    });

    try {
      const operation = await localServiceClient.startOperation(
        'models.hub.download',
        {
          repoId: request.repoId,
          revision: request.revision,
          format: request.format,
          groupId: request.groupId,
          componentPath: request.componentPath ?? null,
          modelDirectory: request.modelDirectory ?? null,
        },
        (event) => {
          if (generation !== operationGeneration || event.name !== 'models.hub.download.progress') return;
          const progress = parseProgress(event.data);
          if (!progress) {
            set({ progressUnavailable: true });
            return;
          }
          set({ progress, progressUnavailable: false });
        },
      );
      cancelOperation = operation.cancel;
      if (generation !== operationGeneration) {
        await operation.cancel();
        return;
      }

      set({ status: cancelRequested ? 'cancelling' : 'downloading' });
      if (cancelRequested) await operation.cancel();

      const response = await operation.result;
      if (generation !== operationGeneration) return;
      set({ status: 'completed', result: parseDownloadResult(response) });
    } catch (error) {
      if (generation !== operationGeneration) return;
      const errorCode = error instanceof LocalServiceError ? error.code : 'modelDownloadFailed';
      set({
        status: errorCode === 'operation_cancelled' ? 'cancelled' : 'failed',
        errorCode,
      });
    } finally {
      if (generation === operationGeneration) cancelOperation = null;
    }
  },
  cancel: async () => {
    if (!isActive(get().status)) return;
    cancelRequested = true;
    set({ status: 'cancelling' });
    await cancelOperation?.();
  },
}));

function parseProgress(value: JsonObject): ModelDownloadProgress | null {
  if (
    typeof value.fileName !== 'string' ||
    !isNonNegativeInteger(value.totalDownloadedBytes) ||
    !isNonNegativeInteger(value.totalBytes) ||
    !isNonNegativeInteger(value.fileIndex) ||
    !isNonNegativeInteger(value.fileCount)
  ) {
    return null;
  }
  return {
    fileName: value.fileName,
    downloadedBytes: value.totalDownloadedBytes,
    totalBytes: value.totalBytes,
    fileIndex: value.fileIndex,
    fileCount: value.fileCount,
  };
}

function parseDownloadResult(value: JsonObject): ModelDownloadResult {
  if (
    typeof value.repoId !== 'string' ||
    typeof value.revision !== 'string' ||
    typeof value.engineId !== 'string' ||
    !isNonNegativeInteger(value.downloadedFiles) ||
    !isNonNegativeInteger(value.totalBytes) ||
    (value.componentPath !== null && typeof value.componentPath !== 'string')
  ) {
    throw new LocalServiceError('invalid_service_response', 'The model download response is invalid.');
  }
  return {
    repoId: value.repoId,
    revision: value.revision,
    engineId: value.engineId,
    downloadedFiles: value.downloadedFiles,
    totalBytes: value.totalBytes,
    componentPath: value.componentPath,
  };
}

function isActive(status: ModelDownloadStatus): boolean {
  return status === 'starting' || status === 'downloading' || status === 'cancelling';
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
