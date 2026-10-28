import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { readProjectSkillIds, writeProjectSkillIds } from '@/modules/projects/data/projectPreferences';
import { localServiceClient } from '@/platform/local-service';

interface ProjectSkill {
  id: string;
  content: string;
}

interface ProjectSkillsPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectSkillsPanel({ project, onClose }: ProjectSkillsPanelProps) {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<ProjectSkill[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);

  useEffect(() => {
    let current = true;
    void loadProjectSkills(project.id)
      .then(({ available, selected }) => {
        if (!current) return;
        setSkills(available);
        setSelectedIds(new Set(selected));
        setLoadFailed(false);
      })
      .catch(() => {
        if (current) setLoadFailed(true);
      })
      .finally(() => {
        if (current) setIsLoading(false);
      });
    return () => {
      current = false;
    };
  }, [project.id]);

  const reload = async () => {
    setIsLoading(true);
    try {
      const loaded = await loadProjectSkills(project.id);
      setSkills(loaded.available);
      setSelectedIds(new Set(loaded.selected));
      setLoadFailed(false);
    } catch {
      setLoadFailed(true);
    } finally {
      setIsLoading(false);
    }
  };

  const toggleSkill = (id: string, selected: boolean) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (selected) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const save = async () => {
    if (isSaving || isLoading || loadFailed) return;
    setIsSaving(true);
    setSaveFailed(false);
    try {
      writeProjectSkillIds(project.id, [...selectedIds]);
      onClose();
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const clearSelection = () => {
    try {
      writeProjectSkillIds(project.id, []);
      onClose();
    } catch {
      setSaveFailed(true);
    }
  };

  const availableIds = new Set(skills.map((skill) => skill.id));
  const staleIds = [...selectedIds].filter((id) => !availableIds.has(id)).sort();

  return (
    <div className="grid gap-3">
      {isLoading ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('historyLoading')}
        </p>
      ) : loadFailed ? (
        <div className="grid gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t('projectSkillsLoadFailed')}
          </p>
          <Button className="justify-self-start" disabled={isLoading} onClick={() => void reload()} size="small">
            {t('retry')}
          </Button>
          <Button disabled={isSaving} onClick={clearSelection} variant="secondary">
            {t('projectSkillsClearSelection')}
          </Button>
        </div>
      ) : (
        <div className="openchat-scroll-area grid max-h-project-skills-list gap-2 overflow-y-auto">
          <p className="text-sm text-secondary-text">{t('projectSkillsDescription')}</p>
          {skills.map((skill) => (
            <details className="openchat-native-disclosure rounded-card border border-border p-3" key={skill.id}>
              <summary className="flex cursor-pointer list-none items-center gap-3 [&::-webkit-details-marker]:hidden">
                <Checkbox
                  aria-label={skill.id}
                  checked={selectedIds.has(skill.id)}
                  disabled={isSaving}
                  onChange={(event) => toggleSkill(skill.id, event.target.checked)}
                  onClick={(event) => event.stopPropagation()}
                />
                <span className="font-code text-sm font-medium text-text">{skill.id}</span>
              </summary>
              <pre className="mt-3 whitespace-pre-wrap break-words font-ui text-sm text-secondary-text">
                {skill.content}
              </pre>
            </details>
          ))}
          {staleIds.map((id) => (
            <div className="flex items-start gap-3 rounded-card border border-warning p-3 text-sm" key={id}>
              <Checkbox
                aria-label={`${id}: ${t('projectSkillsStale')}`}
                checked
                disabled={isSaving}
                onChange={(event) => toggleSkill(id, event.target.checked)}
              />
              <span>
                <span className="font-code text-text">{id}</span>
                <span className="mt-1 block text-secondary-text">{t('projectSkillsStale')}</span>
              </span>
            </div>
          ))}
          {skills.length === 0 && staleIds.length === 0 ? (
            <p className="text-sm text-secondary-text">{t('projectSkillsEmpty')}</p>
          ) : null}
        </div>
      )}
      {saveFailed ? (
        <p className="text-sm text-destructive" role="alert">
          {t('projectSkillsSaveFailed')}
        </p>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button disabled={isSaving} onClick={onClose}>
          {t('cancel')}
        </Button>
        <Button disabled={isLoading || isSaving || loadFailed} onClick={() => void save()} variant="primary">
          {isSaving ? t('saving') : t('projectInstructionsSave')}
        </Button>
      </div>
    </div>
  );
}

async function loadProjectSkills(projectId: string): Promise<{ available: ProjectSkill[]; selected: string[] }> {
  const response = await localServiceClient.call('project.skills.list', { projectId });
  const rawSkills = response.skills;
  if (!Array.isArray(rawSkills) || rawSkills.length > 32) throw new Error('The project Skills were invalid.');
  const available = rawSkills.map(parseProjectSkill);
  if (new Set(available.map((skill) => skill.id)).size !== available.length) {
    throw new Error('Project Skill IDs were not unique.');
  }
  return { available, selected: readProjectSkillIds(projectId) };
}

function parseProjectSkill(value: unknown): ProjectSkill {
  if (
    !isObject(value) ||
    typeof value.id !== 'string' ||
    !/^[a-z][a-z0-9-]{0,63}$/.test(value.id) ||
    typeof value.content !== 'string' ||
    value.content.length === 0 ||
    value.content.length > 16 * 1024
  ) {
    throw new Error('A project Skill was invalid.');
  }
  return { id: value.id, content: value.content };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
