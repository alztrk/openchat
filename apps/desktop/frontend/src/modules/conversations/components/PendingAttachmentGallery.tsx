import { Paperclip, X } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '@/design-system';
import type { PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';

interface PendingAttachmentGalleryProps {
  attachments: readonly PendingChatAttachment[];
  disabled?: boolean;
  onRemove(id: string): void;
}

export function PendingAttachmentGallery({ attachments, disabled = false, onRemove }: PendingAttachmentGalleryProps) {
  const { i18n, t } = useTranslation();
  const [previewAttachmentId, setPreviewAttachmentId] = useState<string | null>(null);
  const [previewZoom, setPreviewZoom] = useState(1);
  const zoomControlId = useId();
  const isScrollable = attachments.length > 4;
  const previewAttachment = attachments.find((attachment) => attachment.id === previewAttachmentId);
  const previewOpen = previewAttachment !== undefined && previewAttachment.previewUrl !== null;

  if (attachments.length === 0) return null;

  return (
    <>
      <div className={`mb-3 max-w-full ${isScrollable ? 'overflow-x-auto pb-2' : ''}`}>
        <ul aria-label={t('attachments')} className={isScrollable ? 'flex min-w-max gap-2' : 'flex flex-wrap gap-2'}>
          {attachments.map((attachment) => (
            <li
              className={
                isScrollable
                  ? 'flex w-56 shrink-0 items-center gap-2 rounded-control border border-border bg-raised p-2'
                  : 'flex max-w-full items-center gap-2 rounded-control border border-border bg-raised p-2'
              }
              key={attachment.id}
            >
              {attachment.kind === 'image' && attachment.previewUrl ? (
                <Button
                  aria-label={`${t('previewImage')}: ${attachment.name}`}
                  className="size-12 shrink-0 overflow-hidden rounded-control p-0"
                  onClick={() => {
                    setPreviewZoom(1);
                    setPreviewAttachmentId(attachment.id);
                  }}
                  title={t('previewImage')}
                  variant="ghost"
                >
                  <img alt="" className="size-full object-cover" src={attachment.previewUrl} />
                </Button>
              ) : (
                <Paperclip aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
              )}
              <span className="min-w-0 flex-1 truncate text-xs text-text">{attachment.name}</span>
              <Button
                aria-label={`${t('removeAttachment')}: ${attachment.name}`}
                className="size-8 shrink-0"
                disabled={disabled}
                onClick={() => onRemove(attachment.id)}
                size="icon"
                variant="ghost"
              >
                <X aria-hidden="true" size={15} />
              </Button>
            </li>
          ))}
        </ul>
      </div>
      <Dialog
        className="message-image-preview-dialog"
        onOpenChange={(open) => {
          if (!open) setPreviewAttachmentId(null);
        }}
        open={previewOpen}
        title={previewAttachment ? `${t('previewImage')}: ${previewAttachment.name}` : t('previewImage')}
      >
        {previewAttachment?.previewUrl ? (
          <div className="grid gap-3">
            <div className="message-image-preview-viewport">
              <img
                alt={previewAttachment.name}
                src={previewAttachment.previewUrl}
                style={{ width: `${previewZoom * 100}%` }}
              />
            </div>
            <div className="message-image-preview-zoom">
              <label className="sr-only" htmlFor={zoomControlId}>
                {t('previewImage')}
              </label>
              <input
                id={zoomControlId}
                max="6"
                min="0.75"
                onChange={(event) => setPreviewZoom(Number(event.currentTarget.value))}
                step="0.25"
                type="range"
                value={previewZoom}
              />
              <output className="min-w-12 text-right text-xs text-secondary-text" htmlFor={zoomControlId}>
                {new Intl.NumberFormat(i18n.resolvedLanguage ?? i18n.language, {
                  maximumFractionDigits: 0,
                  style: 'percent',
                }).format(previewZoom)}
              </output>
            </div>
            <div className="flex justify-end">
              <Button onClick={() => setPreviewAttachmentId(null)} variant="secondary">
                {t('close')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </>
  );
}
