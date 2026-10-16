import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { LocalServiceError } from '@/shared/services/LocalServiceClient';
import { Button, Dialog, Input } from '@/shared/ui';
import type { ProjectMcpServer } from '@/features/chat/data/projectMcp';
import { localServiceClient } from '@/features/chat/stores/chatStore';

interface ProjectMcpCredentialDialogProps {
  projectId: string;
  server: ProjectMcpServer | null;
  open: boolean;
  onOpenChange(open: boolean): void;
}

type CredentialErrorKey = 'projectMcpCredentialUnavailable' | 'projectMcpSecretRequired' | 'projectMcpSecretTooLarge';

export function ProjectMcpCredentialDialog({ projectId, server, open, onOpenChange }: ProjectMcpCredentialDialogProps) {
  const { t } = useTranslation();
  const sessionGeneration = useRef(0);
  const [stored, setStored] = useState<Record<string, boolean | null>>({});
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, CredentialErrorKey>>({});
  const [busyName, setBusyName] = useState<string | null>(null);
  const names = server ? credentialNames(server) : [];

  useEffect(() => {
    const generation = sessionGeneration.current + 1;
    sessionGeneration.current = generation;
    let current = true;
    setStored(Object.fromEntries(names.map((name) => [name, null])));
    setSecrets({});
    setErrors({});
    setBusyName(null);
    const activeProjectId = projectId;
    if (open && server) {
      const activeServerId = server.id;
      names.forEach((name) => {
        void (async () => {
          try {
            const result = await localServiceClient.call('project.mcp.credential.status', {
              projectId: activeProjectId,
              serverId: activeServerId,
              environmentName: name,
            });
            const isStored = result.stored;
            if (typeof isStored !== 'boolean') throw new Error('Credential status is invalid.');
            if (current && generation === sessionGeneration.current) {
              setStored((value) => ({ ...value, [name]: isStored }));
              setErrors((value) => omitKey(value, name));
            }
          } catch {
            if (current && generation === sessionGeneration.current) {
              setStored((value) => ({ ...value, [name]: null }));
              setErrors((value) => ({ ...value, [name]: 'projectMcpCredentialUnavailable' }));
            }
          }
        })();
      });
    }
    return () => {
      current = false;
      if (sessionGeneration.current === generation) sessionGeneration.current += 1;
    };
  }, [open, projectId, server?.id]);

  const saveSecret = async (name: string) => {
    if (!server || busyName) return;
    const generation = sessionGeneration.current;
    const activeProjectId = projectId;
    const activeServerId = server.id;
    const secret = secrets[name] ?? '';
    if (secret.length === 0) {
      setErrors((current) => ({ ...current, [name]: 'projectMcpSecretRequired' }));
      return;
    }
    if (new TextEncoder().encode(secret).length > 2500) {
      setErrors((current) => ({ ...current, [name]: 'projectMcpSecretTooLarge' }));
      return;
    }
    setBusyName(name);
    setErrors((current) => omitKey(current, name));
    try {
      await localServiceClient.call('project.mcp.credential.set', {
        projectId: activeProjectId,
        serverId: activeServerId,
        environmentName: name,
        secret,
      });
      if (generation === sessionGeneration.current) {
        setSecrets((current) => omitKey(current, name));
        setStored((current) => ({ ...current, [name]: true }));
      }
    } catch (error) {
      if (generation === sessionGeneration.current) {
        setErrors((current) => ({ ...current, [name]: credentialErrorKey(error) }));
      }
    } finally {
      if (generation === sessionGeneration.current) setBusyName(null);
    }
  };

  const removeSecret = async (name: string) => {
    if (!server || busyName) return;
    const generation = sessionGeneration.current;
    const activeProjectId = projectId;
    const activeServerId = server.id;
    setBusyName(name);
    setErrors((current) => omitKey(current, name));
    try {
      await localServiceClient.call('project.mcp.credential.remove', {
        projectId: activeProjectId,
        serverId: activeServerId,
        environmentName: name,
      });
      if (generation === sessionGeneration.current) setStored((current) => ({ ...current, [name]: false }));
    } catch {
      if (generation === sessionGeneration.current) {
        setErrors((current) => ({ ...current, [name]: 'projectMcpCredentialUnavailable' }));
      }
    } finally {
      if (generation === sessionGeneration.current) setBusyName(null);
    }
  };

  return (
    <Dialog
      className="max-h-[calc(100vh-2rem)] overflow-y-auto"
      onOpenChange={onOpenChange}
      open={open && server !== null}
      title={`${t('projectMcpCredentials')}${server ? ` · ${server.id}` : ''}`}
    >
      <div className="grid gap-4">
        <p className="text-sm text-secondary-text">{t('projectMcpCredentialHint')}</p>
        {server && names.length === 0 ? <p className="text-sm text-secondary-text">{t('projectMcpEnvironmentVariablesRequired')}</p> : null}
        {names.map((name) => (
          <section className="grid gap-2 border-t border-border pt-3" key={name}>
            <h3 className="text-sm font-medium text-text">{name}</h3>
            <p
              aria-live="polite"
              className="text-xs text-secondary-text"
              role={stored[name] === null && errors[name] === 'projectMcpCredentialUnavailable' ? 'alert' : undefined}
            >
              {stored[name] === null
                ? errors[name] === 'projectMcpCredentialUnavailable'
                  ? t('projectMcpCredentialUnavailable')
                  : t('historyLoading')
                : t(stored[name] ? 'projectMcpCredentialStored' : 'projectMcpCredentialMissing')}
            </p>
            <Input
              autoComplete="new-password"
              disabled={busyName !== null}
              label={t('projectMcpSecret')}
              onChange={(event) => setSecrets((current) => ({ ...current, [name]: event.target.value }))}
              type="password"
              value={secrets[name] ?? ''}
            />
            {errors[name] && !(stored[name] === null && errors[name] === 'projectMcpCredentialUnavailable') ? (
              <p className="text-sm text-destructive" role="alert">{t(errors[name])}</p>
            ) : null}
            <div className="flex flex-wrap justify-end gap-2">
              <Button disabled={busyName !== null} onClick={() => void removeSecret(name)} size="small">
                {t('projectMcpCredentialRemove')}
              </Button>
              <Button disabled={busyName !== null} onClick={() => void saveSecret(name)} size="small" variant="primary">
                {busyName === name ? t('saving') : t('projectMcpCredentialSave')}
              </Button>
            </div>
          </section>
        ))}
        <div className="flex justify-end">
          <Button onClick={() => onOpenChange(false)}>{t('close')}</Button>
        </div>
      </div>
    </Dialog>
  );
}

function credentialNames(server: ProjectMcpServer): string[] {
  return [...new Set([
    ...server.environmentVariables,
    ...(server.authEnvironmentVariable ? [server.authEnvironmentVariable] : []),
  ])];
}

function omitKey<T>(value: Record<string, T>, key: string): Record<string, T> {
  const next = { ...value };
  delete next[key];
  return next;
}

function credentialErrorKey(error: unknown): CredentialErrorKey {
  if (error instanceof LocalServiceError) {
    if (error.code === 'mcp_credential_too_large') return 'projectMcpSecretTooLarge';
    if (error.code === 'mcp_credential_empty') return 'projectMcpSecretRequired';
  }
  return 'projectMcpCredentialUnavailable';
}
