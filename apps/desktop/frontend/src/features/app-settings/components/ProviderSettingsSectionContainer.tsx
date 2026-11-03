import { useChatStore } from '@app/runtime/chatStore';
import { ProviderSettingsSection } from '@app/runtime/providerSettings';

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
