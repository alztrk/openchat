import { Download, Trash2 } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Switch, Textarea } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';

const maximumItemBytes = 4 * 1024;
const maximumTotalBytes = 8 * 1024;
const maximumItems = 20;

interface ProjectMemoryItem {
  id: string;
  content: string;
  origin: 'user' | 'assistant';
}

interface ProjectMemoryState {
  enabled: boolean;
  assistantSavesEnabled: boolean;
  historyContextEnabled: boolean;
  items: ProjectMemoryItem[];
  totalBytes: number;
}

type ErrorKey =
  | 'projectMemoryLoadFailed'
  | 'projectMemoryActionFailed'
  | 'projectMemoryExportFailed'
  | 'projectMemoryItemLimit'
  | 'projectMemorySizeLimit'
  | 'projectMemoryItemSizeLimit';

interface ProjectMemoryPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectMemoryPanel({ project, onClose }: ProjectMemoryPanelProps) {
  const { t } = useTranslation();
  const newNoteId = useId();
  const activeProjectId = useRef<string | null>(project.id);
  const [memory, setMemory] = useState<ProjectMemoryState | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [newNote, setNewNote] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState<ErrorKey | null>(null);
  const newNoteBytes = useMemo(() => byteLength(newNote), [newNote]);

  useEffect(() => {
    let current = true;
    activeProjectId.current = project.id;
    setMemory(null);
    setDrafts({});
    setNewNote('');
    setError(null);
    setIsBusy(false);
    setIsLoading(true);
    void loadMemory(project.id)
      .then((loaded) => {
        if (!current) return;
        setMemory(loaded);
        setDrafts(toDrafts(loaded.items));
      })
      .catch(() => {
        if (current) setError('projectMemoryLoadFailed');
      })
      .finally(() => {
        if (current) setIsLoading(false);
      });
    return () => {
      current = false;
      if (activeProjectId.current === project.id) activeProjectId.current = null;
    };
  }, [project.id]);

  const reload = async () => {
    const requestedProjectId = project.id;
    setIsLoading(true);
    setError(null);
    try {
      const loaded = await loadMemory(requestedProjectId);
      if (activeProjectId.current !== requestedProjectId) return;
      setMemory(loaded);
      setDrafts(toDrafts(loaded.items));
    } catch {
      if (activeProjectId.current === requestedProjectId) setError('projectMemoryLoadFailed');
    } finally {
      if (activeProjectId.current === requestedProjectId) setIsLoading(false);
    }
  };

  const run = async (method: string, extra: Record<string, unknown> = {}) => {
    if (isBusy || !memory) return;
    const requestedProjectId = project.id;
    setIsBusy(true);
    setError(null);
    try {
      const response = await localServiceClient.call(method, { projectId: requestedProjectId, ...extra });
      if (activeProjectId.current !== requestedProjectId) return;
      const updated = parseMemory(response.memory);
      setMemory(updated);
      setDrafts(toDrafts(updated.items));
      if (method === 'project.memory.add') setNewNote('');
    } catch (cause: unknown) {
      if (activeProjectId.current === requestedProjectId) setError(readErrorKey(cause));
    } finally {
      if (activeProjectId.current === requestedProjectId) setIsBusy(false);
    }
  };

  const addNote = () => {
    if (
      !memory ||
      isBusy ||
      newNote.trim().length === 0 ||
      newNoteBytes > maximumItemBytes ||
      memory.items.length >= maximumItems ||
      memory.totalBytes + newNoteBytes > maximumTotalBytes
    ) {
      return;
    }
    void run('project.memory.add', { content: newNote });
  };

  const updateNote = (item: ProjectMemoryItem) => {
    const content = drafts[item.id] ?? item.content;
    const nextBytes = memory
      ? memory.totalBytes - byteLength(item.content) + byteLength(content)
      : maximumTotalBytes + 1;
    if (
      isBusy ||
      content.trim().length === 0 ||
      byteLength(content) > maximumItemBytes ||
      nextBytes > maximumTotalBytes
    ) {
      return;
    }
    void run('project.memory.update', { itemId: item.id, content });
  };

  const exportMemory = () => {
    if (!memory || memory.items.length === 0) return;
    let pendingObjectUrl: string | null = null;
    let pendingAnchor: HTMLAnchorElement | null = null;
    try {
      const payload = {
        format: 'openchat.project-memory.v1',
        project: { id: project.id, name: project.name },
        exportedAt: new Date().toISOString(),
        items: memory.items,
      };
      const objectUrl = URL.createObjectURL(new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' }));
      pendingObjectUrl = objectUrl;
      pendingAnchor = document.createElement('a');
      pendingAnchor.href = objectUrl;
      pendingAnchor.download = `${safeFileName(project.name)}-memory.json`;
      document.body.append(pendingAnchor);
      pendingAnchor.click();
      pendingAnchor.remove();
      pendingAnchor = null;
      window.setTimeout(() => URL.revokeObjectURL(objectUrl), 1000);
      pendingObjectUrl = null;
    } catch {
      pendingAnchor?.remove();
      if (pendingObjectUrl) URL.revokeObjectURL(pendingObjectUrl);
      setError('projectMemoryExportFailed');
    }
  };

  return (
    <div className="grid gap-4">
      <p className="text-sm text-secondary-text">{t('projectMemoryDescription')}</p>
      {isLoading && memory === null ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('historyLoading')}
        </p>
      ) : null}
      {memory ? (
        <>
          <div className="flex items-center justify-between gap-4 rounded-card border border-border p-3 text-sm font-medium text-text">
            <span>{t('projectMemoryEnable')}</span>
            <Switch
              aria-label={t('projectMemoryEnable')}
              checked={memory.enabled}
              disabled={isBusy}
              onCheckedChange={(enabled) => void run('project.memory.set_enabled', { enabled })}
            />
          </div>
          <div className="flex items-center justify-between gap-4 rounded-card border border-border p-3 text-sm font-medium text-text">
            <span>{t('projectMemoryUseHistory')}</span>
            <Switch
              aria-label={t('projectMemoryUseHistory')}
              checked={memory.historyContextEnabled}
              disabled={isBusy}
              onCheckedChange={(enabled) => void run('project.memory.set_history_context_enabled', { enabled })}
            />
          </div>
          <p className="text-sm text-secondary-text">{t('projectMemoryHistoryDescription')}</p>
          <div className="flex items-center justify-between gap-4 rounded-card border border-border p-3 text-sm font-medium text-text">
            <span>{t('projectMemoryAllowAssistantSaves')}</span>
            <Switch
              aria-label={t('projectMemoryAllowAssistantSaves')}
              checked={memory.assistantSavesEnabled}
              disabled={isBusy || !memory.enabled}
              onCheckedChange={(enabled) => void run('project.memory.set_assistant_saves_enabled', { enabled })}
            />
          </div>
          <p className="text-sm text-secondary-text">{t('projectMemoryAssistantSavesDescription')}</p>
          <p className="text-right text-xs text-secondary-text">
            {t('projectMemoryTotalSize', {
              bytes: memory.totalBytes,
              maxBytes: maximumTotalBytes,
              count: memory.items.length,
              maxCount: maximumItems,
            })}
          </p>
          <div className="flex justify-end">
            <Button
              disabled={isBusy || memory.items.length === 0}
              onClick={exportMemory}
              size="small"
              variant="secondary"
            >
              <Download aria-hidden="true" size={15} />
              {t('projectMemoryExport')}
            </Button>
          </div>
          {memory.items.length > 0 ? (
            <div className="grid gap-3">
              {memory.items.map((item, index) => {
                const noteId = `project-memory-note-${item.id}`;
                const content = drafts[item.id] ?? item.content;
                const bytes = byteLength(content);
                const totalBytes = memory.totalBytes - byteLength(item.content) + bytes;
                const changed = content !== item.content;
                return (
                  <section className="grid gap-2 rounded-card border border-border p-3" key={item.id}>
                    <label className="grid gap-2 text-sm font-medium text-text" htmlFor={noteId}>
                      <span className="flex flex-wrap items-center justify-between gap-2">
                        <span>{t('projectMemoryNoteLabel', { index: index + 1 })}</span>
                        <span className="text-xs font-normal text-secondary-text">
                          {t(item.origin === 'assistant' ? 'projectMemoryAssistantOrigin' : 'projectMemoryUserOrigin')}
                        </span>
                      </span>
                      <Textarea
                        className="min-h-24 resize-y rounded-control border border-border bg-raised p-3 font-ui text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                        fieldLayout="none"
                        disabled={isBusy}
                        id={noteId}
                        onChange={(event) => setDrafts((current) => ({ ...current, [item.id]: event.target.value }))}
                        value={content}
                      />
                    </label>
                    <p className="text-right text-xs text-secondary-text">
                      {t('projectMemoryItemSize', { bytes, maxBytes: maximumItemBytes })}
                    </p>
                    <div className="flex flex-wrap justify-end gap-2">
                      <Button
                        aria-label={t('projectMemoryDeleteNote', { index: index + 1 })}
                        disabled={isBusy}
                        onClick={() => void run('project.memory.delete', { itemId: item.id })}
                        size="small"
                        variant="ghost"
                      >
                        <Trash2 aria-hidden="true" size={15} />
                        {t('projectMemoryDelete')}
                      </Button>
                      {changed ? (
                        <>
                          <Button
                            disabled={isBusy}
                            onClick={() => setDrafts((current) => ({ ...current, [item.id]: item.content }))}
                            size="small"
                            variant="secondary"
                          >
                            {t('cancel')}
                          </Button>
                          <Button
                            disabled={
                              isBusy ||
                              content.trim().length === 0 ||
                              bytes > maximumItemBytes ||
                              totalBytes > maximumTotalBytes
                            }
                            onClick={() => updateNote(item)}
                            size="small"
                            variant="primary"
                          >
                            {t('projectMemorySaveNote')}
                          </Button>
                        </>
                      ) : null}
                    </div>
                  </section>
                );
              })}
            </div>
          ) : (
            <p className="rounded-card border border-border p-3 text-sm text-secondary-text">
              {t('projectMemoryEmpty')}
            </p>
          )}
          <div className="grid gap-2 rounded-card border border-border p-3">
            <label className="grid gap-2 text-sm font-medium text-text" htmlFor={newNoteId}>
              {t('projectMemoryNewNote')}
              <Textarea
                className="min-h-24 resize-y rounded-control border border-border bg-raised p-3 font-ui text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                fieldLayout="none"
                disabled={isBusy}
                id={newNoteId}
                onChange={(event) => setNewNote(event.target.value)}
                placeholder={t('projectMemoryHint')}
                value={newNote}
              />
            </label>
            <p className="text-right text-xs text-secondary-text">
              {t('projectMemoryItemSize', { bytes: newNoteBytes, maxBytes: maximumItemBytes })}
            </p>
            <div className="flex justify-end">
              <Button
                disabled={
                  isBusy ||
                  newNote.trim().length === 0 ||
                  newNoteBytes > maximumItemBytes ||
                  memory.items.length >= maximumItems ||
                  memory.totalBytes + newNoteBytes > maximumTotalBytes
                }
                onClick={addNote}
                size="small"
                variant="secondary"
              >
                {t('projectMemoryAdd')}
              </Button>
            </div>
          </div>
        </>
      ) : null}
      {error ? (
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t(error)}
          </p>
          {error === 'projectMemoryLoadFailed' ? (
            <Button disabled={isLoading} onClick={() => void reload()} size="small">
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}
      <div className="flex justify-end">
        <Button disabled={isBusy} onClick={onClose}>
          {t('cancel')}
        </Button>
      </div>
    </div>
  );
}

async function loadMemory(projectId: string): Promise<ProjectMemoryState> {
  const response = await localServiceClient.call('project.memory.get', { projectId });
  return parseMemory(response.memory);
}

function parseMemory(value: unknown): ProjectMemoryState {
  if (
    !isObject(value) ||
    typeof value.enabled !== 'boolean' ||
    typeof value.assistantSavesEnabled !== 'boolean' ||
    typeof value.historyContextEnabled !== 'boolean' ||
    !Array.isArray(value.items) ||
    !isCount(value.totalBytes)
  ) {
    throw new Error('The project memory state is invalid.');
  }
  const items = value.items.map((item): ProjectMemoryItem => {
    if (
      !isObject(item) ||
      typeof item.id !== 'string' ||
      typeof item.content !== 'string' ||
      (item.origin !== 'user' && item.origin !== 'assistant')
    ) {
      throw new Error('A project memory note is invalid.');
    }
    return { id: item.id, content: item.content, origin: item.origin };
  });
  if (
    items.length > maximumItems ||
    value.totalBytes !== items.reduce((sum, item) => sum + byteLength(item.content), 0)
  ) {
    throw new Error('The project memory limits are invalid.');
  }
  return {
    enabled: value.enabled,
    assistantSavesEnabled: value.assistantSavesEnabled,
    historyContextEnabled: value.historyContextEnabled,
    items,
    totalBytes: value.totalBytes,
  };
}

function safeFileName(value: string): string {
  const sanitized = Array.from(value.trim(), (character) => {
    const codePoint = character.charCodeAt(0);
    return codePoint < 32 || codePoint === 127 || '<>:"/\\|?*'.includes(character) ? '-' : character;
  }).join('');
  const compact = sanitized.split(/\s+/).filter(Boolean).join(' ');
  return Array.from(compact).slice(0, 80).join('') || 'project';
}

function readErrorKey(cause: unknown): ErrorKey {
  if (cause instanceof LocalServiceError) {
    if (cause.code === 'project_memory_item_limit') return 'projectMemoryItemLimit';
    if (cause.code === 'project_memory_size_limit') return 'projectMemorySizeLimit';
    if (cause.code === 'project_memory_item_size_limit') return 'projectMemoryItemSizeLimit';
  }
  return 'projectMemoryActionFailed';
}

function toDrafts(items: ProjectMemoryItem[]): Record<string, string> {
  return Object.fromEntries(items.map((item) => [item.id, item.content]));
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
