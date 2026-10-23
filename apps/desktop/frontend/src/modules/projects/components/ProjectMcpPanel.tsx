import { KeyRound, Pencil, Plus, Radio, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Input, Select, Switch } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import {
  type ProjectMcpServer,
  type ProjectMcpTransport,
  parseProjectMcpCatalog,
  validateProjectMcpServer,
} from '@/modules/projects/data/projectMcp';
import {
  loadProjectToolPermissionRules,
  saveProjectToolPermissionRules,
} from '@/modules/projects/data/projectPermissionSettings';
import type { ProjectToolPermissionRule } from '@/modules/projects/data/projectPreferences';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';
import { ProjectMcpCredentialDialog } from './ProjectMcpCredentialDialog';

interface ProjectMcpPanelProps {
  project: ChatProject;
  onClose(): void;
}

type PermissionChoice = ProjectToolPermissionRule | 'inherit';
type TranslationKey = keyof typeof import('@/localization/locales/en.json');
type McpError =
  | 'projectMcpLoadFailed'
  | 'projectMcpSaveFailed'
  | 'projectMcpCredentialUnavailable'
  | 'projectMcpDuplicateId'
  | 'connectorPackageCleanupFailed';
type McpEditorError =
  | 'projectMcpInvalidId'
  | 'projectMcpProgramRequired'
  | 'projectMcpEndpointRequired'
  | 'projectMcpEnvironmentVariablesInvalid'
  | 'projectMcpSaveFailed';
interface McpCheckResult {
  message: string;
  details: string | null;
}

export function ProjectMcpPanel({ project, onClose }: ProjectMcpPanelProps) {
  const { t } = useTranslation();
  const mounted = useRef(false);
  const projectGeneration = useRef(0);
  const [servers, setServers] = useState<ProjectMcpServer[]>([]);
  const [savedServers, setSavedServers] = useState<ProjectMcpServer[]>([]);
  const [rules, setRules] = useState<Record<string, ProjectToolPermissionRule>>({});
  const [toolNames, setToolNames] = useState<Record<string, string[]>>({});
  const [checkResults, setCheckResults] = useState<Record<string, McpCheckResult>>({});
  const [checkingServerId, setCheckingServerId] = useState<string | null>(null);
  const [credentialServer, setCredentialServer] = useState<ProjectMcpServer | null>(null);
  const [editing, setEditing] = useState<{ server: ProjectMcpServer | null; rule: PermissionChoice } | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorKey, setErrorKey] = useState<McpError | null>(null);
  const [errorDetails, setErrorDetails] = useState<string | null>(null);
  const [packageCleanupPending, setPackageCleanupPending] = useState<
    Array<{ id: string; package: NonNullable<ProjectMcpServer['package']> }>
  >([]);

  const loadCatalog = useCallback(async () => {
    const generation = projectGeneration.current;
    const projectId = project.id;
    setIsLoading(true);
    setErrorKey(null);
    setErrorDetails(null);
    try {
      const [response, savedRules] = await Promise.all([
        localServiceClient.call('project.mcp.catalog.get', { projectId }),
        loadProjectToolPermissionRules(projectId),
      ]);
      const catalog = parseProjectMcpCatalog(response);
      if (!mounted.current || generation !== projectGeneration.current) return;
      setServers(catalog.servers);
      setSavedServers(catalog.servers);
      setRules(savedRules);
      setToolNames(deriveToolNamesFromRules(catalog.servers, savedRules));
      setCheckResults({});
      setIsLoading(false);
    } catch (error) {
      if (mounted.current && generation === projectGeneration.current) {
        setToolNames({});
        setCheckResults({});
        setErrorKey('projectMcpLoadFailed');
        setErrorDetails(readErrorDetails(error));
        setIsLoading(false);
      }
    }
  }, [project.id]);

  useEffect(() => {
    const generation = projectGeneration.current + 1;
    projectGeneration.current = generation;
    mounted.current = true;
    setIsLoading(true);
    setIsSaving(false);
    setCheckingServerId(null);
    setCredentialServer(null);
    setEditing(null);
    setServers([]);
    setSavedServers([]);
    setRules({});
    setToolNames({});
    setCheckResults({});
    setErrorKey(null);
    setErrorDetails(null);
    setPackageCleanupPending([]);
    void loadCatalog();
    return () => {
      mounted.current = false;
      if (projectGeneration.current === generation) projectGeneration.current += 1;
    };
  }, [loadCatalog]);

  const updateRule = (name: string, choice: PermissionChoice) => {
    setRules((current) => {
      const next = { ...current };
      if (choice === 'inherit') delete next[name];
      else next[name] = choice;
      return next;
    });
    setErrorKey(null);
    setErrorDetails(null);
  };

  const saveCatalog = async () => {
    if (isLoading || isSaving || errorKey === 'projectMcpLoadFailed') return;
    const generation = projectGeneration.current;
    const projectId = project.id;
    setIsSaving(true);
    setErrorKey(null);
    setErrorDetails(null);
    try {
      const catalog = { version: 1, servers };
      await localServiceClient.call('project.mcp.catalog.save', { projectId, catalog });
      await removeStaleCredentials(projectId, savedServers, servers);
      const nextRules = Object.fromEntries(
        Object.entries(rules).filter(
          ([name]) => !name.startsWith('mcp__') || servers.some((server) => name.startsWith(`mcp__${server.id}__`)),
        ),
      );
      await saveProjectToolPermissionRules(projectId, nextRules);
      const removedPackages = savedServers.flatMap((previous) => {
        if (previous.package === undefined) return [];
        const current = servers.find((server) => server.id === previous.id);
        if (current?.package && samePackage(current.package, previous.package)) return [];
        return [{ id: previous.id, package: previous.package }];
      });
      const cleanupFailures: typeof removedPackages = [];
      let cleanupFailureDetails: string | null = null;
      for (const packageReference of removedPackages) {
        try {
          await removeInstalledMcpPackage(projectId, packageReference.id, packageReference.package);
        } catch (error) {
          cleanupFailures.push(packageReference);
          cleanupFailureDetails ??= readErrorDetails(error);
        }
      }
      if (mounted.current && generation === projectGeneration.current) {
        setSavedServers(servers);
        setPackageCleanupPending(cleanupFailures);
        if (cleanupFailures.length > 0) {
          setErrorKey('connectorPackageCleanupFailed');
          setErrorDetails(cleanupFailureDetails);
        } else {
          onClose();
        }
      }
    } catch (error) {
      if (mounted.current && generation === projectGeneration.current) {
        setErrorKey(
          error instanceof CredentialCleanupError ? 'projectMcpCredentialUnavailable' : 'projectMcpSaveFailed',
        );
        setErrorDetails(error instanceof CredentialCleanupError ? error.details : readErrorDetails(error));
      }
    } finally {
      if (mounted.current && generation === projectGeneration.current) setIsSaving(false);
    }
  };

  const checkServer = async (server: ProjectMcpServer) => {
    if (checkingServerId || isSaving || !server.enabled) return;
    const generation = projectGeneration.current;
    const projectId = project.id;
    setCheckingServerId(server.id);
    setToolNames((current) => omitKey(current, server.id));
    setCheckResults((current) => omitKey(current, server.id));
    try {
      const response = await localServiceClient.call('project.mcp.server.check', { projectId, server }, 40_000);
      const toolCount = response.toolCount;
      const rawToolNames = response.toolNames;
      if (
        response.status !== 'connected' ||
        typeof toolCount !== 'number' ||
        !Number.isInteger(toolCount) ||
        toolCount < 0 ||
        !isMcpToolNames(rawToolNames, toolCount)
      )
        throw new Error('The MCP server check result is invalid.');
      const names = [...new Set(rawToolNames)].sort((left, right) => left.localeCompare(right));
      if (mounted.current && generation === projectGeneration.current) {
        setToolNames((current) => ({ ...current, [server.id]: names }));
        setCheckResults((current) => ({
          ...current,
          [server.id]: { message: t('projectMcpConnected', { count: toolCount }), details: null },
        }));
      }
    } catch (error) {
      if (mounted.current && generation === projectGeneration.current) {
        setToolNames((current) => omitKey(current, server.id));
        setCheckResults((current) => ({
          ...current,
          [server.id]: { message: t('projectMcpCheckFailed'), details: readErrorDetails(error) },
        }));
      }
    } finally {
      if (mounted.current && generation === projectGeneration.current) setCheckingServerId(null);
    }
  };

  const replaceServer = (index: number, server: ProjectMcpServer) => {
    setServers((current) => current.map((item, itemIndex) => (itemIndex === index ? server : item)));
  };

  const saveEditedServer = (server: ProjectMcpServer, permission: PermissionChoice) => {
    const editIndex = editing?.server ? servers.findIndex((item) => item.id === editing.server?.id) : -1;
    if (servers.some((item, index) => item.id === server.id && index !== editIndex)) {
      setErrorKey('projectMcpDuplicateId');
      return;
    }
    const nextServers =
      editIndex < 0 ? [...servers, server] : servers.map((item, index) => (index === editIndex ? server : item));
    if (editIndex >= 0) {
      const previousServer = servers[editIndex];
      if (!previousServer) {
        setErrorKey('projectMcpSaveFailed');
        return;
      }
      setToolNames((current) => omitKey(current, previousServer.id));
      setCheckResults((current) => omitKey(current, previousServer.id));
    }
    setToolNames((current) => omitKey(current, server.id));
    setCheckResults((current) => omitKey(current, server.id));
    setServers(nextServers);
    setEditing(null);
    if (permission !== 'inherit') updateRule(`mcp__${server.id}__*`, permission);
    else updateRule(`mcp__${server.id}__*`, 'inherit');
    setErrorKey(null);
    setErrorDetails(null);
  };

  const removeServer = (server: ProjectMcpServer) => {
    setServers((current) => current.filter((item) => item.id !== server.id));
    setToolNames((current) => omitKey(current, server.id));
    setCheckResults((current) => omitKey(current, server.id));
    setRules((current) =>
      Object.fromEntries(Object.entries(current).filter(([name]) => !name.startsWith(`mcp__${server.id}__`))),
    );
  };

  return (
    <div className="grid gap-3">
      <p className="text-sm text-secondary-text">{t('projectMcpDescription')}</p>
      <p className="text-xs text-secondary-text">{t('projectMcpPermissionsLocal')}</p>
      <p className="text-xs text-secondary-text">{t('projectMcpNoCredentials')}</p>
      {isLoading ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('projectMcpLoading')}
        </p>
      ) : errorKey === 'projectMcpLoadFailed' ? (
        <div className="grid gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t(errorKey)}
          </p>
          {errorDetails ? (
            <p className="break-words text-xs text-secondary-text">
              {t('chatErrorDetails')}: {errorDetails}
            </p>
          ) : null}
          <Button className="justify-self-start" onClick={() => void loadCatalog()} size="small">
            {t('retry')}
          </Button>
        </div>
      ) : (
        <div className="openchat-scroll-area grid max-h-project-mcp-list gap-3 overflow-y-auto pr-1">
          {servers.length === 0 ? (
            <p className="py-4 text-center text-sm text-secondary-text">{t('projectMcpEmpty')}</p>
          ) : null}
          {servers.map((server, index) => (
            <ProjectMcpServerRow
              checking={checkingServerId === server.id}
              checkResult={checkResults[server.id]}
              disabled={isSaving || checkingServerId !== null || editing !== null}
              key={server.id}
              onCheck={() => void checkServer(server)}
              onEdit={() => setEditing({ server, rule: rules[`mcp__${server.id}__*`] ?? 'inherit' })}
              onManageCredentials={() => setCredentialServer(server)}
              onPermissionChange={(value) => updateRule(`mcp__${server.id}__*`, value)}
              onRemove={() => removeServer(server)}
              onToggle={(enabled) => replaceServer(index, { ...server, enabled })}
              permission={rules[`mcp__${server.id}__*`] ?? 'inherit'}
              server={server}
              toolNames={toolNames[server.id] ?? []}
              toolRules={rules}
              onToolPermissionChange={updateRule}
            />
          ))}
        </div>
      )}
      {editing ? (
        <ProjectMcpServerEditor
          key={editing.server?.id ?? 'new'}
          onCancel={() => setEditing(null)}
          onSave={saveEditedServer}
          server={editing.server}
          permission={editing.rule}
        />
      ) : null}
      {errorKey && errorKey !== 'projectMcpLoadFailed' ? (
        <div className="grid gap-1" role="alert">
          <p className="text-sm text-destructive">{t(errorKey)}</p>
          {errorDetails ? (
            <p className="break-words text-xs text-secondary-text">
              {t('chatErrorDetails')}: {errorDetails}
            </p>
          ) : null}
          {packageCleanupPending.length > 0 ? (
            <Button
              disabled={isSaving}
              onClick={() => {
                const pending = packageCleanupPending;
                setIsSaving(true);
                void cleanupInstalledMcpPackages(project.id, pending)
                  .then(() => {
                    setPackageCleanupPending([]);
                    setErrorKey(null);
                    onClose();
                  })
                  .catch((error: unknown) => {
                    setErrorKey('connectorPackageCleanupFailed');
                    setErrorDetails(readErrorDetails(error));
                  })
                  .finally(() => setIsSaving(false));
              }}
              size="small"
              variant="secondary"
            >
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}
      <div className="flex flex-wrap justify-between gap-2">
        <Button
          disabled={
            isSaving ||
            isLoading ||
            editing !== null ||
            packageCleanupPending.length > 0 ||
            errorKey === 'projectMcpLoadFailed' ||
            servers.length >= 8
          }
          onClick={() => setEditing({ server: null, rule: 'inherit' })}
          size="small"
        >
          <Plus aria-hidden="true" size={15} />
          {t('projectMcpAdd')}
        </Button>
        <div className="flex justify-end gap-2">
          <Button disabled={isSaving || packageCleanupPending.length > 0} onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            disabled={isSaving || isLoading || packageCleanupPending.length > 0 || errorKey === 'projectMcpLoadFailed'}
            onClick={() => void saveCatalog()}
            variant="primary"
          >
            {isSaving ? t('saving') : t('save')}
          </Button>
        </div>
      </div>
      <ProjectMcpCredentialDialog
        onOpenChange={(open) => {
          if (!open) setCredentialServer(null);
        }}
        open={credentialServer !== null}
        projectId={project.id}
        server={credentialServer}
      />
    </div>
  );
}

interface ProjectMcpServerRowProps {
  server: ProjectMcpServer;
  permission: PermissionChoice;
  toolNames: readonly string[];
  toolRules: Readonly<Record<string, ProjectToolPermissionRule>>;
  checkResult?: McpCheckResult;
  checking: boolean;
  disabled: boolean;
  onToggle(enabled: boolean): void;
  onPermissionChange(permission: PermissionChoice): void;
  onToolPermissionChange(name: string, permission: PermissionChoice): void;
  onCheck(): void;
  onEdit(): void;
  onRemove(): void;
  onManageCredentials(): void;
}

function ProjectMcpServerRow({
  server,
  permission,
  toolNames,
  toolRules,
  checkResult,
  checking,
  disabled,
  onToggle,
  onPermissionChange,
  onToolPermissionChange,
  onCheck,
  onEdit,
  onRemove,
  onManageCredentials,
}: ProjectMcpServerRowProps) {
  const { t } = useTranslation();
  const endpoint = server.transport === 'stdio' ? server.program : server.endpoint;
  const credentialNames = getCredentialNames(server);
  return (
    <article className="grid gap-2 rounded-card border border-border bg-surface p-3">
      <div className="flex min-w-0 items-start gap-3">
        <Switch
          aria-label={`${t('projectMcpEnabled')}: ${server.id}`}
          checked={server.enabled}
          disabled={disabled}
          onCheckedChange={onToggle}
        />
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-medium text-text">{server.id}</h3>
          <p className="break-all text-xs text-secondary-text">{endpoint}</p>
          {server.arguments.length > 0 ? (
            <p className="text-xs text-secondary-text">
              {t('projectMcpArguments')}: {server.arguments.join(' ')}
            </p>
          ) : null}
          {credentialNames.length > 0 ? (
            <p className="text-xs text-secondary-text">
              {t('projectMcpEnvironmentVariables')}: {credentialNames.join(', ')}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 flex-wrap gap-1">
          <Button
            aria-label={t('projectMcpCredentials')}
            disabled={disabled || credentialNames.length === 0}
            onClick={onManageCredentials}
            size="icon"
            variant="ghost"
          >
            <KeyRound aria-hidden="true" size={15} />
          </Button>
          <Button
            aria-label={t('projectMcpCheck')}
            disabled={disabled || !server.enabled}
            onClick={onCheck}
            size="icon"
            variant="ghost"
          >
            {checking ? (
              <span
                aria-hidden="true"
                className="size-4 animate-spin rounded-full border-2 border-current border-r-transparent"
              />
            ) : (
              <Radio aria-hidden="true" size={15} />
            )}
          </Button>
          <Button aria-label={t('projectMcpEdit')} disabled={disabled} onClick={onEdit} size="icon" variant="ghost">
            <Pencil aria-hidden="true" size={15} />
          </Button>
          <Button aria-label={t('projectMcpRemove')} disabled={disabled} onClick={onRemove} size="icon" variant="ghost">
            <Trash2 aria-hidden="true" size={15} />
          </Button>
        </div>
      </div>
      <Select
        label={t('projectMcpPermissionScope')}
        disabled={disabled}
        onChange={(event) => onPermissionChange(readPermissionChoice(event.target.value))}
        options={permissionOptions(t)}
        value={permission}
      />
      {toolNames.map((name) => (
        <Select
          key={name}
          label={`${name} · ${t('projectMcpPermissionScope')}`}
          disabled={disabled || permission === 'deny'}
          onChange={(event) =>
            onToolPermissionChange(`mcp__${server.id}__${name}`, readPermissionChoice(event.target.value))
          }
          options={permissionOptions(t)}
          value={toolRules[`mcp__${server.id}__${name}`] ?? 'inherit'}
        />
      ))}
      {checkResult ? (
        <div aria-live="polite" className="grid gap-1">
          <p className={`text-xs ${checkResult.details ? 'text-destructive' : 'text-secondary-text'}`}>
            {checkResult.message}
          </p>
          {checkResult.details ? (
            <p className="break-words text-xs text-secondary-text">
              {t('chatErrorDetails')}: {checkResult.details}
            </p>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

interface ProjectMcpServerEditorProps {
  server: ProjectMcpServer | null;
  permission: PermissionChoice;
  onSave(server: ProjectMcpServer, permission: PermissionChoice): void;
  onCancel(): void;
}

function ProjectMcpServerEditor({ server, permission, onSave, onCancel }: ProjectMcpServerEditorProps) {
  const { t } = useTranslation();
  const [id, setId] = useState(server?.id ?? '');
  const [enabled, setEnabled] = useState(server?.enabled ?? true);
  const [transport, setTransport] = useState<ProjectMcpTransport>(server?.transport ?? 'stdio');
  const [program, setProgram] = useState(server?.program ?? '');
  const [argumentsText, setArgumentsText] = useState(server?.arguments.join('\n') ?? '');
  const [environmentText, setEnvironmentText] = useState(server?.environmentVariables.join('\n') ?? '');
  const [endpoint, setEndpoint] = useState(server?.endpoint ?? '');
  const [authEnvironmentVariable, setAuthEnvironmentVariable] = useState(server?.authEnvironmentVariable ?? '');
  const [permissionRule, setPermissionRule] = useState<PermissionChoice>(permission);
  const [errorKey, setErrorKey] = useState<McpEditorError | null>(null);

  const submit = () => {
    const environmentVariables = environmentText
      .split(/\r?\n/)
      .map((name) => name.trim())
      .filter(Boolean);
    const argumentsList = argumentsText.split(/\r?\n/).filter((argument) => argument.length > 0);
    const nextServer: ProjectMcpServer = {
      id: id.trim(),
      enabled,
      transport,
      arguments: transport === 'stdio' ? argumentsList : [],
      environmentVariables: transport === 'stdio' ? environmentVariables : [],
      ...(transport === 'stdio' && server?.package === undefined ? { program: program.trim() } : {}),
      ...(transport === 'stdio' && server?.package !== undefined ? { package: server.package } : {}),
      ...(transport === 'streamableHttp' ? { endpoint: endpoint.trim() } : {}),
      ...(transport === 'streamableHttp' && authEnvironmentVariable.trim()
        ? { authEnvironmentVariable: authEnvironmentVariable.trim() }
        : {}),
    };
    try {
      validateProjectMcpServer(nextServer);
    } catch (error) {
      const code = error instanceof Error ? error.message : 'invalid_server';
      setErrorKey(
        code === 'invalid_id'
          ? 'projectMcpInvalidId'
          : code === 'program_required'
            ? 'projectMcpProgramRequired'
            : code === 'endpoint_required'
              ? 'projectMcpEndpointRequired'
              : code === 'environment_variables_invalid'
                ? 'projectMcpEnvironmentVariablesInvalid'
                : 'projectMcpSaveFailed',
      );
      return;
    }
    onSave(nextServer, permissionRule);
  };

  return (
    <section
      className="grid gap-3 rounded-card border border-border bg-raised p-3"
      aria-label={t(server ? 'projectMcpEdit' : 'projectMcpAdd')}
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Input
          autoComplete="off"
          disabled={server !== null}
          label={t('projectMcpServerId')}
          maxLength={24}
          onChange={(event) => setId(event.target.value)}
          value={id}
        />
        <Select
          disabled={server?.package !== undefined}
          label={t('projectMcpTransport')}
          onChange={(event) => {
            if (event.target.value === 'stdio' || event.target.value === 'streamableHttp')
              setTransport(event.target.value);
          }}
          options={[
            { value: 'stdio', label: t('projectMcpTransportStdio') },
            { value: 'streamableHttp', label: t('projectMcpTransportHttp') },
          ]}
          value={transport}
        />
      </div>
      {transport === 'stdio' ? (
        <>
          <Input
            autoComplete="off"
            disabled={server?.package !== undefined}
            label={t('projectMcpProgram')}
            onChange={(event) => setProgram(event.target.value)}
            value={server?.package ? t('projectMcpManagedPackage', { version: server.package.version }) : program}
          />
          {server?.package ? (
            <p className="text-xs text-secondary-text">{server.package.sourceKeyFingerprint}</p>
          ) : null}
          <label className="grid gap-2 text-sm font-medium text-text">
            {t('projectMcpArguments')}
            <textarea
              className="min-h-20 rounded-control border border-border bg-raised px-3 py-2 text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
              onChange={(event) => setArgumentsText(event.target.value)}
              value={argumentsText}
            />
            <span className="text-xs font-normal text-secondary-text">{t('projectMcpArgumentsHint')}</span>
          </label>
          <label className="grid gap-2 text-sm font-medium text-text">
            {t('projectMcpEnvironmentVariables')}
            <textarea
              className="min-h-16 rounded-control border border-border bg-raised px-3 py-2 text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
              onChange={(event) => setEnvironmentText(event.target.value)}
              value={environmentText}
            />
            <span className="text-xs font-normal text-secondary-text">{t('projectMcpEnvironmentVariablesHint')}</span>
          </label>
        </>
      ) : (
        <>
          <Input
            autoComplete="off"
            label={t('projectMcpEndpoint')}
            onChange={(event) => setEndpoint(event.target.value)}
            type="url"
            value={endpoint}
          />
          <p className="text-xs text-secondary-text">{t('projectMcpEndpointHint')}</p>
          <Input
            autoComplete="off"
            label={t('projectMcpAuthVariable')}
            onChange={(event) => setAuthEnvironmentVariable(event.target.value)}
            value={authEnvironmentVariable}
          />
          <p className="text-xs text-secondary-text">{t('projectMcpAuthVariableHint')}</p>
        </>
      )}
      <div className="flex items-center justify-between gap-3">
        <label className="text-sm text-text" htmlFor="project-mcp-enabled">
          {t('projectMcpEnabled')}
        </label>
        <Switch
          aria-label={t('projectMcpEnabled')}
          checked={enabled}
          id="project-mcp-enabled"
          onCheckedChange={setEnabled}
        />
      </div>
      <Select
        label={t('projectMcpPermissionScope')}
        onChange={(event) => setPermissionRule(readPermissionChoice(event.target.value))}
        options={permissionOptions(t)}
        value={permissionRule}
      />
      {errorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button onClick={onCancel}>{t('cancel')}</Button>
        <Button onClick={submit} variant="primary">
          {t('save')}
        </Button>
      </div>
    </section>
  );
}

function permissionOptions(t: (key: TranslationKey) => string) {
  return [
    { value: 'inherit', label: t('projectToolRuleInherit') },
    { value: 'ask', label: t('projectToolRuleAsk') },
    { value: 'allow', label: t('projectToolRuleAllow') },
    { value: 'deny', label: t('projectToolRuleDeny') },
  ];
}

function readPermissionChoice(value: string): PermissionChoice {
  if (value === 'inherit' || value === 'ask' || value === 'allow' || value === 'deny') return value;
  throw new Error('The selected MCP permission is invalid.');
}

async function removeInstalledMcpPackage(
  projectId: string,
  id: string,
  packageReference: NonNullable<ProjectMcpServer['package']>,
): Promise<void> {
  await localServiceClient.call('connector.package.remove', {
    projectId,
    id,
    package: packageReference,
  });
}

async function cleanupInstalledMcpPackages(
  projectId: string,
  packages: ReadonlyArray<{ id: string; package: NonNullable<ProjectMcpServer['package']> }>,
): Promise<void> {
  for (const packageReference of packages) {
    await removeInstalledMcpPackage(projectId, packageReference.id, packageReference.package);
  }
}

function samePackage(
  first: NonNullable<ProjectMcpServer['package']>,
  second: NonNullable<ProjectMcpServer['package']>,
): boolean {
  return (
    first.version === second.version &&
    first.sha256 === second.sha256 &&
    first.sourceKeyFingerprint === second.sourceKeyFingerprint &&
    first.sizeBytes === second.sizeBytes
  );
}

function isMcpToolNames(value: unknown, expectedCount: number): value is string[] {
  return (
    Array.isArray(value) &&
    value.length === expectedCount &&
    value.every((name: unknown) => typeof name === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(name))
  );
}

function deriveToolNamesFromRules(
  servers: readonly ProjectMcpServer[],
  savedRules: Readonly<Record<string, ProjectToolPermissionRule>>,
): Record<string, string[]> {
  const next: Record<string, string[]> = {};
  for (const server of servers) {
    const prefix = `mcp__${server.id}__`;
    const names = new Set(
      Object.keys(savedRules)
        .filter((name) => name.startsWith(prefix) && !name.endsWith('__*'))
        .map((name) => name.slice(prefix.length))
        .filter((name) => /^[A-Za-z0-9_-]{1,64}$/.test(name)),
    );
    next[server.id] = [...names].sort((left, right) => left.localeCompare(right));
  }
  return next;
}

function getCredentialNames(server: ProjectMcpServer): string[] {
  return [
    ...new Set([
      ...server.environmentVariables,
      ...(server.authEnvironmentVariable ? [server.authEnvironmentVariable] : []),
    ]),
  ];
}

async function removeStaleCredentials(
  projectId: string,
  previousServers: readonly ProjectMcpServer[],
  currentServers: readonly ProjectMcpServer[],
): Promise<void> {
  for (const previous of previousServers) {
    const current = currentServers.find((server) => server.id === previous.id);
    const currentNames = current ? getCredentialNames(current) : [];
    for (const name of getCredentialNames(previous)) {
      if (currentNames.includes(name)) continue;
      try {
        await localServiceClient.call('project.mcp.credential.remove', {
          projectId,
          serverId: previous.id,
          environmentName: name,
        });
      } catch (error) {
        throw new CredentialCleanupError(readErrorDetails(error));
      }
    }
  }
}

class CredentialCleanupError extends Error {
  constructor(readonly details: string | null) {
    super('The stored MCP credential could not be removed.');
  }
}

function readErrorDetails(error: unknown): string | null {
  const value = error instanceof LocalServiceError ? (error.details ?? error.message) : error;
  if (typeof value !== 'string' && !(value instanceof Error)) return null;
  const details = typeof value === 'string' ? value : value.message;
  return details.trim().slice(0, 1000) || null;
}

function omitKey<T>(value: Record<string, T>, key: string): Record<string, T> {
  const next = { ...value };
  delete next[key];
  return next;
}
