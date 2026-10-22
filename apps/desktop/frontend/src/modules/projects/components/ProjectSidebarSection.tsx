import { Folder, FolderOpen, LoaderCircle, MessageSquarePlus, MoreHorizontal, RefreshCw } from 'lucide-react';
import { type DragEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { ConversationSidebarRow } from '@/modules/conversations/components/ConversationSidebarRow';
import { ConversationSidebarSection } from '@/modules/conversations/components/ConversationSidebarSection';
import type { ChatConversation, ChatProject, ExportFormat } from '@/modules/conversations/data/chatStorage';
import type { CustomSidebarFolder, SidebarViewMode } from '@/modules/conversations/data/sidebarPreferences';

interface ProjectSidebarSectionProps {
  projects: readonly ChatProject[];
  customFolders?: readonly CustomSidebarFolder[];
  conversations: readonly ChatConversation[];
  status: 'idle' | 'loading' | 'ready' | 'error';
  collapsed: boolean;
  activeConversationId: string | null;
  viewMode?: SidebarViewMode;
  isBusy: boolean;
  isConversationAiWorking?(conversationId: string): boolean;
  onToggleCollapsed(): void;
  onRetry(): void;
  onOpenOptions(project: ChatProject): void;
  onCreateProjectChat(projectId: string): void;
  onSelectConversation(id: string): void;
  onTogglePinned(conversation: ChatConversation): void;
  onRename(conversation: ChatConversation): void;
  onInlineRename?(conversation: ChatConversation, title: string): Promise<void> | void;
  onDuplicate?(conversation: ChatConversation): void;
  onExport(conversation: ChatConversation, format?: ExportFormat): void;
  onArchive(conversation: ChatConversation): void;
  onDelete(conversation: ChatConversation): void;
  onMoveToProject(conversation: ChatConversation, projectId: string): void;
  onMoveToChats(conversation: ChatConversation): void;
  onMoveToFolder?(conversation: ChatConversation, folderId: string | null): void;
  onDropConversationToProject(conversationId: string, projectId: string): void;
}

export function ProjectSidebarSection({
  projects,
  customFolders = [],
  conversations,
  status,
  collapsed,
  activeConversationId,
  viewMode = 'compact',
  isBusy,
  isConversationAiWorking,
  onToggleCollapsed,
  onRetry,
  onOpenOptions,
  onCreateProjectChat,
  onSelectConversation,
  onTogglePinned,
  onRename,
  onInlineRename,
  onDuplicate,
  onExport,
  onArchive,
  onDelete,
  onMoveToProject,
  onMoveToChats,
  onMoveToFolder,
  onDropConversationToProject,
}: ProjectSidebarSectionProps) {
  const { t } = useTranslation();
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [collapsedProjectIds, setCollapsedProjectIds] = useState<ReadonlySet<string>>(() => new Set());
  const emptyMessage =
    status === 'loading' ? (
      <span aria-live="polite" className="flex items-center gap-2" role="status">
        <LoaderCircle aria-hidden="true" className="motion-safe:animate-spin" size={14} />
        {t('projectLoading')}
      </span>
    ) : status === 'ready' && projects.length === 0 ? (
      t('noProjects')
    ) : undefined;

  const toggleProjectExpanded = (projectId: string) => {
    setCollapsedProjectIds((current) => {
      const next = new Set(current);
      if (next.has(projectId)) next.delete(projectId);
      else next.add(projectId);
      return next;
    });
  };

  const handleProjectDrop = (event: DragEvent<HTMLLIElement>, projectId: string) => {
    if (isBusy) return;
    const conversationId = event.dataTransfer.getData('application/x-openchat-conversation');
    if (!conversationId) return;
    event.preventDefault();
    setDropTargetId(null);
    onDropConversationToProject(conversationId, projectId);
  };

  return (
    <div className="grid gap-1">
      <ConversationSidebarSection
        childrenClassName="grid gap-1"
        collapsed={collapsed}
        emptyMessage={status === 'error' ? undefined : emptyMessage}
        icon={Folder}
        onToggle={onToggleCollapsed}
        title={t('projects')}
      >
        {status === 'ready'
          ? projects.map((project) => {
              const projectConversations = conversations.filter(
                (conversation) => conversation.projectId === project.id && !conversation.isPinned,
              );
              const selected = projectConversations.some((conversation) => conversation.id === activeConversationId);
              const isProjectExpanded = !collapsedProjectIds.has(project.id);
              const ProjectFolderIcon = isProjectExpanded && projectConversations.length > 0 ? FolderOpen : Folder;
              return (
                <li
                  className={`project-sidebar-item group grid gap-0.5 rounded-button transition-colors duration-quick ${dropTargetId === project.id ? 'bg-accent/5 ring-1 ring-accent/60' : ''}`}
                  key={project.id}
                  onDragLeave={(event) => {
                    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget))
                      return;
                    setDropTargetId(null);
                  }}
                  onDragOver={(event) => {
                    if (isBusy || !event.dataTransfer.types.includes('application/x-openchat-conversation')) return;
                    event.preventDefault();
                    event.dataTransfer.dropEffect = 'move';
                    setDropTargetId(project.id);
                  }}
                  onDrop={(event) => handleProjectDrop(event, project.id)}
                >
                  <div
                    className={`relative flex min-h-9 items-center rounded-button transition-colors duration-quick ${
                      selected
                        ? 'bg-selected/65 font-medium text-text'
                        : 'text-text/85 hover:bg-hover/85 hover:text-text focus-within:bg-hover/85 focus-within:text-text'
                    }`}
                  >
                    <button
                      aria-expanded={projectConversations.length > 0 ? isProjectExpanded : undefined}
                      className="flex min-h-9 min-w-0 flex-1 items-center gap-2.5 rounded-button px-3 py-1.5 text-left transition-all duration-quick group-focus-within:pr-16 group-hover:pr-16 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      data-sidebar-nav-item="true"
                      onClick={() => {
                        if (projectConversations.length > 0) {
                          toggleProjectExpanded(project.id);
                        } else {
                          onCreateProjectChat(project.id);
                        }
                      }}
                      title={
                        projectConversations.length > 0
                          ? project.name
                          : t('projectNewChatButtonTitle', { name: project.name })
                      }
                      type="button"
                    >
                      <ProjectFolderIcon
                        aria-hidden="true"
                        className={`shrink-0 transition-colors ${selected ? 'text-accent' : 'text-secondary-icon'}`}
                        size={15}
                      />
                      <span className="sidebar-project-title min-w-0 flex-1 truncate font-medium">{project.name}</span>
                      {projectConversations.length > 0 ? (
                        <span className="shrink-0 text-xs text-secondary-text/70 transition-opacity duration-quick group-focus-within:opacity-0 group-hover:opacity-0">
                          {projectConversations.length}
                        </span>
                      ) : null}
                    </button>
                    <div className="project-sidebar-actions pointer-events-none absolute inset-y-0 right-1.5 flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-quick group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100">
                      <Button
                        aria-label={`${t('newChat')}: ${project.name}`}
                        className="size-7 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-text"
                        disabled={isBusy}
                        onClick={() => onCreateProjectChat(project.id)}
                        size="icon"
                        title={t('projectNewChatButtonTitle', { name: project.name })}
                        variant="ghost"
                      >
                        <MessageSquarePlus aria-hidden="true" size={14} />
                      </Button>
                      <Button
                        aria-label={`${t('projectOptions')}: ${project.name}`}
                        className="size-7 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-text"
                        onClick={() => onOpenOptions(project)}
                        size="icon"
                        title={t('projectOptionsButtonTitle', { name: project.name })}
                        variant="ghost"
                      >
                        <MoreHorizontal aria-hidden="true" size={14} />
                      </Button>
                    </div>
                  </div>
                  {projectConversations.length > 0 && isProjectExpanded ? (
                    <ul className="ml-4 grid gap-0.5 border-l border-border pl-2 py-0.5">
                      {projectConversations.map((conversation) => (
                        <ConversationSidebarRow
                          active={activeConversationId === conversation.id}
                          allowMove
                          archived={false}
                          conversation={conversation}
                          currentProjectId={project.id}
                          customFolders={customFolders}
                          disabled={isBusy}
                          isAiWorking={isConversationAiWorking?.(conversation.id) ?? false}
                          key={conversation.id}
                          onArchive={() => onArchive(conversation)}
                          onDelete={() => onDelete(conversation)}
                          onDuplicate={onDuplicate ? () => onDuplicate(conversation) : undefined}
                          onExport={(format) => onExport(conversation, format)}
                          onInlineRename={onInlineRename ? (title) => onInlineRename(conversation, title) : undefined}
                          onMoveToChats={() => onMoveToChats(conversation)}
                          onMoveToFolder={
                            onMoveToFolder ? (folderId) => onMoveToFolder(conversation, folderId) : undefined
                          }
                          onMoveToProject={(projectId) => onMoveToProject(conversation, projectId)}
                          onRename={() => onRename(conversation)}
                          onSelect={() => onSelectConversation(conversation.id)}
                          onTogglePinned={() => onTogglePinned(conversation)}
                          projects={projects}
                          viewMode={viewMode}
                        />
                      ))}
                    </ul>
                  ) : null}
                </li>
              );
            })
          : null}
      </ConversationSidebarSection>
      {!collapsed && status === 'error' ? (
        <div className="flex items-center justify-between gap-2 rounded-control border border-border bg-surface px-2.5 py-1.5">
          <p className="text-xs text-secondary-text" role="alert">
            {t('projectLoadFailed')}
          </p>
          <Button
            aria-label={t('retry')}
            className="size-7 rounded-control"
            onClick={onRetry}
            size="icon"
            variant="ghost"
          >
            <RefreshCw aria-hidden="true" size={14} />
          </Button>
        </div>
      ) : null}
    </div>
  );
}
