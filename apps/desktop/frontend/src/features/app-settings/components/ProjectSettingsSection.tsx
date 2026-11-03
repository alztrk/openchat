import { FolderKanban, Plus } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@shared/components';
import { ProjectWorkspaceRootsPanel } from '@app/runtime/projectTools';
import { CreateProjectDialog } from '@app/runtime/projectTools';
import { createProject as createProjectStorage } from '@app/runtime/chatStorage';
import { useChatStore } from '@app/runtime/chatStore';
import { SettingsSectionSkeleton } from './SettingsSectionSkeleton';

export function ProjectSettingsSection() {
  const { t } = useTranslation();
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const refreshProjects = useChatStore((state) => state.refreshProjects);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [createFailed, setCreateFailed] = useState(false);
  const selectedProject = projects.find((project) => project.id === selectedProjectId) ?? projects[0] ?? null;

  const createProject = async (name: string, folderSelectionId: string) => {
    const projectId = crypto.randomUUID().replaceAll('-', '');
    try {
      await createProjectStorage(projectId, name, folderSelectionId);
      await refreshProjects();
      setSelectedProjectId(projectId);
      setCreateFailed(false);
      setIsCreateOpen(false);
      return projectId;
    } catch {
      setCreateFailed(true);
      throw new Error('The project could not be created.');
    }
  };

  return (
    <div className="grid gap-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-base font-semibold text-text">{t('projects')}</h2>
        <Button onClick={() => setIsCreateOpen(true)} size="small" variant="secondary">
          <Plus aria-hidden="true" size={16} />
          {t('createProject')}
        </Button>
      </div>
      {projectsStatus === 'idle' || projectsStatus === 'loading' ? (
        <SettingsSectionSkeleton label={t('projectLoading')} />
      ) : projectsStatus === 'error' ? (
        <Card className="flex flex-wrap items-center justify-between gap-3 bg-raised" padding="compact" role="alert">
          <p className="text-sm text-destructive">{t('projectLoadFailed')}</p>
          <Button onClick={() => void refreshProjects()} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </Card>
      ) : projects.length === 0 ? (
        <Card className="grid justify-items-center gap-2 bg-raised py-8 text-center" padding="default">
          <FolderKanban aria-hidden="true" className="text-secondary-icon" size={22} />
          <p className="text-sm text-secondary-text">{t('noProjects')}</p>
        </Card>
      ) : (
        <div className="grid min-w-0 gap-3 lg:grid-cols-3">
          <nav
            aria-label={t('projects')}
            className="grid content-start gap-1.5 rounded-card border border-border bg-raised p-2 lg:col-span-1"
          >
            {projects.map((project) => (
              <Button
                aria-current={selectedProject?.id === project.id ? 'page' : undefined}
                className={`min-h-10 min-w-0 justify-start gap-2.5 rounded-control border px-3 text-left ${
                  selectedProject?.id === project.id
                    ? 'border-border bg-surface text-text'
                    : 'border-transparent bg-transparent hover:border-border hover:bg-surface'
                }`}
                key={project.id}
                onClick={() => setSelectedProjectId(project.id)}
                size="small"
                variant="ghost"
              >
                <FolderKanban aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
                <span className="truncate">{project.name}</span>
              </Button>
            ))}
          </nav>
          {selectedProject ? (
            <section
              aria-label={selectedProject.name}
              className="min-w-0 rounded-card border border-border bg-raised p-3 sm:p-4 lg:col-span-2"
            >
              <h3 className="mb-3 text-sm font-semibold text-text">{selectedProject.name}</h3>
              <ProjectWorkspaceRootsPanel project={selectedProject} />
            </section>
          ) : null}
        </div>
      )}
      {createFailed ? <p className="text-sm text-destructive" role="alert">{t('projectCreateFailed')}</p> : null}
      <CreateProjectDialog onCreate={createProject} onOpenChange={setIsCreateOpen} open={isCreateOpen} />
    </div>
  );
}
