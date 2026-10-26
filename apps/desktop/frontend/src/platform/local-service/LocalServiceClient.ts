/**
 * Owns the renderer's single newline-delimited RPC connection to the desktop sidecar.
 * Writes are serialized, pending requests are keyed by request ID, and timed-out work receives
 * one best-effort cancellation request without recursively cancelling that control request.
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { i18n } from '@/localization/i18n';

const requestTimeoutMs = 15_000;
const maximumMessageBytes = 24 * 1024 * 1024;
const maximumActiveOperations = 64;
const maximumActiveRequestBytes = 64 * 1024 * 1024;
/** JSON object accepted or returned by the local sidecar RPC boundary. */
export type JsonObject = Record<string, unknown>;

/** Copy rendered by the active frontend locale for native tool-permission dialogs. */
export interface NativePermissionDialogCopy {
  title: string;
  intro: string;
  fullAccessIntro: string;
  fullAccessWarning: string;
  projectAllowIntro: string;
  projectAllowWarning: string;
  projectAllowMore: string;
  toolLabel: string;
  targetLabel: string;
  argumentsLabel: string;
  warning: string;
  allowOnce: string;
  enableFullAccess: string;
  saveProjectAllow: string;
  deny: string;
  tooLarge: string;
  close: string;
}

/** Resolves native permission copy through the same i18next resources as the renderer UI. */
export function getNativePermissionDialogCopy(): NativePermissionDialogCopy {
  return {
    title: i18n.t('nativePermissionTitle'),
    intro: i18n.t('nativePermissionIntro'),
    fullAccessIntro: i18n.t('nativePermissionFullAccessIntro'),
    fullAccessWarning: i18n.t('nativePermissionFullAccessWarning'),
    projectAllowIntro: i18n.t('nativePermissionProjectAllowIntro'),
    projectAllowWarning: i18n.t('nativePermissionProjectAllowWarning'),
    projectAllowMore: i18n.t('nativePermissionProjectAllowMore'),
    toolLabel: i18n.t('nativePermissionToolLabel'),
    targetLabel: i18n.t('nativePermissionTargetLabel'),
    argumentsLabel: i18n.t('nativePermissionArgumentsLabel'),
    warning: i18n.t('nativePermissionWarning'),
    allowOnce: i18n.t('nativePermissionAllowOnce'),
    enableFullAccess: i18n.t('nativePermissionEnableFullAccess'),
    saveProjectAllow: i18n.t('nativePermissionSaveProjectAllow'),
    deny: i18n.t('nativePermissionDeny'),
    tooLarge: i18n.t('nativePermissionTooLarge'),
    close: i18n.t('nativePermissionClose'),
  };
}

/** Progress or state event emitted while a sidecar operation is running. */
export interface ServiceEvent {
  name: string;
  data: JsonObject;
}

/** Stable sidecar failure contract, including whether retrying may succeed. */
export class LocalServiceError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly retryable = false,
    readonly details: string | null = null,
  ) {
    super(message);
    this.name = 'LocalServiceError';
  }
}

/** Returns the sidecar's safe diagnostic text without exposing arbitrary rejected values. */
export function localServiceErrorDetails(error: unknown): string | null {
  if (!(error instanceof LocalServiceError)) return null;
  const message = error.message.trim();
  const details = error.details?.trim();
  const diagnostic = [message, details]
    .filter((value): value is string => typeof value === 'string' && value.length > 0)
    .filter((value, index, values) => values.indexOf(value) === index);
  if (diagnostic.length === 0) return null;
  return diagnostic.join('\n').slice(0, 2048);
}

/** Resolver and optional event callback retained until one RPC request completes. */
interface PendingOperation {
  requestBytes: number;
  writePending: boolean;
  resolve(value: JsonObject): void;
  reject(reason: LocalServiceError): void;
  onEvent?: (event: ServiceEvent) => void;
}

/** Handle returned for long-running RPC work, allowing progress observation and cancellation. */
export interface ServiceOperation {
  id: number;
  result: Promise<JsonObject>;
  cancel(): Promise<boolean>;
}

/** Database and service status fields returned by the diagnostic health route. */
export interface ServiceDatabaseHealth {
  status: string;
  schema_version: number;
}

/** Reads the schema version compiled into the native host and validates the IPC response. */
export async function getChatSchemaVersion(): Promise<number> {
  const value: unknown = await invoke('get_chat_schema_version');
  if (!isChatSchemaVersion(value)) {
    throw new LocalServiceError(
      'invalid_chat_schema_version',
      'The native host returned an invalid chat database schema version.',
    );
  }
  return value;
}

/** Native events emitted for one line, safe startup failure, or termination of the owned service process. */
type NativeServiceEvent =
  | { type: 'output'; generation: number; line: string }
  | { type: 'failure'; generation: number; details: string }
  | { type: 'stopped'; generation: number };

/**
 * Manages startup, serialized sidecar writes, RPC response matching, and operation cancellation.
 * A client instance is intended to own one sidecar process for the application lifetime.
 */
export class LocalServiceClient {
  private processGeneration: number | undefined;
  private earlyStoppedGeneration: number | undefined;
  private earlyProcessFailures = new Map<number, string>();
  private processFailure: { generation: number; details: string } | undefined;
  private lastProcessFailure: LocalServiceError | null = null;
  private processEventListener: Promise<void> | undefined;
  private nextRequestId = 0;
  private pending = new Map<number, PendingOperation>();
  private activeRequestBytes = 0;
  private orphanedWriteCount = 0;
  private orphanedWriteBytes = 0;
  private writeQueue: Promise<void> = Promise.resolve();
  private startup: Promise<void> | undefined;
  private processStartup: Promise<void> | undefined;
  private initialized = false;
  private initializationSettings: JsonObject | undefined;
  private broadcastListeners = new Set<(event: ServiceEvent) => void>();

  /** Subscribes to broadcast and progress events emitted by the local service process. */
  subscribe(listener: (event: ServiceEvent) => void): () => void {
    this.broadcastListeners.add(listener);
    return () => {
      this.broadcastListeners.delete(listener);
    };
  }

  private dispatchBroadcast(event: ServiceEvent): void {
    if (event.name === 'app.setting.changed') {
      void invoke('broadcast_window_sync_event', {
        event: 'openchat://app-setting-changed',
        payload: event.data,
      }).catch((cause: unknown) => {
        globalThis.reportError(
          new Error('A local setting update could not be synchronized across windows.', { cause }),
        );
      });
    }
    for (const listener of this.broadcastListeners) {
      try {
        listener(event);
      } catch (cause) {
        globalThis.reportError(new Error('A local service broadcast listener failed.', { cause }));
      }
    }
  }

  /** Starts and initializes the sidecar once, sharing concurrent startup attempts. */
  async initialize(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
    if (this.initialized) {
      if (options.startWithEmptyStorage) {
        throw new LocalServiceError(
          'service_already_initialized',
          'The local service is already initialized and cannot switch storage profiles in this session.',
        );
      }
      return;
    }
    if (!isTauri()) {
      throw new LocalServiceError('desktop_unavailable', 'This feature requires the OpenChat desktop app.');
    }

    const startup = this.startup ?? this.initializeOnce(options.startWithEmptyStorage === true);
    this.startup = startup;
    try {
      await startup;
    } finally {
      if (this.startup === startup) this.startup = undefined;
    }
  }

  /** Returns the application settings included in the one-time service startup response. */
  getStartupSettings(): JsonObject {
    const settings = this.initializationSettings;
    if (!settings) {
      throw new LocalServiceError('startup_settings_unavailable', 'The local service startup settings are unavailable.');
    }
    return settings;
  }

  /** Releases startup settings after the storage adapter has completed initialization. */
  clearStartupSettings(): void {
    this.initializationSettings = undefined;
  }

  /** Sends a bounded request after storage and service initialization has completed. */
  async call(method: string, params: JsonObject = {}, timeoutMs = requestTimeoutMs): Promise<JsonObject> {
    await this.initialize();
    return this.callRaw(method, params, timeoutMs);
  }

  /** Routes permission answers through the native host, which requires OS consent to allow a call. */
  async respondToPermission(approvalRequestId: string, approved: boolean): Promise<boolean> {
    await this.initialize();
    const generation = this.processGeneration;
    if (generation === undefined) {
      throw new LocalServiceError('service_unavailable', 'The local service is not running.', true);
    }
    const result: unknown = await invoke('respond_local_service_permission', {
      generation,
      approvalRequestId,
      approved,
      permissionCopy: getNativePermissionDialogCopy(),
    });
    if (typeof result !== 'boolean') {
      throw new LocalServiceError(
        'invalid_permission_response',
        'The native host returned an invalid tool permission result.',
      );
    }
    return result;
  }

  /**
   * Reads service health without requiring successful database preparation, so startup errors
   * can still report whether the sidecar itself is reachable.
   */
  async readHealthForDiagnostics(): Promise<ServiceDatabaseHealth> {
    if (!isTauri()) {
      throw new LocalServiceError('desktop_unavailable', 'This feature requires the OpenChat desktop app.');
    }
    // Diagnostics must remain readable when database preparation failed.
    await this.startProcess();
    const health = await this.callRaw('system.health', {}, requestTimeoutMs);
    if (
      typeof health.status !== 'string' ||
      typeof health.schema_version !== 'number' ||
      !Number.isSafeInteger(health.schema_version)
    ) {
      throw new LocalServiceError(
        'invalid_service_response',
        'The local service returned invalid diagnostic health data.',
      );
    }
    return {
      status: health.status,
      schema_version: health.schema_version,
    };
  }

  /** Starts long-running RPC work and returns its result, progress stream, and cancel control. */
  async startOperation(
    method: string,
    params: JsonObject = {},
    onEvent: (event: ServiceEvent) => void = () => undefined,
  ): Promise<ServiceOperation> {
    await this.initialize();
    const operation = this.beginRaw(method, params, onEvent);
    // Keep the rejection handled until this operation handle is returned to its caller.
    void operation.result.catch(() => undefined);
    let sendTimer: number | undefined;
    try {
      await Promise.race([
        operation.sent,
        new Promise<void>((_, reject) => {
          sendTimer = window.setTimeout(() => {
            this.removePending(operation.id);
            reject(
              new LocalServiceError('service_timeout', `The local service did not accept ${method} in time.`, true),
            );
          }, requestTimeoutMs);
        }),
      ]);
    } catch (error) {
      this.removePending(operation.id);
      if (error instanceof LocalServiceError && error.code === 'service_timeout') {
        void this.cancelRequest(operation.id).catch((cause: unknown) => {
          this.reportUnexpectedCancellationFailure(cause);
        });
      }
      throw error;
    } finally {
      if (sendTimer !== undefined) window.clearTimeout(sendTimer);
    }
    return {
      id: operation.id,
      result: operation.result,
      cancel: async () => {
        try {
          const result = await this.callRaw('system.cancel', { targetId: operation.id }, requestTimeoutMs, false);
          return result.cancelled === true;
        } catch (error) {
          if (error instanceof LocalServiceError) return false;
          throw error;
        }
      },
    };
  }

  /** Performs the one-time storage preparation and application initialization handshake. */
  private async initializeOnce(startWithEmptyStorage: boolean): Promise<void> {
    const [chatSchemaVersion] = await Promise.all([
      getChatSchemaVersion(),
      this.startProcess(startWithEmptyStorage),
    ]);

    const initialization = await this.callRaw('system.initialize', { chatSchemaVersion }, requestTimeoutMs);
    if (initialization.status !== 'ready') {
      throw new LocalServiceError(
        'storage_initialization_failed',
        'Local conversation storage could not be initialized.',
      );
    }
    this.initializationSettings = initialization;
    this.initialized = true;
  }

  /** Spawns the packaged sidecar and binds its output and lifecycle events to this client. */
  private async startProcess(startWithEmptyStorage = false): Promise<void> {
    if (this.processGeneration !== undefined && !startWithEmptyStorage) return;
    const startup = this.processStartup ?? this.spawnProcess(startWithEmptyStorage);
    this.processStartup = startup;
    try {
      await startup;
    } finally {
      if (this.processStartup === startup) this.processStartup = undefined;
    }
  }

  /** Starts one sidecar process and ignores late lifecycle events from a replaced process. */
  private async spawnProcess(startWithEmptyStorage = false): Promise<void> {
    await this.listenForProcessEvents();
    this.lastProcessFailure = null;
    this.processFailure = undefined;
    this.earlyProcessFailures.clear();
    let generation: number;
    try {
      generation = await invoke<number>(
        startWithEmptyStorage ? 'start_local_service_with_empty_storage' : 'start_local_service',
      );
    } catch (error) {
      throw new LocalServiceError(
        'service_start_failed',
        'The local service could not be started.',
        true,
        readErrorDetails(error),
      );
    }
    if (!Number.isSafeInteger(generation) || generation <= 0) {
      throw new LocalServiceError(
        'invalid_service_generation',
        'The local service returned an invalid process identifier.',
      );
    }
    this.processGeneration = generation;
    const earlyFailureDetails = this.earlyProcessFailures.get(generation);
    this.processFailure = earlyFailureDetails ? { generation, details: earlyFailureDetails } : undefined;
    this.earlyProcessFailures.delete(generation);
    if (this.earlyStoppedGeneration === generation) {
      this.earlyStoppedGeneration = undefined;
      const details = this.processFailure?.details ?? null;
      this.markProcessStopped(generation, details);
    } else {
      this.earlyStoppedGeneration = undefined;
    }
  }

  /** Registers one native event listener before the host starts the process. */
  private async listenForProcessEvents(): Promise<void> {
    if (this.processEventListener) return this.processEventListener;
    const listener = listen<unknown>('openchat:local-service', (event) => {
      if (!isNativeServiceEvent(event.payload)) {
        this.failPending(new LocalServiceError('invalid_service_event', 'The local service emitted an invalid event.'));
        return;
      }
      if (event.payload.type === 'output') {
        if (event.payload.generation === this.processGeneration && typeof event.payload.line === 'string') {
          this.handleResponseLine(event.payload.line);
        }
        return;
      }
      if (event.payload.type === 'failure') {
        const failure = { generation: event.payload.generation, details: event.payload.details };
        if (event.payload.generation === this.processGeneration) {
          this.processFailure = failure;
        } else if (this.processGeneration === undefined && this.processStartup) {
          this.earlyProcessFailures.set(failure.generation, failure.details);
        }
        return;
      }
      if (event.payload.generation === this.processGeneration) {
        const details =
          this.processFailure?.generation === event.payload.generation ? this.processFailure.details : null;
        this.markProcessStopped(event.payload.generation, details);
      } else if (this.processGeneration === undefined && this.processStartup) {
        this.earlyStoppedGeneration = event.payload.generation;
      }
    }).then(() => undefined);
    this.processEventListener = listener;
    try {
      await listener;
    } catch (error) {
      if (this.processEventListener === listener) this.processEventListener = undefined;
      throw error;
    }
  }

  /** Drops pending work and invalidates the client when its current native process stops. */
  private markProcessStopped(generation: number, details: string | null = null): void {
    if (this.processGeneration !== generation) return;
    this.processGeneration = undefined;
    this.initialized = false;
    this.initializationSettings = undefined;
    this.writeQueue = Promise.resolve();
    this.processFailure = undefined;
    this.lastProcessFailure = new LocalServiceError(
      'service_unavailable',
      'The local service stopped unexpectedly.',
      true,
      details,
    );
    this.failPending(this.lastProcessFailure);
  }

  /** Sends one request, bounds its wait, and optionally requests cancellation on timeout. */
  private async callRaw(
    method: string,
    params: JsonObject,
    timeoutMs: number,
    cancelOnTimeout = true,
  ): Promise<JsonObject> {
    const operation = this.beginRaw(method, params);
    let timer: number | undefined;
    try {
      return await Promise.race([
        operation.result,
        new Promise<JsonObject>((_, reject) => {
          timer = window.setTimeout(() => {
            this.removePending(operation.id);
            reject(
              new LocalServiceError('service_timeout', `The local service did not answer ${method} in time.`, true),
            );
          }, timeoutMs);
        }),
      ]);
    } catch (error) {
      if (cancelOnTimeout && error instanceof LocalServiceError && error.code === 'service_timeout') {
        void this.cancelRequest(operation.id).catch((cause: unknown) => {
          this.reportUnexpectedCancellationFailure(cause);
        });
      }
      throw error;
    } finally {
      if (timer !== undefined) window.clearTimeout(timer);
    }
  }

  /** Allocates the request ID, stores its handlers, and queues a serialized pipe write. */
  private beginRaw(
    method: string,
    params: JsonObject,
    onEvent?: (event: ServiceEvent) => void,
  ): { id: number; result: Promise<JsonObject>; sent: Promise<void> } {
    const generation = this.processGeneration;
    if (generation === undefined) {
      throw (
        this.lastProcessFailure ??
        new LocalServiceError('service_unavailable', 'The local service is not running.', true)
      );
    }

    const id = ++this.nextRequestId;
    const payload = `${JSON.stringify({ id, method, params })}\n`;
    const requestBytes = new TextEncoder().encode(payload).length;
    if (requestBytes > maximumMessageBytes) {
      throw new LocalServiceError('request_too_large', 'The local service request exceeded its size limit.');
    }
    if (
      this.pending.size + this.orphanedWriteCount >= maximumActiveOperations ||
      this.activeRequestBytes + this.orphanedWriteBytes + requestBytes > maximumActiveRequestBytes
    ) {
      throw new LocalServiceError(
        'service_capacity_exceeded',
        'The local service is handling too many requests. Wait for current work to finish and try again.',
        true,
      );
    }

    let resolveResult: (value: JsonObject) => void = () => undefined;
    let rejectResult: (reason: LocalServiceError) => void = () => undefined;
    const result = new Promise<JsonObject>((resolve, reject) => {
      resolveResult = resolve;
      rejectResult = reject;
    });
    this.pending.set(id, {
      requestBytes,
      writePending: true,
      resolve: resolveResult,
      reject: rejectResult,
      onEvent,
    });
    this.activeRequestBytes += requestBytes;

    const write = this.writeQueue.then(async () => {
      const pending = this.pending.get(id);
      if (!pending) {
        this.releaseOrphanedWrite(requestBytes);
        return;
      }
      try {
        await invoke('write_local_service', {
          generation,
          line: payload,
          permissionCopy: getNativePermissionDialogCopy(),
        });
      } catch (error) {
        throw nativeWriteError(error);
      } finally {
        const current = this.pending.get(id);
        if (current) {
          current.writePending = false;
        } else {
          this.releaseOrphanedWrite(requestBytes);
        }
      }
    });
    // Recover the serial queue; the request's `sent` promise below carries this write failure.
    this.writeQueue = write.catch(() => undefined);
    const sent = write.catch((error: unknown) => {
      const failure = error instanceof LocalServiceError ? error : nativeWriteError(error);
      const pending = this.removePending(id);
      if (pending) {
        pending.reject(failure);
      }
      throw failure;
    });
    // callRaw observes `result`, not `sent`; the write failure has already been routed to `result`.
    void sent.catch(() => undefined);
    return { id, result, sent };
  }

  /** Makes a best-effort cancellation request without creating a cancellation retry chain. */
  private async cancelRequest(requestId: number): Promise<void> {
    try {
      await this.callRaw('system.cancel', { targetId: requestId }, 5_000, false);
    } catch (error) {
      if (error instanceof LocalServiceError) return;
      throw error;
    }
  }

  /** Reports only unexpected failures from fire-and-forget cancellation requests. */
  private reportUnexpectedCancellationFailure(cause: unknown): void {
    globalThis.reportError(new Error('A local service cancellation request failed unexpectedly.', { cause }));
  }

  /** Parses one response frame and resolves, rejects, or forwards it to the matching request. */
  private handleResponseLine(line: string): void {
    if (line.trim().length === 0) return;
    let response: unknown;
    try {
      response = JSON.parse(line);
    } catch {
      this.failPending(
        new LocalServiceError('invalid_service_response', 'The local service returned an invalid response.'),
      );
      return;
    }
    if (!isJsonObject(response) || typeof response.id !== 'number') {
      this.failPending(
        new LocalServiceError('invalid_service_response', 'The local service returned an invalid response identifier.'),
      );
      return;
    }

    const pending = this.pending.get(response.id);
    if (typeof response.event === 'string') {
      if (!isJsonObject(response.data)) {
        if (pending) {
          pending.reject(
            new LocalServiceError('invalid_service_response', 'The local service returned an invalid event.'),
          );
          this.removePending(response.id);
        }
        return;
      }
      const event: ServiceEvent = { name: response.event, data: response.data };
      if (pending) {
        try {
          pending.onEvent?.(event);
        } catch (cause) {
          this.removePending(response.id);
          pending.reject(
            new LocalServiceError(
              'service_event_handler_failed',
              'A local service progress update could not be processed.',
            ),
          );
          globalThis.reportError(new Error('A local service progress event handler failed.', { cause }));
          void this.cancelRequest(response.id).catch((cancelError: unknown) => {
            this.reportUnexpectedCancellationFailure(cancelError);
          });
        }
      }
      this.dispatchBroadcast(event);
      return;
    }

    if (!pending) return;

    this.removePending(response.id);
    if (isJsonObject(response.error)) {
      const code = typeof response.error.code === 'string' ? response.error.code : 'service_error';
      const message =
        typeof response.error.message === 'string' ? response.error.message : 'The local service request failed.';
      const details =
        typeof response.error.details === 'string' ? response.error.details.trim().slice(0, 1000) || null : null;
      pending.reject(new LocalServiceError(code, message, response.error.retryable === true, details));
      return;
    }
    if (!isJsonObject(response.result)) {
      pending.reject(
        new LocalServiceError('invalid_service_response', 'The local service response did not contain a result.'),
      );
      return;
    }
    pending.resolve(response.result);
  }

  /** Rejects every request whose response can no longer arrive from the sidecar. */
  private failPending(error: LocalServiceError): void {
    for (const [id, pending] of this.pending) {
      this.removePending(id);
      pending.reject(error);
    }
  }

  /** Removes one request and releases the renderer's active payload budget. */
  private removePending(id: number): PendingOperation | undefined {
    const pending = this.pending.get(id);
    if (!pending) return undefined;
    this.pending.delete(id);
    this.activeRequestBytes -= pending.requestBytes;
    if (pending.writePending) {
      this.orphanedWriteCount += 1;
      this.orphanedWriteBytes += pending.requestBytes;
    }
    return pending;
  }

  /** Releases the bounded budget held by a timed-out request still in the write queue. */
  private releaseOrphanedWrite(requestBytes: number): void {
    this.orphanedWriteCount -= 1;
    this.orphanedWriteBytes -= requestBytes;
  }
}

/** Narrows parsed JSON to a non-array object for RPC contract validation. */
function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readErrorDetails(error: unknown): string | null {
  const detail = error instanceof Error ? error.message : typeof error === 'string' ? error : null;
  return detail?.trim().slice(0, 1000) || null;
}

function nativeWriteError(error: unknown): LocalServiceError {
  return new LocalServiceError(
    'native_bridge_failed',
    'The desktop host could not forward the request to the local service.',
    false,
    readErrorDetails(error),
  );
}

/** Validates the shape of the native process event before routing it to the RPC client. */
function isNativeServiceEvent(value: unknown): value is NativeServiceEvent {
  if (!isJsonObject(value) || typeof value.generation !== 'number' || !Number.isSafeInteger(value.generation)) {
    return false;
  }
  if (value.type === 'output') return typeof value.line === 'string';
  if (value.type === 'failure') return typeof value.details === 'string' && value.details.length <= 2048;
  return value.type === 'stopped';
}

function isChatSchemaVersion(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 100;
}
