import { Headphones, Mic, RefreshCw, Volume2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Select, type SelectOption } from '@shared/components';
import {
  configureAudioFeedbackOutput,
  supportsAudioFeedbackOutputRouting,
} from '@shared/lib/preferences/audioFeedback';
import {
  readPreferredAudioInputDeviceId,
  readPreferredAudioOutputDeviceId,
  writePreferredAudioInputDeviceId,
  writePreferredAudioOutputDeviceId,
} from '@shared/lib/preferences/audioDevicePreferences';
import { toast } from '@shared/lib/toast';

const defaultInputValue = 'openchat-system-default-input';
const defaultOutputValue = 'openchat-system-default-output';

export function AudioDeviceSettingsSection() {
  const { t } = useTranslation();
  const [inputDeviceId, setInputDeviceId] = useState(() => readPreferredAudioInputDeviceId() ?? defaultInputValue);
  const [inputDevices, setInputDevices] = useState<MediaDeviceInfo[]>([]);
  const [outputDevices, setOutputDevices] = useState<MediaDeviceInfo[]>([]);
  const [isRefreshingInputs, setIsRefreshingInputs] = useState(false);
  const [outputDeviceId, setOutputDeviceId] = useState(() => readPreferredAudioOutputDeviceId());
  const [outputDeviceLabel, setOutputDeviceLabel] = useState('');
  const outputSelectionSupported = supportsAudioFeedbackOutputRouting();

  const loadDevices = useCallback(async () => {
    if (!navigator.mediaDevices?.enumerateDevices) return [];
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices;
  }, []);

  useEffect(() => {
    let mounted = true;
    const refresh = () => {
      void loadDevices()
        .then((devices) => {
          if (!mounted) return;
          setInputDevices(devices.filter((device) => device.kind === 'audioinput' && device.deviceId !== 'default'));
          setOutputDevices(devices.filter((device) => device.kind === 'audiooutput' && device.deviceId !== 'default'));
          const selectedOutput = devices.find(
            (device) => device.kind === 'audiooutput' && device.deviceId === outputDeviceId,
          );
          if (selectedOutput?.label) setOutputDeviceLabel(selectedOutput.label);
        })
        .catch(() => {
          if (mounted) toast.error(t('audioDeviceListFailed'));
        });
    };
    refresh();
    navigator.mediaDevices?.addEventListener('devicechange', refresh);
    return () => {
      mounted = false;
      navigator.mediaDevices?.removeEventListener('devicechange', refresh);
    };
  }, [loadDevices, outputDeviceId, t]);

  const refreshInputDevices = useCallback(async () => {
    if (!navigator.mediaDevices?.getUserMedia || !navigator.mediaDevices.enumerateDevices) {
      toast.error(t('audioInputUnavailable'));
      return;
    }
    setIsRefreshingInputs(true);
    let temporaryStream: MediaStream | null = null;
    try {
      temporaryStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const devices = await navigator.mediaDevices.enumerateDevices();
      setInputDevices(devices.filter((device) => device.kind === 'audioinput' && device.deviceId !== 'default'));
      const availableOutputs = devices.filter((device) => device.kind === 'audiooutput' && device.deviceId !== 'default');
      setOutputDevices(availableOutputs);
      const selectedOutput = availableOutputs.find((device) => device.deviceId === outputDeviceId);
      if (selectedOutput?.label) setOutputDeviceLabel(selectedOutput.label);
    } catch {
      toast.error(t('audioInputListFailed'));
    } finally {
      for (const track of temporaryStream?.getTracks() ?? []) track.stop();
      setIsRefreshingInputs(false);
    }
  }, [outputDeviceId, t]);

  const selectInputDevice = useCallback(
    async (nextDeviceId: string) => {
      const normalizedDeviceId = nextDeviceId === defaultInputValue ? null : nextDeviceId;
      try {
        await writePreferredAudioInputDeviceId(normalizedDeviceId);
        setInputDeviceId(normalizedDeviceId ?? defaultInputValue);
        toast.success(t('audioInputSaved'));
      } catch {
        toast.error(t('audioDeviceSaveFailed'));
      }
    },
    [t],
  );

  const selectOutputDevice = useCallback(async (selectedDeviceId: string) => {
    const nextDeviceId = selectedDeviceId === defaultOutputValue ? null : selectedDeviceId;
    const selectedOutput = outputDevices.find((device) => device.deviceId === nextDeviceId);
    try {
      const previousDeviceId = outputDeviceId;
      const routed = await configureAudioFeedbackOutput(nextDeviceId);
      if (!routed) {
        toast.error(t('audioOutputRouteFailed'));
        return;
      }
      try {
        await writePreferredAudioOutputDeviceId(nextDeviceId);
      } catch {
        await configureAudioFeedbackOutput(previousDeviceId);
        toast.error(t('audioDeviceSaveFailed'));
        return;
      }
      setOutputDeviceId(nextDeviceId);
      setOutputDeviceLabel(nextDeviceId ? selectedOutput?.label || t('audioSelectedOutput') : '');
      toast.success(t('audioOutputSaved'));
    } catch {
      toast.error(t('audioOutputRouteFailed'));
    }
  }, [outputDeviceId, outputDevices, t]);

  const resetOutputDevice = useCallback(async () => {
    const previousDeviceId = outputDeviceId;
    try {
      const routed = await configureAudioFeedbackOutput(null);
      if (!routed) {
        toast.error(t('audioOutputRouteFailed'));
        return;
      }
      try {
        await writePreferredAudioOutputDeviceId(null);
      } catch {
        await configureAudioFeedbackOutput(previousDeviceId);
        toast.error(t('audioDeviceSaveFailed'));
        return;
      }
      setOutputDeviceId(null);
      setOutputDeviceLabel('');
      toast.success(t('audioOutputSaved'));
    } catch {
      toast.error(t('audioDeviceSaveFailed'));
    }
  }, [outputDeviceId, t]);

  const inputOptions: SelectOption[] = [
    { value: defaultInputValue, label: t('audioSystemDefaultInput') },
    ...inputDevices.map((device, index) => ({
      value: device.deviceId,
      label: device.label || t('audioInputDeviceNumber', { number: index + 1 }),
    })),
  ];
  if (inputDeviceId !== defaultInputValue && !inputDevices.some((device) => device.deviceId === inputDeviceId)) {
    inputOptions.push({ value: inputDeviceId, label: t('audioSavedInputUnavailable') });
  }
  const outputOptions: SelectOption[] = [
    { value: defaultOutputValue, label: t('audioSystemDefaultOutputOption') },
    ...outputDevices.map((device, index) => ({
      value: device.deviceId,
      label: device.label || t('audioOutputDeviceNumber', { number: index + 1 }),
    })),
  ];
  if (outputDeviceId && !outputDevices.some((device) => device.deviceId === outputDeviceId)) {
    outputOptions.push({ value: outputDeviceId, label: t('audioSavedOutputUnavailable') });
  }

  return (
    <Card className="grid gap-5 bg-raised" padding="default">
      <div className="grid gap-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-sm font-medium text-text">
            <Mic aria-hidden="true" size={17} />
            {t('audioInputTitle')}
          </div>
          <Button disabled={isRefreshingInputs} onClick={() => void refreshInputDevices()} size="small" variant="secondary">
            <RefreshCw aria-hidden="true" className={isRefreshingInputs ? 'animate-spin' : undefined} size={15} />
            {t(isRefreshingInputs ? 'audioRefreshingDevices' : 'audioRefreshDevices')}
          </Button>
        </div>
        <Select
          label={t('audioInputLabel')}
          onChange={(event) => void selectInputDevice(event.target.value)}
          options={inputOptions}
          value={inputDeviceId}
        />
        <p className="text-sm text-secondary-text">{t('audioInputPrivacyNote')}</p>
      </div>

      <div className="grid gap-3 border-t border-border pt-5">
        <div className="flex items-center gap-2 text-sm font-medium text-text">
          <Headphones aria-hidden="true" size={17} />
          {t('audioOutputTitle')}
        </div>
        <Select
          disabled={!outputSelectionSupported}
          label={t('audioOutputLabel')}
          onChange={(event) => void selectOutputDevice(event.target.value)}
          options={outputOptions}
          value={outputDeviceId ?? defaultOutputValue}
        />
        {outputDeviceId ? (
          <Button className="justify-self-start" onClick={() => void resetOutputDevice()} variant="ghost">
            <Volume2 aria-hidden="true" size={16} />
            {t('audioUseSystemDefaultOutput')}
          </Button>
        ) : null}
        <p aria-live="polite" className="text-sm text-secondary-text" role="status">
          {outputSelectionSupported && outputDeviceId
            ? t('audioSelectedOutputWithName', { name: outputDeviceLabel || t('audioSelectedOutput') })
            : t('audioSystemDefaultOutput')}
        </p>
        {!outputSelectionSupported ? (
          <p className="text-sm text-secondary-text">{t('audioOutputSelectionUnsupported')}</p>
        ) : null}
      </div>
    </Card>
  );
}
