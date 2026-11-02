import { useChatStore } from '@features/chat/state/chatStore';
import { ProviderSettingsSection } from '@features/provider-settings/components/ProviderSettingsSection';

export function ProviderSettingsSectionContainer() {
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const refreshModels = useChatStore((state) => state.refreshModels);

  return (
    <ProviderSettingsSection
      refreshModels={refreshModels}
      serviceStatus={serviceStatus}
    />
  );
}
