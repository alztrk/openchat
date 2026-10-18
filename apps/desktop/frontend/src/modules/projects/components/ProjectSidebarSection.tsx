import { Folder, FolderPlus, LoaderCircle, MessageSquarePlus, MoreHorizontal, RefreshCw } from 'lucide-react';
import { type DragEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { ConversationSidebarRow } from '@/modules/conversations/components/ConversationSidebarRow';
import { ConversationSidebarSection } from '@/modules/conversations/components/ConversationSidebarSection';
import type { ChatConversation, ChatProject } from '@/modules/conversations/data/chatStorage';

interface ProjectSidebarSectionProps {
  projects: readonly ChatProject[];
  conversations: readonly ChatConversation[];
  status: 'idle' | 'loading' | 'ready' | 'error';
  collapsed: boolean;
  activeConversationId: string | null;
  selectedConversationIds: ReadonlySet<string>;
  selectionMode: boolean;
  isBusy: boolean;
  onToggleCollapsed(): void;
  onRetry(): void;
  onCreateProject(): void;
  onOpenOptions(project: ChatProject): void;
  onCreateProjectChat(projectId: string): void;
  onSelectConversation(id: string): void;
  onToggleSelection(id: string): void;
  onTogglePinned(conversation: ChatConversation): void;
  onToggleBookmark(conversation: ChatConversation): void;
  onEditTags(conversation: ChatConversation): void;
  onRename(conversation: ChatConversation): void;
  onExport(conversation: ChatConversation): void;
  onArchive(conversation: ChatConversation): void;
  onDelete(conversation: ChatConversation): void;
  onMoveToProject(conversation: ChatConversation, projectId: string): void;
  onMoveToChats(conversation: ChatConversation): void;
  onDropConversationToProject(conversationId: string, projectId: string): void;
}

export function ProjectSidebarSection({
  projects,
  conversations,
  status,
  collapsed,
  activeConversationId,
  selectedConversationIds,
  selectionMode,
  isBusy,
  onToggleCollapsed,
  onRetry,
  onCreateProject,
  onOpenOptions,
  onCreateProjectChat,
  onSelectConversation,
  onToggleSelection,
  onTogglePinned,
  onToggleBookmark,
  onEditTags,
  onRename,
  onExport,
  onArchive,
  onDelete,
  onMoveToProject,
  onMoveToChats,
  onDropConversationToProject,
}: ProjectSidebarSectionProps) {
  const { t } = useTranslation();
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const emptyMessage =
    status === 'loading' ? (
      <span aria-live="polite" className="flex items-center gap-2" role="status">
        <LoaderCircle aria-hidden="true" className="motion-safe:animate-spin" size={14} />
        {t('projectLoading')}
      </span>
    ) : status === 'ready' && projects.length === 0 ? (
      t('noProjects')
    ) : undefined;

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
        action={
          <Button
            aria-label={t('createProject')}
            className="size-8"
            disabled={isBusy}
            onClick={onCreateProject}
            size="icon"
            variant="ghost"
          >
            <FolderPlus aria-hidden="true" size={16} />
          </Button>
        }
        childrenClassName="grid gap-3"
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
              return (
                <li
                  className={`project-sidebar-item group grid gap-1 rounded-card ${dropTargetId === project.id ? 'ring-2 ring-accent' : ''}`}
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
                    className={`flex min-h-11 items-center gap-1 rounded-button px-2 ${selected ? 'bg-selected text-text' : 'text-secondary-text hover:bg-hover hover:text-text'}`}
                  >
                    <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
                    <span className="sidebar-project-title min-w-0 flex-1 truncate">{project.name}</span>
                    <div className="project-sidebar-actions pointer-events-none flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-quick group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100">
                      <Button
                        aria-label={`${t('projectOptions')}: ${project.name}`}
                        className="size-10"
                        onClick={() => onOpenOptions(project)}
                        size="icon"
                        title={t('projectOptions')}
                        variant="ghost"
                      >
                        <MoreHorizontal aria-hidden="true" size={18} />
                      </Button>
                      <Button
                        aria-label={`${t('newChat')}: ${project.name}`}
                        className="size-10"
                        disabled={isBusy}
                        onClick={() => onCreateProjectChat(project.id)}
                        size="icon"
                        title={t('newChat')}
                        variant="ghost"
                      >
                        <MessageSquarePlus aria-hidden="true" size={18} />
                      </Button>
                    </div>
                  </div>
                  {projectConversations.length > 0 ? (
                    <ul className="ml-5 grid gap-0.5">
                      {projectConversations.map((conversation) => (
                        <ConversationSidebarRow
                          active={activeConversationId === conversation.id}
                          allowMove
                          allowSelection
                          archived={false}
                          conversation={conversation}
                          currentProjectId={project.id}
                          disabled={isBusy}
                          key={conversation.id}
                          onArchive={() => onArchive(conversation)}
                          onDelete={() => onDelete(conversation)}
                          onEditTags={() => onEditTags(conversation)}
                          onExport={() => onExport(conversation)}
                          onMoveToChats={() => onMoveToChats(conversation)}
                          onMoveToProject={(projectId) => onMoveToProject(conversation, projectId)}
                          onRename={() => onRename(conversation)}
                          onSelect={() => onSelectConversation(conversation.id)}
                          onToggleBookmark={() => onToggleBookmark(conversation)}
                          onTogglePinned={() => onTogglePinned(conversation)}
                          onToggleSelection={() => onToggleSelection(conversation.id)}
                          projects={projects}
                          selectedForBatch={selectedConversationIds.has(conversation.id)}
                          selectionMode={selectionMode}
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
        <div className="flex items-center justify-between gap-2 px-3 py-1">
          <p className="text-xs text-secondary-text" role="alert">
            {t('projectLoadFailed')}
          </p>
          <Button aria-label={t('retry')} className="size-8" onClick={onRetry} size="icon" variant="ghost">
            <RefreshCw aria-hidden="true" size={15} />
          </Button>
        </div>
      ) : null}
    </div>
  );
}
