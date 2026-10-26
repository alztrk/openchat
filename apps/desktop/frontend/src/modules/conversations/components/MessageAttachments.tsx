import { Download, FileText, LoaderCircle } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog, Slider } from '@/design-system';
import type { ChatAttachmentMetadata } from '@/modules/conversations/data/chatAttachments';
import { readMessageAttachment } from '@/modules/conversations/data/chatStorage';

interface MessageAttachmentsProps {
  conversationId: string;
  messageId: string;
  attachments: readonly ChatAttachmentMetadata[];
}

export function MessageAttachments({ conversationId, messageId, attachments }: MessageAttachmentsProps) {
  const { t } = useTranslation();
  if (attachments.length === 0) return null;
  const isScrollable = attachments.length > 4;
  const galleryClassName = isScrollable
    ? 'flex w-full max-w-full gap-2 overflow-x-auto pb-2'
    : attachments.length === 1
      ? 'grid w-full max-w-attachment-card grid-cols-1 gap-2'
      : 'grid w-full max-w-attachment-gallery grid-cols-2 gap-2';
  return (
    <ul aria-label={t('attachments')} className={`mt-3 ${galleryClassName}`}>
      {attachments.map((attachment) => (
        <li
          className={isScrollable ? 'min-w-16 max-w-attachment-card shrink-0' : 'min-w-0'}
          key={attachment.id}
          style={isScrollable ? { width: 'clamp(64px, calc((100% - 1.5rem) / 4), 280px)' } : undefined}
        >
          <MessageAttachment
            attachment={attachment}
            conversationId={conversationId}
            messageId={messageId}
            isScrollable={isScrollable}
          />
        </li>
      ))}
    </ul>
  );
}

interface MessageAttachmentProps {
  attachment: ChatAttachmentMetadata;
  conversationId: string;
  messageId: string;
  isScrollable: boolean;
}

function MessageAttachment({ attachment, conversationId, isScrollable, messageId }: MessageAttachmentProps) {
  const { i18n, t } = useTranslation();
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<'idle' | 'loading' | 'error'>('idle');
  const [isImagePreviewOpen, setIsImagePreviewOpen] = useState(false);
  const [imagePreviewZoom, setImagePreviewZoom] = useState(1);
  const downloadUrl = useRef<string | null>(null);
  const downloadTimer = useRef<number | undefined>(undefined);
  const isMountedRef = useRef(false);
  const zoomControlId = useId();

  useEffect(() => {
    if (attachment.kind !== 'image') return;
    let active = true;
    let createdUrl: string | null = null;
    setStatus('loading');
    void readMessageAttachment(conversationId, messageId, attachment.id)
      .then((bytes) => {
        const blob = new Blob([bytes.slice().buffer], { type: attachment.mimeType });
        createdUrl = URL.createObjectURL(blob);
        if (!active) {
          URL.revokeObjectURL(createdUrl);
          return;
        }
        setImageUrl(createdUrl);
        setStatus('idle');
      })
      .catch(() => {
        if (active) setStatus('error');
      });
    return () => {
      active = false;
      if (createdUrl) URL.revokeObjectURL(createdUrl);
    };
  }, [attachment.id, attachment.kind, attachment.mimeType, conversationId, messageId]);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      if (downloadTimer.current !== undefined) window.clearTimeout(downloadTimer.current);
      if (downloadUrl.current) URL.revokeObjectURL(downloadUrl.current);
    };
  }, []);

  const download = async () => {
    if (downloadTimer.current !== undefined) {
      window.clearTimeout(downloadTimer.current);
      downloadTimer.current = undefined;
    }
    setStatus('loading');
    try {
      const bytes = await readMessageAttachment(conversationId, messageId, attachment.id);
      if (!isMountedRef.current) return;
      const url = URL.createObjectURL(new Blob([bytes.slice().buffer], { type: attachment.mimeType }));
      if (downloadUrl.current) URL.revokeObjectURL(downloadUrl.current);
      downloadUrl.current = url;
      const link = document.createElement('a');
      link.href = url;
      link.download = attachment.name;
      link.click();
      setStatus('idle');
      downloadTimer.current = window.setTimeout(() => {
        URL.revokeObjectURL(url);
        if (downloadUrl.current === url) downloadUrl.current = null;
        downloadTimer.current = undefined;
      }, 1000);
    } catch {
      if (isMountedRef.current) setStatus('error');
    }
  };

  if (attachment.kind === 'image') {
    return (
      <>
        <figure className="grid min-w-0 w-full gap-1">
          {imageUrl ? (
            <Button
              aria-label={`${t('previewImage')}: ${attachment.name}`}
              className="h-attachment-preview w-full overflow-hidden rounded-attachment-preview p-0"
              onClick={() => {
                setImagePreviewZoom(1);
                setIsImagePreviewOpen(true);
              }}
              title={t('previewImage')}
              variant="ghost"
            >
              <img
                alt={attachment.name}
                className="size-full object-cover"
                onError={() => {
                  setImageUrl(null);
                  setStatus('error');
                }}
                src={imageUrl}
              />
            </Button>
          ) : status === 'error' ? (
            <span
              className="flex h-attachment-preview w-full items-center justify-center rounded-attachment-preview bg-surface px-2 text-center text-xs text-accent-icon"
              role="status"
            >
              {t('attachmentUnavailable')}
            </span>
          ) : (
            <span
              aria-label={t('attachmentLibraryLoading')}
              className="flex h-attachment-preview w-full items-center justify-center rounded-attachment-preview bg-surface text-secondary-icon"
              role="status"
            >
              <LoaderCircle aria-hidden="true" className="animate-spin" size={18} />
            </span>
          )}
        </figure>
        <Dialog
          className="message-image-preview-dialog"
          onOpenChange={setIsImagePreviewOpen}
          open={isImagePreviewOpen}
          title={`${t('previewImage')}: ${attachment.name}`}
        >
          {imageUrl ? (
            <div className="grid gap-3">
              <div className="message-image-preview-viewport">
                <img alt={attachment.name} src={imageUrl} style={{ width: `${imagePreviewZoom * 100}%` }} />
              </div>
              <div className="message-image-preview-zoom">
                <Slider
                  id={zoomControlId}
                  label={t('previewImage')}
                  max="6"
                  min="0.75"
                  onChange={(event) => setImagePreviewZoom(Number(event.currentTarget.value))}
                  step="0.25"
                  value={imagePreviewZoom}
                  fieldClassName="min-w-0 flex-1"
                  fieldLayout="inline"
                />
                <output className="min-w-12 text-right text-xs text-secondary-text" htmlFor={zoomControlId}>
                  {new Intl.NumberFormat(i18n.language, { style: 'percent', maximumFractionDigits: 0 }).format(
                    imagePreviewZoom,
                  )}
                </output>
              </div>
              <div className="flex justify-end">
                <Button onClick={() => setIsImagePreviewOpen(false)} variant="secondary">
                  {t('close')}
                </Button>
              </div>
            </div>
          ) : null}
        </Dialog>
      </>
    );
  }

  const sizeLabel =
    attachment.sizeBytes < 1024 * 1024
      ? `${Math.ceil(attachment.sizeBytes / 1024)} KB`
      : `${new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 }).format(attachment.sizeBytes / (1024 * 1024))} MB`;

  return (
    <div className={`grid w-full gap-1 ${isScrollable ? 'min-h-attachment-card content-start' : ''}`}>
      <Button
        aria-label={`${t('attachmentDownload')}: ${attachment.name}`}
        className="w-full justify-start rounded-control border border-border bg-surface px-2 py-2 text-left"
        onClick={() => void download()}
        variant="secondary"
      >
        {status === 'loading' ? (
          <LoaderCircle aria-hidden="true" className="shrink-0 animate-spin" size={19} />
        ) : (
          <FileText aria-hidden="true" className="shrink-0 text-secondary-icon" size={19} />
        )}
        <span className="grid min-w-0 flex-1 text-left">
          <span className="truncate text-xs text-text">{attachment.name}</span>
          <span className="truncate text-xs text-secondary-text">{sizeLabel}</span>
        </span>
        <Download aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
      </Button>
      {status === 'error' ? (
        <span className="text-xs text-destructive" role="alert">
          {t('attachmentReadFailed')}
        </span>
      ) : null}
    </div>
  );
}
