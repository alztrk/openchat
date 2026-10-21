import {
  ArrowLeftRight,
  Clock3,
  Columns2,
  Maximize2,
  MessageSquare,
  MessageSquarePlus,
  PanelRight,
  X,
} from 'lucide-react';
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
  splitConversationId?: string | null;
  isRightPanelOpen?: boolean;
  onToggleSplitView?(): void;
  onToggleRightPanel?(): void;
  isSplitSecondary?: boolean;
  onCloseSplit?(): void;
  onSwapWithPrimary?(): void;
  onPromoteToPrimary?(): void;
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations: propConversations,
  splitConversationId = null,
  isRightPanelOpen = false,
  onToggleSplitView,
  onToggleRightPanel,
  isSplitSecondary = false,
  onCloseSplit,
  onSwapWithPrimary,
  onPromoteToPrimary,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();
  const storeConversations = useChatStore((state) => state.conversations);
  const renameConversation = useChatStore((state) => state.renameConversation);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
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

  const canToggleSplitView = Boolean(
    !temporaryChat && onToggleSplitView && (splitConversationId !== null || conversations.length >= 2),
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
            {isSplitSecondary ? (
              <>
                <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                <span className="truncate text-sm font-semibold text-text">{t('splitViewEmptyHint')}</span>
              </>
            ) : (
              <>
                <MessageSquarePlus aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                <h1 className="truncate text-sm font-semibold text-text">{t('newChat')}</h1>
              </>
            )}
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {isSplitSecondary ? (
          <>
            {onSwapWithPrimary ? (
              <Button
                aria-label={t('swapSplitPanes')}
                className="size-8 rounded-control text-secondary-icon hover:bg-hover hover:text-text"
                onClick={onSwapWithPrimary}
                size="icon"
                title={t('swapSplitPanes')}
                variant="ghost"
              >
                <ArrowLeftRight aria-hidden="true" size={15} />
              </Button>
            ) : null}
            {onPromoteToPrimary ? (
              <Button
                aria-label={t('focusSplitConversation')}
                className="size-8 rounded-control text-secondary-icon hover:bg-hover hover:text-text"
                onClick={onPromoteToPrimary}
                size="icon"
                title={t('focusSplitConversation')}
                variant="ghost"
              >
                <Maximize2 aria-hidden="true" size={15} />
              </Button>
            ) : null}
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
            {activeConversation ? (
              <ConversationExportMenu conversation={activeConversation} disabled={isSending} />
            ) : null}
            {onCloseSplit ? (
              <Button
                aria-label={t('closeSplitView')}
                className="size-8 rounded-control text-secondary-icon hover:bg-hover hover:text-text"
                onClick={onCloseSplit}
                size="icon"
                title={t('closeSplitView')}
                variant="ghost"
              >
                <X aria-hidden="true" size={15} />
              </Button>
            ) : null}
          </>
        ) : (
          <>
            {canToggleSplitView ? (
              <Button
                aria-label={splitConversationId ? t('closeSplitView') : t('splitViewToggle')}
                aria-pressed={splitConversationId !== null}
                className={`size-8 rounded-control ${
                  splitConversationId
                    ? 'bg-accent/15 text-accent hover:bg-accent/20'
                    : 'text-secondary-icon hover:bg-hover hover:text-text'
                }`}
                onClick={onToggleSplitView}
                size="icon"
                title={splitConversationId ? t('closeSplitView') : t('splitViewToggle')}
                variant="ghost"
              >
                <Columns2 aria-hidden="true" size={15} />
              </Button>
            ) : null}
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
              <ConversationExportMenu conversation={activeConversation} disabled={isSending} />
            ) : null}
          </>
        )}
      </div>
    </header>
  );
}
