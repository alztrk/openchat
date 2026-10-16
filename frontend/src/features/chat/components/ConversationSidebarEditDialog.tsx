import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@/shared/ui';
import type { ChatConversation } from '@/features/chat/data/chatStorage';

type EditMode = 'rename' | 'tags';

interface ConversationSidebarEditDialogProps {
  conversation: ChatConversation | null;
  mode: EditMode | null;
  open: boolean;
  onOpenChange(open: boolean): void;
  onSaveTitle(conversationId: string, title: string): Promise<void>;
  onSaveTags(conversationId: string, tags: string[]): Promise<void>;
}

export function ConversationSidebarEditDialog({
  conversation,
  mode,
  open,
  onOpenChange,
  onSaveTitle,
  onSaveTags,
}: ConversationSidebarEditDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    if (!open || !conversation || !mode) return;
    setValue(mode === 'rename' ? conversation.title : conversation.tags.join(', '));
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
    const tags = mode === 'tags'
      ? normalized.split(',').map((tag) => tag.trim()).filter(Boolean)
      : [];
    const distinctTags = new Set(tags.map((tag) => tag.toLowerCase()));
    if (mode === 'tags' && (distinctTags.size > 12 || tags.some((tag) => [...tag].length > 32))) {
      setError(t('conversationTagsHelp'));
      return;
    }

    setIsBusy(true);
    setError(null);
    try {
      if (mode === 'rename') {
        await onSaveTitle(conversation.id, normalized);
      } else {
        await onSaveTags(conversation.id, tags);
      }
      onOpenChange(false);
    } catch {
      setError(t(mode === 'rename' ? 'conversationTitleSaveFailed' : 'conversationTagsSaveFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const title = mode === 'rename' ? t('renameConversation') : t('conversationTagsDialogTitle');

  return (
    <Dialog onOpenChange={onOpenChange} open={open} title={title}>
      <form className="grid gap-4" onSubmit={(event) => void save(event)}>
        <Input
          autoFocus
          disabled={isBusy}
          label={mode === 'rename' ? t('conversationTitle') : t('conversationTagsFieldLabel')}
          maxLength={500}
          onChange={(event) => { setValue(event.target.value); setError(null); }}
          placeholder={mode === 'tags' ? t('conversationTagsFieldHint') : undefined}
          value={value}
        />
        {mode === 'tags' ? <p className="text-sm text-secondary-text">{t('conversationTagsHelp')}</p> : null}
        {error ? <p className="text-sm text-destructive" role="alert">{error}</p> : null}
        <div className="flex justify-end gap-2">
          <Button disabled={isBusy} onClick={() => onOpenChange(false)} type="button">{t('cancel')}</Button>
          <Button disabled={isBusy} type="submit" variant="primary">{t('save')}</Button>
        </div>
      </form>
    </Dialog>
  );
}
