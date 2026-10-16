import { isTauri, invoke } from '@tauri-apps/api/core';
import { Child, Command } from '@tauri-apps/plugin-shell';

const requestTimeoutMs = 15_000;
const maximumMessageBytes = 24 * 1024 * 1024;
export const chatSchemaVersion = 18;

export type JsonObject = Record<string, unknown>;

export interface ServiceEvent {
  name: string;
  data: JsonObject;
}

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

interface PendingOperation {
  resolve(value: JsonObject): void;
  reject(reason: LocalServiceError): void;
  onEvent?: (event: ServiceEvent) => void;
}

export interface ServiceOperation {
  id: number;
  result: Promise<JsonObject>;
  cancel(): Promise<boolean>;
}

export interface ServiceDatabaseHealth {
  status: string;
  database_path?: string;
  storage_root?: string;
}

interface PreparedDatabase {
  databasePath: string;
  schemaVersion: number;
}

export class LocalServiceClient {
  private child: Child | undefined;
  private nextRequestId = 0;
  private pending = new Map<number, PendingOperation>();
  private writeQueue: Promise<void> = Promise.resolve();
  private startup: Promise<void> | undefined;
  private initialized = false;

  async initialize(): Promise<void> {
    if (this.initialized) return;
    if (!isTauri()) {
      throw new LocalServiceError(
        'desktop_unavailable',
        'This feature requires the OpenChat desktop app.',
      );
    }

    const startup = this.startup ?? this.initializeOnce();
    this.startup = startup;
    try {
      await startup;
    } finally {
      if (this.startup === startup) this.startup = undefined;
    }
  }

  async call(
    method: string,
    params: JsonObject = {},
    timeoutMs = requestTimeoutMs,
  ): Promise<JsonObject> {
    await this.initialize();
    return this.callRaw(method, params, timeoutMs);
  }

  async readHealthForDiagnostics(): Promise<JsonObject> {
    if (!isTauri()) {
      throw new LocalServiceError(
        'desktop_unavailable',
        'This feature requires the OpenChat desktop app.',
      );
    }
    // Diagnostics must remain readable when database preparation failed.
    await this.startProcess();
    return this.callRaw('system.health', {}, requestTimeoutMs);
  }

  async startOperation(
    method: string,
    params: JsonObject = {},
    onEvent: (event: ServiceEvent) => void = () => undefined,
  ): Promise<ServiceOperation> {
    await this.initialize();
    const operation = await this.beginRaw(method, params, onEvent);
    return {
      id: operation.id,
      result: operation.result,
      cancel: async () => {
        try {
          const result = await this.callRaw(
            'system.cancel',
            { targetId: operation.id },
            requestTimeoutMs,
          );
          return result.cancelled === true;
        } catch (error) {
          if (error instanceof LocalServiceError) return false;
          throw error;
        }
      },
    };
  }

  private async initializeOnce(): Promise<void> {
    await this.startProcess();
    const health = await this.callRaw('system.health', {}, requestTimeoutMs);
    if (health.status !== 'ready' || typeof health.database_path !== 'string') {
      throw new LocalServiceError(
        'invalid_storage_path',
        'The local service did not provide a valid database path.',
      );
    }

    const preparation = await this.callRaw(
      'system.database.prepare',
      { chatSchemaVersion },
      requestTimeoutMs,
    );
    if (preparation.status !== 'ready') {
      throw new LocalServiceError(
        'database_prepare_failed',
        'The local database could not be safely prepared for an update.',
      );
    }

    const prepared = await invoke<PreparedDatabase>('prepare_chat_database');
    if (
      prepared.schemaVersion !== chatSchemaVersion ||
      normalizePath(prepared.databasePath) !== normalizePath(health.database_path)
    ) {
      throw new LocalServiceError(
        'storage_path_changed',
        'The local service and chat storage reported different database locations.',
      );
    }

    const initialization = await this.callRaw(
      'system.initialize',
      {},
      requestTimeoutMs,
    );
    if (initialization.status !== 'ready') {
      throw new LocalServiceError(
        'storage_initialization_failed',
        'Local conversation storage could not be initialized.',
      );
    }
    this.initialized = true;
  }

  private async startProcess(): Promise<void> {
    if (this.child) return;
    const command = Command.sidecar('binaries/openchat_service');
    command.stdout.on('data', (line) => this.handleResponseLine(line));
    command.on('error', () => this.failPending(
      new LocalServiceError(
        'service_start_failed',
        'The local service could not be started.',
        true,
      ),
    ));
    command.on('close', () => {
      this.child = undefined;
      this.initialized = false;
      this.failPending(
        new LocalServiceError(
          'service_unavailable',
          'The local service stopped unexpectedly.',
          true,
        ),
      );
    });

    try {
      this.child = await command.spawn();
    } catch {
      throw new LocalServiceError(
        'service_start_failed',
        'The local service could not be started.',
        true,
      );
    }
  }

  private async callRaw(
    method: string,
    params: JsonObject,
    timeoutMs: number,
  ): Promise<JsonObject> {
    const operation = await this.beginRaw(method, params);
    let timer: number | undefined;
    try {
      return await Promise.race([
        operation.result,
        new Promise<JsonObject>((_, reject) => {
          timer = window.setTimeout(() => {
            this.pending.delete(operation.id);
            reject(
              new LocalServiceError(
                'service_timeout',
                `The local service did not answer ${method} in time.`,
                true,
              ),
            );
          }, timeoutMs);
        }),
      ]);
    } catch (error) {
      if (error instanceof LocalServiceError && error.code === 'service_timeout') {
        void this.cancelRequest(operation.id).catch(() => undefined);
      }
      throw error;
    } finally {
      if (timer !== undefined) window.clearTimeout(timer);
    }
  }

  private async beginRaw(
    method: string,
    params: JsonObject,
    onEvent?: (event: ServiceEvent) => void,
  ): Promise<{ id: number; result: Promise<JsonObject> }> {
    const child = this.child;
    if (!child) {
      throw new LocalServiceError(
        'service_unavailable',
        'The local service is not running.',
        true,
      );
    }

    const id = ++this.nextRequestId;
    const payload = `${JSON.stringify({ id, method, params })}\n`;
    if (new TextEncoder().encode(payload).length > maximumMessageBytes) {
      throw new LocalServiceError(
        'request_too_large',
        'The local service request exceeded its size limit.',
      );
    }

    let resolveResult: (value: JsonObject) => void = () => undefined;
    let rejectResult: (reason: LocalServiceError) => void = () => undefined;
    const result = new Promise<JsonObject>((resolve, reject) => {
      resolveResult = resolve;
      rejectResult = reject;
    });
    this.pending.set(id, {
      resolve: resolveResult,
      reject: rejectResult,
      onEvent,
    });

    const write = this.writeQueue.then(() => child.write(payload));
    this.writeQueue = write.catch(() => undefined);
    try {
      await write;
    } catch {
      this.pending.delete(id);
      throw new LocalServiceError(
        'service_unavailable',
        'The local service connection was interrupted.',
        true,
      );
    }
    return { id, result };
  }

  private async cancelRequest(requestId: number): Promise<void> {
    try {
      await this.callRaw('system.cancel', { targetId: requestId }, 5_000);
    } catch (error) {
      if (error instanceof LocalServiceError) return;
      throw error;
    }
  }

  private handleResponseLine(line: string): void {
    if (line.trim().length === 0) return;
    let response: unknown;
    try {
      response = JSON.parse(line);
    } catch {
      this.failPending(
        new LocalServiceError(
          'invalid_service_response',
          'The local service returned an invalid response.',
        ),
      );
      return;
    }
    if (!isJsonObject(response) || typeof response.id !== 'number') {
      this.failPending(
        new LocalServiceError(
          'invalid_service_response',
          'The local service returned an invalid response identifier.',
        ),
      );
      return;
    }

    const pending = this.pending.get(response.id);
    if (!pending) return;
    if (typeof response.event === 'string') {
      if (!isJsonObject(response.data)) {
        pending.reject(
          new LocalServiceError(
            'invalid_service_response',
            'The local service returned an invalid event.',
          ),
        );
        this.pending.delete(response.id);
        return;
      }
      pending.onEvent?.({ name: response.event, data: response.data });
      return;
    }

    this.pending.delete(response.id);
    if (isJsonObject(response.error)) {
      const code = typeof response.error.code === 'string'
        ? response.error.code
        : 'service_error';
      const message = typeof response.error.message === 'string'
        ? response.error.message
        : 'The local service request failed.';
      pending.reject(
        new LocalServiceError(
          code,
          message,
          response.error.retryable === true,
        ),
      );
      return;
    }
    if (!isJsonObject(response.result)) {
      pending.reject(
        new LocalServiceError(
          'invalid_service_response',
          'The local service response did not contain a result.',
        ),
      );
      return;
    }
    pending.resolve(response.result);
  }

  private failPending(error: LocalServiceError): void {
    for (const [id, pending] of this.pending) {
      this.pending.delete(id);
      pending.reject(error);
    }
  }
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizePath(path: string): string {
  return path.replaceAll('/', '\\').toLocaleLowerCase('en-US');
}
