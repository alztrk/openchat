import { Brain, ChevronLeft, FolderTree, GitBranch, Hand, NotebookPen, Search, Server, Sparkles, Star } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { ProjectDefaultModelPanel } from './ProjectDefaultModelPanel';
import { ProjectIndexPanel } from './ProjectIndexPanel';
import { ProjectInstructionsPanel } from './ProjectInstructionsPanel';
import { ProjectMemoryPanel } from './ProjectMemoryPanel';
import { ProjectMcpPanel } from './ProjectMcpPanel';
import { ProjectSkillsPanel } from './ProjectSkillsPanel';
import { ProjectToolPermissionsPanel } from './ProjectToolPermissionsPanel';
import { ProjectWorktreesPanel } from './ProjectWorktreesPanel';
import { ProjectWorkspaceRootsPanel } from './ProjectWorkspaceRootsPanel';

type ProjectOptionId =
  | 'workspaceRoots'
  | 'defaultModel'
  | 'instructions'
  | 'memory'
  | 'skills'
  | 'index'
  | 'permissions'
  | 'mcp'
  | 'worktrees';

interface ProjectOptionsDialogProps {
  project: ChatProject | null;
  open: boolean;
  onOpenChange(open: boolean): void;
  onUseWorktree(path: string, branch: string): Promise<void>;
}

export function ProjectOptionsDialog({ project, open, onOpenChange, onUseWorktree }: ProjectOptionsDialogProps) {
  const { t } = useTranslation();
  const [option, setOption] = useState<ProjectOptionId | null>(null);

  useEffect(() => {
    if (!open) setOption(null);
  }, [open]);

  const optionLabels: Record<ProjectOptionId, string> = {
    workspaceRoots: t('projectWorkspaceRootsTitle'),
    defaultModel: t('projectDefaultModelUseCurrent'),
    instructions: t('projectOptionsInstructions'),
    memory: t('projectMemoryTitle'),
    skills: t('projectOptionsSkills'),
    index: t('projectIndexTitle'),
    permissions: t('projectOptionsToolPermissions'),
    mcp: t('projectOptionsMcpServers'),
    worktrees: t('projectOptionsWorktrees'),
  };

  return (
    <Dialog
      className="max-h-dialog-viewport overflow-y-auto"
      onOpenChange={onOpenChange}
      open={open && project !== null}
      title={option ? optionLabels[option] : t('projectOptionsTitle')}
    >
      {project ? (
        <div className="grid gap-3">
          {option ? (
            <Button
              aria-label={t('projectOptionsBack')}
              className="justify-self-start"
              onClick={() => setOption(null)}
              size="small"
              variant="ghost"
            >
              <ChevronLeft aria-hidden="true" size={15} />
              {t('projectOptionsBack')}
            </Button>
          ) : null}
          {option === null ? (
            <div className="grid gap-1">
              <ProjectOptionButton
                icon={FolderTree}
                label={optionLabels.workspaceRoots}
                onClick={() => setOption('workspaceRoots')}
              />
              <ProjectOptionButton
                icon={Star}
                label={optionLabels.defaultModel}
                onClick={() => setOption('defaultModel')}
              />
              <ProjectOptionButton
                icon={NotebookPen}
                label={optionLabels.instructions}
                onClick={() => setOption('instructions')}
              />
              <ProjectOptionButton icon={Brain} label={optionLabels.memory} onClick={() => setOption('memory')} />
              <ProjectOptionButton icon={Sparkles} label={optionLabels.skills} onClick={() => setOption('skills')} />
              <ProjectOptionButton icon={Search} label={optionLabels.index} onClick={() => setOption('index')} />
              <ProjectOptionButton
                icon={Hand}
                label={optionLabels.permissions}
                onClick={() => setOption('permissions')}
              />
              <ProjectOptionButton icon={Server} label={optionLabels.mcp} onClick={() => setOption('mcp')} />
              <ProjectOptionButton
                icon={GitBranch}
                label={optionLabels.worktrees}
                onClick={() => setOption('worktrees')}
              />
            </div>
          ) : null}
          {option === 'workspaceRoots' ? <ProjectWorkspaceRootsPanel project={project} /> : null}
          {option === 'defaultModel' ? (
            <ProjectDefaultModelPanel onClose={() => onOpenChange(false)} project={project} />
          ) : null}
          {option === 'instructions' ? (
            <ProjectInstructionsPanel onClose={() => onOpenChange(false)} project={project} />
          ) : null}
          {option === 'memory' ? <ProjectMemoryPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'skills' ? <ProjectSkillsPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'index' ? <ProjectIndexPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'permissions' ? (
            <ProjectToolPermissionsPanel onClose={() => onOpenChange(false)} project={project} />
          ) : null}
          {option === 'mcp' ? <ProjectMcpPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'worktrees' ? (
            <ProjectWorktreesPanel
              onClose={() => onOpenChange(false)}
              onUseWorktree={onUseWorktree}
              project={project}
            />
          ) : null}
        </div>
      ) : null}
    </Dialog>
  );
}

interface ProjectOptionButtonProps {
  icon: typeof Star;
  label: string;
  onClick(): void;
}

function ProjectOptionButton({ icon: Icon, label, onClick }: ProjectOptionButtonProps) {
  return (
    <Button
      className="min-h-11 w-full justify-start gap-3 px-3 text-left text-sm text-text"
      onClick={onClick}
      size="small"
      type="button"
      variant="ghost"
    >
      <Icon aria-hidden="true" className="text-secondary-icon" size={17} />
      <span className="min-w-0 flex-1">{label}</span>
    </Button>
  );
}
