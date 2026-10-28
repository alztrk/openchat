import { FileCode2 } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { highlightFileLines } from './message/FileSyntaxHighlight';

export interface FileWorkspaceTab {
  id: string;
  kind: 'file';
  conversationId: string | null;
  fileName: string;
  content: string;
  language: string | null;
  changeId: string | null;
}

export function WorkspaceFileView({ file }: { file: FileWorkspaceTab }) {
  const { t } = useTranslation();
  const lines = useMemo(
    () => file.content.split(/\r\n|\r|\n/).map((text, index) => ({ number: index + 1, text })),
    [file.content],
  );
  const highlightedLines = useMemo(
    () =>
      highlightFileLines(
        file.language,
        lines.map((line) => line.text),
      ),
    [file.language, lines],
  );

  return (
    <section aria-label={t('workspaceFileContent', { name: file.fileName })} className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 border-b border-border px-3 text-xs text-secondary-text">
        <FileCode2 aria-hidden="true" className="shrink-0 text-accent" size={14} />
        <span className="truncate" title={file.fileName}>
          {file.fileName}
        </span>
        <span className="ml-auto shrink-0">{t('workspaceFileLineCount', { count: lines.length })}</span>
      </div>
      {file.content.length === 0 ? (
        <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-xs text-secondary-text">
          {t('workspaceFileEmpty')}
        </p>
      ) : (
        <ol
          aria-label={t('workspaceFileContent', { name: file.fileName })}
          className="openchat-scroll-area min-h-0 flex-1 overflow-auto py-2 font-code text-xs leading-5"
        >
          {lines.map((line, index) => (
            <li className="grid grid-cols-tool-lines gap-2 px-3" key={line.number}>
              <span aria-hidden="true" className="select-none text-right text-secondary-text">
                {line.number}
              </span>
              <span className="min-w-0 whitespace-pre-wrap break-words text-text">{highlightedLines[index]}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
