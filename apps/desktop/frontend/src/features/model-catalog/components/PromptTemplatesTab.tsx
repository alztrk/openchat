import { Pencil, Plus, Send, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input, Textarea } from '@shared/components';
import {
  expandPromptTemplate,
  isPromptTemplateNameAvailable,
  PromptTemplateReadError,
  type PromptTemplate,
  type PromptTemplateDraft,
  promptParameterNames,
  promptTemplateLimits,
  readPromptTemplates,
  resetPromptTemplates,
  writePromptTemplates,
} from '@features/model-catalog/data/promptTemplates';

interface PromptTemplatesTabProps {
  onUsePrompt(prompt: string): void;
}

type FormError =
  | 'nameRequired'
  | 'nameExists'
  | 'promptRequired'
  | 'promptTooLong'
  | 'tooManyParameters'
  | 'saveFailed';

type LoadFailure = PromptTemplateReadError | { kind: 'unexpected' };
type RecoveryError = 'exportFailed' | 'resetFailed';
const urlRevocationDelayMs = 1000;

export function PromptTemplatesTab({ onUsePrompt }: PromptTemplatesTabProps) {
  const { t } = useTranslation();
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [loadError, setLoadError] = useState<LoadFailure | null>(null);
  const [draft, setDraft] = useState<PromptTemplateDraft | null>(null);
  const [activeTemplate, setActiveTemplate] = useState<PromptTemplate | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<PromptTemplate | null>(null);
  const [parameterValues, setParameterValues] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<FormError | null>(null);
  const [recoveryError, setRecoveryError] = useState<RecoveryError | null>(null);
  const [isResetDialogOpen, setIsResetDialogOpen] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);

  const loadTemplates = useCallback(() => {
    try {
      setTemplates(readPromptTemplates());
      setLoadError(null);
      setRecoveryError(null);
    } catch (cause: unknown) {
      setLoadError(cause instanceof PromptTemplateReadError ? cause : { kind: 'unexpected' });
    } finally {
      setIsLoaded(true);
    }
  }, []);

  useEffect(() => {
    loadTemplates();
  }, [loadTemplates]);

  const activeParameters = useMemo(
    () => (activeTemplate ? promptParameterNames(activeTemplate.prompt) : []),
    [activeTemplate],
  );

  const persist = async (next: PromptTemplate[]): Promise<boolean> => {
    if (savingRef.current) return false;
    savingRef.current = true;
    setIsSaving(true);
    try {
      await writePromptTemplates(next);
      setTemplates(next);
      setFormError(null);
      return true;
    } catch {
      setFormError('saveFailed');
      return false;
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  };

  const saveDraft = async () => {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) return setFormError('nameRequired');
    if (!isPromptTemplateNameAvailable(templates, name, draft.id)) return setFormError('nameExists');
    if (!draft.prompt.trim()) return setFormError('promptRequired');
    if (new TextEncoder().encode(draft.prompt).byteLength > promptTemplateLimits.maximumPromptBytes) {
      return setFormError('promptTooLong');
    }
    if (promptParameterNames(draft.prompt).length > 16) return setFormError('tooManyParameters');

    const now = Date.now();
    const existing = draft.id ? templates.find((item) => item.id === draft.id) : undefined;
    const nextTemplate: PromptTemplate = {
      id: existing?.id ?? crypto.randomUUID(),
      name,
      prompt: draft.prompt,
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    };
    const next = existing
      ? templates.map((item) => (item.id === existing.id ? nextTemplate : item))
      : [...templates, nextTemplate];
    if (next.length > promptTemplateLimits.maximumTemplates) return setFormError('saveFailed');
    if (await persist(next)) {
      setDraft(null);
      setFormError(null);
    }
  };

  const removeTemplate = async () => {
    if (!pendingRemoval) return;
    if (await persist(templates.filter((item) => item.id !== pendingRemoval.id))) {
      if (activeTemplate?.id === pendingRemoval.id) setActiveTemplate(null);
      if (draft?.id === pendingRemoval.id) setDraft(null);
      setPendingRemoval(null);
    }
  };

  const exportRawData = () => {
    if (loadError?.kind !== 'corrupt' || loadError.rawValue === null) return;
    let pendingObjectUrl: string | null = null;
    let pendingAnchor: HTMLAnchorElement | null = null;
    try {
      const objectUrl = URL.createObjectURL(new Blob([loadError.rawValue], { type: 'text/plain;charset=utf-8' }));
      pendingObjectUrl = objectUrl;
      pendingAnchor = document.createElement('a');
      pendingAnchor.href = objectUrl;
      pendingAnchor.download = 'openchat-prompt-templates-raw.txt';
      document.body.append(pendingAnchor);
      pendingAnchor.click();
      pendingAnchor.remove();
      pendingAnchor = null;
      window.setTimeout(() => URL.revokeObjectURL(objectUrl), urlRevocationDelayMs);
      pendingObjectUrl = null;
      setRecoveryError(null);
    } catch {
      pendingAnchor?.remove();
      if (pendingObjectUrl) URL.revokeObjectURL(pendingObjectUrl);
      setRecoveryError('exportFailed');
    }
  };

  const confirmReset = async () => {
    if (loadError?.kind !== 'corrupt' || isResetting) return;
    setIsResetting(true);
    setRecoveryError(null);
    try {
      await resetPromptTemplates();
      setTemplates([]);
      setLoadError(null);
      setIsResetDialogOpen(false);
    } catch {
      setRecoveryError('resetFailed');
    } finally {
      setIsResetting(false);
    }
  };

  if (!isLoaded) {
    return (
      <p aria-live="polite" className="p-6 text-sm text-secondary-text" role="status">
        {t('promptTemplatesLoading')}
      </p>
    );
  }

  if (loadError) {
    return (
      <section aria-labelledby="prompt-templates-heading" className="mx-auto grid w-full max-w-5xl gap-5 p-5 sm:p-8">
        <h2 className="text-lg font-semibold text-text" id="prompt-templates-heading">
          {t('promptTemplatesTitle')}
        </h2>
        <div className="grid gap-4 rounded-card border border-destructive/30 p-5">
          <p className="text-sm text-destructive" role="alert">
            {loadError.kind === 'corrupt' ? t('promptTemplatesCorruptData') : t('promptTemplatesLoadFailed')}
          </p>
          {recoveryError === 'exportFailed' ? (
            <p className="text-sm text-destructive">{t('promptTemplatesExportFailed')}</p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            {loadError.kind === 'corrupt' ? (
              <Button onClick={exportRawData} variant="secondary">
                {t('promptTemplatesExportRaw')}
              </Button>
            ) : null}
            <Button onClick={loadTemplates} variant="secondary">
              {t('promptTemplatesRetry')}
            </Button>
            {loadError.kind === 'corrupt' ? (
              <Button onClick={() => setIsResetDialogOpen(true)} variant="destructive">
                {t('promptTemplatesReset')}
              </Button>
            ) : null}
          </div>
        </div>
        <Dialog
          onOpenChange={(open) => {
            if (!isResetting) setIsResetDialogOpen(open);
          }}
          open={isResetDialogOpen}
          title={t('promptTemplatesResetTitle')}
        >
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">{t('promptTemplatesResetConfirmation')}</p>
            {recoveryError === 'resetFailed' ? (
              <p className="text-sm text-destructive" role="alert">
                {t('promptTemplatesResetFailed')}
              </p>
            ) : null}
            <div className="flex justify-end gap-2">
              <Button disabled={isResetting} onClick={() => setIsResetDialogOpen(false)} variant="secondary">
                {t('cancel')}
              </Button>
              <Button loading={isResetting} onClick={confirmReset} variant="destructive">
                {t('promptTemplatesResetConfirm')}
              </Button>
            </div>
          </div>
        </Dialog>
      </section>
    );
  }

  return (
    <section aria-labelledby="prompt-templates-heading" className="mx-auto grid w-full max-w-5xl gap-5 p-5 sm:p-8">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-text" id="prompt-templates-heading">
            {t('promptTemplatesTitle')}
          </h2>
          <p className="mt-1 max-w-2xl text-sm text-secondary-text">{t('promptTemplatesDescription')}</p>
        </div>
        <Button
          disabled={templates.length >= promptTemplateLimits.maximumTemplates || draft !== null}
          onClick={() => {
            setFormError(null);
            setDraft({ id: null, name: '', prompt: '' });
            setActiveTemplate(null);
          }}
          size="small"
          variant="secondary"
        >
          <Plus aria-hidden="true" size={16} />
          {t('promptTemplateCreate')}
        </Button>
      </header>

      {draft ? (
        <Card className="grid gap-4" padding="default">
          <h3 className="text-base font-semibold text-text">
            {t(draft.id ? 'promptTemplateEdit' : 'promptTemplateCreate')}
          </h3>
          <Input
            autoFocus
            disabled={isSaving}
            label={t('promptTemplateName')}
            maxLength={promptTemplateLimits.maximumNameLength}
            onChange={(event) => {
              setDraft((current) => (current ? { ...current, name: event.target.value } : current));
              setFormError(null);
            }}
            value={draft.name}
          />
          <div className="grid gap-2">
            <label className="text-sm font-medium text-text" htmlFor="prompt-template-content">
              {t('promptTemplateContent')}
            </label>
            <Textarea
              className="min-h-40 max-h-64 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus"
              disabled={isSaving}
              id="prompt-template-content"
              fieldLayout="none"
              maxLength={promptTemplateLimits.maximumPromptBytes}
              onChange={(event) => {
                setDraft((current) => (current ? { ...current, prompt: event.target.value } : current));
                setFormError(null);
              }}
              placeholder={t('promptTemplateContentPlaceholder')}
              value={draft.prompt}
            />
            <p className="text-xs text-secondary-text">{t('promptTemplateParametersHelp')}</p>
          </div>
          {formError ? (
            <p className="text-sm text-destructive" role="alert">
              {t(formErrorTranslationKey(formError))}
            </p>
          ) : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              disabled={isSaving}
              onClick={() => {
                setDraft(null);
                setFormError(null);
              }}
              variant="secondary"
            >
              {t('cancel')}
            </Button>
            <Button loading={isSaving} onClick={() => void saveDraft()} variant="primary">
              {t('save')}
            </Button>
          </div>
        </Card>
      ) : null}

      {activeTemplate ? (
        <Card className="grid gap-4" padding="default">
          <div>
            <h3 className="text-base font-semibold text-text">
              {t('promptTemplateFillTitle', { name: activeTemplate.name })}
            </h3>
            <p className="mt-1 text-xs text-secondary-text">{t('promptTemplateFillHelp')}</p>
          </div>
          {activeParameters.length > 0 ? (
            <div className="grid gap-3 sm:grid-cols-2">
              {activeParameters.map((parameter) => (
                <Input
                  key={parameter}
                  label={parameter}
                  onChange={(event) =>
                    setParameterValues((current) => ({ ...current, [parameter]: event.target.value }))
                  }
                  value={parameterValues[parameter] ?? ''}
                />
              ))}
            </div>
          ) : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setActiveTemplate(null)} variant="secondary">
              {t('cancel')}
            </Button>
            <Button
              onClick={() => {
                const expanded = expandPromptTemplate(activeTemplate.prompt, parameterValues);
                onUsePrompt(expanded);
              }}
              variant="primary"
            >
              <Send aria-hidden="true" size={15} />
              {t('promptTemplateUse')}
            </Button>
          </div>
        </Card>
      ) : null}

      {templates.length === 0 && !draft ? (
        <Card className="text-sm text-secondary-text" padding="comfortable">
          {t('promptTemplatesEmpty')}
        </Card>
      ) : (
        <ul className="grid gap-3 sm:grid-cols-2">
          {templates.map((template) => (
            <li key={template.id}>
              <Card className="grid h-full min-w-0 gap-3" padding="default">
                <div className="min-w-0">
                  <h3 className="truncate font-semibold text-text">{template.name}</h3>
                  <p className="mt-1 line-clamp-3 whitespace-pre-wrap break-words text-sm text-secondary-text">
                    {template.prompt}
                  </p>
                  {promptParameterNames(template.prompt).length > 0 ? (
                    <p className="mt-2 text-xs text-secondary-text">
                      {t('promptTemplateParameterCount', { count: promptParameterNames(template.prompt).length })}
                    </p>
                  ) : null}
                </div>
                <div className="flex flex-wrap items-center gap-2 border-t border-border pt-3">
                  <Button
                    onClick={() => {
                      setParameterValues({});
                      setActiveTemplate(template);
                      setDraft(null);
                    }}
                    size="small"
                    variant="primary"
                  >
                    <Send aria-hidden="true" size={14} />
                    {t('promptTemplateUse')}
                  </Button>
                  <Button
                    aria-label={t('promptTemplateEdit')}
                    onClick={() => {
                      setDraft({ id: template.id, name: template.name, prompt: template.prompt });
                      setActiveTemplate(null);
                      setFormError(null);
                    }}
                    size="icon"
                    variant="ghost"
                  >
                    <Pencil aria-hidden="true" size={15} />
                  </Button>
                  <Button
                    aria-label={t('promptTemplateDelete')}
                    onClick={() => {
                      setFormError(null);
                      setPendingRemoval(template);
                    }}
                    size="icon"
                    variant="ghost"
                  >
                    <Trash2 aria-hidden="true" size={15} />
                  </Button>
                </div>
              </Card>
            </li>
          ))}
        </ul>
      )}
      <Dialog
        onOpenChange={(open) => {
          if (!open && !isSaving) setPendingRemoval(null);
        }}
        open={pendingRemoval !== null}
        title={t('promptTemplateDeleteTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">
            {t('promptTemplateDeleteConfirmation', { name: pendingRemoval?.name ?? '' })}
          </p>
          {formError === 'saveFailed' ? (
            <p className="text-sm text-destructive" role="alert">
              {t('promptTemplateErrorSaveFailed')}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button disabled={isSaving} onClick={() => setPendingRemoval(null)} variant="secondary">
              {t('cancel')}
            </Button>
            <Button loading={isSaving} onClick={() => void removeTemplate()} variant="destructive">
              {t('promptTemplateDelete')}
            </Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}

function formErrorTranslationKey(
  error: FormError,
):
  | 'promptTemplateErrorNameRequired'
  | 'promptTemplateErrorNameExists'
  | 'promptTemplateErrorPromptRequired'
  | 'promptTemplateErrorPromptTooLong'
  | 'promptTemplateErrorTooManyParameters'
  | 'promptTemplateErrorSaveFailed' {
  if (error === 'nameRequired') return 'promptTemplateErrorNameRequired';
  if (error === 'nameExists') return 'promptTemplateErrorNameExists';
  if (error === 'promptRequired') return 'promptTemplateErrorPromptRequired';
  if (error === 'promptTooLong') return 'promptTemplateErrorPromptTooLong';
  if (error === 'tooManyParameters') return 'promptTemplateErrorTooManyParameters';
  return 'promptTemplateErrorSaveFailed';
}
