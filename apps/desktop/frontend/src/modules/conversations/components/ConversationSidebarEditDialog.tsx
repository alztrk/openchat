import { type FormEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@/design-system';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';

type EditMode = 'rename';

interface ConversationSidebarEditDialogProps {
  conversation: ChatConversation | null;
  mode: EditMode | null;
  open: boolean;
  onOpenChange(open: boolean): void;
  onSaveTitle(conversationId: string, title: string): Promise<void>;
}

export function ConversationSidebarEditDialog({
  conversation,
  mode,
  open,
  onOpenChange,
  onSaveTitle,
}: ConversationSidebarEditDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    if (!open || !conversation || !mode) return;
    setValue(conversation.title);
    setError(null);
  }, [conversation, mode, open]);

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!conversation || !mode || isBusy) return;
    const normalized = value.trim();
    if (mode === 'rename' && !normalized) {
      setError(t('conversationTitleRequired'));
      return;
    }
    setIsBusy(true);
    setError(null);
    try {
      await onSaveTitle(conversation.id, normalized);
      onOpenChange(false);
    } catch {
      setError(t('conversationTitleSaveFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const title = t('renameConversation');

  return (
    <Dialog onOpenChange={onOpenChange} open={open} title={title}>
      <form className="grid gap-4" onSubmit={(event) => void save(event)}>
        <Input
          autoFocus
          disabled={isBusy}
          label={t('conversationTitle')}
          maxLength={500}
          onChange={(event) => {
            setValue(event.target.value);
            setError(null);
          }}
          value={value}
        />
        {error ? (
          <p className="text-sm text-destructive" role="alert">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button disabled={isBusy} onClick={() => onOpenChange(false)} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={isBusy} type="submit" variant="primary">
            {t('save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
