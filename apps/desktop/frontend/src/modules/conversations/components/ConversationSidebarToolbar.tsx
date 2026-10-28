import {
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Clock3,
  Folder,
  FolderPlus,
  Menu,
  Plus,
  SlidersHorizontal,
  SquarePen,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/design-system';
import type { SidebarSortMode, SidebarViewMode } from '@/modules/conversations/data/sidebarPreferences';
import type { AssistantProfile } from '@/modules/settings/data/assistantProfiles';

export interface ConversationSidebarToolbarProps {
  isBusy: boolean;
  temporaryChat: boolean;
  viewMode: SidebarViewMode;
  sortMode: SidebarSortMode;
  agentProfiles: readonly AssistantProfile[];
  agentProfilesLoadState: 'idle' | 'loading' | 'ready' | 'error';
  onNewChat: () => void;
  onNewChatWithAgent: (agent: AssistantProfile) => void;
  onManageAgents: () => void;
  onRetryAgentProfiles: () => void;
  onCreateProject: () => void;
  onStartTemporaryChat: () => void;
  onViewModeChange: (mode: SidebarViewMode) => void;
  onSortModeChange: (mode: SidebarSortMode) => void;
  onCreateFolder: () => void;
}

export function ConversationSidebarToolbar({
  isBusy,
  temporaryChat,
  viewMode,
  sortMode,
  agentProfiles,
  agentProfilesLoadState,
  onNewChat,
  onNewChatWithAgent,
  onManageAgents,
  onRetryAgentProfiles,
  onCreateProject,
  onStartTemporaryChat,
  onViewModeChange,
  onSortModeChange,
  onCreateFolder,
}: ConversationSidebarToolbarProps) {
  const { t } = useTranslation();

  const agentMenuItems = (
    <>
      {agentProfilesLoadState === 'idle' || agentProfilesLoadState === 'loading' ? (
        <p aria-live="polite" className="px-2 py-2 text-xs text-secondary-text" role="status">
          {t('modelsLoading')}
        </p>
      ) : agentProfilesLoadState === 'error' ? (
        <div className="grid gap-1 px-2 py-2" role="alert">
          <p className="text-xs text-destructive">{t('assistantProfilesLoadFailed')}</p>
          <DropdownMenuItem asChild onSelect={onRetryAgentProfiles}>
            <Button className="w-full justify-start" size="small" type="button" variant="ghost">
              {t('retry')}
            </Button>
          </DropdownMenuItem>
        </div>
      ) : agentProfiles.length > 0 ? (
        agentProfiles.map((agent) => (
          <DropdownMenuItem
            asChild
            className="w-full min-w-0 gap-2"
            disabled={isBusy}
            key={agent.id}
            onSelect={() => onNewChatWithAgent(agent)}
            textValue={agent.name}
          >
            <Button className="w-full justify-start" disabled={isBusy} size="small" type="button" variant="ghost">
              <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
              <span className="min-w-0 flex-1 truncate text-left">{agent.name}</span>
            </Button>
          </DropdownMenuItem>
        ))
      ) : (
        <p className="px-2 py-2 text-xs text-secondary-text" role="status">
          {t('assistantProfilesEmpty')}
        </p>
      )}
      <DropdownMenuSeparator />
      <DropdownMenuItem asChild className="w-full min-w-0 gap-2" onSelect={onManageAgents}>
        <Button className="w-full justify-start" size="small" type="button" variant="ghost">
          <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
          <span>{t('assistantProfileManage')}</span>
        </Button>
      </DropdownMenuItem>
    </>
  );

  const viewAndSortMenuItems = (
    <>
      <DropdownMenuLabel>{t('sidebarViewModeLabel')}</DropdownMenuLabel>
      {(['compact', 'detailed'] as const).map((mode) => (
        <DropdownMenuItem
          aria-checked={viewMode === mode}
          className="justify-between"
          key={mode}
          onSelect={() => onViewModeChange(mode)}
          role="menuitemradio"
        >
          <span>{t(mode === 'compact' ? 'sidebarViewCompact' : 'sidebarViewDetailed')}</span>
          {viewMode === mode ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
        </DropdownMenuItem>
      ))}
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t('sidebarSortByLabel')}</DropdownMenuLabel>
      {(
        [
          { id: 'updatedAt', labelKey: 'sidebarSortUpdatedAt' },
          { id: 'createdAt', labelKey: 'sidebarSortCreatedAt' },
          { id: 'alphabetical', labelKey: 'sidebarSortAlphabetical' },
          { id: 'manual', labelKey: 'sidebarSortManual' },
        ] as const
      ).map((option) => (
        <DropdownMenuItem
          aria-checked={sortMode === option.id}
          className="justify-between"
          key={option.id}
          onSelect={() => onSortModeChange(option.id)}
          role="menuitemradio"
        >
          <span>{t(option.labelKey)}</span>
          {sortMode === option.id ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
        </DropdownMenuItem>
      ))}
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={onCreateFolder}>
        <Folder aria-hidden="true" size={14} />
        <span>{t('createCustomFolder')}</span>
      </DropdownMenuItem>
    </>
  );

  return (
    <div className="conversation-sidebar-toolbar sticky top-0 z-10 flex shrink-0 items-center gap-1.5 bg-navigation/80 px-3 pt-3 pb-2 backdrop-blur-md">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            aria-label={t('moreOptions')}
            className="conversation-sidebar-minimal-menu-trigger size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            size="icon"
            title={t('moreOptions')}
            type="button"
            variant="secondary"
          >
            <Menu aria-hidden="true" size={16} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" aria-label={t('moreOptions')} className="min-w-56">
          <DropdownMenuItem onSelect={onNewChat}>
            <SquarePen aria-hidden="true" size={14} />
            <span>{t('newChat')}</span>
          </DropdownMenuItem>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger className="justify-between" disabled={isBusy}>
              <span className="flex min-w-0 items-center gap-3">
                <Bot aria-hidden="true" size={14} />
                <span className="truncate">{t('newChatWithAgent')}</span>
              </span>
              <ChevronRight aria-hidden="true" size={14} />
            </DropdownMenuSubTrigger>
            <DropdownMenuSubContent aria-label={t('newChatWithAgent')} className="min-w-60">
              <DropdownMenuLabel>{t('newChatWithAgent')}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {agentMenuItems}
            </DropdownMenuSubContent>
          </DropdownMenuSub>
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={isBusy} onSelect={onCreateProject}>
            <FolderPlus aria-hidden="true" size={14} />
            <span>{t('createProject')}</span>
          </DropdownMenuItem>
          <DropdownMenuItem
            aria-checked={Boolean(temporaryChat)}
            className="justify-between"
            onSelect={onStartTemporaryChat}
            role="menuitemcheckbox"
          >
            <span className="flex min-w-0 items-center gap-3">
              <Clock3 aria-hidden="true" size={14} />
              <span className="truncate">
                {t(temporaryChat ? 'temporaryChatEndButtonTitle' : 'temporaryChatStartButtonTitle')}
              </span>
            </span>
            {temporaryChat ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuSub>
            <DropdownMenuSubTrigger className="justify-between">
              <span className="flex min-w-0 items-center gap-3">
                <SlidersHorizontal aria-hidden="true" size={14} />
                <span className="truncate">{t('sidebarViewAndSortOptions')}</span>
              </span>
              <ChevronRight aria-hidden="true" size={14} />
            </DropdownMenuSubTrigger>
            <DropdownMenuSubContent aria-label={t('sidebarViewAndSortOptions')} className="min-w-52">
              {viewAndSortMenuItems}
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        </DropdownMenuContent>
      </DropdownMenu>
      <div className="conversation-sidebar-new-chat-group flex h-9 min-w-0 flex-1 overflow-hidden rounded-button border border-border bg-surface/80 shadow-2xs backdrop-blur-sm">
        <Button
          aria-label={t('newChat')}
          className="conversation-sidebar-new-chat-main group min-h-0 h-full min-w-0 flex-1 justify-between rounded-none border-0 bg-transparent px-3 text-left text-settings-description font-medium text-text shadow-none transition-colors duration-quick hover:bg-raised/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus"
          onClick={onNewChat}
          size="small"
          title={t('newChatButtonTitle')}
          type="button"
          variant="ghost"
        >
          <span className="flex min-w-0 items-center gap-2">
            <SquarePen
              aria-hidden="true"
              className="shrink-0 text-secondary-icon transition-colors group-hover:text-text"
              size={15}
            />
            <span className="conversation-sidebar-new-chat-label truncate">{t('newChat')}</span>
          </span>
          <Plus
            aria-hidden="true"
            className="conversation-sidebar-new-chat-plus shrink-0 text-secondary-icon/70 transition-colors group-hover:text-text"
            size={14}
          />
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={t('newChatWithAgent')}
              className="conversation-sidebar-agent-button h-full w-9 shrink-0 gap-1 rounded-none border-0 border-l border-l-border bg-transparent pl-1 pr-0 text-secondary-icon transition-colors duration-quick hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus data-[state=open]:bg-raised data-[state=open]:text-text disabled:opacity-45"
              disabled={isBusy}
              size="icon"
              title={t('newChatWithAgent')}
              type="button"
              variant="ghost"
            >
              <Bot aria-hidden="true" size={15} />
              <ChevronDown aria-hidden="true" className="conversation-sidebar-agent-chevron" size={12} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" aria-label={t('newChatWithAgent')} className="min-w-60">
            <DropdownMenuLabel>{t('newChatWithAgent')}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            {agentMenuItems}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <Button
        aria-label={t('createProject')}
        className="conversation-sidebar-toolbar-button size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-45"
        disabled={isBusy}
        onClick={onCreateProject}
        size="icon"
        title={t('createProjectButtonTitle')}
        type="button"
        variant="secondary"
      >
        <FolderPlus aria-hidden="true" size={15} />
      </Button>
      <Button
        aria-label={temporaryChat ? t('temporaryChatEnd') : t('temporaryChatStart')}
        aria-pressed={Boolean(temporaryChat)}
        className={`conversation-sidebar-toolbar-button size-9 shrink-0 border px-0 backdrop-blur-sm transition-all duration-quick focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
          temporaryChat
            ? 'border-accent/40 bg-accent/15 text-accent hover:bg-accent/25'
            : 'border-border bg-surface/80 text-secondary-icon shadow-2xs hover:border-control-border hover:bg-raised/90 hover:text-text'
        }`}
        onClick={onStartTemporaryChat}
        size="icon"
        title={temporaryChat ? t('temporaryChatEndButtonTitle') : t('temporaryChatStartButtonTitle')}
        type="button"
        variant="ghost"
      >
        <Clock3 aria-hidden="true" size={15} />
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            aria-label={t('sidebarViewAndSortOptions')}
            className="conversation-sidebar-toolbar-button size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus data-[state=open]:border-control-border data-[state=open]:bg-raised data-[state=open]:text-text"
            size="icon"
            title={t('sidebarViewAndSortOptions')}
            type="button"
            variant="secondary"
          >
            <SlidersHorizontal aria-hidden="true" size={15} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent aria-label={t('sidebarViewAndSortOptions')} className="min-w-52">
          {viewAndSortMenuItems}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
