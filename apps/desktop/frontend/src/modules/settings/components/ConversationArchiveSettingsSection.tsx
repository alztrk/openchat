import { Archive, ArchiveRestore, ShieldCheck } from 'lucide-react';
import { type FormEvent, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Dialog, Input } from '@/design-system';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import {
  buildArchiveFileName,
  type ConversationArchiveSummary,
  exportConversationArchive,
  inspectConversationArchive,
  restoreConversationArchive,
  savePendingArchiveExport,
} from '@/modules/settings/data/localDataOperations';
import { LocalServiceError } from '@/platform/local-service';
import { open } from '@/platform/tauri';
import { ArchivePassphraseDialog } from './ArchivePassphraseDialog';

type ErrorKey =
  | 'conversationArchiveUnavailable'
  | 'conversationArchiveNoConversations'
  | 'conversationArchivePickerFailed'
  | 'conversationArchiveInvalidFile'
  | 'conversationArchiveInvalidResponse'
  | 'conversationArchiveExportFailed'
  | 'conversationArchiveImportFailed'
  | 'conversationArchiveInvalidPassphraseOrFile'
  | 'conversationArchivePassphraseInvalid'
  | 'conversationArchiveNotFound'
  | 'conversationArchiveConflict'
  | 'conversationArchiveBusy'
  | 'conversationArchiveStorageFailed'
  | 'conversationArchiveLimitExceeded'
  | 'conversationArchiveTakingLong'
  | 'conversationArchiveOperationFailed'
  | 'historyLoadFailed';

interface ExportRequest {
  conversationIds: string[];
  passphrase: string;
}

export function ConversationArchiveSettingsSection({ enabled }: { enabled: boolean }) {
  const { i18n, t } = useTranslation();
  const conversations = useChatStore((state) => state.conversations);
  const [isWorking, setIsWorking] = useState(false);
  const [isChoosingConversations, setIsChoosingConversations] = useState(false);
  const [archiveToRestore, setArchiveToRestore] = useState<ConversationArchiveSummary | null>(null);
  const [restorePath, setRestorePath] = useState<string | null>(null);
  const [restorePassphrase, setRestorePassphrase] = useState<string | null>(null);
  const [pendingImportPath, setPendingImportPath] = useState<string | null>(null);
  const [isPassphraseOpen, setIsPassphraseOpen] = useState(false);
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [feedback, setFeedback] = useState<{
    key: 'conversationArchiveExportSuccess' | 'conversationArchiveImportSuccess';
    count: number;
  } | null>(null);

  const reportError = (error: unknown, action: 'export' | 'import') => {
    if (error instanceof LocalServiceError) {
      setErrorKey(
        archiveErrorKey(error.code) ??
          (error.code === 'invalid_service_response'
            ? 'conversationArchiveInvalidResponse'
            : action === 'export'
              ? 'conversationArchiveExportFailed'
              : 'conversationArchiveImportFailed'),
      );
    } else {
      setErrorKey(action === 'export' ? 'conversationArchiveExportFailed' : 'conversationArchiveImportFailed');
    }
  };

  const exportSelected = async (request: ExportRequest) => {
    setIsChoosingConversations(false);
    setIsWorking(true);
    setErrorKey(null);
    setFeedback(null);
    try {
      const result = await exportConversationArchive(request.conversationIds, request.passphrase);
      const saved = await savePendingArchiveExport(
        'conversation',
        result.exportId,
        buildArchiveFileName('openchatbackup'),
        t('conversationArchiveSaveTitle'),
      );
      if (!saved) return;
      setFeedback({ key: 'conversationArchiveExportSuccess', count: result.conversationCount });
    } catch (error) {
      reportError(error, 'export');
    } finally {
      setIsWorking(false);
    }
  };

  const startImport = async () => {
    setIsWorking(true);
    setErrorKey(null);
    setFeedback(null);
    try {
      let selected: string | string[] | null;
      try {
        selected = await open({
          multiple: false,
          directory: false,
          filters: [{ name: 'OpenChat conversation archive', extensions: ['openchatbackup'] }],
          title: t('conversationArchiveChooseFile'),
        });
      } catch {
        setErrorKey('conversationArchivePickerFailed');
        return;
      }
      if (selected === null) return;
      if (typeof selected !== 'string' || !selected.trim()) {
        setErrorKey('conversationArchiveInvalidFile');
        return;
      }

      setPendingImportPath(selected);
      setIsPassphraseOpen(true);
    } catch (error) {
      reportError(error, 'import');
    } finally {
      setIsWorking(false);
    }
  };

  const unlockArchive = async (passphrase: string): Promise<boolean> => {
    const path = pendingImportPath;
    if (!path || isWorking) return false;
    setIsWorking(true);
    setErrorKey(null);
    try {
      const summary = await inspectConversationArchive(path, passphrase);
      setRestorePath(path);
      setRestorePassphrase(passphrase);
      setArchiveToRestore(summary);
      setPendingImportPath(null);
      setIsPassphraseOpen(false);
      return true;
    } catch (error) {
      reportError(error, 'import');
      return false;
    } finally {
      setIsWorking(false);
    }
  };

  const restoreArchive = async (policy: 'skip_existing' | 'import_as_copy') => {
    if (!restorePath || restorePassphrase === null || isWorking) return;
    setArchiveToRestore(null);
    setIsWorking(true);
    setErrorKey(null);
    try {
      const result = await restoreConversationArchive(restorePath, restorePassphrase, policy);
      setFeedback({ key: 'conversationArchiveImportSuccess', count: result.conversationCount });
      try {
        await useChatStore.getState().refreshConversations();
      } catch {
        setErrorKey('historyLoadFailed');
      }
    } catch (error) {
      reportError(error, 'import');
    } finally {
      setRestorePath(null);
      setRestorePassphrase(null);
      setIsWorking(false);
    }
  };

  return (
    <Card className="grid gap-4 bg-composer p-4 sm:p-5">
      <div className="flex items-start gap-3">
        <ShieldCheck aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={19} />
        <div className="grid gap-1">
          <h3 className="font-semibold">{t('conversationArchiveTitle')}</h3>
          <p className="text-sm text-secondary-text">{t('conversationArchiveDescription')}</p>
          <p className="text-sm text-secondary-text">{t('conversationArchiveIncludesNotice')}</p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          disabled={!enabled || isWorking}
          onClick={() => {
            setErrorKey(null);
            setFeedback(null);
            if (conversations.length === 0) {
              setErrorKey('conversationArchiveNoConversations');
              return;
            }
            setIsChoosingConversations(true);
          }}
        >
          <Archive aria-hidden="true" size={16} />
          {t('exportConversations')}
        </Button>
        <Button disabled={!enabled || isWorking} onClick={() => void startImport()}>
          <ArchiveRestore aria-hidden="true" size={16} />
          {t('importConversations')}
        </Button>
      </div>

      {!enabled ? (
        <p className="text-sm text-warning" role="status">
          {t('conversationArchiveUnavailable')}
        </p>
      ) : null}
      {isWorking ? (
        <>
          <div
            aria-label={t('conversationArchiveProcessing')}
            className="h-1 overflow-hidden rounded-full bg-selected"
            role="progressbar"
          >
            <div className="h-full w-1/3 animate-pulse rounded-full bg-accent" />
          </div>
          <p aria-live="polite" className="text-sm text-secondary-text">
            {t('conversationArchiveProcessing')}
          </p>
        </>
      ) : null}
      {errorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
      {feedback ? (
        <p aria-live="polite" className="text-sm text-success">
          {t(feedback.key, { count: feedback.count })}
        </p>
      ) : null}

      <ConversationSelectionDialog
        conversations={conversations}
        i18nLanguage={i18n.language}
        onCancel={() => setIsChoosingConversations(false)}
        onExport={(request) => void exportSelected(request)}
        open={isChoosingConversations}
      />

      <ArchivePassphraseDialog
        confirming={false}
        errorMessage={
          errorKey === 'conversationArchiveInvalidPassphraseOrFile' ||
          errorKey === 'conversationArchivePassphraseInvalid'
            ? t(errorKey)
            : null
        }
        onCancel={() => {
          setIsPassphraseOpen(false);
          setPendingImportPath(null);
          setErrorKey(null);
        }}
        onSubmit={unlockArchive}
        open={isPassphraseOpen}
        title={t('conversationArchivePassphraseTitle')}
        working={isWorking}
      />

      <ArchiveRestoreDialog
        language={i18n.language}
        onCancel={() => {
          setArchiveToRestore(null);
          setRestorePath(null);
          setRestorePassphrase(null);
        }}
        onRestore={(policy) => void restoreArchive(policy)}
        open={archiveToRestore !== null}
        summary={archiveToRestore}
      />
    </Card>
  );
}

function ConversationSelectionDialog({
  conversations,
  i18nLanguage,
  onCancel,
  onExport,
  open,
}: {
  conversations: ChatConversation[];
  i18nLanguage: string;
  onCancel(): void;
  onExport(request: ExportRequest): void;
  open: boolean;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [passphrase, setPassphrase] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [errorKey, setErrorKey] = useState<
    | 'conversationArchivePassphraseTooShort'
    | 'conversationArchivePassphraseTooLong'
    | 'conversationArchivePassphraseMismatch'
    | null
  >(null);

  useEffect(() => {
    if (open) return;
    setQuery('');
    setSelectedIds(new Set());
    setPassphrase('');
    setConfirmation('');
    setErrorKey(null);
  }, [open]);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return conversations.filter(
      (conversation) => !normalized || conversation.title.toLocaleLowerCase().includes(normalized),
    );
  }, [conversations, query]);
  const allVisibleSelected = filtered.length > 0 && filtered.every((conversation) => selectedIds.has(conversation.id));

  const toggleAllVisible = () => {
    setSelectedIds((current) => {
      const next = new Set(current);
      for (const conversation of filtered) {
        if (allVisibleSelected) next.delete(conversation.id);
        else next.add(conversation.id);
      }
      return next;
    });
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const passphraseCharacters = Array.from(passphrase).length;
    if (passphraseCharacters < 12) {
      setErrorKey('conversationArchivePassphraseTooShort');
      return;
    }
    if (new TextEncoder().encode(passphrase).byteLength > 512 || passphrase.includes('\0')) {
      setErrorKey('conversationArchivePassphraseTooLong');
      return;
    }
    if (confirmation !== passphrase) {
      setErrorKey('conversationArchivePassphraseMismatch');
      return;
    }
    if (selectedIds.size === 0) return;
    onExport({ conversationIds: [...selectedIds], passphrase });
    setPassphrase('');
    setConfirmation('');
  };

  const dateFormatter = new Intl.DateTimeFormat(i18nLanguage, { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <Dialog
      className="max-h-dialog-archive overflow-y-auto"
      onOpenChange={(isOpen) => {
        if (!isOpen) onCancel();
      }}
      open={open}
      title={t('conversationArchiveSelectTitle')}
    >
      <form className="grid gap-3" onSubmit={(event) => void submit(event)}>
        <p className="text-sm text-secondary-text">
          {t('conversationArchiveSelectedCount', { count: selectedIds.size })}
        </p>
        <Input
          label={t('conversationArchiveSearch')}
          onChange={(event) => setQuery(event.target.value)}
          value={query}
        />
        <Button
          className="justify-self-end"
          disabled={filtered.length === 0}
          onClick={toggleAllVisible}
          size="small"
          type="button"
          variant="ghost"
        >
          {t(allVisibleSelected ? 'conversationArchiveDeselectAll' : 'conversationArchiveSelectAll')}
        </Button>
        <fieldset className="m-0 min-w-0 max-h-60 overflow-y-auto rounded-control border border-border p-0">
          <legend className="sr-only">{t('conversationArchiveSelectTitle')}</legend>
          {filtered.length === 0 ? (
            <p className="p-4 text-sm text-secondary-text">{t('conversationArchiveNoMatches')}</p>
          ) : (
            filtered.map((conversation) => (
              <label
                className="flex min-h-12 cursor-pointer items-start gap-3 border-b border-border px-3 py-2 last:border-b-0 hover:bg-hover"
                key={conversation.id}
              >
                <input
                  checked={selectedIds.has(conversation.id)}
                  className="mt-1 size-4 accent-accent"
                  onChange={(event) =>
                    setSelectedIds((current) => {
                      const next = new Set(current);
                      if (event.target.checked) next.add(conversation.id);
                      else next.delete(conversation.id);
                      return next;
                    })
                  }
                  type="checkbox"
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm text-text">{conversation.title}</span>
                  <span className="mt-0.5 block text-xs text-secondary-text">
                    {dateFormatter.format(new Date(conversation.updatedAt))}
                  </span>
                </span>
              </label>
            ))
          )}
        </fieldset>
        <div className="grid gap-3 sm:grid-cols-2">
          <Input
            autoComplete="new-password"
            label={t('conversationArchivePassphrase')}
            maxLength={512}
            onChange={(event) => {
              setPassphrase(event.target.value);
              setErrorKey(null);
            }}
            type="password"
            value={passphrase}
          />
          <Input
            autoComplete="new-password"
            label={t('conversationArchiveConfirmPassphrase')}
            maxLength={512}
            onChange={(event) => {
              setConfirmation(event.target.value);
              setErrorKey(null);
            }}
            type="password"
            value={confirmation}
          />
        </div>
        <p className="text-sm text-secondary-text">{t('conversationArchivePassphraseRecovery')}</p>
        {errorKey ? (
          <p className="text-sm text-destructive" role="alert">
            {t(errorKey)}
          </p>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button onClick={onCancel} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={selectedIds.size === 0} type="submit">
            <Archive aria-hidden="true" size={16} />
            {t('exportConversations')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function ArchiveRestoreDialog({
  language,
  onCancel,
  onRestore,
  open,
  summary,
}: {
  language: string;
  onCancel(): void;
  onRestore(policy: 'skip_existing' | 'import_as_copy'): void;
  open: boolean;
  summary: ConversationArchiveSummary | null;
}) {
  const { t } = useTranslation();
  const [policy, setPolicy] = useState<'skip_existing' | 'import_as_copy'>('skip_existing');
  useEffect(() => {
    if (!open) setPolicy('skip_existing');
  }, [open]);
  if (!summary) return null;

  const date = new Intl.DateTimeFormat(language, { dateStyle: 'medium', timeStyle: 'short' }).format(
    new Date(summary.createdAtUnixMs),
  );
  const bytes = new Intl.NumberFormat(language, { style: 'unit', unit: 'byte', unitDisplay: 'short' }).format(
    summary.attachmentBytes,
  );

  return (
    <Dialog
      onOpenChange={(isOpen) => {
        if (!isOpen) onCancel();
      }}
      open={open}
      title={t('conversationArchivePreviewTitle')}
    >
      <div className="grid gap-3">
        <SummaryRow label={t('conversationArchiveCreatedAt')} value={date} />
        <SummaryRow label={t('conversationArchiveConversationCount')} value={String(summary.conversationCount)} />
        <SummaryRow label={t('conversationArchiveMessageCount')} value={String(summary.messageCount)} />
        <SummaryRow label={t('conversationArchiveAttachmentCount')} value={`${summary.attachmentCount} · ${bytes}`} />
        {summary.duplicateConversationCount > 0 ? (
          <SummaryRow
            label={t('conversationArchiveDuplicateCount')}
            value={String(summary.duplicateConversationCount)}
          />
        ) : null}
        <fieldset className="grid gap-2 border-t border-border pt-3">
          <legend className="sr-only">{t('conversationArchivePreviewTitle')}</legend>
          <label className="flex min-h-10 items-start gap-3 text-sm">
            <input
              checked={policy === 'skip_existing'}
              className="mt-1 size-4 accent-accent"
              onChange={() => setPolicy('skip_existing')}
              type="radio"
            />
            <span>{t('conversationArchiveSkipDuplicates')}</span>
          </label>
          <label className="flex min-h-10 items-start gap-3 text-sm">
            <input
              checked={policy === 'import_as_copy'}
              className="mt-1 size-4 accent-accent"
              onChange={() => setPolicy('import_as_copy')}
              type="radio"
            />
            <span>{t('conversationArchiveImportCopies')}</span>
          </label>
        </fieldset>
        <p className="text-sm text-secondary-text">{t('conversationArchiveRestoreNotice')}</p>
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button onClick={onCancel}>{t('cancel')}</Button>
          <Button onClick={() => onRestore(policy)}>
            <ArchiveRestore aria-hidden="true" size={16} />
            {t('importConversations')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-wrap justify-between gap-x-4 gap-y-1 text-sm">
      <span className="text-secondary-text">{label}</span>
      <span className="break-words text-right font-medium text-text">{value}</span>
    </div>
  );
}

function archiveErrorKey(code: string): ErrorKey | null {
  switch (code) {
    case 'archive_export_busy':
      return 'conversationArchiveBusy';
    case 'archive_export_conflict':
      return 'conversationArchiveConflict';
    case 'archive_export_invalid':
      return 'conversationArchiveInvalidResponse';
    case 'archive_export_storage_failed':
    case 'archive_export_unavailable':
      return 'conversationArchiveStorageFailed';
    case 'conversation_archive_invalid':
      return 'conversationArchiveInvalidPassphraseOrFile';
    case 'conversation_archive_passphrase_invalid':
      return 'conversationArchivePassphraseInvalid';
    case 'conversation_archive_not_found':
      return 'conversationArchiveNotFound';
    case 'conversation_archive_conflict':
      return 'conversationArchiveConflict';
    case 'conversation_archive_busy':
      return 'conversationArchiveBusy';
    case 'conversation_archive_storage_failed':
      return 'conversationArchiveStorageFailed';
    case 'conversation_archive_limit_exceeded':
      return 'conversationArchiveLimitExceeded';
    case 'service_timeout':
      return 'conversationArchiveTakingLong';
    default:
      return null;
  }
}
