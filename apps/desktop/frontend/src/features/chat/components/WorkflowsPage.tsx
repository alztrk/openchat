import { ArrowDown, ArrowLeft, ArrowUp, Clock3, History, Pause, Play, Plus, Trash2, Workflow, X } from 'lucide-react';
import { type FormEvent, useCallback, useEffect, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { ChatModel } from '@entities/model';
import type { ChatProject } from '@entities/project';
import { Button, Input, Select, Textarea, toast } from '@shared/components';
import { ModelPicker } from '@features/chat/components';
import { favoriteSourceConnection, useChatStore } from '@features/chat/state';
import { modelRouteKey } from '@entities/model/modelPreferences';
import type { ChatModelFavorite } from '@features/chat/data';
import { workflowParameterNames } from '@features/chat/data/workflowTemplateCore';
import { readWorkflowTemplates, exportWorkflowTemplatesJson } from '@features/chat/data/workflowTemplates';
import {
  parseWorkflowAutomation,
  type WorkflowAutomation,
  type WorkflowRun,
  type WorkflowRunStatus,
  type WorkflowSchedule,
  type WorkflowStep,
} from '@features/chat/data/workflowAutomationCore';
import {
  readWorkflowAutomations,
  readWorkflowRuns,
  WorkflowAutomationStorageError,
  writeWorkflowAutomations,
} from '@features/chat/data/workflowAutomations';

interface WorkflowsPageProps {
  models: readonly ChatModel[];
  projects: readonly ChatProject[];
  onRunWorkflow(workflowId: string, scheduledAt: number | null): Promise<WorkflowRun | null>;
  onRetryWorkflow(runId: string): Promise<WorkflowRun | null>;
}

type WorkflowPageView = 'list' | 'edit';
type WorkflowScheduleUpdate = { cadence?: 'daily' | 'weekly'; weekday?: number; hour?: number; minute?: number };
type WorkflowDraft = {
  id: string | null;
  name: string;
  description: string;
  schedule: WorkflowSchedule;
  steps: WorkflowStep[];
  inputDefaults: Record<string, string>;
  modelKey: string;
  projectId: string;
};

export function WorkflowsPage({ models, projects, onRunWorkflow, onRetryWorkflow }: WorkflowsPageProps) {
  const { t } = useTranslation();
  const [view, setView] = useState<WorkflowPageView>('list');
  const [workflows, setWorkflows] = useState<WorkflowAutomation[]>([]);
  const [runs, setRuns] = useState<WorkflowRun[]>([]);
  const [draft, setDraft] = useState<WorkflowDraft | null>(null);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [retryingRunId, setRetryingRunId] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<'unavailable' | 'corrupt' | null>(null);
  const [rawRecoveryData, setRawRecoveryData] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelQuery, setModelQuery] = useState('');

  const modelProviders = useChatStore((state) => state.modelProviders);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const chatGptWorkspaceSelectionRequired = useChatStore((state) => state.chatGptWorkspaceSelectionRequired);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);

  const prompts = draft?.steps.map((step) => step.prompt) ?? [];
  const parameters = useMemo(() => workflowParameterNames(...prompts), [prompts]);
  const pickerModels = useMemo(
    () => models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)) || model.key === draft?.modelKey),
    [draft?.modelKey, hiddenModelKeys, models],
  );
  const selectedModelForPicker = pickerModels.find((model) => model.key === draft?.modelKey) ?? null;
  const projectOptions = useMemo(
    () => [
      { value: '', label: t('workflowAutomationNoProject') },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t],
  );

  const reload = useCallback(() => {
    try {
      setWorkflows(readWorkflowAutomations());
      setRuns(readWorkflowRuns());
      setLoadError(null);
      setRawRecoveryData(null);
    } catch (cause: unknown) {
      setWorkflows([]);
      setRuns([]);
      setLoadError(cause instanceof WorkflowAutomationStorageError && cause.rawValue !== null ? 'corrupt' : 'unavailable');
      setRawRecoveryData(cause instanceof WorkflowAutomationStorageError ? cause.rawValue : null);
    }
  }, []);

  useEffect(() => {
    reload();
    window.addEventListener('openchat-workflow-automations-updated', reload);
    return () => window.removeEventListener('openchat-workflow-automations-updated', reload);
  }, [reload]);

  const beginCreate = () => {
    const model = pickerModels[0];
    if (!model) {
      toast.error(t('workflowAutomationModelRequired'));
      return;
    }
    setDraft({
      id: null,
      name: '',
      description: '',
      schedule: { cadence: 'daily', hour: 9, minute: 0 },
      steps: [{ id: crypto.randomUUID(), name: t('workflowAutomationFirstStep'), prompt: '' }],
      inputDefaults: {},
      modelKey: model.key,
      projectId: '',
    });
    setModelPickerOpen(false);
    setModelQuery('');
    setView('edit');
  };

  const beginEdit = (workflow: WorkflowAutomation) => {
    const model = models.find((item) => item.providerId === workflow.providerId && item.modelId === workflow.modelId);
    setDraft({
      id: workflow.id,
      name: workflow.name,
      description: workflow.description ?? '',
      schedule: workflow.schedule,
      steps: workflow.steps,
      inputDefaults: workflow.inputDefaults,
      modelKey: model?.key ?? '',
      projectId: workflow.projectId ?? '',
    });
    setModelPickerOpen(false);
    setModelQuery('');
    setView('edit');
  };

  const changeSchedule = (update: WorkflowScheduleUpdate) => {
    setDraft((current) => {
      if (!current) return current;
      let nextSchedule: WorkflowSchedule;
      if (update.cadence === 'daily') {
        nextSchedule = { cadence: 'daily', hour: current.schedule.hour, minute: current.schedule.minute };
      } else if (update.cadence === 'weekly') {
        nextSchedule = { cadence: 'weekly', weekday: 1, hour: current.schedule.hour, minute: current.schedule.minute };
      } else if (current.schedule.cadence === 'weekly') {
        nextSchedule = {
          cadence: 'weekly',
          weekday: update.weekday ?? current.schedule.weekday,
          hour: update.hour ?? current.schedule.hour,
          minute: update.minute ?? current.schedule.minute,
        };
      } else {
        nextSchedule = {
          cadence: 'daily',
          hour: update.hour ?? current.schedule.hour,
          minute: update.minute ?? current.schedule.minute,
        };
      }
      return { ...current, schedule: nextSchedule };
    });
  };

  const saveWorkflow = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!draft || saving || loadError) return;
    const model = models.find((item) => item.key === draft.modelKey);
    if (!model) {
      toast.error(t('workflowAutomationModelRequired'));
      return;
    }
    const availableInputs = new Set(workflowParameterNames(...draft.steps.map((step) => step.prompt)));
    const inputDefaults = Object.fromEntries(
      [...availableInputs].map((name) => [name, draft.inputDefaults[name]?.trim() ?? '']),
    );
    if (Object.values(inputDefaults).some((value) => value.length === 0)) {
      toast.error(t('workflowAutomationInputDefaultsRequired'));
      return;
    }
    const current = workflows.find((workflow) => workflow.id === draft.id) ?? null;
    const now = Date.now();
    try {
      setSaving(true);
      const nextWorkflow = parseWorkflowAutomation({
        id: current?.id ?? crypto.randomUUID(),
        name: draft.name.trim(),
        description: draft.description.trim() || undefined,
        enabled: current?.enabled ?? true,
        schedule: draft.schedule,
        steps: draft.steps,
        inputDefaults,
        providerId: model.providerId,
        modelId: model.modelId,
        projectId: draft.projectId || undefined,
        createdAt: current?.createdAt ?? now,
        updatedAt: now,
        nextRunAt: getNextRunAt(draft.schedule, now),
      });
      const nextWorkflows = current
        ? workflows.map((workflow) => (workflow.id === current.id ? nextWorkflow : workflow))
        : [...workflows, nextWorkflow];
      await writeWorkflowAutomations(nextWorkflows);
      setView('list');
      setDraft(null);
      reload();
      toast.success(t(current ? 'workflowAutomationUpdated' : 'workflowAutomationCreated'));
    } catch {
      toast.error(t('workflowAutomationSaveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const toggleWorkflow = async (workflow: WorkflowAutomation) => {
    try {
      await writeWorkflowAutomations(
        workflows.map((item) => (item.id === workflow.id ? { ...item, enabled: !item.enabled, updatedAt: Date.now() } : item)),
      );
    } catch {
      toast.error(t('workflowAutomationSaveFailed'));
    }
  };

  const deleteWorkflow = async (workflow: WorkflowAutomation) => {
    if (pendingDeleteId !== workflow.id) {
      setPendingDeleteId(workflow.id);
      return;
    }
    try {
      await writeWorkflowAutomations(workflows.filter((item) => item.id !== workflow.id));
      setPendingDeleteId(null);
    } catch {
      toast.error(t('workflowAutomationSaveFailed'));
    }
  };

  const runNow = async (workflow: WorkflowAutomation) => {
    if (runningId !== null) return;
    setRunningId(workflow.id);
    try {
      const run = await onRunWorkflow(workflow.id, null);
      if (!run) toast.warning(t('workflowAutomationAlreadyRunning'));
      reload();
    } catch {
      toast.error(t('workflowAutomationRunFailed'));
      reload();
    } finally {
      setRunningId(null);
    }
  };

  const retryRun = async (run: WorkflowRun) => {
    if (retryingRunId !== null) return;
    setRetryingRunId(run.id);
    try {
      const retried = await onRetryWorkflow(run.id);
      if (!retried) toast.warning(t('workflowAutomationAlreadyRunning'));
      reload();
    } catch {
      toast.error(t('workflowAutomationRunFailed'));
      reload();
    } finally {
      setRetryingRunId(null);
    }
  };

  const exportLegacyTemplates = () => {
    try {
      const legacy = readWorkflowTemplates();
      if (legacy.length === 0) {
        toast.info(t('workflowAutomationNoLegacyTemplates'));
        return;
      }
      downloadJson('openchat-workflow-templates.json', exportWorkflowTemplatesJson(legacy));
    } catch {
      toast.error(t('workflowAutomationLegacyExportFailed'));
    }
  };

  const exportRecoveryData = () => {
    if (rawRecoveryData === null) return;
    try {
      downloadJson('openchat-workflows-recovery.json', rawRecoveryData);
    } catch {
      toast.error(t('workflowAutomationLegacyExportFailed'));
    }
  };

  const addStep = () => {
    setDraft((current) => current && current.steps.length < 16
      ? { ...current, steps: [...current.steps, { id: crypto.randomUUID(), name: '', prompt: '' }] }
      : current);
  };

  const selectFavoriteModel = async (favorite: ChatModelFavorite): Promise<boolean> => {
    const model = models.find((candidate) =>
      candidate.providerId === favorite.providerId &&
      candidate.modelId === favorite.modelId &&
      favoriteSourceConnection(candidate) === favorite.sourceConnectionId &&
      (!hiddenModelKeys.includes(modelRouteKey(candidate)) || candidate.key === draft?.modelKey),
    );
    if (!model) return false;
    setDraft((current) => current ? { ...current, modelKey: model.key } : current);
    setModelPickerOpen(false);
    return true;
  };

  const moveStep = (stepId: string, offset: -1 | 1) => {
    setDraft((current) => {
      if (!current) return current;
      const index = current.steps.findIndex((step) => step.id === stepId);
      const targetIndex = index + offset;
      if (index < 0 || targetIndex < 0 || targetIndex >= current.steps.length) return current;
      const steps = [...current.steps];
      const [step] = steps.splice(index, 1);
      if (!step) return current;
      steps.splice(targetIndex, 0, step);
      return { ...current, steps };
    });
  };

  const updateStep = (stepId: string, update: Partial<WorkflowStep>) => {
    setDraft((current) => current && ({
      ...current,
      steps: current.steps.map((step) => (step.id === stepId ? { ...step, ...update } : step)),
    }));
  };

  const removeStep = (stepId: string) => {
    setDraft((current) => current && current.steps.length > 1
      ? { ...current, steps: current.steps.filter((step) => step.id !== stepId) }
      : current);
  };

  return (
    <section aria-labelledby="workflows-page-title" className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border bg-navigation px-5 py-3 sm:px-8">
        <div className="min-w-0">
          <h1 className="text-xl font-semibold leading-7 text-text" id="workflows-page-title">{t('workflowTemplatesTitle')}</h1>
          <p className="mt-0.5 text-sm text-secondary-text">{t('workflowAutomationDescription')}</p>
        </div>
        {view === 'list' && !loadError && workflows.length > 0 ? (
          <Button disabled={workflows.length >= 50} onClick={beginCreate} size="small" variant="primary">
            <Plus aria-hidden="true" size={16} />
            {t('workflowAutomationCreate')}
          </Button>
        ) : null}
      </header>

      {loadError ? (
        <div className="grid flex-1 content-center justify-items-start gap-3 px-5 py-6 md:px-8" role="alert">
          <p className="text-sm text-secondary-text">{t('workflowAutomationStorageUnavailable')}</p>
          {loadError === 'corrupt' && rawRecoveryData !== null ? <Button onClick={exportRecoveryData} size="small" variant="secondary">{t('workflowAutomationRecoveryExport')}</Button> : null}
        </div>
      ) : view === 'edit' && draft ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 md:px-8">
          <form className="mx-auto grid max-w-3xl gap-5" onSubmit={(event) => void saveWorkflow(event)}>
            <Button className="w-fit" onClick={() => { setView('list'); setDraft(null); }} size="small" variant="ghost">
              <ArrowLeft aria-hidden="true" size={16} />
              {t('workflowTemplateBack')}
            </Button>
            <div className="grid gap-4 sm:grid-cols-2">
              <Input label={t('workflowTemplateName')} maxLength={64} onChange={(event) => setDraft({ ...draft, name: event.currentTarget.value })} required value={draft.name} />
              <Input label={t('workflowTemplateDescription')} maxLength={256} onChange={(event) => setDraft({ ...draft, description: event.currentTarget.value })} value={draft.description} />
              <fieldset className="grid min-w-0 gap-2">
                <legend className="text-sm font-medium text-text">{t('workflowAutomationModel')}</legend>
                <ModelPicker
                  canSelectProvider
                  chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
                  disabled={models.length === 0}
                  emptyModelsLabel={t('noModelsAvailable')}
                  fastModeAvailable={false}
                  fastModeEnabled={false}
                  fastModeLoading={false}
                  fastModeSaving={false}
                  hiddenModelKeys={hiddenModelKeys}
                  isOpen={modelPickerOpen}
                  knownModels={models}
                  modelFavorites={modelFavorites}
                  modelProviders={modelProviders}
                  models={pickerModels}
                  onFastModeToggle={() => undefined}
                  onOpenChange={setModelPickerOpen}
                  onQueryChange={setModelQuery}
                  onRemoveFavorite={(favorite) => { void removeModelFavorite(favorite); }}
                  onSelect={(model) => {
                    setDraft((current) => current ? { ...current, modelKey: model.key } : current);
                    setModelPickerOpen(false);
                  }}
                  onSelectFavorite={selectFavoriteModel}
                  onSelectProvider={() => setModelQuery('')}
                  onToggleFavorite={(model) => { void toggleFavoriteModel(model); }}
                  query={modelQuery}
                  selectedModel={selectedModelForPicker}
                  showFastModeControl={false}
                />
              </fieldset>
              <Select label={t('workflowAutomationProject')} onChange={(event) => setDraft({ ...draft, projectId: event.currentTarget.value })} options={projectOptions} value={draft.projectId} />
            </div>
            <section aria-labelledby="workflow-schedule-title" className="grid gap-3 border-t border-border pt-4">
              <h2 className="font-medium text-text" id="workflow-schedule-title">{t('workflowAutomationSchedule')}</h2>
              <div className="grid gap-4 sm:grid-cols-3">
                <Select
                  label={t('workflowAutomationRepeat')}
                  onChange={(event) => changeSchedule({ cadence: event.currentTarget.value === 'weekly' ? 'weekly' : 'daily' })}
                  options={[
                    { value: 'daily', label: t('workflowAutomationDaily') },
                    { value: 'weekly', label: t('workflowAutomationWeekly') },
                  ]}
                  value={draft.schedule.cadence}
                />
                {draft.schedule.cadence === 'weekly' ? (
                  <Select
                    label={t('workflowAutomationWeekday')}
                    onChange={(event) => changeSchedule({ weekday: Number(event.currentTarget.value) })}
                    options={weekdayOptions(t)}
                    value={String(draft.schedule.weekday)}
                  />
                ) : null}
                <Input label={t('workflowAutomationTime')} onChange={(event) => {
                  const [hourText = '9', minuteText = '0'] = event.currentTarget.value.split(':');
                  changeSchedule({ hour: Number(hourText), minute: Number(minuteText) });
                }} required type="time" value={`${String(draft.schedule.hour).padStart(2, '0')}:${String(draft.schedule.minute).padStart(2, '0')}`} />
              </div>
              <p className="text-xs text-secondary-text">{t('workflowAutomationRunsWhileOpen')}</p>
            </section>
            <section aria-labelledby="workflow-steps-title" className="grid gap-4 border-t border-border pt-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="font-medium text-text" id="workflow-steps-title">{t('workflowAutomationSteps')}</h2>
                <Button disabled={draft.steps.length >= 16} onClick={addStep} size="small" variant="secondary"><Plus aria-hidden="true" size={15} />{t('workflowAutomationAddStep')}</Button>
              </div>
              {draft.steps.map((step, index) => (
                <article className="overflow-hidden rounded-container border border-border bg-surface" key={step.id}>
                  <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
                    <h3 className="min-w-0 truncate text-sm font-medium text-text">{t('workflowAutomationStepName', { number: index + 1 })}</h3>
                    <div className="flex shrink-0 items-center">
                      <Button aria-label={t('workflowAutomationMoveStepUp')} disabled={index === 0} onClick={() => moveStep(step.id, -1)} size="icon" variant="ghost"><ArrowUp aria-hidden="true" size={16} /></Button>
                      <Button aria-label={t('workflowAutomationMoveStepDown')} disabled={index === draft.steps.length - 1} onClick={() => moveStep(step.id, 1)} size="icon" variant="ghost"><ArrowDown aria-hidden="true" size={16} /></Button>
                      <Button aria-label={t('workflowAutomationRemoveStep')} disabled={draft.steps.length < 2} onClick={() => removeStep(step.id)} size="icon" variant="ghost"><X aria-hidden="true" size={16} /></Button>
                    </div>
                  </div>
                  <div className="grid gap-3 p-4">
                    <Input label={t('workflowAutomationStepTitle')} maxLength={64} onChange={(event) => updateStep(step.id, { name: event.currentTarget.value })} required value={step.name} />
                    <Textarea label={t('workflowAutomationInstruction')} maxLength={32 * 1024} onChange={(event) => updateStep(step.id, { prompt: event.currentTarget.value })} placeholder={t('workflowAutomationInstructionPlaceholder')} required value={step.prompt} />
                  </div>
                </article>
              ))}
            </section>
            {parameters.length > 0 ? (
              <section aria-labelledby="workflow-inputs-title" className="grid gap-3 border-t border-border pt-4">
                <h2 className="font-medium text-text" id="workflow-inputs-title">{t('workflowAutomationInputDefaults')}</h2>
                {parameters.map((name) => (
                  <Input key={name} label={name} onChange={(event) => setDraft({ ...draft, inputDefaults: { ...draft.inputDefaults, [name]: event.currentTarget.value } })} required value={draft.inputDefaults[name] ?? ''} />
                ))}
              </section>
            ) : null}
            <div className="flex justify-end gap-2 border-t border-border pt-4">
              <Button onClick={() => { setView('list'); setDraft(null); }} variant="secondary">{t('cancel')}</Button>
              <Button loading={saving} type="submit" variant="primary">{t('save')}</Button>
            </div>
          </form>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 md:px-8">
          <div className="mx-auto grid w-full max-w-5xl gap-8">
            {workflows.length === 0 ? (
              <div className="grid min-h-64 content-center justify-items-center gap-3 text-center">
                <Workflow aria-hidden="true" className="text-secondary-icon" size={25} />
                <p className="max-w-md text-sm text-secondary-text">{t('workflowAutomationEmpty')}</p>
                <Button onClick={beginCreate} size="small" variant="primary"><Plus aria-hidden="true" size={16} />{t('workflowAutomationCreate')}</Button>
              </div>
            ) : (
              <section aria-labelledby="workflow-list-title">
                <h2 className="mb-2 font-medium text-text" id="workflow-list-title">{t('workflowTemplatesTitle')}</h2>
                <ul className="divide-y divide-border">
                  {workflows.map((workflow) => (
                    <li className="flex flex-wrap items-center gap-3 py-3" key={workflow.id}>
                      <div className="min-w-0 flex-1">
                        <button className="block max-w-full truncate text-left text-sm font-medium text-text hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus" onClick={() => beginEdit(workflow)} type="button">{workflow.name}</button>
                        <p className="mt-0.5 truncate text-xs text-secondary-text">{workflow.description || scheduleDescription(workflow.schedule, t)} · {formatNextRun(workflow.nextRunAt, t)}</p>
                      </div>
                      <span className={`text-xs ${workflow.enabled ? 'text-success' : 'text-secondary-text'}`}>{workflow.enabled ? t('workflowAutomationActive') : t('workflowAutomationPaused')}</span>
                      <div className="flex shrink-0 items-center gap-1">
                        <Button disabled={runningId !== null} onClick={() => void runNow(workflow)} size="small" variant="secondary"><Play aria-hidden="true" size={15} />{t('workflowAutomationRunNow')}</Button>
                        <Button aria-label={t(workflow.enabled ? 'workflowAutomationPause' : 'workflowAutomationResume')} onClick={() => void toggleWorkflow(workflow)} size="icon" variant="ghost">{workflow.enabled ? <Pause aria-hidden="true" size={16} /> : <Play aria-hidden="true" size={16} />}</Button>
                        <Button aria-label={t('edit')} onClick={() => beginEdit(workflow)} size="small" variant="ghost">{t('edit')}</Button>
                        {pendingDeleteId === workflow.id ? (
                          <>
                            <Button onClick={() => setPendingDeleteId(null)} size="small" variant="secondary">{t('cancel')}</Button>
                            <Button onClick={() => void deleteWorkflow(workflow)} size="small" variant="destructive">{t('workflowAutomationDelete')}</Button>
                          </>
                        ) : (
                          <Button aria-label={t('workflowAutomationDelete')} onClick={() => void deleteWorkflow(workflow)} size="icon" variant="ghost"><Trash2 aria-hidden="true" size={16} /></Button>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            <section aria-labelledby="workflow-runs-title">
              <div className="mb-2 flex items-center gap-2">
                <History aria-hidden="true" className="text-secondary-icon" size={16} />
                <h2 className="font-medium text-text" id="workflow-runs-title">{t('workflowAutomationHistory')}</h2>
              </div>
              {runs.length === 0 ? <p className="py-3 text-sm text-secondary-text">{t('workflowAutomationNoRuns')}</p> : (
                <ul className="divide-y divide-border">
                  {[...runs].reverse().slice(0, 50).map((run) => (
                    <li className="flex flex-wrap items-center gap-3 py-3" key={run.id}>
                      <Clock3 aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium text-text">{run.workflowName}</p>
                        <p className="text-xs text-secondary-text">{formatRunDate(run.startedAt)} · {t('workflowAutomationStepsProgress', { completed: run.completedSteps, total: run.totalSteps })}</p>
                      </div>
                      <span className="text-xs text-secondary-text">{t(runStatusTranslationKey(run.status))}</span>
                      {(run.status === 'failed' || run.status === 'interrupted') ? <Button disabled={retryingRunId !== null} loading={retryingRunId === run.id} onClick={() => void retryRun(run)} size="small" variant="secondary">{t('retry')}</Button> : null}
                    </li>
                  ))}
                </ul>
              )}
            </section>

            <div className="border-t border-border pt-4">
              <Button onClick={exportLegacyTemplates} size="small" variant="ghost">{t('workflowAutomationLegacyExport')}</Button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

function weekdayOptions(t: TFunction): { value: string; label: string }[] {
  return [
    { value: '1', label: t('weekdayMonday') },
    { value: '2', label: t('weekdayTuesday') },
    { value: '3', label: t('weekdayWednesday') },
    { value: '4', label: t('weekdayThursday') },
    { value: '5', label: t('weekdayFriday') },
    { value: '6', label: t('weekdaySaturday') },
    { value: '0', label: t('weekdaySunday') },
  ];
}

function scheduleDescription(schedule: WorkflowSchedule, t: TFunction): string {
  const time = `${String(schedule.hour).padStart(2, '0')}:${String(schedule.minute).padStart(2, '0')}`;
  return schedule.cadence === 'daily'
    ? t('workflowAutomationDailyAt', { time })
    : t('workflowAutomationWeeklyAt', { weekday: weekdayOptions(t).find((item) => Number(item.value) === schedule.weekday)?.label ?? '', time });
}

function formatNextRun(value: number, t: TFunction): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return t('workflowAutomationScheduleUnavailable');
  return `${t('workflowAutomationNextRun')}: ${new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(date)}`;
}

function formatRunDate(value: number): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value));
}

function getNextRunAt(schedule: WorkflowSchedule, now: number): number {
  const date = new Date(now);
  date.setHours(schedule.hour, schedule.minute, 0, 0);
  if (schedule.cadence === 'weekly') date.setDate(date.getDate() + ((schedule.weekday - date.getDay() + 7) % 7));
  if (date.getTime() <= now) date.setDate(date.getDate() + (schedule.cadence === 'daily' ? 1 : 7));
  return date.getTime();
}

function runStatusTranslationKey(status: WorkflowRunStatus):
  | 'workflowAutomationRunRunning'
  | 'workflowAutomationRunCompleted'
  | 'workflowAutomationRunFailedState'
  | 'workflowAutomationRunInterrupted' {
  switch (status) {
    case 'running': return 'workflowAutomationRunRunning';
    case 'completed': return 'workflowAutomationRunCompleted';
    case 'failed': return 'workflowAutomationRunFailedState';
    case 'interrupted': return 'workflowAutomationRunInterrupted';
  }
}

function downloadJson(filename: string, json: string): void {
  const url = URL.createObjectURL(new Blob([json], { type: 'application/json;charset=utf-8' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
