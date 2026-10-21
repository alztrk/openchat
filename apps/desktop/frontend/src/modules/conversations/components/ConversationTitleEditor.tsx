import { Check, Pencil, X } from 'lucide-react';
import { type FormEvent, type KeyboardEvent, useCallback, useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Input } from '@/design-system';

interface ConversationTitleEditorProps {
  title: string;
  isAiWorking?: boolean;
  onSave(title: string): Promise<void>;
}

export function ConversationTitleEditor({ isAiWorking = false, onSave, title }: ConversationTitleEditorProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(title);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const restoreTitleFocus = useRef(false);
  const inputId = useId();
  const titleButtonId = useId();

  const focusInput = useCallback(() => {
    const input = document.getElementById(inputId);
    if (input instanceof HTMLInputElement) {
      input.focus();
      input.select();
    }
  }, [inputId]);

  const focusTitleButton = useCallback(() => {
    const button = document.getElementById(titleButtonId);
    if (button instanceof HTMLButtonElement) button.focus();
  }, [titleButtonId]);

  useEffect(() => {
    setValue(title);
    setIsEditing(false);
    setIsSaving(false);
    setError(null);
  }, [title]);

  useEffect(() => {
    if (!isEditing) return;
    focusInput();
  }, [focusInput, isEditing]);

  useEffect(() => {
    if (isEditing || !restoreTitleFocus.current) return;
    restoreTitleFocus.current = false;
    focusTitleButton();
  }, [focusTitleButton, isEditing]);

  const cancel = () => {
    if (isSaving) return;
    setValue(title);
    setError(null);
    restoreTitleFocus.current = true;
    setIsEditing(false);
  };

  const save = async (event?: FormEvent<HTMLFormElement>) => {
    event?.preventDefault();
    if (isSaving) return;
    const normalized = value.trim();
    if (normalized.length === 0) {
      setError(t('conversationTitleRequired'));
      focusInput();
      return;
    }
    setError(null);
    setIsSaving(true);
    try {
      await onSave(normalized);
      restoreTitleFocus.current = true;
      setIsEditing(false);
    } catch {
      setError(t('conversationTitleSaveFailed'));
      focusInput();
    } finally {
      setIsSaving(false);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      cancel();
    }
  };

  if (!isEditing) {
    return (
      <Button
        aria-label={t('renameConversation')}
        className="min-w-0 flex-1 justify-start gap-2 px-2 text-left"
        id={titleButtonId}
        onClick={() => {
          setValue(title);
          setError(null);
          setIsEditing(true);
        }}
        title={t('renameConversation')}
        variant="ghost"
      >
        <span
          className={`min-w-0 flex-1 truncate text-base font-semibold ${isAiWorking ? 'openchat-title-shimmer' : 'text-text'}`}
        >
          {title}
        </span>
        <Pencil aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
      </Button>
    );
  }

  return (
    <form className="flex min-w-0 flex-1 items-start gap-2" onSubmit={(event) => void save(event)}>
      <Input
        aria-label={t('conversationTitle')}
        className="min-h-9 py-1.5 text-base font-semibold"
        disabled={isSaving}
        error={error ?? undefined}
        id={inputId}
        maxLength={500}
        onChange={(event) => {
          setValue(event.target.value);
          if (error) setError(null);
        }}
        onKeyDown={handleKeyDown}
        value={value}
      />
      <div className="flex shrink-0 gap-1 pt-0.5">
        <Button
          aria-label={t('save')}
          className="size-9"
          disabled={isSaving}
          size="icon"
          type="submit"
          variant="primary"
        >
          <Check aria-hidden="true" size={16} />
        </Button>
        <Button
          aria-label={t('cancel')}
          className="size-9"
          disabled={isSaving}
          onClick={cancel}
          size="icon"
          type="button"
          variant="ghost"
        >
          <X aria-hidden="true" size={16} />
        </Button>
      </div>
    </form>
  );
}
