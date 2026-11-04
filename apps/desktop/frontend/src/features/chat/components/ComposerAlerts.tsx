import { useTranslation } from 'react-i18next';
import { Button } from '@shared/components';
import type { ChatAttachmentErrorCode } from '@features/chat/data/chatAttachments';
import type { ChatErrorCode } from '@features/chat/state';
import type { ChatCommandFeedbackKey } from '@entities/model/composerUtils';
import { formatServiceErrorMessage } from '../state/chatStoreErrorDetails';

export interface ComposerModelBannerProps {
  hasModels: boolean;
  hasVisibleModels: boolean;
  isNewChat: boolean;
  onOpenSettings(): void;
}

export function ComposerModelBanner({
  hasModels,
  hasVisibleModels,
  isNewChat,
  onOpenSettings,
}: ComposerModelBannerProps) {
  const { t } = useTranslation();

  if (hasModels && !hasVisibleModels && isNewChat) {
    return (
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-surface px-4 py-3 text-sm text-secondary-text">
        <span>{t('noVisibleModels')}</span>
        <Button onClick={onOpenSettings} size="small" variant="secondary">
          {t('settings')}
        </Button>
      </div>
    );
  }

  return null;
}

export interface ComposerFeedbackListProps {
  attachmentErrorKey: ChatAttachmentErrorCode | 'modelDoesNotSupportImages' | null;
  commandFeedbackKey: ChatCommandFeedbackKey | null;
  successFeedback: boolean;
  chatErrorCode: ChatErrorCode | null;
  chatErrorMessage: string | null;
  opencodeRetryAtUnixMs: number | null;
}

export function ComposerFeedbackList({
  attachmentErrorKey,
  commandFeedbackKey,
  successFeedback,
  chatErrorCode,
  chatErrorMessage,
  opencodeRetryAtUnixMs,
}: ComposerFeedbackListProps) {
  const { t, i18n } = useTranslation();

  return (
    <>
      {attachmentErrorKey ? (
        <p className="mt-2 text-sm text-destructive" role="alert">
          {t(attachmentErrorKey)}
        </p>
      ) : null}
      {commandFeedbackKey ? (
        <p
          className={`mt-2 text-sm ${successFeedback ? 'text-success' : 'text-destructive'}`}
          role={successFeedback ? 'status' : 'alert'}
        >
          {t(commandFeedbackKey)}
        </p>
      ) : null}
      {chatErrorCode && chatErrorCode !== 'searchMessageUnavailable' && chatErrorCode !== 'messageHistoryLoadFailed' ? (
        <div className="mt-2 text-sm text-destructive" role="alert">
          <p>{t(chatErrorCode)}</p>
          {chatErrorMessage ? (
            <p className="mt-1 break-words text-xs">{formatServiceErrorMessage(chatErrorMessage, t)}</p>
          ) : null}
          {chatErrorCode === 'chatRequestFailed' && opencodeRetryAtUnixMs !== null ? (
            <p className="mt-1 text-xs">
              {t('openCodeRetryAfterAt', {
                date: new Intl.DateTimeFormat(i18n.resolvedLanguage ?? i18n.language, {
                  dateStyle: 'medium',
                  timeStyle: 'short',
                }).format(new Date(opencodeRetryAtUnixMs)),
              })}
            </p>
          ) : null}
        </div>
      ) : null}
    </>
  );
}
