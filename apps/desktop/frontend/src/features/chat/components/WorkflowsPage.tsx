import { ArrowLeft, Download, FileUp, Pencil, Plus, Trash2, Workflow } from 'lucide-react';
import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Textarea, toast } from '@shared/components';
import {
  buildWorkflowStartingPrompt,
  exportWorkflowTemplatesJson,
  importWorkflowTemplatesJson,
  isWorkflowTemplateNameAvailable,
  readWorkflowTemplates,
  workflowParameterNames,
  WorkflowTemplateReadError,
  writeWorkflowTemplates,
  type WorkflowTemplate,
  type WorkflowParameter,
} from '@features/chat/data/workflowTemplates';

const maximumImportBytes = 4 * 1024 * 1024;

export interface WorkflowsPageProps {
  onStartConversation(prompt: string, template: WorkflowTemplate, values: Record<string, string>): void;
}

type WorkflowView = 'list' | 'edit' | 'launch';

export function WorkflowsPage({ onStartConversation }: WorkflowsPageProps) {
  const { t } = useTranslation();
  const importInputRef = useRef<HTMLInputElement>(null);
  const [templates, setTemplates] = useState<WorkflowTemplate[]>([]);
  const [view, setView] = useState<WorkflowView>('list');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [prompt, setPrompt] = useState('');
  const [beforeStartPrompt, setBeforeStartPrompt] = useState('');
  const [afterResponsePrompt, setAfterResponsePrompt] = useState('');
  const [parameterValues, setParameterValues] = useState<Record<string, string>>({});
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<WorkflowTemplateReadError | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  const selectedTemplate = templates.find((template) => template.id === selectedId) ?? null;
  const editingParameters = useMemo(
    () => workflowParameterNames(prompt, beforeStartPrompt, afterResponsePrompt),
    [prompt, beforeStartPrompt, afterResponsePrompt],
  );
  const promptTooLong = new TextEncoder().encode(prompt.trim()).byteLength > 32 * 1024;
  const hookPromptTooLong = [beforeStartPrompt, afterResponsePrompt].some(
    (value) => new TextEncoder().encode(value).byteLength > 32 * 1024,
  );
  const tooManyParameters = editingParameters.length > 16;
  const canSave =
    name.trim().length > 0 &&
    prompt.trim().length > 0 &&
    !promptTooLong &&
    !hookPromptTooLong &&
    !tooManyParameters &&
    isWorkflowTemplateNameAvailable(templates, name, editingId);

  useEffect(() => {
    setView('list');
    setSelectedId(null);
    setEditingId(null);
    setPendingDeleteId(null);
    try {
      setTemplates(readWorkflowTemplates());
      setLoadError(null);
    } catch (cause) {
      setTemplates([]);
      setLoadError(
        cause instanceof WorkflowTemplateReadError
          ? cause
          : new WorkflowTemplateReadError('unavailable'),
      );
    }
  }, []);

  const beginCreate = () => {
    setEditingId(null);
    setName('');
    setDescription('');
    setPrompt('');
    setBeforeStartPrompt('');
    setAfterResponsePrompt('');
    setView('edit');
  };

  const beginEdit = (template: WorkflowTemplate) => {
    setEditingId(template.id);
    setName(template.name);
    setDescription(template.description ?? '');
    setPrompt(template.startingPrompt);
    setBeforeStartPrompt(template.beforeStartPrompt ?? '');
    setAfterResponsePrompt(template.afterResponsePrompt ?? '');
    setView('edit');
  };

  const saveTemplate = async () => {
    if (!canSave || loadError || isBusy) return;
    const current = templates.find((template) => template.id === editingId) ?? null;
    const now = Date.now();
    const previousParameters = new Map(current?.parameters.map((parameter) => [parameter.name, parameter]) ?? []);
    const nextTemplate: WorkflowTemplate = {
      id: current?.id ?? crypto.randomUUID(),
      name: name.trim(),
      description: description.trim() || undefined,
      startingPrompt: prompt.trim(),
      beforeStartPrompt: beforeStartPrompt.trim() || undefined,
      afterResponsePrompt: afterResponsePrompt.trim() || undefined,
      parameters: editingParameters.map((parameterName) => previousParameters.get(parameterName) ?? { name: parameterName }),
      providerId: current?.providerId,
      modelId: current?.modelId,
      systemPrompt: current?.systemPrompt,
      tools: current?.tools,
      createdAt: current?.createdAt ?? now,
      updatedAt: now,
    };
    const nextTemplates = current
      ? templates.map((template) => (template.id === current.id ? nextTemplate : template))
      : [...templates, nextTemplate];

    setIsBusy(true);
    try {
      await writeWorkflowTemplates(nextTemplates);
      setTemplates(nextTemplates);
      setSelectedId(nextTemplate.id);
      setView('list');
      toast.success(t(current ? 'workflowTemplateUpdated' : 'workflowTemplateSaved'));
    } catch {
      toast.error(t('workflowTemplateSaveFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const deleteTemplate = async (template: WorkflowTemplate) => {
    if (loadError || isBusy) return;
    if (pendingDeleteId !== template.id) {
      setPendingDeleteId(template.id);
      return;
    }
    const nextTemplates = templates.filter((item) => item.id !== template.id);
    setIsBusy(true);
    try {
      await writeWorkflowTemplates(nextTemplates);
      setTemplates(nextTemplates);
      setSelectedId((current) => (current === template.id ? null : current));
      setPendingDeleteId(null);
      toast.success(t('workflowTemplateDeleted'));
    } catch {
      toast.error(t('workflowTemplateSaveFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const importTemplates = async (file: File | undefined) => {
    if (!file || loadError || isBusy) return;
    if (file.size > maximumImportBytes) {
      toast.error(t('workflowTemplateImportFailed'));
      return;
    }
    setIsBusy(true);
    try {
      const result = importWorkflowTemplatesJson(await file.text(), templates);
      if (result.imported.length === 0) {
        toast.info(t('workflowTemplateImportEmpty', { skipped: result.skippedCount }));
        return;
      }
      const nextTemplates = [...templates, ...result.imported];
      await writeWorkflowTemplates(nextTemplates);
      setTemplates(nextTemplates);
      toast.success(t('workflowTemplateImported', { count: result.imported.length, skipped: result.skippedCount }));
    } catch {
      toast.error(t('workflowTemplateImportFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const exportTemplates = () => {
    if (templates.length === 0) return;
    try {
      const blob = new Blob([exportWorkflowTemplatesJson(templates)], { type: 'application/json;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'openchat-workflows.json';
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
    } catch {
      toast.error(t('workflowTemplateExportFailed'));
    }
  };

  const beginLaunch = (template: WorkflowTemplate) => {
    setSelectedId(template.id);
    setParameterValues(
      Object.fromEntries(template.parameters.map((parameter) => [parameter.name, parameter.defaultValue ?? ''])),
    );
    setView('launch');
  };

  const launchTemplate = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedTemplate) return;
    const missingRequired = selectedTemplate.parameters.some(
      (parameter) => parameter.required !== false && !parameterValues[parameter.name]?.trim(),
    );
    if (missingRequired) return;
    onStartConversation(
      buildWorkflowStartingPrompt(
        selectedTemplate.startingPrompt,
        selectedTemplate.beforeStartPrompt,
        parameterValues,
      ),
      selectedTemplate,
      parameterValues,
    );
  };

  const rawRecovery = loadError?.kind === 'corrupt' ? loadError.rawValue : null;
  const exportRawData = () => {
    if (rawRecovery === null) return;
    try {
      const url = URL.createObjectURL(new Blob([rawRecovery], { type: 'application/json;charset=utf-8' }));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'openchat-workflows-recovery.json';
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
    } catch {
      toast.error(t('workflowTemplateExportFailed'));
    }
  };

  return (
    <section aria-labelledby="workflows-page-title" className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border bg-navigation px-5 py-3 sm:px-8">
        <div className="min-w-0">
          <h1
            className="text-xl font-semibold leading-7 text-text"
            id="workflows-page-title"
          >
            {t('workflowTemplatesTitle')}
          </h1>
          <p className="mt-0.5 text-sm text-secondary-text">{t('workflowTemplateLibraryDescription')}</p>
        </div>
        {view === 'list' && !loadError ? (
          <div className="flex shrink-0 items-center gap-2">
            <Button disabled={isBusy} onClick={() => importInputRef.current?.click()} size="small" variant="secondary">
              <FileUp aria-hidden="true" size={16} />
              {t('workflowTemplateImport')}
            </Button>
            {templates.length > 0 ? (
              <>
                <Button disabled={isBusy} onClick={exportTemplates} size="small" variant="secondary">
                  <Download aria-hidden="true" size={16} />
                  {t('workflowTemplateExport')}
                </Button>
                <Button disabled={isBusy || templates.length >= 50} onClick={beginCreate} size="small" variant="primary">
                  <Plus aria-hidden="true" size={16} />
                  {t('workflowTemplateCreate')}
                </Button>
              </>
            ) : null}
          </div>
        ) : null}
      </header>
      <input
        accept="application/json,.json"
        aria-label={t('workflowTemplateImport')}
        className="sr-only"
        onChange={(event) => {
          const file = event.currentTarget.files?.[0];
          event.currentTarget.value = '';
          void importTemplates(file);
        }}
        ref={importInputRef}
        type="file"
      />

      {loadError ? (
        <div className="grid flex-1 content-center justify-items-start gap-3 px-5 py-6 md:px-8" role="alert">
          <p className="text-sm text-secondary-text">{t('workflowTemplateStorageUnavailable')}</p>
          {rawRecovery !== null ? (
            <Button onClick={exportRawData} size="small" variant="secondary">
              <Download aria-hidden="true" size={16} />
              {t('workflowTemplateExportRecovery')}
            </Button>
          ) : null}
        </div>
      ) : view === 'edit' ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 md:px-8">
          <form
            className="grid max-w-3xl gap-5"
            onSubmit={(event) => {
              event.preventDefault();
              void saveTemplate();
            }}
          >
            <Button className="w-fit" onClick={() => setView('list')} size="small" variant="ghost">
              <ArrowLeft aria-hidden="true" size={16} />
              {t('workflowTemplateBack')}
            </Button>
            <Input
              autoFocus
              label={t('workflowTemplateName')}
              maxLength={64}
              onChange={(event) => setName(event.currentTarget.value)}
              required
              value={name}
            />
            <Input
              label={t('workflowTemplateDescription')}
              maxLength={256}
              onChange={(event) => setDescription(event.currentTarget.value)}
              value={description}
            />
            <Textarea
              label={t('workflowTemplatePrompt')}
              description={t('workflowTemplateParametersHint')}
              maxLength={32 * 1024}
              onChange={(event) => setPrompt(event.currentTarget.value)}
              required
              value={prompt}
            />
            <Textarea
              description={t('workflowTemplateBeforeStartHint')}
              label={t('workflowTemplateBeforeStartPrompt')}
              maxLength={32 * 1024}
              onChange={(event) => setBeforeStartPrompt(event.currentTarget.value)}
              value={beforeStartPrompt}
            />
            <Textarea
              description={t('workflowTemplateAfterResponseHint')}
              label={t('workflowTemplateAfterResponsePrompt')}
              maxLength={32 * 1024}
              onChange={(event) => setAfterResponsePrompt(event.currentTarget.value)}
              value={afterResponsePrompt}
            />
            {!isWorkflowTemplateNameAvailable(templates, name, editingId) ? (
              <p className="text-sm text-destructive" role="alert">{t('workflowTemplateDuplicateName')}</p>
            ) : promptTooLong ? (
              <p className="text-sm text-destructive" role="alert">{t('workflowTemplatePromptLimit')}</p>
            ) : hookPromptTooLong ? (
              <p className="text-sm text-destructive" role="alert">{t('workflowTemplateHookPromptLimit')}</p>
            ) : tooManyParameters ? (
              <p className="text-sm text-destructive" role="alert">{t('workflowTemplateParametersLimit', { count: 16 })}</p>
            ) : null}
            <div className="flex justify-end gap-2 border-t border-border pt-4">
              <Button onClick={() => setView('list')} variant="secondary">{t('cancel')}</Button>
              <Button
                disabled={!canSave || isBusy || (templates.length >= 50 && editingId === null)}
                loading={isBusy}
                type="submit"
                variant="primary"
              >
                {t('save')}
              </Button>
            </div>
          </form>
        </div>
      ) : view === 'launch' && selectedTemplate ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 md:px-8">
          <form className="grid max-w-3xl gap-5" onSubmit={launchTemplate}>
            <Button className="w-fit" onClick={() => setView('list')} size="small" variant="ghost">
              <ArrowLeft aria-hidden="true" size={16} />
              {t('workflowTemplateBack')}
            </Button>
            <div>
              <h2 className="text-base font-semibold text-text">{selectedTemplate.name}</h2>
              {selectedTemplate.description ? <p className="mt-1 text-sm text-secondary-text">{selectedTemplate.description}</p> : null}
            </div>
            {selectedTemplate.parameters.map((parameter: WorkflowParameter) => (
              <Input
                key={parameter.name}
                label={parameter.label ?? parameter.name}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setParameterValues((current) => ({ ...current, [parameter.name]: value }));
                }}
                required={parameter.required !== false}
                value={parameterValues[parameter.name] ?? ''}
              />
            ))}
            <p className="text-sm text-secondary-text">{t('workflowTemplateNewConversationHint')}</p>
            <div className="flex justify-end gap-2 border-t border-border pt-4">
              <Button onClick={() => setView('list')} variant="secondary">{t('cancel')}</Button>
              <Button type="submit" variant="primary">
                <Workflow aria-hidden="true" size={16} />
                {t('workflowTemplateStart')}
              </Button>
            </div>
          </form>
        </div>
      ) : templates.length === 0 ? (
        <div className="grid min-h-0 flex-1 content-center justify-items-center gap-3 px-5 py-10 text-center">
          <Workflow aria-hidden="true" className="text-secondary-icon" size={24} />
          <p className="max-w-md text-sm text-secondary-text">{t('workflowTemplateEmpty')}</p>
          <Button disabled={isBusy} onClick={beginCreate} size="small" variant="primary">
            <Plus aria-hidden="true" size={16} />
            {t('workflowTemplateCreate')}
          </Button>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 md:px-8">
          <ul className="divide-y divide-border">
            {templates.map((template) => (
              <li className="flex min-w-0 items-center gap-2 py-3" key={template.id}>
                <button
                  className="group flex min-h-11 min-w-0 flex-1 items-center gap-3 rounded-control px-2 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onClick={() => beginLaunch(template)}
                  type="button"
                >
                  <Workflow aria-hidden="true" className="shrink-0 text-secondary-icon" size={17} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-text">{template.name}</span>
                    <span className="mt-0.5 block truncate text-xs text-secondary-text">
                      {template.description || template.startingPrompt}
                    </span>
                  </span>
                </button>
                <div className="flex shrink-0 items-center gap-1">
                  {pendingDeleteId === template.id ? (
                    <>
                      <Button disabled={isBusy} onClick={() => setPendingDeleteId(null)} size="small" variant="secondary">{t('cancel')}</Button>
                      <Button disabled={isBusy} onClick={() => void deleteTemplate(template)} size="small" variant="destructive">{t('workflowTemplateConfirmDelete')}</Button>
                    </>
                  ) : (
                    <>
                      <Button aria-label={t('workflowTemplateEdit', { name: template.name })} onClick={() => beginEdit(template)} size="icon" variant="ghost">
                        <Pencil aria-hidden="true" size={16} />
                      </Button>
                      <Button aria-label={t('workflowTemplateDelete', { name: template.name })} onClick={() => void deleteTemplate(template)} size="icon" variant="ghost">
                        <Trash2 aria-hidden="true" size={16} />
                      </Button>
                    </>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
