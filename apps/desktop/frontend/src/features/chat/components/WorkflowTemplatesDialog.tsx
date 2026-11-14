import { ArrowLeft, Download, FileUp, Pencil, Plus, Trash2, Workflow } from 'lucide-react';
import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog, Input, Textarea, toast } from '@shared/components';
import {
  expandWorkflowTemplate,
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

export interface WorkflowTemplatesDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  onStartConversation(prompt: string, template: WorkflowTemplate): void;
}

type WorkflowView = 'list' | 'edit' | 'launch';

export function WorkflowTemplatesDialog({ open, onOpenChange, onStartConversation }: WorkflowTemplatesDialogProps) {
  const { t } = useTranslation();
  const importInputRef = useRef<HTMLInputElement>(null);
  const [templates, setTemplates] = useState<WorkflowTemplate[]>([]);
  const [view, setView] = useState<WorkflowView>('list');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [prompt, setPrompt] = useState('');
  const [parameterValues, setParameterValues] = useState<Record<string, string>>({});
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<WorkflowTemplateReadError | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  const selectedTemplate = templates.find((template) => template.id === selectedId) ?? null;
  const editingParameters = useMemo(() => workflowParameterNames(prompt), [prompt]);
  const promptTooLong = new TextEncoder().encode(prompt.trim()).byteLength > 32 * 1024;
  const tooManyParameters = editingParameters.length > 16;
  const canSave =
    name.trim().length > 0 &&
    prompt.trim().length > 0 &&
    !promptTooLong &&
    !tooManyParameters &&
    isWorkflowTemplateNameAvailable(templates, name, editingId);

  useEffect(() => {
    if (!open) return;
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
  }, [open]);

  const beginCreate = () => {
    setEditingId(null);
    setName('');
    setDescription('');
    setPrompt('');
    setView('edit');
  };

  const beginEdit = (template: WorkflowTemplate) => {
    setEditingId(template.id);
    setName(template.name);
    setDescription(template.description ?? '');
    setPrompt(template.startingPrompt);
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
    onStartConversation(expandWorkflowTemplate(selectedTemplate.startingPrompt, parameterValues), selectedTemplate);
    onOpenChange(false);
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
    <Dialog
      onOpenChange={onOpenChange}
      open={open}
      size="wide"
      title={t('workflowTemplatesTitle')}
    >
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
        <div className="grid gap-3" role="alert">
          <p className="text-sm text-secondary-text">{t('workflowTemplateStorageUnavailable')}</p>
          {rawRecovery !== null ? (
            <Button className="w-fit" onClick={exportRawData} variant="secondary">
              <Download aria-hidden="true" size={16} />
              {t('workflowTemplateExportRecovery')}
            </Button>
          ) : null}
        </div>
      ) : view === 'edit' ? (
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void saveTemplate();
          }}
        >
          <Button className="w-fit" onClick={() => setView('list')} variant="ghost">
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
          {!isWorkflowTemplateNameAvailable(templates, name, editingId) ? (
            <p className="text-sm text-destructive" role="alert">{t('workflowTemplateDuplicateName')}</p>
          ) : promptTooLong ? (
            <p className="text-sm text-destructive" role="alert">{t('workflowTemplatePromptLimit')}</p>
          ) : tooManyParameters ? (
            <p className="text-sm text-destructive" role="alert">{t('workflowTemplateParametersLimit', { count: 16 })}</p>
          ) : null}
          <div className="flex justify-end gap-2">
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
      ) : view === 'launch' && selectedTemplate ? (
        <form className="grid gap-4" onSubmit={launchTemplate}>
          <Button className="w-fit" onClick={() => setView('list')} variant="ghost">
            <ArrowLeft aria-hidden="true" size={16} />
            {t('workflowTemplateBack')}
          </Button>
          <div>
            <h3 className="font-semibold">{selectedTemplate.name}</h3>
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
          <p className="text-xs text-secondary-text">{t('workflowTemplateNewConversationHint')}</p>
          <div className="flex justify-end gap-2">
            <Button onClick={() => setView('list')} variant="secondary">{t('cancel')}</Button>
            <Button type="submit" variant="primary">
              <Workflow aria-hidden="true" size={16} />
              {t('workflowTemplateStart')}
            </Button>
          </div>
        </form>
      ) : (
        <div className="grid gap-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="text-sm text-secondary-text">{t('workflowTemplateLibraryDescription')}</p>
            <div className="flex flex-wrap gap-2">
              <Button disabled={isBusy} onClick={() => importInputRef.current?.click()} size="small" variant="secondary">
                <FileUp aria-hidden="true" size={16} />
                {t('workflowTemplateImport')}
              </Button>
              <Button disabled={isBusy || templates.length === 0} onClick={exportTemplates} size="small" variant="secondary">
                <Download aria-hidden="true" size={16} />
                {t('workflowTemplateExport')}
              </Button>
              <Button disabled={isBusy || templates.length >= 50} onClick={beginCreate} size="small" variant="primary">
                <Plus aria-hidden="true" size={16} />
                {t('workflowTemplateCreate')}
              </Button>
            </div>
          </div>
          {templates.length === 0 ? (
            <div className="rounded-card border border-dashed border-border px-4 py-8 text-center text-sm text-secondary-text">
              {t('workflowTemplateEmpty')}
            </div>
          ) : (
            <ul className="grid gap-2">
              {templates.map((template) => (
                <li className="flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-raised p-3" key={template.id}>
                  <button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus" onClick={() => beginLaunch(template)} type="button">
                    <span className="block truncate font-medium">{template.name}</span>
                    <span className="mt-1 block line-clamp-2 text-sm text-secondary-text">
                      {template.description || template.startingPrompt}
                    </span>
                  </button>
                  <div className="flex shrink-0 gap-1">
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
          )}
        </div>
      )}
    </Dialog>
  );
}
