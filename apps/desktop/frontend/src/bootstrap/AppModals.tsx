import { type Dispatch, lazy, type SetStateAction, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import type { SettingsSection } from '@/modules/settings/components/SettingsPage';
import type { AssistantAgentSnapshot } from '@/modules/settings/data/assistantProfiles';
import type { AppSurface } from '@/shell/NavigationRail';

const TemporaryChatExitDialog = lazy(() =>
  import('@/modules/conversations/components/TemporaryChatExitDialog').then((module) => ({
    default: module.TemporaryChatExitDialog,
  })),
);
const CommandPalette = lazy(() =>
  import('@/shell/CommandPalette').then((module) => ({ default: module.CommandPalette })),
);

export interface AppModalsProps {
  activeSurface: AppSurface;
  closeTemporaryChatExitDialog: (open: boolean) => void;
  discardTemporaryChatAndContinue: () => void;
  isCommandPaletteOpen: boolean;
  isDark: boolean;
  isSavingTemporaryChat: boolean;
  navigate: (surface: AppSurface) => void;
  navigateSurface: (surface: AppSurface) => void;
  openAgentLibrary: () => void;
  openSearchConversation: (conversationId: string) => void;
  requestTemporaryChatExit: (action: () => void | Promise<void>) => void;
  saveTemporaryChatAndContinue: () => Promise<void>;
  setIsCommandPaletteOpen: (open: boolean) => void;
  setSettingsSectionRequest: Dispatch<SetStateAction<{ section: SettingsSection; requestId: number }>>;
  startWorkspaceAgentChat: (agent: AssistantAgentSnapshot) => void;
  startWorkspaceChat: () => void;
  temporaryExitIntent: (() => void | Promise<void>) | null;
  toggleTheme: () => Promise<void>;
}

export function AppModals({
  activeSurface,
  closeTemporaryChatExitDialog,
  discardTemporaryChatAndContinue,
  isCommandPaletteOpen,
  isDark,
  isSavingTemporaryChat,
  navigate,
  navigateSurface,
  openAgentLibrary,
  openSearchConversation,
  requestTemporaryChatExit,
  saveTemporaryChatAndContinue,
  setIsCommandPaletteOpen,
  setSettingsSectionRequest,
  startWorkspaceAgentChat,
  startWorkspaceChat,
  temporaryExitIntent,
  toggleTheme,
}: AppModalsProps) {
  const { t } = useTranslation();
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const temporaryChatMessageCount = useChatStore((state) => state.messages.length);
  const temporaryChatErrorCode = useChatStore((state) => state.errorCode);

  return (
    <>
      {temporaryExitIntent !== null ? (
        <Suspense fallback={null}>
          <TemporaryChatExitDialog
            canSave={temporaryChatMessageCount > 0 && temporaryChat?.route !== null}
            errorMessage={
              temporaryChatErrorCode === 'temporaryChatSaveFailed' ||
              temporaryChatErrorCode === 'temporaryChatSaveCleanupFailed'
                ? t(temporaryChatErrorCode)
                : null
            }
            isBusy={isSavingTemporaryChat}
            onDiscard={discardTemporaryChatAndContinue}
            onOpenChange={closeTemporaryChatExitDialog}
            onSave={() => void saveTemporaryChatAndContinue()}
            open
          />
        </Suspense>
      ) : null}
      {isCommandPaletteOpen ? (
        <Suspense fallback={null}>
          <CommandPalette
            isDark={isDark}
            onNewChat={startWorkspaceChat}
            onNavigate={navigate}
            onOpenAgents={openAgentLibrary}
            onSelectModel={(modelKey) => {
              const chat = useChatStore.getState();
              void chat.selectConversationModel(modelKey, chat.activeConversationId ?? undefined);
            }}
            onStartChatWithAgent={startWorkspaceAgentChat}
            onOpenChange={setIsCommandPaletteOpen}
            onOpenSettingsSection={(section) => {
              requestTemporaryChatExit(() => {
                setSettingsSectionRequest((current) => ({ section, requestId: current.requestId + 1 }));
                if (activeSurface !== 'settings') navigateSurface('settings');
              });
            }}
            onSelectConversation={(conversationId) => openSearchConversation(conversationId)}
            onStartProjectChat={(projectId) => {
              requestTemporaryChatExit(() => {
                navigateSurface('chats');
                useChatStore.getState().startNewChat(projectId);
              });
            }}
            onToggleTheme={toggleTheme}
            open
          />
        </Suspense>
      ) : null}
    </>
  );
}
