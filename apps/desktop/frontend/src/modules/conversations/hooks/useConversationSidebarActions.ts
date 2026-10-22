import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  type ChatConversation,
  createProject as createProjectStorage,
  deleteConversation as deleteConversationStorage,
  duplicateConversation as duplicateConversationStorage,
  getConversation,
  moveConversations,
  renameConversation as renameConversationStorage,
  setConversationPinned,
  setConversationsArchived,
} from '@/modules/conversations/data/chatStorage';
import { type ExportFormat, exportConversationToFile } from '@/modules/conversations/data/conversationExport';
import { conversationMarkdownLabels } from '@/modules/conversations/data/conversationMarkdownLabels';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { localServiceClient } from '@/platform/local-service';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

export function useConversationSidebarActions() {
  const { t, i18n } = useTranslation();
  const [isBusy, setIsBusy] = useState(false);
  const [feedback, setFeedback] = useState<{ key: TranslationKey; success: boolean } | null>(null);
  const isBusyRef = useRef(false);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const isSending = useChatStore((state) => state.isSending);
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);
  const refreshConversations = useChatStore((state) => state.refreshConversations);
  const refreshProjects = useChatStore((state) => state.refreshProjects);

  const isConversationBusy = (conversationId: string) =>
    sendingConversationId === conversationId ||
    compactingConversationId === conversationId ||
    (isSending && activeConversationId === conversationId);

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
    if (willArchive && isConversationBusy(conversation.id)) {
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

  const exportConversation = async (conversation: ChatConversation, format: ExportFormat = 'md') => {
    if (!beginAction()) return;
    try {
      const labels = conversationMarkdownLabels(conversation, t, i18n.language);
      const saved = await exportConversationToFile(conversation, format, labels, t('exportConversation'));
      if (!saved) return;
      const feedbackKey: TranslationKey =
        format === 'md'
          ? 'conversationExportedMarkdown'
          : format === 'html'
            ? 'conversationExportedHtml'
            : 'conversationExportedJson';
      showFeedback(feedbackKey, true);
    } catch {
      showFeedback('conversationExportFailed');
    } finally {
      endAction();
    }
  };

  const deleteConversation = async (conversation: ChatConversation): Promise<boolean> => {
    if (!beginAction()) return false;
    if (isConversationBusy(conversation.id)) {
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

  const createProject = async (name: string, folderSelectionId: string) => {
    if (!beginAction()) throw new Error('A sidebar action is already in progress.');
    try {
      const projectId = crypto.randomUUID().replaceAll('-', '');
      await createProjectStorage(projectId, name, folderSelectionId);
      try {
        await refreshProjects();
      } catch {
        showFeedback('projectLoadFailed');
        return projectId;
      }
      if (useChatStore.getState().projectsStatus === 'error') {
        showFeedback('projectLoadFailed');
        return projectId;
      }
      showFeedback('projectCreated', true);
      return projectId;
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

  const togglePinned = (conversation: ChatConversation) =>
    runMutation(() => setConversationPinned(conversation.id, !conversation.isPinned), 'chatHistoryUnavailable');

  const duplicateConversation = async (conversation: ChatConversation): Promise<ChatConversation | null> => {
    if (!beginAction()) return null;
    try {
      const duplicateId = crypto.randomUUID().replaceAll('-', '');
      const duplicateTitle = `${conversation.title} (${t('conversationDuplicateSuffix')})`.slice(0, 500);
      const duplicated = await duplicateConversationStorage(conversation.id, duplicateId, duplicateTitle);
      try {
        await refreshConversations();
      } catch {
        showFeedback('chatHistoryUnavailable');
        return duplicated;
      }
      showFeedback('conversationDuplicated', true);
      return duplicated;
    } catch {
      showFeedback('conversationDuplicateFailed');
      return null;
    } finally {
      endAction();
    }
  };

  return {
    archiveConversation,
    createProject,
    deleteConversation,
    duplicateConversation,
    exportConversation,
    feedback,
    isBusy,
    isConversationBusy,
    moveConversation,
    saveTitle,
    showFeedback,
    togglePinned,
  };
}
