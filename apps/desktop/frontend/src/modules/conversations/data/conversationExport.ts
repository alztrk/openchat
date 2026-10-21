import { replaceAsciiControlCharacters } from '@/shared/utils/textValidation';
import { type ChatConversation, type ChatMessage, getMessages, saveConversationExport } from './chatStorage';
import { cleanMarkdown } from './messageCopyUtils';

export type ExportFormat = 'md' | 'html' | 'json';

export interface ConversationMarkdownLabels {
  locale: string;
  provider: string;
  modelUnavailable: string;
  user: string;
  assistant: string;
  exportProvider: string;
  exportModel: string;
  exportCreated: string;
  exportStatus: string;
  exportToolActivity: string;
  exportMessagesCount: string;
  exportTitleBadge: string;
  exportInput: string;
  exportOutput: string;
  exportTargetPath: string;
  responseInProgress: string;
  responseFailed: string;
  responseStopped: string;
  reasoningSummary: string;
  toolAwaitingApproval: string;
  toolWaitingForUser: string;
  toolRunning: string;
  toolCompleted: string;
  toolFailed: string;
  toolDenied: string;
  toolCancelled: string;
  toolStatus(status: ExportToolStatus): string;
  secondsShort(seconds: number): string;
  formatDateTime(date: number | string | Date | null | undefined): string;
  formatTime(date: number | string | Date | null | undefined): string;
}

export type ExportToolStatus =
  | 'awaitingApproval'
  | 'waitingForUser'
  | 'running'
  | 'completed'
  | 'failed'
  | 'denied'
  | 'cancelled';

export interface ReasoningSummary {
  content: string;
  elapsedMicroseconds: number | null;
}

export interface ToolActivity {
  name: string;
  status: ExportToolStatus;
  arguments: unknown;
  targetPath: string | null;
  hasOutput: boolean;
  output: unknown;
}

export function parseReasoningSummaries(value: string): ReasoningSummary[] {
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item): ReasoningSummary => {
      if (!isRecord(item) || typeof item.content !== 'string') {
        throw new Error('A reasoning summary is invalid.');
      }
      const elapsed = item.elapsedMicroseconds;
      return {
        content: item.content,
        elapsedMicroseconds:
          typeof elapsed === 'number' && Number.isFinite(elapsed) && elapsed >= 0 ? elapsed : null,
      };
    });
  } catch {
    return [];
  }
}

export function parseToolActivities(value: string): ToolActivity[] {
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item): ToolActivity => {
      if (!isRecord(item) || typeof item.name !== 'string' || !Object.hasOwn(item, 'arguments')) {
        throw new Error('A tool activity is invalid.');
      }
      const status: ExportToolStatus = isToolStatus(item.status) ? item.status : 'completed';
      return {
        name: item.name,
        status,
        arguments: item.arguments,
        targetPath: typeof item.targetPath === 'string' ? item.targetPath : null,
        hasOutput: Object.hasOwn(item, 'output'),
        output: item.output,
      };
    });
  } catch {
    return [];
  }
}

function isToolStatus(value: unknown): value is ExportToolStatus {
  return (
    value === 'awaitingApproval' ||
    value === 'waitingForUser' ||
    value === 'running' ||
    value === 'completed' ||
    value === 'failed' ||
    value === 'denied' ||
    value === 'cancelled'
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function markdownCodeFence(content: string): string {
  let longestRun = 2;
  for (const match of content.matchAll(/`+/g)) {
    if (match[0].length > longestRun) longestRun = match[0].length;
  }
  return '`'.repeat(longestRun + 1);
}

export function buildConversationMarkdown(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
  labels: ConversationMarkdownLabels,
): string {
  const modelLabel = conversation.modelId ?? labels.modelUnavailable;
  const createdDate = labels.formatDateTime(conversation.createdAt);
  const markdown = [
    `# ${conversation.title}`,
    '',
    `> **${labels.exportProvider}:** ${labels.provider}  `,
    `> **${labels.exportModel}:** ${modelLabel}  `,
    `> **${labels.exportCreated}:** ${createdDate}  `,
    `> **${labels.exportMessagesCount}:** ${messages.length}`,
    '',
    '---',
    '',
  ];

  for (const message of messages) {
    const isUser = message.role === 'user';
    const speaker = isUser ? labels.user : `${labels.assistant} (${modelLabel})`;
    const timestamp = message.createdAt === null ? '' : ` · ${labels.formatDateTime(message.createdAt)}`;
    markdown.push(`## ${speaker}${timestamp}`, '');

    if (message.status !== 'completed') {
      const status =
        message.status === 'streaming'
          ? labels.responseInProgress
          : message.status === 'failed'
            ? labels.responseFailed
            : labels.responseStopped;
      markdown.push(`> **${labels.exportStatus}:** ${status}`, '');
    }

    for (const summary of parseReasoningSummaries(message.reasoningSummaries)) {
      if (!summary.content.trim()) continue;
      const duration =
        summary.elapsedMicroseconds === null
          ? ''
          : ` · ${labels.secondsShort(Math.floor(summary.elapsedMicroseconds / 1_000_000))}`;
      markdown.push(
        '<details>',
        `<summary><strong>${labels.reasoningSummary}${duration}</strong></summary>`,
        '',
        summary.content,
        '',
        '</details>',
        '',
      );
    }

    for (const activity of parseToolActivities(message.toolActivities)) {
      const toolData: Record<string, unknown> = {
        name: activity.name,
        status: labels.toolStatus(activity.status),
        arguments: activity.arguments,
      };
      if (activity.targetPath !== null) toolData.targetPath = activity.targetPath;
      if (activity.hasOutput) toolData.output = activity.output;
      const formatted = JSON.stringify(toolData, null, 2);
      if (formatted !== undefined) {
        const fence = markdownCodeFence(formatted);
        markdown.push(
          '<details>',
          `<summary><strong>${labels.exportToolActivity}:</strong> <code>${activity.name}</code> (${labels.toolStatus(activity.status)})</summary>`,
          '',
          `${fence}json`,
          formatted,
          fence,
          '',
          '</details>',
          '',
        );
      }
    }

    const content = message.role === 'assistant' ? cleanMarkdown(message.content) : message.content;
    markdown.push(content, '', '---', '');
  }

  return markdown.join('\n');
}

export function buildConversationJson(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
): string {
  const transcript = {
    $schema: 'https://openchat.dev/schemas/transcript-v1.json',
    version: 1,
    conversation: {
      id: conversation.id,
      title: conversation.title,
      createdAt: new Date(conversation.createdAt).toISOString(),
      updatedAt: new Date(conversation.updatedAt).toISOString(),
      providerId: conversation.providerId,
      modelId: conversation.modelId,
      projectId: conversation.projectId,
      isBookmarked: conversation.isBookmarked,
      isPinned: conversation.isPinned,
      tags: conversation.tags,
    },
    messages: messages.map((message) => ({
      id: message.id,
      role: message.role,
      createdAt: message.createdAt !== null ? new Date(message.createdAt).toISOString() : null,
      status: message.status,
      providerId: message.providerId,
      modelId: message.modelId,
      content: message.role === 'assistant' ? cleanMarkdown(message.content) : message.content,
      rawContent: message.content,
      reasoningSummaries: parseReasoningSummaries(message.reasoningSummaries),
      toolActivities: parseToolActivities(message.toolActivities),
      attachments: message.attachments,
    })),
  };
  return JSON.stringify(transcript, null, 2);
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function parseMarkdownTable(lines: string[]): string {
  if (lines.length < 2) {
    return lines.map((line) => `<p>${renderInlineMarkdown(line)}</p>`).join('\n');
  }

  const parseCells = (row: string) => {
    let clean = row.trim();
    if (clean.startsWith('|')) clean = clean.slice(1);
    if (clean.endsWith('|')) clean = clean.slice(0, -1);
    return clean.split('|').map((c) => c.trim());
  };

  const headerCells = parseCells(lines[0] ?? '');
  const delimiterCells = parseCells(lines[1] ?? '');

  if (!delimiterCells.every((cell) => /^:?-+:?$/.test(cell))) {
    return lines.map((line) => `<p>${renderInlineMarkdown(line)}</p>`).join('\n');
  }

  const alignments = delimiterCells.map((c) => {
    if (c.startsWith(':') && c.endsWith(':')) return 'center';
    if (c.endsWith(':')) return 'right';
    return 'left';
  });

  const thead = `<thead><tr>${headerCells
    .map((cell, idx) => {
      const align = alignments[idx] ?? 'left';
      return `<th style="text-align:${align}">${renderInlineMarkdown(cell)}</th>`;
    })
    .join('')}</tr></thead>`;

  const tbodyRows = lines.slice(2).map((rowLine) => {
    const cells = parseCells(rowLine);
    return `<tr>${cells
      .map((cell, idx) => {
        const align = alignments[idx] ?? 'left';
        return `<td style="text-align:${align}">${renderInlineMarkdown(cell)}</td>`;
      })
      .join('')}</tr>`;
  });

  return `<div class="table-container"><table>${thead}<tbody>${tbodyRows.join('')}</tbody></table></div>`;
}

function renderMarkdownToHtml(markdown: string): string {
  const lines = markdown.split(/\r?\n/);
  const result: string[] = [];
  let inCodeBlock = false;
  let codeBlockLang = '';
  let codeBlockLines: string[] = [];
  let inList: 'ul' | 'ol' | null = null;
  let tableLines: string[] = [];

  const closeListIfNeeded = () => {
    if (inList) {
      result.push(inList === 'ul' ? '</ul>' : '</ol>');
      inList = null;
    }
  };

  const flushTableIfNeeded = () => {
    if (tableLines.length > 0) {
      result.push(parseMarkdownTable(tableLines));
      tableLines = [];
    }
  };

  for (const line of lines) {
    const codeFenceMatch = line.match(/^(?:```|~~~)(.*)$/);
    if (codeFenceMatch) {
      flushTableIfNeeded();
      closeListIfNeeded();
      if (inCodeBlock) {
        const langBadge = codeBlockLang
          ? `<div class="code-header"><span class="code-lang">${escapeHtml(codeBlockLang)}</span></div>`
          : '';
        result.push(
          `<div class="code-card">${langBadge}<pre><code class="${codeBlockLang ? `language-${escapeHtml(codeBlockLang)}` : ''}">${escapeHtml(
            codeBlockLines.join('\n'),
          )}</code></pre></div>`,
        );
        inCodeBlock = false;
        codeBlockLines = [];
        codeBlockLang = '';
      } else {
        inCodeBlock = true;
        codeBlockLang = codeFenceMatch[1]?.trim() ?? '';
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockLines.push(line);
      continue;
    }

    const trimmed = line.trim();

    // Table line accumulation
    if (trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 1) {
      closeListIfNeeded();
      tableLines.push(trimmed);
      continue;
    }
    flushTableIfNeeded();

    if (!trimmed) {
      closeListIfNeeded();
      continue;
    }

    // Headings
    const headingMatch = trimmed.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      closeListIfNeeded();
      const level = headingMatch[1]?.length ?? 1;
      const text = renderInlineMarkdown(headingMatch[2] ?? '');
      result.push(`<h${level}>${text}</h${level}>`);
      continue;
    }

    // Blockquote
    const quoteMatch = trimmed.match(/^>\s?(.*)$/);
    if (quoteMatch) {
      closeListIfNeeded();
      const text = renderInlineMarkdown(quoteMatch[1] ?? '');
      result.push(`<blockquote><p>${text}</p></blockquote>`);
      continue;
    }

    // Task list item
    const taskMatch = trimmed.match(/^[-*+]\s+\[([ xX])\]\s+(.+)$/);
    if (taskMatch) {
      if (inList !== 'ul') {
        closeListIfNeeded();
        result.push('<ul class="task-list">');
        inList = 'ul';
      }
      const isChecked = taskMatch[1]?.toLowerCase() === 'x';
      result.push(
        `<li class="task-item"><input type="checkbox" disabled ${isChecked ? 'checked ' : ''}/><span>${renderInlineMarkdown(taskMatch[2] ?? '')}</span></li>`,
      );
      continue;
    }

    // Unordered list
    const ulMatch = trimmed.match(/^[-*+]\s+(.+)$/);
    if (ulMatch) {
      if (inList !== 'ul') {
        closeListIfNeeded();
        result.push('<ul>');
        inList = 'ul';
      }
      result.push(`<li>${renderInlineMarkdown(ulMatch[1] ?? '')}</li>`);
      continue;
    }

    // Ordered list
    const olMatch = trimmed.match(/^\d+\.\s+(.+)$/);
    if (olMatch) {
      if (inList !== 'ol') {
        closeListIfNeeded();
        result.push('<ol>');
        inList = 'ol';
      }
      result.push(`<li>${renderInlineMarkdown(olMatch[1] ?? '')}</li>`);
      continue;
    }

    // Horizontal rule
    if (/^(?:[-*_]\s*){3,}$/.test(trimmed)) {
      closeListIfNeeded();
      result.push('<hr />');
      continue;
    }

    // Regular paragraph
    closeListIfNeeded();
    result.push(`<p>${renderInlineMarkdown(line)}</p>`);
  }

  flushTableIfNeeded();
  if (inCodeBlock) {
    const langBadge = codeBlockLang
      ? `<div class="code-header"><span class="code-lang">${escapeHtml(codeBlockLang)}</span></div>`
      : '';
    result.push(
      `<div class="code-card">${langBadge}<pre><code class="${codeBlockLang ? `language-${escapeHtml(codeBlockLang)}` : ''}">${escapeHtml(
        codeBlockLines.join('\n'),
      )}</code></pre></div>`,
    );
  }
  closeListIfNeeded();

  return result.join('\n');
}

function renderInlineMarkdown(text: string): string {
  let escaped = escapeHtml(text);

  // Inline code: `code`
  escaped = escaped.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Bold & Italic: ***text*** or ___text___
  escaped = escaped.replace(/(\*\*\*|___)(.*?)\1/g, '<strong><em>$2</em></strong>');

  // Bold: **text** or __text__
  escaped = escaped.replace(/(\*\*|__)(.*?)\1/g, '<strong>$2</strong>');

  // Italic: *text* or _text_
  escaped = escaped.replace(/(^|[^\w])(\*|_)(.*?)\2([^\w]|$)/g, '$1<em>$3</em>$4');

  // Strikethrough: ~~text~~
  escaped = escaped.replace(/~~(.*?)~~/g, '<del>$1</del>');

  // Links: [text](url)
  escaped = escaped.replace(/\[([^\]]+)\]\(([^)]*)\)/g, (_match, linkText: string, url: string) => {
    const cleanUrl = url.trim();
    if (/^(?:https?:\/\/|mailto:)/i.test(cleanUrl)) {
      return `<a href="${cleanUrl}" rel="noopener noreferrer" target="_blank">${linkText}</a>`;
    }
    return linkText;
  });

  return escaped;
}

export function buildConversationHtml(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
  labels: ConversationMarkdownLabels,
): string {
  const modelLabel = conversation.modelId ?? labels.modelUnavailable;
  const createdDate = labels.formatDateTime(conversation.createdAt);

  const renderedMessages = messages
    .map((message) => {
      const isUser = message.role === 'user';
      const speaker = isUser ? labels.user : modelLabel;
      const roleBadge = isUser ? labels.user : labels.assistant;
      const formattedTimestamp = message.createdAt === null ? '' : labels.formatDateTime(message.createdAt);
      const isoTimestamp = message.createdAt === null ? '' : new Date(message.createdAt).toISOString();
      const roleClass = isUser ? 'message-user' : 'message-assistant';

      const statusBadge =
        message.status !== 'completed'
          ? `<div class="status-banner status-${escapeHtml(message.status)}">
              <svg class="status-icon" viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
                <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 1.5a5.5 5.5 0 1 1 0 11 5.5 5.5 0 0 1 0-11zm-.75 2.5v4.5h1.5V5h-1.5zm0 5.5v1.5h1.5V10.5h-1.5z"/>
              </svg>
              <span><strong>${escapeHtml(labels.exportStatus)}:</strong> ${escapeHtml(
                message.status === 'streaming'
                  ? labels.responseInProgress
                  : message.status === 'failed'
                    ? labels.responseFailed
                    : labels.responseStopped,
              )}</span>
            </div>`
          : '';

      const reasoningBlocks = parseReasoningSummaries(message.reasoningSummaries)
        .filter((s) => s.content.trim().length > 0)
        .map((summary) => {
          const duration =
            summary.elapsedMicroseconds === null
              ? ''
              : ` · ${labels.secondsShort(Math.floor(summary.elapsedMicroseconds / 1_000_000))}`;
          return `
          <details class="reasoning-block">
            <summary class="reasoning-summary">
              <span class="reasoning-label">
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 2a7 7 0 0 0-7 7c0 2.38 1.19 4.47 3 5.74V17a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-2.26c1.81-1.27 3-3.36 3-5.74a7 7 0 0 0-7-7z"/>
                  <path d="M9 21h6"/>
                </svg>
                <span>${escapeHtml(labels.reasoningSummary)}${escapeHtml(duration)}</span>
              </span>
              <svg class="chevron" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9"/>
              </svg>
            </summary>
            <div class="reasoning-content">${escapeHtml(summary.content)}</div>
          </details>`;
        })
        .join('\n');

      const toolBlocks = parseToolActivities(message.toolActivities)
        .map((activity) => {
          const toolData = {
            name: activity.name,
            status: labels.toolStatus(activity.status),
            arguments: activity.arguments,
            ...(activity.targetPath !== null ? { targetPath: activity.targetPath } : {}),
            ...(activity.hasOutput ? { output: activity.output } : {}),
          };
          const jsonText = JSON.stringify(toolData, null, 2);
          return `
          <details class="tool-activity-block">
            <summary class="tool-summary">
              <span class="tool-label">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="4 17 10 11 4 5"/>
                  <line x1="12" y1="19" x2="20" y2="19"/>
                </svg>
                <code class="tool-name">${escapeHtml(activity.name)}</code>
                <span class="tool-status status-pill-${escapeHtml(activity.status)}">${escapeHtml(labels.toolStatus(activity.status))}</span>
              </span>
              <svg class="chevron" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9"/>
              </svg>
            </summary>
            <div class="tool-payload">
              <pre><code>${escapeHtml(jsonText)}</code></pre>
            </div>
          </details>`;
        })
        .join('\n');

      const bodyText = message.role === 'assistant' ? cleanMarkdown(message.content) : message.content;
      const bodyHtml = renderMarkdownToHtml(bodyText);

      const avatarSvg = isUser
        ? `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
            <circle cx="12" cy="7" r="4"/>
          </svg>`
        : `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
          </svg>`;

      return `
      <article class="message ${roleClass}">
        <header class="message-header">
          <div class="speaker-meta">
            <div class="speaker-avatar" aria-hidden="true">${avatarSvg}</div>
            <span class="speaker-name">${escapeHtml(speaker)}</span>
            <span class="speaker-badge">${escapeHtml(roleBadge)}</span>
          </div>
          ${
            formattedTimestamp
              ? `<time class="timestamp" datetime="${escapeHtml(isoTimestamp)}">${escapeHtml(formattedTimestamp)}</time>`
              : ''
          }
        </header>
        ${statusBadge}
        ${reasoningBlocks}
        ${toolBlocks}
        <div class="message-body">${bodyHtml}</div>
      </article>`;
    })
    .join('\n');

  return `<!DOCTYPE html>
<html lang="${escapeHtml(labels.locale)}">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <meta name="generator" content="OpenChat" />
  <title>${escapeHtml(conversation.title)}</title>
  <style>
    :root {
      --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
      --color-bg: #f8fafc;
      --color-card: #ffffff;
      --color-card-border: #e2e8f0;
      --color-text: #0f172a;
      --color-text-secondary: #334155;
      --color-muted: #64748b;
      --color-user-bg: #f8faff;
      --color-user-border: #dbeafe;
      --color-user-accent: #2563eb;
      --color-assistant-accent: #4f46e5;
      --color-badge-bg: #f1f5f9;
      --color-badge-text: #475569;
      --color-code-bg: #0f172a;
      --color-code-header: #1e293b;
      --color-code-text: #f8fafc;
      --color-inline-code-bg: #f1f5f9;
      --color-inline-code-border: #e2e8f0;
      --color-link: #2563eb;
      --color-quote-border: #3b82f6;
      --color-table-border: #e2e8f0;
      --color-table-header: #f8fafc;
      --color-table-zebra: #fafbfd;
      --color-reasoning-bg: #faf5ff;
      --color-reasoning-border: #e9d5ff;
      --color-reasoning-text: #581c87;
      --color-tool-bg: #f8fafc;
      --color-tool-border: #e2e8f0;
      --color-chip-bg: #f1f5f9;
      --color-chip-border: #e2e8f0;
      --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.04);
      --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.06), 0 2px 4px -2px rgba(0, 0, 0, 0.04);
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --color-bg: #090d16;
        --color-card: #111827;
        --color-card-border: #1f293d;
        --color-text: #f8fafc;
        --color-text-secondary: #cbd5e1;
        --color-muted: #94a3b8;
        --color-user-bg: #141c2e;
        --color-user-border: #1e2e4a;
        --color-user-accent: #3b82f6;
        --color-assistant-accent: #818cf8;
        --color-badge-bg: #1e293b;
        --color-badge-text: #cbd5e1;
        --color-code-bg: #030712;
        --color-code-header: #0f172a;
        --color-code-text: #f9fafb;
        --color-inline-code-bg: #1e293b;
        --color-inline-code-border: #334155;
        --color-link: #60a5fa;
        --color-quote-border: #60a5fa;
        --color-table-border: #1e293b;
        --color-table-header: #0d121f;
        --color-table-zebra: #141b2b;
        --color-reasoning-bg: #1e1338;
        --color-reasoning-border: #3b1d60;
        --color-reasoning-text: #d8b4fe;
        --color-tool-bg: #0d121f;
        --color-tool-border: #1e293b;
        --color-chip-bg: #162032;
        --color-chip-border: #1f2d42;
        --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.3);
        --shadow-md: 0 4px 12px rgba(0, 0, 0, 0.4);
      }
    }
    * { box-sizing: border-box; }
    body {
      font-family: var(--font-sans);
      background-color: var(--color-bg);
      color: var(--color-text);
      line-height: 1.65;
      margin: 0;
      padding: 2.5rem 1rem 4rem;
      -webkit-font-smoothing: antialiased;
    }
    .container {
      max-width: 860px;
      margin: 0 auto;
    }
    .hero-header {
      background-color: var(--color-card);
      border: 1px solid var(--color-card-border);
      border-radius: 1rem;
      padding: 1.75rem 2rem;
      margin-bottom: 2rem;
      box-shadow: var(--shadow-sm);
    }
    .brand-row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-bottom: 0.75rem;
      font-size: 0.75rem;
      font-weight: 600;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      color: var(--color-muted);
    }
    .brand-badge {
      display: inline-flex;
      align-items: center;
      gap: 0.35rem;
      padding: 0.2rem 0.55rem;
      border-radius: 9999px;
      background-color: var(--color-badge-bg);
      color: var(--color-badge-text);
      font-size: 0.72rem;
      font-weight: 600;
    }
    .conversation-title {
      font-size: 1.85rem;
      font-weight: 700;
      line-height: 1.25;
      letter-spacing: -0.02em;
      margin: 0 0 1.25rem 0;
      color: var(--color-text);
    }
    .meta-grid {
      display: flex;
      flex-wrap: wrap;
      gap: 0.65rem;
    }
    .meta-chip {
      display: inline-flex;
      align-items: center;
      gap: 0.45rem;
      padding: 0.35rem 0.75rem;
      border-radius: 0.5rem;
      background-color: var(--color-chip-bg);
      border: 1px solid var(--color-chip-border);
      font-size: 0.8rem;
      color: var(--color-muted);
    }
    .meta-chip strong {
      color: var(--color-text);
      font-weight: 600;
    }
    .meta-chip svg {
      color: var(--color-muted);
      flex-shrink: 0;
    }
    .message-list {
      display: flex;
      flex-direction: column;
      gap: 1.25rem;
    }
    .message {
      padding: 1.35rem 1.6rem;
      border-radius: 0.875rem;
      background-color: var(--color-card);
      border: 1px solid var(--color-card-border);
      box-shadow: var(--shadow-sm);
      overflow-wrap: break-word;
      transition: border-color 0.15s ease;
    }
    .message-user {
      background-color: var(--color-user-bg);
      border-color: var(--color-user-border);
    }
    .message-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 0.9rem;
      gap: 0.75rem;
    }
    .speaker-meta {
      display: flex;
      align-items: center;
      gap: 0.55rem;
    }
    .speaker-avatar {
      width: 28px;
      height: 28px;
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: var(--color-badge-bg);
      color: var(--color-badge-text);
      flex-shrink: 0;
    }
    .message-user .speaker-avatar {
      background-color: #dbeafe;
      color: var(--color-user-accent);
    }
    @media (prefers-color-scheme: dark) {
      .message-user .speaker-avatar {
        background-color: #1e3a8a;
        color: #93c5fd;
      }
    }
    .speaker-name {
      font-weight: 650;
      font-size: 0.95rem;
      color: var(--color-text);
    }
    .speaker-badge {
      font-size: 0.7rem;
      font-weight: 600;
      padding: 0.15rem 0.45rem;
      border-radius: 0.25rem;
      background-color: var(--color-badge-bg);
      color: var(--color-badge-text);
      text-transform: uppercase;
      letter-spacing: 0.03em;
    }
    .timestamp {
      color: var(--color-muted);
      font-size: 0.775rem;
      white-space: nowrap;
    }
    .message-body {
      font-size: 0.95rem;
      color: var(--color-text-secondary);
      line-height: 1.65;
    }
    .message-body > *:first-child { margin-top: 0; }
    .message-body > *:last-child { margin-bottom: 0; }
    .message-body p { margin: 0.65rem 0; }
    .message-body h1, .message-body h2, .message-body h3, .message-body h4 {
      color: var(--color-text);
      font-weight: 650;
      margin: 1.25rem 0 0.5rem;
      line-height: 1.3;
    }
    .message-body h1 { font-size: 1.35rem; border-bottom: 1px solid var(--color-card-border); padding-bottom: 0.35rem; }
    .message-body h2 { font-size: 1.2rem; }
    .message-body h3 { font-size: 1.05rem; }
    .message-body h4 { font-size: 0.95rem; }
    .message-body ul, .message-body ol {
      margin: 0.5rem 0;
      padding-left: 1.35rem;
    }
    .message-body li { margin: 0.25rem 0; }
    .task-list { list-style: none; padding-left: 0 !important; }
    .task-item {
      display: flex;
      align-items: baseline;
      gap: 0.5rem;
      margin: 0.25rem 0;
    }
    .task-item input[type="checkbox"] {
      margin: 0;
      accent-color: var(--color-user-accent);
    }
    .code-card {
      margin: 0.85rem 0;
      border-radius: 0.5rem;
      overflow: hidden;
      background-color: var(--color-code-bg);
      border: 1px solid var(--color-card-border);
    }
    .code-header {
      background-color: var(--color-code-header);
      padding: 0.35rem 0.75rem;
      font-size: 0.725rem;
      font-weight: 600;
      color: #94a3b8;
      font-family: var(--font-mono);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    pre {
      margin: 0;
      padding: 0.9rem 1rem;
      overflow-x: auto;
      font-family: var(--font-mono);
      font-size: 0.85rem;
      line-height: 1.55;
      color: var(--color-code-text);
      background: transparent;
    }
    code {
      font-family: var(--font-mono);
      font-size: 0.875em;
    }
    p code, li code, td code, th code {
      background-color: var(--color-inline-code-bg);
      border: 1px solid var(--color-inline-code-border);
      padding: 0.12rem 0.35rem;
      border-radius: 0.25rem;
      font-size: 0.85em;
      color: var(--color-text);
    }
    blockquote {
      border-left: 3px solid var(--color-quote-border);
      margin: 0.85rem 0;
      padding: 0.25rem 0 0.25rem 1rem;
      color: var(--color-muted);
      background-color: transparent;
    }
    blockquote p { margin: 0.25rem 0; }
    a {
      color: var(--color-link);
      text-decoration: underline;
      text-underline-offset: 2px;
    }
    .table-container {
      width: 100%;
      overflow-x: auto;
      margin: 1rem 0;
      border: 1px solid var(--color-table-border);
      border-radius: 0.5rem;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.875rem;
      text-align: left;
    }
    th, td {
      padding: 0.6rem 0.85rem;
      border-bottom: 1px solid var(--color-table-border);
    }
    th {
      background-color: var(--color-table-header);
      font-weight: 600;
      color: var(--color-text);
    }
    tbody tr:nth-child(even) {
      background-color: var(--color-table-zebra);
    }
    tbody tr:last-child td {
      border-bottom: none;
    }
    details {
      margin: 0.75rem 0;
      border-radius: 0.5rem;
      border: 1px solid var(--color-card-border);
      overflow: hidden;
    }
    details summary {
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 0.55rem 0.85rem;
      user-select: none;
      list-style: none;
      font-size: 0.825rem;
      font-weight: 600;
      transition: background-color 0.15s ease;
    }
    details summary::-webkit-details-marker { display: none; }
    .chevron {
      transition: transform 0.2s ease;
      flex-shrink: 0;
    }
    details[open] .chevron {
      transform: rotate(180deg);
    }
    .reasoning-block {
      background-color: var(--color-reasoning-bg);
      border-color: var(--color-reasoning-border);
    }
    .reasoning-summary {
      color: var(--color-reasoning-text);
    }
    .reasoning-label {
      display: inline-flex;
      align-items: center;
      gap: 0.45rem;
    }
    .reasoning-content {
      padding: 0.75rem 1rem;
      font-size: 0.85rem;
      line-height: 1.6;
      border-top: 1px solid var(--color-reasoning-border);
      white-space: pre-wrap;
      color: var(--color-text-secondary);
    }
    .tool-activity-block {
      background-color: var(--color-tool-bg);
      border-color: var(--color-tool-border);
    }
    .tool-summary {
      color: var(--color-muted);
    }
    .tool-label {
      display: inline-flex;
      align-items: center;
      gap: 0.45rem;
    }
    .tool-name {
      font-weight: 600;
      color: var(--color-text);
    }
    .tool-status {
      font-size: 0.7rem;
      padding: 0.1rem 0.4rem;
      border-radius: 0.25rem;
      font-weight: 600;
      text-transform: capitalize;
    }
    .status-pill-completed { background-color: #dcfce7; color: #166534; }
    .status-pill-running, .status-pill-waitingForUser, .status-pill-awaitingApproval { background-color: #fef3c7; color: #92400e; }
    .status-pill-failed, .status-pill-denied, .status-pill-cancelled { background-color: #fee2e2; color: #991b1b; }
    @media (prefers-color-scheme: dark) {
      .status-pill-completed { background-color: #14532d; color: #86efac; }
      .status-pill-running, .status-pill-waitingForUser, .status-pill-awaitingApproval { background-color: #78350f; color: #fde68a; }
      .status-pill-failed, .status-pill-denied, .status-pill-cancelled { background-color: #7f1d1d; color: #fca5a5; }
    }
    .tool-payload {
      border-top: 1px solid var(--color-tool-border);
      background-color: var(--color-code-bg);
    }
    .status-banner {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.45rem 0.75rem;
      border-radius: 0.375rem;
      margin-bottom: 0.75rem;
      font-size: 0.8rem;
    }
    .status-streaming { background-color: #fef3c7; color: #92400e; }
    .status-failed { background-color: #fee2e2; color: #991b1b; }
    .status-stopped { background-color: #f1f5f9; color: #475569; }
    @media (prefers-color-scheme: dark) {
      .status-streaming { background-color: #78350f; color: #fde68a; }
      .status-failed { background-color: #7f1d1d; color: #fca5a5; }
      .status-stopped { background-color: #1e293b; color: #cbd5e1; }
    }
    hr {
      border: none;
      border-top: 1px solid var(--color-card-border);
      margin: 1.25rem 0;
    }
    .export-footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: 2.5rem;
      padding-top: 1.25rem;
      border-top: 1px solid var(--color-card-border);
      font-size: 0.75rem;
      color: var(--color-muted);
    }
    @media print {
      body {
        background-color: #ffffff !important;
        color: #000000 !important;
        padding: 0 !important;
      }
      .hero-header, .message {
        box-shadow: none !important;
        border: 1px solid #d1d5db !important;
      }
      .message {
        break-inside: avoid;
        page-break-inside: avoid;
      }
      .message-user {
        background-color: #f9fafb !important;
      }
      .export-footer {
        display: none !important;
      }
    }
  </style>
</head>
<body>
  <main class="container">
    <header class="hero-header">
      <div class="brand-row">
        <span class="brand-badge">
          <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
          </svg>
          OpenChat
        </span>
        <span>${escapeHtml(labels.exportTitleBadge)}</span>
      </div>
      <h1 class="conversation-title">${escapeHtml(conversation.title)}</h1>
      <div class="meta-grid">
        <div class="meta-chip">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/>
            <rect x="2" y="14" width="20" height="8" rx="2" ry="2"/>
            <line x1="6" y1="6" x2="6.01" y2="6"/>
            <line x1="6" y1="18" x2="6.01" y2="18"/>
          </svg>
          <span>${escapeHtml(labels.exportProvider)}: <strong>${escapeHtml(labels.provider)}</strong></span>
        </div>
        <div class="meta-chip">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="2"/>
            <rect x="9" y="9" width="6" height="6"/>
            <line x1="9" y1="1" x2="9" y2="4"/>
            <line x1="15" y1="1" x2="15" y2="4"/>
            <line x1="9" y1="20" x2="9" y2="23"/>
            <line x1="15" y1="20" x2="15" y2="23"/>
            <line x1="20" y1="9" x2="23" y2="9"/>
            <line x1="20" y1="14" x2="23" y2="14"/>
            <line x1="1" y1="9" x2="4" y2="9"/>
            <line x1="1" y1="14" x2="4" y2="14"/>
          </svg>
          <span>${escapeHtml(labels.exportModel)}: <strong>${escapeHtml(modelLabel)}</strong></span>
        </div>
        <div class="meta-chip">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
            <line x1="16" y1="2" x2="16" y2="6"/>
            <line x1="8" y1="2" x2="8" y2="6"/>
            <line x1="3" y1="10" x2="21" y2="10"/>
          </svg>
          <span>${escapeHtml(labels.exportCreated)}: <strong>${escapeHtml(createdDate)}</strong></span>
        </div>
        <div class="meta-chip">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
          </svg>
          <span>${escapeHtml(labels.exportMessagesCount)}: <strong>${messages.length}</strong></span>
        </div>
      </div>
    </header>
    <section class="message-list">
      ${renderedMessages}
    </section>
    <footer class="export-footer">
      <span>OpenChat · ${escapeHtml(labels.exportTitleBadge)}</span>
      <span>${escapeHtml(labels.formatDateTime(Date.now()))}</span>
    </footer>
  </main>
</body>
</html>`;
}

export function conversationExportFileName(title: string, format: ExportFormat): string {
  const cleaned = title.trim().replace(/[<>:"/\\|?*]/g, '_');
  const safeName = replaceAsciiControlCharacters(cleaned, '_')
    .replace(/\s+/g, '_')
    .replace(/[. ]+$/g, '');
  const baseName = safeName ? [...safeName].slice(0, 80).join('') : 'conversation';
  return `${baseName}.${format}`;
}

export function conversationMarkdownFileName(title: string): string {
  return conversationExportFileName(title, 'md');
}

export async function exportConversationToFile(
  conversation: ChatConversation,
  format: ExportFormat,
  labels: ConversationMarkdownLabels,
  exportTitle = 'Export conversation',
): Promise<boolean> {
  const messages = await getMessages(conversation.id);
  let content = '';
  if (format === 'md') {
    content = buildConversationMarkdown(conversation, messages, labels);
  } else if (format === 'html') {
    content = buildConversationHtml(conversation, messages, labels);
  } else {
    content = buildConversationJson(conversation, messages);
  }
  const fileName = conversationExportFileName(conversation.title, format);
  return saveConversationExport(fileName, exportTitle, content, format);
}
