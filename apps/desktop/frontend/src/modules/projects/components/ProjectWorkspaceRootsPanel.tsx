import { Folder, FolderPlus, LoaderCircle, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import {
  addProjectWorkspaceRoot,
  type ChatProject,
  removeProjectWorkspaceRoot,
} from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { discardProjectFolderSelection, selectProjectFolder } from '@/platform/tauri';

interface ProjectWorkspaceRootsPanelProps {
  project: ChatProject;
}

type ErrorKey = 'projectRootSelectionFailed' | 'projectRootAddFailed' | 'projectRootRemoveFailed';

export function ProjectWorkspaceRootsPanel({ project }: ProjectWorkspaceRootsPanelProps) {
  const { t } = useTranslation();
  const [roots, setRoots] = useState(project.workspaceRoots);
  const [isAdding, setIsAdding] = useState(false);
  const [removingPath, setRemovingPath] = useState<string | null>(null);
  const [error, setError] = useState<ErrorKey | null>(null);

  useEffect(() => {
    setRoots(project.workspaceRoots);
  }, [project.workspaceRoots]);

  const addRoot = async () => {
    if (isAdding || roots.length >= 8) return;
    setIsAdding(true);
    setError(null);
    let selectionId: string | null = null;
    try {
      const selection = await selectProjectFolder({ title: t('projectWorkspaceRootChooseFolder') });
      if (!selection) return;
      selectionId = selection.selectionId;
      const canonicalRootPath = await addProjectWorkspaceRoot(project.id, selectionId);
      selectionId = null;
      const updatedRoots = [...roots, canonicalRootPath];
      setRoots(updatedRoots);
      useChatStore.setState((state) => ({
        projects: state.projects.map((item) =>
          item.id === project.id ? { ...item, workspaceRoots: updatedRoots } : item,
        ),
      }));
    } catch {
      setError(selectionId ? 'projectRootAddFailed' : 'projectRootSelectionFailed');
      if (selectionId) {
        try {
          await discardProjectFolderSelection(selectionId);
        } catch (cleanupError) {
          globalThis.reportError(
            new Error('The unused workspace folder selection could not be released.', {
              cause: cleanupError,
            }),
          );
        }
      }
    } finally {
      setIsAdding(false);
    }
  };

  const removeRoot = async (rootPath: string) => {
    if (removingPath !== null || roots[0] === rootPath) return;
    setRemovingPath(rootPath);
    setError(null);
    try {
      await removeProjectWorkspaceRoot(project.id, rootPath);
      const updatedRoots = roots.filter((path) => path !== rootPath);
      setRoots(updatedRoots);
      useChatStore.setState((state) => ({
        projects: state.projects.map((item) =>
          item.id === project.id ? { ...item, workspaceRoots: updatedRoots } : item,
        ),
      }));
    } catch {
      setError('projectRootRemoveFailed');
    } finally {
      setRemovingPath(null);
    }
  };

  return (
    <section className="grid gap-3" aria-labelledby="project-workspace-roots-heading">
      <div>
        <h3 id="project-workspace-roots-heading" className="text-sm font-semibold text-text">
          {t('projectWorkspaceRootsTitle')}
        </h3>
        <p className="mt-1 text-xs text-secondary-text">{t('projectWorkspaceRootsDescription')}</p>
      </div>
      <ul className="grid gap-2">
        {roots.map((rootPath, index) => (
          <li
            className="flex min-w-0 items-center gap-2 rounded-control border border-border bg-surface px-3 py-2"
            key={rootPath}
          >
            <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
            <span className="min-w-0 flex-1 truncate font-code text-xs text-text" title={rootPath}>
              {rootPath}
            </span>
            {index === 0 ? (
              <span className="shrink-0 text-xs text-secondary-text">{t('projectWorkspaceRootPrimary')}</span>
            ) : (
              <Button
                aria-label={t('projectWorkspaceRootRemove')}
                disabled={removingPath !== null || isAdding}
                onClick={() => void removeRoot(rootPath)}
                size="small"
                variant="ghost"
              >
                {removingPath === rootPath ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={14} />
                ) : (
                  <Trash2 aria-hidden="true" size={14} />
                )}
              </Button>
            )}
          </li>
        ))}
      </ul>
      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {t(error)}
        </p>
      ) : null}
      <Button
        className="justify-self-start"
        disabled={isAdding || removingPath !== null || roots.length >= 8}
        onClick={() => void addRoot()}
        size="small"
        variant="secondary"
      >
        {isAdding ? (
          <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={15} />
        ) : (
          <FolderPlus aria-hidden="true" size={15} />
        )}
        {isAdding ? t('projectWorkspaceRootAdding') : t('projectWorkspaceRootAdd')}
      </Button>
    </section>
  );
}
