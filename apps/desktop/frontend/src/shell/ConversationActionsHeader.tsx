import {
  Clock3,
  MessageSquare,
  MessageSquarePlus,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Search,
} from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import { ConversationTitleEditor } from '@/modules/conversations/components/ConversationTitleEditor';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';

export interface ConversationActionsHeaderProps {
  activeConversationId?: string | null;
  conversations?: ChatConversation[];
  isSidebarOpen?: boolean;
  onNewChat(): void;
  onOpenSearch(): void;
  onStartTemporaryChat(): void;
  onToggleSidebar?(): void;
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations: propConversations,
  isSidebarOpen = true,
  onNewChat,
  onOpenSearch,
  onStartTemporaryChat,
  onToggleSidebar,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();
  const storeConversations = useChatStore((state) => state.conversations);
  const renameConversation = useChatStore((state) => state.renameConversation);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const models = useChatStore((state) => state.models);
  const isSending = useChatStore((state) => state.isSending);

  const conversations = propConversations ?? storeConversations;
  const activeConversation = useMemo(
    () => (activeConversationId ? conversations.find((conversation) => conversation.id === activeConversationId) ?? null : null),
    [conversations, activeConversationId],
  );

  const activeModel = useMemo(
    () => models.find((model) => model.key === selectedModelKey) ?? null,
    [models, selectedModelKey],
  );

  const modelDisplayName =
    activeModel?.displayName ??
    activeConversation?.modelId ??
    temporaryChat?.route?.displayName ??
    null;

  return (
    <header className="app-content-divider flex h-12 shrink-0 items-center justify-between gap-3 border-b bg-navigation px-2 sm:px-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {onToggleSidebar ? (
          <Tooltip label={t(isSidebarOpen ? 'collapseSidebars' : 'showSidebars')}>
            <Button
              aria-label={t(isSidebarOpen ? 'collapseSidebars' : 'showSidebars')}
              className="size-9 shrink-0 text-secondary-icon hover:text-text"
              onClick={onToggleSidebar}
              size="icon"
              variant="ghost"
            >
              {isSidebarOpen ? (
                <PanelLeftClose aria-hidden="true" size={17} />
              ) : (
                <PanelLeftOpen aria-hidden="true" size={17} />
              )}
            </Button>
          </Tooltip>
        ) : null}

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
      </div>

      {modelDisplayName ? (
        <div className="hidden md:flex shrink-0 items-center gap-1.5 rounded-full border border-border bg-surface px-2.5 py-1 text-xs text-secondary-text shadow-xs">
          <span
            className={`size-2 shrink-0 rounded-full ${isSending ? 'animate-pulse bg-accent' : 'bg-success'}`}
            aria-hidden="true"
          />
          <span className="max-w-[160px] truncate font-medium text-text">{modelDisplayName}</span>
          {isSending ? (
            <span className="text-accent font-medium">{t('modelStatusGenerating')}</span>
          ) : null}
        </div>
      ) : null}

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
