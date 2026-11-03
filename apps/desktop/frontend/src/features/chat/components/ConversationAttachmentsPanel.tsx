import { TitleTooltip } from "@shared/components";
import { FileText, LoaderCircle, Paperclip } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { getMessages, type ChatMessage } from '@app/runtime/chatStorage';
import { useChatStore } from '@app/runtime/chatStore';
import { MessageAttachments } from './MessageAttachments';

export function ConversationAttachmentsPanel({ conversationId }: { conversationId: string | null }) {
  const { i18n, t } = useTranslation();
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const activeMessages = useChatStore((state) => state.messages);
  const activeAttachmentVersion = useMemo(
    () =>
      activeConversationId === conversationId
        ? activeMessages
        .map((message) => `${message.id}:${message.attachments.map((attachment) => attachment.id).join(',')}`)
        .join('|')
        : '',
    [activeConversationId, activeMessages, conversationId],
  );
  const [conversationMessages, setConversationMessages] = useState<
    | { conversationId: string; refreshKey: string; status: 'loading' | 'error' }
    | { conversationId: string; refreshKey: string; status: 'ready'; messages: ChatMessage[] }
    | null
  >(null);

  useEffect(() => {
    let active = true;
    if (!conversationId) {
      setConversationMessages(null);
      return;
    }

    setConversationMessages({ conversationId, refreshKey: activeAttachmentVersion, status: 'loading' });
    void getMessages(conversationId)
      .then((messages) => {
        if (active) setConversationMessages({ conversationId, refreshKey: activeAttachmentVersion, status: 'ready', messages });
      })
      .catch(() => {
        if (active) setConversationMessages({ conversationId, refreshKey: activeAttachmentVersion, status: 'error' });
      });

    return () => {
      active = false;
    };
  }, [activeAttachmentVersion, conversationId]);

  const hasConversationMessages =
    conversationMessages !== null &&
    conversationMessages.conversationId === conversationId &&
    conversationMessages.refreshKey === activeAttachmentVersion;
  const messages = hasConversationMessages && conversationMessages.status === 'ready'
    ? conversationMessages.messages
      : [];
  const attachments = useMemo(
    () =>
      messages.flatMap((message) =>
        message.attachments.map((attachment) => ({
          attachment,
          conversationId: message.conversationId,
          messageId: message.id,
        })),
      ),
    [messages],
  );
  const isLoading =
    !hasConversationMessages || conversationMessages.status === 'loading';
  const hasError = hasConversationMessages && conversationMessages.status === 'error';

  if (!conversationId) {
    return <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-sm text-secondary-text">{t('conversationUnavailable')}</p>;
  }

  return (
    <section aria-label={t('conversationAttachmentsTitle')} className="flex min-h-0 flex-1 flex-col">
      {isLoading && attachments.length === 0 ? (
        <div aria-label={t('conversationAttachmentsLoading')} className="flex min-h-0 flex-1 items-center justify-center" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin text-accent motion-reduce:animate-none" size={20} />
        </div>
      ) : hasError && attachments.length === 0 ? (
        <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-sm text-destructive" role="alert">{t('conversationAttachmentsLoadFailed')}</p>
      ) : attachments.length === 0 ? (
        <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-sm text-secondary-text">{t('conversationAttachmentsEmpty')}</p>
      ) : (
        <ul aria-label={t('conversationAttachmentsTitle')} className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
          {attachments.map(({ attachment, conversationId: itemConversationId, messageId }) => (
            <li className="min-w-0 rounded-control border border-border bg-surface p-3" key={`${messageId}:${attachment.id}`}>
              <div className="mb-2 flex min-w-0 items-center gap-2">
                {attachment.kind === 'image' ? <Paperclip aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} /> : <FileText aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />}
                <TitleTooltip title={attachment.name}><span className="truncate text-sm font-medium text-text" >{attachment.name}</span></TitleTooltip>
                <span className="ml-auto shrink-0 text-xs text-secondary-text">
                  {new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 }).format(attachment.sizeBytes / 1024)} KB
                </span>
              </div>
              <MessageAttachments attachments={[attachment]} conversationId={itemConversationId} messageId={messageId} />
            </li>
          ))}
        </ul>
      )}
      {hasError && attachments.length > 0 ? <p className="px-3 py-2 text-xs text-destructive" role="alert">{t('conversationAttachmentsLoadFailed')}</p> : null}
      {isLoading && attachments.length > 0 ? (
        <p aria-live="polite" className="shrink-0 px-3 py-2 text-xs text-secondary-text">{t('conversationAttachmentsLoading')}</p>
      ) : null}
    </section>
  );
}
