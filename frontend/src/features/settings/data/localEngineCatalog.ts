import {
  LocalServiceError,
  type JsonObject,
  type ServiceEvent,
} from '@/shared/services/LocalServiceClient';

export interface EngineVariant {
  variantId: string;
  os: string;
  architecture: string;
  accelerator: string;
  canInstall: boolean;
  installed: boolean;
  recommended: boolean;
  status: VariantStatus;
  runtimeRequirements: string[];
}

export type EngineCatalogStatus = 'installable' | 'blocked' | 'deprecated';
export type RuntimeStatus = 'running' | 'stopped' | 'unhealthy' | 'unavailable';
export type VariantStatus =
  | 'available'
  | 'installed'
  | 'update_available'
  | 'deprecated'
  | 'blocked'
  | 'unsupported_platform'
  | 'hardware_unavailable'
  | 'driver_unsupported'
  | 'driver_version_unavailable';
export type InstallPhase = 'downloading' | 'verifying' | 'extracting' | 'publishing';

export interface LocalEngine {
  engineId: string;
  displayName: string;
  releaseTag: string;
  latestUpstreamReleaseTag: string | null;
  releaseCheckStatus: string;
  channel: string;
  catalogStatus: EngineCatalogStatus;
  statusReason: string | null;
  modelDirectory: string;
  runtimeStatus: RuntimeStatus;
  variants: EngineVariant[];
}

export interface LocalModel {
  id: string;
  engineId: string;
  displayName: string;
  path: string;
  pathKind: 'file' | 'directory';
  pathExists: boolean;
  isAvailable: boolean;
  reason: string | null;
}

export interface ExternalServer {
  processId: number;
  port: number;
  modelIds: string[];
}

export interface ExternalCandidate {
  processId: number;
  port: number;
}

export interface EngineCatalog {
  engines: LocalEngine[];
  models: LocalModel[];
  runningModelIds: string[];
  externalServers: ExternalServer[];
  executablePath: string | null;
  executableAvailable: boolean;
  hostOs: string;
}

export interface InstallProgress {
  engineId: string;
  variantId: string;
  phase: InstallPhase;
  assetName: string;
  assetIndex: number | null;
  assetCount: number | null;
  downloadedBytes: number;
  totalBytes: number;
}

export interface PendingRegistration {
  engineId: string;
  path: string;
  pathKind: 'file' | 'directory';
}

export interface DiscoveryResult {
  models: Array<{ engineId: string; displayName: string; path: string; pathKind: 'file' | 'directory' }>;
  truncated: boolean;
}

type TranslationKey = keyof typeof import('@/i18n/locales/en.json');

export interface Notice {
  key: TranslationKey;
  values?: Record<string, string | number>;
}

export type LoadState = 'loading' | 'ready' | 'error';
export type BusyAction = 'install' | 'start' | 'scan' | 'register' | 'remove' | 'executable' | 'external';
export type EngineErrorKey =
  | 'localEnginesLoadFailed'
  | 'localEngineInstallFailed'
  | 'localModelActionError'
  | 'localEngineSettingsFailed'
  | 'localEngineExternalServerScanFailed'
  | 'localEngineExternalServerConnectFailed';
export type VariantStatusKey =
  | 'localEngineAvailable'
  | 'localEngineUpdateAvailable'
  | 'localEngineBlocked'
  | 'localEngineDeprecated'
  | 'localEngineUnsupportedPlatform'
  | 'localEngineHardwareUnavailable'
  | 'localEngineDriverUnsupported'
  | 'localEngineDriverVersionUnavailable'
  | 'localEngineNotInstalled'
  | 'localEngineInstalled';
export type InstallPhaseKey =
  | 'localEngineStageDownloading'
  | 'localEngineStageVerifying'
  | 'localEngineStageExtracting'
  | 'localEngineStagePublishing';
export type RuntimeStatusKey =
  | 'localEngineRunning'
  | 'localEngineStopped'
  | 'localEngineUnhealthy'
  | 'localEngineUnavailable';

export function parseCatalog(value: JsonObject): EngineCatalog {
  if (!Array.isArray(value.engines) || !Array.isArray(value.models) || !isObject(value.runtime) || !Array.isArray(value.externalLlamaServers)) {
    throw new Error('The local engine catalog is invalid.');
  }
  const runtime = value.runtime;
  const servers = runtime.servers;
  if (!Array.isArray(servers)) throw new Error('The local engine runtime catalog is invalid.');
  const runningModelIds = servers.flatMap((server: unknown): string[] => {
    if (!isObject(server) || typeof server.status !== 'string') throw new Error('The runtime status is invalid.');
    return server.status === 'running' && typeof server.modelId === 'string' ? [server.modelId] : [];
  });
  return {
    engines: value.engines.map(parseEngine),
    models: value.models.map(parseModel),
    runningModelIds,
    externalServers: value.externalLlamaServers.map(parseExternalServer),
    executablePath: optionalString(value.llamaServerExecutablePath),
    executableAvailable: value.llamaServerExecutableAvailable === true,
    hostOs: parseHostOs(value.host),
  };
}

export function parseExternalCandidate(value: unknown): ExternalCandidate[] {
  if (!isObject(value) || !positiveInteger(value.processId) || !positiveInteger(value.port)) return [];
  return [{ processId: value.processId, port: value.port }];
}

export function parseDiscovery(value: JsonObject): DiscoveryResult {
  if (!Array.isArray(value.models) || typeof value.truncated !== 'boolean') throw new Error('The model discovery response is invalid.');
  const models = value.models.flatMap((item): DiscoveryResult['models'] => {
    if (!isObject(item) || typeof item.engineId !== 'string' || typeof item.displayName !== 'string' || typeof item.path !== 'string' || (item.pathKind !== 'file' && item.pathKind !== 'directory')) return [];
    return [{ engineId: item.engineId, displayName: item.displayName, path: item.path, pathKind: item.pathKind }];
  });
  return { models, truncated: value.truncated };
}

export function parseInstallProgress(event: ServiceEvent): InstallProgress | null {
  if (event.name !== 'local.engines.install.progress' || typeof event.data.engineId !== 'string' || typeof event.data.variantId !== 'string' || !isInstallPhase(event.data.phase) || typeof event.data.assetName !== 'string' || !nonNegativeNumber(event.data.downloadedBytes) || !nonNegativeNumber(event.data.totalBytes)) return null;
  return {
    engineId: event.data.engineId,
    variantId: event.data.variantId,
    phase: event.data.phase,
    assetName: event.data.assetName,
    assetIndex: nonNegativeIntegerOrNull(event.data.assetIndex),
    assetCount: nonNegativeIntegerOrNull(event.data.assetCount),
    downloadedBytes: event.data.downloadedBytes,
    totalBytes: event.data.totalBytes,
  };
}

export function variantStatusKey(status: VariantStatus): VariantStatusKey {
  switch (status) {
    case 'available': return 'localEngineAvailable';
    case 'installed': return 'localEngineInstalled';
    case 'update_available': return 'localEngineUpdateAvailable';
    case 'blocked': return 'localEngineBlocked';
    case 'deprecated': return 'localEngineDeprecated';
    case 'unsupported_platform': return 'localEngineUnsupportedPlatform';
    case 'hardware_unavailable': return 'localEngineHardwareUnavailable';
    case 'driver_unsupported': return 'localEngineDriverUnsupported';
    case 'driver_version_unavailable': return 'localEngineDriverVersionUnavailable';
  }
}

export function channelStatusKey(channel: string): TranslationKey | null {
  switch (channel) {
    case 'stable': return 'localEngineStable';
    case 'preview': return 'localEnginePreview';
    case 'nightly': return 'localEngineNightly';
    default: return null;
  }
}

export function catalogStatusKey(status: EngineCatalogStatus): TranslationKey {
  switch (status) {
    case 'installable': return 'localEngineAvailable';
    case 'blocked': return 'localEngineBlocked';
    case 'deprecated': return 'localEngineDeprecated';
  }
}

export function acceleratorLabel(accelerator: string): string {
  switch (accelerator) {
    case 'cuda': return 'CUDA';
    case 'vulkan': return 'Vulkan';
    case 'metal': return 'Metal';
    case 'xpu': return 'XPU';
    case 'cpu': return 'CPU';
    default: return accelerator;
  }
}

export function runtimeStatusKey(status: RuntimeStatus): RuntimeStatusKey {
  switch (status) {
    case 'running': return 'localEngineRunning';
    case 'stopped': return 'localEngineStopped';
    case 'unhealthy': return 'localEngineUnhealthy';
    case 'unavailable': return 'localEngineUnavailable';
  }
}

export function installPhaseKey(phase: InstallPhase): InstallPhaseKey {
  switch (phase) {
    case 'downloading': return 'localEngineStageDownloading';
    case 'verifying': return 'localEngineStageVerifying';
    case 'extracting': return 'localEngineStageExtracting';
    case 'publishing': return 'localEngineStagePublishing';
  }
}

export function formatBytes(bytes: number, locale: string): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let amount = bytes;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit++;
  }
  return `${amount.toLocaleString(locale, { maximumFractionDigits: unit === 0 ? 0 : 1 })} ${units[unit]}`;
}

export function modelActionErrorKey(error: unknown): EngineErrorKey {
  if (error instanceof LocalServiceError) {
    switch (error.code) {
      case 'local_engine_process_scan_failed': return 'localEngineExternalServerScanFailed';
      case 'local_engine_external_server_auth_required':
      case 'local_engine_external_server_not_found':
      case 'local_engine_external_server_not_ready':
      case 'local_engine_external_server_catalog_invalid':
      case 'local_engine_external_server_capabilities_invalid': return 'localEngineExternalServerConnectFailed';
      case 'local_engine_settings_unavailable':
      case 'local_engine_executable_path_invalid': return 'localEngineSettingsFailed';
      default: return 'localModelActionError';
    }
  }
  return 'localModelActionError';
}

function parseEngine(value: unknown): LocalEngine {
  if (!isObject(value) || typeof value.engineId !== 'string' || typeof value.displayName !== 'string' || typeof value.releaseTag !== 'string' || typeof value.channel !== 'string' || !isEngineCatalogStatus(value.catalogStatus) || !isRuntimeStatus(value.runtimeStatus) || !Array.isArray(value.variants) || typeof value.modelDirectory !== 'string') throw new Error('The local engine entry is invalid.');
  return {
    engineId: value.engineId,
    displayName: value.displayName,
    releaseTag: value.releaseTag,
    latestUpstreamReleaseTag: optionalString(value.latestUpstreamReleaseTag),
    releaseCheckStatus: typeof value.releaseCheckStatus === 'string' ? value.releaseCheckStatus : 'not_checked',
    channel: value.channel,
    catalogStatus: value.catalogStatus,
    statusReason: optionalString(value.statusReason),
    modelDirectory: value.modelDirectory,
    runtimeStatus: value.runtimeStatus,
    variants: value.variants.map(parseVariant),
  };
}

function parseVariant(value: unknown): EngineVariant {
  if (!isObject(value) || typeof value.variantId !== 'string' || typeof value.os !== 'string' || typeof value.architecture !== 'string' || typeof value.accelerator !== 'string' || typeof value.canInstall !== 'boolean' || typeof value.installed !== 'boolean' || typeof value.recommended !== 'boolean' || !isVariantStatus(value.status) || !Array.isArray(value.runtimeRequirements) || !value.runtimeRequirements.every((item) => typeof item === 'string')) throw new Error('The local engine variant is invalid.');
  return {
    variantId: value.variantId,
    os: value.os,
    architecture: value.architecture,
    accelerator: value.accelerator,
    canInstall: value.canInstall,
    installed: value.installed,
    recommended: value.recommended,
    status: value.status,
    runtimeRequirements: value.runtimeRequirements,
  };
}

function parseModel(value: unknown): LocalModel {
  if (!isObject(value) || typeof value.id !== 'string' || typeof value.engineId !== 'string' || typeof value.displayName !== 'string' || typeof value.path !== 'string' || (value.pathKind !== 'file' && value.pathKind !== 'directory') || typeof value.pathExists !== 'boolean' || typeof value.isAvailable !== 'boolean') throw new Error('The local model entry is invalid.');
  return {
    id: value.id,
    engineId: value.engineId,
    displayName: value.displayName,
    path: value.path,
    pathKind: value.pathKind,
    pathExists: value.pathExists,
    isAvailable: value.isAvailable,
    reason: optionalString(value.reason),
  };
}

function parseExternalServer(value: unknown): ExternalServer {
  if (!isObject(value) || !positiveInteger(value.processId) || !positiveInteger(value.port) || !Array.isArray(value.modelIds) || !value.modelIds.every((modelId) => typeof modelId === 'string')) throw new Error('The external server status is invalid.');
  return { processId: value.processId, port: value.port, modelIds: value.modelIds };
}

function parseHostOs(value: unknown): string {
  if (!isObject(value) || typeof value.os !== 'string') return 'unknown';
  return value.os;
}

function optionalString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function positiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}

function nonNegativeIntegerOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function nonNegativeNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function isInstallPhase(value: unknown): value is InstallPhase {
  return value === 'downloading' || value === 'verifying' || value === 'extracting' || value === 'publishing';
}

function isEngineCatalogStatus(value: unknown): value is EngineCatalogStatus {
  return value === 'installable' || value === 'blocked' || value === 'deprecated';
}

function isRuntimeStatus(value: unknown): value is RuntimeStatus {
  return value === 'running' || value === 'stopped' || value === 'unhealthy' || value === 'unavailable';
}

function isVariantStatus(value: unknown): value is VariantStatus {
  return value === 'available' || value === 'installed' || value === 'update_available' || value === 'deprecated' || value === 'blocked' || value === 'unsupported_platform' || value === 'hardware_unavailable' || value === 'driver_unsupported' || value === 'driver_version_unavailable';
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
