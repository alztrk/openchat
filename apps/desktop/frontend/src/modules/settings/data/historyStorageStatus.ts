export type HistoryStorageStatus = 'loading' | 'available' | 'unavailable' | 'corrupt' | 'backupUnavailable';

export function historyStorageStatusForInitialization(
  serviceStatus: 'idle' | 'loading' | 'ready' | 'error',
  errorCode: string | null,
): HistoryStorageStatus {
  if (serviceStatus === 'ready') return 'available';
  if (serviceStatus === 'idle' || serviceStatus === 'loading') return 'loading';
  if (errorCode === 'database_corrupt') return 'corrupt';
  if (errorCode === 'database_prepare_failed') return 'backupUnavailable';
  return 'unavailable';
}
