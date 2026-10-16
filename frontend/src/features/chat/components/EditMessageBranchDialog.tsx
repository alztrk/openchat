import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ChatMessage } from '@/features/chat/data/chatStorage';
import { Button, Dialog } from '@/shared/ui';

interface EditMessageBranchDialogProps {
  message: ChatMessage | null;
  open: boolean;
  busy: boolean;
  onOpenChange(open: boolean): void;
  onSubmit(content: string): Promise<boolean>;
}

export function EditMessageBranchDialog({
  busy,
  message,
  onOpenChange,
  onSubmit,
  open,
}: EditMessageBranchDialogProps) {
  const { t } = useTranslation();
  const [content, setContent] = useState('');

  useEffect(() => {
    if (open) setContent(message?.content ?? '');
  }, [message, open]);

  const canSubmit = message !== null && (content.trim().length > 0 || message.attachments.length > 0);
  const submit = () => {
    if (!canSubmit || busy) return;
    void onSubmit(content).then((created) => {
      if (created) onOpenChange(false);
    });
  };

  return (
    <Dialog
      onOpenChange={onOpenChange}
      open={open}
      title={t('conversationBranchEditTitle')}
    >
      <div className="grid gap-4">
        <label className="grid gap-2 text-sm font-medium text-text" htmlFor="conversation-branch-message">
          {t('conversationBranchEditLabel')}
          <textarea
            autoFocus
            className="min-h-32 w-full resize-y rounded-control border border-border bg-background px-3 py-2 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
            disabled={busy}
            id="conversation-branch-message"
            onChange={(event) => setContent(event.target.value)}
            value={content}
          />
        </label>
        <div className="flex justify-end gap-2">
          <Button disabled={busy} onClick={() => onOpenChange(false)}>{t('cancel')}</Button>
          <Button disabled={!canSubmit || busy} onClick={submit} variant="primary">
            {t('conversationBranchStart')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
