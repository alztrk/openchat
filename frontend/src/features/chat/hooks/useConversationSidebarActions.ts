import { useRef, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';

import {
  createProject as createProjectStorage,
  deleteConversation as deleteConversationStorage,
  getConversation,
  getMessages,
  moveConversations,
  renameConversation as renameConversationStorage,
  saveConversationMarkdown,
  setConversationBookmarked,
  setConversationPinned,
  setConversationTags,
  setConversationsArchived,
  type ChatConversation,
} from '@/features/chat/data/chatStorage';
import {
  buildConversationMarkdown,
  conversationMarkdownFileName,
} from '@/features/chat/data/conversationMarkdownExport';
import { conversationMarkdownLabels } from '@/features/chat/data/conversationMarkdownLabels';
import { localServiceClient, useChatStore } from '@/features/chat/stores/chatStore';

type TranslationKey = keyof typeof import('@/i18n/locales/en.json');

interface UseConversationSidebarActionsProps {
  selectedIds: ReadonlySet<string>;
  closeSelection(): void;
}

export function useConversationSidebarActions({ selectedIds, closeSelection }: UseConversationSidebarActionsProps) {
  const { t } = useTranslation();
  const [isBusy, setIsBusy] = useState(false);
  const [feedback, setFeedback] = useState<{ key: TranslationKey; success: boolean } | null>(null);
  const isBusyRef = useRef(false);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const isSending = useChatStore((state) => state.isSending);
  const conversations = useChatStore((state) => state.conversations);
  const activeConversations = conversations.filter((item) => !item.isArchived);
  const refreshConversations = useChatStore((state) => state.refreshConversations);
  const refreshProjects = useChatStore((state) => state.refreshProjects);

  const showFeedback = (key: TranslationKey, success = false) => setFeedback({ key, success });

  const beginAction = () => {
    if (isBusyRef.current) return false;
    isBusyRef.current = true;
    setIsBusy(true);
    setFeedback(null);
    return true;
  };

  const endAction = () => {
    isBusyRef.current = false;
    setIsBusy(false);
  };

  const runMutation = async (
    operation: () => Promise<void>,
    failureKey: TranslationKey,
    successKey?: TranslationKey,
  ): Promise<boolean> => {
    if (!beginAction()) return false;
    try {
      await operation();
      try {
        await refreshConversations();
      } catch {
        showFeedback('chatHistoryUnavailable');
        return true;
      }
      if (successKey) showFeedback(successKey, true);
      return true;
    } catch {
      showFeedback(failureKey);
      return false;
    } finally {
      endAction();
    }
  };

  const archiveConversation = async (conversation: ChatConversation) => {
    const willArchive = !conversation.isArchived;
    if (willArchive && isSending && activeConversationId === conversation.id) {
      showFeedback('stopResponseBeforeArchive');
      return;
    }
    await runMutation(
      () => setConversationsArchived([conversation.id], willArchive),
      willArchive ? 'conversationArchiveFailed' : 'conversationRestoreFailed',
      willArchive ? 'conversationArchived' : 'conversationRestored',
    );
  };

  const moveConversation = async (conversation: ChatConversation, projectId: string | null) => {
    if (conversation.projectId === projectId && !conversation.isPinned) return;
    await runMutation(() => moveConversations([conversation.id], projectId), 'projectMoveFailed');
  };

  const exportConversation = async (conversation: ChatConversation) => {
    if (!beginAction()) return;
    try {
      const messages = await getMessages(conversation.id);
      const markdown = buildConversationMarkdown(
        conversation,
        messages,
        conversationMarkdownLabels(conversation, t),
      );
      const path = await save({
        defaultPath: conversationMarkdownFileName(conversation.title),
        filters: [{ name: 'Markdown', extensions: ['md'] }],
        title: t('exportConversation'),
      });
      if (path === null) return;
      await saveConversationMarkdown(path, markdown);
      showFeedback('conversationExported', true);
    } catch {
      showFeedback('conversationExportFailed');
    } finally {
      endAction();
    }
  };

  const deleteConversation = async (conversation: ChatConversation): Promise<boolean> => {
    if (!beginAction()) return false;
    if (isSending && activeConversationId === conversation.id) {
      showFeedback('stopResponseBeforeDelete');
      endAction();
      return false;
    }
    try {
      let deleted = false;
      let cleanupFailed = false;
      try {
        await deleteConversationStorage(conversation.id);
        deleted = true;
      } catch {
        try {
          deleted = (await getConversation(conversation.id)) === null;
        } catch {
          deleted = false;
        }
        cleanupFailed = deleted;
      }
      if (!deleted) {
        let historyUnavailable = false;
        try {
          await refreshConversations();
        } catch {
          historyUnavailable = true;
        }
        showFeedback(historyUnavailable ? 'chatHistoryUnavailable' : 'conversationDeleteFailed');
        return false;
      }
      try {
        await localServiceClient.call('chat.file_changes.delete', { conversationId: conversation.id });
      } catch {
        cleanupFailed = true;
      }
      let historyUnavailable = false;
      try {
        await refreshConversations();
      } catch {
        historyUnavailable = true;
      }
      showFeedback(
        historyUnavailable
          ? 'chatHistoryUnavailable'
          : cleanupFailed
            ? 'conversationDeletedFileChangesCleanupFailed'
            : 'conversationDeleted',
        !historyUnavailable && !cleanupFailed,
      );
      return true;
    } finally {
      endAction();
    }
  };

  const createProject = async (name: string, folderPath: string) => {
    if (!beginAction()) throw new Error('A sidebar action is already in progress.');
    try {
      await createProjectStorage(crypto.randomUUID().replaceAll('-', ''), name, folderPath);
      try {
        await refreshProjects();
      } catch {
        showFeedback('projectLoadFailed');
        return;
      }
      if (useChatStore.getState().projectsStatus === 'error') {
        showFeedback('projectLoadFailed');
        return;
      }
      showFeedback('projectCreated', true);
    } catch {
      showFeedback('projectCreateFailed');
      throw new Error('The project could not be created.');
    } finally {
      endAction();
    }
  };

  const saveTitle = async (conversationId: string, title: string) => {
    const saved = await runMutation(
      () => renameConversationStorage(conversationId, title),
      'conversationTitleSaveFailed',
    );
    if (!saved) throw new Error('The conversation title could not be saved.');
  };

  const saveTags = async (conversationId: string, tags: string[]) => {
    const saved = await runMutation(
      () => setConversationTags(conversationId, tags),
      'conversationTagsSaveFailed',
    );
    if (!saved) throw new Error('Conversation tags could not be saved.');
  };

  const toggleBookmark = (conversation: ChatConversation) => runMutation(
    () => setConversationBookmarked(conversation.id, !conversation.isBookmarked),
    'conversationBookmarkFailed',
  );

  const togglePinned = (conversation: ChatConversation) => runMutation(
    () => setConversationPinned(conversation.id, !conversation.isPinned),
    'chatHistoryUnavailable',
  );

  const archiveSelected = async () => {
    const ids = [...selectedIds].filter((id) => activeConversations.some((conversation) => conversation.id === id));
    if (ids.length === 0 || isBusy) return;
    if (isSending && activeConversationId !== null && ids.includes(activeConversationId)) {
      showFeedback('stopResponseBeforeArchive');
      return;
    }
    const archived = await runMutation(
      () => setConversationsArchived(ids, true),
      'conversationArchiveFailed',
      'conversationArchived',
    );
    if (archived) closeSelection();
  };

  const moveSelected = async (moveTarget: string) => {
    const ids = [...selectedIds].filter((id) => activeConversations.some((conversation) => conversation.id === id));
    if (ids.length === 0 || isBusy) return;
    const projectId = moveTarget === '__chats__' ? null : moveTarget;
    const moved = await runMutation(() => moveConversations(ids, projectId), 'projectMoveFailed');
    if (moved) closeSelection();
  };

  return {
    archiveConversation,
    archiveSelected,
    createProject,
    deleteConversation,
    exportConversation,
    feedback,
    isBusy,
    moveConversation,
    moveSelected,
    saveTags,
    saveTitle,
    showFeedback,
    toggleBookmark,
    togglePinned,
  };
}
