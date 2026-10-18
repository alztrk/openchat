import { FolderOpen, Globe, LoaderCircle, LockKeyhole, ServerCog, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import type { ChatToolPermissionRequest } from '@/modules/conversations/stores/chatStore';

interface ToolPermissionCardProps {
  request: ChatToolPermissionRequest;
  isResponding: boolean;
  errorKey: 'toolPermissionRequestExpired' | 'toolPermissionResponseFailed' | null;
  onRespond(approved: boolean): void;
}

export function ToolPermissionCard({ request, isResponding, errorKey, onRespond }: ToolPermissionCardProps) {
  const { t } = useTranslation();
  const isMcpServerStart = request.toolName === 'mcp_server_start';
  const isRemoteMcpServer = request.arguments.transport === 'streamable_http';
  const toolName = isMcpServerStart
    ? t(isRemoteMcpServer ? 'toolPermissionMcpServerConnect' : 'toolPermissionMcpServerStart')
    : getToolName(request.toolName, t);
  const Icon = isMcpServerStart
    ? ServerCog
    : request.toolName === 'web_search' || request.toolName === 'read_url_content' || request.toolName === 'read_url'
      ? Globe
      : request.toolName === 'execute_command' ||
          request.toolName === 'bash' ||
          request.toolName === 'send_terminal_input'
        ? Terminal
        : FolderOpen;
  const details = getRequestDetails(request, t);

  return (
    <section
      aria-label={t('toolPermissionRequestTitle')}
      className="mx-auto mb-2 w-full max-w-composer rounded-card border border-border bg-raised p-3"
    >
      <div className="flex min-w-0 flex-col gap-3 compact:flex-row compact:items-start">
        <div className="flex min-w-0 flex-1 items-start gap-2.5">
          <span
            aria-hidden="true"
            className="grid size-9 shrink-0 place-items-center rounded-control bg-surface text-accent-icon"
          >
            <LockKeyhole size={18} />
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-semibold text-text">{toolName}</h3>
            <p className="mt-1 text-sm text-secondary-text">
              {t(
                isMcpServerStart
                  ? isRemoteMcpServer
                    ? 'toolPermissionMcpServerHttpStartDescription'
                    : 'toolPermissionMcpServerLocalStartDescription'
                  : 'toolPermissionRequestDescription',
              )}
            </p>
            <p className="mt-2 flex min-w-0 items-start gap-2 text-sm text-secondary-text">
              <Icon aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
              <span className="max-h-10 overflow-hidden break-words">
                {isMcpServerStart
                  ? t('toolPermissionMcpServerTarget', { name: request.targetPath })
                  : request.targetPath}
              </span>
            </p>
            {details.map(({ label, value }) => (
              <div className="mt-2 grid gap-1" key={label}>
                <span className="text-xs font-medium text-secondary-text">{label}</span>
                <pre className="max-h-24 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2 font-mono text-xs text-text">
                  {value}
                </pre>
              </div>
            ))}
            {errorKey ? (
              <p className="mt-2 text-sm text-destructive" role="alert">
                {t(errorKey)}
              </p>
            ) : null}
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap justify-end gap-2 self-end compact:self-start">
          <Button disabled={isResponding} onClick={() => onRespond(false)}>
            {t('toolPermissionDeny')}
          </Button>
          <Button disabled={isResponding} onClick={() => onRespond(true)} variant="primary">
            {isResponding ? (
              <span aria-live="polite" className="inline-flex items-center gap-2" role="status">
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                {t(
                  isMcpServerStart
                    ? isRemoteMcpServer
                      ? 'toolPermissionMcpServerAllowConnectionOnce'
                      : 'toolPermissionMcpServerAllowStartOnce'
                    : 'toolPermissionAllowOnce',
                )}
              </span>
            ) : (
              t(
                isMcpServerStart
                  ? isRemoteMcpServer
                    ? 'toolPermissionMcpServerAllowConnectionOnce'
                    : 'toolPermissionMcpServerAllowStartOnce'
                  : 'toolPermissionAllowOnce',
              )
            )}
          </Button>
        </div>
      </div>
    </section>
  );
}

type TranslationKey = keyof typeof import('@/localization/locales/en.json');
type Translate = (key: TranslationKey, options?: Record<string, string | number>) => string;

function getToolName(name: string, t: Translate): string {
  switch (name) {
    case 'list_files':
    case 'glob':
    case 'list_directory':
      return t('toolListFiles');
    case 'search_files':
      return t('toolSearchFiles');
    case 'read_file':
    case 'read':
      return t('toolReadFile');
    case 'get_file_info':
      return t('toolGetFileInfo');
    case 'write_file':
    case 'write':
      return t('toolWriteFile');
    case 'edit_file':
    case 'edit':
      return t('toolEditFile');
    case 'execute_command':
    case 'bash':
      return t('toolExecuteCommand');
    case 'run_project_task':
      return t('toolRunProjectTask');
    case 'send_terminal_input':
      return t('toolSendTerminalInput');
    case 'git_status':
      return t('toolGitStatus');
    case 'git_diff':
      return t('toolGitDiff');
    case 'git_history':
      return t('toolGitHistory');
    case 'web_search':
      return t('toolWebSearch');
    case 'read_url_content':
    case 'read_url':
      return t('toolReadUrlContent');
    default:
      return name.startsWith('project_tool__')
        ? t('toolConfiguredProjectTool', { name: name.slice('project_tool__'.length) })
        : t('unknownTool');
  }
}

function getRequestDetails(request: ChatToolPermissionRequest, t: Translate): { label: string; value: string }[] {
  const argument = (key: string): string | null => {
    const value = request.arguments[key];
    return typeof value === 'string' ? value : null;
  };
  const scalar = (value: unknown): string | null => {
    if (typeof value === 'string') return value;
    if (typeof value === 'number' && Number.isFinite(value)) return String(value);
    if (typeof value === 'boolean') return t(value ? 'commonYes' : 'commonNo');
    return null;
  };
  const detail = (label: TranslationKey, value: unknown): { label: string; value: string } | null => {
    const text = scalar(value);
    return text === null ? null : { label: t(label), value: text };
  };
  const details = (() => {
    switch (request.toolName) {
      case 'mcp_server_start': {
        const transport = argument('transport');
        const transportLabel =
          transport === 'streamable_http'
            ? t('projectMcpTransportHttp')
            : transport === 'stdio'
              ? t('projectMcpTransportStdio')
              : null;
        return [detail('projectMcpTransport', transportLabel)];
      }
      case 'list_files':
      case 'glob':
      case 'list_directory':
        return [
          detail('toolPermissionOffset', request.arguments.offset ?? 0),
          detail('toolPermissionLimit', request.arguments.limit ?? 100),
        ];
      case 'search_files':
      case 'grep':
        return [
          detail('toolPermissionQuery', argument('query') ?? argument('pattern')),
          detail('toolPermissionIncludeHidden', request.arguments.includeHidden ?? false),
          detail('toolPermissionOffset', request.arguments.offset ?? 0),
          detail('toolPermissionLimit', request.arguments.limit ?? 40),
        ];
      case 'read_file':
      case 'read':
        return [
          detail('toolPermissionStartLine', request.arguments.startLine ?? request.arguments.offset ?? 1),
          detail('toolPermissionLineCount', request.arguments.lineCount ?? request.arguments.limit ?? 500),
        ];
      case 'write_file':
      case 'write':
        return [detail('toolPermissionContent', argument('content'))];
      case 'edit_file':
      case 'edit':
        return [
          detail('toolPermissionOldText', argument('oldString') ?? argument('old_string')),
          detail('toolPermissionNewText', argument('newString') ?? argument('new_string')),
        ];
      case 'run_project_task':
        return [
          detail('toolPermissionTask', argument('task')),
          detail('toolPermissionCommand', argument('command')),
          detail('toolPermissionTimeout', request.arguments.timeout_seconds),
        ];
      case 'execute_command':
      case 'bash':
        return [
          detail('toolPermissionCommand', argument('command')),
          detail('toolPermissionTerminalId', argument('terminal_id') ?? argument('terminalId')),
          detail('toolPermissionInput', argument('input')),
        ];
      case 'send_terminal_input':
        return [
          detail('toolPermissionTerminalId', argument('terminal_id') ?? argument('terminalId')),
          detail('toolPermissionInput', argument('input')),
        ];
      case 'web_search':
        return [
          detail('toolSearchQuery', argument('query')),
          detail('toolPermissionLimit', request.arguments.limit ?? 5),
        ];
      case 'read_url_content':
      case 'read_url':
        return [
          detail('toolUrl', argument('url')),
          detail('toolPermissionLimit', request.arguments.max_chars ?? request.arguments.maxChars ?? 6000),
        ];
      default:
        return [];
    }
  })();
  return details.filter((item): item is { label: string; value: string } => item !== null);
}
