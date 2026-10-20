import {
  Clock3,
  MessageSquare,
  MessageSquarePlus,
} from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ConversationTitleEditor } from '@/modules/conversations/components/ConversationTitleEditor';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ConversationExportMenu } from './ConversationExportMenu';

export interface ConversationActionsHeaderProps {
  activeConversationId?: string | null;
  conversations?: ChatConversation[];
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations: propConversations,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();
  const storeConversations = useChatStore((state) => state.conversations);
  const renameConversation = useChatStore((state) => state.renameConversation);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const isSending = useChatStore((state) => state.isSending);

  const conversations = propConversations ?? storeConversations;
  const activeConversation = useMemo(
    () => (activeConversationId ? conversations.find((conversation) => conversation.id === activeConversationId) ?? null : null),
    [conversations, activeConversationId],
  );

  return (
    <header className="app-content-divider flex h-12 shrink-0 items-center justify-between gap-3 border-b bg-navigation px-2 sm:px-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {temporaryChat ? (
          <div className="flex min-w-0 items-center gap-2">
            <Clock3 aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1 className="truncate text-sm font-semibold text-text">{t('temporaryChatTitle')}</h1>
          </div>
        ) : activeConversation ? (
          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <div className="min-w-0 flex-1 [&_button]:min-h-8 [&_button]:px-1 [&_input]:!text-sm [&_input]:!font-medium [&_span]:!text-sm [&_span]:!font-medium">
              <ConversationTitleEditor
                key={activeConversation.id}
                onSave={(title) => renameConversation(activeConversation.id, title)}
                title={activeConversation.title}
              />
            </div>
          </div>
        ) : (
          <div className="flex min-w-0 items-center gap-2 select-none">
            <MessageSquarePlus aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1 className="truncate text-sm font-semibold text-text">{t('newChat')}</h1>
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        {activeConversation && !temporaryChat ? (
          <ConversationExportMenu
            conversation={activeConversation}
            disabled={isSending}
          />
        ) : null}
      </div>
    </header>
  );
}
