export type ChatStorageOperation = 'messageSave' | 'fileChangesList';

export interface ChatStorageFailure {
  code: string;
  retryable: boolean;
  diagnosticCode: string | null;
}

const SERVICE_RESTART_RETRY_DELAYS_MS = [250, 750, 1500] as const;
const SERVICE_TIMEOUT_RETRY_DELAYS_MS = [1000] as const;
const CAPACITY_RETRY_DELAYS_MS = [250, 750, 1500, 3000, 5000] as const;
const MESSAGE_SAVE_RETRY_DELAYS_MS = [250, 750, 1500] as const;
const MESSAGE_SAVE_BUSY_SNAPSHOT_RETRY_DELAYS_MS = [250, 750, 1500, 3000] as const;
const FILE_CHANGE_LIST_RETRY_DELAYS_MS = [250, 750] as const;
const FILE_CHANGE_LOCK_RETRY_DELAYS_MS = [500, 1000, 2000] as const;
const STORAGE_BROKER_RETRY_DELAYS_MS = [250, 750, 1500] as const;
const STORAGE_INITIALIZATION_ERROR_CODES = new Set(['database_prepare_failed', 'storage_initialization_failed']);

const RETRYABLE_STORAGE_DETAILS = new Set([
  'sqlite_busy',
  'sqlite_busy_snapshot',
  'sqlite_busy_timeout',
  'sqlite_5',
  'sqlite_517',
  'database_queue_full',
  'database_queue_wait_timeout',
  'storage_broker_timeout',
  'storage_broker_transport_unavailable',
  'storage_broker_unexpected_eof',
]);

export function chatStorageRetryDelay(
  operation: ChatStorageOperation,
  failure: ChatStorageFailure,
  attempt: number,
): number | null {
  if (!failure.retryable || !Number.isInteger(attempt) || attempt < 0) return null;

  let delays: readonly number[] | null = null;
  if (failure.code === 'service_unavailable' || failure.code === 'service_start_failed') {
    delays = SERVICE_RESTART_RETRY_DELAYS_MS;
  } else if (failure.code === 'service_timeout') {
    delays = SERVICE_TIMEOUT_RETRY_DELAYS_MS;
  } else if (failure.code === 'service_capacity_exceeded' || failure.code === 'local_service_busy') {
    delays = CAPACITY_RETRY_DELAYS_MS;
  } else if (operation === 'messageSave' && failure.diagnosticCode !== null) {
    if (!RETRYABLE_STORAGE_DETAILS.has(failure.diagnosticCode)) return null;
    if (failure.diagnosticCode === 'sqlite_busy_snapshot' || failure.diagnosticCode === 'sqlite_517') {
      delays = MESSAGE_SAVE_BUSY_SNAPSHOT_RETRY_DELAYS_MS;
    } else if (failure.diagnosticCode.startsWith('storage_broker_')) {
      delays = STORAGE_BROKER_RETRY_DELAYS_MS;
    } else {
      delays = MESSAGE_SAVE_RETRY_DELAYS_MS;
    }
  } else if (
    operation === 'fileChangesList' &&
    failure.diagnosticCode !== null
  ) {
    if (failure.code === 'file_changes_temporarily_busy' && failure.diagnosticCode === 'file_change_lock_busy') {
      delays = FILE_CHANGE_LOCK_RETRY_DELAYS_MS;
    } else if (
      (failure.code === 'file_changes_temporarily_busy' ||
        STORAGE_INITIALIZATION_ERROR_CODES.has(failure.code)) &&
      RETRYABLE_STORAGE_DETAILS.has(failure.diagnosticCode)
    ) {
      delays =
        failure.diagnosticCode.startsWith('storage_broker_')
          ? STORAGE_BROKER_RETRY_DELAYS_MS
          : FILE_CHANGE_LIST_RETRY_DELAYS_MS;
    }
  }

  return delays?.[attempt] ?? null;
}
