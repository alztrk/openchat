import { useEffect, useState } from 'react';
import { Brain } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Tooltip } from '@/shared/ui';
import { useChatStore } from '@/features/chat/stores/chatStore';
import {
  readPersonalMemoryExcludedConversationIds,
  setPersonalMemoryExcludedForConversation,
} from '@/features/settings/data/personalMemory';

export function PersonalMemoryConversationControl() {
  const { t } = useTranslation();
  const conversationId = useChatStore((state) => state.activeConversationId);
  const excludedForNewConversation = useChatStore((state) => state.personalMemoryExcludedForNewConversation);
  const setExcludedForNewConversation = useChatStore((state) => state.setPersonalMemoryExcludedForNewConversation);
  const isSending = useChatStore((state) => state.isSending);
  const [excluded, setExcluded] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [errorKey, setErrorKey] = useState<'personalMemoryLoadFailed' | 'personalMemorySaveFailed' | null>(null);

  useEffect(() => {
    if (conversationId === null) {
      setExcluded(excludedForNewConversation);
      setErrorKey(null);
      setIsLoading(false);
      return;
    }
    setIsLoading(true);
    try {
      setExcluded(readPersonalMemoryExcludedConversationIds().has(conversationId));
      setErrorKey(null);
    } catch {
      setExcluded(true);
      setErrorKey('personalMemoryLoadFailed');
    } finally {
      setIsLoading(false);
    }
  }, [conversationId, excludedForNewConversation]);

  const toggle = () => {
    if (isLoading || isSending) return;
    const next = !excluded;
    try {
      if (conversationId === null) setExcludedForNewConversation(next);
      else setPersonalMemoryExcludedForConversation(conversationId, next);
      setExcluded(next);
      setErrorKey(null);
    } catch {
      setErrorKey('personalMemorySaveFailed');
    }
  };

  const label = t(excluded ? 'personalMemoryUseHereTooltip' : 'personalMemoryExcludeHereTooltip');
  return (
    <div className="grid justify-items-end gap-1">
      <Tooltip label={label}>
        <Button
          aria-label={label}
          aria-pressed={excluded}
          className={excluded ? 'bg-selected text-accent' : 'text-secondary-icon'}
          disabled={isLoading || isSending}
          onClick={toggle}
          size="icon"
          variant="ghost"
        >
          <Brain aria-hidden="true" size={18} />
        </Button>
      </Tooltip>
      {errorKey ? <p className="text-xs text-destructive" role="alert">{t(errorKey)}</p> : null}
    </div>
  );
}
