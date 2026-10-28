import { Bot, Pencil, Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Checkbox, Dialog, Input, Select, Switch, Textarea } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { type ProjectMcpServer, parseProjectMcpCatalog } from '@/modules/projects/data/projectMcp';
import type {
  AssistantProfile,
  AssistantProfileTemplateId,
  AssistantResponseStyle,
} from '@/modules/settings/data/assistantProfiles';
import { readyAssistantAgentTemplates } from '@/modules/settings/data/readyAssistantAgents';
import { useAssistantProfilesStore } from '@/modules/settings/stores/assistantProfilesStore';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';

type ProjectCatalogState = 'idle' | 'loading' | 'ready' | 'error';
type McpToolsState = 'idle' | 'loading' | 'ready' | 'error';

const builtInToolNames = [
  'list_files',
  'search_files',
  'search_project_code',
  'query_language_server',
  'read_file',
  'write_file',
  'edit_file',
  'execute_command',
  'send_terminal_input',
  'get_file_info',
  'web_search',
  'read_url_content',
  'generate_image',
  'ask_user',
  'run_project_task',
  'delegate_task',
  'list_subagent_models',
  'get_subagent_result',
  'cancel_subagent',
  'save_memory',
] as const;
const retiredToolNames = new Set(['git_status', 'git_diff', 'git_history']);

interface ProfileDraft {
  id: string | null;
  templateId: AssistantProfileTemplateId | null;
  name: string;
  instructions: string;
  responseStyle: AssistantResponseStyle;
  restrictTools: boolean;
  toolAllowlist: string[];
}

const emptyDraft: ProfileDraft = {
  id: null,
  templateId: null,
  name: '',
  instructions: '',
  responseStyle: 'balanced',
  restrictTools: false,
  toolAllowlist: [],
};

export function AssistantProfilesSettingsSection({ showReadyAgents = false }: { showReadyAgents?: boolean }) {
  const { t } = useTranslation();
  const profiles = useAssistantProfilesStore((state) => state.profiles);
  const loadState = useAssistantProfilesStore((state) => state.loadState);
  const saveState = useAssistantProfilesStore((state) => state.saveState);
  const errorKey = useAssistantProfilesStore((state) => state.errorKey);
  const initialize = useAssistantProfilesStore((state) => state.initialize);
  const retry = useAssistantProfilesStore((state) => state.retry);
  const saveProfile = useAssistantProfilesStore((state) => state.saveProfile);
  const removeProfile = useAssistantProfilesStore((state) => state.removeProfile);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const pendingProjectId = useChatStore((state) => state.pendingProjectId);
  const conversations = useChatStore((state) => state.conversations);
  const activeProjectId =
    pendingProjectId ??
    conversations.find((conversation) => conversation.id === activeConversationId)?.projectId ??
    null;
  const activeProjectIdRef = useRef(activeProjectId);
  const projectGenerationRef = useRef(0);
  const [draft, setDraft] = useState<ProfileDraft | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<AssistantProfile | null>(null);
  const [viewingReadyTemplate, setViewingReadyTemplate] = useState<
    (typeof readyAssistantAgentTemplates)[number] | null
  >(null);
  const [formError, setFormError] = useState<
    | 'assistantProfileNameRequired'
    | 'assistantProfileNameTooLong'
    | 'assistantProfileInstructionsTooLong'
    | 'assistantProfileToolLimit'
    | null
  >(null);
  const [projectCatalogState, setProjectCatalogState] = useState<ProjectCatalogState>('idle');
  const [projectCatalogAttempt, setProjectCatalogAttempt] = useState(0);
  const [projectToolNames, setProjectToolNames] = useState<string[]>([]);
  const [mcpServers, setMcpServers] = useState<ProjectMcpServer[]>([]);
  const [mcpToolNames, setMcpToolNames] = useState<string[]>([]);
  const [mcpToolsState, setMcpToolsState] = useState<McpToolsState>('idle');
  const [mcpToolsPartialFailure, setMcpToolsPartialFailure] = useState(false);
  const [mcpToolsErrorDetails, setMcpToolsErrorDetails] = useState<string | null>(null);

  useEffect(() => initialize(), [initialize]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The attempt counter intentionally retriggers this load effect.
  useEffect(() => {
    const generation = ++projectGenerationRef.current;
    activeProjectIdRef.current = activeProjectId;
    if (activeProjectId === null) {
      setProjectCatalogState('idle');
      setProjectToolNames([]);
      setMcpServers([]);
      setMcpToolNames([]);
      setMcpToolsState('idle');
      setMcpToolsPartialFailure(false);
      setMcpToolsErrorDetails(null);
      return;
    }

    let cancelled = false;
    setProjectCatalogState('loading');
    setProjectToolNames([]);
    setMcpServers([]);
    setMcpToolNames([]);
    setMcpToolsState('idle');
    setMcpToolsPartialFailure(false);
    setMcpToolsErrorDetails(null);
    void Promise.all([
      localServiceClient.call('project.tools.list', { projectId: activeProjectId }),
      localServiceClient.call('project.mcp.catalog.get', { projectId: activeProjectId }),
    ])
      .then(([toolsResponse, mcpResponse]) => {
        if (cancelled) return;
        const tools = parseProjectToolNames(toolsResponse);
        const catalog = parseProjectMcpCatalog(mcpResponse);
        setProjectToolNames(tools);
        setMcpServers(catalog.servers.filter((server) => server.enabled));
        setProjectCatalogState('ready');
      })
      .catch(() => {
        if (!cancelled) setProjectCatalogState('error');
      });
    return () => {
      cancelled = true;
      if (projectGenerationRef.current === generation) projectGenerationRef.current += 1;
    };
  }, [activeProjectId, projectCatalogAttempt]);

  const preservedProfileTools = useMemo(() => {
    const availableNames = new Set([...builtInToolNames, ...projectToolNames, ...mcpToolNames]);
    return profiles
      .flatMap((profile) => profile.toolAllowlist ?? [])
      .filter((name) => !availableNames.has(name) && !retiredToolNames.has(name));
  }, [mcpToolNames, profiles, projectToolNames]);

  const beginEdit = (profile: AssistantProfile) => {
    setFormError(null);
    setDraft({
      id: profile.id,
      templateId: profile.templateId ?? null,
      name: profile.name,
      instructions: profile.instructions,
      responseStyle: profile.responseStyle,
      restrictTools: profile.toolAllowlist !== null,
      toolAllowlist: profile.toolAllowlist?.filter((name) => !retiredToolNames.has(name)) ?? [],
    });
  };

  const saveDraft = async () => {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) {
      setFormError('assistantProfileNameRequired');
      return;
    }
    if (name.length > 64) {
      setFormError('assistantProfileNameTooLong');
      return;
    }
    if (new TextEncoder().encode(draft.instructions).byteLength > 16 * 1024) {
      setFormError('assistantProfileInstructionsTooLong');
      return;
    }

    const profile: AssistantProfile = {
      id: draft.id ?? crypto.randomUUID(),
      ...(draft.templateId ? { templateId: draft.templateId } : {}),
      name,
      instructions: draft.instructions,
      responseStyle: draft.responseStyle,
      toolAllowlist: draft.restrictTools ? [...new Set(draft.toolAllowlist)] : null,
    };

    if (await saveProfile(profile)) {
      setDraft(null);
      setFormError(null);
    }
  };

  const addReadyAgent = async (template: (typeof readyAssistantAgentTemplates)[number]) => {
    if (loadState !== 'ready') return;
    await saveProfile({
      id: crypto.randomUUID(),
      templateId: template.id,
      name: t(template.nameKey),
      instructions: template.instructions,
      responseStyle: 'balanced',
      toolAllowlist: null,
    });
  };

  const toggleTool = (toolName: string) => {
    if (draft && !draft.toolAllowlist.includes(toolName) && draft.toolAllowlist.length >= 32) {
      setFormError('assistantProfileToolLimit');
      return;
    }
    setFormError(null);
    setDraft((current) => {
      if (!current) return current;
      const selected = current.toolAllowlist.includes(toolName);
      return {
        ...current,
        toolAllowlist: selected
          ? current.toolAllowlist.filter((name) => name !== toolName)
          : [...current.toolAllowlist, toolName],
      };
    });
  };

  const loadMcpTools = async () => {
    if (activeProjectId === null || mcpServers.length === 0 || mcpToolsState === 'loading') return;
    const projectId = activeProjectId;
    const generation = projectGenerationRef.current;
    setMcpToolsState('loading');
    setMcpToolsPartialFailure(false);
    setMcpToolsErrorDetails(null);
    const results = await Promise.allSettled(
      mcpServers.map(async (server) => {
        const response = await localServiceClient.call('project.mcp.server.check', { projectId, server }, 40_000);
        return parseCheckedMcpToolNames(response, server.id);
      }),
    );
    if (projectGenerationRef.current !== generation || projectId !== activeProjectIdRef.current) return;
    const names = results.flatMap((result) => (result.status === 'fulfilled' ? result.value : []));
    setMcpToolNames([...new Set(names)].sort((left, right) => left.localeCompare(right)));
    const failed = results.some((result) => result.status === 'rejected');
    const firstFailure = results.find((result) => result.status === 'rejected');
    setMcpToolsErrorDetails(firstFailure?.status === 'rejected' ? readServiceErrorDetails(firstFailure.reason) : null);
    setMcpToolsPartialFailure(failed && names.length > 0);
    setMcpToolsState(failed && names.length === 0 ? 'error' : 'ready');
  };

  return (
    <section aria-label={t('assistantProfiles')} className="grid gap-4">
      <p className="text-sm text-secondary-text">{t('assistantProfilesDescription')}</p>
      {loadState === 'loading' || loadState === 'idle' ? (
        <p aria-live="polite" className="text-sm text-secondary-text">
          {t('modelsLoading')}
        </p>
      ) : loadState === 'error' ? (
        <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
          <p className="text-sm text-destructive">{t(errorKey ?? 'assistantProfilesLoadFailed')}</p>
          <Button onClick={retry} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : (
        <>
          {errorKey ? (
            <p className="text-sm text-destructive" role="alert">
              {t(errorKey)}
            </p>
          ) : null}
          {profiles.length === 0 ? (
            <Card className="flex items-center gap-3" padding="default" role="status">
              <Bot aria-hidden="true" className="text-secondary-icon" size={18} />
              <p className="text-sm text-secondary-text">{t('assistantProfilesEmpty')}</p>
            </Card>
          ) : (
            <div className="grid gap-2">
              {profiles.map((profile) => (
                <Card className="flex min-w-0 flex-wrap items-center gap-3" key={profile.id} padding="compact">
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-medium text-text">{profile.name}</p>
                    <p className="mt-1 text-xs text-secondary-text">
                      {t('assistantProfileStyle', { style: t(responseStyleLabelKey(profile.responseStyle)) })}
                      {profile.toolAllowlist === null
                        ? ` · ${t('assistantProfileToolAccessUsesAppPermissions')}`
                        : ` · ${t('assistantProfileAllowedToolCount', { count: profile.toolAllowlist.filter((name) => !retiredToolNames.has(name)).length })}`}
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-1">
                    <Button
                      aria-label={t('assistantProfileEdit')}
                      onClick={() => beginEdit(profile)}
                      size="icon"
                      variant="ghost"
                    >
                      <Pencil aria-hidden="true" size={16} />
                    </Button>
                    <Button
                      aria-label={t('assistantProfileRemove')}
                      onClick={() => setPendingRemoval(profile)}
                      size="icon"
                      variant="ghost"
                    >
                      <Trash2 aria-hidden="true" size={16} />
                    </Button>
                  </div>
                </Card>
              ))}
            </div>
          )}

          {draft ? (
            <Card className="grid gap-4" padding="default">
              <h2 className="text-lg font-semibold">
                {t(draft.id ? 'assistantProfileEdit' : 'assistantProfileCreate')}
              </h2>
              <Input
                autoFocus
                label={t('assistantProfileName')}
                maxLength={64}
                onChange={(event) => {
                  setDraft((current) => (current ? { ...current, name: event.target.value } : current));
                  setFormError(null);
                }}
                value={draft.name}
              />
              <Textarea
                className="min-h-32 max-h-64 resize-y bg-composer text-sm"
                description={t('assistantProfileInstructionsHelp')}
                id="assistant-profile-instructions"
                label={t('assistantProfileInstructions')}
                maxLength={16 * 1024}
                onChange={(event) => {
                  setDraft((current) => (current ? { ...current, instructions: event.target.value } : current));
                  setFormError(null);
                }}
                value={draft.instructions}
              />
              <Select
                label={t('assistantResponseStyle')}
                onChange={(event) => {
                  const responseStyle = parseResponseStyle(event.target.value);
                  if (responseStyle) setDraft((current) => (current ? { ...current, responseStyle } : current));
                }}
                options={[
                  { value: 'concise', label: t('assistantResponseStyleConcise') },
                  { value: 'balanced', label: t('assistantResponseStyleBalanced') },
                  { value: 'detailed', label: t('assistantResponseStyleDetailed') },
                ]}
                value={draft.responseStyle}
              />
              <div className="grid gap-2">
                <div className="flex items-center justify-between gap-3 text-sm font-medium text-text">
                  <span id="assistant-profile-restrict-tools-label">{t('assistantProfileRestrictTools')}</span>
                  <Switch
                    aria-label={t('assistantProfileRestrictTools')}
                    aria-describedby="assistant-profile-restrict-tools-help"
                    aria-labelledby="assistant-profile-restrict-tools-label"
                    checked={draft.restrictTools}
                    onCheckedChange={(checked) =>
                      setDraft((current) => (current ? { ...current, restrictTools: checked } : current))
                    }
                  />
                </div>
                <p className="text-xs text-secondary-text" id="assistant-profile-restrict-tools-help">
                  {t('assistantProfileRestrictToolsHelp')}
                </p>
              </div>
              {draft.restrictTools ? (
                <fieldset className="grid gap-2 rounded-control border border-border p-3">
                  <legend className="px-1 text-sm font-medium text-text">{t('assistantProfileRestrictTools')}</legend>
                  <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                    {builtInToolNames.map((toolName) => (
                      <label
                        className="flex min-w-0 items-start gap-2 text-sm text-text"
                        htmlFor={`assistant-profile-tool-${toolName}`}
                        key={toolName}
                      >
                        <Checkbox
                          checked={draft.toolAllowlist.includes(toolName)}
                          className="mt-0.5"
                          id={`assistant-profile-tool-${toolName}`}
                          onChange={() => toggleTool(toolName)}
                        />
                        <span>
                          {toolName === 'run_project_task' ? t('assistantProfileProjectTasks') : null}
                          {toolName === 'search_project_code' ? t('toolSearchProjectCode') : null}
                          {toolName === 'query_language_server' ? t('toolQueryLanguageServer') : null}
                          {toolName === 'run_project_task' ? (
                            <code className="ml-1 break-all text-xs">{toolName}</code>
                          ) : (
                            <code className="break-all text-xs">{toolName}</code>
                          )}
                        </span>
                      </label>
                    ))}
                  </div>
                  <div className="grid gap-2 border-t border-border pt-3">
                    <p className="text-sm font-medium text-text">{t('assistantProfileProjectTools')}</p>
                    {projectCatalogState === 'loading' ? (
                      <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                        {t('modelsLoading')}
                      </p>
                    ) : projectCatalogState === 'error' ? (
                      <div className="flex flex-wrap items-center gap-2" role="alert">
                        <p className="text-xs text-destructive">{t('assistantProfileProjectCatalogUnavailable')}</p>
                        <Button
                          onClick={() => setProjectCatalogAttempt((attempt) => attempt + 1)}
                          size="small"
                          variant="secondary"
                        >
                          {t('retry')}
                        </Button>
                      </div>
                    ) : activeProjectId === null ? (
                      <p className="text-xs text-secondary-text">{t('assistantProfileProjectContextRequired')}</p>
                    ) : projectToolNames.length === 0 && mcpServers.length === 0 ? (
                      <p className="text-xs text-secondary-text">{t('assistantProfileNoProjectTools')}</p>
                    ) : null}
                    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                      {projectToolNames.map((toolName) => (
                        <ProfileToolCheckbox
                          key={toolName}
                          name={toolName}
                          label={t('toolConfiguredProjectTool', { name: toolName.slice('project_tool__'.length) })}
                          checked={draft.toolAllowlist.includes(toolName)}
                          onChange={() => toggleTool(toolName)}
                        />
                      ))}
                      {mcpToolNames.map((toolName) => (
                        <ProfileToolCheckbox
                          key={toolName}
                          name={toolName}
                          label={toolName}
                          checked={draft.toolAllowlist.includes(toolName)}
                          onChange={() => toggleTool(toolName)}
                        />
                      ))}
                    </div>
                    {mcpServers.length > 0 ? (
                      <div className="grid justify-items-start gap-2">
                        <Button
                          disabled={mcpToolsState === 'loading' || projectCatalogState !== 'ready'}
                          onClick={() => void loadMcpTools()}
                          size="small"
                          variant="secondary"
                        >
                          {mcpToolsState === 'loading'
                            ? t('assistantProfileLoadingMcpTools')
                            : t('assistantProfileLoadMcpTools')}
                        </Button>
                        {mcpToolsPartialFailure || mcpToolsState === 'error' ? (
                          <div className="grid gap-1" role="alert">
                            <p className="text-xs text-destructive">{t('assistantProfileMcpUnavailable')}</p>
                            {mcpToolsErrorDetails ? (
                              <p className="break-words text-xs text-secondary-text">
                                {t('chatErrorDetails')}: {mcpToolsErrorDetails}
                              </p>
                            ) : null}
                          </div>
                        ) : null}
                      </div>
                    ) : null}
                    {preservedProfileTools.length > 0 ? (
                      <>
                        <p className="text-xs font-medium text-secondary-text">
                          {t('assistantProfileUnavailableTools')}
                        </p>
                        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                          {[...new Set(preservedProfileTools)].map((toolName) => (
                            <ProfileToolCheckbox
                              key={toolName}
                              name={toolName}
                              label={toolName}
                              checked={draft.toolAllowlist.includes(toolName)}
                              onChange={() => toggleTool(toolName)}
                            />
                          ))}
                        </div>
                      </>
                    ) : null}
                  </div>
                  {draft.toolAllowlist.length === 0 ? (
                    <p className="text-xs text-secondary-text">{t('assistantProfileToolAccessDisabledHelp')}</p>
                  ) : null}
                </fieldset>
              ) : null}
              {formError ? (
                <p className="text-sm text-destructive" role="alert">
                  {t(formError)}
                </p>
              ) : null}
              <div className="flex flex-wrap justify-end gap-2">
                <Button disabled={saveState === 'saving'} onClick={() => setDraft(null)} variant="secondary">
                  {t('cancel')}
                </Button>
                <Button loading={saveState === 'saving'} onClick={() => void saveDraft()} variant="primary">
                  {t('save')}
                </Button>
              </div>
            </Card>
          ) : (
            <Button
              className="justify-self-start"
              disabled={profiles.length >= 20 || saveState === 'saving'}
              onClick={() => {
                setFormError(null);
                setDraft(emptyDraft);
              }}
              size="small"
              variant="secondary"
            >
              <Plus aria-hidden="true" size={16} />
              {t('assistantProfileCreate')}
            </Button>
          )}
        </>
      )}
      {showReadyAgents ? (
        <div className="grid gap-3 border-t border-border pt-4">
          <div>
            <h2 className="text-base font-semibold text-text">{t('assistantReadyAgents')}</h2>
            <p className="mt-1 text-sm text-secondary-text">{t('assistantReadyAgentsDescription')}</p>
          </div>
          {loadState === 'ready' && profiles.length >= 20 ? (
            <p className="text-sm text-secondary-text" role="status">
              {t('assistantProfilesLimitReached')}
            </p>
          ) : null}
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {readyAssistantAgentTemplates.map((template) => {
              const name = t(template.nameKey);
              const alreadyExists = profiles.some(
                (profile) =>
                  profile.templateId === template.id ||
                  profile.name.toLocaleLowerCase('en-US') === name.toLocaleLowerCase('en-US'),
              );
              const buttonLabel = t(alreadyExists ? 'assistantReadyAgentAlreadyExists' : 'assistantReadyAgentAdd');
              return (
                <Card className="grid content-start gap-3" key={template.id} padding="default">
                  <div className="flex min-w-0 items-center gap-2">
                    <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={17} />
                    <h3 className="min-w-0 truncate font-medium text-text">{name}</h3>
                  </div>
                  <p className="text-sm text-secondary-text">{t(template.descriptionKey)}</p>
                  <div className="flex flex-wrap gap-2">
                    <Button onClick={() => setViewingReadyTemplate(template)} size="small" variant="secondary">
                      {t('assistantReadyAgentViewInstructions')}
                    </Button>
                    <Button
                      aria-label={`${buttonLabel}: ${name}`}
                      disabled={
                        loadState !== 'ready' || profiles.length >= 20 || alreadyExists || saveState === 'saving'
                      }
                      loading={saveState === 'saving'}
                      onClick={() => void addReadyAgent(template)}
                      size="small"
                      variant="secondary"
                    >
                      {buttonLabel}
                    </Button>
                  </div>
                </Card>
              );
            })}
          </div>
        </div>
      ) : null}
      <Dialog
        size="wide"
        onOpenChange={(open) => {
          if (!open) setViewingReadyTemplate(null);
        }}
        open={viewingReadyTemplate !== null}
        title={
          viewingReadyTemplate
            ? t('assistantReadyAgentInstructionsTitle', {
                agentName: t(viewingReadyTemplate.nameKey),
              })
            : t('assistantReadyAgents')
        }
      >
        {viewingReadyTemplate ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">{t('assistantReadyAgentInstructionsSupplement')}</p>
            <div className="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer p-3 text-sm leading-6 text-text">
              {viewingReadyTemplate.instructions}
            </div>
            <div className="flex justify-end">
              <Button onClick={() => setViewingReadyTemplate(null)} variant="secondary">
                {t('close')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
      <Dialog
        onOpenChange={(open) => {
          if (!open) setPendingRemoval(null);
        }}
        open={pendingRemoval !== null}
        title={t('assistantProfileRemoveTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">
            {t('assistantProfileRemoveConfirmation', { profileName: pendingRemoval?.name ?? '' })}
          </p>
          <div className="flex justify-end gap-2">
            <Button onClick={() => setPendingRemoval(null)} variant="secondary">
              {t('cancel')}
            </Button>
            <Button
              onClick={() => {
                if (pendingRemoval) {
                  void removeProfile(pendingRemoval.id).then((removed) => {
                    if (removed) setPendingRemoval(null);
                  });
                }
              }}
              variant="destructive"
            >
              <Trash2 aria-hidden="true" size={16} />
              {t('assistantProfileRemove')}
            </Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}

function readServiceErrorDetails(error: unknown): string | null {
  const value = error instanceof LocalServiceError ? (error.details ?? error.message) : error;
  const details = value instanceof Error ? value.message : typeof value === 'string' ? value : null;
  return details?.trim().slice(0, 1000) || null;
}

function ProfileToolCheckbox({
  checked,
  label,
  name,
  onChange,
}: {
  checked: boolean;
  label: string;
  name: string;
  onChange(): void;
}) {
  const checkboxId = `assistant-profile-tool-${name}`;

  return (
    <label className="flex min-w-0 items-start gap-2 text-sm text-text" htmlFor={checkboxId}>
      <Checkbox checked={checked} className="mt-0.5" id={checkboxId} onChange={onChange} />
      <span className="min-w-0">
        <span>{label}</span>
        {label === name ? null : <code className="ml-1 break-all text-xs text-secondary-text">{name}</code>}
      </span>
    </label>
  );
}

function parseProjectToolNames(value: unknown): string[] {
  if (!isRecord(value) || !Array.isArray(value.tools) || value.tools.length > 32) {
    throw new Error('The project tool list is invalid.');
  }
  const ids = value.tools.map((tool) => {
    if (!isRecord(tool) || typeof tool.id !== 'string' || !/^[a-z0-9_-]{1,48}$/.test(tool.id)) {
      throw new Error('The project tool list is invalid.');
    }
    return `project_tool__${tool.id}`;
  });
  if (new Set(ids).size !== ids.length) throw new Error('The project tool list is invalid.');
  return ids;
}

function parseCheckedMcpToolNames(value: unknown, serverId: string): string[] {
  if (
    !isRecord(value) ||
    value.status !== 'connected' ||
    typeof value.toolCount !== 'number' ||
    !Number.isInteger(value.toolCount) ||
    value.toolCount < 0 ||
    value.toolCount > 128 ||
    !isStringArray(value.toolNames)
  ) {
    throw new Error('The MCP tool list is invalid.');
  }
  const prefix = `mcp__${serverId}__`;
  const names = value.toolNames;
  if (
    names.length !== value.toolCount ||
    names.some(
      (name) => name.length > 64 || !name.startsWith(prefix) || !/^[A-Za-z0-9_-]+$/.test(name.slice(prefix.length)),
    )
  ) {
    throw new Error('The MCP tool list is invalid.');
  }
  if (new Set(names).size !== names.length) throw new Error('The MCP tool list is invalid.');
  return names;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function responseStyleLabelKey(
  value: AssistantResponseStyle,
): 'assistantResponseStyleConcise' | 'assistantResponseStyleBalanced' | 'assistantResponseStyleDetailed' {
  if (value === 'concise') return 'assistantResponseStyleConcise';
  if (value === 'balanced') return 'assistantResponseStyleBalanced';
  return 'assistantResponseStyleDetailed';
}

function parseResponseStyle(value: string): AssistantResponseStyle | null {
  return value === 'concise' || value === 'balanced' || value === 'detailed' ? value : null;
}
