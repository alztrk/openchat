import { useChatStore } from '@features/chat/state';
import { ProviderSettingsSection } from '@features/provider-settings';

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
