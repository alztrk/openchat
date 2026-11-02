import { Check, File, Folder, FolderOpen } from 'lucide-react';
import { cloneElement, isValidElement, lazy, Suspense, useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, TitleTooltip } from '@shared/components';
import { readConversationFileChangeDiff, type ChatFileChangeDiff } from '@features/chat/data/chatFileChanges';
import type { ToolActivity } from './messageData';
import { ReadCodeLines } from './ReadCodeLines';
import { detectFileLanguage, highlightFileLines, useHighlightingState } from './FileSyntaxHighlight';
import { DiffContent } from '../ChatFileChangesDiffViewer';
import { PathHeading, PreviewBlock, ResultFootnote, ToolResultNotice } from './ToolSharedComponents';
import {
  asRecord,
  formatBytes,
  formatToolPath,
  getRequestedToolPath,
  getRequestedSearchQuery,
  integerValue,
  isNonNegativeInteger,
  pathName,
  stringValue,
  type ToolRecord,
} from './toolResultUtils';

const HighlightedReadLines = lazy(() =>
  import('./HighlightedReadLines').then((module) => ({ default: module.HighlightedReadLines })),
);

export function FileListingResult({
  activity,
  output,
  requestedPath,
}: {
  activity: ToolActivity;
  output: ToolRecord;
  requestedPath: string | null;
}) {
  const { t } = useTranslation();
  const entries = output.entries;
  if (!Array.isArray(entries)) return <ToolResultNotice message={t('toolListingUnavailable')} />;
  const parsedEntries = entries.flatMap((entry): Array<{ path: string; name: string; isDirectory: boolean }> => {
    const item = asRecord(entry);
    if (
      !item ||
      typeof item.path !== 'string' ||
      item.path.length === 0 ||
      (item.type !== 'file' && item.type !== 'directory')
    )
      return [];
    return [{ path: item.path, name: pathName(item.path), isDirectory: item.type === 'directory' }];
  });
  if (parsedEntries.length !== entries.length) return <ToolResultNotice message={t('toolListingUnavailable')} />;
  const location = requestedPath ?? stringValue(asRecord(activity.arguments)?.cwd);
  const hasMore = isNonNegativeInteger(output.nextOffset);

  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        {location ? (
          <>
            <FolderOpen aria-hidden="true" className="shrink-0" size={15} />
            <TitleTooltip title={location}><span className="min-w-0 flex-1 truncate" >
              {formatToolPath(location)}
            </span></TitleTooltip>
          </>
        ) : (
          <span className="min-w-0 flex-1" />
        )}
        <span className="shrink-0">{t('toolFileCount', { count: parsedEntries.length })}</span>
      </div>
      {parsedEntries.length === 0 ? (
        <ToolResultNotice message={t('toolEmptyListing')} />
      ) : (
        <ul className="max-h-56 overflow-auto rounded-control border border-border bg-surface py-1">
          {parsedEntries.map((entry) => (
            <li className="flex min-h-8 items-center gap-2 px-2.5 text-xs text-text" key={entry.path}>
              {entry.isDirectory ? (
                <Folder aria-hidden="true" className="shrink-0 text-accent-icon" size={16} />
              ) : (
                <File aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
              )}
              <TitleTooltip title={entry.name}><span className="min-w-0 flex-1 truncate" >
                {entry.name}
              </span></TitleTooltip>
            </li>
          ))}
        </ul>
      )}
      {hasMore ? <ResultFootnote message={t('toolMoreFilesAvailable')} /> : null}
      {output.truncated === true || (activity.status === 'completed' && output.incomplete === true) ? (
        <ResultFootnote message={t('toolListingIncomplete')} />
      ) : null}
    </div>
  );
}

export function SearchResult({
  activity,
  output,
  onOpenFile,
}: {
  activity: ToolActivity;
  output: ToolRecord;
  onOpenFile?: (path: string, content: string) => void;
}) {
  const { t } = useTranslation();
  if (!Array.isArray(output.matches)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const matches = output.matches.flatMap((value): Array<{ path: string; line: number; text: string }> => {
    const item = asRecord(value);
    const line = integerValue(item?.line);
    if (
      !item ||
      typeof item.path !== 'string' ||
      !item.path ||
      line === null ||
      line < 1 ||
      typeof item.text !== 'string'
    )
      return [];
    return [{ path: item.path, line, text: item.text }];
  });
  if (matches.length !== output.matches.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const query = getRequestedSearchQuery(activity) ?? '';
  return (
    <div>
      {matches.length === 0 ? (
        <ToolResultNotice message={t('toolSearchNoMatches')} />
      ) : (
        <ul className="max-h-64 overflow-auto border-y border-border">
          {matches.map((match) => (
            <li
              className="grid gap-1 border-b border-border px-3 py-2 last:border-b-0"
              key={`${match.path}:${match.line}`}
            >
              <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
                <File aria-hidden="true" className="shrink-0" size={14} />
                {onOpenFile ? (
                  <Button
                    aria-label={t('workspaceFileContent', { name: formatToolPath(match.path) })}
                    className="h-auto min-h-0 min-w-0 flex-1 justify-start truncate bg-transparent p-0 text-left text-xs text-secondary-text hover:bg-transparent hover:text-accent"
                    onClick={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      onOpenFile(match.path, match.text);
                    }}
                    size="small"
                    title={formatToolPath(match.path)}
                    type="button"
                    variant="ghost"
                  >
                    {formatToolPath(match.path)}
                  </Button>
                ) : (
                  <TitleTooltip title={match.path}><span className="min-w-0 flex-1 truncate" >
                    {formatToolPath(match.path)}
                  </span></TitleTooltip>
                )}
                <span className="shrink-0 rounded-control bg-composer px-1.5 py-0.5">{match.line}</span>
              </div>
              <pre className="whitespace-pre-wrap break-words pl-5 font-code text-xs leading-5 text-text">
                <HighlightedSearchLine path={match.path} query={query} text={match.text} />
              </pre>
            </li>
          ))}
        </ul>
      )}
      {isNonNegativeInteger(output.nextOffset) ? (
        <ResultFootnote message={t('toolSearchMoreResults')} />
      ) : output.truncated === true ? (
        <ResultFootnote message={t('toolOperationTruncated')} />
      ) : null}
    </div>
  );
}

function HighlightedSearchLine({ path, query, text }: { path: string; query: string; text: string }) {
  useHighlightingState();
  const tokens = highlightFileLines(detectFileLanguage(path), [text])[0] ?? [text];
  return tokens.map((token) => {
    if (!isValidElement<{ children?: ReactNode; className?: string }>(token)) {
      return typeof token === 'string' ? renderSearchMatch(token, query) : token;
    }
    const children = token.props.children;
    return cloneElement(
      token,
      {},
      typeof children === 'string' ? renderSearchMatch(children, query) : children,
    );
  });
}

function renderSearchMatch(text: string, query: string): ReactNode {
  if (!query) return text;
  const parts: ReactNode[] = [];
  const normalizedText = text.toLocaleLowerCase();
  const normalizedQuery = query.toLocaleLowerCase();
  let cursor = 0;
  let matchIndex = normalizedText.indexOf(normalizedQuery, cursor);
  while (matchIndex >= 0) {
    if (matchIndex > cursor) parts.push(text.slice(cursor, matchIndex));
    parts.push(
      <mark className="rounded-sm bg-accent/20 text-accent" key={`${matchIndex}-${parts.length}`}>
        {text.slice(matchIndex, matchIndex + query.length)}
      </mark>,
    );
    cursor = matchIndex + query.length;
    matchIndex = normalizedText.indexOf(normalizedQuery, cursor);
  }
  if (cursor === 0) return text;
  if (cursor < text.length) parts.push(text.slice(cursor));
  return parts;
}

export function ReadResult({ output }: { output: ToolRecord }) {
  const { t } = useTranslation();
  const path = stringValue(output.path) ?? stringValue(output.p);
  const rawLines = output.lines ?? output.d;
  if (!path || !Array.isArray(rawLines)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const lines = rawLines.flatMap((value): Array<{ number: number; text: string }> => {
    if (Array.isArray(value)) {
      const number = integerValue(value[0]);
      const text = value[1];
      return number !== null && number > 0 && typeof text === 'string' ? [{ number, text }] : [];
    }
    const item = asRecord(value);
    const number = integerValue(item?.line);
    return item && number !== null && number > 0 && typeof item.text === 'string' ? [{ number, text: item.text }] : [];
  });
  if (lines.length !== rawLines.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const plainTextLines = lines.map((line) => line.text || ' ');
  return (
    <div>
      {lines.length === 0 ? (
        <ToolResultNotice message={t('toolReadNoLines')} />
      ) : (
        <Suspense fallback={<ReadCodeLines contents={plainTextLines} lines={lines} />}>
          <HighlightedReadLines lines={lines} path={path} />
        </Suspense>
      )}
    </div>
  );
}

export function getReadFileContent(output: unknown): string | null {
  const record = asRecord(output);
  const rawLines = record?.lines ?? record?.d;
  if (!Array.isArray(rawLines)) return null;
  const lines = rawLines.map((value): string | null => {
    if (Array.isArray(value)) return typeof value[1] === 'string' ? value[1] : null;
    const item = asRecord(value);
    return stringValue(item?.text);
  });
  return lines.some((line) => line === null) ? null : lines.join('\n');
}

export function FileInfoResult({ output, locale }: { output: ToolRecord; locale: string }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const type = output.type;
  const size = integerValue(output.size);
  if (!path || (type !== 'file' && type !== 'directory') || size === null || size < 0)
    return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const isDirectory = type === 'directory';
  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-3 rounded-control border border-border bg-surface p-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-control bg-selected text-accent-icon">
          {isDirectory ? <Folder aria-hidden="true" size={20} /> : <File aria-hidden="true" size={20} />}
        </span>
        <div className="min-w-0">
          <TitleTooltip title={path}><p className="truncate text-sm font-semibold text-text" >
            {pathName(path)}
          </p></TitleTooltip>
          <TitleTooltip title={path}><p className="mt-0.5 break-all text-xs text-secondary-text" >
            {formatToolPath(path)}
          </p></TitleTooltip>
        </div>
      </div>
      <div className="flex flex-wrap gap-2 text-xs text-secondary-text">
        <span className="rounded-control border border-border px-2 py-1">
          {t(isDirectory ? 'toolFileTypeDirectory' : 'toolFileTypeFile')}
        </span>
        {!isDirectory ? (
          <span className="rounded-control border border-border px-2 py-1">{formatBytes(size, locale)}</span>
        ) : null}
      </div>
    </div>
  );
}

export function WriteResult({
  activity,
  output,
  locale,
}: {
  activity: ToolActivity;
  output: ToolRecord;
  locale: string;
}) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const size = integerValue(output.bytesWritten);
  if (output.success !== true || !path || size === null || size < 0)
    return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const content = stringValue(asRecord(activity.arguments)?.content);
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex items-center gap-2 text-sm font-medium text-success">
        <Check aria-hidden="true" size={16} />
        {t('toolWriteSuccess', { size: formatBytes(size, locale) })}
      </p>
      <PathHeading path={path} />
      {content !== null ? <PreviewBlock label={t('toolFilePreview')} value={content} /> : null}
    </div>
  );
}

export function EditResult({ activity, conversationId }: { activity: ToolActivity; conversationId: string | null }) {
  const requestedPath = getRequestedToolPath(activity);
  const editedFile = requestedPath ? findMatchingFileChange(activity.fileChanges, requestedPath) : null;
  const editedFileId = editedFile?.id ?? null;
  const editedFilePath = editedFile?.path ?? requestedPath ?? '';
  const [diff, setDiff] = useState<ChatFileChangeDiff | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let current = true;
    if (!conversationId || !editedFileId) {
      setDiff({ path: editedFilePath, isBinary: false, available: false, diff: '', truncated: false });
      setIsLoading(false);
      return () => {
        current = false;
      };
    }

    setIsLoading(true);
    void readConversationFileChangeDiff(conversationId, editedFileId)
      .then((result) => {
        if (!current) return;
        setDiff(result);
        setIsLoading(false);
      })
      .catch(() => {
        if (!current) return;
        setDiff({ path: editedFilePath, isBinary: false, available: false, diff: '', truncated: false });
        setIsLoading(false);
      });
    return () => {
      current = false;
    };
  }, [conversationId, editedFileId, editedFilePath]);

  return (
    <div className="flex h-64 min-h-0 flex-col overflow-hidden">
      <DiffContent
        canRevertHunks={false}
        diff={diff}
        flush
        isLoading={isLoading}
        mutatingHunkIndex={null}
        onRevertHunk={() => undefined}
        viewMode="unified"
      />
    </div>
  );
}

function findMatchingFileChange(activityChanges: ToolActivity['fileChanges'], requestedPath: string) {
  const normalizePath = (path: string) => path.replaceAll('\\', '/').toLowerCase();
  const normalizedRequestedPath = normalizePath(requestedPath);
  const exactMatch = activityChanges.find((change) => normalizePath(change.path) === normalizedRequestedPath);
  if (exactMatch) return exactMatch;

  const fileName = normalizedRequestedPath.split('/').at(-1);
  if (!fileName) return null;
  const nameMatches = activityChanges.filter((change) => normalizePath(change.path).split('/').at(-1) === fileName);
  return nameMatches.length === 1 ? nameMatches[0] : null;
}
