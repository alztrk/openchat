import { i18n } from '@/localization/i18n';
import { parseAgentQuestionGroup } from '@/modules/conversations/data/agentQuestions';
import type { ChatConversation, ChatMessage } from '@/modules/conversations/data/chatStorage';
import {
  readNotificationPreferences,
  readWindowsNotificationStatus,
  sendAssistantResponseNotification,
  sendUserQuestionNotification,
} from './notificationPreferences';

type WindowFocusState = 'unknown' | 'focused' | 'backgrounded';

let windowFocusState: WindowFocusState = 'unknown';

export function setNotificationWindowFocus(focused: boolean): void {
  windowFocusState = focused ? 'focused' : 'backgrounded';
}

export function resetNotificationWindowFocus(): void {
  windowFocusState = 'unknown';
}

export async function notifyAssistantResponse(conversation: ChatConversation, message: ChatMessage): Promise<boolean> {
  if (windowFocusState !== 'backgrounded') return false;
  const preferences = readNotificationPreferences();
  if (!preferences.responses) return false;
  if ((await readWindowsNotificationStatus()) !== 'enabled') return false;
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
  if ((await readWindowsNotificationStatus()) !== 'enabled') return false;
  const group = parseAgentQuestionGroup(rawGroup);
  return sendUserQuestionNotification({
    groupId: group.id,
    conversationId: group.conversationId,
    title: i18n.t('userQuestionNotificationTitle'),
    body: i18n.t('userQuestionNotificationBody'),
  });
}
