import {
  BookmarkCheck,
  BookmarkPlus,
  Brain,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Copy,
  GitBranch,
  Link,
  LoaderCircle,
  RefreshCw,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '@/design-system';
import type { ChatFileChange } from '@/modules/conversations/data/chatFileChanges';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import { ProviderIcon } from '@/modules/providers/components/ProviderIcon';
import { parsePersonalMemoryFactIds, readPersonalMemoryFacts } from '@/modules/settings/data/personalMemory';
import { MessageAttachments } from './MessageAttachments';
import { CitationSources } from './message/CitationSources';
import { FileChangesSummaryCard } from './message/FileChangesSummaryCard';
import { MessageCopyMenu } from './message/MessageCopyMenu';
import { MessageMarkdown } from './message/MessageMarkdown';
import { parseCitationSources, parseReasoningSummaries, parseToolActivities } from './message/messageData';
import { ReasoningSummary } from './message/ReasoningSummary';
import { ToolActivityAccordion } from './message/ToolActivityAccordion';

interface PersonalMemorySource {
  id: string;
  content: string;
}

export interface MessageItemProps {
  message: ChatMessage;
  assistantModelFallback?: string | null;
  assistantProviderFallback?: string | null;
  isSearchMatch?: boolean;
  isSavedOutput?: boolean;
  isSavingSavedOutput?: boolean;
  onToggleSavedOutput?: (message: ChatMessage) => void;
  onRetryMessage?: (message: ChatMessage) => void;
  onEditMessage?: (message: ChatMessage) => void;
  onSelectResponseVersion?: (message: ChatMessage, versionIndex: number) => void;
  responseVersionIndex?: number;
  responseVersionCount?: number;
  latestFileChanges?: ChatFileChange[];
  onOpenFileChanges?: () => void;
  onOpenConnections?: () => void;
}

export function MessageItem({
  isSearchMatch = false,
  assistantModelFallback = null,
  assistantProviderFallback = null,
  isSavedOutput = false,
  isSavingSavedOutput = false,
  message,
  onEditMessage,
  onRetryMessage,
  onSelectResponseVersion,
  onToggleSavedOutput,
  responseVersionCount,
  responseVersionIndex,
  latestFileChanges = [],
  onOpenFileChanges,
  onOpenConnections,
}: MessageItemProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const [copyResult, setCopyResult] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [memorySourcesOpen, setMemorySourcesOpen] = useState(false);
  const [memorySources, setMemorySources] = useState<PersonalMemorySource[]>([]);
  const [memorySourcesError, setMemorySourcesError] = useState(false);
  const resetTimer = useRef<number | undefined>(undefined);
  const isUser = message.role === 'user';
  const personalMemoryFactIds = parsePersonalMemoryFactIds(message.personalMemoryFactIds) ?? [];
  const citationSources = isUser ? [] : parseCitationSources(message.citationSources);
  const reasoningSummaries = isUser ? [] : parseReasoningSummaries(message.reasoningSummaries);
  const toolActivities = isUser ? [] : parseToolActivities(message.toolActivities);
  const canRetry = !isUser && onRetryMessage !== undefined && message.status !== 'streaming';
  const canSaveOutput =
    !isUser && onToggleSavedOutput !== undefined && message.status === 'completed' && message.content.trim().length > 0;
  const hasResponseVersions =
    !isUser &&
    onSelectResponseVersion !== undefined &&
    responseVersionIndex !== undefined &&
    responseVersionCount !== undefined &&
    responseVersionCount > 1;

  useEffect(
    () => () => {
      if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    },
    [],
  );

  const copyMessage = async () => {
    try {
      await navigator.clipboard.writeText(message.content);
      setCopyResult('copied');
    } catch {
      setCopyResult('failed');
    }
    if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setCopyResult('idle'), 2000);
  };

  const viewMemorySources = async () => {
    if (personalMemoryFactIds.length === 0) return;
    try {
      const facts = await readPersonalMemoryFacts();
      setMemorySources(
        personalMemoryFactIds.map((id) => ({
          id,
          content: facts.find((fact) => fact.id === id)?.content ?? t('personalMemorySourceUnavailable'),
        })),
      );
      setMemorySourcesError(false);
      setMemorySourcesOpen(true);
    } catch {
      setMemorySourcesError(true);
      setMemorySourcesOpen(true);
    }
  };

  return (
    <article
      aria-current={isSearchMatch ? 'location' : undefined}
      aria-label={isUser ? t('userMessage') : undefined}
      className={`group flex w-full ${isUser ? 'justify-end' : 'justify-start'}`}
      id={`openchat-message-${message.id}`}
    >
      <div
        className={
          isUser
            ? 'max-w-user-message rounded-2xl rounded-br-md bg-selected px-4 py-3'
            : 'w-full max-w-assistant-message px-1 py-2'
        }
      >
        {!isUser ? (
          <AssistantHeader
            modelId={message.modelId ?? assistantModelFallback}
            providerId={message.providerId ?? assistantProviderFallback}
          />
        ) : null}
        {hasResponseVersions ? (
          <ResponseVersionSelector
            count={responseVersionCount ?? 0}
            index={responseVersionIndex ?? 0}
            onSelect={(index) => onSelectResponseVersion?.(message, index)}
          />
        ) : null}
        {!isUser && reasoningSummaries.length > 0 ? (
          <div className="mt-2 grid gap-2">
            {reasoningSummaries
              .filter((summary) => summary.content.trim().length > 0)
              .map((summary) => (
                <ReasoningSummary key={summary.id} summary={summary} />
              ))}
          </div>
        ) : null}
        {!isUser && toolActivities.length > 0 ? (
          <div className="mt-2 grid gap-2">
            {toolActivities.map((activity) => (
              <ToolActivityAccordion activity={activity} key={activity.callId} />
            ))}
          </div>
        ) : null}
        {message.status === 'streaming' && message.content.trim().length === 0 && !isUser ? (
          <ResponseSkeleton />
        ) : message.content ? (
          isUser ? (
            <p className="conversation-message-content whitespace-pre-wrap break-words text-text">{message.content}</p>
          ) : (
            <MessageMarkdown content={message.content} citationSources={citationSources} />
          )
        ) : null}
        {message.status === 'streaming' && message.content.trim().length > 0 && !isUser ? (
          <span
            aria-label={t('responseInProgress')}
            className="ml-1 inline-block h-4 w-1 animate-pulse rounded-xs bg-accent align-middle motion-reduce:animate-none"
            role="status"
          />
        ) : null}
        {message.attachments.length > 0 ? (
          <MessageAttachments
            attachments={message.attachments}
            conversationId={message.conversationId}
            messageId={message.id}
          />
        ) : null}
        {!isUser && message.status === 'failed' ? (
          <FailureCard
            failureCode={message.failureCode}
            onOpenConnections={needsConnectionRecovery(message.failureCode) ? onOpenConnections : undefined}
            onRetry={canRetry ? () => onRetryMessage?.(message) : undefined}
          />
        ) : null}
        <div className={`mt-2 flex flex-wrap items-center gap-1 ${isUser ? 'justify-end' : 'justify-start'}`}>
          {message.createdAt !== null ? (
            <time className="px-1 text-xs text-secondary-text" dateTime={new Date(message.createdAt).toISOString()}>
              {formatTime(message.createdAt, locale)}
            </time>
          ) : null}
          {!isUser ? <ResponseMetrics message={message} /> : null}
          {message.status === 'stopped' ? (
            <span className="px-1 text-xs text-secondary-text">{t('responseStopped')}</span>
          ) : null}
          {canRetry && message.status !== 'failed' ? (
            <Button
              aria-label={t('retry')}
              className="size-8 opacity-70 group-hover:opacity-100"
              onClick={() => onRetryMessage?.(message)}
              size="icon"
              variant="ghost"
            >
              <RefreshCw aria-hidden="true" size={16} />
            </Button>
          ) : null}
          {message.content ? (
            !isUser ? (
              <MessageCopyMenu content={message.content} />
            ) : (
              <Button
                aria-label={t(copyResult === 'copied' ? 'messageCopied' : 'copyMessage')}
                className="size-8 opacity-70 group-hover:opacity-100"
                onClick={() => void copyMessage()}
                size="icon"
                variant="ghost"
              >
                {copyResult === 'copied' ? (
                  <Check aria-hidden="true" size={16} />
                ) : (
                  <Copy aria-hidden="true" size={16} />
                )}
              </Button>
            )
          ) : null}
          {isUser && onEditMessage ? (
            <Button
              aria-label={t('conversationBranchEditTitle')}
              className="size-8 opacity-70 group-hover:opacity-100"
              onClick={() => onEditMessage(message)}
              size="icon"
              variant="ghost"
            >
              <GitBranch aria-hidden="true" size={16} />
            </Button>
          ) : null}
          {canSaveOutput ? (
            <Button
              aria-label={t(isSavedOutput ? 'removeSavedResponse' : 'saveResponse')}
              className={`size-8 opacity-70 group-hover:opacity-100 ${isSavedOutput ? 'text-accent-icon' : ''}`}
              disabled={isSavingSavedOutput}
              onClick={() => onToggleSavedOutput?.(message)}
              size="icon"
              variant="ghost"
            >
              {isSavingSavedOutput ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : isSavedOutput ? (
                <BookmarkCheck aria-hidden="true" size={16} />
              ) : (
                <BookmarkPlus aria-hidden="true" size={16} />
              )}
            </Button>
          ) : null}
          {!isUser && personalMemoryFactIds.length > 0 ? (
            <Button
              aria-label={t('personalMemoryResponseLabel', { count: personalMemoryFactIds.length })}
              onClick={viewMemorySources}
              size="small"
              variant="ghost"
            >
              <Brain aria-hidden="true" size={15} />
              {t('personalMemoryResponseLabel', { count: personalMemoryFactIds.length })}
            </Button>
          ) : null}
        </div>
        {copyResult === 'failed' ? (
          <p className="text-xs text-destructive" role="status">
            {t('messageCopyFailed')}
          </p>
        ) : null}
        {!isUser && message.status !== 'streaming' ? (
          <FileChangesSummaryCard
            activities={toolActivities}
            latestChanges={latestFileChanges}
            onViewChanges={onOpenFileChanges}
          />
        ) : null}
        {citationSources.length > 0 && message.content.trim().length === 0 ? (
          <CitationSources sources={citationSources} />
        ) : null}
      </div>
      <Dialog
        onOpenChange={(open) => {
          if (!open) setMemorySourcesOpen(false);
        }}
        open={memorySourcesOpen}
        title={t('personalMemorySourcesTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('personalMemorySourcesDescription')}</p>
          {memorySourcesError ? (
            <p className="text-sm text-destructive" role="alert">
              {t('personalMemoryLoadFailed')}
            </p>
          ) : (
            <ul className="grid gap-2">
              {memorySources.map((source) => (
                <li
                  className="break-words rounded-control border border-border bg-background p-3 text-sm"
                  key={source.id}
                >
                  {source.content}
                </li>
              ))}
            </ul>
          )}
          <div className="flex justify-end border-t border-border pt-3">
            <Button onClick={() => setMemorySourcesOpen(false)}>{t('close')}</Button>
          </div>
        </div>
      </Dialog>
    </article>
  );
}

function AssistantHeader({ modelId, providerId }: { modelId: string | null; providerId: string | null }) {
  const { t } = useTranslation();
  const modelLabel = modelId?.trim() || t('messageModelUnavailable');
  const providerLabel = providerId?.trim() ? providerName(providerId, t) : null;
  return (
    <div className="flex min-h-5 items-center gap-2 text-sm font-semibold text-text">
      <ProviderIcon className="text-secondary-icon" providerId={providerId ?? 'unknown'} size={18} />
      <span className="min-w-0 truncate">{modelLabel}</span>
      {providerLabel ? (
        <span className="shrink-0 text-xs font-normal text-secondary-text">· {providerLabel}</span>
      ) : null}
    </div>
  );
}

interface ResponseVersionSelectorProps {
  index: number;
  count: number;
  onSelect(index: number): void;
}

function ResponseVersionSelector({ count, index, onSelect }: ResponseVersionSelectorProps) {
  const { t } = useTranslation();
  if (count < 2) return null;
  return (
    <div className="mt-1 flex items-center gap-1">
      <Button
        aria-label={t('previousResponseVersion')}
        className="size-8"
        disabled={index <= 0}
        onClick={() => onSelect(index - 1)}
        size="icon"
        variant="ghost"
      >
        <ChevronLeft aria-hidden="true" size={16} />
      </Button>
      <span aria-live="polite" className="min-w-28 flex-1 text-center text-xs text-secondary-text">
        {t('responseVersionCount', { current: index + 1, total: count })}
      </span>
      <Button
        aria-label={t('nextResponseVersion')}
        className="size-8"
        disabled={index + 1 >= count}
        onClick={() => onSelect(index + 1)}
        size="icon"
        variant="ghost"
      >
        <ChevronRight aria-hidden="true" size={16} />
      </Button>
    </div>
  );
}

function ResponseSkeleton() {
  const { t } = useTranslation();
  return (
    <div aria-label={t('responseInProgress')} className="mt-4 grid gap-2" role="status">
      <span className="h-3 w-3/4 animate-pulse rounded bg-selected motion-reduce:animate-none" />
      <span className="h-3 w-11/12 animate-pulse rounded bg-selected motion-reduce:animate-none" />
      <span className="h-3 w-1/2 animate-pulse rounded bg-selected motion-reduce:animate-none" />
    </div>
  );
}

function ResponseMetrics({ message }: { message: ChatMessage }) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const values: string[] = [];
  if (message.tokensPerSecond !== null) {
    values.push(t('responseTokenRate', { rate: formatNumber(message.tokensPerSecond, 1, locale) }));
  }
  if (message.outputTokens !== null) {
    values.push(t('responseTokenCount', { count: formatNumber(message.outputTokens, 0, locale) }));
  }
  if (message.elapsedMicroseconds !== null) {
    values.push(formatDuration(message.elapsedMicroseconds));
  }
  return values.length > 0 ? <span className="px-1 text-xs text-secondary-text">{values.join(' · ')}</span> : null;
}

function FailureCard({
  failureCode,
  onOpenConnections,
  onRetry,
}: {
  failureCode: string | null;
  onOpenConnections?: () => void;
  onRetry?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="mt-3 flex items-start gap-3 rounded-card border border-destructive/50 bg-destructive/5 px-3 py-3 text-sm"
      role="alert"
    >
      <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0 text-destructive" size={18} />
      <div className="min-w-0 flex-1">
        <p className="font-semibold text-destructive">{t('responseFailed')}</p>
        <p className="mt-1 text-text">{t(failureTranslationKey(failureCode))}</p>
        {onRetry ? (
          <Button className="mt-3" onClick={onRetry} size="small">
            <RefreshCw aria-hidden="true" size={15} />
            {t('retry')}
          </Button>
        ) : null}
        {onOpenConnections ? (
          <Button className="mt-3" onClick={onOpenConnections} size="small" variant="secondary">
            <Link aria-hidden="true" size={15} />
            {t('goToConnections')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const failureKeys: Partial<Record<string, TranslationKey>> = {
  opencode_free_tier_restricted: 'openCodeFreeTierRestricted',
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
  attachment_unavailable: 'attachmentUnavailable',
  model_does_not_support_images: 'modelDoesNotSupportImages',
  provider_request_failed: 'providerRequestFailed',
  invalid_provider_response: 'providerRequestFailed',
  response_too_large: 'providerResponseTooLarge',
  message_save_failed: 'messageSaveFailed',
  attachment_save_failed: 'attachmentSaveFailed',
};

function failureTranslationKey(code: string | null): TranslationKey {
  return (code ? failureKeys[code] : undefined) ?? 'chatRequestFailed';
}

function needsConnectionRecovery(code: string | null): boolean {
  return code === 'authentication_required' || code === 'refresh_rejected' || code === 'invalid_api_key';
}

const providerKeys: Partial<Record<string, TranslationKey>> = {
  opencode: 'openCodeProvider',
  copilot: 'providerNameCopilot',
  chatgpt: 'chatGptProvider',
  chatgpt_api: 'chatGptProvider',
  gemini: 'geminiProvider',
  groq: 'groqProvider',
  cerebras: 'cerebrasProvider',
  openrouter: 'openRouterProvider',
  mistral: 'mistralProvider',
  deepseek: 'deepSeekProvider',
  xai: 'xAiProvider',
  perplexity: 'perplexityProvider',
  together: 'togetherProvider',
  sambanova: 'sambaNovaProvider',
  anthropic: 'anthropicProvider',
  fireworks: 'fireworksProvider',
  deepinfra: 'deepInfraProvider',
  dashscope: 'dashScopeProvider',
  cohere: 'cohereProvider',
  llama_cpp: 'localEngines',
};

function providerName(providerId: string, t: (key: TranslationKey) => string): string {
  const key = providerKeys[providerId];
  return key ? t(key) : providerId;
}

function formatTime(timestamp: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: '2-digit', hourCycle: 'h23', minute: '2-digit' }).format(timestamp);
}

function formatNumber(value: number, maximumFractionDigits: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits }).format(value);
}

function formatDuration(elapsedMicroseconds: number): string {
  const seconds = Math.max(0, Math.round(elapsedMicroseconds / 1_000_000));
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
