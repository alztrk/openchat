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
  modelError: boolean;
}

export function ComposerFeedbackList({
  attachmentErrorKey,
  commandFeedbackKey,
  successFeedback,
  chatErrorCode,
  modelError,
}: ComposerFeedbackListProps) {
  const { t } = useTranslation();

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
        <p className="mt-2 text-sm text-destructive" role="alert">
          {t(chatErrorCode)}
        </p>
      ) : null}
      {modelError ? (
        <p className="mt-2 text-sm text-warning" role="status">
          {t('modelCatalogSourcesUnavailable')}
        </p>
      ) : null}
    </>
  );
}
