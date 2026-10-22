import { BookOpen, Download, LoaderCircle, Pencil, Save } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Select } from '@/design-system';
import {
  getSavedOutputVersion,
  listSavedOutputVersions,
  saveSavedOutputVersion,
  selectSavedOutputVersion,
  saveConversationMarkdown,
  type SavedOutputSummary,
  type SavedOutputVersionSummary,
} from '@/modules/conversations/data/chatStorage';
import { conversationMarkdownFileName } from '@/modules/conversations/data/conversationExport';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { MessageMarkdown } from './message/MessageMarkdown';

interface SavedResponseWorkspaceDialogProps {
  output: SavedOutputSummary | null;
  open: boolean;
  onOpenChange(open: boolean): void;
}

type LoadState = 'loading' | 'ready' | 'error';
type WorkspaceError = 'outputWorkspaceVersionsFailed' | 'outputWorkspaceSaveFailed' | 'outputWorkspaceExportFailed';

export function SavedResponseWorkspaceDialog({ output, open, onOpenChange }: SavedResponseWorkspaceDialogProps) {
  const { t } = useTranslation();
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [versions, setVersions] = useState<SavedOutputVersionSummary[]>([]);
  const [selectedVersion, setSelectedVersion] = useState<number | null>(null);
  const [content, setContent] = useState('');
  const [draft, setDraft] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<WorkspaceError | null>(null);
  const [feedback, setFeedback] = useState<'outputWorkspaceExported' | null>(null);
  const [isPreview, setIsPreview] = useState(false);
  const [pendingVersion, setPendingVersion] = useState<number | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [openSourceAfterDiscard, setOpenSourceAfterDiscard] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const openConversation = useChatStore((state) => state.selectConversation);
  const isDirty = draft !== content;

  useEffect(() => {
    if (!open || !output) return;
    let cancelled = false;
    setLoadState('loading');
    setError(null);
    setFeedback(null);
    setIsPreview(false);
    setPendingVersion(null);
    setConfirmDiscard(false);
    setOpenSourceAfterDiscard(false);
    setContent('');
    setDraft('');
    setSelectedVersion(null);
    void (async () => {
      try {
        const loadedVersions = await listSavedOutputVersions(output.conversationId, output.messageId);
        const selected = loadedVersions.find((version) => version.versionNumber === output.selectedVersion);
        if (!selected) throw new Error('The selected saved response version is unavailable.');
        const version = await getSavedOutputVersion(
          output.conversationId,
          output.messageId,
          selected.versionNumber,
        );
        if (cancelled) return;
        setVersions(loadedVersions);
        setSelectedVersion(version.versionNumber);
        setContent(version.content);
        setDraft(version.content);
        setLoadState('ready');
      } catch {
        if (cancelled) return;
        setError('outputWorkspaceVersionsFailed');
        setLoadState('error');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [loadAttempt, open, output]);

  const versionOptions = useMemo(
    () => versions.map((version) => ({
      value: String(version.versionNumber),
      label: t('outputWorkspaceVersion', { version: version.versionNumber }),
    })),
    [t, versions],
  );

  const loadVersion = async (versionNumber: number) => {
    if (!output) return;
    setLoadState('loading');
    setError(null);
    try {
      await selectSavedOutputVersion(output.conversationId, output.messageId, versionNumber);
      const version = await getSavedOutputVersion(output.conversationId, output.messageId, versionNumber);
      setSelectedVersion(version.versionNumber);
      setContent(version.content);
      setDraft(version.content);
      setLoadState('ready');
      setIsPreview(false);
    } catch {
      setError('outputWorkspaceVersionsFailed');
      setLoadState('error');
    }
  };

  const chooseVersion = (value: string) => {
    const versionNumber = Number(value);
    if (!Number.isSafeInteger(versionNumber) || versionNumber < 1 || versionNumber === selectedVersion) return;
    if (isDirty) {
      setPendingVersion(versionNumber);
      setOpenSourceAfterDiscard(false);
      setConfirmDiscard(true);
      return;
    }
    void loadVersion(versionNumber);
  };

  const saveVersion = async () => {
    if (!output || loadState !== 'ready' || isSaving || !isDirty || !draft.trim()) return;
    setIsSaving(true);
    setError(null);
    setFeedback(null);
    try {
      const saved = await saveSavedOutputVersion(output.conversationId, output.messageId, draft);
      const nextVersions = await listSavedOutputVersions(output.conversationId, output.messageId);
      setVersions(nextVersions);
      setSelectedVersion(saved.versionNumber);
      setContent(saved.content);
      setDraft(saved.content);
      setIsPreview(false);
    } catch {
      setError('outputWorkspaceSaveFailed');
    } finally {
      setIsSaving(false);
    }
  };

  const exportVersion = async () => {
    if (!output || selectedVersion === null || loadState !== 'ready') return;
    setError(null);
    setFeedback(null);
    try {
      const exported = await saveConversationMarkdown(
        conversationMarkdownFileName(output.conversationTitle),
        t('outputWorkspaceExport'),
        content,
      );
      if (!exported) {
        setError('outputWorkspaceExportFailed');
        return;
      }
      setFeedback('outputWorkspaceExported');
    } catch {
      setError('outputWorkspaceExportFailed');
    }
  };

  const requestClose = (nextOpen: boolean) => {
    if (nextOpen) return;
    if (isDirty) {
      setConfirmDiscard(true);
      setPendingVersion(null);
      setOpenSourceAfterDiscard(false);
      return;
    }
    onOpenChange(false);
  };

  const discardChanges = () => {
    const nextVersion = pendingVersion;
    setConfirmDiscard(false);
    setPendingVersion(null);
    if (nextVersion !== null) {
      void loadVersion(nextVersion);
      return;
    }
    if (openSourceAfterDiscard && output) {
      void openConversation(output.conversationId, output.messageId);
      setOpenSourceAfterDiscard(false);
      onOpenChange(false);
      return;
    }
    onOpenChange(false);
  };

  return (
    <>
      <Dialog
        className="w-full max-w-4xl"
        onOpenChange={requestClose}
        open={open && output !== null}
        title={t('outputWorkspaceTitle')}
      >
        {output ? (
          <div className="saved-response-workspace-body">
            <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
              <div className="min-w-0">
                <h2 className="truncate font-medium text-text">{output.conversationTitle}</h2>
                <p className="mt-1 text-sm text-secondary-text">{t('outputWorkspaceDescription')}</p>
              </div>
              <Button
                className="justify-self-start sm:justify-self-end"
                onClick={() => {
                  if (isDirty) {
                    setPendingVersion(null);
                    setOpenSourceAfterDiscard(true);
                    setConfirmDiscard(true);
                    return;
                  }
                  void openConversation(output.conversationId, output.messageId);
                  onOpenChange(false);
                }}
                size="small"
                variant="secondary"
              >
                <BookOpen aria-hidden="true" size={15} />
                {t('outputOpenConversation')}
              </Button>
            </div>
            {loadState === 'loading' ? (
              <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                {t('outputsLoading')}
              </p>
            ) : loadState === 'error' ? (
              <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
                <p className="text-sm text-destructive">{t(error ?? 'outputWorkspaceVersionsFailed')}</p>
                <Button onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small" variant="secondary">
                  {t('retry')}
                </Button>
              </div>
            ) : (
              <>
                <Select
                  label={t('outputWorkspaceVersionSelect')}
                  onChange={(event) => chooseVersion(event.target.value)}
                  options={versionOptions}
                  value={selectedVersion === null ? '' : String(selectedVersion)}
                />
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex gap-2" role="group" aria-label={t('outputWorkspaceMode')}>
                    <Button onClick={() => setIsPreview(false)} size="small" variant={isPreview ? 'ghost' : 'secondary'}>
                      <Pencil aria-hidden="true" size={15} />
                      {t('outputWorkspaceEdit')}
                    </Button>
                    <Button onClick={() => setIsPreview(true)} size="small" variant={isPreview ? 'secondary' : 'ghost'}>
                      <BookOpen aria-hidden="true" size={15} />
                      {t('outputWorkspacePreview')}
                    </Button>
                  </div>
                  <p className="text-xs text-secondary-text">{t('outputWorkspaceMarkdownSource')}</p>
                </div>
                {isPreview ? (
                  <div className="min-h-48 overflow-auto rounded-control border border-border bg-background p-4">
                    <MessageMarkdown content={draft} citationSources={[]} />
                  </div>
                ) : (
                  <label className="grid gap-2 text-sm font-medium text-text" htmlFor="saved-response-workspace-content">
                    {t('outputWorkspaceContentLabel')}
                    <textarea
                        className="saved-response-workspace-editor w-full resize-y rounded-control border border-border bg-composer px-3 py-2 font-mono text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      id="saved-response-workspace-content"
                      onChange={(event) => {
                        setDraft(event.target.value);
                        setError(null);
                        setFeedback(null);
                      }}
                      value={draft}
                    />
                  </label>
                )}
                {!draft.trim() ? <p className="text-sm text-destructive" role="alert">{t('outputWorkspaceEmpty')}</p> : null}
                {error ? <p className="text-sm text-destructive" role="alert">{t(error)}</p> : null}
                {feedback ? <p aria-live="polite" className="text-sm text-success" role="status">{t(feedback)}</p> : null}
                <div className="flex flex-wrap justify-between gap-2 border-t border-border pt-3">
                  <Button disabled={isSaving || !draft.trim()} onClick={() => void exportVersion()} size="small" variant="secondary">
                    <Download aria-hidden="true" size={15} />
                    {t('outputWorkspaceExport')}
                  </Button>
                  <Button disabled={isSaving || !isDirty || !draft.trim()} onClick={() => void saveVersion()} size="small" variant="primary">
                    {isSaving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : <Save aria-hidden="true" size={15} />}
                    {isSaving ? t('saving') : t('outputWorkspaceSaveVersion')}
                  </Button>
                </div>
              </>
            )}
          </div>
        ) : null}
      </Dialog>
      <Dialog
        onOpenChange={(nextOpen) => {
          if (!nextOpen) setConfirmDiscard(false);
        }}
        open={confirmDiscard}
        title={t('outputWorkspaceUnsavedTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('outputWorkspaceUnsavedBody')}</p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              onClick={() => {
                setConfirmDiscard(false);
                setPendingVersion(null);
                setOpenSourceAfterDiscard(false);
              }}
              variant="secondary"
            >
              {t('outputWorkspaceKeepEditing')}
            </Button>
            <Button onClick={discardChanges} variant="destructive">{t('outputWorkspaceDiscard')}</Button>
          </div>
        </div>
      </Dialog>
    </>
  );
}
