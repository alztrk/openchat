import { Clock3, Files, MessageSquare, MessageSquarePlus, PanelRight } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { ConversationTitleEditor } from '@/modules/conversations/components/ConversationTitleEditor';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ConversationExportMenu } from './ConversationExportMenu';

export interface ConversationActionsHeaderProps {
  activeConversationId?: string | null;
  conversations?: ChatConversation[];
  isRightPanelOpen?: boolean;
  onToggleRightPanel?(): void;
  isPanelConversation?: boolean;
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations: propConversations,
  isRightPanelOpen = false,
  onToggleRightPanel,
  isPanelConversation = false,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();
  const storeConversations = useChatStore((state) => state.conversations);
  const renameConversation = useChatStore((state) => state.renameConversation);
  const temporaryChat = useChatStore((state) => (isPanelConversation ? null : state.temporaryChat));
  const isSending = useChatStore((state) => state.isSending);
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);

  const conversations = propConversations ?? storeConversations;
  const activeConversation = useMemo(
    () =>
      activeConversationId && activeConversationId !== '__empty__'
        ? (conversations.find((conversation) => conversation.id === activeConversationId) ?? null)
        : null,
    [conversations, activeConversationId],
  );

  const isActiveConversationAiWorking = Boolean(
    activeConversation &&
      (sendingConversationId === activeConversation.id ||
        compactingConversationId === activeConversation.id ||
        (isSending && activeConversationId === activeConversation.id)),
  );

  return (
    <header className="app-content-divider flex h-12 shrink-0 items-center justify-between gap-3 border-b bg-navigation/80 px-2 sm:px-3 backdrop-blur-md">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {temporaryChat ? (
          <div className="flex min-w-0 items-center gap-2">
            <Clock3 aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1 className={`truncate text-sm font-semibold ${isSending ? 'openchat-title-shimmer' : 'text-text'}`}>
              {t('temporaryChatTitle')}
            </h1>
          </div>
        ) : activeConversation ? (
          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <div className="min-w-0 flex-1 [&_button]:min-h-8 [&_button]:px-1 [&_input]:!text-sm [&_input]:!font-medium [&_span]:!text-sm [&_span]:!font-medium">
              <ConversationTitleEditor
                isAiWorking={isActiveConversationAiWorking}
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

      <div className="flex shrink-0 items-center gap-1">
        <Button
          aria-label={t('attachmentLibraryOpenPicker')}
          className="size-8 rounded-control text-secondary-icon"
          disabled
          size="icon"
          title={t('attachmentLibraryOpenPicker')}
          type="button"
          variant="ghost"
        >
          <Files aria-hidden="true" size={15} />
        </Button>
        {onToggleRightPanel ? (
          <Button
            aria-label={isRightPanelOpen ? t('closeRightPanel') : t('openRightPanel')}
            aria-pressed={isRightPanelOpen}
            className={`size-8 rounded-control ${
              isRightPanelOpen
                ? 'bg-accent/15 text-accent hover:bg-accent/20'
                : 'text-secondary-icon hover:bg-hover hover:text-text'
            }`}
            onClick={onToggleRightPanel}
            size="icon"
            title={isRightPanelOpen ? t('closeRightPanel') : t('openRightPanel')}
            variant="ghost"
          >
            <PanelRight aria-hidden="true" size={15} />
          </Button>
        ) : null}
        {activeConversation && !temporaryChat ? (
          <ConversationExportMenu conversation={activeConversation} disabled={isActiveConversationAiWorking} />
        ) : null}
      </div>
    </header>
  );
}
