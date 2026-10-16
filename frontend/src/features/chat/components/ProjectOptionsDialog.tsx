import { useEffect, useState } from 'react';
import {
  ChevronLeft,
  Hand,
  NotebookPen,
  Server,
  Search,
  Sparkles,
  Star,
  GitBranch,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/shared/ui';
import type { ChatProject } from '@/features/chat/data/chatStorage';
import { ProjectDefaultModelPanel } from './ProjectDefaultModelPanel';
import { ProjectIndexPanel } from './ProjectIndexPanel';
import { ProjectInstructionsPanel } from './ProjectInstructionsPanel';
import { ProjectSkillsPanel } from './ProjectSkillsPanel';
import { ProjectToolPermissionsPanel } from './ProjectToolPermissionsPanel';
import { ProjectMcpPanel } from './ProjectMcpPanel';
import { ProjectWorktreesPanel } from './ProjectWorktreesPanel';

type ProjectOptionId = 'defaultModel' | 'instructions' | 'skills' | 'index' | 'permissions' | 'mcp' | 'worktrees';

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
    defaultModel: t('projectDefaultModelUseCurrent'),
    instructions: t('projectOptionsInstructions'),
    skills: t('projectOptionsSkills'),
    index: t('projectIndexTitle'),
    permissions: t('projectOptionsToolPermissions'),
    mcp: t('projectOptionsMcpServers'),
    worktrees: t('projectOptionsWorktrees'),
  };

  return (
    <Dialog
      className="max-h-[calc(100vh-2rem)] overflow-y-auto"
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
              <ChevronLeft aria-hidden="true" size={15} />{t('projectOptionsBack')}
            </Button>
          ) : null}
          {option === null ? (
            <div className="grid gap-1">
              <ProjectOptionButton icon={Star} label={optionLabels.defaultModel} onClick={() => setOption('defaultModel')} />
              <ProjectOptionButton icon={NotebookPen} label={optionLabels.instructions} onClick={() => setOption('instructions')} />
              <ProjectOptionButton icon={Sparkles} label={optionLabels.skills} onClick={() => setOption('skills')} />
              <ProjectOptionButton icon={Search} label={optionLabels.index} onClick={() => setOption('index')} />
              <ProjectOptionButton icon={Hand} label={optionLabels.permissions} onClick={() => setOption('permissions')} />
              <ProjectOptionButton icon={Server} label={optionLabels.mcp} onClick={() => setOption('mcp')} />
              <ProjectOptionButton icon={GitBranch} label={optionLabels.worktrees} onClick={() => setOption('worktrees')} />
            </div>
          ) : null}
          {option === 'defaultModel' ? <ProjectDefaultModelPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'instructions' ? <ProjectInstructionsPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'skills' ? <ProjectSkillsPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'index' ? <ProjectIndexPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'permissions' ? <ProjectToolPermissionsPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'mcp' ? <ProjectMcpPanel onClose={() => onOpenChange(false)} project={project} /> : null}
          {option === 'worktrees' ? <ProjectWorktreesPanel onClose={() => onOpenChange(false)} onUseWorktree={onUseWorktree} project={project} /> : null}
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
    <button
      className="flex min-h-11 items-center gap-3 rounded-button px-3 text-left text-sm text-text hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      onClick={onClick}
      type="button"
    >
      <Icon aria-hidden="true" className="text-secondary-icon" size={17} />
      <span className="min-w-0 flex-1">{label}</span>
    </button>
  );
}
