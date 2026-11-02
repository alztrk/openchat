/**
 * Owns the renderer's single newline-delimited RPC connection to the desktop sidecar.
 * Writes are serialized, pending requests are keyed by request ID, and timed-out work receives
 * one best-effort cancellation request without recursively cancelling that control request.
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const requestTimeoutMs = 30_000;
const maximumMessageBytes = 24 * 1024 * 1024;
const maximumActiveOperations = 64;
// Preserve part of the shared RPC budget for short-lived persistence and control requests.
const maximumActiveChatOperations = 48;
const maximumActiveRequestBytes = 64 * 1024 * 1024;
const maximumActiveChatRequestBytes = maximumActiveRequestBytes - maximumMessageBytes;
const contextUsageMethods = new Set(['chat.context.usage.estimate', 'chat.context.usage.full_estimate']);
/** JSON object accepted or returned by the local sidecar RPC boundary. */
export type JsonObject = Record<string, unknown>;

/** Supported native dialog locale identifiers; the host owns all displayed copy. */
export type NativeDialogLocale = 'de' | 'en' | 'es' | 'fr' | 'tr';

/** Selects a host-embedded locale without sending renderer-authored dialog text. */
export async function getNativeDialogLocale(): Promise<NativeDialogLocale> {
  const { i18n, i18nReady } = await import('@shared/lib/i18n/i18n');
  await i18nReady;
  const language = i18n.resolvedLanguage?.toLowerCase().split('-')[0];
  if (language === 'de' || language === 'en' || language === 'es' || language === 'fr' || language === 'tr') {
    return language;
  }
  throw new Error('The selected locale is not supported for native security dialogs.');
}

/** Progress or state event emitted while a sidecar operation is running. */
export interface ServiceEvent {
  name: string;
  data: JsonObject;
}

export type ServiceErrorCategory =
  | 'validation'
  | 'authentication'
  | 'authorization'
  | 'not_found'
  | 'conflict'
  | 'rate_limit'
  | 'network'
  | 'provider'
  | 'storage'
  | 'filesystem'
  | 'process'
  | 'protocol'
  | 'timeout'
  | 'resource_limit'
  | 'cancelled'
  | 'unavailable'
  | 'internal';

const serviceErrorCategoryTranslationKeys = {
  validation: 'serviceErrorValidation',
  authentication: 'serviceErrorAuthentication',
  authorization: 'serviceErrorAuthorization',
  not_found: 'serviceErrorNotFound',
  conflict: 'serviceErrorConflict',
  rate_limit: 'serviceErrorRateLimit',
  network: 'serviceErrorNetwork',
  provider: 'serviceErrorProvider',
  storage: 'serviceErrorStorage',
  filesystem: 'serviceErrorFilesystem',
  process: 'serviceErrorProcess',
  protocol: 'serviceErrorProtocol',
  timeout: 'serviceErrorTimeout',
  resource_limit: 'serviceErrorResourceLimit',
  cancelled: 'serviceErrorCancelled',
  unavailable: 'serviceErrorUnavailable',
  internal: 'serviceErrorInternal',
} as const satisfies Record<ServiceErrorCategory, string>;

type ServiceErrorCategoryTranslationKey = (typeof serviceErrorCategoryTranslationKeys)[ServiceErrorCategory];

export function serviceErrorCategoryTranslationKey(category: ServiceErrorCategory): ServiceErrorCategoryTranslationKey {
  return serviceErrorCategoryTranslationKeys[category];
}

const explicitServiceErrorCategories: Partial<Record<string, ServiceErrorCategory>> = {
  browser_inspect_failed: 'network',
  browser_open_failed: 'process',
  browser_url_blocked: 'authorization',
  connection_changed: 'conflict',
  connection_remove_worker_failed: 'process',
  connector_catalog_invalid_data: 'protocol',
  connector_catalog_invalid_source: 'validation',
  connector_package_in_use: 'conflict',
  connector_package_integrity_failed: 'validation',
  context_compaction_failed: 'provider',
  context_compaction_unavailable: 'protocol',
  context_window_exceeded: 'resource_limit',
  conversation_not_routed: 'validation',
  duplicate_request_id: 'conflict',
  fast_mode_unsupported: 'validation',
  filesystem_already_exists: 'conflict',
  filesystem_file_locked: 'conflict',
  goal_objective_too_long: 'validation',
  goal_state_changed: 'conflict',
  hugging_face_download_incomplete: 'network',
  hugging_face_revision_changed: 'conflict',
  image_collection_name_exists: 'conflict',
  image_generation_failed: 'provider',
  image_generation_incomplete: 'provider',
  local_service_output_failed: 'protocol',
  local_service_protocol_failed: 'protocol',
  native_bridge_failed: 'protocol',
  opencode_free_tier_restricted: 'authorization',
  personal_memory_disabled: 'authorization',
  plan_mode_tools_disabled: 'authorization',
  project_history_context_cleanup_failed: 'storage',
  project_instructions_invalid_encoding: 'validation',
  project_memory_assistant_saves_disabled: 'authorization',
  project_memory_disabled: 'authorization',
  project_memory_invalid_content: 'validation',
  project_not_git_repository: 'validation',
  project_skill_invalid_encoding: 'validation',
  project_skill_invalid_id: 'validation',
  project_skill_invalid_selection: 'validation',
  project_skill_missing: 'not_found',
  project_skills_too_many: 'resource_limit',
  project_task_changed: 'conflict',
  question_delivery_failed: 'unavailable',
  question_route_changed: 'conflict',
  question_run_checkpoint_missing: 'storage',
  question_wait_interrupted: 'cancelled',
  refresh_failed: 'network',
  refresh_rejected: 'authentication',
  request_too_complex: 'resource_limit',
  security_scan_already_exists: 'conflict',
  security_scan_not_active: 'conflict',
  security_scan_tools_unsupported: 'validation',
  semantic_memory_prepare_failed: 'storage',
  semantic_memory_progress_failed: 'unavailable',
  serialization_failed: 'protocol',
  title_generation_failed: 'provider',
  title_generation_incomplete: 'provider',
  tool_not_available: 'authorization',
  tool_execution_unavailable: 'unavailable',
};

export interface LocalServiceErrorMetadata {
  category?: ServiceErrorCategory;
  diagnosticCode?: string | null;
  correlationId?: string | null;
  retryAfterMs?: number | null;
}

/** Stable sidecar failure contract, including whether retrying may succeed. */
export class LocalServiceError extends Error {
  readonly category: ServiceErrorCategory;
  readonly diagnosticCode: string | null;
  readonly correlationId: string | null;
  readonly retryAfterMs: number | null;
  /** @deprecated Use diagnosticCode. */
  readonly details: string | null;

  constructor(
    readonly code: string,
    message: string,
    readonly retryable = false,
    details: string | null = null,
    metadata: LocalServiceErrorMetadata = {},
  ) {
    super(message);
    this.name = 'LocalServiceError';
    const diagnosticCode = isSafeDiagnosticCode(metadata.diagnosticCode) ? metadata.diagnosticCode : details;
    this.category = metadata.category ?? classifyServiceErrorCode(code);
    this.diagnosticCode = isSafeDiagnosticCode(diagnosticCode) ? diagnosticCode : null;
    this.details = this.diagnosticCode;
    this.correlationId = isCorrelationId(metadata.correlationId) ? metadata.correlationId : null;
    this.retryAfterMs = isRetryAfterMs(metadata.retryAfterMs) ? metadata.retryAfterMs : null;
  }
}

/** Returns stable class and code details without exposing arbitrary rejected values. */
export function localServiceErrorDetails(error: unknown): string | null {
  const contract = readServiceErrorContract(error);
  if (!contract) return null;
  const diagnostic = [
    `Class: ${contract.category}`,
    `Code: ${contract.code}`,
    contract.diagnosticCode ? `Diagnostic: ${contract.diagnosticCode}` : null,
    contract.correlationId ? `Reference: ${contract.correlationId}` : null,
  ].filter((value): value is string => typeof value === 'string' && value.length > 0);
  return diagnostic.join('\n').slice(0, MAX_DIAGNOSTIC_TRUNCATION_LENGTH);
}

interface ServiceErrorContract {
  code: string;
  category: ServiceErrorCategory;
  diagnosticCode: string | null;
  correlationId: string | null;
}

function readServiceErrorContract(error: unknown): ServiceErrorContract | null {
  if (error instanceof LocalServiceError) {
    return {
      code: error.code,
      category: error.category,
      diagnosticCode: error.diagnosticCode,
      correlationId: error.correlationId,
    };
  }

  let value: unknown = error;
  if (value instanceof Error) value = value.message;
  if (typeof value === 'string' && value.length <= MAX_ERROR_TRUNCATION_LENGTH) {
    try {
      const parsed: unknown = JSON.parse(value);
      value = parsed;
    } catch {
      return null;
    }
  }
  if (!isJsonObject(value) || !isSafeDiagnosticCode(value.code)) return null;

  return {
    code: value.code,
    category: isServiceErrorCategory(value.category) ? value.category : classifyServiceErrorCode(value.code),
    diagnosticCode: isSafeDiagnosticCode(value.diagnosticCode) ? value.diagnosticCode : null,
    correlationId: isCorrelationId(value.correlationId) ? value.correlationId : null,
  };
}

// Error message truncation limits
const MAX_DIAGNOSTIC_TRUNCATION_LENGTH = 2_048;
const MAX_ERROR_TRUNCATION_LENGTH = 1_000;

/** Resolver and optional event callback retained until one RPC request completes. */
interface PendingOperation {
  requestBytes: number;
  writePending: boolean;
  isChatOperation: boolean;
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
  | { type: 'failure'; generation: number; failure: NativeServiceFailure }
  | { type: 'stopped'; generation: number };

interface NativeServiceFailure {
  code: string;
  category: ServiceErrorCategory;
  message: string;
  diagnosticCode?: string;
  correlationId: string;
  retryable: boolean;
}

/**
 * Manages startup, serialized sidecar writes, RPC response matching, and operation cancellation.
 * A client instance is intended to own one sidecar process for the application lifetime.
 */
export class LocalServiceClient {
  private processGeneration: number | undefined;
  private earlyStoppedGeneration: number | undefined;
  private earlyProcessFailures = new Map<number, LocalServiceError>();
  private processFailure: { generation: number; error: LocalServiceError } | undefined;
  private lastProcessFailure: LocalServiceError | null = null;
  private processEventListener: Promise<void> | undefined;
  private nextRequestId = 0;
  private pending = new Map<number, PendingOperation>();
  private activeChatOperationCount = 0;
  private activeChatRequestBytes = 0;
  private activeRequestBytes = 0;
  private orphanedWriteCount = 0;
  private orphanedWriteBytes = 0;
  private writeQueue: Promise<void> = Promise.resolve();
  private startup: Promise<void> | undefined;
  private processStartup: Promise<void> | undefined;
  private initialized = false;
  private contextUsageSupported = false;
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
      throw new LocalServiceError(
        'startup_settings_unavailable',
        'The local service startup settings are unavailable.',
      );
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
    if (contextUsageMethods.has(method) && !this.contextUsageSupported) {
      throw unsupportedContextUsageError();
    }
    try {
      return await this.callRaw(method, params, timeoutMs);
    } catch (error) {
      if (contextUsageMethods.has(method) && error instanceof LocalServiceError && error.code === 'method_not_found') {
        this.contextUsageSupported = false;
        throw new LocalServiceError(
          'service_protocol_mismatch',
          'The local service advertised context usage support but does not implement the requested method.',
          false,
          'context_usage_rpc_missing',
        );
      }
      throw error;
    }
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
      permissionLocale: await getNativeDialogLocale(),
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
    const [chatSchemaVersion] = await Promise.all([getChatSchemaVersion(), this.startProcess(startWithEmptyStorage)]);

    const initialization = await this.callRaw('system.initialize', { chatSchemaVersion }, requestTimeoutMs);
    if (initialization.status !== 'ready') {
      throw new LocalServiceError(
        'storage_initialization_failed',
        'Local conversation storage could not be initialized.',
      );
    }
    this.contextUsageSupported =
      isJsonObject(initialization.capabilities) && initialization.capabilities.contextUsageProtocolVersion === 1;
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
      const startupFailure = readNativeStartupFailure(error);
      if (startupFailure) {
        throw new LocalServiceError(
          startupFailure.code,
          startupFailure.message,
          startupFailure.retryable,
          startupFailure.diagnosticCode ?? null,
          {
            category: startupFailure.category,
            diagnosticCode: startupFailure.diagnosticCode ?? null,
            correlationId: startupFailure.correlationId,
          },
        );
      }
      throw new LocalServiceError(
        'service_start_failed',
        'The local service could not be started.',
        true,
        null,
        { category: 'process' },
      );
    }
    if (!Number.isSafeInteger(generation) || generation <= 0) {
      throw new LocalServiceError(
        'invalid_service_generation',
        'The local service returned an invalid process identifier.',
      );
    }
    this.processGeneration = generation;
    this.contextUsageSupported = false;
    const earlyFailure = this.earlyProcessFailures.get(generation);
    this.processFailure = earlyFailure ? { generation, error: earlyFailure } : undefined;
    this.earlyProcessFailures.delete(generation);
    if (this.earlyStoppedGeneration === generation) {
      this.earlyStoppedGeneration = undefined;
      this.markProcessStopped(generation, this.processFailure?.error);
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
        const failure = {
          generation: event.payload.generation,
          error: new LocalServiceError(
            event.payload.failure.code,
            event.payload.failure.message,
            event.payload.failure.retryable,
            event.payload.failure.diagnosticCode ?? null,
            {
              category: event.payload.failure.category,
              diagnosticCode: event.payload.failure.diagnosticCode ?? null,
              correlationId: event.payload.failure.correlationId,
            },
          ),
        };
        if (event.payload.generation === this.processGeneration) {
          this.processFailure = failure;
        } else if (this.processGeneration === undefined && this.processStartup) {
          this.earlyProcessFailures.set(failure.generation, failure.error);
        }
        return;
      }
      if (event.payload.generation === this.processGeneration) {
        const failure =
          this.processFailure?.generation === event.payload.generation ? this.processFailure.error : undefined;
        this.markProcessStopped(event.payload.generation, failure);
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
  private markProcessStopped(generation: number, failure?: LocalServiceError): void {
    if (this.processGeneration !== generation) return;
    this.processGeneration = undefined;
    this.initialized = false;
    this.contextUsageSupported = false;
    this.initializationSettings = undefined;
    this.writeQueue = Promise.resolve();
    this.processFailure = undefined;
    this.lastProcessFailure =
      failure ??
      new LocalServiceError('service_unavailable', 'The local service stopped unexpectedly.', true, null, {
        category: 'unavailable',
      });
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
    const isChatOperation = method === 'chat.send' || method === 'chat.temporary.send';
    const exceedsPendingChatOperationLimit =
      isChatOperation && this.activeChatOperationCount >= maximumActiveChatOperations;
    const exceedsPendingChatBytesLimit =
      isChatOperation && this.activeChatRequestBytes + requestBytes > maximumActiveChatRequestBytes;
    const exceedsPendingOperationLimit =
      this.pending.size + this.orphanedWriteCount >= maximumActiveOperations;
    const exceedsPendingBytesLimit =
      this.activeRequestBytes + this.orphanedWriteBytes + requestBytes > maximumActiveRequestBytes;
    if (
      exceedsPendingChatOperationLimit ||
      exceedsPendingChatBytesLimit ||
      exceedsPendingOperationLimit ||
      exceedsPendingBytesLimit
    ) {
      throw new LocalServiceError(
        'service_capacity_exceeded',
        'The local service is handling too many requests. Wait for current work to finish and try again.',
        true,
        exceedsPendingChatOperationLimit
          ? 'client_pending_chat_operation_limit'
          : exceedsPendingChatBytesLimit
            ? 'client_pending_chat_bytes_limit'
          : exceedsPendingOperationLimit
            ? 'client_pending_operation_limit'
            : 'client_pending_bytes_limit',
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
      isChatOperation,
      resolve: resolveResult,
      reject: rejectResult,
      onEvent,
    });
    if (isChatOperation) {
      this.activeChatOperationCount += 1;
      this.activeChatRequestBytes += requestBytes;
    }
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
          permissionLocale: await getNativeDialogLocale(),
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
      const code = isSafeDiagnosticCode(response.error.code) ? response.error.code : 'service_error';
      const message =
        typeof response.error.message === 'string' ? response.error.message : 'The local service request failed.';
      const legacyDetails = typeof response.error.details === 'string' ? response.error.details : null;
      const diagnosticCode = isSafeDiagnosticCode(response.error.diagnosticCode)
        ? response.error.diagnosticCode
        : legacyDetails;
      const category = isServiceErrorCategory(response.error.category) ? response.error.category : undefined;
      const correlationId = isCorrelationId(response.error.correlationId) ? response.error.correlationId : null;
      const retryAfterMs = isRetryAfterMs(response.error.retryAfterMs) ? response.error.retryAfterMs : null;
      pending.reject(
        new LocalServiceError(code, message.slice(0, MAX_ERROR_TRUNCATION_LENGTH), response.error.retryable === true, null, {
          category,
          diagnosticCode,
          correlationId,
          retryAfterMs,
        }),
      );
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
    if (pending.isChatOperation) {
      this.activeChatOperationCount -= 1;
      this.activeChatRequestBytes -= pending.requestBytes;
    }
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

function isSafeDiagnosticCode(value: unknown): value is string {
  return typeof value === 'string' && /^[a-z][a-z0-9_]{0,95}$/u.test(value);
}

function isCorrelationId(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{32}$/u.test(value);
}

function isRetryAfterMs(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 86_400_000;
}

function isServiceErrorCategory(value: unknown): value is ServiceErrorCategory {
  return (
    value === 'validation' ||
    value === 'authentication' ||
    value === 'authorization' ||
    value === 'not_found' ||
    value === 'conflict' ||
    value === 'rate_limit' ||
    value === 'network' ||
    value === 'provider' ||
    value === 'storage' ||
    value === 'filesystem' ||
    value === 'process' ||
    value === 'protocol' ||
    value === 'timeout' ||
    value === 'resource_limit' ||
    value === 'cancelled' ||
    value === 'unavailable' ||
    value === 'internal'
  );
}

export function classifyServiceErrorCode(code: string): ServiceErrorCategory {
  const exactCategory = Object.hasOwn(explicitServiceErrorCategories, code)
    ? explicitServiceErrorCategories[code]
    : undefined;
  if (exactCategory) return exactCategory;

  if (code.includes('cancel') || code.includes('aborted')) return 'cancelled';
  if (
    code.includes('authorization') ||
    code.includes('permission') ||
    code.includes('approval') ||
    code.includes('denied') ||
    code.includes('forbidden') ||
    code.includes('outside_project') ||
    code.includes('unsafe')
  ) {
    return 'authorization';
  }
  if (
    code.includes('authentication') ||
    code.includes('credential') ||
    code.includes('passphrase') ||
    code.includes('auth_') ||
    code.includes('oauth_') ||
    code.includes('unauthorized') ||
    code === 'invalid_api_key'
  ) {
    return 'authentication';
  }
  if (code.includes('not_found')) return 'not_found';
  if (code.includes('rate_limit') || code.includes('rate_limited') || code.includes('quota') || code.includes('usage_limit')) {
    return 'rate_limit';
  }
  if (
    code.includes('conflict') ||
    code.includes('stale') ||
    code.includes('already_active') ||
    code.includes('already_reverted') ||
    code.includes('in_progress')
  ) {
    return 'conflict';
  }
  if (
    code.includes('storage') ||
    code.includes('sqlite') ||
    (code.includes('database') && !code.includes('queue_full') && !code.includes('queue_timeout')) ||
    code.endsWith('_save_failed') ||
    code.endsWith('_load_failed') ||
    code.endsWith('_persistence_failed')
  ) {
    return 'storage';
  }
  if (code.includes('timeout') || code.includes('timed_out')) return 'timeout';
  if (
    code.includes('network') ||
    code.includes('connection_refused') ||
    code.includes('connection_reset') ||
    code === 'connection_unavailable' ||
    code.includes('dns') ||
    code.endsWith('_search_failed') ||
    code.endsWith('_read_url_failed') ||
    code.endsWith('_navigation_failed')
  ) {
    return 'network';
  }
  if (code.includes('filesystem') || code.includes('file_') || code.includes('path_') || code.includes('workspace') || code.includes('worktree')) {
    return 'filesystem';
  }
  if (
    code.includes('protocol') ||
    code === 'invalid_service_response' ||
    code.includes('schema_version') ||
    code.startsWith('rpc_')
  ) {
    return 'protocol';
  }
  if (code.includes('provider') || code.includes('model') || code.includes('inference') || code.startsWith('response_')) {
    return 'provider';
  }
  if (
    code.includes('process') ||
    code.includes('terminal') ||
    code.startsWith('local_engine') ||
    code.startsWith('mcp_server_') ||
    code.startsWith('service_start_') ||
    code === 'command_failed' ||
    code === 'mcp_tool_failed' ||
    code.startsWith('project_tool_') ||
    code.startsWith('computer_use_') ||
    code.startsWith('project_task_')
  ) {
    return 'process';
  }
  if (code.includes('limit') || code.includes('too_large') || code.includes('queue_full') || code.includes('capacity') || code.includes('busy')) {
    return 'resource_limit';
  }
  if (code.includes('unavailable') || code === 'local_service_operation_failed') return 'unavailable';
  if (
    code.startsWith('invalid_') ||
    code.startsWith('unsupported_') ||
    code.startsWith('missing_') ||
    code.startsWith('required_') ||
    code.endsWith('_invalid') ||
    code.endsWith('_required') ||
    code.endsWith('_empty')
  ) {
    return 'validation';
  }
  return 'internal';
}

/** Narrows parsed JSON to a non-array object for RPC contract validation. */
function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function unsupportedContextUsageError(): LocalServiceError {
  return new LocalServiceError(
    'service_capability_unavailable',
    'The local service does not support detailed context usage estimates.',
    false,
    'context_usage_rpc_missing',
  );
}

function readErrorDetails(error: unknown): string | null {
  const detail = error instanceof Error ? error.message : typeof error === 'string' ? error : null;
  return detail?.trim().slice(0, MAX_ERROR_TRUNCATION_LENGTH) || null;
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
  if (value.type === 'failure') return isNativeServiceFailure(value.failure);
  return value.type === 'stopped';
}

function isNativeServiceFailure(value: unknown): value is NativeServiceFailure {
  return (
    isJsonObject(value) &&
    isSafeDiagnosticCode(value.code) &&
    isServiceErrorCategory(value.category) &&
    typeof value.message === 'string' &&
    value.message.length > 0 &&
    value.message.length <= 512 &&
    !containsControlCharacters(value.message) &&
    (value.diagnosticCode === undefined || isSafeDiagnosticCode(value.diagnosticCode)) &&
    isCorrelationId(value.correlationId) &&
    typeof value.retryable === 'boolean'
  );
}

function readNativeStartupFailure(error: unknown): NativeServiceFailure | null {
  let value: unknown = error;
  if (value instanceof Error) value = value.message;
  if (typeof value === 'string' && value.length <= MAX_DIAGNOSTIC_TRUNCATION_LENGTH) {
    try {
      const parsed: unknown = JSON.parse(value);
      value = parsed;
    } catch {
      return null;
    }
  }
  return isNativeServiceFailure(value) ? value : null;
}

function containsControlCharacters(value: string): boolean {
  return Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0);
    return codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f);
  });
}

function isChatSchemaVersion(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 100;
}
