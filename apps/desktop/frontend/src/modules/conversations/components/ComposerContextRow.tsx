import { Bot, Check, ChevronDown, Folder, FolderMinus, FolderPlus, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  Input,
} from '@/design-system';
import type { PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import { useConversationSidebarActions } from '@/modules/conversations/hooks/useConversationSidebarActions';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { CreateProjectDialog } from '@/modules/projects/components/CreateProjectDialog';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { formatModelRouteKey, modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { useAssistantProfilesStore } from '@/modules/settings/stores/assistantProfilesStore';
import { ContextUsageIndicator } from './ContextUsageIndicator';

export interface ComposerContextRowProps {
  isTemporaryChat: boolean;
  activeConversationId: string | null;
  isSending: boolean;
  projects: Array<{ id: string; name: string }>;
  projectsStatus: 'idle' | 'loading' | 'ready' | 'error';
  contextProjectId: string | null;
  onSelectProject(projectId: string | null): void;
  selectedModel: ChatModel | undefined;
  contextToolPermissionMode: ToolPermissionMode | 'plan' | null;
  contextWindow: number | null;
  messageDraft: string;
  messages: ChatMessage[];
  pendingAttachments: PendingChatAttachment[];
}

export function ComposerContextRow({
  isTemporaryChat,
  activeConversationId,
  isSending,
  projects,
  projectsStatus,
  contextProjectId,
  onSelectProject,
  selectedModel,
  contextToolPermissionMode,
  contextWindow,
  messageDraft,
  messages,
  pendingAttachments,
}: ComposerContextRowProps) {
  const { t, i18n } = useTranslation();
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [projectSearch, setProjectSearch] = useState('');
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const projectSearchContainerRef = useRef<HTMLDivElement>(null);
  const { createProject } = useConversationSidebarActions();
  const profiles = useAssistantProfilesStore((state) => state.profiles);
  const activeProfileId = useAssistantProfilesStore((state) => state.activeProfileId);
  const initializeProfiles = useAssistantProfilesStore((state) => state.initialize);
  const selectProfile = useAssistantProfilesStore((state) => state.selectProfile);
  const activeProfile = profiles.find((profile) => profile.id === activeProfileId) ?? null;
  const selectModel = useChatStore((state) => state.selectModel);
  const selectedProject = projects.find((project) => project.id === contextProjectId);
  const filteredProjects = useMemo(() => {
    const query = projectSearch.trim().toLocaleLowerCase(i18n.resolvedLanguage ?? i18n.language);
    if (!query) return projects;
    return projects.filter((project) =>
      project.name.toLocaleLowerCase(i18n.resolvedLanguage ?? i18n.language).includes(query),
    );
  }, [i18n.language, i18n.resolvedLanguage, projectSearch, projects]);

  const canChooseProject =
    !isTemporaryChat && activeConversationId === null && !isSending && projectsStatus === 'ready';

  useEffect(() => {
    if (!projectMenuOpen) return;
    const frame = window.requestAnimationFrame(() => {
      projectSearchContainerRef.current?.querySelector<HTMLInputElement>('input[aria-label]')?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [projectMenuOpen]);

  useEffect(() => initializeProfiles(), [initializeProfiles]);

  const chooseProfile = (profileId: string | null) => {
    if (!selectProfile(profileId)) return;
    if (activeConversationId !== null || contextProjectId !== null || profileId === null) return;
    const profile = profiles.find((item) => item.id === profileId);
    if (!profile?.preferredModel) return;
    const preferredModelPreference = profile.preferredModel;
    const preferredModel = useChatStore
      .getState()
      .models.find((model) => modelRouteKey(model) === formatModelRouteKey(preferredModelPreference));
    if (preferredModel) selectModel(preferredModel.key);
  };

  return (
    <div className="composer-context-row flex min-w-0 items-center justify-between gap-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
        <DropdownMenu
          onOpenChange={(open) => {
            setProjectMenuOpen(open);
            if (!open) setProjectSearch('');
          }}
          open={projectMenuOpen}
        >
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={t('selectProject')}
              className="openchat-dropdown-trigger openchat-dropdown-trigger--subtle openchat-dropdown-trigger--compact max-w-64 min-w-0 justify-start text-xs"
              disabled={!canChooseProject}
              title={selectedProject?.name ?? t('noProjectSelected')}
              variant="ghost"
            >
              <span className="min-w-0 flex-1 truncate text-left">
                {selectedProject?.name ?? t('noProjectSelected')}
              </span>
              <ChevronDown aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="composer-project-menu p-1.5" side="top" sideOffset={8}>
            <div className="relative px-1 pb-1.5" ref={projectSearchContainerRef}>
              <Search
                aria-hidden="true"
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-secondary-icon"
                size={14}
              />
              <Input
                aria-label={t('searchProjects')}
                className="min-h-9 border-0 bg-transparent py-1 pl-8 pr-2 text-sm focus-visible:ring-0"
                onChange={(event) => setProjectSearch(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key !== 'Escape') event.stopPropagation();
                }}
                placeholder={t('searchProjects')}
                value={projectSearch}
              />
            </div>
            <div className="composer-project-list">
              {filteredProjects.length > 0 ? (
                filteredProjects.map((project) => {
                  const isSelected = project.id === contextProjectId;
                  return (
                    <DropdownMenuItem
                      asChild
                      className={`w-full min-h-8 gap-2 px-2 py-1 text-left text-xs ${isSelected ? 'bg-selected text-text' : ''}`}
                      key={project.id}
                      onSelect={() => onSelectProject(project.id)}
                      textValue={project.name}
                    >
                      <button aria-current={isSelected ? 'true' : undefined} type="button">
                        <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                        <span className="min-w-0 flex-1 truncate">{project.name}</span>
                        {isSelected ? <Check aria-hidden="true" className="shrink-0 text-accent" size={14} /> : null}
                      </button>
                    </DropdownMenuItem>
                  );
                })
              ) : (
                <p className="px-2 py-3 text-xs text-secondary-text" role="status">
                  {projects.length === 0
                    ? t('noProjects')
                    : projectSearch.trim()
                      ? t('noMatchingProjects')
                      : t('noProjects')}
                </p>
              )}
            </div>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              asChild
              className="w-full min-h-8 gap-2 px-2 py-1 text-left text-xs"
              onSelect={() => setCreateProjectOpen(true)}
            >
              <button type="button">
                <FolderPlus aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                <span>{t('createProject')}</span>
              </button>
            </DropdownMenuItem>
            <DropdownMenuItem
              asChild
              className="w-full min-h-8 gap-2 px-2 py-1 text-left text-xs"
              disabled={contextProjectId === null}
              onSelect={() => onSelectProject(null)}
            >
              <button disabled={contextProjectId === null} type="button">
                <FolderMinus aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                <span>{t('workWithoutProject')}</span>
              </button>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <DropdownMenu onOpenChange={setProfileMenuOpen} open={profileMenuOpen}>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={t('assistantProfiles')}
              className="openchat-dropdown-trigger openchat-dropdown-trigger--subtle openchat-dropdown-trigger--compact max-w-52 min-w-0 justify-start text-xs"
              disabled={isTemporaryChat || isSending}
              title={activeProfile?.name ?? t('assistantProfiles')}
              variant="ghost"
            >
              <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
              <span className="min-w-0 truncate text-left">{activeProfile?.name ?? t('assistantProfileDeactivate')}</span>
              <ChevronDown aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-56 p-1.5" side="top" sideOffset={8}>
            <DropdownMenuItem
              asChild
              className={`w-full min-h-8 gap-2 px-2 py-1 text-left text-xs ${activeProfileId === null ? 'bg-selected text-text' : ''}`}
              onSelect={() => chooseProfile(null)}
              textValue={t('assistantProfileDeactivate')}
            >
              <button aria-current={activeProfileId === null ? 'true' : undefined} type="button">
                <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                <span className="min-w-0 flex-1 truncate">{t('assistantProfileDeactivate')}</span>
                {activeProfileId === null ? <Check aria-hidden="true" className="shrink-0 text-accent" size={14} /> : null}
              </button>
            </DropdownMenuItem>
            {profiles.map((profile) => {
              const isSelected = profile.id === activeProfileId;
              return (
                <DropdownMenuItem
                  asChild
                  className={`w-full min-h-8 gap-2 px-2 py-1 text-left text-xs ${isSelected ? 'bg-selected text-text' : ''}`}
                  key={profile.id}
                  onSelect={() => chooseProfile(profile.id)}
                  textValue={profile.name}
                >
                  <button aria-current={isSelected ? 'true' : undefined} type="button">
                    <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                    <span className="min-w-0 flex-1 truncate">{profile.name}</span>
                    {isSelected ? <Check aria-hidden="true" className="shrink-0 text-accent" size={14} /> : null}
                  </button>
                </DropdownMenuItem>
              );
            })}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      {selectedModel && contextToolPermissionMode !== null ? (
        <ContextUsageIndicator
          contextWindow={contextWindow}
          conversationId={activeConversationId}
          isTemporaryChat={isTemporaryChat}
          draft={messageDraft}
          isSending={isSending}
          messages={messages}
          model={selectedModel}
          pendingAttachments={pendingAttachments}
          projectId={contextProjectId}
          toolPermissionMode={contextToolPermissionMode}
        />
      ) : selectedModel ? (
        <p className="max-w-48 text-xs text-destructive" role="alert">
          {t('toolPermissionSettingsLoadFailed')}
        </p>
      ) : null}
      <CreateProjectDialog
        onCreate={async (name, folderSelectionId) => {
          const projectId = await createProject(name, folderSelectionId);
          onSelectProject(projectId);
        }}
        onOpenChange={setCreateProjectOpen}
        open={createProjectOpen}
      />
    </div>
  );
}
