import { Clock3, MessageSquare, Plus, Search } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, TabsList, TabsTrigger } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';

const MAX_VISIBLE_TABS = 20;

interface ConversationActionsHeaderProps {
  activeConversationId?: string | null;
  conversations: ChatConversation[];
  onNewChat(): void;
  onOpenSearch(): void;
  onStartTemporaryChat(): void;
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations,
  onNewChat,
  onOpenSearch,
  onStartTemporaryChat,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();

  const visibleTabs = useMemo(() => {
    const unarchived = conversations.filter((conversation) => !conversation.isArchived);
    unarchived.sort((left, right) => right.updatedAt - left.updatedAt);

    if (
      activeConversationId &&
      !unarchived.slice(0, MAX_VISIBLE_TABS).some((conversation) => conversation.id === activeConversationId)
    ) {
      const active = unarchived.find((conversation) => conversation.id === activeConversationId);
      if (active) {
        return [active, ...unarchived.slice(0, MAX_VISIBLE_TABS - 1)];
      }
    }
    return unarchived.slice(0, MAX_VISIBLE_TABS);
  }, [conversations, activeConversationId]);

  return (
    <header className="app-content-divider flex h-12 shrink-0 items-center gap-2 border-b bg-navigation px-2 sm:px-3">
      <TabsList aria-label={t('chats')} className="conversation-tabs-list" variant="rounded">
        {visibleTabs.map((conversation) => (
          <TabsTrigger
            className="conversation-tab-trigger"
            key={conversation.id}
            title={conversation.title}
            value={conversation.id}
          >
            <MessageSquare aria-hidden="true" size={15} />
            <span className="truncate">{conversation.title}</span>
          </TabsTrigger>
        ))}
      </TabsList>
      <div className="flex shrink-0 items-center gap-0.5">
        <Button className="h-9 min-h-9 px-2.5" onClick={onNewChat} size="small" variant="ghost">
          <Plus aria-hidden="true" size={16} />
          <span className="hidden sm:inline">{t('newChat')}</span>
        </Button>
        <Tooltip label={t('temporaryChatStart')}>
          <Button
            aria-label={t('temporaryChatStart')}
            className="size-9"
            onClick={onStartTemporaryChat}
            size="icon"
            variant="ghost"
          >
            <Clock3 aria-hidden="true" size={17} />
          </Button>
        </Tooltip>
        <Tooltip label={t('searchChats')}>
          <Button aria-label={t('searchChats')} className="size-9" onClick={onOpenSearch} size="icon" variant="ghost">
            <Search aria-hidden="true" size={17} />
          </Button>
        </Tooltip>
      </div>
    </header>
  );
}
