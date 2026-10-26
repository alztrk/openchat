import { Check, Copy } from 'lucide-react';
import { Fragment, type ReactNode, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { isWithinRichMarkdownBudget } from '@/modules/conversations/data/messageMarkdownBudget';
import { withStableKeys } from '@/shared/utils/reactKeys';
import { CitationSourceDialog } from './CitationSourceDialog';
import { CitationSources } from './CitationSources';
import { type CitationSource, isSafeHttpUrl } from './messageData';

interface MessageMarkdownProps {
  allowExternalLinks?: boolean;
  content: string;
  citationSources: readonly CitationSource[];
  generateHeadingIds?: boolean;
  imageResolver?: (source: string) => string | null;
  linkResolver?: (destination: string) => string | null;
}

interface MarkdownRenderOptions {
  allowExternalLinks: boolean;
  generateHeadingIds: boolean;
  imageResolver: ((source: string) => string | null) | null;
  linkResolver: ((destination: string) => string | null) | null;
}

const MAX_MARKDOWN_LIST_DEPTH = 64;

interface CodeBlockProps {
  code: string;
  language?: string;
}

function CodeBlock({ code, language }: CodeBlockProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard access might be blocked in some browser/webview contexts
    }
  };

  const copyLabel = copied ? t('toolTerminalCopied', 'Copied') : t('toolCopyContent', 'Copy');

  return (
    <div className="relative my-3 overflow-hidden rounded-control border border-border/40 bg-composer">
      <div className="flex items-center justify-between border-b border-border/40 px-3 py-1.5 text-xs text-secondary-text select-none">
        <span className="font-code font-medium">{language || 'text'}</span>
        <Button
          size="small"
          variant="ghost"
          onClick={handleCopy}
          className="px-2 py-0.5 text-xs"
          title={copyLabel}
          aria-label={copyLabel}
        >
          {copied ? <Check className="size-3.5 text-green-500" /> : <Copy className="size-3.5" />}
          <span>{copyLabel}</span>
        </Button>
      </div>
      <pre className="max-w-full overflow-x-auto p-3 text-sm">
        <code className="font-code whitespace-pre-wrap break-words">{code}</code>
      </pre>
    </div>
  );
}

/** Renders conversation Markdown while keeping provider-controlled links constrained to safe targets. */
export function MessageMarkdown({
  allowExternalLinks = true,
  content,
  citationSources,
  generateHeadingIds = false,
  imageResolver = undefined,
  linkResolver = undefined,
}: MessageMarkdownProps) {
  const sourceById = useMemo(() => new Map(citationSources.map((source) => [source.id, source])), [citationSources]);
  const [selectedSource, setSelectedSource] = useState<CitationSource | null>(null);
  const options: MarkdownRenderOptions = useMemo(
    () => ({
      allowExternalLinks,
      generateHeadingIds,
      imageResolver: imageResolver ?? null,
      linkResolver: linkResolver ?? null,
    }),
    [allowExternalLinks, generateHeadingIds, imageResolver, linkResolver],
  );
  return (
    <>
      <div className="conversation-message-content min-w-0 break-words text-text">
        {isWithinRichMarkdownBudget(content) ? (
          <MarkdownBlocks
            content={content}
            onCitationSelect={setSelectedSource}
            options={options}
            sourceById={sourceById}
          />
        ) : (
          <CodeBlock code={content} language="" />
        )}
        {citationSources.length > 0 ? <CitationSources onSelect={setSelectedSource} sources={citationSources} /> : null}
      </div>
      <CitationSourceDialog
        onOpenChange={(open) => {
          if (!open) setSelectedSource(null);
        }}
        open={selectedSource !== null}
        source={selectedSource}
      />
    </>
  );
}

interface MarkdownBlocksProps {
  content: string;
  onCitationSelect(source: CitationSource): void;
  options: MarkdownRenderOptions;
  sourceById: ReadonlyMap<string, CitationSource>;
}

function MarkdownBlocks({ content, onCitationSelect, options, sourceById }: MarkdownBlocksProps) {
  const lines = content.replaceAll('\r\n', '\n').replaceAll('\r', '\n').split('\n');
  const blocks: ReactNode[] = [];
  const headingOccurrences = new Map<string, number>();
  let index = 0;
  let blockIndex = 0;
  const nextHeadingId = (value: string): string | undefined => {
    const slug = headingSlug(value);
    if (slug.length === 0) return undefined;
    const occurrence = headingOccurrences.get(slug) ?? 0;
    headingOccurrences.set(slug, occurrence + 1);
    return occurrence === 0 ? slug : `${slug}-${occurrence}`;
  };
  while (index < lines.length) {
    const line = lines[index] ?? '';
    if (line.trim().length === 0) {
      index += 1;
      continue;
    }

    if (/^\s{0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$/.test(line)) {
      blocks.push(<hr className="my-4 border-border" key={`rule-${blockIndex}`} />);
      blockIndex += 1;
      index += 1;
      continue;
    }

    const fence = /^(?:\s{0,3})(```|~~~)([^`]*)$/.exec(line);
    if (fence) {
      const marker = fence[1] ?? '```';
      const language = (fence[2] ?? '').trim();
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length && !(lines[index] ?? '').trimStart().startsWith(marker)) {
        codeLines.push(lines[index] ?? '');
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push(<CodeBlock code={codeLines.join('\n')} key={`code-${blockIndex}`} language={language} />);
      blockIndex += 1;
      continue;
    }

    const heading = /^(\s{0,3})(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (heading) {
      blocks.push(
        <MarkdownHeading
          content={heading[3] ?? ''}
          key={`heading-${blockIndex}`}
          level={Math.min(6, heading[2]?.length ?? 1)}
          headingId={options.generateHeadingIds ? nextHeadingId(heading[3] ?? '') : undefined}
          options={options}
          onCitationSelect={onCitationSelect}
          sourceById={sourceById}
        />,
      );
      blockIndex += 1;
      index += 1;
      continue;
    }

    if (/^\s{0,3}>/.test(line)) {
      const quoteLines: string[] = [];
      while (index < lines.length && /^\s{0,3}>/.test(lines[index] ?? '')) {
        quoteLines.push((lines[index] ?? '').replace(/^\s{0,3}>\s?/, ''));
        index += 1;
      }
      blocks.push(
        <blockquote className="my-3 border-l-2 border-border pl-3 text-secondary-text" key={`quote-${blockIndex}`}>
          {renderInline(quoteLines.join('\n'), sourceById, `quote-${blockIndex}`, onCitationSelect, options)}
        </blockquote>,
      );
      blockIndex += 1;
      continue;
    }

    const list = parseMarkdownList(lines, index, sourceById, onCitationSelect, `list-${blockIndex}`, options);
    if (list) {
      blocks.push(list.node);
      index = list.nextIndex;
      blockIndex += 1;
      continue;
    }

    const table = parseTable(lines, index, sourceById, blockIndex, onCitationSelect, options);
    if (table) {
      blocks.push(table.node);
      index = table.nextIndex;
      blockIndex += 1;
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const current = lines[index] ?? '';
      const isSetextHeadingRule = /^(?:\s{0,3})(?:=+|-+)\s*$/.test(current);
      if (
        current.trim().length === 0 ||
        isBlockStart(current) ||
        isTableStart(lines, index) ||
        (paragraphLines.length > 0 && isSetextHeadingRule)
      )
        break;
      paragraphLines.push(current);
      index += 1;
    }
    const setextHeading = /^(?:\s{0,3})(=+|-+)\s*$/.exec(lines[index] ?? '');
    if (setextHeading && paragraphLines.length > 0) {
      blocks.push(
        <MarkdownHeading
          content={paragraphLines.join('\n')}
          key={`heading-${blockIndex}`}
          level={setextHeading[1]?.startsWith('=') ? 1 : 2}
          headingId={options.generateHeadingIds ? nextHeadingId(paragraphLines.join('\n')) : undefined}
          options={options}
          onCitationSelect={onCitationSelect}
          sourceById={sourceById}
        />,
      );
      index += 1;
    } else {
      blocks.push(
        <p className="my-2 whitespace-pre-wrap" key={`paragraph-${blockIndex}`}>
          {renderInline(paragraphLines.join('\n'), sourceById, `paragraph-${blockIndex}`, onCitationSelect, options)}
        </p>,
      );
    }
    blockIndex += 1;
  }
  return blocks;
}

interface MarkdownHeadingProps {
  content: string;
  headingId: string | undefined;
  level: number;
  options: MarkdownRenderOptions;
  onCitationSelect(source: CitationSource): void;
  sourceById: ReadonlyMap<string, CitationSource>;
}

function MarkdownHeading({ content, headingId, level, options, onCitationSelect, sourceById }: MarkdownHeadingProps) {
  const className = [
    'mb-2 mt-4 font-semibold text-text first:mt-0',
    options.generateHeadingIds ? 'model-readme-heading' : '',
  ]
    .filter(Boolean)
    .join(' ');
  const children = renderInline(content, sourceById, `heading-${level}`, onCitationSelect, options);
  switch (level) {
    case 1:
      return (
        <h1 className={className} id={headingId}>
          {children}
        </h1>
      );
    case 2:
      return (
        <h2 className={className} id={headingId}>
          {children}
        </h2>
      );
    case 3:
      return (
        <h3 className={className} id={headingId}>
          {children}
        </h3>
      );
    case 4:
      return (
        <h4 className={className} id={headingId}>
          {children}
        </h4>
      );
    case 5:
      return (
        <h5 className={className} id={headingId}>
          {children}
        </h5>
      );
    default:
      return (
        <h6 className={className} id={headingId}>
          {children}
        </h6>
      );
  }
}

function renderInline(
  value: string,
  sourceById: ReadonlyMap<string, CitationSource>,
  keyPrefix: string,
  onCitationSelect: (source: CitationSource) => void,
  options: MarkdownRenderOptions,
): ReactNode[] {
  const tokenPattern =
    /(!\[[^\]\n]*\]\((?:<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\)|`[^`\n]+`|\*\*[^*\n]+\*\*|__[^_\n]+__|~~[^~\n]+~~|\[[^\]\n]+\]\((?:<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\)|<https?:\/\/[^>\s]+>|https?:\/\/[^\s<>]+|www\.[^\s<>]+|\[[PSU]\d+(?:-[A-Za-z0-9]+)?\]|\*[^*\n]+\*|_[^_\n]+_)/gi;
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let tokenIndex = 0;
  for (const match of value.matchAll(tokenPattern)) {
    const token = match[0];
    const start = match.index ?? cursor;
    if (start > cursor) nodes.push(value.slice(cursor, start));
    const key = `${keyPrefix}-${tokenIndex}`;
    if (/^(?:<https?:\/\/|https?:\/\/|www\.)/i.test(token)) {
      nodes.push(
        options.allowExternalLinks
          ? renderAutolink(token, key)
          : token.startsWith('<') && token.endsWith('>')
            ? token.slice(1, -1)
            : token,
      );
    } else {
      nodes.push(renderInlineToken(token, sourceById, key, onCitationSelect, options));
    }
    cursor = start + token.length;
    tokenIndex += 1;
  }
  if (cursor < value.length) nodes.push(value.slice(cursor));
  return nodes;
}

interface ListMarker {
  content: string;
  indent: number;
  ordered: boolean;
  start: number | null;
}

interface ParsedMarkdownList {
  node: ReactNode;
  nextIndex: number;
}

interface MarkdownListItem {
  content: string;
  nested: ReactNode[];
}

/** Parses one list and flattens deeper provider-controlled nesting after a fixed depth. */
function parseMarkdownList(
  lines: readonly string[],
  startIndex: number,
  sourceById: ReadonlyMap<string, CitationSource>,
  onCitationSelect: (source: CitationSource) => void,
  keyPrefix: string,
  options: MarkdownRenderOptions,
  depth = 0,
): ParsedMarkdownList | null {
  const firstMarker = parseListMarker(lines[startIndex] ?? '');
  if (!firstMarker) return null;

  const items: MarkdownListItem[] = [];
  let index = startIndex;
  while (index < lines.length) {
    const marker = parseListMarker(lines[index] ?? '');
    if (!marker || marker.indent !== firstMarker.indent || marker.ordered !== firstMarker.ordered) break;

    const item: MarkdownListItem = { content: marker.content, nested: [] };
    index += 1;
    while (index < lines.length) {
      const currentLine = lines[index] ?? '';
      const nestedMarker = parseListMarker(currentLine);
      if (nestedMarker && nestedMarker.indent > firstMarker.indent) {
        if (depth >= MAX_MARKDOWN_LIST_DEPTH) {
          const lineIndent = /^\s*/.exec(currentLine)?.[0].length ?? 0;
          item.content += `\n${currentLine.slice(Math.min(lineIndent, firstMarker.indent + 2))}`;
          index += 1;
          continue;
        }
        const nested = parseMarkdownList(
          lines,
          index,
          sourceById,
          onCitationSelect,
          `${keyPrefix}-nested-${items.length}`,
          options,
          depth + 1,
        );
        if (!nested) break;
        item.nested.push(nested.node);
        index = nested.nextIndex;
        continue;
      }
      if (nestedMarker) break;
      if (currentLine.trim().length === 0) {
        let nextIndex = index + 1;
        while (nextIndex < lines.length && (lines[nextIndex] ?? '').trim().length === 0) nextIndex += 1;
        const nextMarker = parseListMarker(lines[nextIndex] ?? '');
        if (nextMarker && nextMarker.indent >= firstMarker.indent) {
          index = nextIndex;
          if (nextMarker.indent > firstMarker.indent) continue;
        }
        break;
      }
      const lineIndent = /^(\s*)/.exec(currentLine)?.[1]?.length ?? 0;
      if (lineIndent <= firstMarker.indent) break;
      item.content += `\n${currentLine.slice(Math.min(lineIndent, firstMarker.indent + 2))}`;
      index += 1;
    }
    items.push(item);
  }

  const List = firstMarker.ordered ? 'ol' : 'ul';
  const listProps = firstMarker.ordered && firstMarker.start !== null ? { start: firstMarker.start } : {};
  const keyedItems = withStableKeys(items, (item) => item.content, `${keyPrefix}-item`);
  return {
    node: (
      <List
        className={`${firstMarker.ordered ? 'list-decimal' : 'list-disc'} my-3 space-y-1 pl-6`}
        key={keyPrefix}
        {...listProps}
      >
        {keyedItems.map(({ item, key }) => (
          <li className="whitespace-pre-wrap" key={key}>
            {renderInline(item.content, sourceById, key, onCitationSelect, options)}
            {item.nested}
          </li>
        ))}
      </List>
    ),
    nextIndex: index,
  };
}

function parseListMarker(line: string): ListMarker | null {
  const match = /^(\s*)([-+*]|(\d+)[.)])\s+(.*)$/.exec(line);
  if (!match) return null;
  const number = match[3];
  const parsedStart = number === undefined ? null : Number(number);
  return {
    content: match[4] ?? '',
    indent: match[1]?.length ?? 0,
    ordered: number !== undefined,
    start: parsedStart !== null && Number.isSafeInteger(parsedStart) ? parsedStart : null,
  };
}

function renderAutolink(token: string, key: string): ReactNode {
  const displayText = token.startsWith('<') && token.endsWith('>') ? token.slice(1, -1) : token;
  let url = displayText;
  while (/[.,!?;:*_~]$/.test(url)) url = url.slice(0, -1);
  while (url.endsWith(')') && (url.match(/\(/g)?.length ?? 0) < (url.match(/\)/g)?.length ?? 0)) {
    url = url.slice(0, -1);
  }
  while (url.endsWith(']') && (url.match(/\[/g)?.length ?? 0) < (url.match(/\]/g)?.length ?? 0)) {
    url = url.slice(0, -1);
  }
  if (url.length === 0) return token;
  const href = url.toLocaleLowerCase('en').startsWith('www.') ? `https://${url}` : url;
  if (!isSafeHttpUrl(href)) return token;
  return (
    <Fragment key={key}>
      <a
        className="text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover"
        href={href}
        rel="noopener noreferrer"
        target="_blank"
      >
        {url}
      </a>
      {displayText.slice(url.length)}
    </Fragment>
  );
}

function renderInlineToken(
  token: string,
  sourceById: ReadonlyMap<string, CitationSource>,
  key: string,
  onCitationSelect: (source: CitationSource) => void,
  options: MarkdownRenderOptions,
): ReactNode {
  const image = /^!\[([^\]]*)\]\((<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\)$/i.exec(token);
  if (image && options.imageResolver) {
    const source = stripMarkdownDestination(image[2] ?? '');
    const src = options.imageResolver(source);
    if (src !== null && isSafeHttpUrl(src)) {
      return (
        <img
          alt={image[1] ?? ''}
          className="model-readme-markdown-image"
          decoding="async"
          key={key}
          loading="lazy"
          referrerPolicy="no-referrer"
          src={src}
        />
      );
    }
    return image[1] ?? '';
  }
  if (token.startsWith('`') && token.endsWith('`')) {
    return (
      <code className="rounded bg-composer px-1 py-0.5 font-code text-relative-small" key={key}>
        {token.slice(1, -1)}
      </code>
    );
  }
  if (token.startsWith('**') && token.endsWith('**')) {
    return <strong key={key}>{renderInline(token.slice(2, -2), sourceById, key, onCitationSelect, options)}</strong>;
  }
  if (token.startsWith('__') && token.endsWith('__')) {
    return <strong key={key}>{renderInline(token.slice(2, -2), sourceById, key, onCitationSelect, options)}</strong>;
  }
  if (token.startsWith('~~') && token.endsWith('~~')) {
    return <del key={key}>{renderInline(token.slice(2, -2), sourceById, key, onCitationSelect, options)}</del>;
  }
  const link = /^\[([^\]]+)\]\((<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\)$/i.exec(token);
  if (link) {
    const destination = stripMarkdownDestination(link[2] ?? '');
    const href = options.linkResolver
      ? options.linkResolver(destination)
      : isSafeHttpUrl(destination)
        ? destination
        : null;
    if (href === null || !isSafeMarkdownLinkTarget(href)) return link[1] ?? token;
    if (!options.allowExternalLinks) return link[1] ?? token;
    const isFragment = href.startsWith('#');
    return (
      <a
        className="text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover"
        href={href}
        key={key}
        rel={isFragment ? undefined : 'noopener noreferrer'}
        target={isFragment ? undefined : '_blank'}
      >
        {link[1]}
      </a>
    );
  }
  const citation = /^\[([PSU]\d+(?:-[A-Za-z0-9]+)?)\]$/.exec(token);
  if (citation) {
    const source = sourceById.get(citation[1] ?? '');
    if (source) {
      return (
        <button
          aria-haspopup="dialog"
          aria-label={source.title}
          className="cursor-pointer border-0 bg-transparent p-0 text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          key={key}
          onClick={() => onCitationSelect(source)}
          title={source.title}
          type="button"
        >
          {token}
        </button>
      );
    }
  }
  if (token.startsWith('*') && token.endsWith('*')) {
    return <em key={key}>{renderInline(token.slice(1, -1), sourceById, key, onCitationSelect, options)}</em>;
  }
  if (token.startsWith('_') && token.endsWith('_')) {
    return <em key={key}>{renderInline(token.slice(1, -1), sourceById, key, onCitationSelect, options)}</em>;
  }
  return token;
}

function stripMarkdownDestination(value: string): string {
  const trimmed = value.trim();
  const unwrapped = trimmed.startsWith('<') && trimmed.endsWith('>') ? trimmed.slice(1, -1) : trimmed;
  return unwrapped.replace(/\\([\\()])/g, '$1');
}

function isSafeMarkdownLinkTarget(value: string): boolean {
  return isSafeHttpUrl(value) || /^#[\p{L}\p{N}_:.%-]*$/u.test(value);
}

function headingSlug(value: string): string {
  return value
    .replace(/<[^>]*>/g, ' ')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[`*_~]/g, '')
    .normalize('NFKD')
    .replace(/\p{Diacritic}/gu, '')
    .toLocaleLowerCase('en')
    .replace(/[^\p{Letter}\p{Number}\s-]/gu, '')
    .trim()
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-');
}

interface ParsedTable {
  node: ReactNode;
  nextIndex: number;
}

interface TableRow {
  cells: string[];
}

function parseTable(
  lines: readonly string[],
  startIndex: number,
  sourceById: ReadonlyMap<string, CitationSource>,
  blockIndex: number,
  onCitationSelect: (source: CitationSource) => void,
  options: MarkdownRenderOptions,
): ParsedTable | null {
  const header = splitTableRow(lines[startIndex] ?? '');
  const delimiter = splitTableRow(lines[startIndex + 1] ?? '');
  if (!header || !delimiter || !isTableDelimiter(delimiter.cells)) return null;

  const columnCount = Math.max(header.cells.length, delimiter.cells.length);
  const alignments = Array.from({ length: columnCount }, (_, columnIndex) =>
    tableAlignment(delimiter.cells[columnIndex] ?? ''),
  );
  const rows: TableRow[] = [{ cells: normalizeTableCells(header.cells, columnCount) }];
  let nextIndex = startIndex + 2;
  while (nextIndex < lines.length) {
    const row = splitTableRow(lines[nextIndex] ?? '');
    if (!row) break;
    rows.push({ cells: normalizeTableCells(row.cells, columnCount) });
    nextIndex += 1;
  }
  const headerCells = withStableKeys(rows[0]?.cells ?? [], (cell) => cell, `table-${blockIndex}-header`).map(
    ({ item, key }, index) => ({ item, key, alignment: alignments[index] }),
  );
  const keyedRows = withStableKeys(rows.slice(1), (row) => row.cells.join('\0'), `table-${blockIndex}-row`).map(
    ({ item: row, key }) => ({
      key,
      cells: withStableKeys(row.cells, (cell) => cell, `${key}-cell`),
    }),
  );

  return {
    node: (
      <div className="my-3 max-w-full overflow-x-auto" key={`table-${blockIndex}`}>
        <table className="w-full border-collapse text-left text-sm">
          <thead>
            <tr>
              {headerCells.map(({ item: cell, key, alignment }) => (
                <th
                  className="border border-border bg-composer px-2 py-1 font-semibold"
                  key={key}
                  scope="col"
                  style={{ textAlign: alignment }}
                >
                  {renderInline(cell, sourceById, key, onCitationSelect, options)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {keyedRows.map(({ cells, key }) => (
              <tr key={key}>
                {cells.map(({ item: cell, key: cellKey }, columnIndex) => (
                  <td
                    className="border border-border px-2 py-1 align-top"
                    key={cellKey}
                    style={{ textAlign: alignments[columnIndex] }}
                  >
                    {renderInline(cell, sourceById, cellKey, onCitationSelect, options)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    ),
    nextIndex,
  };
}

function isTableStart(lines: readonly string[], index: number): boolean {
  const header = splitTableRow(lines[index] ?? '');
  const delimiter = splitTableRow(lines[index + 1] ?? '');
  return header !== null && delimiter !== null && isTableDelimiter(delimiter.cells);
}

function splitTableRow(line: string): TableRow | null {
  const trimmed = line.trim();
  if (!trimmed.includes('|')) return null;
  const source = trimmed.startsWith('|') ? trimmed.slice(1) : trimmed;
  const withoutTrailingPipe = source.endsWith('|') ? source.slice(0, -1) : source;
  const cells: string[] = [];
  let current = '';
  let escaped = false;
  let inCode = false;
  for (const character of withoutTrailingPipe) {
    if (escaped) {
      current += character;
      escaped = false;
      continue;
    }
    if (character === '\\') {
      escaped = true;
      continue;
    }
    if (character === '`') inCode = !inCode;
    if (character === '|' && !inCode) {
      cells.push(current.trim());
      current = '';
      continue;
    }
    current += character;
  }
  if (escaped) current += '\\';
  cells.push(current.trim());
  return cells.length > 0 ? { cells } : null;
}

function isTableDelimiter(cells: readonly string[]): boolean {
  return cells.length > 0 && cells.every((cell) => /^:?-+:?$/.test(cell.trim()));
}

function tableAlignment(cell: string): 'left' | 'center' | 'right' | undefined {
  const trimmed = cell.trim();
  if (trimmed.startsWith(':') && trimmed.endsWith(':')) return 'center';
  if (trimmed.startsWith(':')) return 'left';
  if (trimmed.endsWith(':')) return 'right';
  return undefined;
}

function normalizeTableCells(cells: readonly string[], columnCount: number): string[] {
  return Array.from({ length: columnCount }, (_, index) => cells[index] ?? '');
}

function isBlockStart(line: string): boolean {
  return /^(?:\s{0,3})(?:```|~~~|#{1,6}\s|>|[-+*]\s|\d+[.)]\s|(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$)/.test(line);
}
