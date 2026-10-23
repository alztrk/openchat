import { Pencil, Plus, Send, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input } from '@/design-system';
import {
  expandPromptTemplate,
  isPromptTemplateNameAvailable,
  promptParameterNames,
  promptTemplateLimits,
  readPromptTemplates,
  writePromptTemplates,
  type PromptTemplate,
  type PromptTemplateDraft,
} from '@/modules/model-library/data/promptTemplates';

interface PromptTemplatesTabProps {
  onUsePrompt(prompt: string): void;
}

type FormError = 'nameRequired' | 'nameExists' | 'promptRequired' | 'promptTooLong' | 'tooManyParameters' | 'saveFailed';

export function PromptTemplatesTab({ onUsePrompt }: PromptTemplatesTabProps) {
  const { t } = useTranslation();
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [draft, setDraft] = useState<PromptTemplateDraft | null>(null);
  const [activeTemplate, setActiveTemplate] = useState<PromptTemplate | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<PromptTemplate | null>(null);
  const [parameterValues, setParameterValues] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<FormError | null>(null);

  useEffect(() => {
    try {
      setTemplates(readPromptTemplates());
      setLoadFailed(false);
    } catch {
      setLoadFailed(true);
    } finally {
      setIsLoaded(true);
    }
  }, []);

  const activeParameters = useMemo(
    () => (activeTemplate ? promptParameterNames(activeTemplate.prompt) : []),
    [activeTemplate],
  );

  const persist = (next: PromptTemplate[]) => {
    try {
      writePromptTemplates(next);
      setTemplates(next);
      setLoadFailed(false);
      setFormError(null);
      return true;
    } catch {
      setFormError('saveFailed');
      return false;
    }
  };

  const saveDraft = () => {
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
    if (persist(next)) {
      setDraft(null);
      setFormError(null);
    }
  };

  const removeTemplate = () => {
    if (!pendingRemoval) return;
    if (persist(templates.filter((item) => item.id !== pendingRemoval.id))) {
      if (activeTemplate?.id === pendingRemoval.id) setActiveTemplate(null);
      if (draft?.id === pendingRemoval.id) setDraft(null);
      setPendingRemoval(null);
    }
  };

  if (!isLoaded) {
    return <p aria-live="polite" className="p-6 text-sm text-secondary-text" role="status">{t('modelsLoading')}</p>;
  }

  if (loadFailed) {
    return <p className="m-6 text-sm text-destructive" role="alert">{t('promptTemplatesLoadFailed')}</p>;
  }

  return (
    <section aria-labelledby="prompt-templates-heading" className="mx-auto grid w-full max-w-5xl gap-5 p-5 sm:p-8">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-text" id="prompt-templates-heading">{t('promptTemplatesTitle')}</h2>
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
        <Card className="grid gap-4 p-4">
          <h3 className="text-base font-semibold text-text">{t(draft.id ? 'promptTemplateEdit' : 'promptTemplateCreate')}</h3>
          <Input
            autoFocus
            label={t('promptTemplateName')}
            maxLength={promptTemplateLimits.maximumNameLength}
            onChange={(event) => {
              setDraft((current) => current ? { ...current, name: event.target.value } : current);
              setFormError(null);
            }}
            value={draft.name}
          />
          <div className="grid gap-2">
            <label className="text-sm font-medium text-text" htmlFor="prompt-template-content">{t('promptTemplateContent')}</label>
            <textarea
              className="min-h-40 max-h-64 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus"
              id="prompt-template-content"
              maxLength={promptTemplateLimits.maximumPromptBytes}
              onChange={(event) => {
                setDraft((current) => current ? { ...current, prompt: event.target.value } : current);
                setFormError(null);
              }}
              placeholder={t('promptTemplateContentPlaceholder')}
              value={draft.prompt}
            />
            <p className="text-xs text-secondary-text">{t('promptTemplateParametersHelp')}</p>
          </div>
          {formError ? <p className="text-sm text-destructive" role="alert">{t(formErrorTranslationKey(formError))}</p> : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => { setDraft(null); setFormError(null); }} variant="secondary">{t('cancel')}</Button>
            <Button onClick={saveDraft} variant="primary">{t('save')}</Button>
          </div>
        </Card>
      ) : null}

      {activeTemplate ? (
        <Card className="grid gap-4 p-4">
          <div>
            <h3 className="text-base font-semibold text-text">{t('promptTemplateFillTitle', { name: activeTemplate.name })}</h3>
            <p className="mt-1 text-xs text-secondary-text">{t('promptTemplateFillHelp')}</p>
          </div>
          {activeParameters.length > 0 ? (
            <div className="grid gap-3 sm:grid-cols-2">
              {activeParameters.map((parameter) => (
                <Input
                  key={parameter}
                  label={parameter}
                  onChange={(event) => setParameterValues((current) => ({ ...current, [parameter]: event.target.value }))}
                  value={parameterValues[parameter] ?? ''}
                />
              ))}
            </div>
          ) : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setActiveTemplate(null)} variant="secondary">{t('cancel')}</Button>
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
        <Card className="p-5 text-sm text-secondary-text">{t('promptTemplatesEmpty')}</Card>
      ) : (
        <ul className="grid gap-3 sm:grid-cols-2">
          {templates.map((template) => (
            <li key={template.id}>
              <Card className="grid h-full min-w-0 gap-3 p-4">
                <div className="min-w-0">
                  <h3 className="truncate font-semibold text-text">{template.name}</h3>
                  <p className="mt-1 line-clamp-3 whitespace-pre-wrap break-words text-sm text-secondary-text">{template.prompt}</p>
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
                  ><Pencil aria-hidden="true" size={15} /></Button>
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
          if (!open) setPendingRemoval(null);
        }}
        open={pendingRemoval !== null}
        title={t('promptTemplateDeleteTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('promptTemplateDeleteConfirmation', { name: pendingRemoval?.name ?? '' })}</p>
          {formError === 'saveFailed' ? <p className="text-sm text-destructive" role="alert">{t('promptTemplateErrorSaveFailed')}</p> : null}
          <div className="flex justify-end gap-2">
            <Button onClick={() => setPendingRemoval(null)} variant="secondary">{t('cancel')}</Button>
            <Button onClick={removeTemplate} variant="destructive">{t('promptTemplateDelete')}</Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}

function formErrorTranslationKey(error: FormError):
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
