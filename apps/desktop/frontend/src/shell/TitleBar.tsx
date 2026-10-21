import { ArrowLeft, ArrowRight, Copy, Minus, PanelLeftClose, PanelLeftOpen, Square, X } from 'lucide-react';
import { type MouseEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import { getCurrentWindow, isTauri } from '@/platform/tauri';
import { TitleBarSearch } from './TitleBarSearch';

export interface TitleBarProps {
  canGoBack: boolean;
  canGoForward: boolean;
  isSidebarOpen?: boolean;
  onGoBack(): void;
  onGoForward(): void;
  onSelectConversation(conversationId: string, messageId?: string | null, searchQuery?: string | null): void;
  onToggleSidebar?(): void;
}

export function TitleBar({
  canGoBack,
  canGoForward,
  isSidebarOpen = true,
  onGoBack,
  onGoForward,
  onSelectConversation,
  onToggleSidebar,
}: TitleBarProps) {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);
  const desktop = isTauri();

  useEffect(() => {
    if (!desktop) return;

    const currentWindow = getCurrentWindow();
    let active = true;
    let unlistenResize: (() => void) | undefined;

    void currentWindow.isMaximized().then((value) => {
      if (active) setMaximized(value);
    });
    void currentWindow
      .onResized(() => {
        void currentWindow.isMaximized().then((value) => {
          if (active) setMaximized(value);
        });
      })
      .then((unlisten) => {
        if (active) unlistenResize = unlisten;
        else unlisten();
      });

    return () => {
      active = false;
      unlistenResize?.();
    };
  }, [desktop]);

  function handleTitleBarMouseDown(event: MouseEvent<HTMLElement>): void {
    if (!desktop || event.button !== 0) return;
    if (event.target instanceof Element && event.target.closest('button, input, [data-titlebar-no-drag]')) return;

    const currentWindow = getCurrentWindow();
    if (event.detail === 2) {
      void currentWindow.toggleMaximize();
      return;
    }
    void currentWindow.startDragging();
  }

  return (
    <>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: Tauri uses this header's native drag gesture to move the window. */}
      <header
        className="app-shell-chrome app-shell-header titlebar relative flex h-12 shrink-0 items-center justify-between"
        onMouseDown={handleTitleBarMouseDown}
      >
        <div className="titlebar-navigation items-center gap-2">
          <Tooltip label={t('previousPage')}>
            <Button
              aria-label={t('previousPage')}
              className="size-8"
              disabled={!canGoBack}
              onClick={onGoBack}
              size="icon"
              variant="ghost"
            >
              <ArrowLeft aria-hidden="true" size={18} />
            </Button>
          </Tooltip>
          <Tooltip label={t('nextPage')}>
            <Button
              aria-label={t('nextPage')}
              className="size-8"
              disabled={!canGoForward}
              onClick={onGoForward}
              size="icon"
              variant="ghost"
            >
              <ArrowRight aria-hidden="true" size={18} />
            </Button>
          </Tooltip>
          {onToggleSidebar ? (
            <Tooltip label={t(isSidebarOpen ? 'collapseSidebars' : 'showSidebars')}>
              <Button
                aria-label={t(isSidebarOpen ? 'collapseSidebars' : 'showSidebars')}
                className="size-8 text-secondary-icon opacity-70 transition-opacity hover:text-text hover:opacity-100"
                onClick={onToggleSidebar}
                size="icon"
                variant="ghost"
              >
                {isSidebarOpen ? (
                  <PanelLeftClose aria-hidden="true" size={18} />
                ) : (
                  <PanelLeftOpen aria-hidden="true" size={18} />
                )}
              </Button>
            </Tooltip>
          ) : null}
        </div>
        <div className="titlebar-drag-region min-w-3 flex-1 self-stretch" />
        <TitleBarSearch onSelectConversation={onSelectConversation} />
        <div className="titlebar-drag-region min-w-3 flex-1 self-stretch" />
        {desktop ? (
          <div className="titlebar-window-controls h-full items-center gap-2">
            <Tooltip label={t('minimizeWindow')}>
              <Button
                aria-label={t('minimizeWindow')}
                className="size-8 rounded-control"
                onClick={() => void getCurrentWindow().minimize()}
                size="icon"
                variant="ghost"
              >
                <Minus aria-hidden="true" size={12} />
              </Button>
            </Tooltip>
            <Tooltip label={t(maximized ? 'restoreWindow' : 'maximizeWindow')}>
              <Button
                aria-label={t(maximized ? 'restoreWindow' : 'maximizeWindow')}
                className="size-8 rounded-control"
                onClick={() => void getCurrentWindow().toggleMaximize()}
                size="icon"
                variant="ghost"
              >
                {maximized ? <Copy aria-hidden="true" size={12} /> : <Square aria-hidden="true" size={12} />}
              </Button>
            </Tooltip>
            <Tooltip label={t('close')}>
              <Button
                aria-label={t('close')}
                className="size-8 rounded-control hover:bg-destructive hover:text-white"
                onClick={() => void getCurrentWindow().close()}
                size="icon"
                variant="ghost"
              >
                <X aria-hidden="true" size={12} />
              </Button>
            </Tooltip>
          </div>
        ) : null}
      </header>
    </>
  );
}
