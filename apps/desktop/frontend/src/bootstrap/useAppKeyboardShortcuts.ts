import { useEffect } from 'react';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import {
  keyboardShortcutActions,
  keyboardShortcutMatches,
  keyboardShortcutRequiresModifier,
  useKeyboardShortcutsStore,
} from '@/modules/settings/stores/keyboardShortcutsStore';
import type { AppSurface } from '@/shell/NavigationRail';

export function isEditableShortcutTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.closest('input, textarea, select, [role="textbox"], [contenteditable="true"]') !== null)
  );
}

export function isDialogShortcutTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.closest('[role="dialog"], dialog') !== null;
}

export interface UseAppKeyboardShortcutsOptions {
  activeSurface: AppSurface;
  handleTogglePrimaryRightPanel: () => void;
  navigateSurface: (surface: AppSurface) => void;
  requestTemporaryChatExit: (action: () => void | Promise<void>) => void;
  setIsCommandPaletteOpen: (open: boolean) => void;
  setIsSidebarOpen: (updater: (open: boolean) => boolean) => void;
  startWorkspaceChat: () => void;
}

export function useAppKeyboardShortcuts({
  activeSurface,
  handleTogglePrimaryRightPanel,
  navigateSurface,
  requestTemporaryChatExit,
  setIsCommandPaletteOpen,
  setIsSidebarOpen,
  startWorkspaceChat,
}: UseAppKeyboardShortcutsOptions): void {
  const keyboardBindings = useKeyboardShortcutsStore((state) => state.bindings);
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
        const target = event.target;
        if (isEditableShortcutTarget(target) || isDialogShortcutTarget(target)) return;
        event.preventDefault();
        setIsSidebarOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [setIsSidebarOpen]);

  useEffect(() => {
    const handleGlobalShortcut = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat) return;
      if (isDialogShortcutTarget(event.target)) return;

      const action = keyboardShortcutActions.find((candidate) => {
        const binding = keyboardBindings[candidate];
        if (
          binding === null ||
          (keyboardShortcutRequiresModifier(candidate) && !event.ctrlKey && !event.altKey && !event.metaKey)
        ) {
          return false;
        }
        return keyboardShortcutMatches(event, binding);
      });
      if (!action || action === 'sendMessage' || action === 'insertNewLine' || action === 'stopResponse') return;
      if (isEditableShortcutTarget(event.target) && action !== 'openCommandPalette') return;

      event.preventDefault();
      switch (action) {
        case 'newChat':
          startWorkspaceChat();
          break;
        case 'openSearch':
          window.dispatchEvent(new Event('openchat-open-title-search'));
          break;
        case 'openModelPicker':
          requestTemporaryChatExit(() => {
            navigateSurface('chats');
            window.dispatchEvent(new Event('openchat-open-model-picker'));
          });
          break;
        case 'openCommandPalette':
          setIsCommandPaletteOpen(true);
          break;
        case 'toggleRightPanel':
          if (activeSurface === 'chats') handleTogglePrimaryRightPanel();
          break;
        case 'previousConversation':
        case 'nextConversation': {
          const available = conversations.filter((conversation) => !conversation.isArchived);
          const index = available.findIndex((conversation) => conversation.id === activeConversationId);
          const offset = action === 'previousConversation' ? -1 : 1;
          const target = index >= 0 ? available[index + offset] : undefined;
          if (target) {
            requestTemporaryChatExit(async () => {
              navigateSurface('chats');
              await useChatStore.getState().selectConversation(target.id);
            });
          }
          break;
        }
        default:
          break;
      }
    };
    window.addEventListener('keydown', handleGlobalShortcut, true);
    return () => window.removeEventListener('keydown', handleGlobalShortcut, true);
  }, [
    activeConversationId,
    activeSurface,
    conversations,
    handleTogglePrimaryRightPanel,
    keyboardBindings,
    navigateSurface,
    requestTemporaryChatExit,
    setIsCommandPaletteOpen,
    startWorkspaceChat,
  ]);
}
