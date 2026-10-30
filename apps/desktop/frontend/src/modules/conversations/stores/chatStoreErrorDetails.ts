import type { TFunction } from 'i18next';
import { LocalServiceError } from '@/platform/local-service';

const knownServiceErrorKeyMap: Record<string, string> = {
  'chat history could not be updated.': 'chatHistoryUnavailable',
  'chat history could not be read.': 'chatHistoryUnavailable',
  'chat context usage could not be measured.': 'chatContextUsageUnavailable',
  'local conversation storage is unavailable.': 'storageUnavailable',
  'database operation queue wait timed out': 'databaseQueueTimeout',
  'database operation queue is full': 'databaseQueueFull',
  'the model provider returned a response openchat could not read.': 'invalidProviderResponse',
  'the provider requested a tool that was not included in the request.': 'providerRequestedUnlistedTool',
  'the selected model is no longer available. choose another model.': 'modelUnavailable',
  'the requested conversation was not found.': 'conversationNotFound',
  'the requested message was not found.': 'messageNotFound',
  'the assistant response could not be saved.': 'messageSaveFailed',
  'the conversation could not be opened.': 'conversationUnavailable',
  'loading conversation messages timed out.': 'localServiceTimeout',
  'the saved chat goal is invalid.': 'securityScanStatusInvalid',
  'the saved chat goal checklist is invalid.': 'securityScanStatusInvalid',
};

const serviceErrorCodeKeyMap: Record<string, string> = {
  opencode_free_tier_restricted: 'openCodeFreeTierRestricted',
  opencode_free_usage_limit: 'openCodeFreeUsageLimit',
  authentication_required: 'providerAuthenticationRequired',
  refresh_rejected: 'providerAuthenticationRequired',
  invalid_api_key: 'providerAuthenticationRequired',
  provider_credentials_missing: 'providerCredentialMissing',
  rate_limited: 'providerRateLimited',
  provider_tool_request_rejected: 'providerToolRequestRejected',
  network_unavailable: 'providerNetworkUnavailable',
  model_unavailable: 'selectedModelUnavailable',
  local_engine_not_installed: 'localModelEngineNotReady',
  local_engine_unavailable: 'localModelEngineNotReady',
  local_model_unavailable: 'localModelInvalid',
  local_engine_start_failed: 'localModelStartError',
  local_engine_start_timeout: 'localModelStartTimeout',
  local_engine_runtime_unavailable: 'localModelRuntimeUnavailable',
  local_engine_capability_unavailable: 'localModelContextUnavailable',
  local_model_inference_failed: 'localModelInferenceFailed',
  context_window_exceeded: 'contextWindowExceeded',
  context_compaction_input_too_large: 'contextWindowExceeded',
  context_compaction_failed: 'contextWindowExceeded',
  attachment_unavailable: 'attachmentUnavailable',
  model_does_not_support_images: 'modelDoesNotSupportImages',
  provider_request_failed: 'providerRequestFailed',
  invalid_provider_response: 'invalidProviderResponse',
  provider_unlisted_tool: 'providerRequestedUnlistedTool',
  response_too_large: 'providerResponseTooLarge',
  tool_response_limit: 'toolResponseLimit',
  message_save_failed: 'messageSaveFailed',
  attachment_save_failed: 'attachmentSaveFailed',
  storage_unavailable: 'chatHistoryUnavailable',
  storage_chat_unavailable: 'chatHistoryUnavailable',
  goal_storage_unavailable: 'chatHistoryUnavailable',
  security_scan_storage_unavailable: 'chatHistoryUnavailable',
  goal_checkpoint_invalid: 'securityScanStatusInvalid',
  goal_state_changed: 'securityScanStatusInvalid',
  security_scan_completion_summary_required: 'securityScanIncompleteChecklist',
  security_scan_checklist_required: 'securityScanIncompleteChecklist',
  security_scan_checklist_incomplete: 'securityScanIncompleteChecklist',
  service_timeout: 'localServiceTimeout',
  service_unavailable: 'localServiceUnavailable',
  local_service_operation_failed: 'localServiceUnavailable',
  protocol_unavailable: 'localServiceUnavailable',
  conversation_not_found: 'conversationNotFound',
  message_not_found: 'messageNotFound',
  security_scan_request_failed: 'securityScanRequestFailed',
};

export function formatFailureCodeMessage(
  failureCode: string | null | undefined,
  t: TFunction,
  fallbackKey = 'securityScanRequestFailed',
): string | null {
  if (!failureCode) return null;
  const key = serviceErrorCodeKeyMap[failureCode] ?? fallbackKey;
  return t(key, { defaultValue: key });
}

const knownServiceErrorCodeMap: Record<string, string> = {
  'chat history could not be updated.': 'storage_chat_unavailable',
  'chat history could not be read.': 'storage_chat_unavailable',
  'local conversation storage is unavailable.': 'storage_unavailable',
  'loading conversation messages timed out.': 'service_timeout',
  'the saved chat goal is invalid.': 'goal_checkpoint_invalid',
  'the saved chat goal checklist is invalid.': 'goal_checkpoint_invalid',
  'the provider requested a tool that was not included in the request.': 'provider_unlisted_tool',
};

export function resolveServiceFailureCode(error: unknown, fallbackCode: string): string {
  if (error instanceof LocalServiceError) {
    if (
      error.code === 'invalid_provider_response' &&
      error.message.toLowerCase().includes('tool that was not included in the request')
    ) {
      return 'provider_unlisted_tool';
    }
    return error.code;
  }
  if (error instanceof Error) {
    const lower = error.message.trim().toLowerCase();
    const mappedCode = knownServiceErrorCodeMap[lower];
    if (mappedCode) {
      return mappedCode;
    }
    for (const [pattern, code] of Object.entries(knownServiceErrorCodeMap)) {
      if (lower.includes(pattern)) {
        return code;
      }
    }
  }
  return fallbackCode;
}

export function formatUnknownServiceError(error: unknown, t: TFunction, fallbackKey: string): string {
  if (error instanceof LocalServiceError) {
    const resolvedCode = resolveServiceFailureCode(error, error.code);
    const codeKey = serviceErrorCodeKeyMap[resolvedCode];
    if (codeKey) {
      return t(codeKey, { defaultValue: codeKey });
    }
    const lower = error.message.trim().toLowerCase();
    const messageKey = knownServiceErrorKeyMap[lower];
    if (messageKey) {
      return t(messageKey, { defaultValue: messageKey });
    }
    for (const [pattern, key] of Object.entries(knownServiceErrorKeyMap)) {
      if (lower.includes(pattern)) {
        return t(key, { defaultValue: key });
      }
    }
  } else if (error instanceof Error) {
    const lower = error.message.trim().toLowerCase();
    const messageKey = knownServiceErrorKeyMap[lower];
    if (messageKey) {
      return t(messageKey, { defaultValue: messageKey });
    }
    for (const [pattern, key] of Object.entries(knownServiceErrorKeyMap)) {
      if (lower.includes(pattern)) {
        return t(key, { defaultValue: key });
      }
    }
  }
  return t(fallbackKey, { defaultValue: fallbackKey });
}

export function formatServiceErrorMessage(errorMessage: string | null | undefined, t: TFunction): string | null {
  if (!errorMessage) return null;
  const trimmed = errorMessage.trim();
  if (trimmed.length === 0) return null;

  const lookupKey = knownServiceErrorKeyMap[trimmed.toLowerCase()];
  if (lookupKey) {
    return t(lookupKey, { defaultValue: lookupKey });
  }

  for (const [pattern, key] of Object.entries(knownServiceErrorKeyMap)) {
    if (trimmed.toLowerCase().includes(pattern)) {
      return t(key, { defaultValue: key });
    }
  }

  return trimmed;
}

export function isRedundantHistoryError(errorMessage: string | null | undefined): boolean {
  if (!errorMessage) return true;
  const lower = errorMessage.trim().toLowerCase();
  return (
    lower.includes('chat history could not be updated') ||
    lower.includes('chat history could not be read') ||
    lower.includes('storage_unavailable')
  );
}

export function getOpenCodeRetryAtUnixMs(error: unknown, providerId: string): number | null {
  if (
    providerId !== 'opencode' ||
    !(error instanceof LocalServiceError) ||
    (error.code !== 'rate_limited' && error.code !== 'opencode_free_usage_limit')
  ) {
    return null;
  }

  const match = /(?:^|[;\s])retry-after-seconds=(\d+)(?=$|[;\s])/u.exec(error.details ?? '');
  if (!match) return null;

  const retryAfterSeconds = Number(match[1]);
  if (!Number.isSafeInteger(retryAfterSeconds) || retryAfterSeconds < 0) return null;

  const retryAtUnixMs = Date.now() + retryAfterSeconds * 1000;
  if (!Number.isSafeInteger(retryAtUnixMs) || retryAtUnixMs > 8_640_000_000_000_000) return null;
  return retryAtUnixMs;
}
