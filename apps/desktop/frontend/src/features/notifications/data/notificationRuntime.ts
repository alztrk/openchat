import { i18n } from '@shared/lib/i18n/i18n';
import { parseAgentQuestionGroup } from '@shared/lib/preferences/agentQuestions';
import type { ChatConversation, ChatMessage } from '@entities/conversation';
import { getConversation } from '@features/chat/data';
import { isAppWindowBackgrounded, resetAppWindowFocus, setAppWindowFocus } from '@shared/lib/platform/windowFocus';
import {
  readNotificationPreferences,
  readWindowsNotificationStatus,
  shouldSkipWindowsNotificationDelivery,
  sendAssistantResponseNotification,
  sendUserQuestionNotification,
  shouldSuppressNotification,
} from '@shared/lib/preferences/notificationPreferences';

export function setNotificationWindowFocus(focused: boolean): void {
  setAppWindowFocus(focused);
}

export function resetNotificationWindowFocus(): void {
  resetAppWindowFocus();
}

export async function notifyAssistantResponse(conversation: ChatConversation, message: ChatMessage): Promise<boolean> {
  if (!isAppWindowBackgrounded()) return false;
  const preferences = readNotificationPreferences();
  if (!preferences.responses) return false;
  if (shouldSuppressNotification(preferences, conversation.projectId)) return false;
  if (shouldSkipWindowsNotificationDelivery(await readWindowsNotificationStatus())) return false;
  const title = conversation.title.trim();
  const content = message.content.trim();
  if (!title || !content) return false;

  return sendAssistantResponseNotification({
    conversationId: conversation.id,
    assistantMessageId: message.id,
    title,
    content,
    genericBody: i18n.t('notificationResponseReady'),
    sendLabel: i18n.t('send'),
    replyLabel: i18n.t('messageHint'),
    showPreview: preferences.responsePreview,
    allowQuickReply: preferences.quickReply,
  });
}

export async function notifyPendingQuestion(rawGroup: unknown): Promise<boolean> {
  const preferences = readNotificationPreferences();
  if (!preferences.questions) return false;
  const group = parseAgentQuestionGroup(rawGroup);
  let projectId: string | null = null;
  if (preferences.mutedProjectIds.length > 0) {
    try {
      projectId = (await getConversation(group.conversationId))?.projectId ?? null;
    } catch (cause) {
      globalThis.reportError(new Error('The notification project could not be determined.', { cause }));
      return false;
    }
  }
  if (shouldSuppressNotification(preferences, projectId)) return false;
  if (shouldSkipWindowsNotificationDelivery(await readWindowsNotificationStatus())) return false;
  return sendUserQuestionNotification({
    groupId: group.id,
    conversationId: group.conversationId,
    title: i18n.t('userQuestionNotificationTitle'),
    body: i18n.t('userQuestionNotificationBody'),
  });
}
