import { Check, Copy, ExternalLink, Globe, Search, Terminal } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { isSafeHttpUrl, type ToolActivity } from './messageData';
import {
  CopyTextButton,
  DiffBlock,
  MetaChip,
  ResultFootnote,
  SafeWebAttribution,
  ToolResultNotice,
} from './ToolSharedComponents';
import {
  asRecord,
  formatRetrievedAt,
  formatToolPath,
  integerValue,
  isNonNegativeInteger,
  safeTimestamp,
  stringValue,
  type ToolRecord,
  translateToolError,
} from './toolResultUtils';

export function TerminalResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { t } = useTranslation();
  const argumentsValue = asRecord(activity.arguments);
  const command = stringValue(argumentsValue?.command) ?? stringValue(output.command);
  const terminalId =
    stringValue(argumentsValue?.terminal_id) ??
    stringValue(argumentsValue?.terminalId) ??
    stringValue(output.terminal_id) ??
    stringValue(output.terminalId);
  const input = stringValue(argumentsValue?.input);
  const rawOutput = stringValue(output.output) ?? '';
  // Keep the ANSI control-sequence matcher explicit; this strips terminal styling from displayed output.
  // biome-ignore lint/suspicious/noControlCharactersInRegex: The escape character identifies ANSI CSI sequences.
  const cleanOutput = rawOutput.replace(/\u001b\[[0-9;]*[a-zA-Z]/g, '').trim();
  const errorMessage = translateToolError(output, t);
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
        <span className="min-w-0 flex-1 truncate">
          {terminalId ? t('terminalSession', { terminal: terminalId }) : t('toolExecuteCommand')}
        </span>
        {status ? (
          <span
            aria-live="polite"
            className={`shrink-0 rounded-full bg-composer px-2 py-1 ${running ? 'text-accent' : 'text-secondary-text'}`}
            role={running ? 'status' : undefined}
          >
            {status}
          </span>
        ) : null}
        {copiedText ? (
          <Button aria-label={t('toolCopyContent')} onClick={copyOutput} size="icon" variant="ghost">
            {copyState === 'copied' ? <Check aria-hidden="true" size={15} /> : <Copy aria-hidden="true" size={15} />}
          </Button>
        ) : null}
      </div>
      {copyState === 'copied' ? (
        <p aria-live="polite" className="text-xs text-secondary-text" role="status">
          {t('toolTerminalCopied')}
        </p>
      ) : null}
      {copyState === 'failed' ? (
        <p className="text-xs text-destructive" role="alert">
          {t('toolCopyFailed')}
        </p>
      ) : null}
      {command ? (
        <pre className="overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer px-2.5 py-2 font-code text-xs text-text">
          $ {command}
        </pre>
      ) : null}
      {input ? (
        <pre className="overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer px-2.5 py-2 font-code text-xs text-text">
          &gt; {input.trimEnd()}
        </pre>
      ) : null}
      <div
        aria-live={errorMessage || running ? 'polite' : undefined}
        className="max-h-60 min-h-11 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2.5 font-code text-xs leading-5"
      >
        {errorMessage ? (
          <span className="text-destructive">{errorMessage}</span>
        ) : cleanOutput ? (
          <span className="text-text">{cleanOutput}</span>
        ) : running ? (
          <span className="text-secondary-text">{t('toolTerminalWaitingOutput')}</span>
        ) : (
          <span className="text-secondary-text">{t('toolTerminalNoOutput')}</span>
        )}
      </div>
      {output.truncated === true ? <ResultFootnote message={t('toolListingIncomplete')} /> : null}
    </div>
  );
}

export function WebSearchResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { i18n, t } = useTranslation();
  const args = asRecord(activity.arguments);
  const query = stringValue(args?.query) ?? stringValue(output.query) ?? '';
  const sourceType = stringValue(output.sourceType);
  const sourceLabel = sourceType === 'local_web_search' ? t('toolLocalWebSource') : null;
  const retrievedAt = sourceType === 'local_web_search' ? safeTimestamp(output.retrievedAtUnixMs) : null;
  const attributionHtml = sourceType === 'provider_native' ? stringValue(output.attributionHtml) : null;
  if (!Array.isArray(output.results)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const results = output.results.flatMap(
    (value): Array<{ title: string; url: string; snippet: string; engine: string; sourceId: string | null }> => {
      const item = asRecord(value);
      if (
        !item ||
        typeof item.title !== 'string' ||
        typeof item.url !== 'string' ||
        !isSafeHttpUrl(item.url) ||
        typeof item.snippet !== 'string' ||
        typeof item.engine !== 'string'
      )
        return [];
      return [
        {
          title: item.title,
          url: item.url,
          snippet: item.snippet,
          engine: item.engine,
          sourceId: stringValue(item.sourceId),
        },
      ];
    },
  );
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
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <Search aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
        <span className="min-w-0 flex-1 truncate font-medium text-secondary-text">{query || t('toolSearchQuery')}</span>
        <span className="shrink-0 rounded-control border border-border bg-surface px-1.5 py-0.5 font-semibold text-secondary-text">
          {t('toolWebSearchResultCount', { count: results.length })}
        </span>
      </div>
      <ul className="grid gap-2">
        {results.map((result) => {
          const domain = new URL(result.url).host || result.url;
          return (
            <li className="grid gap-1 rounded-control border border-border bg-surface p-2.5" key={result.url}>
              <div className="flex min-w-0 items-center gap-1 text-xs text-secondary-text">
                <span className="shrink-0 rounded-control bg-selected px-1.5 py-0.5 font-semibold text-accent-icon">
                  {result.engine.toUpperCase()}
                </span>
                <span className="min-w-0 flex-1 truncate" title={domain}>
                  {domain}
                </span>
                <CopyTextButton content={result.url} label={t('toolCopyUrl')} />
                <a
                  aria-label={t('toolOpenUrl')}
                  className="grid size-11 shrink-0 place-items-center rounded-button text-accent-icon hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  href={result.url}
                  rel="noopener noreferrer"
                  target="_blank"
                >
                  <ExternalLink aria-hidden="true" size={14} />
                </a>
              </div>
              <a
                className="line-clamp-2 text-xs font-semibold leading-5 text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover"
                href={result.url}
                rel="noopener noreferrer"
                target="_blank"
              >
                {result.title}
              </a>
              {result.snippet ? <p className="text-xs leading-5 text-secondary-text">{result.snippet}</p> : null}
              {result.sourceId || sourceLabel || retrievedAt !== null ? (
                <div className="flex flex-wrap items-center gap-2 text-xs text-secondary-text">
                  {result.sourceId ? (
                    <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">
                      {t('toolCitationSource', { sourceId: result.sourceId })}
                    </span>
                  ) : null}
                  {sourceLabel ? (
                    <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{sourceLabel}</span>
                  ) : null}
                  {retrievedAt !== null ? (
                    <span>{t('toolSourceRetrievedAt', { time: formatRetrievedAt(retrievedAt, i18n.language) })}</span>
                  ) : null}
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function GitResult({
  activity,
  output,
  locale,
}: {
  activity: ToolActivity;
  output: ToolRecord;
  locale: string;
}) {
  const { t } = useTranslation();
  if (activity.name === 'git_status') {
    const branch = stringValue(output.branch);
    const upstream = stringValue(output.upstream);
    const files = output.files;
    if (!Array.isArray(files)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    const parsed = files.flatMap(
      (value): Array<{ path: string; status: string; staged: boolean; unstaged: boolean }> => {
        const file = asRecord(value);
        if (
          !file ||
          typeof file.path !== 'string' ||
          typeof file.status !== 'string' ||
          typeof file.staged !== 'boolean' ||
          typeof file.unstaged !== 'boolean'
        )
          return [];
        return [{ path: file.path, status: file.status, staged: file.staged, unstaged: file.unstaged }];
      },
    );
    if (parsed.length !== files.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    return (
      <div className="grid gap-2 pb-2">
        <div className="flex flex-wrap gap-2 text-xs">
          {branch ? <MetaChip label={t('toolGitBranch')} value={branch} /> : null}
          {upstream ? <MetaChip label={t('toolGitUpstream')} value={upstream} /> : null}
          {isNonNegativeInteger(output.ahead) && output.ahead > 0 ? (
            <MetaChip label={t('toolGitAhead')} value={String(output.ahead)} />
          ) : null}
          {isNonNegativeInteger(output.behind) && output.behind > 0 ? (
            <MetaChip label={t('toolGitBehind')} value={String(output.behind)} />
          ) : null}
        </div>
        {parsed.length === 0 ? (
          <ToolResultNotice message={output.truncated === true ? t('toolOperationTruncated') : t('toolGitNoChanges')} />
        ) : (
          <ul className="max-h-56 overflow-auto rounded-control border border-border bg-surface">
            {parsed.map((file) => (
              <li
                className="flex min-h-9 flex-wrap items-center gap-2 border-b border-border px-2.5 py-2 last:border-b-0"
                key={file.path}
              >
                <span className="min-w-0 flex-1 truncate font-code text-xs text-text" title={file.path}>
                  {formatToolPath(file.path)}
                </span>
                <span className="text-xs text-secondary-text">{file.status}</span>
                {file.staged ? <MetaChip label={t('toolGitStaged')} /> : null}
                {file.unstaged ? <MetaChip label={t('toolGitUnstaged')} /> : null}
              </li>
            ))}
          </ul>
        )}
        {output.truncated === true ? <ResultFootnote message={t('toolOperationTruncated')} /> : null}
      </div>
    );
  }
  if (activity.name === 'git_diff') {
    if (
      typeof output.staged !== 'string' ||
      typeof output.unstaged !== 'string' ||
      typeof output.stagedTruncated !== 'boolean' ||
      typeof output.unstagedTruncated !== 'boolean'
    )
      return <ToolResultNotice message={t('toolOperationUnavailable')} />;
    if (output.staged.length === 0 && output.unstaged.length === 0)
      return <ToolResultNotice message={t('toolGitNoDiff')} />;
    return (
      <div className="grid gap-2 pb-2">
        {output.staged ? (
          <DiffBlock label={t('toolGitStaged')} value={output.staged} truncated={output.stagedTruncated} />
        ) : null}
        {output.unstaged ? (
          <DiffBlock label={t('toolGitUnstaged')} value={output.unstaged} truncated={output.unstagedTruncated} />
        ) : null}
      </div>
    );
  }
  if (!Array.isArray(output.entries)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const entries = output.entries.flatMap((value): Array<{ id: string; timestamp: number; subject: string }> => {
    const entry = asRecord(value);
    if (
      !entry ||
      typeof entry.id !== 'string' ||
      !isNonNegativeInteger(entry.timestampUnixSeconds) ||
      entry.timestampUnixSeconds > 8640000000000 ||
      typeof entry.subject !== 'string'
    )
      return [];
    return [{ id: entry.id, timestamp: entry.timestampUnixSeconds, subject: entry.subject }];
  });
  if (entries.length !== output.entries.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  if (entries.length === 0)
    return (
      <ToolResultNotice message={output.truncated === true ? t('toolOperationTruncated') : t('toolGitNoHistory')} />
    );
  return (
    <ol className="grid gap-0 pb-2">
      {entries.map((entry) => (
        <li className="flex min-w-0 items-start gap-3 border-b border-border py-2 last:border-b-0" key={entry.id}>
          <code className="w-14 shrink-0 font-code text-xs text-secondary-text">{entry.id.slice(0, 8)}</code>
          <span className="min-w-0 flex-1 text-xs text-text">{entry.subject}</span>
          <time
            className="shrink-0 text-xs text-secondary-text"
            dateTime={new Date(entry.timestamp * 1000).toISOString()}
          >
            {new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(entry.timestamp * 1000)}
          </time>
        </li>
      ))}
    </ol>
  );
}

export function ReadUrlResult({
  activity,
  output,
  locale,
}: {
  activity: ToolActivity;
  output: ToolRecord;
  locale: string;
}) {
  const { t } = useTranslation();
  const args = asRecord(activity.arguments);
  const url = stringValue(output.url) ?? stringValue(args?.url) ?? activity.targetPath;
  const title = stringValue(output.title);
  const content = stringValue(output.content);
  const length = integerValue(output.length);
  if (
    !url ||
    !isSafeHttpUrl(url) ||
    title === null ||
    content === null ||
    length === null ||
    length < 0 ||
    typeof output.truncated !== 'boolean'
  )
    return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const retrievedAt = safeTimestamp(output.retrievedAtUnixMs);
  const sourceId = stringValue(output.sourceId);
  const sourceType = stringValue(output.sourceType);
  const sourceLabel = sourceType === 'local_read_url' ? t('toolLocalPageSource') : null;
  return (
    <div className="grid gap-2 rounded-control border border-border bg-surface p-2.5 pb-2">
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <Globe aria-hidden="true" className="shrink-0 text-accent-icon" size={14} />
        <span className="min-w-0 flex-1 truncate font-semibold text-text">{title || new URL(url).host}</span>
        <span className="rounded-control bg-selected px-2 py-1 text-secondary-text">
          {t('toolReadUrlLength', { count: length })}
        </span>
        {output.truncated ? (
          <span className="rounded-control bg-selected px-2 py-1 text-warning">{t('toolOperationTruncated')}</span>
        ) : null}
      </div>
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <span className="min-w-0 flex-1 truncate font-code" title={url}>
          {url}
        </span>
        <CopyTextButton content={content} label={t('toolCopyContent')} />
        <a
          aria-label={t('toolOpenUrl')}
          className="grid size-11 shrink-0 place-items-center rounded-button text-accent-icon hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          href={url}
          rel="noopener noreferrer"
          target="_blank"
        >
          <ExternalLink aria-hidden="true" size={14} />
        </a>
      </div>
      {sourceId || sourceLabel || retrievedAt !== null ? (
        <div className="flex flex-wrap items-center gap-2 text-xs text-secondary-text">
          {sourceId ? (
            <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">
              {t('toolCitationSource', { sourceId })}
            </span>
          ) : null}
          {sourceLabel ? (
            <span className="rounded-control bg-selected px-1.5 py-0.5 text-secondary-text">{sourceLabel}</span>
          ) : null}
          {retrievedAt !== null ? (
            <span>{t('toolSourceRetrievedAt', { time: formatRetrievedAt(retrievedAt, locale) })}</span>
          ) : null}
        </div>
      ) : null}
      {content ? (
        <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer p-2 text-xs leading-5 text-text">
          {content}
        </pre>
      ) : null}
    </div>
  );
}
