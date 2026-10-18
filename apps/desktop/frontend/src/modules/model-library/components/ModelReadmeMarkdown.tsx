import { createElement, type ReactNode } from 'react';

import { MessageMarkdown } from '@/modules/conversations/components/message/MessageMarkdown';
import { isSafeHttpUrl } from '@/modules/conversations/components/message/messageData';
import { parseInertHtmlFragment } from '@/shared/utils/inertHtml';
import { withStableKeys } from '@/shared/utils/reactKeys';

type ReadmeBlock =
  | { kind: 'markdown'; content: string }
  | { kind: 'html'; content: string }
  | { kind: 'details'; content: string; summary: string };

const detailsOpen = /^\s*<details(?:\s[^>]*)?>\s*$/i;
const detailsClose = /^\s*<\/details\s*>\s*$/i;
const codeFence = /^\s{0,3}(`{3,}|~{3,})/;
const htmlBlockOpen = /^\s*<(table|div|p|ul|ol|pre|blockquote|h[1-6]|section|figure|dl|details)(?:\s[^>]*)?>/i;
const MAX_MODEL_README_DETAILS_DEPTH = 32;
const MAX_SAFE_HTML_DEPTH = 64;

interface ModelReadmeMarkdownProps {
  content: string;
  repoId: string;
  revision: string;
}

/** Renders model README content with safe links, an HTML allowlist, and bounded nested disclosures. */
export function ModelReadmeMarkdown({ content, repoId, revision }: ModelReadmeMarkdownProps) {
  return <ModelReadmeMarkdownContent content={content} depth={0} repoId={repoId} revision={revision} />;
}

/** Renders one README disclosure level; deeper model-authored disclosures are rendered as plain text. */
function ModelReadmeMarkdownContent({
  content,
  depth,
  repoId,
  revision,
}: ModelReadmeMarkdownProps & { depth: number }) {
  const blocks = splitReadme(stripModelCardMetadata(content));
  const resolveUrl = (value: string) => resolveModelCardUrl(value, repoId, revision);
  const keyedBlocks = withStableKeys(
    blocks,
    (block) => `${block.kind}\0${block.kind === 'details' ? block.summary : ''}\0${block.content}`,
    'readme-block',
  );

  return (
    <div className="model-readme-markdown grid gap-2">
      {keyedBlocks.map(({ item: block, key }) =>
        block.kind === 'markdown' ? (
          <MessageMarkdown
            allowExternalLinks
            citationSources={[]}
            content={normalizeInlineHtml(block.content)}
            generateHeadingIds
            imageResolver={resolveUrl}
            key={key}
            linkResolver={resolveUrl}
          />
        ) : block.kind === 'html' ? (
          <SafeHtmlFragment content={block.content} key={key} resolveUrl={resolveUrl} />
        ) : (
          <details
            className="openchat-native-disclosure rounded-control border border-border bg-composer px-3 py-2"
            key={key}
          >
            <summary className="cursor-pointer font-medium text-text">{block.summary}</summary>
            <div className="pt-2">
              {depth < MAX_MODEL_README_DETAILS_DEPTH ? (
                <ModelReadmeMarkdownContent
                  content={block.content}
                  depth={depth + 1}
                  repoId={repoId}
                  revision={revision}
                />
              ) : (
                <pre className="whitespace-pre-wrap break-words text-sm text-secondary-text">{block.content}</pre>
              )}
            </div>
          </details>
        ),
      )}
    </div>
  );
}

function stripModelCardMetadata(source: string): string {
  const normalized = source
    .replace(/^\uFEFF/, '')
    .replaceAll('\r\n', '\n')
    .replaceAll('\r', '\n');
  const lines = normalized.split('\n');
  if (lines[0]?.trim() !== '---') return normalized;
  const closingIndex = lines.findIndex((line, index) => index > 0 && /^(?:---|\.\.\.)\s*$/.test(line.trim()));
  return closingIndex < 0
    ? normalized
    : lines
        .slice(closingIndex + 1)
        .join('\n')
        .trimStart();
}

function resolveModelCardUrl(value: string, repoId: string, revision: string): string | null {
  const destination = value.trim();
  if (destination.length === 0) return null;
  if (destination.startsWith('#')) return /^#[\p{L}\p{N}_:.%-]*$/u.test(destination) ? destination : null;

  const repositoryPath = repoId.split('/').map(encodeURIComponent).join('/');
  const baseUrl = `https://huggingface.co/${repositoryPath}/resolve/${encodeURIComponent(revision)}/`;
  try {
    const url = new URL(destination, baseUrl);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.href : null;
  } catch {
    return null;
  }
}

function splitReadme(source: string): ReadmeBlock[] {
  const lines = source.replaceAll('\r\n', '\n').replaceAll('\r', '\n').split('\n');
  const blocks: ReadmeBlock[] = [];
  let markdownLines: string[] = [];
  let fenceMarker: string | null = null;

  const flushMarkdown = () => {
    if (markdownLines.length === 0) return;
    blocks.push({ kind: 'markdown', content: markdownLines.join('\n') });
    markdownLines = [];
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? '';
    const fence = codeFence.exec(line)?.[1];
    if (fenceMarker !== null) {
      markdownLines.push(line);
      if (fence && fence[0] === fenceMarker[0] && fence.length >= fenceMarker.length) {
        fenceMarker = null;
      }
      continue;
    }
    if (fence) {
      fenceMarker = fence;
      markdownLines.push(line);
      continue;
    }
    if (!detailsOpen.test(line)) {
      const htmlOpening = htmlBlockOpen.exec(line);
      if (!htmlOpening) {
        markdownLines.push(line);
        continue;
      }
      const tagName = htmlOpening[1]?.toLowerCase();
      if (!tagName) {
        markdownLines.push(line);
        continue;
      }
      const endIndex = findHtmlBlockEnd(lines, index, tagName);
      if (endIndex === null) {
        markdownLines.push(line);
        continue;
      }
      flushMarkdown();
      blocks.push({ kind: 'html', content: lines.slice(index, endIndex + 1).join('\n') });
      index = endIndex;
      continue;
    }

    const endIndex = findDetailsEnd(lines, index);
    if (endIndex === null) {
      markdownLines.push(line);
      continue;
    }

    const inside = lines.slice(index + 1, endIndex);
    const summary = takeSummary(inside);
    if (summary === null) {
      markdownLines.push(...lines.slice(index, endIndex + 1));
      index = endIndex;
      continue;
    }

    flushMarkdown();
    blocks.push({
      kind: 'details',
      summary: summary.text,
      content: [...inside.slice(0, summary.start), ...inside.slice(summary.end + 1)].join('\n'),
    });
    index = endIndex;
  }

  flushMarkdown();
  return blocks;
}

function findDetailsEnd(lines: readonly string[], startIndex: number): number | null {
  let depth = 1;
  let fenceMarker: string | null = null;
  for (let index = startIndex + 1; index < lines.length; index += 1) {
    const line = lines[index] ?? '';
    const fence = codeFence.exec(line)?.[1];
    if (fenceMarker !== null) {
      if (fence && fence[0] === fenceMarker[0] && fence.length >= fenceMarker.length) fenceMarker = null;
      continue;
    }
    if (fence) {
      fenceMarker = fence;
      continue;
    }
    if (detailsOpen.test(line)) depth += 1;
    if (detailsClose.test(line)) {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return null;
}

function takeSummary(lines: readonly string[]): { start: number; end: number; text: string } | null {
  const opening = lines.findIndex((line) => /<summary(?:\s[^>]*)?>/i.test(line));
  if (opening < 0) return null;
  const closing = lines.slice(opening).findIndex((line) => /<\/summary\s*>/i.test(line));
  if (closing < 0) return null;
  const end = opening + closing;
  const html = lines.slice(opening, end + 1).join('\n');
  const match = /<summary(?:\s[^>]*)?>([\s\S]*?)<\/summary\s*>/i.exec(html);
  const text = match?.[1]?.replace(/<[^>]*>/g, '').trim();
  return text ? { start: opening, end, text } : null;
}

function findHtmlBlockEnd(lines: readonly string[], startIndex: number, tagName: string): number | null {
  const tags = /<\s*(\/?)\s*([a-z][a-z0-9:-]*)\b[^>]*>/gi;
  let depth = 0;
  for (let index = startIndex; index < lines.length; index += 1) {
    for (const match of (lines[index] ?? '').matchAll(tags)) {
      if (match[2]?.toLowerCase() !== tagName) continue;
      const tag = match[0];
      if (match[1]) {
        depth -= 1;
        if (depth === 0) return index;
      } else if (!/\/\s*>$/.test(tag)) {
        depth += 1;
      }
    }
  }
  return null;
}

function normalizeInlineHtml(content: string): string {
  let fenceMarker: string | null = null;
  return content
    .replaceAll('\r\n', '\n')
    .replaceAll('\r', '\n')
    .split('\n')
    .map((line) => {
      const fence = codeFence.exec(line)?.[1];
      if (fenceMarker !== null) {
        if (fence && fence[0] === fenceMarker[0] && fence.length >= fenceMarker.length) fenceMarker = null;
        return line;
      }
      if (fence) {
        fenceMarker = fence;
        return line;
      }

      const output: string[] = [];
      const codeSpan = /(`+)[^`]*?\1/g;
      let cursor = 0;
      for (const match of line.matchAll(codeSpan)) {
        const start = match.index ?? cursor;
        if (start > cursor) output.push(normalizeInlineHtmlText(line.slice(cursor, start)));
        output.push(match[0]);
        cursor = start + match[0].length;
      }
      if (cursor < line.length) output.push(normalizeInlineHtmlText(line.slice(cursor)));
      return output.join('');
    })
    .join('\n');
}

function normalizeInlineHtmlText(content: string): string {
  const withImages = content.replace(/<img\b([^>]*)\/?>/gi, (_match, attributes: string) => {
    const source = readHtmlAttribute(attributes, 'src');
    if (source === null) return '';
    const alt = readHtmlAttribute(attributes, 'alt') ?? '';
    const safeAlt = alt.replaceAll('\\', '\\\\').replaceAll(']', '\\]');
    return `![${safeAlt}](<${source.replace(/[<>]/g, '')}>)`;
  });
  const withLinks = withImages.replace(
    /<a\b([^>]*)>([\s\S]*?)<\/a\s*>/gi,
    (_match, attributes: string, children: string) => {
      const href = readHtmlAttribute(attributes, 'href');
      return href === null ? children : `[${children}](${href})`;
    },
  );
  return withLinks
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<(strong|b)(?:\s[^>]*)?>/gi, '**')
    .replace(/<\/(?:strong|b)\s*>/gi, '**')
    .replace(/<(em|i)(?:\s[^>]*)?>/gi, '_')
    .replace(/<\/(?:em|i)\s*>/gi, '_')
    .replace(/<(del|s)(?:\s[^>]*)?>/gi, '~~')
    .replace(/<\/(?:del|s)\s*>/gi, '~~')
    .replace(/<code(?:\s[^>]*)?>/gi, '`')
    .replace(/<\/code\s*>/gi, '`')
    .replace(/<a\b[^>]*>/gi, '')
    .replace(/<\/a\s*>/gi, '')
    .replace(/<img\b[^>]*\/?>/gi, '')
    .replace(/<\/?(?:span|small|sup|sub|u)\b[^>]*>/gi, '');
}

function readHtmlAttribute(attributes: string, name: string): string | null {
  let index = 0;
  while (index < attributes.length) {
    while (index < attributes.length && /[\s/]/.test(attributes[index] ?? '')) index += 1;
    const nameStart = index;
    while (index < attributes.length && !/[\s=/>]/.test(attributes[index] ?? '')) index += 1;
    if (index === nameStart) {
      index += 1;
      continue;
    }

    const attributeName = attributes.slice(nameStart, index).toLowerCase();
    while (index < attributes.length && /\s/.test(attributes[index] ?? '')) index += 1;
    if (attributes[index] !== '=') continue;
    index += 1;
    while (index < attributes.length && /\s/.test(attributes[index] ?? '')) index += 1;

    const quote = attributes[index] === '"' || attributes[index] === "'" ? attributes[index] : null;
    if (quote !== null) index += 1;
    const valueStart = index;
    if (quote !== null) {
      while (index < attributes.length && attributes[index] !== quote) index += 1;
    } else {
      while (index < attributes.length && !/[\s>]/.test(attributes[index] ?? '')) index += 1;
    }
    const value = attributes.slice(valueStart, index);
    if (quote !== null && attributes[index] === quote) index += 1;

    if (attributeName === name) return value.length <= 4096 ? value : null;
  }
  return null;
}

/** Converts model-authored HTML to an allowlisted React tree without executing embedded markup. */
function SafeHtmlFragment({ content, resolveUrl }: { content: string; resolveUrl(value: string): string | null }) {
  const parsed = parseInertHtmlFragment(content);
  if (parsed === null) return <>{content}</>;
  const nodes = Array.from(parsed.childNodes).map((node, index) =>
    renderSafeHtmlNode(node, `readme-${index}`, resolveUrl),
  );
  return <div className="conversation-message-content min-w-0 break-words text-text">{nodes}</div>;
}

/** Converts allowlisted README elements and flattens overly deep external markup to text. */
function renderSafeHtmlNode(
  node: Node,
  key: string,
  resolveUrl: (value: string) => string | null,
  depth = 0,
): ReactNode {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent;
  if (!(node instanceof Element)) return null;
  const element = node;
  const tagName = element.localName.toLowerCase();
  if (blockedHtmlTags.has(tagName)) return null;
  if (depth >= MAX_SAFE_HTML_DEPTH) return element.textContent;
  const children = Array.from(element.childNodes).map((child, index) =>
    renderSafeHtmlNode(child, `${key}-${index}`, resolveUrl, depth + 1),
  );
  const className = htmlTagClasses[tagName];
  const props: { key: string; className?: string; colSpan?: number; rowSpan?: number; start?: number; id?: string } = {
    key,
  };
  if (className) props.className = className;
  const id = element.getAttribute('id');
  if (id !== null && /^[A-Za-z][\w:.-]{0,127}$/.test(id)) props.id = id;
  const colSpan = safeHtmlInteger(element.getAttribute('colspan'));
  const rowSpan = safeHtmlInteger(element.getAttribute('rowspan'));
  const start = safeHtmlInteger(element.getAttribute('start'));
  if (tagName === 'td' || tagName === 'th') {
    if (colSpan !== null) props.colSpan = colSpan;
    if (rowSpan !== null) props.rowSpan = rowSpan;
  }
  if (tagName === 'ol' && start !== null) props.start = start;
  if (tagName === 'br' || tagName === 'hr') return createElement(tagName, props);
  if (tagName === 'a') {
    const rawHref = element.getAttribute('href');
    const href = rawHref === null ? null : resolveUrl(rawHref);
    if (href === null || (!isSafeHttpUrl(href) && !/^#[\p{L}\p{N}_:.%-]*$/u.test(href))) {
      return createElement('span', props, children);
    }
    const isFragment = href.startsWith('#');
    return createElement(
      'a',
      {
        ...props,
        className: 'text-accent underline decoration-accent/50 underline-offset-2 hover:text-accent-hover',
        href,
        rel: isFragment ? undefined : 'noopener noreferrer',
        target: isFragment ? undefined : '_blank',
      },
      children,
    );
  }
  if (tagName === 'img') {
    const rawSrc = element.getAttribute('src');
    const src = rawSrc === null ? null : resolveUrl(rawSrc);
    if (src === null || !isSafeHttpUrl(src)) return null;
    return createElement('img', {
      ...props,
      alt: element.getAttribute('alt') ?? '',
      className: 'model-readme-markdown-image',
      decoding: 'async',
      loading: 'lazy',
      referrerPolicy: 'no-referrer',
      src,
      title: element.getAttribute('title') ?? undefined,
    });
  }
  if (tagName === 'table') {
    return (
      <div className="model-readme-table-scroll" key={key}>
        {createElement(tagName, props, children)}
      </div>
    );
  }
  if (safeHtmlTags.has(tagName)) return createElement(tagName, props, children);
  return <>{children}</>;
}

function safeHtmlInteger(value: string | null): number | null {
  if (value === null || !/^\d{1,3}$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

const blockedHtmlTags = new Set([
  'script',
  'style',
  'iframe',
  'object',
  'embed',
  'form',
  'input',
  'textarea',
  'select',
  'option',
  'button',
  'video',
  'audio',
  'source',
  'link',
  'meta',
  'base',
  'svg',
  'canvas',
]);

const safeHtmlTags = new Set([
  'p',
  'div',
  'span',
  'figure',
  'figcaption',
  'dl',
  'dt',
  'dd',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'strong',
  'b',
  'em',
  'i',
  'del',
  's',
  'ul',
  'ol',
  'li',
  'blockquote',
  'pre',
  'code',
  'table',
  'thead',
  'tbody',
  'tfoot',
  'tr',
  'th',
  'td',
  'br',
  'hr',
  'details',
  'summary',
  'sup',
  'sub',
  'a',
  'img',
  'mark',
  'kbd',
  'q',
]);

const htmlTagClasses: Record<string, string> = {
  p: 'my-2 whitespace-pre-wrap',
  figure: 'my-3 grid gap-2',
  figcaption: 'text-sm text-secondary-text',
  dl: 'my-3 grid gap-2',
  dt: 'font-semibold',
  dd: 'text-secondary-text',
  h1: 'mb-2 mt-4 text-xl font-semibold first:mt-0',
  h2: 'mb-2 mt-4 text-lg font-semibold first:mt-0',
  h3: 'mb-2 mt-4 text-base font-semibold first:mt-0',
  h4: 'mb-2 mt-4 text-sm font-semibold first:mt-0',
  h5: 'mb-2 mt-4 text-sm font-semibold first:mt-0',
  h6: 'mb-2 mt-4 text-sm font-semibold first:mt-0',
  pre: 'my-3 max-w-full overflow-x-auto whitespace-pre-wrap rounded-control bg-composer px-3 py-3 text-sm',
  code: 'font-code',
  blockquote: 'my-3 border-l-2 border-border pl-3 text-secondary-text',
  ul: 'my-2 list-inside list-disc',
  ol: 'my-2 list-inside list-decimal',
  table: 'my-3 w-full border-collapse text-sm',
  th: 'border border-border p-1.5 text-left font-semibold',
  td: 'border border-border p-1.5 align-top',
  details: 'rounded-control border border-border bg-composer px-3 py-2',
  summary: 'cursor-pointer font-medium text-text',
};
