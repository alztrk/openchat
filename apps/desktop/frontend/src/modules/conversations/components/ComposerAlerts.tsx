import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import type { ChatAttachmentErrorCode } from '@/modules/conversations/data/chatAttachments';
import type { ChatErrorCode } from '@/modules/conversations/stores/chatStore';
import type { ChatCommandFeedbackKey } from './composerUtils';

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
  opencodeRetryAtUnixMs: number | null;
}

export function ComposerFeedbackList({
  attachmentErrorKey,
  commandFeedbackKey,
  successFeedback,
  chatErrorCode,
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
