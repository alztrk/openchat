/** Main app layout with sidebar, chat workspace, and right panel */
import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent } from 'react';
import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { lazy, Suspense } from 'react';
import { NavigationRail } from '@/shell/NavigationRail';
import { TitleBar } from '@/shell/TitleBar';
import { BrandMark } from '@/shell/BrandMark';
import { ConversationActionsHeader } from '@/shell/ConversationActionsHeader';
import { useAppDragDrop } from '@/bootstrap/useAppDragDrop';
import { useChatStore } from '@features/chat/state/chatStore';

const ConversationPane = lazy(() => import('@features/chat/components/ConversationPane').then((m) => ({ default: m.ConversationPane })));
const ConversationSidebar = lazy(() => import('@features/chat/components/ConversationSidebar').then((m) => ({ default: m.ConversationSidebar })));
const ChatFileChangesPanel = lazy(() => import('@features/chat/components/ChatFileChangesPanel').then((m) => ({ default: m.ChatFileChangesPanel })));

/** Main app layout component - ~200 lines */
export function AppLayout({
  activeSurface,
  isDark,
  surfaceHeadingRef,
  onNavigate,
  onToggleTheme,
  canGoBack,
  canGoForward,
  onGoBack,
  onGoForward,
  onOpenCommandPalette,
  onSelectConversation,
  onToggleSidebar,
  isSidebarOpen,
  setIsSidebarOpen,
  startWorkspaceChat,
  startWorkspaceTemporaryChat,
  requestTemporaryChatExit,
  openAgentLibrary,
}: {
  activeSurface: string;
  isDark: boolean;
  surfaceHeadingRef: React.RefObject<HTMLElement>;
  onNavigate: (surface: string) => void;
  onToggleTheme: () => void;
  canGoBack: boolean;
  canGoForward: boolean;
  onGoBack: () => void;
  onGoForward: () => void;
  onOpenCommandPalette: () => void;
  onSelectConversation: (id: string) => void;
  onToggleSidebar?: () => void;
  isSidebarOpen: boolean;
  setIsSidebarOpen: (open: boolean) => void;
  startWorkspaceChat: () => void;
  startWorkspaceTemporaryChat: () => void;
  requestTemporaryChatExit: (action: () => void | Promise<void>) => void;
  openAgentLibrary: () => void;
}) {
  const { t } = useTranslation();
  const [isRightPanelOpen, setIsRightPanelOpen] = useState(false);
  const [workspaceRatio, setWorkspaceRatio] = useState(50);
  const workspaceDropRef = useRef<HTMLDivElement>(null);
  const rightPanelShellRef = useRef<HTMLDivElement>(null);

  const conversations = useChatStore((s) => s.conversations);
  const activeConversationId = useChatStore((s) => s.activeConversationId);
  const serviceStatus = useChatStore((s) => s.serviceStatus);

  const handleTogglePrimaryRightPanel = useCallback(() => {
    if (isRightPanelOpen) {
      setIsRightPanelOpen(false);
      return;
    }
    setIsRightPanelOpen(true);
  }, [isRightPanelOpen]);

  const { handleWorkspaceDragEnter, handleWorkspaceDragLeave, handleWorkspaceDragOver, handleWorkspaceDrop } = useAppDragDrop({
    isRightPanelOpen,
    rightPanelShellRef,
    workspaceDropRef,
    onOpenConversationInPanel: () => {},
    onOpenConversationInPrimary: () => {},
  });

  const handleWorkspaceResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const container = workspaceDropRef.current;
    if (!container) return;

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      if (bounds.width <= 0) return;
      const nextRatio = Math.max(30, Math.min(70, ((moveEvent.clientX - bounds.left) / bounds.width) * 100));
      setWorkspaceRatio(nextRatio);
    };

    const handlePointerUp = (upEvent: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      if (bounds.width > 0) {
        const finalRatio = Math.max(30, Math.min(70, ((upEvent.clientX - bounds.left) / bounds.width) * 100));
        setWorkspaceRatio(finalRatio);
      }
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);
  };

  const handleWorkspaceResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowLeft' ? -5 : 5;
    setWorkspaceRatio((current) => Math.max(30, Math.min(70, current + delta)));
  };

  const headingMap: Record<string, string> = { chats: 'chats', design: 'design', models: 'modelLibrary', security: 'securityScansTitle', settings: 'settings' };
  const headingKey = (headingMap[activeSurface] ?? 'chats') as string;

  return (
    <div className="app-shell h-screen min-h-desktop-window overflow-hidden bg-background text-text">
      <TitleBar canGoBack={canGoBack} canGoForward={canGoForward} isSidebarOpen={isSidebarOpen} onGoBack={onGoBack} onGoForward={onGoForward} onOpenCommandPalette={() => onOpenCommandPalette()} onSelectConversation={onSelectConversation} onToggleSidebar={onToggleSidebar} />
      <NavigationRail activeSurface={activeSurface as 'chats' | 'design' | 'models' | 'security' | 'settings'} isDark={isDark} onNavigate={onNavigate} onToggleTheme={onToggleTheme} />
      <div className="app-shell-main relative flex min-h-0 min-w-0">
        <main aria-label={t(headingKey as never)} className="app-main-surface relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden" data-motion-input="pointer" key={activeSurface} ref={surfaceHeadingRef} tabIndex={-1}>
          <div className="flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden">
            <div className="conversation-sidebar-shell" data-open={isSidebarOpen ? 'true' : 'false'} inert={!isSidebarOpen}>
              <Suspense fallback={<div className="flex h-full min-h-0 w-full items-center justify-center px-4 py-8"><p className="text-center text-xs text-secondary-text">{t('conversationsLoading')}</p></div>}>
                <ConversationSidebar onCloseMobile={() => setIsSidebarOpen(false)} onManageAgents={openAgentLibrary} onNewAgentChat={() => {}} onNewChat={startWorkspaceChat} onRequestTemporaryExit={requestTemporaryChatExit} onStartTemporaryChat={startWorkspaceTemporaryChat} />
              </Suspense>
            </div>
            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
              <section aria-label={t('composerDropTargetLabel')} className="chat-workspace-layout relative flex min-h-0 min-w-0 flex-1 overflow-hidden" onDragEnter={handleWorkspaceDragEnter} onDragLeave={handleWorkspaceDragLeave} onDragOver={handleWorkspaceDragOver} onDrop={handleWorkspaceDrop} ref={workspaceDropRef}>
                <div className="chat-primary-pane relative flex min-h-0 min-w-0 flex-col overflow-hidden" style={isRightPanelOpen ? { flex: `${workspaceRatio} 1 0%` } : { flex: '1 1 0%' }}>
                  <Suspense fallback={<p className="flex min-h-0 flex-1 items-center justify-center px-6 py-8 text-sm text-secondary-text" role="status">{t('messageHistoryLoading')}</p>}>
                    <ConversationPane header={() => <ConversationActionsHeader activeConversationId={activeConversationId} conversations={conversations} isRightPanelOpen={isRightPanelOpen} onToggleRightPanel={handleTogglePrimaryRightPanel} />} hideInternalRightPanel isLargeText={false} isRightPanelOpen={isRightPanelOpen} notificationFeedbackKey={null} onDismissNotificationFeedback={() => {}} onOpenRightPanel={() => {}} onOpenWorkspaceFile={() => {}} onOpenSettings={() => onNavigate('settings')} onQuestionFocusHandled={() => {}} onRequestTemporaryExit={requestTemporaryChatExit} onToggleRightPanel={handleTogglePrimaryRightPanel} questionFocusTarget={null} />
                  </Suspense>
                </div>
                {isRightPanelOpen && (
                  <div aria-label={t('resizeWorkspacePanel')} aria-orientation="vertical" aria-valuemax={70} aria-valuemin={30} aria-valuenow={workspaceRatio} className="chat-workspace-panel-separator group relative z-20 flex w-1.5 shrink-0 cursor-col-resize items-center justify-center bg-border/60 transition-colors hover:bg-accent/50 focus-visible:bg-accent focus-visible:outline-none" onKeyDown={handleWorkspaceResizeKeyDown} onPointerDown={handleWorkspaceResizePointerDown} role="separator" tabIndex={0}>
                    <span className="h-8 w-0.5 rounded-full bg-secondary-icon/50 transition-colors group-hover:bg-accent" />
                  </div>
                )}
                <div className="chat-right-panel-shell" data-open={isRightPanelOpen ? 'true' : 'false'} inert={!isRightPanelOpen} ref={rightPanelShellRef} style={isRightPanelOpen ? { flex: `${100 - workspaceRatio} 1 0%` } : undefined}>
                  {isRightPanelOpen && <Suspense fallback={null}><ChatFileChangesPanel conversationId={activeConversationId} conversations={conversations} conversationListReady={serviceStatus === 'ready'} fileRequest={null} initialTab={null} panelRequest={null} renderConversation={() => null} /></Suspense>}
                </div>
              </section>
            </div>
          </div>
        </main>
      </div>
      <footer className="app-shell-chrome app-shell-footer flex select-none items-center justify-between gap-2 px-2 py-1 text-xs font-medium text-secondary-text">
        <BrandMark />
      </footer>
    </div>
  );
}
