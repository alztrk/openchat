import { useState, type ReactNode } from 'react';
import type { TFunction } from 'i18next';
import {
  AlertCircle,
  Check,
  CircleAlert,
  Copy,
  ExternalLink,
  File,
  FileText,
  Folder,
  FolderOpen,
  Globe,
  Info,
  LoaderCircle,
  Search,
  Terminal,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';
import { isSafeHttpUrl, type ToolActivity } from './messageData';

type ToolRecord = Record<string, unknown>;

interface ToolActivityResultsProps {
  activity: ToolActivity;
}

export function ToolActivityResults({ activity }: ToolActivityResultsProps) {
  const { i18n, t } = useTranslation();
  const output = asRecord(activity.output);
  const requestedPath = getRequestedToolPath(activity);
  const errorMessage = stringValue(asRecord(output?.error)?.message) ?? stringValue(output?.error);

  if (activity.status === 'denied' || activity.status === 'cancelled') {
    return <ToolResultNotice message={t(activity.status === 'denied' ? 'toolDenied' : 'toolCancelled')} path={requestedPath} />;
  }
  if (activity.status === 'failed' || errorMessage) {
    if (isTerminalTool(activity.name) && output !== null) {
      return <TerminalResult activity={activity} output={output} />;
    }
    return <ToolResultNotice isError message={errorMessage ?? t('toolOperationFailed')} path={requestedPath} />;
  }
  if (output === null) {
    const message = activity.status === 'running'
      ? 'toolOperationWorking'
      : activity.status === 'awaitingApproval'
        ? 'toolAwaitingApproval'
        : 'toolOperationUnavailable';
    return <ToolResultNotice isWorking={activity.status === 'running'} message={t(message)} />;
  }

  switch (activity.name) {
    case 'list_files':
    case 'glob':
    case 'list_directory':
      return <FileListingResult activity={activity} output={output} requestedPath={requestedPath} />;
    case 'execute_command':
    case 'bash':
    case 'run_project_task':
    case 'send_terminal_input':
      return <TerminalResult activity={activity} output={output} />;
    case 'search_files':
    case 'grep':
      return <SearchResult activity={activity} output={output} />;
    case 'read_file':
    case 'read':
      return <ReadResult output={output} />;
    case 'get_file_info':
      return <FileInfoResult output={output} locale={i18n.language} />;
    case 'write_file':
    case 'write':
      return <WriteResult activity={activity} output={output} locale={i18n.language} />;
    case 'edit_file':
    case 'edit':
      return <EditResult activity={activity} output={output} />;
    case 'web_search':
      return <WebSearchResult activity={activity} output={output} />;
    case 'git_status':
    case 'git_diff':
    case 'git_history':
      return <GitResult activity={activity} output={output} locale={i18n.language} />;
    case 'read_url_content':
    case 'read_url':
      return <ReadUrlResult activity={activity} output={output} locale={i18n.language} />;
    default:
      return null;
  }
}

function FileListingResult({ activity, output, requestedPath }: {
  activity: ToolActivity;
  output: ToolRecord;
  requestedPath: string | null;
}) {
  const { t } = useTranslation();
  const entries = output.entries;
  if (!Array.isArray(entries)) return <ToolResultNotice message={t('toolListingUnavailable')} />;
  const parsedEntries = entries.flatMap((entry): Array<{ path: string; name: string; isDirectory: boolean }> => {
    const item = asRecord(entry);
    if (!item || typeof item.path !== 'string' || item.path.length === 0 || (item.type !== 'file' && item.type !== 'directory')) return [];
    return [{ path: item.path, name: pathName(item.path), isDirectory: item.type === 'directory' }];
  });
  if (parsedEntries.length !== entries.length) return <ToolResultNotice message={t('toolListingUnavailable')} />;
  const location = requestedPath ?? stringValue(asRecord(activity.arguments)?.cwd);
  const hasMore = isNonNegativeInteger(output.nextOffset);

  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        {location ? <><FolderOpen aria-hidden="true" className="shrink-0" size={15} /><span className="min-w-0 flex-1 truncate" title={location}>{formatToolPath(location, t)}</span></> : <span className="min-w-0 flex-1" />}
        <span className="shrink-0">{t('toolFileCount', { count: parsedEntries.length })}</span>
      </div>
      {parsedEntries.length === 0 ? (
        <ToolResultNotice message={t('toolEmptyListing')} />
      ) : (
        <ul className="max-h-56 overflow-auto rounded-control border border-border bg-surface py-1">
          {parsedEntries.map((entry) => (
            <li className="flex min-h-8 items-center gap-2 px-2.5 text-xs text-text" key={entry.path}>
              {entry.isDirectory ? <Folder aria-hidden="true" className="shrink-0 text-accent-icon" size={16} /> : <File aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />}
              <span className="min-w-0 flex-1 truncate" title={entry.name}>{entry.name}</span>
            </li>
          ))}
        </ul>
      )}
      {hasMore ? <ResultFootnote message={t('toolMoreFilesAvailable')} /> : null}
      {output.truncated === true || activity.status === 'completed' && output.incomplete === true
        ? <ResultFootnote message={t('toolListingIncomplete')} />
        : null}
    </div>
  );
}

function TerminalResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { t } = useTranslation();
  const argumentsValue = asRecord(activity.arguments);
  const command = stringValue(argumentsValue?.command) ?? stringValue(output.command);
  const terminalId = stringValue(argumentsValue?.terminal_id)
    ?? stringValue(argumentsValue?.terminalId)
    ?? stringValue(output.terminal_id)
    ?? stringValue(output.terminalId);
  const input = stringValue(argumentsValue?.input);
  const rawOutput = stringValue(output.output) ?? '';
  const cleanOutput = rawOutput.replace(/\u001b\[[0-9;]*[a-zA-Z]/g, '').trim();
  const errorMessage = stringValue(asRecord(output.error)?.message) ?? stringValue(output.error);
  const running = output.is_running === true || activity.status === 'running';
  const waitingForInput = output.waiting_for_input === true || (running && output.waiting_for_input !== false);
  const terminated = output.status === 'terminated' || output.is_killed === true;
  const exitCode = integerValue(output.exit_code);
  const copiedText = cleanOutput.length > 0 ? cleanOutput : command;
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const status = terminated
    ? t('toolTerminalTerminated')
    : running
      ? t(waitingForInput ? 'toolTerminalWaitingForInput' : 'toolTerminalRunning')
      : exitCode !== null
        ? t('toolTerminalExitCode', { code: exitCode })
        : null;

  const copyOutput = () => {
    if (!copiedText) return;
    void navigator.clipboard.writeText(copiedText).then(
      () => setCopyState('copied'),
      () => setCopyState('failed'),
    );
  };

  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <Terminal aria-hidden="true" className="shrink-0" size={15} />
        <span className="min-w-0 flex-1 truncate">{terminalId ? t('terminalSession', { terminal: terminalId }) : t('toolExecuteCommand')}</span>
        {status ? <span aria-live="polite" className={`shrink-0 rounded-full bg-composer px-2 py-1 ${running ? 'text-accent' : 'text-secondary-text'}`} role={running ? 'status' : undefined}>{status}</span> : null}
        {copiedText ? (
          <Button aria-label={t('toolCopyContent')} onClick={copyOutput} size="icon" variant="ghost">
            {copyState === 'copied' ? <Check aria-hidden="true" size={15} /> : <Copy aria-hidden="true" size={15} />}
          </Button>
        ) : null}
      </div>
      {copyState === 'copied' ? <p aria-live="polite" className="text-xs text-secondary-text" role="status">{t('toolTerminalCopied')}</p> : null}
      {copyState === 'failed' ? <p className="text-xs text-destructive" role="alert">{t('toolCopyFailed')}</p> : null}
      {command ? <pre className="overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer px-2.5 py-2 font-code text-xs text-text">$ {command}</pre> : null}
      {input ? <pre className="overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer px-2.5 py-2 font-code text-xs text-text">&gt; {input.trimEnd()}</pre> : null}
      <div aria-live={errorMessage || running ? 'polite' : undefined} className="max-h-60 min-h-11 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2.5 font-code text-xs leading-5">
        {errorMessage ? <span className="text-destructive">{errorMessage}</span> : cleanOutput ? <span className="text-text">{cleanOutput}</span> : running ? <span className="text-secondary-text">{t('toolTerminalWaitingOutput')}</span> : <span className="text-secondary-text">{t('toolTerminalNoOutput')}</span>}
      </div>
      {output.truncated === true ? <ResultFootnote message={t('toolListingIncomplete')} /> : null}
    </div>
  );
}

function SearchResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { t } = useTranslation();
  if (!Array.isArray(output.matches)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const matches = output.matches.flatMap((value): Array<{ path: string; line: number; text: string }> => {
    const item = asRecord(value);
    const line = integerValue(item?.line);
    if (!item || typeof item.path !== 'string' || !item.path || line === null || line < 1 || typeof item.text !== 'string') return [];
    return [{ path: item.path, line, text: item.text }];
  });
  if (matches.length !== output.matches.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const query = stringValue(asRecord(activity.arguments)?.query) ?? stringValue(output.query);
  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <Search aria-hidden="true" className="shrink-0 text-accent-icon" size={15} />
        <span className="min-w-0 flex-1 truncate font-semibold text-text">{query ?? ''}</span>
        <span className="shrink-0">{t('toolSearchMatchCount', { count: matches.length })}</span>
      </div>
      {matches.length === 0 ? <ToolResultNotice message={t('toolSearchNoMatches')} /> : (
        <ul className="max-h-64 overflow-auto rounded-control border border-border bg-surface">
          {matches.map((match, index) => (
            <li className="grid gap-1 border-b border-border px-2.5 py-2 last:border-b-0" key={`${match.path}:${match.line}:${index}`}>
              <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
                <File aria-hidden="true" className="shrink-0" size={14} />
                <span className="min-w-0 flex-1 truncate" title={match.path}>{formatToolPath(match.path, t)}</span>
        <span className="shrink-0 rounded-control bg-composer px-1.5 py-0.5">{match.line}</span>
              </div>
              <pre className="whitespace-pre-wrap break-words pl-5 font-code text-xs leading-5 text-text">{match.text}</pre>
            </li>
          ))}
        </ul>
      )}
      {isNonNegativeInteger(output.nextOffset) ? <ResultFootnote message={t('toolSearchMoreResults')} /> : output.truncated === true ? <ResultFootnote message={t('toolOperationTruncated')} /> : null}
    </div>
  );
}

function ReadResult({ output }: { output: ToolRecord }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  if (!path || !Array.isArray(output.lines)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const lines = output.lines.flatMap((value): Array<{ number: number; text: string }> => {
    const item = asRecord(value);
    const number = integerValue(item?.line);
    return item && number !== null && number > 0 && typeof item.text === 'string'
      ? [{ number, text: item.text }]
      : [];
  });
  if (lines.length !== output.lines.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex min-w-0 items-center gap-2 text-xs text-secondary-text"><FileText aria-hidden="true" size={15} /><span className="truncate" title={path}>{formatToolPath(path, t)}</span></p>
      {lines.length === 0 ? <ToolResultNotice message={t('toolReadNoLines')} /> : (
        <ol className="max-h-64 overflow-auto rounded-control border border-border bg-surface py-1">
          {lines.map((line) => (
            <li className="grid grid-cols-[2.5rem_minmax(0,1fr)] gap-2 px-2.5 py-0.5 font-code text-xs leading-5" key={line.number}>
              <span className="text-right text-secondary-text">{line.number}</span><span className="whitespace-pre-wrap break-words text-text">{line.text || ' '}</span>
            </li>
          ))}
        </ol>
      )}
      <p className="text-xs text-secondary-text">{t('toolReadLineCount', { count: lines.length })}</p>
      {isNonNegativeInteger(output.nextLine) ? <ResultFootnote message={t('toolReadMoreLines')} /> : null}
    </div>
  );
}

function FileInfoResult({ output, locale }: { output: ToolRecord; locale: string }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const type = output.type;
  const size = integerValue(output.size);
  if (!path || (type !== 'file' && type !== 'directory') || size === null || size < 0) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const isDirectory = type === 'directory';
  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-3 rounded-control border border-border bg-surface p-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-control bg-selected text-accent-icon">{isDirectory ? <Folder aria-hidden="true" size={20} /> : <File aria-hidden="true" size={20} />}</span>
        <div className="min-w-0"><p className="truncate text-sm font-semibold text-text" title={path}>{pathName(path)}</p><p className="mt-0.5 break-all text-xs text-secondary-text" title={path}>{formatToolPath(path, t)}</p></div>
      </div>
      <div className="flex flex-wrap gap-2 text-xs text-secondary-text">
        <span className="rounded-control border border-border px-2 py-1">{t(isDirectory ? 'toolFileTypeDirectory' : 'toolFileTypeFile')}</span>
        {!isDirectory ? <span className="rounded-control border border-border px-2 py-1">{formatBytes(size, locale)}</span> : null}
      </div>
    </div>
  );
}

function WriteResult({ activity, output, locale }: { activity: ToolActivity; output: ToolRecord; locale: string }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const size = integerValue(output.bytesWritten);
  if (output.success !== true || !path || size === null || size < 0) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const content = stringValue(asRecord(activity.arguments)?.content);
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex items-center gap-2 text-sm font-medium text-success"><Check aria-hidden="true" size={16} />{t('toolWriteSuccess', { size: formatBytes(size, locale) })}</p>
      <PathHeading path={path} />
      {content !== null ? <PreviewBlock label={t('toolFilePreview')} value={content} /> : null}
    </div>
  );
}

function EditResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const replacements = integerValue(output.replacements);
  if (output.success !== true || !path || replacements === null || replacements < 1) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const argumentsValue = asRecord(activity.arguments);
  const before = stringValue(argumentsValue?.oldString) ?? stringValue(argumentsValue?.old_string);
  const after = stringValue(argumentsValue?.newString) ?? stringValue(argumentsValue?.new_string);
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex items-center gap-2 text-sm font-medium text-success"><Check aria-hidden="true" size={16} />{t('toolEditSuccess', { count: replacements })}</p>
      <PathHeading path={path} />
      {before !== null && after !== null ? (
        <div className="grid gap-2 sm:grid-cols-2">
          <PreviewBlock label={t('toolEditBefore')} value={before} />
          <PreviewBlock label={t('toolEditAfter')} value={after} />
        </div>
      ) : null}
    </div>
  );
}

function WebSearchResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { i18n, t } = useTranslation();
  const args = asRecord(activity.arguments);
  const query = stringValue(args?.query) ?? stringValue(output.query) ?? '';
  const sourceType = stringValue(output.sourceType);
  const sourceLabel = sourceType === 'local_web_search' ? t('toolLocalWebSource') : null;
  const retrievedAt = sourceType === 'local_web_search'
    ? safeTimestamp(output.retrievedAtUnixMs)
    : null;
  const attributionHtml = sourceType === 'provider_native'
    ? stringValue(output.attributionHtml)
    : null;
  if (!Array.isArray(output.results)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const results = output.results.flatMap((value): Array<{ title: string; url: string; snippet: string; engine: string; sourceId: string | null }> => {
    const item = asRecord(value);
    if (!item || typeof item.title !== 'string' || typeof item.url !== 'string' || !isSafeHttpUrl(item.url) || typeof item.snippet !== 'string' || typeof item.engine !== 'string') return [];
    return [{ title: item.title, url: item.url, snippet: item.snippet, engine: item.engine, sourceId: stringValue(item.sourceId) }];
  });
  if (results.length !== output.results.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  if (results.length === 0) {
    return (
      <div className="grid gap-2 pb-2">
        {attributionHtml ? <SafeWebAttribution html={attributionHtml} /> : null}
        <ToolResultNotice message={t('toolWebSearchNoResults')} />
      </div>
    );
  }
  return (
    <div className="grid gap-2 pb-2">
      {attributionHtml ? <SafeWebAttribution html={attributionHtml} /> : null}
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text"><Search aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} /><span className="min-w-0 flex-1 truncate font-medium text-secondary-text">{query || t('toolSearchQuery')}</span><span className="shrink-0 rounded-control border border-border bg-surface px-1.5 py-0.5 font-semibold text-secondary-text">{t('toolWebSearchResultCount', { count: results.length })}</span></div>
      <ul className="grid gap-2">
        {results.map((result, index) => {
          const domain = new URL(result.url).host || result.url;
          return (
            <li className="grid gap-1 rounded-control border border-border bg-surface p-2.5" key={`${result.url}:${index}`}>
              <div className="flex min-w-0 items-center gap-1 text-xs text-secondary-text">
                <span className="shrink-0 rounded-control bg-selected px-1.5 py-0.5 font-semibold text-accent-icon">{result.engine.toUpperCase()}</span>
                <span className="min-w-0 flex-1 truncate" title={domain}>{domain}</span>
                <CopyTextButton content={result.url} label={t('toolCopyUrl')} />
                <a aria-label={t('toolOpenUrl')} className="grid size-11 shrink-0 place-items-center rounded-button text-accent-icon hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus" href={result.url} rel="noopener noreferrer" target="_blank"><ExternalLink aria-hidden="true" size={14} /></a>
              </div>
              <a className="line-clamp-2 text-xs font-semibold leading-5 text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover" href={result.url} rel="noopener noreferrer" target="_blank">{result.title}</a>
              {result.snippet ? <p className="text-xs leading-5 text-secondary-text">{result.snippet}</p> : null}
              {result.sourceId || sourceLabel || retrievedAt !== null ? (
                <div className="flex flex-wrap items-center gap-2 text-xs text-secondary-text">
                  {result.sourceId ? <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{t('toolCitationSource', { sourceId: result.sourceId })}</span> : null}
                  {sourceLabel ? <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{sourceLabel}</span> : null}
                  {retrievedAt !== null ? <span>{t('toolSourceRetrievedAt', { time: formatRetrievedAt(retrievedAt, i18n.language) })}</span> : null}
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function GitResult({ activity, output, locale }: { activity: ToolActivity; output: ToolRecord; locale: string }) {
  const { t } = useTranslation();
  if (activity.name === 'git_status') {
    const branch = stringValue(output.branch);
    const upstream = stringValue(output.upstream);
    const files = output.files;
    if (!Array.isArray(files)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    const parsed = files.flatMap((value): Array<{ path: string; status: string; staged: boolean; unstaged: boolean }> => {
      const file = asRecord(value);
      if (!file || typeof file.path !== 'string' || typeof file.status !== 'string' || typeof file.staged !== 'boolean' || typeof file.unstaged !== 'boolean') return [];
      return [{ path: file.path, status: file.status, staged: file.staged, unstaged: file.unstaged }];
    });
    if (parsed.length !== files.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    return (
      <div className="grid gap-2 pb-2">
        <div className="flex flex-wrap gap-2 text-xs">
          {branch ? <MetaChip label={t('toolGitBranch')} value={branch} /> : null}
          {upstream ? <MetaChip label={t('toolGitUpstream')} value={upstream} /> : null}
          {isNonNegativeInteger(output.ahead) && output.ahead > 0 ? <MetaChip label={t('toolGitAhead')} value={String(output.ahead)} /> : null}
          {isNonNegativeInteger(output.behind) && output.behind > 0 ? <MetaChip label={t('toolGitBehind')} value={String(output.behind)} /> : null}
        </div>
        {parsed.length === 0 ? <ToolResultNotice message={output.truncated === true ? t('toolOperationTruncated') : t('toolGitNoChanges')} /> : (
          <ul className="max-h-56 overflow-auto rounded-control border border-border bg-surface">
            {parsed.map((file) => <li className="flex min-h-9 flex-wrap items-center gap-2 border-b border-border px-2.5 py-2 last:border-b-0" key={file.path}><span className="min-w-0 flex-1 truncate font-code text-xs text-text" title={file.path}>{formatToolPath(file.path, t)}</span><span className="text-xs text-secondary-text">{file.status}</span>{file.staged ? <MetaChip label={t('toolGitStaged')} /> : null}{file.unstaged ? <MetaChip label={t('toolGitUnstaged')} /> : null}</li>)}
          </ul>
        )}
        {output.truncated === true ? <ResultFootnote message={t('toolOperationTruncated')} /> : null}
      </div>
    );
  }
  if (activity.name === 'git_diff') {
    if (typeof output.staged !== 'string' || typeof output.unstaged !== 'string' || typeof output.stagedTruncated !== 'boolean' || typeof output.unstagedTruncated !== 'boolean') return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    if (output.staged.length === 0 && output.unstaged.length === 0) return <ToolResultNotice message={t('toolGitNoDiff')} />;
    return (
      <div className="grid gap-2 pb-2">
        {output.staged ? <DiffBlock label={t('toolGitStaged')} value={output.staged} truncated={output.stagedTruncated} /> : null}
        {output.unstaged ? <DiffBlock label={t('toolGitUnstaged')} value={output.unstaged} truncated={output.unstagedTruncated} /> : null}
      </div>
    );
  }
  if (!Array.isArray(output.entries)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const entries = output.entries.flatMap((value): Array<{ id: string; timestamp: number; subject: string }> => {
    const entry = asRecord(value);
    if (!entry || typeof entry.id !== 'string' || !isNonNegativeInteger(entry.timestampUnixSeconds) || entry.timestampUnixSeconds > 8640000000000 || typeof entry.subject !== 'string') return [];
    return [{ id: entry.id, timestamp: entry.timestampUnixSeconds, subject: entry.subject }];
  });
  if (entries.length !== output.entries.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  if (entries.length === 0) return <ToolResultNotice message={output.truncated === true ? t('toolOperationTruncated') : t('toolGitNoHistory')} />;
  return (
    <ol className="grid gap-0 pb-2">
      {entries.map((entry) => (
        <li className="flex min-w-0 items-start gap-3 border-b border-border py-2 last:border-b-0" key={entry.id}>
          <code className="w-14 shrink-0 font-code text-xs text-secondary-text">{entry.id.slice(0, 8)}</code>
          <span className="min-w-0 flex-1 text-xs text-text">{entry.subject}</span>
          <time className="shrink-0 text-xs text-secondary-text" dateTime={new Date(entry.timestamp * 1000).toISOString()}>{new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(entry.timestamp * 1000)}</time>
        </li>
      ))}
    </ol>
  );
}

function ReadUrlResult({ activity, output, locale }: { activity: ToolActivity; output: ToolRecord; locale: string }) {
  const { t } = useTranslation();
  const args = asRecord(activity.arguments);
  const url = stringValue(output.url) ?? stringValue(args?.url) ?? activity.targetPath;
  const title = stringValue(output.title);
  const content = stringValue(output.content);
  const length = integerValue(output.length);
  if (!url || !isSafeHttpUrl(url) || title === null || content === null || length === null || length < 0 || typeof output.truncated !== 'boolean') return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const retrievedAt = safeTimestamp(output.retrievedAtUnixMs);
  const sourceId = stringValue(output.sourceId);
  const sourceType = stringValue(output.sourceType);
  const sourceLabel = sourceType === 'local_read_url' ? t('toolLocalPageSource') : null;
  return (
    <div className="grid gap-2 rounded-control border border-border bg-surface p-2.5 pb-2">
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <Globe aria-hidden="true" className="shrink-0 text-accent-icon" size={14} />
        <span className="min-w-0 flex-1 truncate font-semibold text-text">{title || new URL(url).host}</span>
        <span className="rounded-control bg-selected px-2 py-1 text-secondary-text">{t('toolReadUrlLength', { count: length })}</span>
        {output.truncated ? <span className="rounded-control bg-selected px-2 py-1 text-warning">{t('toolOperationTruncated')}</span> : null}
      </div>
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <span className="min-w-0 flex-1 truncate font-code" title={url}>{url}</span>
        <CopyTextButton content={content} label={t('toolCopyContent')} />
        <a aria-label={t('toolOpenUrl')} className="grid size-11 shrink-0 place-items-center rounded-button text-accent-icon hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus" href={url} rel="noopener noreferrer" target="_blank"><ExternalLink aria-hidden="true" size={14} /></a>
      </div>
      {sourceId || sourceLabel || retrievedAt !== null ? (
        <div className="flex flex-wrap items-center gap-2 text-xs text-secondary-text">
          {sourceId ? <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{t('toolCitationSource', { sourceId })}</span> : null}
          {sourceLabel ? <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{sourceLabel}</span> : null}
          {retrievedAt !== null ? <span>{t('toolSourceRetrievedAt', { time: formatRetrievedAt(retrievedAt, locale) })}</span> : null}
        </div>
      ) : null}
      {content ? <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer p-2 text-xs leading-5 text-text">{content}</pre> : null}
    </div>
  );
}

function PathHeading({ path }: { path: string }) {
  const { t } = useTranslation();
  return <p className="flex min-w-0 items-center gap-2 text-xs text-secondary-text"><File aria-hidden="true" className="shrink-0" size={14} /><span className="truncate" title={path}>{formatToolPath(path, t)}</span></p>;
}

function CopyTextButton({ content, label }: { content: string; label: string }) {
  const { t } = useTranslation();
  const [copyFailed, setCopyFailed] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopyFailed(false);
    } catch {
      setCopyFailed(true);
    }
  };
  return (
    <span className="grid shrink-0 justify-items-center">
      <Button aria-label={label} className="size-11" title={label} onClick={() => void copy()} size="icon" variant="ghost"><Copy aria-hidden="true" size={14} /></Button>
      {copyFailed ? <span className="text-xs text-destructive" role="alert">{t('toolCopyFailed')}</span> : null}
    </span>
  );
}

function SafeWebAttribution({ html }: { html: string }) {
  const { t } = useTranslation();
  const nodes = safeAttributionNodes(html);
  if (nodes.length === 0) return null;
  return <div aria-label={t('toolSourceDetails')} className="text-xs leading-5 text-secondary-text">{nodes}</div>;
}

function PreviewBlock({ label, value }: { label: string; value: string }) {
  return <div className="min-w-0"><p className="mb-1 text-xs font-medium text-secondary-text">{label}</p><pre className="max-h-36 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2 font-code text-xs text-text">{preview(value)}</pre></div>;
}

function DiffBlock({ label, value, truncated }: { label: string; value: string; truncated: boolean }) {
  const { t } = useTranslation();
  return <section aria-label={label} className="grid gap-1"><h4 className="text-xs font-semibold text-secondary-text">{label}</h4><pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2 font-code text-xs leading-5 text-text">{value}</pre>{truncated ? <ResultFootnote message={t('toolOperationTruncated')} /> : null}</section>;
}

function MetaChip({ label, value }: { label: string; value?: string }) {
  return <span className="rounded-control border border-border bg-surface px-2 py-1 text-secondary-text">{value ? <><span>{label}: </span><span className="font-medium text-text">{value}</span></> : label}</span>;
}

function ResultFootnote({ message }: { message: string }) {
  return <p className="flex items-start gap-1.5 text-xs leading-5 text-secondary-text"><Info aria-hidden="true" className="mt-0.5 shrink-0" size={13} /><span>{message}</span></p>;
}

function ToolResultNotice({ message, path, isError = false, isWorking = false }: { message: string; path?: string | null; isError?: boolean; isWorking?: boolean }) {
  const { t } = useTranslation();
  const Icon = isWorking ? LoaderCircle : isError ? CircleAlert : AlertCircle;
  return (
    <div className={`flex items-start gap-2 rounded-control px-2.5 py-2 text-xs leading-5 ${isError ? 'border border-destructive/30 bg-destructive/10 text-destructive' : 'bg-surface text-secondary-text'}`}>
      {isWorking || isError ? <Icon aria-hidden="true" className={`mt-0.5 shrink-0 ${isWorking ? 'animate-spin' : ''}`} size={15} /> : null}
      <div className="min-w-0 flex-1">
        <span aria-live={isWorking || isError ? 'polite' : undefined} role={isWorking || isError ? 'status' : undefined}>{message}</span>
        {path ? <p className="mt-1 break-all font-code text-secondary-text" title={path}>{formatToolPath(path, t)}</p> : null}
      </div>
    </div>
  );
}

function asRecord(value: unknown): ToolRecord | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  const record: ToolRecord = {};
  for (const [key, item] of Object.entries(value)) record[key] = item;
  return record;
}

function stringValue(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function integerValue(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : null;
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function pathName(path: string): string {
  const segments = path.replaceAll('\\', '/').split('/');
  return segments.at(-1) || path;
}

function formatBytes(bytes: number, locale: string): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let amount = bytes;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit++;
  }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(amount)} ${units[unit]}`;
}

function preview(value: string): string {
  const normalized = value.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  const lines = normalized.split('\n');
  const shown = lines.slice(0, 3).join('\n').slice(0, 260);
  return lines.length > 3 || normalized.length > 260 ? `${shown}…` : shown || ' ';
}

function safeTimestamp(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 8640000000000000 ? value : null;
}

export function getRequestedToolPath(activity: ToolActivity): string | null {
  const argumentPath = stringValue(asRecord(activity.arguments)?.path)?.trim();
  if (argumentPath) return argumentPath;
  const targetPath = activity.targetPath?.trim();
  return targetPath || null;
}

export function formatToolPath(path: string, t: TFunction): string {
  const normalized = path.replaceAll('\\', '/').trim();
  const lower = normalized.toLowerCase();
  const aliases = [
    { prefix: 'desktop:/', label: t('toolDesktopLocation') },
    { prefix: 'project:/', label: t('toolProjectLocation') },
    { prefix: 'openchat:/', label: t('toolOpenChatLocation') },
  ];
  for (const alias of aliases) {
    const root = alias.prefix.slice(0, -1);
    if (lower === root || lower === alias.prefix) return alias.label;
    if (lower.startsWith(alias.prefix)) {
      const suffix = normalized.slice(alias.prefix.length).replace(/^\/+/, '');
      return suffix ? `${alias.label} /${suffix}` : alias.label;
    }
  }
  return path;
}

function isTerminalTool(name: string): boolean {
  return name === 'execute_command' ||
    name === 'bash' ||
    name === 'run_project_task' ||
    name === 'send_terminal_input';
}

function formatRetrievedAt(timestamp: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(timestamp);
}

function safeAttributionNodes(html: string): ReactNode[] {
  if (html.trim().length === 0 || html.length > 32 * 1024 || typeof DOMParser === 'undefined') return [];
  const document = new DOMParser().parseFromString(html, 'text/html');
  return Array.from(document.body.childNodes).flatMap((node, index) => {
    const rendered = renderAttributionNode(node, `attribution-${index}`);
    return rendered === null || rendered === '' ? [] : [rendered];
  });
}

function renderAttributionNode(node: Node, key: string): ReactNode {
  if (node.nodeType === 3) return node.textContent;
  if (node.nodeType !== 1) return null;
  const element = node as Element;
  const tag = element.tagName.toLowerCase();
  if (tag === 'script' || tag === 'style' || tag === 'template' || tag === 'iframe' || tag === 'object' || tag === 'embed' || tag === 'svg' || tag === 'img') return null;
  const children = Array.from(element.childNodes).map((child, index) => renderAttributionNode(child, `${key}-${index}`));
  if (tag === 'br') return <br key={key} />;
  if (tag === 'a') {
    const href = element.getAttribute('href');
    return href !== null && isSafeHttpUrl(href)
      ? <a className="text-accent hover:underline" href={href} key={key} rel="noopener noreferrer" target="_blank">{children}</a>
      : <span key={key}>{children}</span>;
  }
  if (tag === 'div' || tag === 'p' || tag === 'li') return <span className="block" key={key}>{children}</span>;
  if (tag === 'ul' || tag === 'ol') return <span className="block" key={key}>{children}</span>;
  if (tag === 'strong' || tag === 'b') return <strong key={key}>{children}</strong>;
  if (tag === 'em' || tag === 'i') return <em key={key}>{children}</em>;
  return <span key={key}>{children}</span>;
}
