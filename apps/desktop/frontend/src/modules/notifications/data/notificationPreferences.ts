import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { invoke } from '@/platform/tauri';

export interface NotificationPreferences {
  responses: boolean;
  questions: boolean;
  responsePreview: boolean;
  quickReply: boolean;
  soundEnabled: boolean;
  soundMuted: boolean;
  soundVolume: number;
  failureSoundEnabled: boolean;
  quietHoursEnabled: boolean;
  quietHoursStart: string;
  quietHoursEnd: string;
  mutedProjectIds: string[];
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
  quietHoursEnabled: 'notifications.quiet_hours_enabled',
  quietHoursStart: 'notifications.quiet_hours_start',
  quietHoursEnd: 'notifications.quiet_hours_end',
  mutedProjectIds: 'notifications.muted_project_ids',
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
  quietHoursEnabled: false,
  quietHoursStart: '22:00',
  quietHoursEnd: '08:00',
  mutedProjectIds: [],
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
    quietHoursEnabled: readBooleanPreference('quietHoursEnabled'),
    quietHoursStart: readTimePreference('quietHoursStart'),
    quietHoursEnd: readTimePreference('quietHoursEnd'),
    mutedProjectIds: readStringArrayPreference('mutedProjectIds'),
  };
}

export function writeNotificationPreference<K extends keyof NotificationPreferences>(
  key: K,
  value: NotificationPreferences[K],
): Promise<void> {
  try {
    if (key === 'soundVolume') {
      if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) {
        throw new Error('Notification sound volume is invalid.');
      }
    }
    if (key === 'quietHoursStart' || key === 'quietHoursEnd') {
      if (typeof value !== 'string' || !/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) {
        throw new Error('Notification quiet hours are invalid.');
      }
    }
    if (key === 'mutedProjectIds') {
      if (
        !Array.isArray(value) ||
        value.length > 1000 ||
        !value.every((id: unknown) => typeof id === 'string' && id.length > 0 && id.length <= 128)
      ) {
        throw new Error('Muted project identifiers are invalid.');
      }
    }
    if (
      key !== 'soundVolume' &&
      key !== 'quietHoursStart' &&
      key !== 'quietHoursEnd' &&
      key !== 'mutedProjectIds' &&
      typeof value !== 'boolean'
    ) {
      throw new Error('Notification preference is invalid.');
    }
    return unifiedStorage.setPersistedItem(
      preferenceKeys[key],
      Array.isArray(value) ? JSON.stringify(value) : String(value),
    );
  } catch {
    throw new Error('Notification preferences could not be saved.');
  }
}

export function shouldSuppressNotification(
  preferences: NotificationPreferences,
  projectId: string | null,
  now: Date = new Date(),
): boolean {
  if (projectId !== null && preferences.mutedProjectIds.includes(projectId)) return true;
  if (!preferences.quietHoursEnabled || preferences.quietHoursStart === preferences.quietHoursEnd) return false;
  const currentMinute = now.getHours() * 60 + now.getMinutes();
  const startMinute = timeToMinutes(preferences.quietHoursStart);
  const endMinute = timeToMinutes(preferences.quietHoursEnd);
  return startMinute < endMinute
    ? currentMinute >= startMinute && currentMinute < endMinute
    : currentMinute >= startMinute || currentMinute < endMinute;
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
  | 'failureSoundEnabled'
  | 'quietHoursEnabled';

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

function readTimePreference(key: 'quietHoursStart' | 'quietHoursEnd'): string {
  let value: string | null;
  try {
    value = unifiedStorage.getItem(preferenceKeys[key]);
  } catch {
    throw new Error('Notification preferences could not be read.');
  }
  if (value === null) return defaultPreferences[key];
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) throw new Error('Notification preferences are invalid.');
  return value;
}

function readStringArrayPreference(key: 'mutedProjectIds'): string[] {
  let value: string | null;
  try {
    value = unifiedStorage.getItem(preferenceKeys[key]);
  } catch {
    throw new Error('Notification preferences could not be read.');
  }
  if (value === null) return defaultPreferences[key];
  const parsed: unknown = JSON.parse(value);
  if (
    !Array.isArray(parsed) ||
    parsed.length > 1000 ||
    !parsed.every((item: unknown) => typeof item === 'string' && item.length > 0 && item.length <= 128)
  ) {
    throw new Error('Notification preferences are invalid.');
  }
  return [...new Set(parsed.filter((item: unknown): item is string => typeof item === 'string'))];
}

function timeToMinutes(value: string): number {
  const [hours, minutes] = value.split(':').map(Number);
  return (hours ?? 0) * 60 + (minutes ?? 0);
}
