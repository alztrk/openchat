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
export const chatSchemaVersion = 18;

/** JSON object accepted or returned by the local sidecar RPC boundary. */
export type JsonObject = Record<string, unknown>;

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
  ) {
    super(message);
    this.name = 'LocalServiceError';
  }
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

/** Database location and schema version confirmed by the Tauri host. */
interface PreparedDatabase {
  databasePath: string;
  schemaVersion: number;
}

/** Native event emitted for one line or termination of the owned service process. */
interface NativeServiceEvent {
  type: 'output' | 'stopped';
  generation: number;
  line?: string;
}

/**
 * Manages startup, serialized sidecar writes, RPC response matching, and operation cancellation.
 * A client instance is intended to own one sidecar process for the application lifetime.
 */
export class LocalServiceClient {
  private processGeneration: number | undefined;
  private earlyStoppedGeneration: number | undefined;
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

  /** Starts and initializes the sidecar once, sharing concurrent startup attempts. */
  async initialize(): Promise<void> {
    if (this.initialized) return;
    if (!isTauri()) {
      throw new LocalServiceError('desktop_unavailable', 'This feature requires the OpenChat desktop app.');
    }

    const startup = this.startup ?? this.initializeOnce();
    this.startup = startup;
    try {
      await startup;
    } finally {
      if (this.startup === startup) this.startup = undefined;
    }
  }

  /** Sends a bounded request after storage and service initialization has completed. */
  async call(method: string, params: JsonObject = {}, timeoutMs = requestTimeoutMs): Promise<JsonObject> {
    await this.initialize();
    return this.callRaw(method, params, timeoutMs);
  }

  /** Routes permission answers through the native host, which requires OS consent to allow a call. */
  async respondToPermission(approvalRequestId: string, approved: boolean, locale: string): Promise<boolean> {
    await this.initialize();
    const generation = this.processGeneration;
    if (generation === undefined) {
      throw new LocalServiceError('service_unavailable', 'The local service is not running.', true);
    }
    const result: unknown = await invoke('respond_local_service_permission', {
      generation,
      approvalRequestId,
      approved,
      locale,
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
  private async initializeOnce(): Promise<void> {
    await this.startProcess();
    const health = await this.callRaw('system.health', {}, requestTimeoutMs);
    if (health.status !== 'ready' || typeof health.database_path !== 'string') {
      throw new LocalServiceError('invalid_storage_path', 'The local service did not provide a valid database path.');
    }

    const preparation = await this.callRaw('system.database.prepare', { chatSchemaVersion }, requestTimeoutMs);
    if (preparation.status !== 'ready') {
      throw new LocalServiceError(
        'database_prepare_failed',
        'The local database could not be safely prepared for an update.',
      );
    }

    const prepared: unknown = await invoke('prepare_chat_database');
    if (!isPreparedDatabase(prepared)) {
      throw new LocalServiceError(
        'invalid_database_preparation',
        'The native database preparation returned invalid data.',
      );
    }
    if (
      prepared.schemaVersion !== chatSchemaVersion ||
      normalizePath(prepared.databasePath) !== normalizePath(health.database_path)
    ) {
      throw new LocalServiceError(
        'storage_path_changed',
        'The local service and chat storage reported different database locations.',
      );
    }

    const initialization = await this.callRaw('system.initialize', {}, requestTimeoutMs);
    if (initialization.status !== 'ready') {
      throw new LocalServiceError(
        'storage_initialization_failed',
        'Local conversation storage could not be initialized.',
      );
    }
    this.initialized = true;
  }

  /** Spawns the packaged sidecar and binds its output and lifecycle events to this client. */
  private async startProcess(): Promise<void> {
    if (this.processGeneration !== undefined) return;
    const startup = this.processStartup ?? this.spawnProcess();
    this.processStartup = startup;
    try {
      await startup;
    } finally {
      if (this.processStartup === startup) this.processStartup = undefined;
    }
  }

  /** Starts one sidecar process and ignores late lifecycle events from a replaced process. */
  private async spawnProcess(): Promise<void> {
    await this.listenForProcessEvents();
    let generation: number;
    try {
      generation = await invoke<number>('start_local_service');
    } catch {
      throw new LocalServiceError('service_start_failed', 'The local service could not be started.', true);
    }
    if (!Number.isSafeInteger(generation) || generation <= 0) {
      throw new LocalServiceError(
        'invalid_service_generation',
        'The local service returned an invalid process identifier.',
      );
    }
    this.processGeneration = generation;
    if (this.earlyStoppedGeneration === generation) {
      this.earlyStoppedGeneration = undefined;
      this.markProcessStopped(generation);
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
      if (event.payload.generation === this.processGeneration) {
        this.markProcessStopped(event.payload.generation);
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
  private markProcessStopped(generation: number): void {
    if (this.processGeneration !== generation) return;
    this.processGeneration = undefined;
    this.initialized = false;
    this.writeQueue = Promise.resolve();
    this.failPending(new LocalServiceError('service_unavailable', 'The local service stopped unexpectedly.', true));
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
      throw new LocalServiceError('service_unavailable', 'The local service is not running.', true);
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
          locale: i18n.resolvedLanguage ?? i18n.language,
        });
      } catch {
        throw new LocalServiceError('service_unavailable', 'The local service connection was interrupted.', true);
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
    const sent = write.catch(() => {
      const pending = this.removePending(id);
      if (pending) {
        pending.reject(
          new LocalServiceError('service_unavailable', 'The local service connection was interrupted.', true),
        );
      }
      throw new LocalServiceError('service_unavailable', 'The local service connection was interrupted.', true);
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
    if (!pending) return;
    if (typeof response.event === 'string') {
      if (!isJsonObject(response.data)) {
        pending.reject(
          new LocalServiceError('invalid_service_response', 'The local service returned an invalid event.'),
        );
        this.removePending(response.id);
        return;
      }
      try {
        pending.onEvent?.({ name: response.event, data: response.data });
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
      return;
    }

    this.removePending(response.id);
    if (isJsonObject(response.error)) {
      const code = typeof response.error.code === 'string' ? response.error.code : 'service_error';
      const message =
        typeof response.error.message === 'string' ? response.error.message : 'The local service request failed.';
      pending.reject(new LocalServiceError(code, message, response.error.retryable === true));
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

/** Validates the shape of the native process event before routing it to the RPC client. */
function isNativeServiceEvent(value: unknown): value is NativeServiceEvent {
  if (!isJsonObject(value) || typeof value.generation !== 'number' || !Number.isSafeInteger(value.generation)) {
    return false;
  }
  if (value.type === 'output') return typeof value.line === 'string';
  return value.type === 'stopped';
}

/** Validates the Tauri host's database-preparation result at the IPC boundary. */
function isPreparedDatabase(value: unknown): value is PreparedDatabase {
  return (
    isJsonObject(value) &&
    typeof value.databasePath === 'string' &&
    typeof value.schemaVersion === 'number' &&
    Number.isSafeInteger(value.schemaVersion)
  );
}

/** Normalizes Windows path separators and case before comparing the two storage authorities. */
function normalizePath(path: string): string {
  return path.replaceAll('/', '\\').toLocaleLowerCase('en-US');
}
