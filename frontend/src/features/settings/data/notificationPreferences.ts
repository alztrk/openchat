import { invoke } from '@tauri-apps/api/core';

export interface NotificationPreferences {
  responses: boolean;
  questions: boolean;
  responsePreview: boolean;
  quickReply: boolean;
}

export type WindowsNotificationStatus =
  | 'enabled'
  | 'disabledForApplication'
  | 'disabledForUser'
  | 'disabledByGroupPolicy'
  | 'disabledByManifest'
  | 'unavailable'
  | 'notSupported';

export interface AssistantResponseNotificationInput {
  conversationId: string;
  assistantMessageId: string;
  title: string;
  content: string;
  genericBody: string;
  sendLabel: string;
  replyLabel: string;
  showPreview: boolean;
  allowQuickReply: boolean;
}

export interface UserQuestionNotificationInput {
  groupId: string;
  conversationId: string;
  title: string;
  body: string;
}

const preferenceKeys: Record<keyof NotificationPreferences, string> = {
  responses: 'notifications.responses',
  questions: 'notifications.questions',
  responsePreview: 'notifications.response_preview',
  quickReply: 'notifications.quick_reply',
};

const defaultPreferences: NotificationPreferences = {
  responses: true,
  questions: true,
  responsePreview: true,
  quickReply: true,
};

export function readNotificationPreferences(): NotificationPreferences {
  return {
    responses: readPreference('responses'),
    questions: readPreference('questions'),
    responsePreview: readPreference('responsePreview'),
    quickReply: readPreference('quickReply'),
  };
}

export function writeNotificationPreference(
  key: keyof NotificationPreferences,
  value: boolean,
): void {
  try {
    window.localStorage.setItem(preferenceKeys[key], String(value));
  } catch {
    throw new Error('Notification preferences could not be saved.');
  }
}

export function readWindowsNotificationStatus(): Promise<WindowsNotificationStatus> {
  return invoke<unknown>('read_windows_notification_status').then((value) => {
    if (!isWindowsNotificationStatus(value)) {
      throw new Error('Windows notification status is invalid.');
    }
    return value;
  });
}

export function openWindowsNotificationSettings(): Promise<void> {
  return invoke<void>('open_windows_notification_settings');
}

export function sendTestNotification(title: string, body: string): Promise<boolean> {
  return invoke<boolean>('send_test_notification', { title, body });
}

export function sendAssistantResponseNotification(
  request: AssistantResponseNotificationInput,
): Promise<boolean> {
  return invoke<boolean>('send_assistant_response_notification', { request });
}

export function sendUserQuestionNotification(
  request: UserQuestionNotificationInput,
): Promise<boolean> {
  return invoke<boolean>('send_user_question_notification', { request });
}

function isWindowsNotificationStatus(value: unknown): value is WindowsNotificationStatus {
  return value === 'enabled' ||
    value === 'disabledForApplication' ||
    value === 'disabledForUser' ||
    value === 'disabledByGroupPolicy' ||
    value === 'disabledByManifest' ||
    value === 'unavailable' ||
    value === 'notSupported';
}

function readPreference(key: keyof NotificationPreferences): boolean {
  let value: string | null;
  try {
    value = window.localStorage.getItem(preferenceKeys[key]);
  } catch {
    throw new Error('Notification preferences could not be read.');
  }
  if (value === null) return defaultPreferences[key];
  if (value !== 'true' && value !== 'false') {
    throw new Error('Notification preferences are invalid.');
  }
  return value === 'true';
}
