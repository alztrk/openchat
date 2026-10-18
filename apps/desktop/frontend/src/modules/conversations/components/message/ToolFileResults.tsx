import { Check, File, FileText, Folder, FolderOpen, Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { ToolActivity } from './messageData';
import { PathHeading, PreviewBlock, ResultFootnote, ToolResultNotice } from './ToolSharedComponents';
import {
  asRecord,
  formatBytes,
  formatToolPath,
  integerValue,
  isNonNegativeInteger,
  pathName,
  stringValue,
  type ToolRecord,
} from './toolResultUtils';

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
            <span className="min-w-0 flex-1 truncate" title={location}>
              {formatToolPath(location, t)}
            </span>
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
              <span className="min-w-0 flex-1 truncate" title={entry.name}>
                {entry.name}
              </span>
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

export function SearchResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
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
  const query = stringValue(asRecord(activity.arguments)?.query) ?? stringValue(output.query);
  return (
    <div className="grid gap-2 pb-2">
      <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <Search aria-hidden="true" className="shrink-0 text-accent-icon" size={15} />
        <span className="min-w-0 flex-1 truncate font-semibold text-text">{query ?? ''}</span>
        <span className="shrink-0">{t('toolSearchMatchCount', { count: matches.length })}</span>
      </div>
      {matches.length === 0 ? (
        <ToolResultNotice message={t('toolSearchNoMatches')} />
      ) : (
        <ul className="max-h-64 overflow-auto rounded-control border border-border bg-surface">
          {matches.map((match) => (
            <li
              className="grid gap-1 border-b border-border px-2.5 py-2 last:border-b-0"
              key={`${match.path}:${match.line}`}
            >
              <div className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
                <File aria-hidden="true" className="shrink-0" size={14} />
                <span className="min-w-0 flex-1 truncate" title={match.path}>
                  {formatToolPath(match.path, t)}
                </span>
                <span className="shrink-0 rounded-control bg-composer px-1.5 py-0.5">{match.line}</span>
              </div>
              <pre className="whitespace-pre-wrap break-words pl-5 font-code text-xs leading-5 text-text">
                {match.text}
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

export function ReadResult({ output }: { output: ToolRecord }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  if (!path || !Array.isArray(output.lines)) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const lines = output.lines.flatMap((value): Array<{ number: number; text: string }> => {
    const item = asRecord(value);
    const number = integerValue(item?.line);
    return item && number !== null && number > 0 && typeof item.text === 'string' ? [{ number, text: item.text }] : [];
  });
  if (lines.length !== output.lines.length) return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
        <FileText aria-hidden="true" size={15} />
        <span className="truncate" title={path}>
          {formatToolPath(path, t)}
        </span>
      </p>
      {lines.length === 0 ? (
        <ToolResultNotice message={t('toolReadNoLines')} />
      ) : (
        <ol className="max-h-64 overflow-auto rounded-control border border-border bg-surface py-1">
          {lines.map((line) => (
            <li className="grid grid-cols-tool-lines gap-2 px-2.5 py-0.5 font-code text-xs leading-5" key={line.number}>
              <span className="text-right text-secondary-text">{line.number}</span>
              <span className="whitespace-pre-wrap break-words text-text">{line.text || ' '}</span>
            </li>
          ))}
        </ol>
      )}
      <p className="text-xs text-secondary-text">{t('toolReadLineCount', { count: lines.length })}</p>
      {isNonNegativeInteger(output.nextLine) ? <ResultFootnote message={t('toolReadMoreLines')} /> : null}
    </div>
  );
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
          <p className="truncate text-sm font-semibold text-text" title={path}>
            {pathName(path)}
          </p>
          <p className="mt-0.5 break-all text-xs text-secondary-text" title={path}>
            {formatToolPath(path, t)}
          </p>
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

export function EditResult({ activity, output }: { activity: ToolActivity; output: ToolRecord }) {
  const { t } = useTranslation();
  const path = stringValue(output.path);
  const replacements = integerValue(output.replacements);
  if (output.success !== true || !path || replacements === null || replacements < 1)
    return <ToolResultNotice message={t('toolOperationUnavailable')} />;
  const argumentsValue = asRecord(activity.arguments);
  const before = stringValue(argumentsValue?.oldString) ?? stringValue(argumentsValue?.old_string);
  const after = stringValue(argumentsValue?.newString) ?? stringValue(argumentsValue?.new_string);
  return (
    <div className="grid gap-2 pb-2">
      <p className="flex items-center gap-2 text-sm font-medium text-success">
        <Check aria-hidden="true" size={16} />
        {t('toolEditSuccess', { count: replacements })}
      </p>
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
