import { invoke } from '@/platform/tauri';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export interface NotificationPreferences {
  responses: boolean;
  questions: boolean;
  responsePreview: boolean;
  quickReply: boolean;
  soundEnabled: boolean;
  soundMuted: boolean;
  soundVolume: number;
  failureSoundEnabled: boolean;
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
  soundEnabled: 'notifications.sound_enabled',
  soundMuted: 'notifications.sound_muted',
  soundVolume: 'notifications.sound_volume',
  failureSoundEnabled: 'notifications.failure_sound_enabled',
};

const defaultPreferences: NotificationPreferences = {
  responses: true,
  questions: true,
  responsePreview: true,
  quickReply: true,
  soundEnabled: true,
  soundMuted: false,
  soundVolume: 0.7,
  failureSoundEnabled: true,
};

export function readNotificationPreferences(): NotificationPreferences {
  return {
    responses: readBooleanPreference('responses'),
    questions: readBooleanPreference('questions'),
    responsePreview: readBooleanPreference('responsePreview'),
    quickReply: readBooleanPreference('quickReply'),
    soundEnabled: readBooleanPreference('soundEnabled'),
    soundMuted: readBooleanPreference('soundMuted'),
    soundVolume: readNumberPreference('soundVolume'),
    failureSoundEnabled: readBooleanPreference('failureSoundEnabled'),
  };
}

export function writeNotificationPreference<K extends keyof NotificationPreferences>(
  key: K,
  value: NotificationPreferences[K],
): void {
  try {
    unifiedStorage.setItem(preferenceKeys[key], String(value));
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

export function sendAssistantResponseNotification(request: AssistantResponseNotificationInput): Promise<boolean> {
  return invoke<boolean>('send_assistant_response_notification', { request });
}

export function sendUserQuestionNotification(request: UserQuestionNotificationInput): Promise<boolean> {
  return invoke<boolean>('send_user_question_notification', { request });
}

function isWindowsNotificationStatus(value: unknown): value is WindowsNotificationStatus {
  return (
    value === 'enabled' ||
    value === 'disabledForApplication' ||
    value === 'disabledForUser' ||
    value === 'disabledByGroupPolicy' ||
    value === 'disabledByManifest' ||
    value === 'unavailable' ||
    value === 'notSupported'
  );
}

type BooleanPreferenceKey =
  | 'responses'
  | 'questions'
  | 'responsePreview'
  | 'quickReply'
  | 'soundEnabled'
  | 'soundMuted'
  | 'failureSoundEnabled';

function readBooleanPreference(key: BooleanPreferenceKey): boolean {
  let value: string | null;
  try {
    value = unifiedStorage.getItem(preferenceKeys[key]);
  } catch {
    throw new Error('Notification preferences could not be read.');
  }
  if (value === null) return defaultPreferences[key];
  if (value !== 'true' && value !== 'false') {
    throw new Error('Notification preferences are invalid.');
  }
  return value === 'true';
}

function readNumberPreference(key: 'soundVolume'): number {
  let value: string | null;
  try {
    value = unifiedStorage.getItem(preferenceKeys[key]);
  } catch {
    throw new Error('Notification preferences could not be read.');
  }
  if (value === null) return defaultPreferences[key];
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 1) {
    throw new Error('Notification preferences are invalid.');
  }
  return parsed;
}
