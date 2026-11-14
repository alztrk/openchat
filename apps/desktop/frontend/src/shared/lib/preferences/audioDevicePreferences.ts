import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

const inputDeviceKey = 'audio.input_device_id';
const outputDeviceKey = 'audio.output_device_id';

export function readPreferredAudioInputDeviceId(): string | null {
  return readDeviceId(inputDeviceKey);
}

export function writePreferredAudioInputDeviceId(deviceId: string | null): Promise<void> {
  return writeDeviceId(inputDeviceKey, deviceId);
}

export function readPreferredAudioOutputDeviceId(): string | null {
  return readDeviceId(outputDeviceKey);
}

export function writePreferredAudioOutputDeviceId(deviceId: string | null): Promise<void> {
  return writeDeviceId(outputDeviceKey, deviceId);
}

function readDeviceId(key: string): string | null {
  const value = unifiedStorage.getItem(key);
  return value && value.length > 0 ? value : null;
}

function writeDeviceId(key: string, deviceId: string | null): Promise<void> {
  return unifiedStorage.setPersistedItem(key, deviceId ?? '');
}
