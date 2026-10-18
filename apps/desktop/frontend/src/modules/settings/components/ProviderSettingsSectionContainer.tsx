import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ProviderSettingsSection } from '@/modules/providers/components/ProviderSettingsSection';

export function ProviderSettingsSectionContainer() {
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const models = useChatStore((state) => state.models);
  const refreshModels = useChatStore((state) => state.refreshModels);

  return (
    <ProviderSettingsSection
      modelError={modelError}
      modelStatus={modelStatus}
      models={models}
      refreshModels={refreshModels}
      serviceStatus={serviceStatus}
    />
  );
}
