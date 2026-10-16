import { Clock3, MessageSquare, Plus, Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { ChatConversation } from '@/features/chat/data/chatStorage';
import { Button, TabsList, TabsTrigger } from '@/shared/ui';
import { Tooltip } from '@/shared/ui/Tooltip';

interface ConversationActionsHeaderProps {
  conversations: ChatConversation[];
  onNewChat(): void;
  onOpenSearch(): void;
  onStartTemporaryChat(): void;
}

export function ConversationActionsHeader({
  conversations,
  onNewChat,
  onOpenSearch,
  onStartTemporaryChat,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();

  return (
    <header className="app-content-divider flex h-12 shrink-0 items-center gap-2 border-b bg-navigation px-2 sm:px-3">
      <TabsList aria-label={t('chats')} className="conversation-tabs-list" variant="rounded">
        {[...conversations.filter((conversation) => !conversation.isArchived)]
          .sort((left, right) => right.updatedAt - left.updatedAt)
          .map((conversation) => (
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
          <Button
            aria-label={t('searchChats')}
            className="size-9"
            onClick={onOpenSearch}
            size="icon"
            variant="ghost"
          >
            <Search aria-hidden="true" size={17} />
          </Button>
        </Tooltip>
      </div>
    </header>
  );
}
